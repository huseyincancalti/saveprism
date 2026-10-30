//! Writer for Minecraft region files.

use std::io::Write;

use flate2::Compression;
use flate2::write::{GzEncoder, ZlibEncoder};
use saveprism_core::{Error, Limits, NodeId, Params, Result, SaveDoc, Value};

use super::read::{self, Location};
use super::{MCA_WRITE, ORIGINAL};

const SECTOR_LEN: usize = 4096;
const MAX_SECTORS: usize = 255;

fn write_error(message: impl Into<String>) -> Error {
    Error::new(MCA_WRITE, message)
}

fn field(doc: &SaveDoc, id: NodeId, name: &str) -> Option<NodeId> {
    match &doc.get(id)?.value {
        Value::Object(entries) => entries
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| *value),
        _ => None,
    }
}

fn int_field(doc: &SaveDoc, id: NodeId, name: &str) -> Result<i128> {
    let field = field(doc, id, name)
        .ok_or_else(|| write_error(format!("a region chunk has no \"{name}\"")))?;
    match doc
        .get(field)
        .ok_or_else(|| write_error("a region chunk entry is missing"))?
        .value
    {
        Value::Int { v, .. } => Ok(v),
        _ => Err(write_error(format!("\"{name}\" is not an integer"))),
    }
}

fn str_field<'a>(doc: &'a SaveDoc, id: NodeId, name: &str) -> Result<Option<&'a str>> {
    let Some(field) = field(doc, id, name) else {
        return Ok(None);
    };
    match &doc
        .get(field)
        .ok_or_else(|| write_error("a region chunk entry is missing"))?
        .value
    {
        Value::Str(text) => Ok(Some(text)),
        _ => Err(write_error(format!("\"{name}\" is not text"))),
    }
}

fn bytes_field<'a>(doc: &'a SaveDoc, id: NodeId, name: &str) -> Result<Option<&'a [u8]>> {
    let Some(field) = field(doc, id, name) else {
        return Ok(None);
    };
    match &doc
        .get(field)
        .ok_or_else(|| write_error("a region chunk entry is missing"))?
        .value
    {
        Value::Bytes(bytes) => Ok(Some(bytes)),
        _ => Err(write_error(format!("\"{name}\" is not bytes"))),
    }
}

fn compress(compression: u8, data: &[u8]) -> Result<Vec<u8>> {
    match compression {
        1 => {
            let mut writer = GzEncoder::new(Vec::new(), Compression::default());
            writer.write_all(data).map_err(|source| {
                write_error("the gzip chunk could not be compressed").with_source(source)
            })?;
            writer.finish().map_err(|source| {
                write_error("the gzip chunk could not be finished").with_source(source)
            })
        }
        2 => {
            let mut writer = ZlibEncoder::new(Vec::new(), Compression::default());
            writer.write_all(data).map_err(|source| {
                write_error("the zlib chunk could not be compressed").with_source(source)
            })?;
            writer.finish().map_err(|source| {
                write_error("the zlib chunk could not be finished").with_source(source)
            })
        }
        3 => Ok(data.to_vec()),
        other => Err(write_error(format!(
            "chunk compression type {other} cannot be written"
        ))),
    }
}

fn set_location(out: &mut [u8], index: usize, offset: usize, sectors: usize) -> Result<()> {
    if offset >= 1 << 24 {
        return Err(write_error("the region has more than 16,777,215 sectors"));
    }
    if sectors > MAX_SECTORS {
        return Err(write_error("a chunk needs more than 255 sectors"));
    }
    let at = index
        .checked_mul(4)
        .ok_or_else(|| write_error("the chunk index is too large"))?;
    let entry = [
        ((offset >> 16) & 0xFF) as u8,
        ((offset >> 8) & 0xFF) as u8,
        (offset & 0xFF) as u8,
        sectors as u8,
    ];
    let slot = out
        .get_mut(at..at + 4)
        .ok_or_else(|| write_error("the location table is cut short"))?;
    slot.copy_from_slice(&entry);
    Ok(())
}

fn payload(length: usize, compression: u8, compressed: &[u8]) -> Result<Vec<u8>> {
    let length =
        u32::try_from(length).map_err(|_| write_error("a chunk payload is larger than 4 GiB"))?;
    let mut out = Vec::with_capacity(4 + length as usize);
    out.extend_from_slice(&length.to_be_bytes());
    out.push(compression);
    out.extend_from_slice(compressed);
    Ok(out)
}

fn patch_chunk(
    out: &mut Vec<u8>,
    index: usize,
    location: Location,
    compression: u8,
    compressed: &[u8],
) -> Result<bool> {
    let length = 1 + compressed.len();
    let bytes = payload(length, compression, compressed)?;
    let needed = bytes.len();
    let old_bytes = location
        .sectors
        .checked_mul(SECTOR_LEN)
        .ok_or_else(|| write_error("the old chunk size is too large"))?;
    if needed <= old_bytes {
        let start = location
            .offset
            .checked_mul(SECTOR_LEN)
            .ok_or_else(|| write_error("the old chunk offset is too large"))?;
        let end = start
            .checked_add(needed)
            .ok_or_else(|| write_error("the chunk end is too large"))?;
        if out.get(start..end) == Some(bytes.as_slice()) {
            return Ok(false);
        }
        out.get_mut(start..end)
            .ok_or_else(|| write_error("the old chunk area is outside the file"))?
            .copy_from_slice(&bytes);
        let pad_end = start
            .checked_add(old_bytes)
            .ok_or_else(|| write_error("the old chunk padding is too large"))?;
        if let Some(padding) = out.get_mut(end..pad_end) {
            padding.fill(0);
        }
        return Ok(true);
    }

    let sectors = needed.div_ceil(SECTOR_LEN);
    if sectors > MAX_SECTORS {
        return Err(write_error("a chunk needs more than 255 sectors"));
    }
    let offset = out.len().div_ceil(SECTOR_LEN);
    let start = offset
        .checked_mul(SECTOR_LEN)
        .ok_or_else(|| write_error("the region is too large"))?;
    if start > out.len() {
        out.resize(start, 0);
    }
    let end = start
        .checked_add(sectors * SECTOR_LEN)
        .ok_or_else(|| write_error("the region is too large"))?;
    out.resize(end, 0);
    out.get_mut(start..start + bytes.len())
        .ok_or_else(|| write_error("the new chunk area is outside the file"))?
        .copy_from_slice(&bytes);
    set_location(out, index, offset, sectors)?;
    Ok(true)
}

/// Writes the document back by patching the original region.
pub(super) fn encode(doc: &SaveDoc, params: &Params) -> Result<Vec<u8>> {
    let original = params
        .bytes(ORIGINAL)
        .map_err(|_| write_error("the recipe has no original region bytes"))?;
    let header = read::parse_header(original)?;
    let root = doc
        .get(doc.root())
        .ok_or_else(|| write_error("the document has no root"))?;
    if !matches!(root.value, Value::Object(_)) {
        return Err(write_error("the region root is not an object"));
    }
    let chunks = field(doc, doc.root(), "chunks")
        .ok_or_else(|| write_error("the region root has no \"chunks\""))?;
    let Value::Array(items) = &doc
        .get(chunks)
        .ok_or_else(|| write_error("the region chunk list is missing"))?
        .value
    else {
        return Err(write_error("\"chunks\" is not an array"));
    };

    let mut out = original.to_vec();
    let mut changed = false;
    let mut seen = vec![false; 1024];
    let limits = Limits::default();
    for item in items {
        let index = int_field(doc, *item, "index")?;
        let index = usize::try_from(index)
            .ok()
            .filter(|index| *index < 1024)
            .ok_or_else(|| write_error("a chunk index is outside 0 to 1023"))?;
        if seen.get(index).copied().unwrap_or(true) {
            return Err(write_error(format!("chunk {index} appears twice")));
        }
        if let Some(slot) = seen.get_mut(index) {
            *slot = true;
        }
        let location = header
            .locations
            .get(index)
            .copied()
            .flatten()
            .ok_or_else(|| write_error(format!("chunk {index} was empty in the original")))?;
        let raw = read::read_raw(original, location)?;

        let timestamp = int_field(doc, *item, "timestamp")?;
        let timestamp = u32::try_from(timestamp)
            .map_err(|_| write_error("a chunk timestamp is outside the u32 range"))?;
        let original_timestamp = header.timestamps.get(index).copied().unwrap_or(0);
        if timestamp != original_timestamp {
            let at = SECTOR_LEN + index * 4;
            out.get_mut(at..at + 4)
                .ok_or_else(|| write_error("the timestamp table is cut short"))?
                .copy_from_slice(&timestamp.to_be_bytes());
            changed = true;
        }

        if field(doc, *item, "data").is_some() {
            if raw.compression & 0x80 != 0 || !matches!(raw.compression, 1..=3) {
                return Err(write_error(
                    "an opaque chunk cannot carry an editable NBT tree",
                ));
            }
            let root_name = str_field(doc, *item, "root_name")?.unwrap_or("");
            let data = field(doc, *item, "data")
                .ok_or_else(|| write_error("the chunk data is missing"))?;
            let encoded = crate::nbt::encode_java_node(doc, data, root_name)?;
            let old = read::decompress(raw.compression, &raw.compressed, &limits, original.len())?;
            if encoded != old {
                let compressed = compress(raw.compression, &encoded)?;
                changed |= patch_chunk(&mut out, index, location, raw.compression, &compressed)?;
            }
        } else if let Some(bytes) = bytes_field(doc, *item, "raw")? {
            if raw.compression & 0x80 != 0 {
                return Err(write_error(
                    "an external chunk cannot be written from raw bytes",
                ));
            }
            if bytes != raw.compressed.as_slice() {
                changed |= patch_chunk(&mut out, index, location, raw.compression, bytes)?;
            }
        }
    }
    if changed {
        Ok(out)
    } else {
        Ok(original.to_vec())
    }
}
