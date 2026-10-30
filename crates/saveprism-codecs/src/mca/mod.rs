//! `format.minecraft-region`: Minecraft Java Edition region files (`.mca`, `.mcr`).
//!
//! Written from the public "Region file format" and "Anvil file format" descriptions on
//! minecraft.wiki. A region is a 8 KiB header (1024 chunk locations, then 1024 timestamps) and
//! 4 KiB sectors. Each non-empty location points at a chunk payload: a big-endian `u32` length
//! (which includes the following compression byte), the compression type, and the compressed
//! bytes of one Java big-endian NBT tree.
//!
//! Compression types 1 (gzip), 2 (zlib) and 3 (uncompressed) are decoded. Chunks whose NBT tree
//! cannot be parsed, use another compression type, or live in an external `.mcc` file are shown
//! as read-only `raw` bytes, so the rest of the region stays editable and the file can still be
//! written back unchanged.
//!
//! # Tree
//!
//! The root is a read-only `Object` with the hint `MCA:region` and one key, `chunks`, an array of
//! chunk objects (one per non-empty location, in region index order):
//!
//! | Key | Node | Editable |
//! |---|---|---|
//! | `index` | `Int` (I32), 0 to 1023 | no |
//! | `x`, `z` | `Int` (I32), local chunk coordinates | no |
//! | `timestamp` | `Int` (I64), the header timestamp | no |
//! | `compression` | `Int` (I32), the original scheme | no |
//! | `external` | `Bool`, true when the payload is in a `.mcc` file | no |
//! | `root_name` | `Str`, the NBT root name | no |
//! | `data` | NBT tree of an editable chunk | leaves |
//! | `raw` | `Bytes`, compressed payload of a chunk that stays opaque | no |
//! | `note` | `Str`, why the chunk is opaque | no |
//!
//! # Writing
//!
//! The original file is kept as the codec parameter `original`. An unchanged document writes it
//! back byte for byte. When a chunk's NBT tree changed, it is compressed with its original scheme
//! and written into its old sectors when it still fits; otherwise it is appended at the next
//! free sector and the location table is updated. Other chunks, unused sectors, external entries
//! and trailing bytes stay as they were. Timestamps are read-only, so the writer changes only the
//! chunks whose NBT bytes differ.

mod read;
#[cfg(test)]
mod tests;
mod write;

use std::sync::Arc;

use saveprism_core::limits::len_u64;
use saveprism_core::{
    Codec, Confidence, DecodeCtx, Decoded, ErrorCode, Params, Registry, Result, SaveDoc, SniffCtx,
};

/// The file is not a region, a location is invalid or a chunk is cut short.
pub const MCA_PARSE: ErrorCode = ErrorCode::new("SP-MCA-001");
/// A document cannot be written as a region file.
pub const MCA_WRITE: ErrorCode = ErrorCode::new("SP-MCA-002");

/// Prefix of every hint this codec writes.
pub const HINT_PREFIX: &str = "MCA:";
/// Parameter: the original region file.
pub(crate) const ORIGINAL: &str = "original";

/// Registers the codec as `format.minecraft-region`.
pub fn register(reg: &mut Registry) {
    reg.add_codec(Arc::new(MinecraftRegionCodec));
}

/// The `format.minecraft-region` codec.
#[derive(Clone, Copy, Debug, Default)]
pub struct MinecraftRegionCodec;

impl Codec for MinecraftRegionCodec {
    fn id(&self) -> &'static str {
        "format.minecraft-region"
    }

    /// A valid location table and at least one structurally valid chunk, or a `.mca`/`.mcr` name.
    fn sniff(&self, input: &[u8], ctx: &SniffCtx<'_>) -> Confidence {
        let Some(header) = read::sniff(input) else {
            return Confidence::NONE;
        };
        let named = ctx.file_name.is_some_and(|name| {
            let lower = name.to_ascii_lowercase();
            lower.ends_with(".mca") || lower.ends_with(".mcr")
        });
        if header == 0 {
            return Confidence::new(if named { 0.6 } else { 0.0 });
        }
        Confidence::new(if named { 0.95 } else { 0.75 })
    }

    fn decode(&self, input: &[u8], ctx: &mut DecodeCtx<'_>) -> Result<Decoded> {
        ctx.limits().check_input(len_u64(input.len()))?;
        let doc = read::decode(input, ctx)?;
        ctx.charge_nodes(len_u64(doc.len().saturating_sub(1)))?;
        let mut params = Params::new();
        params.set(ORIGINAL, input.to_vec());
        Ok(Decoded { doc, params })
    }

    fn encode(&self, doc: &SaveDoc, params: &Params) -> Result<Vec<u8>> {
        write::encode(doc, params)
    }
}
