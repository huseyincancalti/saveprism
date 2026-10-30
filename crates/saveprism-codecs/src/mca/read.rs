//! Reader for Minecraft region files.

use std::io::Read;

use flate2::read::{GzDecoder, ZlibDecoder};
use saveprism_core::limits::len_u64;
use saveprism_core::{
    Error, IntWidth, Limits, Node, NodeFlags, NodeId, Result, SaveDoc, TypeHint, Value, codes,
};

use super::{HINT_PREFIX, MCA_PARSE};

const HEADER_LEN: usize = 8192;
const SECTOR_LEN: usize = 4096;

#[derive(Clone, Copy, Debug)]
pub(super) struct Location {
    pub offset: usize,
    pub sectors: usize,
}

#[derive(Clone, Debug)]
pub(super) struct Header {
    pub locations: Vec<Option<Location>>,
    pub timestamps: Vec<u32>,
}

#[derive(Clone, Debug)]
pub(super) struct RawChunk {
    pub compression: u8,
    pub compressed: Vec<u8>,
}

fn parse_error(message: impl Into<String>, offset: usize) -> Error {
    Error::new(MCA_PARSE, message).with_offset(len_u64(offset))
}

fn hint(kind: &str) -> TypeHint {
    TypeHint::new(format!("{HINT_PREFIX}{kind}"))
}

fn read_u32(input: &[u8], at: usize) -> Option<u32> {
    let bytes: [u8; 4] = input.get(at..at.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_be_bytes(bytes))
}

/// Parses the location and timestamp tables and checks that the chunk ranges do not overlap.
pub(super) fn parse_header(input: &[u8]) -> Result<Header> {
    if input.len() < HEADER_LEN {
        return Err(parse_error(
            format!("a region needs at least {HEADER_LEN} bytes for its header"),
            input.len(),
        ));
    }
    let sectors = input.len().div_ceil(SECTOR_LEN);
    let mut locations = Vec::with_capacity(1024);
    let mut ranges = Vec::new();
    for index in 0..1024 {
        let at = index * 4;
        let entry = read_u32(input, at)
            .ok_or_else(|| parse_error("the location table is cut short", at))?;
        let offset = usize::try_from(entry >> 8).unwrap_or(usize::MAX);
        let count = usize::from((entry & 0xFF) as u8);
        match (offset, count) {
            (0, 0) => locations.push(None),
            (0, _) | (_, 0) => {
                return Err(parse_error(
                    format!("chunk {index} has an offset of {offset} and {count} sectors"),
                    at,
                ));
            }
            _ if offset < 2 => {
                return Err(parse_error(
                    format!("chunk {index} points into the two header sectors"),
                    at,
                ));
            }
            _ if offset.saturating_add(count) > sectors => {
                return Err(parse_error(
                    format!("chunk {index} runs past the end of the file"),
                    at,
                ));
            }
            _ => {
                ranges.push((offset, offset + count, index));
                locations.push(Some(Location {
                    offset,
                    sectors: count,
                }));
            }
        }
    }
    ranges.sort_unstable_by_key(|range| range.0);
    for pair in ranges.windows(2) {
        if let [left, right] = pair
            && right.0 < left.1
        {
            return Err(parse_error(
                format!("chunks {} and {} overlap", left.2, right.2),
                right.0 * SECTOR_LEN,
            ));
        }
    }
    let mut timestamps = Vec::with_capacity(1024);
    for index in 0..1024 {
        let at = SECTOR_LEN + index * 4;
        let timestamp = read_u32(input, at)
            .ok_or_else(|| parse_error("the timestamp table is cut short", at))?;
        timestamps.push(timestamp);
    }
    Ok(Header {
        locations,
        timestamps,
    })
}

/// Reads the length, compression byte and compressed bytes of one chunk.
pub(super) fn read_raw(input: &[u8], location: Location) -> Result<RawChunk> {
    let start = location
        .offset
        .checked_mul(SECTOR_LEN)
        .ok_or_else(|| parse_error("the chunk offset is too large", 0))?;
    let length = read_u32(input, start)
        .ok_or_else(|| parse_error("the chunk length is cut short", start))?;
    let length =
        usize::try_from(length).map_err(|_| parse_error("the chunk length is too large", start))?;
    if length < 1 {
        return Err(parse_error("the chunk length is zero", start));
    }
    let available = location
        .sectors
        .checked_mul(SECTOR_LEN)
        .and_then(|bytes| bytes.checked_sub(4))
        .ok_or_else(|| parse_error("the chunk sector count is too small", start))?;
    if length > available {
        return Err(parse_error(
            format!("the chunk says {length} bytes but its sectors hold {available}"),
            start,
        ));
    }
    let data_start = start + 4;
    let data_end = data_start
        .checked_add(length)
        .ok_or_else(|| parse_error("the chunk end is too large", start))?;
    let data = input
        .get(data_start..data_end)
        .ok_or_else(|| parse_error("the chunk is cut short", start))?;
    let compression = data
        .first()
        .copied()
        .ok_or_else(|| parse_error("the compression byte is missing", start))?;
    Ok(RawChunk {
        compression,
        compressed: data.get(1..).unwrap_or_default().to_vec(),
    })
}

fn is_limit_error(error: &Error) -> bool {
    matches!(
        error.code(),
        codes::CORE_LIMIT_INPUT
            | codes::CORE_LIMIT_DECODED
            | codes::CORE_LIMIT_DEPTH
            | codes::CORE_LIMIT_NODES
            | codes::CORE_LIMIT_LAYERS
    )
}

/// Decompresses a chunk payload under the region budget.
pub(super) fn decompress(
    compression: u8,
    data: &[u8],
    limits: &Limits,
    input_len: usize,
) -> Result<Vec<u8>> {
    match compression {
        1 => {
            let budget = limits.output_budget(len_u64(input_len));
            let mut out = Vec::new();
            GzDecoder::new(data)
                .take(budget.saturating_add(1))
                .read_to_end(&mut out)
                .map_err(|source| {
                    Error::new(MCA_PARSE, "the gzip chunk could not be decompressed")
                        .with_source(source)
                })?;
            if len_u64(out.len()) > budget {
                return Err(Error::new(
                    codes::CORE_LIMIT_DECODED,
                    format!(
                        "the gzip chunk is larger than {budget} bytes (possible decompression bomb)"
                    ),
                ));
            }
            Ok(out)
        }
        2 => {
            let budget = limits.output_budget(len_u64(input_len));
            let mut out = Vec::new();
            ZlibDecoder::new(data)
                .take(budget.saturating_add(1))
                .read_to_end(&mut out)
                .map_err(|source| {
                    Error::new(MCA_PARSE, "the zlib chunk could not be decompressed")
                        .with_source(source)
                })?;
            if len_u64(out.len()) > budget {
                return Err(Error::new(
                    codes::CORE_LIMIT_DECODED,
                    format!(
                        "the zlib chunk is larger than {budget} bytes (possible decompression bomb)"
                    ),
                ));
            }
            Ok(out)
        }
        3 => {
            limits.check_output(len_u64(input_len), len_u64(data.len()))?;
            Ok(data.to_vec())
        }
        other => Err(parse_error(
            format!("chunk compression type {other} is not decoded"),
            0,
        )),
    }
}

fn int_node(doc: &mut SaveDoc, value: i128, width: IntWidth, kind: &str) -> Result<NodeId> {
    let mut node = Node::new(Value::Int { v: value, width });
    node.type_hint = Some(hint(kind));
    node.flags.insert(NodeFlags::READ_ONLY);
    doc.push(node)
}

fn string_node(doc: &mut SaveDoc, value: &str, kind: &str) -> Result<NodeId> {
    let mut node = Node::new(Value::Str(value.to_owned()));
    node.type_hint = Some(hint(kind));
    node.flags.insert(NodeFlags::READ_ONLY);
    doc.push(node)
}

fn bool_node(doc: &mut SaveDoc, value: bool, kind: &str) -> Result<NodeId> {
    let mut node = Node::new(Value::Bool(value));
    node.type_hint = Some(hint(kind));
    node.flags.insert(NodeFlags::READ_ONLY);
    doc.push(node)
}

fn bytes_node(doc: &mut SaveDoc, value: Vec<u8>, kind: &str) -> Result<NodeId> {
    let mut node = Node::new(Value::Bytes(value));
    node.type_hint = Some(hint(kind));
    node.flags.insert(NodeFlags::READ_ONLY);
    doc.push(node)
}

fn opaque_note(compression: u8) -> &'static str {
    match compression & 0x7F {
        1..=4 if compression & 0x80 != 0 => "the chunk payload is stored in an external .mcc file",
        1..=3 => "the chunk NBT tree could not be parsed",
        4 => "LZ4 chunk compression is not decoded",
        _ => "the chunk compression type is not decoded",
    }
}

struct ChunkNodes {
    index: NodeId,
    x: NodeId,
    z: NodeId,
    timestamp: NodeId,
    compression: NodeId,
    external: NodeId,
    root_name: Option<NodeId>,
    data: Option<NodeId>,
    raw: Option<NodeId>,
    note: Option<NodeId>,
}

fn build_chunk(
    doc: &mut SaveDoc,
    index: usize,
    timestamp: u32,
    raw: &RawChunk,
    limits: &Limits,
    input_len: usize,
) -> Result<NodeId> {
    let external = raw.compression & 0x80 != 0;
    let mut body = None;
    let mut root_name = None;
    let mut note = None;
    if !external && matches!(raw.compression & 0x7F, 1..=3) {
        match decompress(raw.compression & 0x7F, &raw.compressed, limits, input_len) {
            Ok(data) => match crate::nbt::parse_java_into(&data, limits, doc) {
                Ok((root, name)) => {
                    body = Some(root);
                    root_name = Some(name);
                }
                Err(error) if is_limit_error(&error) => return Err(error),
                Err(_) => note = Some(opaque_note(raw.compression)),
            },
            Err(error) if is_limit_error(&error) => return Err(error),
            Err(_) => note = Some(opaque_note(raw.compression)),
        }
    } else {
        note = Some(opaque_note(raw.compression));
    }

    let nodes = ChunkNodes {
        index: int_node(
            doc,
            i128::try_from(index).unwrap_or(i128::MAX),
            IntWidth::I32,
            "index",
        )?,
        x: int_node(
            doc,
            i128::try_from(index & 31).unwrap_or(i128::MAX),
            IntWidth::I32,
            "x",
        )?,
        z: int_node(
            doc,
            i128::try_from(index >> 5).unwrap_or(i128::MAX),
            IntWidth::I32,
            "z",
        )?,
        timestamp: int_node(doc, i128::from(timestamp), IntWidth::I64, "timestamp")?,
        compression: int_node(
            doc,
            i128::from(raw.compression),
            IntWidth::I32,
            "compression",
        )?,
        external: bool_node(doc, external, "external")?,
        root_name: match root_name {
            Some(name) => Some(string_node(doc, &name, "root-name")?),
            None => None,
        },
        data: body,
        raw: if body.is_none() && !external {
            Some(bytes_node(doc, raw.compressed.clone(), "raw")?)
        } else {
            None
        },
        note: match note {
            Some(note) => Some(string_node(doc, note, "note")?),
            None => None,
        },
    };

    let mut entries = vec![
        ("index".to_owned(), nodes.index),
        ("x".to_owned(), nodes.x),
        ("z".to_owned(), nodes.z),
        ("timestamp".to_owned(), nodes.timestamp),
        ("compression".to_owned(), nodes.compression),
        ("external".to_owned(), nodes.external),
    ];
    if let Some(root_name) = nodes.root_name {
        entries.push(("root_name".to_owned(), root_name));
    }
    if let Some(data) = nodes.data {
        entries.push(("data".to_owned(), data));
    }
    if let Some(raw) = nodes.raw {
        entries.push(("raw".to_owned(), raw));
    }
    if let Some(note) = nodes.note {
        entries.push(("note".to_owned(), note));
    }
    let mut node = Node::new(Value::Object(entries));
    node.type_hint = Some(hint("chunk"));
    node.flags.insert(NodeFlags::READ_ONLY);
    doc.push(node)
}

/// Decodes every non-empty chunk into one document.
pub(super) fn decode(input: &[u8], ctx: &saveprism_core::DecodeCtx<'_>) -> Result<SaveDoc> {
    let header = parse_header(input)?;
    let mut doc = ctx.new_doc();
    let mut chunks = Vec::new();
    for (index, location) in header.locations.iter().enumerate() {
        let Some(location) = location else {
            continue;
        };
        let raw = read_raw(input, *location)?;
        let node = build_chunk(
            &mut doc,
            index,
            header.timestamps.get(index).copied().unwrap_or(0),
            &raw,
            ctx.limits(),
            input.len(),
        )?;
        chunks.push(node);
    }
    let mut array = Node::new(Value::Array(chunks));
    array.type_hint = Some(hint("chunks"));
    array.flags.insert(NodeFlags::READ_ONLY);
    let chunks = doc.push(array)?;
    let mut root = Node::new(Value::Object(vec![("chunks".to_owned(), chunks)]));
    root.type_hint = Some(hint("region"));
    root.flags.insert(NodeFlags::READ_ONLY);
    let root = doc.push(root)?;
    doc.set_root(root);
    Ok(doc)
}

/// The number of non-empty chunks when the header and every chunk header look valid.
pub(super) fn sniff(input: &[u8]) -> Option<usize> {
    let header = parse_header(input).ok()?;
    let mut count = 0;
    for location in header.locations.iter().flatten() {
        let start = location.offset.checked_mul(SECTOR_LEN)?;
        let length = usize::try_from(read_u32(input, start)?).ok()?;
        let capacity = location.sectors.checked_mul(SECTOR_LEN)?.checked_sub(4)?;
        if length < 1 || length > capacity {
            return None;
        }
        let compression = *input.get(start.checked_add(4)?)?;
        if !matches!(compression & 0x7F, 1..=4) {
            return None;
        }
        count += 1;
    }
    Some(count)
}
