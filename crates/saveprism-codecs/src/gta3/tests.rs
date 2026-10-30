//! Tests for the GTA III codec.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use std::path::PathBuf;

use saveprism_core::{
    Codec, DecodeCtx, Decoded, Edit, IntWidth, Limits, OwnedValue, Path, SniffCtx, Value,
    apply_edits,
};

use super::Gta3Codec;

fn fixture() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/synthetic/gta-iii/invented.save");
    std::fs::read(path).expect("the GTA III fixture exists")
}

fn decode_with(bytes: &[u8], limits: &Limits) -> saveprism_core::Result<Decoded> {
    Gta3Codec.decode(bytes, &mut DecodeCtx::new(limits))
}

fn decode(bytes: &[u8]) -> Decoded {
    decode_with(bytes, &Limits::default()).expect("the save parses")
}

fn encode(decoded: &Decoded) -> Vec<u8> {
    Gta3Codec
        .encode(&decoded.doc, &decoded.params)
        .expect("the save writes")
}

fn value(decoded: &Decoded, path: &str) -> Value {
    let id = decoded
        .doc
        .resolve(&Path::parse(path).expect("path"))
        .expect("node");
    decoded.doc.get(id).expect("value").value.clone()
}

fn int(value: i128, width: IntWidth) -> Value {
    Value::Int { v: value, width }
}

fn set(decoded: &mut Decoded, path: &str, value: OwnedValue) -> saveprism_core::Result<()> {
    apply_edits(
        &mut decoded.doc,
        &[Edit::Set {
            path: Path::parse(path).expect("path"),
            value,
        }],
        &Limits::default(),
    )
}

fn checksum_at(bytes: &[u8]) -> usize {
    bytes.len() - 4
}

fn checksum(bytes: &[u8]) -> u32 {
    bytes[..checksum_at(bytes)]
        .iter()
        .fold(0u32, |sum, byte| sum.wrapping_add(u32::from(*byte)))
}

fn starts(bytes: &[u8]) -> Vec<usize> {
    let mut out = Vec::with_capacity(22);
    let mut at = 0usize;
    for _ in 0..22 {
        out.push(at);
        let size = u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"));
        at += 4 + usize::try_from(size).expect("size");
    }
    out
}

#[test]
fn sniff_needs_the_classic_size_and_a_plausible_first_block() {
    let bytes = fixture();
    assert!(Gta3Codec.sniff(&bytes, &SniffCtx::default()).value() > 0.9);
    assert_eq!(
        Gta3Codec.sniff(b"not a save", &SniffCtx::default()).value(),
        0.0
    );
    let mut short = bytes.clone();
    short.truncate(short.len() - 1);
    assert_eq!(Gta3Codec.sniff(&short, &SniffCtx::default()).value(), 0.0);
    let mut no_size = bytes;
    no_size[..4].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(Gta3Codec.sniff(&no_size, &SniffCtx::default()).value(), 0.0);
}

#[test]
fn the_invented_save_round_trips_and_shows_its_fields() {
    let bytes = fixture();
    let decoded = decode(&bytes);
    assert_eq!(encode(&decoded), bytes);
    assert_eq!(
        value(&decoded, "/title"),
        Value::Str("Invented Save".to_owned())
    );
    assert_eq!(value(&decoded, "/current_island"), int(1, IntWidth::U32));
    assert_eq!(value(&decoded, "/game_time/hour"), int(8, IntWidth::U8));
    assert_eq!(value(&decoded, "/game_time/minute"), int(27, IntWidth::U8));
    assert_eq!(value(&decoded, "/player/money"), int(80_000, IntWidth::U32));
    assert_eq!(
        value(&decoded, "/player/money_on_screen"),
        int(80_000, IntWidth::U32)
    );
    assert_eq!(value(&decoded, "/max_wanted_level"), int(4, IntWidth::U32));
    assert_eq!(value(&decoded, "/max_chaos"), int(1800, IntWidth::U32));
}

#[test]
fn edits_rewrite_their_fields_and_the_checksum() {
    let bytes = fixture();
    let starts = starts(&bytes);
    let mut decoded = decode(&bytes);
    set(&mut decoded, "/player/money", OwnedValue::Int(123_456)).expect("money edit");
    set(&mut decoded, "/game_time/hour", OwnedValue::Int(21)).expect("hour edit");
    set(&mut decoded, "/max_wanted_level", OwnedValue::Int(6)).expect("wanted edit");
    let written = encode(&decoded);
    assert_eq!(written.len(), bytes.len());
    let block0 = starts[0] + 4;
    assert_eq!(written[block0 + 0x5C], 21);
    let block1 = starts[1] + 8;
    assert_eq!(
        u32::from_le_bytes(
            written[block1 + 1534..block1 + 1538]
                .try_into()
                .expect("four")
        ),
        6
    );
    let block16 = starts[16] + 8;
    assert_eq!(
        u32::from_le_bytes(written[block16..block16 + 4].try_into().expect("four")),
        123_456
    );
    assert_eq!(
        u32::from_le_bytes(
            written[checksum_at(&written)..checksum_at(&written) + 4]
                .try_into()
                .expect("four bytes")
        ),
        checksum(&written)
    );
    let again = decode(&written);
    assert_eq!(value(&again, "/player/money"), int(123_456, IntWidth::U32));
    assert_eq!(value(&again, "/game_time/hour"), int(21, IntWidth::U8));
    assert_eq!(value(&again, "/max_wanted_level"), int(6, IntWidth::U32));
    assert_eq!(encode(&again), written, "an edited save is stable");
}

#[test]
fn a_wrong_checksum_is_kept_when_nothing_is_edited() {
    let mut bytes = fixture();
    let at = checksum_at(&bytes);
    bytes[at..at + 4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
    let decoded = decode(&bytes);
    assert_eq!(encode(&decoded), bytes);
}

#[test]
fn bad_sizes_and_truncation_are_refused() {
    let bytes = fixture();

    let mut small = bytes.clone();
    small[..4].copy_from_slice(&3u32.to_le_bytes());
    assert!(decode_with(&small, &Limits::default()).is_err());

    let mut shifted = bytes.clone();
    let second = starts(&bytes)[1];
    shifted[second..second + 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
    assert!(decode_with(&shifted, &Limits::default()).is_err());

    assert!(decode_with(&bytes[..bytes.len() - 1], &Limits::default()).is_err());
}

#[test]
fn a_missing_original_is_a_write_error() {
    let decoded = decode(&fixture());
    let empty = saveprism_core::Params::new();
    assert!(Gta3Codec.encode(&decoded.doc, &empty).is_err());
}

#[test]
fn an_edit_outside_the_u32_range_is_refused() {
    let mut decoded = decode(&fixture());
    assert!(
        set(&mut decoded, "/player/money", OwnedValue::Int(-1)).is_err(),
        "the U32 node rejects a negative value before writing"
    );
}

#[test]
fn an_edit_of_a_read_only_field_is_refused() {
    let mut decoded = decode(&fixture());
    assert!(set(&mut decoded, "/title", OwnedValue::Str("Other".to_owned())).is_err());
    assert!(set(&mut decoded, "/current_island", OwnedValue::Int(2)).is_err());
}
