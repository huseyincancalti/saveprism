//! `format.gta-sa`: Grand Theft Auto: San Andreas (classic PC) save games.
//!
//! The layout is from the public GTA:SA save description on GTAMods Wiki and the MIT-licensed
//! 010 Editor template by Seemann (`x87/GTA-Save-Formats`, `formats/gtasasave.bt`), read for
//! behaviour only. No code was copied. The MIT-licensed `libsavegame` by Paul Breeuwsma was used
//! to confirm the block order and field offsets. The full behavioural spec is in
//! `docs/formats/gta-sa.md`.
//!
//! # Layout
//!
//! The PC file is always 202,752 bytes. It holds 30 sequential data blocks, each starting with
//! the five bytes `BLOCK` and then a block-specific body with no common size field, followed by
//! copies of blocks 15 to 17, one padding block and a little-endian `u32` checksum. With the
//! copies and the padding block the file contains exactly 34 `BLOCK` markers.
//!
//! The block bodies are not self-describing, so boundaries come from the known marker positions.
//! The parser checks that block 10 is exactly 18,892 bytes long, as every classic PC save must be.
//!
//! # Tree
//!
//! The root is a read-only `Object` with the hint `GTASA:object:save`:
//!
//! | Key | Node | Editable |
//! |---|---|---|
//! | `title` | `Str` from block 0 | no |
//! | `game_time` | `Object` with `hour`, `minute`, `day_of_week` (U8) | each value |
//! | `player` | `Object` with `health`, `armour` (F32), `max_health`, `max_armour` (U8), `money`, `money_on_screen` (I32) | each value |
//!
//! # Exact writing
//!
//! Decoding keeps the original bytes as a parameter. Writing copies them and puts every edited
//! value back at its byte span; the file length never changes. The money and maximum-health
//! fields also live in the copy of block 15 near the end of the file, so an edit writes both
//! copies. When at least one value changed, the checksum is recomputed as the wrapping sum of the
//! bytes before it. An unchanged document writes the original bytes exactly, including an
//! already-wrong checksum.

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

/// The file is not a classic PC GTA:SA save, or a block is missing or cut short.
pub const GTASA_PARSE: ErrorCode = ErrorCode::new("SP-GTASA-001");
/// A value cannot be written back (wrong type, out of range, or the original bytes are missing).
pub const GTASA_WRITE: ErrorCode = ErrorCode::new("SP-GTASA-002");

/// Prefix of every hint this codec writes.
pub const HINT_PREFIX: &str = "GTASA:";

const ORIGINAL: &str = "original";
const CHECKSUM_AT: &str = "checksum_at";
const MIRROR_15_AT: &str = "mirror_15_at";

/// Registers the San Andreas codec as `format.gta-sa`.
pub fn register(reg: &mut Registry) {
    reg.add_codec(Arc::new(GtaSaCodec));
}

/// The `format.gta-sa` codec.
#[derive(Clone, Copy, Debug, Default)]
pub struct GtaSaCodec;

impl Codec for GtaSaCodec {
    fn id(&self) -> &'static str {
        "format.gta-sa"
    }

    /// A 202,752-byte file that starts with `BLOCK`.
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
        let (doc, positions) = read::read_all(input, shared)?;
        ctx.charge_nodes(len_u64(doc.len()))?;
        let mut params = Params::new();
        params.set(ORIGINAL, input.to_vec());
        params.set(
            CHECKSUM_AT,
            i64::try_from(positions.checksum_at).unwrap_or(i64::MAX),
        );
        params.set(
            MIRROR_15_AT,
            i64::try_from(positions.mirror_15).unwrap_or(i64::MAX),
        );
        Ok(Decoded { doc, params })
    }

    fn encode(&self, doc: &SaveDoc, params: &Params) -> Result<Vec<u8>> {
        write::encode(doc, params)
    }
}

/// A parse error at a byte offset.
pub(crate) fn error_at(message: impl Into<String>, at: usize) -> Error {
    Error::new(GTASA_PARSE, message).with_offset(len_u64(at))
}

/// A write error.
pub(crate) fn write_error(message: impl Into<String>) -> Error {
    Error::new(GTASA_WRITE, message)
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
        _ => Err(write_error("the original GTA:SA save is missing")),
    }
}
