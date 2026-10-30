//! `format.gamemaker-ds-map` and `format.gamemaker-ds-list`: the hexadecimal text that
//! `ds_map_write` and `ds_list_write` return in GameMaker Studio 1/2.
//!
//! The byte layout follows the public reverse-engineered notes and the open-source HTML5 runtime:
//! the text is two hexadecimal digits per byte; the bytes are a little-endian `u32` magic, a
//! little-endian `u32` count, then the entries. Maps use magic 402 (old, version 3) or 403
//! (current), lists use 302 or 303. Values are typed with the runtime's `variable_WriteValue`
//! codes: real (`0`, IEEE-754 `f64`), string (`1`, `u32` length and bytes), array (`2`), pointer
//! (`3`, `i64`), undefined (`5`, no payload), int32 (`7`), int64 (`10`) and bool (`13`, `f64`).
//! In the version 3 layout an array starts with `1` and a length; a real 2D array starts with its
//! row count and each row carries its own length.
//!
//! GameMaker changed these formats more than once. The scalar types and the two layouts above are
//! opened; an unknown type or magic is refused instead of guessed.
//!
//! # Tree
//!
//! A map root is a read-only `Map` (hint `GM:map`) so a numeric key stays a number. Keys are
//! read-only; values are editable. A list root is a read-only `Array` (hint `GM:list`); its items
//! are editable. Nested array nodes are read-only containers, but their items are editable.
//!
//! | Value | Node | Hint | Editable |
//! |---|---|---|---|
//! | real | `Float` (F64) | `GM:real` | yes |
//! | string | `Str`, or read-only `Bytes` when not UTF-8 | `GM:string` | text yes |
//! | array | `Array` | `GM:array` | items |
//! | 2D array | `Array` of row arrays | `GM:array2d` | items |
//! | pointer | `Int` (I64) | `GM:ptr` | yes |
//! | undefined | `Null` | `GM:undefined` | no |
//! | int32 | `Int` (I32) | `GM:int32` | yes |
//! | int64 | `Int` (I64) | `GM:int64` | yes |
//! | bool | `Bool` | `GM:bool` | yes |
//!
//! # Exact writing
//!
//! The original text is the codec parameter `original` (with any trailing line ending kept in
//! `trailing`) and the magic number is `magic`. An unchanged dump is written back as the original
//! text, including lowercase hex; an edit writes the canonical uppercase dump. Entry order and
//! types are preserved, so an edit never changes the shape.

mod read;
#[cfg(test)]
mod tests;
mod write;

use std::sync::Arc;

use saveprism_core::limits::len_u64;
use saveprism_core::{
    Codec, Confidence, DecodeCtx, Decoded, Error, ErrorCode, Params, Registry, Result, SaveDoc,
    SniffCtx,
};

/// The text is not a GameMaker `ds_map` or `ds_list` dump.
pub const GM_PARSE: ErrorCode = ErrorCode::new("SP-GMD-001");
/// The document cannot be written as a GameMaker dump.
pub const GM_WRITE: ErrorCode = ErrorCode::new("SP-GMD-002");

/// Prefix of every hint this codec writes.
pub const PREFIX: &str = "GM:";
/// Parameter: the serialization magic.
const MAGIC_PARAM: &str = "magic";

const MAP_MAGIC_OLD: u32 = 402;
const MAP_MAGIC: u32 = 403;
const LIST_MAGIC_OLD: u32 = 302;
const LIST_MAGIC: u32 = 303;

/// Registers `format.gamemaker-ds-map` and `format.gamemaker-ds-list`.
pub fn register(reg: &mut Registry) {
    reg.add_codec(Arc::new(GameMakerDsMapCodec));
    reg.add_codec(Arc::new(GameMakerDsListCodec));
}

/// The `format.gamemaker-ds-map` codec.
#[derive(Clone, Copy, Debug, Default)]
pub struct GameMakerDsMapCodec;

/// The `format.gamemaker-ds-list` codec.
#[derive(Clone, Copy, Debug, Default)]
pub struct GameMakerDsListCodec;

#[derive(Clone, Copy)]
enum Kind {
    Map,
    List,
}

impl Kind {
    fn magics(self) -> [u32; 2] {
        match self {
            Self::Map => [MAP_MAGIC_OLD, MAP_MAGIC],
            Self::List => [LIST_MAGIC_OLD, LIST_MAGIC],
        }
    }

    fn default_magic(self) -> u32 {
        match self {
            // Recipes written before `magic` was stored used the original 402 writer.
            Self::Map => MAP_MAGIC_OLD,
            Self::List => LIST_MAGIC,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Map => "ds_map",
            Self::List => "ds_list",
        }
    }
}

/// The text without trailing ASCII whitespace, plus the bytes that were stripped.
fn split_trailing(input: &[u8]) -> (&[u8], &[u8]) {
    let mut end = input.len();
    while end > 0
        && input
            .get(end - 1)
            .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        end -= 1;
    }
    (
        input.get(..end).unwrap_or_default(),
        input.get(end..).unwrap_or_default(),
    )
}

/// True when the whole text is an even run of hexadecimal digits.
fn is_hex_text(text: &[u8]) -> bool {
    !text.is_empty() && text.len().is_multiple_of(2) && text.iter().all(u8::is_ascii_hexdigit)
}

/// Decodes the first eight hex digits as the magic number.
fn text_magic(text: &[u8]) -> Option<u32> {
    let header = text.get(..8)?;
    let digits = std::str::from_utf8(header).ok()?;
    let bytes = read::hex_bytes(digits).ok()?;
    read::magic(&bytes)
}

fn sniff(input: &[u8], kind: Kind) -> Confidence {
    let (text, _) = split_trailing(input);
    if !is_hex_text(text) {
        return Confidence::NONE;
    }
    match text_magic(text) {
        Some(magic) if kind.magics().contains(&magic) => Confidence::new(0.88),
        _ => Confidence::NONE,
    }
}

fn decode(input: &[u8], ctx: &mut DecodeCtx<'_>, kind: Kind) -> Result<Decoded> {
    ctx.limits().check_input(len_u64(input.len()))?;
    let (text, trailing) = split_trailing(input);
    if !is_hex_text(text) {
        return Err(Error::new(
            GM_PARSE,
            format!(
                "the GameMaker {} dump is not an even run of hex digits",
                kind.name()
            ),
        ));
    }
    let Ok(ascii) = std::str::from_utf8(text) else {
        return Err(Error::new(
            GM_PARSE,
            format!("the GameMaker {} dump is not ASCII", kind.name()),
        ));
    };
    let bytes = read::hex_bytes(ascii)?;
    let magic = read::magic(&bytes).ok_or_else(|| {
        Error::new(
            GM_PARSE,
            format!("the GameMaker {} is too short", kind.name()),
        )
    })?;
    if !kind.magics().contains(&magic) {
        return Err(Error::new(
            GM_PARSE,
            format!(
                "the GameMaker {} magic is {magic}, not {} or {}",
                kind.name(),
                kind.magics()[0],
                kind.magics()[1]
            ),
        ));
    }
    let mut doc = ctx.new_doc();
    let root = match kind {
        Kind::Map => read::parse_map(&bytes, &mut doc, ctx.limits().max_nodes)?,
        Kind::List => read::parse_list(&bytes, &mut doc, ctx.limits().max_nodes)?,
    };
    doc.set_root(root);
    ctx.charge_nodes(len_u64(doc.len().saturating_sub(1)))?;
    let mut params = Params::new();
    params.set("original", input.to_vec());
    params.set(MAGIC_PARAM, i64::from(magic));
    if !trailing.is_empty() {
        params.set("trailing", trailing.to_vec());
    }
    Ok(Decoded { doc, params })
}

fn encode(doc: &SaveDoc, params: &Params, kind: Kind) -> Result<Vec<u8>> {
    let original = params
        .opt_bytes("original")?
        .ok_or_else(|| Error::new(GM_WRITE, "original GameMaker text is missing"))?;
    let trailing = params.opt_bytes("trailing")?.unwrap_or_default();
    let magic = match params.opt_int(MAGIC_PARAM)? {
        Some(magic) => u32::try_from(magic)
            .map_err(|_| Error::new(GM_WRITE, "the GameMaker magic is out of range"))?,
        None => kind.default_magic(),
    };
    if !kind.magics().contains(&magic) {
        return Err(Error::new(
            GM_WRITE,
            format!("{magic} is not a {} magic", kind.name()),
        ));
    }
    let layout = write::Layout::new(magic);
    let canonical = match kind {
        Kind::Map => write::render_map(doc, layout)?,
        Kind::List => write::render_list(doc, layout)?,
    };
    let (original_text, _) = split_trailing(original);
    if canonical.eq_ignore_ascii_case(original_text) {
        return Ok(original.to_vec());
    }
    let mut out = canonical;
    out.extend_from_slice(trailing);
    Ok(out)
}

impl Codec for GameMakerDsMapCodec {
    fn id(&self) -> &'static str {
        "format.gamemaker-ds-map"
    }

    fn sniff(&self, input: &[u8], _ctx: &SniffCtx<'_>) -> Confidence {
        sniff(input, Kind::Map)
    }

    fn decode(&self, input: &[u8], ctx: &mut DecodeCtx<'_>) -> Result<Decoded> {
        decode(input, ctx, Kind::Map)
    }

    fn encode(&self, doc: &SaveDoc, params: &Params) -> Result<Vec<u8>> {
        encode(doc, params, Kind::Map)
    }
}

impl Codec for GameMakerDsListCodec {
    fn id(&self) -> &'static str {
        "format.gamemaker-ds-list"
    }

    fn sniff(&self, input: &[u8], _ctx: &SniffCtx<'_>) -> Confidence {
        sniff(input, Kind::List)
    }

    fn decode(&self, input: &[u8], ctx: &mut DecodeCtx<'_>) -> Result<Decoded> {
        decode(input, ctx, Kind::List)
    }

    fn encode(&self, doc: &SaveDoc, params: &Params) -> Result<Vec<u8>> {
        encode(doc, params, Kind::List)
    }
}
