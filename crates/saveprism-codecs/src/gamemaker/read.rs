//! Parser for GameMaker `ds_map_write` and `ds_list_write` hexadecimal dumps.

use saveprism_core::{
    Error, FloatWidth, IntWidth, Node, NodeFlags, NodeId, Result, SaveDoc, TypeHint, Value, codes,
};

use super::{GM_PARSE, PREFIX};

/// The type codes used by GameMaker's `variable_WriteValue`.
pub(crate) const VALUE_REAL: u32 = 0;
pub(crate) const VALUE_STRING: u32 = 1;
pub(crate) const VALUE_ARRAY: u32 = 2;
pub(crate) const VALUE_PTR: u32 = 3;
pub(crate) const VALUE_UNDEFINED: u32 = 5;
pub(crate) const VALUE_INT32: u32 = 7;
pub(crate) const VALUE_INT64: u32 = 10;
pub(crate) const VALUE_BOOL: u32 = 13;

/// Converts the hexadecimal text into bytes.
pub(crate) fn hex_bytes(text: &str) -> Result<Vec<u8>> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return Err(Error::new(
            GM_PARSE,
            "the GameMaker ds_map dump has an odd number of hex digits",
        ));
    }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.as_chunks::<2>().0 {
        let digits = std::str::from_utf8(pair)
            .map_err(|_| Error::new(GM_PARSE, "the GameMaker ds_map dump is not ASCII"))?;
        let byte = u8::from_str_radix(digits, 16)
            .map_err(|_| Error::new(GM_PARSE, "the GameMaker ds_map dump has a bad hex digit"))?;
        out.push(byte);
    }
    Ok(out)
}

pub(crate) fn magic(bytes: &[u8]) -> Option<u32> {
    let array: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
    Some(u32::from_le_bytes(array))
}

pub(crate) fn version(magic: u32) -> Option<u32> {
    match magic {
        402 | 302 => Some(3),
        403 | 303 => Some(0),
        _ => None,
    }
}

/// Parses the bytes of a serialized map into a read-only `Map` root.
pub(crate) fn parse_map(bytes: &[u8], doc: &mut SaveDoc, max_nodes: u64) -> Result<NodeId> {
    let magic =
        magic(bytes).ok_or_else(|| Error::new(GM_PARSE, "the GameMaker ds_map is too short"))?;
    let version = version(magic).ok_or_else(|| {
        Error::new(
            GM_PARSE,
            format!("the GameMaker ds_map magic is {magic}, not 402 or 403"),
        )
    })?;
    let mut cursor = Cursor {
        data: bytes,
        pos: 4,
    };
    let count = u64::from(cursor.u32()?);
    if count.saturating_mul(2) > max_nodes {
        return Err(Error::new(
            codes::CORE_LIMIT_NODES,
            format!("the GameMaker ds_map has {count} entries (node limit)"),
        ));
    }
    let Ok(count) = usize::try_from(count) else {
        return Err(Error::new(
            GM_PARSE,
            "the GameMaker ds_map count is too large",
        ));
    };
    let mut entries = Vec::with_capacity(count.min(1024));
    for _ in 0..count {
        let key = cursor.object(doc, true, version)?;
        let value = cursor.object(doc, false, version)?;
        entries.push((key, value));
    }
    cursor.finish(bytes.len())?;
    let mut root = Node::new(Value::Map(entries));
    root.type_hint = Some(TypeHint::new(format!("{PREFIX}map")));
    root.flags.insert(NodeFlags::READ_ONLY);
    doc.push(root)
}

/// Parses the bytes of a serialized list into a read-only `Array` root.
pub(crate) fn parse_list(bytes: &[u8], doc: &mut SaveDoc, max_nodes: u64) -> Result<NodeId> {
    let magic =
        magic(bytes).ok_or_else(|| Error::new(GM_PARSE, "the GameMaker ds_list is too short"))?;
    let version = version(magic).ok_or_else(|| {
        Error::new(
            GM_PARSE,
            format!("the GameMaker ds_list magic is {magic}, not 302 or 303"),
        )
    })?;
    let mut cursor = Cursor {
        data: bytes,
        pos: 4,
    };
    let count = u64::from(cursor.u32()?);
    if count > max_nodes {
        return Err(Error::new(
            codes::CORE_LIMIT_NODES,
            format!("the GameMaker ds_list has {count} entries (node limit)"),
        ));
    }
    let Ok(count) = usize::try_from(count) else {
        return Err(Error::new(
            GM_PARSE,
            "the GameMaker ds_list count is too large",
        ));
    };
    let mut items = Vec::with_capacity(count.min(1024));
    for _ in 0..count {
        items.push(cursor.object(doc, false, version)?);
    }
    cursor.finish(bytes.len())?;
    let mut root = Node::new(Value::Array(items));
    root.type_hint = Some(TypeHint::new(format!("{PREFIX}list")));
    root.flags.insert(NodeFlags::READ_ONLY);
    doc.push(root)
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Cursor<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8]> {
        let end = self
            .pos
            .checked_add(count)
            .ok_or_else(|| Error::new(GM_PARSE, "GameMaker offset overflow"))?;
        let slice = self.data.get(self.pos..end).ok_or_else(|| {
            Error::new(
                GM_PARSE,
                format!("the GameMaker data is cut short at byte {}", self.pos),
            )
        })?;
        self.pos = end;
        Ok(slice)
    }

    fn u32(&mut self) -> Result<u32> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| Error::new(GM_PARSE, "the GameMaker data is cut short"))?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn u64(&mut self) -> Result<u64> {
        let bytes: [u8; 8] = self
            .take(8)?
            .try_into()
            .map_err(|_| Error::new(GM_PARSE, "the GameMaker data is cut short"))?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn finish(&self, len: usize) -> Result<()> {
        if self.pos != len {
            return Err(Error::new(
                GM_PARSE,
                format!(
                    "{} bytes follow the last GameMaker entry",
                    len.saturating_sub(self.pos)
                ),
            ));
        }
        Ok(())
    }

    fn hint(key: bool, kind: &str) -> String {
        if key {
            format!("{PREFIX}key:{kind}")
        } else {
            format!("{PREFIX}{kind}")
        }
    }

    fn push(
        &mut self,
        doc: &mut SaveDoc,
        value: Value,
        hint: String,
        read_only: bool,
    ) -> Result<NodeId> {
        let mut node = Node::new(value);
        node.type_hint = Some(TypeHint::new(hint));
        if read_only {
            node.flags.insert(NodeFlags::READ_ONLY);
        }
        doc.push(node)
    }

    fn object(&mut self, doc: &mut SaveDoc, key: bool, version: u32) -> Result<NodeId> {
        let kind = self.u32()?;
        match kind {
            VALUE_REAL => {
                let value = f64::from_bits(self.u64()?);
                self.push(
                    doc,
                    Value::Float {
                        v: value,
                        width: FloatWidth::F64,
                    },
                    Self::hint(key, "real"),
                    key,
                )
            }
            VALUE_STRING => {
                let length = usize::try_from(self.u32()?)
                    .map_err(|_| Error::new(GM_PARSE, "a GameMaker string is too long"))?;
                let data = self.take(length)?.to_vec();
                if key {
                    let text = std::str::from_utf8(&data)
                        .map_err(|_| Error::new(GM_PARSE, "a GameMaker map key is not UTF-8"))?;
                    self.push(
                        doc,
                        Value::Str(text.to_owned()),
                        Self::hint(true, "string"),
                        true,
                    )
                } else {
                    match std::str::from_utf8(&data) {
                        Ok(text) => self.push(
                            doc,
                            Value::Str(text.to_owned()),
                            Self::hint(false, "string"),
                            false,
                        ),
                        Err(_) => {
                            self.push(doc, Value::Bytes(data), Self::hint(false, "string"), true)
                        }
                    }
                }
            }
            VALUE_ARRAY if key => Err(Error::new(
                GM_PARSE,
                "a GameMaker map key cannot be an array",
            )),
            VALUE_ARRAY => self.array(doc, version),
            VALUE_PTR => {
                let value = self.u64()? as i64;
                self.push(
                    doc,
                    Value::Int {
                        v: i128::from(value),
                        width: IntWidth::I64,
                    },
                    Self::hint(key, "ptr"),
                    key,
                )
            }
            VALUE_UNDEFINED => self.push(doc, Value::Null, Self::hint(key, "undefined"), true),
            VALUE_INT32 => {
                let value = self.u32()? as i32;
                self.push(
                    doc,
                    Value::Int {
                        v: i128::from(value),
                        width: IntWidth::I32,
                    },
                    Self::hint(key, "int32"),
                    key,
                )
            }
            VALUE_INT64 => {
                let value = self.u64()? as i64;
                self.push(
                    doc,
                    Value::Int {
                        v: i128::from(value),
                        width: IntWidth::I64,
                    },
                    Self::hint(key, "int64"),
                    key,
                )
            }
            VALUE_BOOL => {
                let value = f64::from_bits(self.u64()?) != 0.0;
                self.push(doc, Value::Bool(value), Self::hint(key, "bool"), key)
            }
            other => Err(Error::new(
                GM_PARSE,
                format!("unsupported GameMaker value type {other}"),
            )),
        }
    }

    fn array(&mut self, doc: &mut SaveDoc, version: u32) -> Result<NodeId> {
        if version == 3 {
            let outer = self.u32()?;
            if outer == 1 {
                let count = usize::try_from(self.u32()?)
                    .map_err(|_| Error::new(GM_PARSE, "a GameMaker array is too long"))?;
                let mut items = Vec::with_capacity(count.min(1024));
                for _ in 0..count {
                    items.push(self.object(doc, false, version)?);
                }
                let mut node = Node::new(Value::Array(items));
                node.type_hint = Some(TypeHint::new(format!("{PREFIX}array")));
                node.flags.insert(NodeFlags::READ_ONLY);
                doc.push(node)
            } else {
                let outer = usize::try_from(outer)
                    .map_err(|_| Error::new(GM_PARSE, "a GameMaker array is too long"))?;
                let mut rows = Vec::with_capacity(outer.min(1024));
                for _ in 0..outer {
                    let count = usize::try_from(self.u32()?)
                        .map_err(|_| Error::new(GM_PARSE, "a GameMaker array row is too long"))?;
                    let mut items = Vec::with_capacity(count.min(1024));
                    for _ in 0..count {
                        items.push(self.object(doc, false, version)?);
                    }
                    let mut row = Node::new(Value::Array(items));
                    row.type_hint = Some(TypeHint::new(format!("{PREFIX}array")));
                    row.flags.insert(NodeFlags::READ_ONLY);
                    rows.push(doc.push(row)?);
                }
                let mut node = Node::new(Value::Array(rows));
                node.type_hint = Some(TypeHint::new(format!("{PREFIX}array2d")));
                node.flags.insert(NodeFlags::READ_ONLY);
                doc.push(node)
            }
        } else {
            let count = usize::try_from(self.u32()?)
                .map_err(|_| Error::new(GM_PARSE, "a GameMaker array is too long"))?;
            let mut items = Vec::with_capacity(count.min(1024));
            for _ in 0..count {
                items.push(self.object(doc, false, version)?);
            }
            let mut node = Node::new(Value::Array(items));
            node.type_hint = Some(TypeHint::new(format!("{PREFIX}array")));
            node.flags.insert(NodeFlags::READ_ONLY);
            doc.push(node)
        }
    }
}
