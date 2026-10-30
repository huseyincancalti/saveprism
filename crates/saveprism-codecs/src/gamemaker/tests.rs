//! Tests for the GameMaker `ds_map_write` codec.
#![allow(clippy::expect_used, clippy::indexing_slicing)]

use saveprism_core::{Codec, DecodeCtx, Edit, Limits, OwnedValue, Path, Value};

use super::{GameMakerDsListCodec, GameMakerDsMapCodec};

fn real(value: f64) -> Vec<u8> {
    let mut out = 0_u32.to_le_bytes().to_vec();
    out.extend_from_slice(&value.to_bits().to_le_bytes());
    out
}

fn text(value: &str) -> Vec<u8> {
    let mut out = 1_u32.to_le_bytes().to_vec();
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    out
}

fn int32(value: i32) -> Vec<u8> {
    let mut out = 7_u32.to_le_bytes().to_vec();
    out.extend_from_slice(&value.to_le_bytes());
    out
}

fn int64(value: i64) -> Vec<u8> {
    let mut out = 10_u32.to_le_bytes().to_vec();
    out.extend_from_slice(&value.to_le_bytes());
    out
}

fn boolean(value: bool) -> Vec<u8> {
    let mut out = 13_u32.to_le_bytes().to_vec();
    out.extend_from_slice(&(if value { 1.0_f64 } else { 0.0_f64 }).to_le_bytes());
    out
}

fn undefined() -> Vec<u8> {
    5_u32.to_le_bytes().to_vec()
}

fn array(values: &[Vec<u8>], version: u32) -> Vec<u8> {
    let mut out = 2_u32.to_le_bytes().to_vec();
    if version == 3 {
        out.extend_from_slice(&1_u32.to_le_bytes());
    }
    out.extend_from_slice(&(values.len() as u32).to_le_bytes());
    for value in values {
        out.extend_from_slice(value);
    }
    out
}

fn array_2d(rows: &[&[Vec<u8>]]) -> Vec<u8> {
    let mut out = 2_u32.to_le_bytes().to_vec();
    out.extend_from_slice(&(rows.len() as u32).to_le_bytes());
    for row in rows {
        out.extend_from_slice(&(row.len() as u32).to_le_bytes());
        for value in *row {
            out.extend_from_slice(value);
        }
    }
    out
}

fn list_bytes() -> Vec<u8> {
    let values = [
        real(3.5),
        text("Mira"),
        int32(-12),
        int64(9_007_199_254_740_993),
        boolean(false),
        undefined(),
        array(&[real(1.0), text("two")], 0),
    ];
    let mut out = 303_u32.to_le_bytes().to_vec();
    out.extend_from_slice(&(values.len() as u32).to_le_bytes());
    for value in values {
        out.extend_from_slice(&value);
    }
    out
}

fn legacy_list_bytes() -> Vec<u8> {
    let values = [
        real(1.5),
        array(&[real(1.0), text("two")], 3),
        array_2d(&[&[real(1.0), real(2.0)], &[text("three")]]),
    ];
    let mut out = 302_u32.to_le_bytes().to_vec();
    out.extend_from_slice(&(values.len() as u32).to_le_bytes());
    for value in values {
        out.extend_from_slice(&value);
    }
    out
}

fn map_bytes() -> Vec<u8> {
    let mut out = 402_u32.to_le_bytes().to_vec();
    out.extend_from_slice(&3_u32.to_le_bytes());
    out.extend_from_slice(&text("gold"));
    out.extend_from_slice(&real(120.0));
    out.extend_from_slice(&text("name"));
    out.extend_from_slice(&text("Mira"));
    out.extend_from_slice(&real(2.5));
    out.extend_from_slice(&text("pi"));
    out
}

fn hex(bytes: &[u8], upper: bool) -> Vec<u8> {
    bytes
        .iter()
        .flat_map(|byte| {
            let text = if upper {
                format!("{byte:02X}")
            } else {
                format!("{byte:02x}")
            };
            text.into_bytes()
        })
        .collect()
}

fn decode(input: &[u8]) -> saveprism_core::Decoded {
    GameMakerDsMapCodec
        .decode(input, &mut DecodeCtx::new(&Limits::default()))
        .expect("ds_map parses")
}

fn encode(decoded: &saveprism_core::Decoded) -> Vec<u8> {
    GameMakerDsMapCodec
        .encode(&decoded.doc, &decoded.params)
        .expect("ds_map writes")
}

fn decode_list(input: &[u8]) -> saveprism_core::Decoded {
    GameMakerDsListCodec
        .decode(input, &mut DecodeCtx::new(&Limits::default()))
        .expect("ds_list parses")
}

fn encode_list(decoded: &saveprism_core::Decoded) -> Vec<u8> {
    GameMakerDsListCodec
        .encode(&decoded.doc, &decoded.params)
        .expect("ds_list writes")
}

fn value(decoded: &saveprism_core::Decoded, path: &str) -> Value {
    let id = decoded
        .doc
        .resolve(&Path::parse(path).expect("path"))
        .expect("node");
    decoded
        .doc
        .get(id)
        .map(|node| node.value.clone())
        .expect("value")
}

#[test]
fn a_map_round_trips_and_edits_keep_its_shape() {
    let input = hex(&map_bytes(), true);
    let mut decoded = decode(&input);
    assert_eq!(encode(&decoded), input);
    assert_eq!(value(&decoded, "/1"), Value::Str("Mira".into()));
    assert_eq!(
        value(&decoded, "/2.key"),
        Value::Float {
            v: 2.5,
            width: saveprism_core::FloatWidth::F64
        }
    );
    saveprism_core::apply_edits(
        &mut decoded.doc,
        &[Edit::Set {
            path: Path::parse("/0").expect("path"),
            value: OwnedValue::Int(999),
        }],
        &Limits::default(),
    )
    .expect("edit applies");
    let output = encode(&decoded);
    let again = decode(&output);
    assert_eq!(
        value(&again, "/0"),
        Value::Float {
            v: 999.0,
            width: saveprism_core::FloatWidth::F64
        }
    );
    assert_eq!(value(&again, "/1"), Value::Str("Mira".into()));
}

#[test]
fn a_lowercase_dump_is_written_back_unchanged() {
    let input = hex(&map_bytes(), false);
    let decoded = decode(&input);
    assert_eq!(encode(&decoded), input);
}

#[test]
fn a_trailing_newline_is_kept() {
    let mut input = hex(&map_bytes(), true);
    input.extend_from_slice(b"\r\n");
    let decoded = decode(&input);
    assert_eq!(encode(&decoded), input);
}

#[test]
fn unknown_value_types_are_refused() {
    let mut bytes = 402_u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&text("gold"));
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    let input = hex(&bytes, true);
    assert!(
        GameMakerDsMapCodec
            .decode(&input, &mut DecodeCtx::new(&Limits::default()))
            .is_err()
    );
}

#[test]
fn generated_fixtures_round_trip() {
    let base =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/synthetic/gamemaker");
    for name in ["ds-map.txt", "ds-map-lower.txt"] {
        let input = std::fs::read(base.join(name)).expect("fixture exists");
        let decoded = decode(&input);
        assert_eq!(encode(&decoded), input, "{name}");
    }
}

#[test]
fn a_list_round_trips_and_edits_keep_its_shape() {
    let input = hex(&list_bytes(), true);
    let mut decoded = decode_list(&input);
    assert_eq!(encode_list(&decoded), input);
    assert_eq!(value(&decoded, "/1"), Value::Str("Mira".into()));
    assert_eq!(
        value(&decoded, "/2"),
        Value::Int {
            v: -12,
            width: saveprism_core::IntWidth::I32
        }
    );
    assert_eq!(value(&decoded, "/4"), Value::Bool(false));
    assert_eq!(value(&decoded, "/5"), Value::Null);
    assert_eq!(value(&decoded, "/6/1"), Value::Str("two".into()));
    saveprism_core::apply_edits(
        &mut decoded.doc,
        &[Edit::Set {
            path: Path::parse("/6/0").expect("path"),
            value: OwnedValue::Float(7.5),
        }],
        &Limits::default(),
    )
    .expect("edit applies");
    let output = encode_list(&decoded);
    let again = decode_list(&output);
    assert_eq!(
        value(&again, "/6/0"),
        Value::Float {
            v: 7.5,
            width: saveprism_core::FloatWidth::F64
        }
    );
    assert_eq!(value(&again, "/1"), Value::Str("Mira".into()));
}

#[test]
fn a_legacy_list_keeps_its_2d_arrays() {
    let input = hex(&legacy_list_bytes(), true);
    let decoded = decode_list(&input);
    assert_eq!(encode_list(&decoded), input);
    assert_eq!(value(&decoded, "/2/1/0"), Value::Str("three".into()));
}

#[test]
fn list_detection_uses_the_magic() {
    let input = hex(&list_bytes(), true);
    let ctx = saveprism_core::SniffCtx::default();
    assert!(GameMakerDsListCodec.sniff(&input, &ctx).value() > 0.5);
    assert_eq!(GameMakerDsMapCodec.sniff(&input, &ctx).value(), 0.0);
}

#[test]
fn generated_list_fixtures_round_trip() {
    let base =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/synthetic/gamemaker");
    for name in ["ds-list.txt", "ds-list-legacy.txt", "ds-map-v403.txt"] {
        let input = std::fs::read(base.join(name)).expect("fixture exists");
        if name.contains("list") {
            let decoded = decode_list(&input);
            assert_eq!(encode_list(&decoded), input, "{name}");
        } else {
            let decoded = decode(&input);
            assert_eq!(encode(&decoded), input, "{name}");
        }
    }
}
