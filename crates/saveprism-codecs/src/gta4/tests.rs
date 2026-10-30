//! Tests for the GTA IV codec.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

use std::path::PathBuf;

use saveprism_core::{
    Codec, DecodeCtx, Decoded, Edit, FloatWidth, IntWidth, Limits, OwnedValue, Path, SniffCtx,
    Value, apply_edits,
};

use super::Gta4Codec;

fn fixture() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/synthetic/gta-iv/invented.save");
    std::fs::read(path).expect("the GTA IV fixture exists")
}

fn decode_with(bytes: &[u8], limits: &Limits) -> saveprism_core::Result<Decoded> {
    Gta4Codec.decode(bytes, &mut DecodeCtx::new(limits))
}

fn decode(bytes: &[u8]) -> Decoded {
    decode_with(bytes, &Limits::default()).expect("the save parses")
}

fn encode(decoded: &Decoded) -> Vec<u8> {
    Gta4Codec
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
    bytes.len() - 4 - 0x16C
}

fn checksum(bytes: &[u8]) -> u32 {
    bytes[..checksum_at(bytes)]
        .iter()
        .fold(0u32, |sum, byte| sum.wrapping_add(u32::from(*byte)))
}

#[test]
fn sniff_needs_the_metadata_and_the_simple_vars_block() {
    let bytes = fixture();
    assert!(Gta4Codec.sniff(&bytes, &SniffCtx::default()).value() > 0.9);
    assert_eq!(
        Gta4Codec.sniff(b"not a save", &SniffCtx::default()).value(),
        0.0
    );
    let mut no_save = bytes.clone();
    no_save[0x0C..0x10].copy_from_slice(b"NOPE");
    assert_eq!(Gta4Codec.sniff(&no_save, &SniffCtx::default()).value(), 0.0);
    let mut wrong_size = bytes;
    wrong_size[0x115..0x119].copy_from_slice(&0xB8u32.to_le_bytes());
    assert_eq!(
        Gta4Codec.sniff(&wrong_size, &SniffCtx::default()).value(),
        0.0
    );
}

#[test]
fn the_invented_save_round_trips_and_shows_its_fields() {
    let bytes = fixture();
    let decoded = decode(&bytes);
    assert_eq!(encode(&decoded), bytes);
    assert_eq!(value(&decoded, "/version"), int(0x39, IntWidth::U32));
    assert_eq!(
        value(&decoded, "/last_mission"),
        Value::Str("Invented Mission".to_owned())
    );
    assert_eq!(value(&decoded, "/game_time/month"), int(5, IntWidth::U32));
    assert_eq!(value(&decoded, "/game_time/day"), int(14, IntWidth::U32));
    assert_eq!(value(&decoded, "/game_time/hour"), int(17, IntWidth::U32));
    assert_eq!(value(&decoded, "/game_time/minute"), int(21, IntWidth::U32));
    assert_eq!(
        value(&decoded, "/game_time/day_of_week"),
        int(3, IntWidth::U32)
    );
    assert_eq!(
        value(&decoded, "/player/money"),
        int(123_456, IntWidth::U32)
    );
    assert_eq!(
        value(&decoded, "/player/money_on_screen"),
        int(123_456, IntWidth::U32)
    );
    assert_eq!(
        value(&decoded, "/player/max_health"),
        int(200, IntWidth::U16)
    );
    assert_eq!(
        value(&decoded, "/player/max_armour"),
        int(100, IntWidth::U16)
    );
    assert_eq!(value(&decoded, "/player/health"), float(150.5));
    assert_eq!(value(&decoded, "/player/armour"), float(25.25));
    assert_eq!(value(&decoded, "/max_wanted_level"), int(6, IntWidth::U32));
    assert_eq!(value(&decoded, "/max_chaos"), int(6900, IntWidth::U32));
}

#[test]
fn edits_rewrite_only_their_fields_and_the_checksum() {
    let bytes = fixture();
    let mut decoded = decode(&bytes);
    set(&mut decoded, "/player/money", OwnedValue::Int(999_999)).expect("money edit");
    set(&mut decoded, "/player/health", OwnedValue::Float(42.0)).expect("health edit");
    set(&mut decoded, "/player/max_health", OwnedValue::Int(250)).expect("max health edit");
    let written = encode(&decoded);
    assert_eq!(written.len(), bytes.len());
    assert_eq!(&written[..0x110], &bytes[..0x110]);
    // Money, health and max health changed; the checksum changed too.
    assert_ne!(
        &written[0x1c9 + 9 + 0x18..0x1c9 + 9 + 0x1c],
        &bytes[0x1c9 + 9 + 0x18..0x1c9 + 9 + 0x1c]
    );
    assert_ne!(
        &written[0x1c9 + 9 + 0x60..0x1c9 + 9 + 0x64],
        &bytes[0x1c9 + 9 + 0x60..0x1c9 + 9 + 0x64]
    );
    assert_ne!(
        &written[checksum_at(&bytes)..checksum_at(&bytes) + 4],
        &bytes[checksum_at(&bytes)..checksum_at(&bytes) + 4]
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
    assert_eq!(value(&again, "/player/money"), int(999_999, IntWidth::U32));
    assert_eq!(value(&again, "/player/health"), float(42.0));
    assert_eq!(value(&again, "/player/max_health"), int(250, IntWidth::U16));
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
fn bad_blocks_and_end_marks_are_refused() {
    let bytes = fixture();

    let mut not_block = bytes.clone();
    not_block[0x110..0x115].copy_from_slice(b"BLOCH");
    assert!(decode_with(&not_block, &Limits::default()).is_err());

    let mut short_block = bytes.clone();
    short_block[0x115..0x119].copy_from_slice(&4u32.to_le_bytes());
    assert!(decode_with(&short_block, &Limits::default()).is_err());

    let mut no_end = bytes.clone();
    let end = checksum_at(&bytes) + 4;
    no_end[end..end + 4].copy_from_slice(b"BAD\0");
    assert!(decode_with(&no_end, &Limits::default()).is_err());

    assert!(decode_with(&bytes[..bytes.len() - 1], &Limits::default()).is_err());
}

#[test]
fn a_missing_original_is_a_write_error() {
    let decoded = decode(&fixture());
    let empty = saveprism_core::Params::new();
    assert!(Gta4Codec.encode(&decoded.doc, &empty).is_err());
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
    assert!(
        set(
            &mut decoded,
            "/last_mission",
            OwnedValue::Str("Other".to_owned())
        )
        .is_err()
    );
}
