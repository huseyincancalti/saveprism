//! Parser for a classic PC GTA:SA save: block positions and the fields SavePrism shows.

use saveprism_core::{DecodeCtx, FloatWidth, IntWidth, NodeId, Result, SaveDoc, Value};

use crate::gta_sa::{error_at, push};

/// The five bytes that start every data block.
pub(crate) const BLOCK: &[u8; 5] = b"BLOCK";
/// Classic PC saves are always this many bytes.
pub(crate) const FILE_SIZE: usize = 202_752;
/// The checksum is a little-endian `u32` at the end.
pub(crate) const CHECKSUM_SIZE: usize = 4;
/// The file contains 30 data blocks, then copies of 15 to 17, then a padding block.
pub(crate) const MARKER_COUNT: usize = 34;
/// Block 10 is always this long, which distinguishes the marker list from stray text.
pub(crate) const BLOCK_10_LEN: usize = 18_892;
/// The copy of block 15 starts at this marker.
pub(crate) const MIRROR_15: usize = 30;

/// Offsets needed by the writer.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Positions {
    pub checksum_at: usize,
    /// Offset of the first data byte in the copied block 15.
    pub mirror_15: usize,
}

/// True when the input looks like a classic PC GTA:SA save.
pub(crate) fn has_header(input: &[u8]) -> bool {
    input.len() == FILE_SIZE && input.starts_with(&BLOCK[..])
}

/// Reads the whole save and returns the document and the offsets the writer needs.
pub(crate) fn read_all(input: &[u8], ctx: &DecodeCtx<'_>) -> Result<(SaveDoc, Positions)> {
    if !has_header(input) {
        return Err(error_at("not a classic PC GTA:SA save", 0));
    }
    let starts = scan_blocks(input)?;
    let checksum_at = input.len() - CHECKSUM_SIZE;
    let mirror_15 = starts[MIRROR_15]
        .checked_add(BLOCK.len())
        .ok_or_else(|| error_at("the copied block 15 offset overflows", starts[MIRROR_15]))?;

    let block0 = block_data(input, &starts, 0, 0x89, "block 0")?;
    let block2 = block_data(input, &starts, 2, 0x28, "block 2")?;
    let block15 = block_data(input, &starts, 15, 0x25, "block 15")?;
    let mirror = block_data(input, &starts, MIRROR_15, 0x25, "the copied block 15")?;

    let title = title_at(block0.0, 4, 100);
    let mut doc = ctx.new_doc();
    let title = push(&mut doc, Value::Str(title), "str:title", None, true)?;

    let hour = field_u8_in(
        &mut doc, block0.0, block0.1, 0x86, "block 0", "u8:hour", false,
    )?;
    let minute = field_u8_in(
        &mut doc,
        block0.0,
        block0.1,
        0x87,
        "block 0",
        "u8:minute",
        false,
    )?;
    let day_of_week = field_u8_in(
        &mut doc,
        block0.0,
        block0.1,
        0x88,
        "block 0",
        "u8:day-of-week",
        false,
    )?;
    let time = push(
        &mut doc,
        Value::Object(vec![
            ("hour".to_owned(), hour),
            ("minute".to_owned(), minute),
            ("day_of_week".to_owned(), day_of_week),
        ]),
        "object:game-time",
        None,
        true,
    )?;

    let health = field_f32_in(
        &mut doc,
        block2.0,
        block2.1,
        0x20,
        "block 2",
        "f32:health",
        false,
    )?;
    let armour = field_f32_in(
        &mut doc,
        block2.0,
        block2.1,
        0x24,
        "block 2",
        "f32:armour",
        false,
    )?;
    let max_health = field_u8_in(
        &mut doc,
        block15.0,
        block15.1,
        0x23,
        "block 15",
        "u8:max-health",
        false,
    )?;
    let max_armour = field_u8_in(
        &mut doc,
        block15.0,
        block15.1,
        0x24,
        "block 15",
        "u8:max-armour",
        false,
    )?;
    let money = field_i32_in(
        &mut doc,
        block15.0,
        block15.1,
        0x04,
        "block 15",
        "i32:money",
        false,
    )?;
    let display = field_i32_in(
        &mut doc,
        block15.0,
        block15.1,
        0x10,
        "block 15",
        "i32:money-on-screen",
        false,
    )?;
    let player = push(
        &mut doc,
        Value::Object(vec![
            ("health".to_owned(), health),
            ("armour".to_owned(), armour),
            ("max_health".to_owned(), max_health),
            ("max_armour".to_owned(), max_armour),
            ("money".to_owned(), money),
            ("money_on_screen".to_owned(), display),
        ]),
        "object:player",
        None,
        true,
    )?;

    // The copied block is read so a corrupt copy is refused, but the tree reads the first copy.
    let _ = mirror;

    let root = push(
        &mut doc,
        Value::Object(vec![
            ("title".to_owned(), title),
            ("game_time".to_owned(), time),
            ("player".to_owned(), player),
        ]),
        "object:save",
        None,
        true,
    )?;
    doc.set_root(root);
    Ok((
        doc,
        Positions {
            checksum_at,
            mirror_15,
        },
    ))
}

/// Finds the 34 `BLOCK` markers and checks that block 10 has its required length.
fn scan_blocks(input: &[u8]) -> Result<[usize; MARKER_COUNT]> {
    let mut starts = [0usize; MARKER_COUNT];
    let mut at = 0usize;
    for (index, slot) in starts.iter_mut().enumerate() {
        let found = input
            .get(at..)
            .and_then(|rest| rest.windows(BLOCK.len()).position(|window| window == BLOCK))
            .ok_or_else(|| error_at("the save is missing a BLOCK marker", at))?;
        *slot = at.saturating_add(found);
        at = slot.saturating_add(BLOCK.len());
        if index == 0 && *slot != 0 {
            return Err(error_at("the save does not start with BLOCK", *slot));
        }
    }
    let block_10 = starts[10];
    let block_11 = starts[11];
    let length = block_11
        .checked_sub(block_10)
        .and_then(|value| value.checked_sub(BLOCK.len()))
        .ok_or_else(|| error_at("the block markers are out of order", block_10))?;
    if length != BLOCK_10_LEN {
        return Err(error_at(
            format!("block 10 is {length} bytes, not {BLOCK_10_LEN}"),
            block_10,
        ));
    }
    Ok(starts)
}

/// The data slice for a block and its absolute base offset.
fn block_data<'a>(
    input: &'a [u8],
    starts: &[usize; MARKER_COUNT],
    index: usize,
    needed: usize,
    name: &str,
) -> Result<(&'a [u8], usize)> {
    let start = *starts
        .get(index)
        .ok_or_else(|| error_at(format!("the {name} marker is missing"), 0))?;
    let base = start
        .checked_add(BLOCK.len())
        .ok_or_else(|| error_at(format!("the {name} offset overflows"), start))?;
    let end = starts
        .get(index.saturating_add(1))
        .copied()
        .unwrap_or(input.len().saturating_sub(CHECKSUM_SIZE));
    if end < base || end.saturating_sub(base) < needed {
        return Err(error_at(format!("the {name} body is too short"), start));
    }
    let data = input
        .get(base..end)
        .ok_or_else(|| error_at(format!("the {name} body is cut short"), start))?;
    Ok((data, base))
}

/// Reads the NUL-terminated save name from block 0.
fn title_at(block: &[u8], at: usize, len: usize) -> String {
    let end = at.saturating_add(len).min(block.len());
    let bytes = block.get(at..end).unwrap_or_default();
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(bytes.get(..end).unwrap_or_default()).into_owned()
}

fn field_u8_in(
    doc: &mut SaveDoc,
    block: &[u8],
    base: usize,
    offset: usize,
    block_name: &str,
    hint: &str,
    read_only: bool,
) -> Result<NodeId> {
    let value = block.get(offset).copied().ok_or_else(|| {
        error_at(
            format!("{block_name} field {offset:#x} is cut short"),
            base.saturating_add(offset),
        )
    })?;
    push(
        doc,
        int(i128::from(value), IntWidth::U8),
        hint,
        Some((base + offset, 1)),
        read_only,
    )
}

fn field_i32_in(
    doc: &mut SaveDoc,
    block: &[u8],
    base: usize,
    offset: usize,
    block_name: &str,
    hint: &str,
    read_only: bool,
) -> Result<NodeId> {
    let value = i32_at(block, offset).ok_or_else(|| {
        error_at(
            format!("{block_name} field {offset:#x} is cut short"),
            base.saturating_add(offset),
        )
    })?;
    push(
        doc,
        int(i128::from(value), IntWidth::I32),
        hint,
        Some((base + offset, 4)),
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
            base.saturating_add(offset),
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

fn u32_at(input: &[u8], at: usize) -> Option<u32> {
    input
        .get(at..at.checked_add(4)?)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(u32::from_le_bytes)
}

fn i32_at(input: &[u8], at: usize) -> Option<i32> {
    u32_at(input, at).map(|value| value as i32)
}

fn f32_at(input: &[u8], at: usize) -> Option<f32> {
    u32_at(input, at).map(f32::from_bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_i32_reader_does_not_panic_at_the_end() {
        assert_eq!(i32_at(b"\xff\xff\xff\xff", 0), Some(-1));
        assert_eq!(i32_at(b"\x01\x02\x03", 0), None);
        assert_eq!(i32_at(b"", usize::MAX), None);
    }

    #[test]
    fn a_title_stops_at_a_nul() {
        assert_eq!(title_at(b"\0ABC\0DEF", 0, 8), "");
        assert_eq!(title_at(b"ABC\0DEF", 0, 8), "ABC");
        assert_eq!(title_at(b"ABC", 0, 8), "ABC");
    }
}
