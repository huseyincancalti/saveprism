//! NBT writing (see the module documentation for the tree and parameters).

use saveprism_core::{Error, FloatWidth, Node, NodeId, Params, Result, SaveDoc, Value};

use super::{ENDIAN, HEADER, NBT_WRITE, ROOT_NAME, VERSION};

fn write_error(message: impl Into<String>) -> Error {
    Error::new(NBT_WRITE, message)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Endian {
    Big,
    Little,
}

pub(super) fn encode(doc: &SaveDoc, params: &Params) -> Result<Vec<u8>> {
    let endian = match params.str(ENDIAN)? {
        "big" => Endian::Big,
        "little" => Endian::Little,
        other => return Err(write_error(format!("unknown NBT endian value {other}"))),
    };
    let header = params.bool(HEADER)?;
    let version = if header {
        i32::try_from(params.int(VERSION)?)
            .map_err(|_| write_error("the Bedrock version does not fit in 32 bits"))?
    } else {
        0
    };
    let root_name = params.str(ROOT_NAME)?;
    let body = encode_node(doc, doc.root(), root_name, endian)?;
    if header {
        let length =
            i32::try_from(body.len()).map_err(|_| write_error("the NBT payload is too large"))?;
        let mut out = Vec::with_capacity(body.len() + 8);
        out.extend_from_slice(&version.to_le_bytes());
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(&body);
        Ok(out)
    } else {
        Ok(body)
    }
}

/// Encodes one big-endian root tag, name included, without the Bedrock header.
pub(super) fn encode_java_node(doc: &SaveDoc, id: NodeId, root_name: &str) -> Result<Vec<u8>> {
    encode_node(doc, id, root_name, Endian::Big)
}

fn encode_node(doc: &SaveDoc, id: NodeId, root_name: &str, endian: Endian) -> Result<Vec<u8>> {
    let root = doc
        .get(id)
        .ok_or_else(|| write_error("the document has no root"))?;
    let tag = tag_of(root)?;
    let mut body = Vec::new();
    body.push(tag);
    write_name(&mut body, root_name, endian)?;
    write_payload(doc, id, tag, endian, &mut body)?;
    Ok(body)
}

fn tag_of(node: &Node) -> Result<u8> {
    let hint = node
        .type_hint
        .as_ref()
        .map(|hint| hint.as_str())
        .ok_or_else(|| write_error("an NBT node has no type hint"))?;
    let rest = hint
        .strip_prefix(super::HINT_PREFIX)
        .ok_or_else(|| write_error("an NBT type hint has the wrong prefix"))?;
    Ok(match rest {
        "byte" => 1,
        "short" => 2,
        "int" => 3,
        "long" => 4,
        "float" => 5,
        "double" => 6,
        "byte-array" => 7,
        "string" => 8,
        "compound" => 10,
        "int-array" => 11,
        "long-array" => 12,
        other => {
            if let Some(element) = other.strip_prefix("list:") {
                let _ = element
                    .parse::<u8>()
                    .map_err(|_| write_error("an NBT list hint is malformed"))?;
                9
            } else {
                return Err(write_error(format!("unknown NBT type hint {other}")));
            }
        }
    })
}

fn list_element(node: &Node) -> Result<u8> {
    let hint = node
        .type_hint
        .as_ref()
        .map(|hint| hint.as_str())
        .ok_or_else(|| write_error("an NBT list has no type hint"))?;
    hint.strip_prefix("NBT:list:")
        .and_then(|text| text.parse::<u8>().ok())
        .ok_or_else(|| write_error("an NBT list hint is malformed"))
}

fn node(doc: &SaveDoc, id: NodeId) -> Result<&Node> {
    doc.get(id)
        .ok_or_else(|| write_error(format!("node {} is missing", id.index())))
}

fn write_name(out: &mut Vec<u8>, name: &str, endian: Endian) -> Result<()> {
    let length = u16::try_from(name.len())
        .map_err(|_| write_error("an NBT name is longer than 65535 bytes"))?;
    put_u16(out, length, endian);
    out.extend_from_slice(name.as_bytes());
    Ok(())
}

fn put_u16(out: &mut Vec<u8>, value: u16, endian: Endian) {
    match endian {
        Endian::Big => out.extend_from_slice(&value.to_be_bytes()),
        Endian::Little => out.extend_from_slice(&value.to_le_bytes()),
    }
}

fn put_i32(out: &mut Vec<u8>, value: i32, endian: Endian) {
    match endian {
        Endian::Big => out.extend_from_slice(&value.to_be_bytes()),
        Endian::Little => out.extend_from_slice(&value.to_le_bytes()),
    }
}

fn put_u32(out: &mut Vec<u8>, value: u32, endian: Endian) {
    match endian {
        Endian::Big => out.extend_from_slice(&value.to_be_bytes()),
        Endian::Little => out.extend_from_slice(&value.to_le_bytes()),
    }
}

fn put_i64(out: &mut Vec<u8>, value: i64, endian: Endian) {
    match endian {
        Endian::Big => out.extend_from_slice(&value.to_be_bytes()),
        Endian::Little => out.extend_from_slice(&value.to_le_bytes()),
    }
}

fn write_payload(
    doc: &SaveDoc,
    id: NodeId,
    tag: u8,
    endian: Endian,
    out: &mut Vec<u8>,
) -> Result<()> {
    let current = node(doc, id)?;
    match tag {
        1 => {
            let value = int_value(current)?;
            let value =
                i8::try_from(value).map_err(|_| write_error("a byte value is out of range"))?;
            out.push(value as u8);
        }
        2 => {
            let value = int_value(current)?;
            let value =
                i16::try_from(value).map_err(|_| write_error("a short value is out of range"))?;
            put_u16(out, value as u16, endian);
        }
        3 => {
            let value = int_value(current)?;
            let value =
                i32::try_from(value).map_err(|_| write_error("an int value is out of range"))?;
            put_i32(out, value, endian);
        }
        4 => {
            let value = int_value(current)?;
            let value =
                i64::try_from(value).map_err(|_| write_error("a long value is out of range"))?;
            put_i64(out, value, endian);
        }
        5 => {
            let value = float_value(current)?;
            put_u32(out, (value as f32).to_bits(), endian);
        }
        6 => {
            let value = float_value(current)?;
            put_i64(out, value.to_bits() as i64, endian);
        }
        7 => {
            let Value::Array(items) = &current.value else {
                return Err(write_error("a byte array is not an array"));
            };
            let count = i32::try_from(items.len())
                .map_err(|_| write_error("the byte array is too large"))?;
            put_i32(out, count, endian);
            for child in items {
                let value = int_value(node(doc, *child)?)?;
                let value = i8::try_from(value)
                    .map_err(|_| write_error("a byte array element is out of range"))?;
                out.push(value as u8);
            }
        }
        8 => {
            let Value::Str(text) = &current.value else {
                return Err(write_error("an NBT string does not hold text"));
            };
            write_name(out, text, endian)?;
        }
        9 => {
            let Value::Array(items) = &current.value else {
                return Err(write_error("an NBT list is not an array"));
            };
            let element = list_element(current)?;
            if !items.is_empty() && element == 0 {
                return Err(write_error(
                    "a non-empty NBT list cannot have the end tag as its element type",
                ));
            }
            out.push(element);
            let count =
                i32::try_from(items.len()).map_err(|_| write_error("the NBT list is too large"))?;
            put_i32(out, count, endian);
            for child in items {
                write_payload(doc, *child, element, endian, out)?;
            }
        }
        10 => {
            let Value::Object(entries) = &current.value else {
                return Err(write_error("an NBT compound is not an object"));
            };
            for (name, child) in entries {
                let child_node = node(doc, *child)?;
                let child_tag = tag_of(child_node)?;
                if child_tag == 0 {
                    return Err(write_error("a compound entry has the end tag type"));
                }
                out.push(child_tag);
                write_name(out, name, endian)?;
                write_payload(doc, *child, child_tag, endian, out)?;
            }
            out.push(0);
        }
        11 => {
            let Value::Array(items) = &current.value else {
                return Err(write_error("an int array is not an array"));
            };
            let count = i32::try_from(items.len())
                .map_err(|_| write_error("the int array is too large"))?;
            put_i32(out, count, endian);
            for child in items {
                let value = int_value(node(doc, *child)?)?;
                let value = i32::try_from(value)
                    .map_err(|_| write_error("an int array element is out of range"))?;
                put_i32(out, value, endian);
            }
        }
        12 => {
            let Value::Array(items) = &current.value else {
                return Err(write_error("a long array is not an array"));
            };
            let count = i32::try_from(items.len())
                .map_err(|_| write_error("the long array is too large"))?;
            put_i32(out, count, endian);
            for child in items {
                let value = int_value(node(doc, *child)?)?;
                let value = i64::try_from(value)
                    .map_err(|_| write_error("a long array element is out of range"))?;
                put_i64(out, value, endian);
            }
        }
        _ => return Err(write_error(format!("unknown NBT tag type {tag}"))),
    }
    Ok(())
}

fn int_value(node: &Node) -> Result<i128> {
    match node.value {
        Value::Int { v, .. } => Ok(v),
        _ => Err(write_error("an NBT integer node holds another value")),
    }
}

fn float_value(node: &Node) -> Result<f64> {
    match node.value {
        Value::Float { v, width } => Ok(match width {
            FloatWidth::F64 => v,
            FloatWidth::F32 => f64::from(v as f32),
        }),
        _ => Err(write_error("an NBT floating node holds another value")),
    }
}
