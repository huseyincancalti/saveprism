//! `format.gta-iv`: Grand Theft Auto IV (PC) save games (`SGTA4xx`).
//!
//! The layout is from the public GTA IV save description on GTAMods Wiki and the MIT-licensed
//! 010 Editor template by Seemann (`x87/GTA-Save-Formats`, `formats/gta4save.bt`), read for
//! behaviour only. No code was copied. The full behavioural spec is in `docs/formats/gta-iv.md`.
//!
//! # Layout
//!
//! - Metadata: 0x110 bytes. Little-endian `u32` version, `u32` file size, `u32` globals size,
//!   the bytes `SAVE`, then the last mission as a NUL-terminated UTF-16LE string (128 code units).
//! - 32 data blocks. Each starts with `BLOCK`, a little-endian `u32` size that includes the nine
//!   header bytes, and the block data. The size is what makes the blocks skippable; the fields
//!   SavePrism reads are in `SimpleVars` (block 0) and `PlayerInfo` (block 1).
//! - A little-endian `u32` checksum (the sum of every preceding byte, wrapping), then the fixed
//!   0x16C-byte end block that starts with `END\0`.
//!
//! # Tree
//!
//! The root is a read-only `Object` with the hint `GTA4:object:save`:
//!
//! | Key | Node | Editable |
//! |---|---|---|
//! | `version`, `file_size`, `globals_size` | `Int` (U32) | no |
//! | `last_mission` | `Str` from the metadata | no |
//! | `game_time` | `Object` with `month`, `day`, `hour`, `minute`, `day_of_week` (U32) | each value |
//! | `player` | `Object` with `money`, `money_on_screen` (U32), `max_health`, `max_armour` (U16), `health`, `armour` (F32) | each value |
//! | `max_wanted_level`, `max_chaos` | `Int` (U32) from `SimpleVars` | yes |
//!
//! # Exact writing
//!
//! Decoding keeps the original bytes as a parameter. Writing copies them and puts every edited
//! value back at its byte span; the file length never changes. When at least one value changed,
//! the checksum is recomputed as the wrapping sum of the bytes before it. The game ignores the
//! checksum, but keeping it consistent avoids tools that check it. An unchanged document writes
//! the original bytes exactly, including an already-wrong checksum.

mod read;
#[cfg(test)]
mod tests;
mod write;

use std::sync::Arc;

use saveprism_core::limits::len_u64;
use saveprism_core::{
    ByteSpan, Codec, Confidence, DecodeCtx, Decoded, Error, ErrorCode, Node, NodeFlags, NodeId,
    ParamValue, Params, Registry, Result, SaveDoc, SniffCtx, TypeHint, Value,
};

/// The file is not a GTA IV save, or a block, the checksum or the end block is cut short.
pub const GTA4_PARSE: ErrorCode = ErrorCode::new("SP-GTA4-001");
/// A value cannot be written back (wrong type, out of range, or the original bytes are missing).
pub const GTA4_WRITE: ErrorCode = ErrorCode::new("SP-GTA4-002");

/// Prefix of every hint this codec writes.
pub const HINT_PREFIX: &str = "GTA4:";

const ORIGINAL: &str = "original";
const CHECKSUM_AT: &str = "checksum_at";

/// Registers the GTA IV codec as `format.gta-iv`.
pub fn register(reg: &mut Registry) {
    reg.add_codec(Arc::new(Gta4Codec));
}

/// The `format.gta-iv` codec.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gta4Codec;

impl Codec for Gta4Codec {
    fn id(&self) -> &'static str {
        "format.gta-iv"
    }

    /// `SAVE` in the metadata and a 0xB9-byte `SimpleVars` block.
    fn sniff(&self, input: &[u8], _ctx: &SniffCtx<'_>) -> Confidence {
        if read::has_header(input) {
            Confidence::new(0.98)
        } else {
            Confidence::NONE
        }
    }

    fn decode(&self, input: &[u8], ctx: &mut DecodeCtx<'_>) -> Result<Decoded> {
        ctx.limits().check_input(len_u64(input.len()))?;
        let shared: &DecodeCtx<'_> = ctx;
        let (doc, checksum_at) = read::read_all(input, shared)?;
        ctx.charge_nodes(len_u64(doc.len()))?;
        let mut params = Params::new();
        params.set(ORIGINAL, input.to_vec());
        params.set(CHECKSUM_AT, i64::try_from(checksum_at).unwrap_or(i64::MAX));
        Ok(Decoded { doc, params })
    }

    fn encode(&self, doc: &SaveDoc, params: &Params) -> Result<Vec<u8>> {
        write::encode(doc, params)
    }
}

/// A parse error at a byte offset.
pub(crate) fn error_at(message: impl Into<String>, at: usize) -> Error {
    Error::new(GTA4_PARSE, message).with_offset(len_u64(at))
}

/// A write error.
pub(crate) fn write_error(message: impl Into<String>) -> Error {
    Error::new(GTA4_WRITE, message)
}

/// Pushes a node with a hint, an optional absolute byte span and the read-only flag.
pub(crate) fn push(
    doc: &mut SaveDoc,
    value: Value,
    kind: &str,
    span: Option<(usize, usize)>,
    read_only: bool,
) -> Result<NodeId> {
    let mut node = Node::new(value);
    node.type_hint = Some(TypeHint::new(format!("{HINT_PREFIX}{kind}")));
    node.span = span.map(|(start, len)| ByteSpan {
        start: len_u64(start),
        end: len_u64(start).saturating_add(len_u64(len)),
    });
    if read_only {
        node.flags.insert(NodeFlags::READ_ONLY);
    }
    doc.push(node)
}

/// The original bytes parameter, or a write error.
pub(crate) fn original(params: &Params) -> Result<&Vec<u8>> {
    match params.get(ORIGINAL) {
        Some(ParamValue::Bytes(bytes)) => Ok(bytes),
        _ => Err(write_error("the original GTA IV save is missing")),
    }
}
