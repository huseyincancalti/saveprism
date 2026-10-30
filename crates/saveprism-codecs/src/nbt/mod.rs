//! `format.nbt`: Minecraft Named Binary Tag (Java big-endian and Bedrock little-endian).
//!
//! Written from the public NBT format description on minecraft.wiki ("NBT format") and the
//! Bedrock storage notes (little-endian numbers, fixed 16-bit string lengths, 32-bit list and array
//! counts, and the 8-byte `level.dat` header: storage version then payload length). GZip wrapping
//! is a separate layer (`compression.gzip`).
//!
//! # Tree
//!
//! | Tag | Node | Hint | Editable |
//! |---|---|---|---|
//! | 1 Byte | `Int` (`I8`) | `NBT:byte` | yes |
//! | 2 Short | `Int` (`I16`) | `NBT:short` | yes |
//! | 3 Int | `Int` (`I32`) | `NBT:int` | yes |
//! | 4 Long | `Int` (`I64`) | `NBT:long` | yes |
//! | 5 Float | `Float` (`F32`) | `NBT:float` | yes |
//! | 6 Double | `Float` (`F64`) | `NBT:double` | yes |
//! | 7 Byte Array | `Array` of `NBT:byte` | `NBT:byte-array` | per element |
//! | 8 String | `Str` | `NBT:string` | yes |
//! | 9 List | `Array` | `NBT:list:<element tag>` | container read-only |
//! | 10 Compound | `Object` | `NBT:compound` | container read-only |
//! | 11 Int Array | `Array` of `NBT:int` | `NBT:int-array` | per element |
//! | 12 Long Array | `Array` of `NBT:long` | `NBT:long-array` | per element |
//!
//! Names (root and compound keys) are kept. The write mode is stored in the parameters `endian`
//! (`big` or `little`), `header` (the Bedrock 8-byte header), `version` (its first integer) and
//! `root_name`.
//!
//! # Writing
//!
//! The tree is written again with the original tag types, names and list element type; string
//! lengths and array/list counts are recomputed. The format is canonical, so an unchanged document
//! gives the original bytes.

mod read;
mod write;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use saveprism_core::limits::len_u64;
use saveprism_core::{
    Codec, Confidence, DecodeCtx, Decoded, ErrorCode, Params, Registry, Result, SaveDoc, SniffCtx,
};

/// The tree is truncated, a tag type is unknown or a name/string is not UTF-8.
pub const NBT_PARSE: ErrorCode = ErrorCode::new("SP-NBT-001");
/// A document cannot be written as NBT (a value is out of range, a string is too long, ...).
pub const NBT_WRITE: ErrorCode = ErrorCode::new("SP-NBT-002");

/// Prefix of every hint this codec writes.
pub const HINT_PREFIX: &str = "NBT:";
/// Parameter: `big` or `little`.
pub(crate) const ENDIAN: &str = "endian";
/// Parameter: the Bedrock 8-byte header is present.
pub(crate) const HEADER: &str = "header";
/// Parameter: the first header integer (storage version).
pub(crate) const VERSION: &str = "version";
/// Parameter: name of the root tag.
pub(crate) const ROOT_NAME: &str = "root_name";

/// Registers the codec as `format.nbt`.
pub fn register(reg: &mut Registry) {
    reg.add_codec(Arc::new(NbtCodec));
}

/// Parses one big-endian root tag into an existing document, for container formats such as the
/// Minecraft region codec.
pub(crate) fn parse_java_into(
    input: &[u8],
    limits: &saveprism_core::Limits,
    doc: &mut SaveDoc,
) -> Result<(saveprism_core::NodeId, String)> {
    read::parse_java_into(input, limits, doc)
}

/// Encodes one big-endian root tag from an existing document.
pub(crate) fn encode_java_node(
    doc: &SaveDoc,
    id: saveprism_core::NodeId,
    root_name: &str,
) -> Result<Vec<u8>> {
    write::encode_java_node(doc, id, root_name)
}

/// The `format.nbt` codec.
#[derive(Clone, Copy, Debug, Default)]
pub struct NbtCodec;

impl Codec for NbtCodec {
    fn id(&self) -> &'static str {
        "format.nbt"
    }

    /// A complete parse of big-endian, little-endian or header-prefixed Bedrock data.
    fn sniff(&self, input: &[u8], _ctx: &SniffCtx<'_>) -> Confidence {
        if input.len() < 3 || !read::plausible(input, &saveprism_core::Limits::default()) {
            return Confidence::NONE;
        }
        Confidence::new(0.4)
    }

    fn decode(&self, input: &[u8], ctx: &mut DecodeCtx<'_>) -> Result<Decoded> {
        ctx.limits().check_input(len_u64(input.len()))?;
        let (doc, params) = read::decode(input, ctx.limits())?;
        ctx.charge_nodes(len_u64(doc.len().saturating_sub(1)))?;
        Ok(Decoded { doc, params })
    }

    fn encode(&self, doc: &SaveDoc, params: &Params) -> Result<Vec<u8>> {
        write::encode(doc, params)
    }
}
