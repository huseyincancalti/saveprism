//! Canonical writer for GameMaker `ds_map` and `ds_list` trees (uppercase hexadecimal text).

use saveprism_core::{Error, Node, NodeId, Result, SaveDoc, Value};

use super::GM_WRITE;
use super::read::{
    VALUE_ARRAY, VALUE_BOOL, VALUE_INT32, VALUE_INT64, VALUE_PTR, VALUE_REAL, VALUE_STRING,
    VALUE_UNDEFINED,
};

#[derive(Clone, Copy)]
pub(crate) struct Layout {
    pub magic: u32,
    pub version: u32,
}

impl Layout {
    pub(crate) fn new(magic: u32) -> Self {
        let version = match magic {
            402 | 302 => 3,
            403 | 303 => 0,
            _ => 0,
        };
        Self { magic, version }
    }
}

fn error(message: impl Into<String>) -> Error {
    Error::new(GM_WRITE, message)
}

fn node(doc: &SaveDoc, id: NodeId) -> Result<&Node> {
    doc.get(id)
        .ok_or_else(|| error("a GameMaker entry is missing"))
}

fn hint(node: &Node) -> Result<&str> {
    let hint = node
        .type_hint
        .as_ref()
        .map_or("", saveprism_core::TypeHint::as_str);
    if let Some(kind) = hint.strip_prefix("GM:key:") {
        Ok(kind)
    } else {
        hint.strip_prefix("GM:")
            .ok_or_else(|| error("a GameMaker node has the wrong type hint"))
    }
}

/// Renders a document as a `ds_map_write` hexadecimal dump.
pub(crate) fn render_map(doc: &SaveDoc, layout: Layout) -> Result<Vec<u8>> {
    let root = node(doc, doc.root())?;
    let Value::Map(entries) = &root.value else {
        return Err(error("the GameMaker ds_map root is not a map"));
    };
    let count = u32::try_from(entries.len())
        .map_err(|_| error("the GameMaker ds_map has too many entries"))?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&layout.magic.to_le_bytes());
    bytes.extend_from_slice(&count.to_le_bytes());
    for (key, value) in entries {
        render_object(doc, *key, true, layout, &mut bytes)?;
        render_object(doc, *value, false, layout, &mut bytes)?;
    }
    Ok(hex(&bytes))
}

/// Renders a document as a `ds_list_write` hexadecimal dump.
pub(crate) fn render_list(doc: &SaveDoc, layout: Layout) -> Result<Vec<u8>> {
    let root = node(doc, doc.root())?;
    let Value::Array(items) = &root.value else {
        return Err(error("the GameMaker ds_list root is not a list"));
    };
    let count = u32::try_from(items.len())
        .map_err(|_| error("the GameMaker ds_list has too many entries"))?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&layout.magic.to_le_bytes());
    bytes.extend_from_slice(&count.to_le_bytes());
    for item in items {
        render_object(doc, *item, false, layout, &mut bytes)?;
    }
    Ok(hex(&bytes))
}

fn hex(bytes: &[u8]) -> Vec<u8> {
    let mut text = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        text.push_str(&format!("{byte:02X}"));
    }
    text.into_bytes()
}

fn render_object(
    doc: &SaveDoc,
    id: NodeId,
    key: bool,
    layout: Layout,
    out: &mut Vec<u8>,
) -> Result<()> {
    let current = node(doc, id)?;
    let kind = hint(current)?;
    if key
        && current
            .type_hint
            .as_ref()
            .is_none_or(|hint| !hint.as_str().starts_with("GM:key:"))
    {
        return Err(error("a GameMaker map key has a value hint"));
    }
    match (kind, &current.value) {
        ("real", Value::Float { v, .. }) => render_real(*v, out),
        ("real", Value::Int { v, .. }) => render_real(*v as f64, out),
        ("string", Value::Str(text)) => render_string(text.as_bytes(), out)?,
        ("string", Value::Bytes(bytes)) => render_string(bytes, out)?,
        ("array", Value::Array(items)) => {
            out.extend_from_slice(&VALUE_ARRAY.to_le_bytes());
            if layout.version == 3 {
                out.extend_from_slice(&1_u32.to_le_bytes());
            }
            render_array_items(doc, items.as_slice(), layout, out)?;
        }
        ("array2d", Value::Array(rows)) => {
            if rows.len() == 1 {
                return Err(error(
                    "a one-row GameMaker 2D array cannot be told apart from a 1D array",
                ));
            }
            out.extend_from_slice(&VALUE_ARRAY.to_le_bytes());
            if layout.version != 3 {
                return Err(error("a GameMaker 2D array needs the version 3 layout"));
            }
            let count = u32::try_from(rows.len())
                .map_err(|_| error("a GameMaker array has too many rows"))?;
            out.extend_from_slice(&count.to_le_bytes());
            for row in rows {
                let Value::Array(items) = &node(doc, *row)?.value else {
                    return Err(error("a GameMaker 2D array row is not an array"));
                };
                render_array_items(doc, items.as_slice(), layout, out)?;
            }
        }
        ("ptr", Value::Int { v, .. }) => {
            let value =
                i64::try_from(*v).map_err(|_| error("a GameMaker pointer is out of range"))?;
            out.extend_from_slice(&VALUE_PTR.to_le_bytes());
            out.extend_from_slice(&value.to_le_bytes());
        }
        ("int32", Value::Int { v, .. }) => {
            let value =
                i32::try_from(*v).map_err(|_| error("a GameMaker int32 is out of range"))?;
            out.extend_from_slice(&VALUE_INT32.to_le_bytes());
            out.extend_from_slice(&value.to_le_bytes());
        }
        ("int64", Value::Int { v, .. }) => {
            let value =
                i64::try_from(*v).map_err(|_| error("a GameMaker int64 is out of range"))?;
            out.extend_from_slice(&VALUE_INT64.to_le_bytes());
            out.extend_from_slice(&value.to_le_bytes());
        }
        ("bool", Value::Bool(value)) => {
            out.extend_from_slice(&VALUE_BOOL.to_le_bytes());
            out.extend_from_slice(
                &if *value { 1.0_f64 } else { 0.0_f64 }
                    .to_bits()
                    .to_le_bytes(),
            );
        }
        ("undefined", Value::Null) => {
            out.extend_from_slice(&VALUE_UNDEFINED.to_le_bytes());
        }
        _ => {
            return Err(error(format!(
                "a GameMaker {} has an unsupported type (hint {kind})",
                if key { "key" } else { "value" }
            )));
        }
    }
    Ok(())
}

fn render_array_items(
    doc: &SaveDoc,
    items: &[NodeId],
    layout: Layout,
    out: &mut Vec<u8>,
) -> Result<()> {
    let count =
        u32::try_from(items.len()).map_err(|_| error("a GameMaker array has too many entries"))?;
    out.extend_from_slice(&count.to_le_bytes());
    for item in items {
        render_object(doc, *item, false, layout, out)?;
    }
    Ok(())
}

fn render_real(value: f64, out: &mut Vec<u8>) {
    out.extend_from_slice(&VALUE_REAL.to_le_bytes());
    out.extend_from_slice(&value.to_bits().to_le_bytes());
}

fn render_string(bytes: &[u8], out: &mut Vec<u8>) -> Result<()> {
    let length = u32::try_from(bytes.len()).map_err(|_| error("a GameMaker string is too long"))?;
    out.extend_from_slice(&VALUE_STRING.to_le_bytes());
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}
