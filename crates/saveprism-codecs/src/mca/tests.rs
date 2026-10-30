//! Tests for the Minecraft region codec.
#![allow(clippy::expect_used, clippy::indexing_slicing)]

use std::path::PathBuf;

use saveprism_core::detect::{DetectOptions, detect};
use saveprism_core::{
    Codec, DecodeCtx, Decoded, Edit, Limits, OwnedValue, Path, Registry, Value, apply_edits,
};

use super::*;

const FIXTURE: &str = "synthetic/minecraft-region/r.0.0.mca";

fn fixture() -> Vec<u8> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    std::fs::read(base.join(FIXTURE)).expect("fixture exists")
}

fn decode(input: &[u8]) -> Decoded {
    MinecraftRegionCodec
        .decode(input, &mut DecodeCtx::new(&Limits::default()))
        .expect("region parses")
}

fn encode(decoded: &Decoded) -> Vec<u8> {
    MinecraftRegionCodec
        .encode(&decoded.doc, &decoded.params)
        .expect("region writes")
}

fn value(decoded: &Decoded, path: &str) -> Value {
    let id = decoded
        .doc
        .resolve(&Path::parse(path).expect("path"))
        .expect("node");
    decoded.doc.get(id).expect("node").value.clone()
}

fn set(decoded: &mut Decoded, path: &str, value: OwnedValue) {
    apply_edits(
        &mut decoded.doc,
        &[Edit::Set {
            path: Path::parse(path).expect("path"),
            value,
        }],
        &Limits::default(),
    )
    .expect("edit applies");
}

fn registry() -> Registry {
    let mut reg = Registry::default();
    crate::register_all(&mut reg);
    reg
}

#[test]
fn the_fixture_round_trips_exactly() {
    let input = fixture();
    let decoded = decode(&input);
    assert_eq!(encode(&decoded), input);
}

#[test]
fn the_tree_shows_every_chunk() {
    let decoded = decode(&fixture());
    assert_eq!(
        value(&decoded, "/chunks/0/index"),
        Value::Int {
            v: 0,
            width: saveprism_core::IntWidth::I32
        }
    );
    assert_eq!(
        value(&decoded, "/chunks/0/data/Level/LevelName"),
        Value::Str("Invented".into())
    );
    assert_eq!(
        value(&decoded, "/chunks/1/compression"),
        Value::Int {
            v: 3,
            width: saveprism_core::IntWidth::I32
        }
    );
    assert_eq!(value(&decoded, "/chunks/3/external"), Value::Bool(false));
    let id = decoded
        .doc
        .resolve(&Path::parse("/chunks/3/note").expect("path"))
        .expect("note");
    assert!(matches!(
        decoded.doc.get(id).expect("node").value,
        Value::Str(_)
    ));
}

#[test]
fn an_edit_keeps_the_other_chunks_and_the_timestamp_table() {
    let input = fixture();
    let mut decoded = decode(&input);
    set(
        &mut decoded,
        "/chunks/0/data/Level/LevelName",
        OwnedValue::Str("Edited".into()),
    );
    let output = encode(&decoded);
    assert_ne!(output, input);
    let again = decode(&output);
    assert_eq!(
        value(&again, "/chunks/0/data/Level/LevelName"),
        Value::Str("Edited".into())
    );
    assert_eq!(
        value(&again, "/chunks/1/data/Level/LevelName"),
        Value::Str("Invented".into())
    );
    assert_eq!(
        value(&again, "/chunks/2/data/Level/LevelName"),
        Value::Str("Invented".into())
    );
    assert_eq!(
        output.get(4096..8192).expect("timestamp table"),
        input.get(4096..8192).expect("timestamp table")
    );
}

#[test]
fn a_large_edit_moves_the_chunk_to_a_new_sector() {
    let input = fixture();
    let mut decoded = decode(&input);
    let large: String = (0..9000)
        .map(|index| {
            let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
            char::from(alphabet[index % alphabet.len()])
        })
        .collect();
    set(
        &mut decoded,
        "/chunks/1/data/Level/LevelName",
        OwnedValue::Str(large.clone()),
    );
    let output = encode(&decoded);
    assert!(output.len() > input.len());
    let again = decode(&output);
    assert_eq!(
        value(&again, "/chunks/1/data/Level/LevelName"),
        Value::Str(large)
    );
}

#[test]
fn opaque_chunks_are_read_only() {
    let mut decoded = decode(&fixture());
    let error = apply_edits(
        &mut decoded.doc,
        &[Edit::Set {
            path: Path::parse("/chunks/3/raw").expect("path"),
            value: OwnedValue::Bytes(vec![1, 2, 3]),
        }],
        &Limits::default(),
    )
    .expect_err("raw chunks are read-only");
    assert_eq!(error.code(), saveprism_core::codes::CORE_READ_ONLY);
}

#[test]
fn a_bad_header_is_refused() {
    let mut input = fixture();
    input[0..4].copy_from_slice(&[0, 0, 0, 2]);
    assert!(
        MinecraftRegionCodec
            .decode(&input, &mut DecodeCtx::new(&Limits::default()))
            .is_err()
    );
}

#[test]
fn detection_uses_the_extension_and_the_header() {
    let input = fixture();
    let with_name = detect(
        &input,
        &registry(),
        &Limits::default(),
        &DetectOptions {
            file_name: Some("r.0.0.mca".into()),
            ..DetectOptions::default()
        },
    )
    .expect("detect");
    assert_eq!(
        with_name.best.recipe.steps.last().expect("step").kind,
        "format.minecraft-region"
    );
    let without_name = detect(
        &input,
        &registry(),
        &Limits::default(),
        &DetectOptions::default(),
    )
    .expect("detect");
    assert_eq!(
        without_name.best.recipe.steps.last().expect("step").kind,
        "format.minecraft-region"
    );
}
