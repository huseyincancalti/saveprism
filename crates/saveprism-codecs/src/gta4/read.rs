//! Parser for a GTA IV save: metadata, 32 `BLOCK` records and the fields SavePrism shows.

use saveprism_core::{DecodeCtx, FloatWidth, IntWidth, NodeId, Result, SaveDoc, Value};

use crate::gta4::{error_at, push};

/// The five bytes that start every data block.
pub(crate) const BLOCK: &[u8; 5] = b"BLOCK";
/// Metadata is always 0x110 bytes.
pub(crate) const META_SIZE: usize = 0x110;
/// The end block after the checksum is always 0x16C bytes.
pub(crate) const END_SIZE: usize = 0x16C;
/// The checksum is a little-endian `u32`.
pub(crate) const CHECKSUM_SIZE: usize = 4;
/// GTA IV writes 32 data blocks.
pub(crate) const BLOCK_COUNT: usize = 32;

/// A block that was read from the file.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Block {
    /// Offset of the `BLOCK` marker.
    pub start: usize,
    /// Size from the header, including the nine header bytes.
    pub size: usize,
}

impl Block {
    /// Offset of the first data byte after `BLOCK` and the size.
    fn data(self) -> usize {
        self.start.saturating_add(9)
    }

    fn data_len(self) -> usize {
        self.size.saturating_sub(9)
    }
}

/// True when the input starts like a GTA IV PC save.
pub(crate) fn has_header(input: &[u8]) -> bool {
    input.get(0x0C..0x10) == Some(b"SAVE")
        && input.get(META_SIZE..META_SIZE + 5) == Some(&BLOCK[..])
        && u32_at(input, META_SIZE + 5) == Some(0xB9)
}

/// Reads the whole save and returns the document and the checksum offset.
pub(crate) fn read_all(input: &[u8], ctx: &DecodeCtx<'_>) -> Result<(SaveDoc, usize)> {
    if !has_header(input) {
        return Err(error_at("not a GTA IV save", 0));
    }
    let version = u32_at(input, 0x00).ok_or_else(|| error_at("the version is cut short", 0x00))?;
    let file_size =
        u32_at(input, 0x04).ok_or_else(|| error_at("the file size is cut short", 0x04))?;
    let globals_size =
        u32_at(input, 0x08).ok_or_else(|| error_at("the globals size is cut short", 0x08))?;
    let last_mission = utf16_at(input, 0x10, 128).unwrap_or_default();

    let mut blocks = Vec::with_capacity(BLOCK_COUNT);
    let mut at = META_SIZE;
    for index in 0..BLOCK_COUNT {
        let block = read_block(input, at, index)?;
        at = at
            .checked_add(block.size)
            .ok_or_else(|| error_at("the block offset overflows", at))?;
        blocks.push(block);
    }
    let checksum_at = at;
    let end_at = checksum_at
        .checked_add(CHECKSUM_SIZE)
        .ok_or_else(|| error_at("the end block offset overflows", checksum_at))?;
    if input.get(end_at..end_at + 4) != Some(&b"END\0"[..]) {
        return Err(error_at(
            "the end block is missing or not marked END",
            end_at,
        ));
    }
    let total = end_at
        .checked_add(END_SIZE)
        .ok_or_else(|| error_at("the file size overflows", end_at))?;
    if input.len() < total {
        return Err(error_at(
            "the save is cut short inside the end block",
            input.len(),
        ));
    }

    let simple = block_data(input, blocks.first().copied(), 0x88, "SimpleVars")?;
    let player = block_data(input, blocks.get(1).copied(), 0x68, "PlayerInfo")?;

    let mut doc = ctx.new_doc();
    let version = push(
        &mut doc,
        int(i128::from(version), IntWidth::U32),
        "u32:version",
        Some((0x00, 4)),
        true,
    )?;
    let file_size = push(
        &mut doc,
        int(i128::from(file_size), IntWidth::U32),
        "u32:file-size",
        Some((0x04, 4)),
        true,
    )?;
    let globals_size = push(
        &mut doc,
        int(i128::from(globals_size), IntWidth::U32),
        "u32:globals-size",
        Some((0x08, 4)),
        true,
    )?;
    let last_mission = push(
        &mut doc,
        Value::Str(last_mission),
        "str:last-mission",
        None,
        true,
    )?;

    let time = time_object(&mut doc, simple)?;
    let player = player_object(&mut doc, player)?;
    let max_wanted = field_u32_in(
        &mut doc,
        simple.0,
        simple.1,
        0x80,
        "SimpleVars",
        "u32:max-wanted-level",
        false,
    )?;
    let max_chaos = field_u32_in(
        &mut doc,
        simple.0,
        simple.1,
        0x84,
        "SimpleVars",
        "u32:max-chaos",
        false,
    )?;

    let root = push(
        &mut doc,
        Value::Object(vec![
            ("version".to_owned(), version),
            ("file_size".to_owned(), file_size),
            ("globals_size".to_owned(), globals_size),
            ("last_mission".to_owned(), last_mission),
            ("game_time".to_owned(), time),
            ("player".to_owned(), player),
            ("max_wanted_level".to_owned(), max_wanted),
            ("max_chaos".to_owned(), max_chaos),
        ]),
        "object:save",
        None,
        true,
    )?;
    doc.set_root(root);
    Ok((doc, checksum_at))
}

/// Reads one block header and checks that the block fits in the input.
fn read_block(input: &[u8], at: usize, index: usize) -> Result<Block> {
    if input.get(at..at + 5) != Some(&BLOCK[..]) {
        return Err(error_at(
            format!("block {index} does not start with BLOCK"),
            at,
        ));
    }
    let size = u32_at(input, at + 5)
        .ok_or_else(|| error_at(format!("block {index} has no size"), at + 5))?;
    let size = usize::try_from(size)
        .map_err(|_| error_at(format!("block {index} is too large"), at + 5))?;
    if size < 9 {
        return Err(error_at(
            format!("block {index} is smaller than its header"),
            at,
        ));
    }
    let end = at
        .checked_add(size)
        .ok_or_else(|| error_at("the block size overflows", at))?;
    if end > input.len() {
        return Err(error_at(
            format!("block {index} is cut short at byte {end}"),
            at,
        ));
    }
    Ok(Block { start: at, size })
}

/// The data slice and its absolute offset, or a parse error when the block is too short.
fn block_data<'a>(
    input: &'a [u8],
    block: Option<Block>,
    needed: usize,
    name: &str,
) -> Result<(&'a [u8], usize)> {
    let Some(block) = block else {
        return Err(error_at(format!("the {name} block is missing"), META_SIZE));
    };
    if block.data_len() < needed {
        return Err(error_at(
            format!("the {name} block is too short"),
            block.start,
        ));
    }
    let data = input
        .get(block.data()..block.data() + block.data_len())
        .ok_or_else(|| error_at(format!("the {name} block is cut short"), block.start))?;
    Ok((data, block.data()))
}

/// The `game_time` object: the in-game calendar fields from SimpleVars.
fn time_object(doc: &mut SaveDoc, simple: (&[u8], usize)) -> Result<NodeId> {
    let fields = [
        ("month", 0x28, "u32:month"),
        ("day", 0x2C, "u32:day"),
        ("hour", 0x30, "u32:hour"),
        ("minute", 0x34, "u32:minute"),
        ("day_of_week", 0x38, "u32:day-of-week"),
    ];
    let mut entries = Vec::with_capacity(fields.len());
    for (key, offset, hint) in fields {
        let value = field_u32_in(doc, simple.0, simple.1, offset, "SimpleVars", hint, false)?;
        entries.push((key.to_owned(), value));
    }
    push(doc, Value::Object(entries), "object:game-time", None, true)
}

/// The `player` object: money, health and armour from PlayerInfo.
fn player_object(doc: &mut SaveDoc, player: (&[u8], usize)) -> Result<NodeId> {
    let money = field_u32_in(
        doc,
        player.0,
        player.1,
        0x18,
        "PlayerInfo",
        "u32:money",
        false,
    )?;
    let display = field_u32_in(
        doc,
        player.0,
        player.1,
        0x20,
        "PlayerInfo",
        "u32:money-on-screen",
        false,
    )?;
    let max_health = field_u16_in(
        doc,
        player.0,
        player.1,
        0x34,
        "PlayerInfo",
        "u16:max-health",
        false,
    )?;
    let max_armour = field_u16_in(
        doc,
        player.0,
        player.1,
        0x36,
        "PlayerInfo",
        "u16:max-armour",
        false,
    )?;
    let health = field_f32_in(
        doc,
        player.0,
        player.1,
        0x60,
        "PlayerInfo",
        "f32:health",
        false,
    )?;
    let armour = field_f32_in(
        doc,
        player.0,
        player.1,
        0x64,
        "PlayerInfo",
        "f32:armour",
        false,
    )?;
    push(
        doc,
        Value::Object(vec![
            ("money".to_owned(), money),
            ("money_on_screen".to_owned(), display),
            ("max_health".to_owned(), max_health),
            ("max_armour".to_owned(), max_armour),
            ("health".to_owned(), health),
            ("armour".to_owned(), armour),
        ]),
        "object:player",
        None,
        true,
    )
}

fn field_u32_in(
    doc: &mut SaveDoc,
    block: &[u8],
    base: usize,
    offset: usize,
    block_name: &str,
    hint: &str,
    read_only: bool,
) -> Result<NodeId> {
    let value = u32_at(block, offset).ok_or_else(|| {
        error_at(
            format!("{block_name} field {offset:#x} is cut short"),
            offset,
        )
    })?;
    push(
        doc,
        int(i128::from(value), IntWidth::U32),
        hint,
        Some((base + offset, 4)),
        read_only,
    )
}

fn field_u16_in(
    doc: &mut SaveDoc,
    block: &[u8],
    base: usize,
    offset: usize,
    block_name: &str,
    hint: &str,
    read_only: bool,
) -> Result<NodeId> {
    let value = u16_at(block, offset).ok_or_else(|| {
        error_at(
            format!("{block_name} field {offset:#x} is cut short"),
            offset,
        )
    })?;
    push(
        doc,
        int(i128::from(value), IntWidth::U16),
        hint,
        Some((base + offset, 2)),
        read_only,
    )
}

fn field_f32_in(
    doc: &mut SaveDoc,
    block: &[u8],
    base: usize,
    offset: usize,
    block_name: &str,
    hint: &str,
    read_only: bool,
) -> Result<NodeId> {
    let value = f32_at(block, offset).ok_or_else(|| {
        error_at(
            format!("{block_name} field {offset:#x} is cut short"),
            offset,
        )
    })?;
    push(
        doc,
        Value::Float {
            v: f64::from(value),
            width: FloatWidth::F32,
        },
        hint,
        Some((base + offset, 4)),
        read_only,
    )
}

fn int(value: i128, width: IntWidth) -> Value {
    Value::Int { v: value, width }
}

/// Reads a little-endian `u32` without panicking.
pub(crate) fn u32_at(input: &[u8], at: usize) -> Option<u32> {
    input
        .get(at..at.checked_add(4)?)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(u32::from_le_bytes)
}

fn u16_at(input: &[u8], at: usize) -> Option<u16> {
    input
        .get(at..at.checked_add(2)?)
        .and_then(|bytes| <[u8; 2]>::try_from(bytes).ok())
        .map(u16::from_le_bytes)
}

fn f32_at(input: &[u8], at: usize) -> Option<f32> {
    u32_at(input, at).map(f32::from_bits)
}

/// Decodes a NUL-terminated UTF-16LE string of at most `chars` code units.
fn utf16_at(input: &[u8], at: usize, chars: usize) -> Option<String> {
    let bytes = input.get(at..at.checked_add(chars.checked_mul(2)?)?)?;
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .take_while(|unit| *unit != 0)
        .collect();
    Some(String::from_utf16_lossy(&units))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_u32_reader_does_not_panic_at_the_end() {
        assert_eq!(u32_at(b"\x01\x02\x03\x04", 0), Some(0x0403_0201));
        assert_eq!(u32_at(b"\x01\x02\x03", 0), None);
        assert_eq!(u32_at(b"", usize::MAX), None);
    }

    #[test]
    fn utf16_stops_at_a_nul() {
        let text = utf16_at(
            &[
                0x41, 0, 0x42, 0, 0, 0, 0x43, 0, 0x44, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
            0,
            10,
        );
        assert_eq!(text.as_deref(), Some("AB"));
    }
}
