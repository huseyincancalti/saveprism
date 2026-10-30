//! `format.gta-iii`: Grand Theft Auto III (classic PC) save games.
//!
//! The layout is from the public GTA III save description on GTAMods Wiki and the MIT-licensed
//! 010 Editor template by Seemann (`x87/GTA-Save-Formats`, `formats/gta3save.bt`), read for
//! behaviour only. No code was copied. The full behavioural spec is in `docs/formats/gta-iii.md`.
//!
//! # Layout
//!
//! The PC file is always 201,820 bytes. It holds 22 blocks. Every block starts with a
//! little-endian `u32` size that does not include itself, so the next block is at `start + 4 +
//! size`. Block 0 has no second size. Blocks 1 to 19 start their body with another `u32` size,
//! then the actual data. Blocks 20 and 21 are padding. A little-endian `u32` checksum ends the
//! file.
//!
//! # Tree
//!
//! The root is a read-only `Object` with the hint `GTA3:object:save`:
//!
//! | Key | Node | Editable |
//! |---|---|---|
//! | `title` | `Str` from block 0 | no |
//! | `current_island` | `Int` (U32) from block 0 | no |
//! | `game_time` | `Object` with `hour`, `minute` (U8) | each value |
//! | `player` | `Object` with `money`, `money_on_screen` (U32) | each value |
//! | `max_wanted_level`, `max_chaos` | `Int` (U32) from block 1 | yes |
//!
//! # Exact writing
//!
//! Decoding keeps the original bytes as a parameter. Writing copies them and puts every edited
//! value back at its byte span; the file length never changes. When at least one value changed,
//! the checksum is recomputed as the wrapping sum of the bytes before it. An unchanged document
//! writes the original bytes exactly, including an already-wrong checksum.

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

/// The file is not a classic PC GTA III save, or a block is missing or cut short.
pub const GTA3_PARSE: ErrorCode = ErrorCode::new("SP-GTA3-001");
/// A value cannot be written back (wrong type, out of range, or the original bytes are missing).
pub const GTA3_WRITE: ErrorCode = ErrorCode::new("SP-GTA3-002");

/// Prefix of every hint this codec writes.
pub const HINT_PREFIX: &str = "GTA3:";

const ORIGINAL: &str = "original";
const CHECKSUM_AT: &str = "checksum_at";

/// Registers the GTA III codec as `format.gta-iii`.
pub fn register(reg: &mut Registry) {
    reg.add_codec(Arc::new(Gta3Codec));
}

/// The `format.gta-iii` codec.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gta3Codec;

impl Codec for Gta3Codec {
    fn id(&self) -> &'static str {
        "format.gta-iii"
    }

    /// A 201,820-byte file with a plausible first block size.
    fn sniff(&self, input: &[u8], _ctx: &SniffCtx<'_>) -> Confidence {
        if read::has_header(input) {
            Confidence::new(0.95)
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
    Error::new(GTA3_PARSE, message).with_offset(len_u64(at))
}

/// A write error.
pub(crate) fn write_error(message: impl Into<String>) -> Error {
    Error::new(GTA3_WRITE, message)
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
        _ => Err(write_error("the original GTA III save is missing")),
    }
}
