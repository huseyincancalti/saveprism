//! Parser for a classic PC GTA III save: blocks and the fields SavePrism shows.

use saveprism_core::{DecodeCtx, IntWidth, NodeId, Result, SaveDoc, Value};

use crate::gta3::{error_at, push};

/// Classic PC saves are always this many bytes.
pub(crate) const FILE_SIZE: usize = 201_820;
/// The checksum is a little-endian `u32` at the end.
pub(crate) const CHECKSUM_SIZE: usize = 4;
/// Blocks 0 to 19 hold data; blocks 20 and 21 are padding.
pub(crate) const BLOCK_COUNT: usize = 22;

/// True when the input looks like a classic PC GTA III save.
pub(crate) fn has_header(input: &[u8]) -> bool {
    if input.len() != FILE_SIZE {
        return false;
    }
    let Some(size) = u32_at(input, 0) else {
        return false;
    };
    size >= 0x100 && usize::try_from(size).is_ok_and(|size| size < FILE_SIZE - CHECKSUM_SIZE)
}

/// Reads the whole save and returns the document and the checksum offset.
pub(crate) fn read_all(input: &[u8], ctx: &DecodeCtx<'_>) -> Result<(SaveDoc, usize)> {
    if !has_header(input) {
        return Err(error_at("not a classic PC GTA III save", 0));
    }
    let (starts, checksum_at) = scan_blocks(input)?;

    let block0 = block_body(input, &starts, 0, 0, 0x61, "block 0")?;
    let block1 = block_body(input, &starts, 1, 4, 1538, "block 1")?;
    let block16 = block_body(input, &starts, 16, 4, 0x13, "block 16")?;

    let title = utf16_at(block0.0, 0, 24).unwrap_or_default();
    let hour = block0
        .0
        .get(0x5C)
        .copied()
        .ok_or_else(|| error_at("the game hour is cut short", block0.1.saturating_add(0x5C)))?;
    let minute = block0.0.get(0x60).copied().ok_or_else(|| {
        error_at(
            "the game minute is cut short",
            block0.1.saturating_add(0x60),
        )
    })?;

    let mut doc = ctx.new_doc();
    let title = push(&mut doc, Value::Str(title), "str:title", None, true)?;
    let island = field_u32_in(
        &mut doc,
        block0.0,
        block0.1,
        0x44,
        "block 0",
        "u32:current-island",
        true,
    )?;
    let hour = push(
        &mut doc,
        int(i128::from(hour), IntWidth::U8),
        "u8:hour",
        Some((block0.1 + 0x5C, 1)),
        false,
    )?;
    let minute = push(
        &mut doc,
        int(i128::from(minute), IntWidth::U8),
        "u8:minute",
        Some((block0.1 + 0x60, 1)),
        false,
    )?;
    let time = push(
        &mut doc,
        Value::Object(vec![
            ("hour".to_owned(), hour),
            ("minute".to_owned(), minute),
        ]),
        "object:game-time",
        None,
        true,
    )?;

    // The player record starts after the player count; the wanted values follow the 0x5F0-byte
    // CPed dump.
    let wanted_at = 0x04usize.saturating_add(1530);
    let chaos_at = 0x04usize.saturating_add(1534);
    let max_wanted = field_u32_in(
        &mut doc,
        block1.0,
        block1.1,
        wanted_at,
        "block 1",
        "u32:max-wanted-level",
        false,
    )?;
    let max_chaos = field_u32_in(
        &mut doc,
        block1.0,
        block1.1,
        chaos_at,
        "block 1",
        "u32:max-chaos",
        false,
    )?;

    let money = field_u32_in(
        &mut doc,
        block16.0,
        block16.1,
        0x00,
        "block 16",
        "u32:money",
        false,
    )?;
    let display = field_u32_in(
        &mut doc,
        block16.0,
        block16.1,
        0x0F,
        "block 16",
        "u32:money-on-screen",
        false,
    )?;
    let player = push(
        &mut doc,
        Value::Object(vec![
            ("money".to_owned(), money),
            ("money_on_screen".to_owned(), display),
        ]),
        "object:player",
        None,
        true,
    )?;

    let root = push(
        &mut doc,
        Value::Object(vec![
            ("title".to_owned(), title),
            ("current_island".to_owned(), island),
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

/// Walks the 22 top-level blocks and returns their offsets and the checksum offset.
fn scan_blocks(input: &[u8]) -> Result<([usize; BLOCK_COUNT], usize)> {
    let checksum_at = input.len() - CHECKSUM_SIZE;
    let mut starts = [0usize; BLOCK_COUNT];
    let mut at = 0usize;
    for (index, slot) in starts.iter_mut().enumerate() {
        *slot = at;
        let size =
            u32_at(input, at).ok_or_else(|| error_at(format!("block {index} has no size"), at))?;
        let size = usize::try_from(size)
            .map_err(|_| error_at(format!("block {index} is too large"), at))?;
        if size < 4 {
            return Err(error_at(
                format!("block {index} is smaller than its size field"),
                at,
            ));
        }
        at = at
            .checked_add(4)
            .and_then(|value| value.checked_add(size))
            .ok_or_else(|| error_at("the block offset overflows", at))?;
        if at > checksum_at {
            return Err(error_at(
                format!("block {index} runs into the checksum"),
                *slot,
            ));
        }
    }
    if at != checksum_at {
        return Err(error_at(
            format!("the blocks end at byte {at}, not {checksum_at}"),
            at,
        ));
    }
    Ok((starts, checksum_at))
}

/// The body of a block and its absolute offset. `extra` skips the nested size field.
fn block_body<'a>(
    input: &'a [u8],
    starts: &[usize; BLOCK_COUNT],
    index: usize,
    extra: usize,
    needed: usize,
    name: &str,
) -> Result<(&'a [u8], usize)> {
    let start = *starts
        .get(index)
        .ok_or_else(|| error_at(format!("the {name} marker is missing"), 0))?;
    let body = start
        .checked_add(4)
        .and_then(|value| value.checked_add(extra))
        .ok_or_else(|| error_at(format!("the {name} offset overflows"), start))?;
    let end = starts
        .get(index.saturating_add(1))
        .copied()
        .unwrap_or_else(|| input.len().saturating_sub(CHECKSUM_SIZE));
    if end < body || end.saturating_sub(body) < needed {
        return Err(error_at(format!("the {name} body is too short"), start));
    }
    let data = input
        .get(body..end)
        .ok_or_else(|| error_at(format!("the {name} body is cut short"), start))?;
    Ok((data, body))
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
            base.saturating_add(offset),
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

fn int(value: i128, width: IntWidth) -> Value {
    Value::Int { v: value, width }
}

fn u32_at(input: &[u8], at: usize) -> Option<u32> {
    input
        .get(at..at.checked_add(4)?)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(u32::from_le_bytes)
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
