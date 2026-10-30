//! Writer for a GTA III save: copy the original bytes and patch the edited spans.

use saveprism_core::{Params, Result, SaveDoc, TypeHint, Value};

use crate::gta3::{CHECKSUM_AT, HINT_PREFIX, original, write_error};

/// Writes the document, keeping every byte that was not edited.
pub(crate) fn encode(doc: &SaveDoc, params: &Params) -> Result<Vec<u8>> {
    let original = original(params)?;
    let checksum_at = params
        .opt_int(CHECKSUM_AT)?
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| write_error("the checksum offset is missing"))?;
    let mut out = original.clone();
    let mut changed = false;
    let mut stack = vec![doc.root()];
    let mut visited = 0usize;
    while let Some(id) = stack.pop() {
        visited = visited.saturating_add(1);
        if visited > doc.len().saturating_add(1) {
            return Err(write_error("the GTA III tree has a cycle"));
        }
        let Some(node) = doc.get(id) else {
            continue;
        };
        stack.extend(doc.children(id));
        if node.flags.contains(saveprism_core::NodeFlags::READ_ONLY) {
            continue;
        }
        let Some(hint) = node.type_hint.as_ref().map(TypeHint::as_str) else {
            continue;
        };
        let Some(kind) = hint.strip_prefix(HINT_PREFIX) else {
            continue;
        };
        if kind.starts_with("u8:") {
            let value = int_value(&node.value)?;
            let value = u8::try_from(value)
                .map_err(|_| write_error(format!("{value} is not an unsigned 8-bit value")))?;
            changed |= write_span(&mut out, node.span, &[value])?;
        } else if kind.starts_with("u32:") {
            let value = int_value(&node.value)?;
            let value = u32::try_from(value)
                .map_err(|_| write_error(format!("{value} is not an unsigned 32-bit value")))?;
            changed |= write_span(&mut out, node.span, &value.to_le_bytes())?;
        }
    }
    if changed {
        let checksum = out
            .get(..checksum_at)
            .ok_or_else(|| write_error("the checksum offset is outside the save"))?
            .iter()
            .fold(0u32, |sum, byte| sum.wrapping_add(u32::from(*byte)));
        let slot = out
            .get_mut(checksum_at..checksum_at.saturating_add(4))
            .ok_or_else(|| write_error("the checksum is outside the save"))?;
        slot.copy_from_slice(&checksum.to_le_bytes());
    }
    Ok(out)
}

fn int_value(value: &Value) -> Result<i128> {
    match value {
        Value::Int { v, .. } => Ok(*v),
        other => Err(write_error(format!("{other:?} is not an integer"))),
    }
}

/// Writes `bytes` at a node span; returns whether any byte changed.
fn write_span(
    out: &mut [u8],
    span: Option<saveprism_core::ByteSpan>,
    bytes: &[u8],
) -> Result<bool> {
    let Some(span) = span else {
        return Err(write_error("an editable value has no byte span"));
    };
    let start =
        usize::try_from(span.start).map_err(|_| write_error("the value offset is too large"))?;
    let end = usize::try_from(span.end).map_err(|_| write_error("the value end is too large"))?;
    let slot = out
        .get_mut(start..end)
        .ok_or_else(|| write_error("a value is outside the original save"))?;
    if slot.len() != bytes.len() {
        return Err(write_error(format!(
            "a value has {} bytes, the file has {}",
            bytes.len(),
            slot.len()
        )));
    }
    let changed = slot != bytes;
    slot.copy_from_slice(bytes);
    Ok(changed)
}
