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

/// The raw payload of a list holding one compound `{Y: <y>}`.
fn one_section(y: u8) -> [u8; 11] {
    [10, 0, 0, 0, 1, 1, 0, 1, b'Y', y, 0]
}

/// A region with one uncompressed chunk holding a string, a byte, int and long array, a root
/// `sections` list, and a pre-1.18 `Level` with `Sections` and a deeper `Deep/sections` byte.
fn region_with_arrays() -> Vec<u8> {
    let mut nbt = vec![0x0A, 0, 0];
    nbt.extend_from_slice(&[0x08, 0, 4]);
    nbt.extend_from_slice(b"Name");
    nbt.extend_from_slice(&[0, 1, b'a']);
    nbt.extend_from_slice(&[0x07, 0, 5]);
    nbt.extend_from_slice(b"Light");
    nbt.extend_from_slice(&[0, 0, 0, 3, 1, 2, 3]);
    nbt.extend_from_slice(&[0x0B, 0, 3]);
    nbt.extend_from_slice(b"Ids");
    nbt.extend_from_slice(&[0, 0, 0, 2, 0, 0, 0, 7, 0xFF, 0xFF, 0xFF, 0xFF]);
    nbt.extend_from_slice(&[0x0C, 0, 6]);
    nbt.extend_from_slice(b"States");
    nbt.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 5]);
    nbt.extend_from_slice(&[0x09, 0, 8]);
    nbt.extend_from_slice(b"sections");
    nbt.extend_from_slice(&one_section(0));
    nbt.extend_from_slice(&[0x0A, 0, 5]);
    nbt.extend_from_slice(b"Level");
    nbt.extend_from_slice(&[0x09, 0, 8]);
    nbt.extend_from_slice(b"Sections");
    nbt.extend_from_slice(&one_section(5));
    nbt.extend_from_slice(&[0x0A, 0, 4]);
    nbt.extend_from_slice(b"Deep");
    nbt.extend_from_slice(&[0x01, 0, 8]);
    nbt.extend_from_slice(b"sections");
    nbt.extend_from_slice(&[7, 0, 0]);
    nbt.push(0);
    let mut region = vec![0u8; 8192];
    region[..4].copy_from_slice(&[0, 0, 2, 1]);
    region.extend_from_slice(&u32::try_from(nbt.len() + 1).expect("small").to_be_bytes());
    region.push(3);
    region.extend_from_slice(&nbt);
    region.resize(8192 + 4096, 0);
    region
}

#[test]
fn chunk_arrays_and_sections_are_one_read_only_block_each() {
    let input = region_with_arrays();
    let mut decoded = decode(&input);
    assert_eq!(encode(&decoded), input);
    assert_eq!(
        value(&decoded, "/chunks/0/data/sections"),
        Value::Bytes(one_section(0).to_vec())
    );
    assert_eq!(
        value(&decoded, "/chunks/0/data/Level/Sections"),
        Value::Bytes(one_section(5).to_vec())
    );
    assert_eq!(
        value(&decoded, "/chunks/0/data/Level/Deep/sections"),
        Value::Int {
            v: 7,
            width: saveprism_core::IntWidth::I8
        }
    );
    assert_eq!(
        value(&decoded, "/chunks/0/data/Light"),
        Value::Bytes(vec![1, 2, 3])
    );
    assert_eq!(
        value(&decoded, "/chunks/0/data/Ids"),
        Value::Bytes(vec![0, 0, 0, 7, 0xFF, 0xFF, 0xFF, 0xFF])
    );
    let states = decoded
        .doc
        .resolve(&Path::parse("/chunks/0/data/States").expect("path"))
        .expect("node");
    assert!(
        decoded
            .doc
            .get(states)
            .expect("node")
            .flags
            .contains(saveprism_core::NodeFlags::READ_ONLY)
    );

    set(
        &mut decoded,
        "/chunks/0/data/Name",
        OwnedValue::Str("b".into()),
    );
    let again = decode(&encode(&decoded));
    assert_eq!(value(&again, "/chunks/0/data/Name"), Value::Str("b".into()));
    assert_eq!(
        value(&again, "/chunks/0/data/Light"),
        Value::Bytes(vec![1, 2, 3])
    );
    assert_eq!(
        value(&again, "/chunks/0/data/States"),
        Value::Bytes(vec![0, 0, 0, 0, 0, 0, 0, 5])
    );
    assert_eq!(
        value(&again, "/chunks/0/data/Level/Sections"),
        Value::Bytes(one_section(5).to_vec())
    );
}

#[test]
#[ignore = "reads the local, git-ignored fixtures/external/minecraft-region regions"]
fn real_regions_round_trip_and_edit_one_chunk() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/external/minecraft-region");
    let Ok(entries) = std::fs::read_dir(&root) else {
        eprintln!("no real regions under {}", root.display());
        return;
    };
    let mut regions: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mca"))
        .collect();
    regions.sort();
    assert!(!regions.is_empty());
    for path in regions {
        let input = std::fs::read(&path).expect("region reads");
        let mut decoded = decode(&input);
        assert_eq!(encode(&decoded), input, "{}", path.display());
        let Value::Int { v: edited, .. } = value(&decoded, "/chunks/0/index") else {
            panic!("{} has no chunk index", path.display());
        };
        let Value::Int { v: inhabited, .. } = value(&decoded, "/chunks/0/data/InhabitedTime")
        else {
            panic!("{} has no InhabitedTime in its first chunk", path.display());
        };
        set(
            &mut decoded,
            "/chunks/0/data/InhabitedTime",
            OwnedValue::Int(inhabited + 1000),
        );
        let output = encode(&decoded);
        let again = decode(&output);
        assert_eq!(
            value(&again, "/chunks/0/data/InhabitedTime"),
            Value::Int {
                v: inhabited + 1000,
                width: saveprism_core::IntWidth::I64
            }
        );
        let old = read::parse_header(&input).expect("old header");
        let new = read::parse_header(&output).expect("new header");
        for (index, (before, after)) in old.locations.iter().zip(&new.locations).enumerate() {
            if i128::try_from(index).expect("small") == edited {
                continue;
            }
            match (before, after) {
                (Some(before), Some(after)) => assert_eq!(
                    read::read_raw(&input, *before)
                        .expect("old chunk")
                        .compressed,
                    read::read_raw(&output, *after)
                        .expect("new chunk")
                        .compressed,
                    "{}: chunk {index} changed",
                    path.display()
                ),
                (None, None) => {}
                _ => panic!("{}: chunk {index} appeared or vanished", path.display()),
            }
        }
    }
}
