//! Tests for the GTA:SA codec.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use std::path::PathBuf;

use saveprism_core::{
    Codec, DecodeCtx, Decoded, Edit, FloatWidth, IntWidth, Limits, OwnedValue, Path, SniffCtx,
    Value, apply_edits,
};

use super::{GtaSaCodec, read::MARKER_COUNT};

fn fixture() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/synthetic/gta-sa/invented.save");
    std::fs::read(path).expect("the GTA:SA fixture exists")
}

fn decode_with(bytes: &[u8], limits: &Limits) -> saveprism_core::Result<Decoded> {
    GtaSaCodec.decode(bytes, &mut DecodeCtx::new(limits))
}

fn decode(bytes: &[u8]) -> Decoded {
    decode_with(bytes, &Limits::default()).expect("the save parses")
}

fn encode(decoded: &Decoded) -> Vec<u8> {
    GtaSaCodec
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

fn float(value: f64) -> Value {
    Value::Float {
        v: value,
        width: FloatWidth::F32,
    }
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

fn starts(bytes: &[u8]) -> [usize; MARKER_COUNT] {
    let mut out = [0usize; MARKER_COUNT];
    let mut at = 0usize;
    for slot in &mut out {
        let found = bytes[at..]
            .windows(5)
            .position(|window| window == b"BLOCK")
            .expect("marker");
        *slot = at + found;
        at = *slot + 5;
    }
    out
}

#[test]
fn sniff_needs_the_classic_size_and_the_first_marker() {
    let bytes = fixture();
    assert!(GtaSaCodec.sniff(&bytes, &SniffCtx::default()).value() > 0.9);
    assert_eq!(
        GtaSaCodec
            .sniff(b"not a save", &SniffCtx::default())
            .value(),
        0.0
    );
    let mut short = bytes.clone();
    short.truncate(short.len() - 1);
    assert_eq!(GtaSaCodec.sniff(&short, &SniffCtx::default()).value(), 0.0);
    let mut no_block = bytes;
    no_block[..5].copy_from_slice(b"BLOCH");
    assert_eq!(
        GtaSaCodec.sniff(&no_block, &SniffCtx::default()).value(),
        0.0
    );
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
    assert_eq!(value(&decoded, "/game_time/hour"), int(9, IntWidth::U8));
    assert_eq!(value(&decoded, "/game_time/minute"), int(41, IntWidth::U8));
    assert_eq!(
        value(&decoded, "/game_time/day_of_week"),
        int(2, IntWidth::U8)
    );
    assert_eq!(value(&decoded, "/player/health"), float(75.5));
    assert_eq!(value(&decoded, "/player/armour"), float(12.25));
    assert_eq!(
        value(&decoded, "/player/max_health"),
        int(150, IntWidth::U8)
    );
    assert_eq!(
        value(&decoded, "/player/max_armour"),
        int(100, IntWidth::U8)
    );
    assert_eq!(
        value(&decoded, "/player/money"),
        int(250_000, IntWidth::I32)
    );
    assert_eq!(
        value(&decoded, "/player/money_on_screen"),
        int(250_000, IntWidth::I32)
    );
}

#[test]
fn edits_rewrite_both_block_15_copies_and_the_checksum() {
    let bytes = fixture();
    let starts = starts(&bytes);
    let mut decoded = decode(&bytes);
    set(&mut decoded, "/player/money", OwnedValue::Int(999_999)).expect("money edit");
    set(&mut decoded, "/player/health", OwnedValue::Float(42.0)).expect("health edit");
    set(&mut decoded, "/player/max_health", OwnedValue::Int(200)).expect("max health edit");
    let written = encode(&decoded);
    assert_eq!(written.len(), bytes.len());

    let primary = starts[15] + 5;
    let mirror = starts[30] + 5;
    assert_eq!(
        i32::from_le_bytes(
            written[primary + 0x04..primary + 0x08]
                .try_into()
                .expect("four")
        ),
        999_999
    );
    assert_eq!(
        i32::from_le_bytes(
            written[mirror + 0x04..mirror + 0x08]
                .try_into()
                .expect("four")
        ),
        999_999
    );
    assert_eq!(written[primary + 0x23], 200);
    assert_eq!(written[mirror + 0x23], 200);
    let health = starts[2] + 5 + 0x20;
    assert_eq!(
        f32::from_le_bytes(written[health..health + 4].try_into().expect("four")),
        42.0
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
    assert_eq!(value(&again, "/player/money"), int(999_999, IntWidth::I32));
    assert_eq!(value(&again, "/player/health"), float(42.0));
    assert_eq!(value(&again, "/player/max_health"), int(200, IntWidth::U8));
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
fn bad_markers_and_block_lengths_are_refused() {
    let bytes = fixture();

    let mut not_block = bytes.clone();
    not_block[0x200..0x205].copy_from_slice(b"BLOCH");
    assert!(decode_with(&not_block, &Limits::default()).is_err());

    let mut short_block = bytes.clone();
    short_block[0x59D1..0x59D6].copy_from_slice(b"BLOCH");
    assert!(decode_with(&short_block, &Limits::default()).is_err());

    assert!(decode_with(&bytes[..bytes.len() - 1], &Limits::default()).is_err());
}

#[test]
fn a_missing_original_is_a_write_error() {
    let decoded = decode(&fixture());
    let empty = saveprism_core::Params::new();
    assert!(GtaSaCodec.encode(&decoded.doc, &empty).is_err());
}

#[test]
fn an_edit_outside_the_i32_range_is_refused() {
    let mut decoded = decode(&fixture());
    assert!(
        set(
            &mut decoded,
            "/player/money",
            OwnedValue::Int(i128::from(i32::MAX) + 1)
        )
        .is_err(),
        "the I32 node rejects the value before writing"
    );
}

#[test]
fn an_edit_of_a_read_only_field_is_refused() {
    let mut decoded = decode(&fixture());
    assert!(set(&mut decoded, "/title", OwnedValue::Str("Other".to_owned())).is_err());
}
