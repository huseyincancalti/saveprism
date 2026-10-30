//! The NBT reader (see the module documentation for the tree and parameters).

use saveprism_core::limits::len_u64;
use saveprism_core::{
    Error, FloatWidth, IntWidth, Limits, Node, NodeFlags, NodeId, Params, Result, SaveDoc,
    TypeHint, Value, codes,
};

use super::{ENDIAN, HEADER, HINT_PREFIX, NBT_PARSE, ROOT_NAME, VERSION};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Endian {
    Big,
    Little,
}

impl Endian {
    fn name(self) -> &'static str {
        match self {
            Endian::Big => "big",
            Endian::Little => "little",
        }
    }
}

fn parse_error(message: impl Into<String>, at: usize) -> Error {
    Error::new(NBT_PARSE, message).with_offset(len_u64(at))
}

fn hint(kind: &str) -> TypeHint {
    TypeHint::new(format!("{HINT_PREFIX}{kind}"))
}

fn le_i32_at(input: &[u8], at: usize) -> Option<i32> {
    let bytes: [u8; 4] = input.get(at..at.checked_add(4)?)?.try_into().ok()?;
    Some(i32::from_le_bytes(bytes))
}

pub(super) fn decode(input: &[u8], limits: &Limits) -> Result<(SaveDoc, Params)> {
    if input.len() >= 8
        && let (Some(version), Some(size)) = (le_i32_at(input, 0), le_i32_at(input, 4))
        && size >= 0
        && usize::try_from(size).ok() == Some(input.len() - 8)
        && (0..=100_000).contains(&version)
        && let Ok((doc, root_name)) = parse_at(input, limits, Endian::Little, 8)
    {
        let mut params = Params::new();
        params.set(ENDIAN, "little");
        params.set(HEADER, true);
        params.set(VERSION, i64::from(version));
        params.set(ROOT_NAME, root_name);
        return Ok((doc, params));
    }
    for endian in [Endian::Big, Endian::Little] {
        if let Ok((doc, root_name)) = parse_at(input, limits, endian, 0) {
            let mut params = Params::new();
            params.set(ENDIAN, endian.name());
            params.set(HEADER, false);
            params.set(ROOT_NAME, root_name);
            return Ok((doc, params));
        }
    }
    Err(parse_error("not a complete NBT tree", 0))
}

/// True when a complete NBT tree was parsed (any supported layout).
pub(super) fn plausible(input: &[u8], limits: &Limits) -> bool {
    decode(input, limits).is_ok()
}

fn parse_at(
    input: &[u8],
    limits: &Limits,
    endian: Endian,
    offset: usize,
) -> Result<(SaveDoc, String)> {
    let mut doc = SaveDoc::with_node_limit(limits.max_nodes);
    let (root, name) = parse_into(input, limits, endian, offset, &mut doc)?;
    doc.set_root(root);
    Ok((doc, name))
}

/// Parses one big-endian root tag into an existing document. The whole input must be the tag.
pub(super) fn parse_java_into(
    input: &[u8],
    limits: &Limits,
    doc: &mut SaveDoc,
) -> Result<(NodeId, String)> {
    parse_into(input, limits, Endian::Big, 0, doc)
}

fn parse_into(
    input: &[u8],
    limits: &Limits,
    endian: Endian,
    offset: usize,
    doc: &mut SaveDoc,
) -> Result<(NodeId, String)> {
    let mut parser = Parser {
        input,
        pos: offset,
        endian,
        limits,
        doc,
    };
    let (root, name) = parser.root()?;
    if parser.pos != input.len() {
        return Err(parse_error(
            "the NBT data continues after the root tag",
            parser.pos,
        ));
    }
    Ok((root, name))
}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
    endian: Endian,
    limits: &'a Limits,
    doc: &'a mut SaveDoc,
}

impl Parser<'_> {
    fn take(&mut self, count: usize, what: &str) -> Result<&[u8]> {
        let start = self.pos;
        let end = start
            .checked_add(count)
            .ok_or_else(|| parse_error(format!("the {what} is too long"), start))?;
        let slice = self
            .input
            .get(start..end)
            .ok_or_else(|| parse_error(format!("the {what} is cut short"), start))?;
        self.pos = end;
        Ok(slice)
    }

    fn byte(&mut self, what: &str) -> Result<u8> {
        let bytes = self.take(1, what)?;
        bytes
            .first()
            .copied()
            .ok_or_else(|| parse_error(format!("the {what} is cut short"), self.pos))
    }

    fn u16(&mut self, what: &str) -> Result<u16> {
        let bytes = self.take(2, what)?;
        let array: [u8; 2] = bytes
            .try_into()
            .map_err(|_| parse_error(format!("the {what} is cut short"), self.pos))?;
        Ok(match self.endian {
            Endian::Big => u16::from_be_bytes(array),
            Endian::Little => u16::from_le_bytes(array),
        })
    }

    fn u32(&mut self, what: &str) -> Result<u32> {
        let bytes = self.take(4, what)?;
        let array: [u8; 4] = bytes
            .try_into()
            .map_err(|_| parse_error(format!("the {what} is cut short"), self.pos))?;
        Ok(match self.endian {
            Endian::Big => u32::from_be_bytes(array),
            Endian::Little => u32::from_le_bytes(array),
        })
    }

    fn u64(&mut self, what: &str) -> Result<u64> {
        let bytes = self.take(8, what)?;
        let array: [u8; 8] = bytes
            .try_into()
            .map_err(|_| parse_error(format!("the {what} is cut short"), self.pos))?;
        Ok(match self.endian {
            Endian::Big => u64::from_be_bytes(array),
            Endian::Little => u64::from_le_bytes(array),
        })
    }

    fn i32(&mut self, what: &str) -> Result<i32> {
        self.u32(what).map(|value| value as i32)
    }

    fn check_depth(&self, depth: usize) -> Result<()> {
        if depth > self.limits.max_depth {
            return Err(Error::new(
                codes::CORE_LIMIT_DEPTH,
                format!("nesting deeper than {} levels", self.limits.max_depth),
            ));
        }
        Ok(())
    }

    fn push(&mut self, value: Value, kind: &str, read_only: bool) -> Result<NodeId> {
        let mut node = Node::new(value);
        node.type_hint = Some(hint(kind));
        if read_only {
            node.flags.insert(NodeFlags::READ_ONLY);
        }
        self.doc.push(node)
    }

    fn string(&mut self, what: &str) -> Result<String> {
        let start = self.pos;
        let length = usize::from(self.u16(what)?);
        let data = self.take(length, what)?;
        String::from_utf8(data.to_vec())
            .map_err(|_| parse_error(format!("the {what} is not UTF-8"), start))
    }

    fn root(&mut self) -> Result<(NodeId, String)> {
        let tag = self.byte("root tag type")?;
        if !(1..=12).contains(&tag) {
            return Err(parse_error("the root tag type is not 1 to 12", 0));
        }
        let name = self.string("root tag name")?;
        let node = self.payload(tag, 0)?;
        Ok((node, name))
    }

    fn compound(&mut self, depth: usize) -> Result<NodeId> {
        let mut entries = Vec::new();
        loop {
            let tag = self.byte("compound element type")?;
            if tag == 0 {
                break;
            }
            if tag > 12 {
                return Err(parse_error(
                    format!("unknown NBT tag type {tag}"),
                    self.pos - 1,
                ));
            }
            let name = self.string("compound key")?;
            let value = self.payload(tag, depth + 1)?;
            entries.push((name, value));
        }
        self.push(Value::Object(entries), "compound", true)
    }

    fn list(&mut self, depth: usize) -> Result<NodeId> {
        let element = self.byte("list element type")?;
        if element > 12 {
            return Err(parse_error(
                format!("unknown NBT list element type {element}"),
                self.pos - 1,
            ));
        }
        let count = self.i32("list length")?;
        if count < 0 {
            return Err(parse_error("the list length is negative", self.pos - 4));
        }
        let count = usize::try_from(count)
            .map_err(|_| parse_error("the list length is too large", self.pos - 4))?;
        if count > 0 && element == 0 {
            return Err(parse_error(
                "a non-empty list has the end tag as its element type",
                self.pos,
            ));
        }
        let mut items = Vec::with_capacity(count.min(1024));
        for _ in 0..count {
            items.push(self.payload(element, depth + 1)?);
        }
        self.push(Value::Array(items), &format!("list:{element}"), true)
    }

    fn byte_array(&mut self, depth: usize) -> Result<NodeId> {
        let _ = depth;
        let count = self.i32("byte array length")?;
        if count < 0 {
            return Err(parse_error(
                "the byte array length is negative",
                self.pos - 4,
            ));
        }
        let count = usize::try_from(count)
            .map_err(|_| parse_error("the byte array is too large", self.pos - 4))?;
        let mut items = Vec::with_capacity(count.min(1024));
        for _ in 0..count {
            let value = self.byte("byte array element")?;
            items.push(self.push(
                Value::Int {
                    v: i128::from(value as i8),
                    width: IntWidth::I8,
                },
                "byte",
                false,
            )?);
        }
        self.push(Value::Array(items), "byte-array", true)
    }

    fn int_array(&mut self, depth: usize) -> Result<NodeId> {
        let _ = depth;
        let count = self.i32("int array length")?;
        if count < 0 {
            return Err(parse_error(
                "the int array length is negative",
                self.pos - 4,
            ));
        }
        let count = usize::try_from(count)
            .map_err(|_| parse_error("the int array is too large", self.pos - 4))?;
        let mut items = Vec::with_capacity(count.min(1024));
        for _ in 0..count {
            let value = self.i32("int array element")?;
            items.push(self.push(
                Value::Int {
                    v: i128::from(value),
                    width: IntWidth::I32,
                },
                "int",
                false,
            )?);
        }
        self.push(Value::Array(items), "int-array", true)
    }

    fn long_array(&mut self, depth: usize) -> Result<NodeId> {
        let _ = depth;
        let count = self.i32("long array length")?;
        if count < 0 {
            return Err(parse_error(
                "the long array length is negative",
                self.pos - 4,
            ));
        }
        let count = usize::try_from(count)
            .map_err(|_| parse_error("the long array is too large", self.pos - 4))?;
        let mut items = Vec::with_capacity(count.min(1024));
        for _ in 0..count {
            let value = self.u64("long array element")? as i64;
            items.push(self.push(
                Value::Int {
                    v: i128::from(value),
                    width: IntWidth::I64,
                },
                "long",
                false,
            )?);
        }
        self.push(Value::Array(items), "long-array", true)
    }

    fn payload(&mut self, tag: u8, depth: usize) -> Result<NodeId> {
        self.check_depth(depth)?;
        match tag {
            1 => {
                let value = self.byte("byte")? as i8;
                self.push(
                    Value::Int {
                        v: i128::from(value),
                        width: IntWidth::I8,
                    },
                    "byte",
                    false,
                )
            }
            2 => {
                let value = self.u16("short")? as i16;
                self.push(
                    Value::Int {
                        v: i128::from(value),
                        width: IntWidth::I16,
                    },
                    "short",
                    false,
                )
            }
            3 => {
                let value = self.i32("int")?;
                self.push(
                    Value::Int {
                        v: i128::from(value),
                        width: IntWidth::I32,
                    },
                    "int",
                    false,
                )
            }
            4 => {
                let value = self.u64("long")? as i64;
                self.push(
                    Value::Int {
                        v: i128::from(value),
                        width: IntWidth::I64,
                    },
                    "long",
                    false,
                )
            }
            5 => {
                let bits = self.u32("float")?;
                self.push(
                    Value::Float {
                        v: f64::from(f32::from_bits(bits)),
                        width: FloatWidth::F32,
                    },
                    "float",
                    false,
                )
            }
            6 => {
                let bits = self.u64("double")?;
                self.push(
                    Value::Float {
                        v: f64::from_bits(bits),
                        width: FloatWidth::F64,
                    },
                    "double",
                    false,
                )
            }
            7 => self.byte_array(depth),
            8 => {
                let text = self.string("string")?;
                self.push(Value::Str(text), "string", false)
            }
            9 => self.list(depth),
            10 => self.compound(depth),
            11 => self.int_array(depth),
            12 => self.long_array(depth),
            _ => Err(parse_error(
                format!("unknown NBT tag type {tag}"),
                self.pos - 1,
            )),
        }
    }
}
