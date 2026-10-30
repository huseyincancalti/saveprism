//! Detector tests with the real layers and the JSON codec.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]

use saveprism_codecs::register_all;
use saveprism_core::detect::{DetectOptions, detect};
use saveprism_core::pipeline;
use saveprism_core::{Limits, Recipe, Registry, Step, codes};

const JSON: &str = r#"{"name":"hero","level":7,"items":["sword","shield"],"gold":120}"#;

fn registry() -> Registry {
    let mut reg = Registry::default();
    register_all(&mut reg);
    reg
}

/// The file bytes of `JSON` wrapped in `layers` (outermost first), made with the pipeline.
fn make_input(reg: &Registry, layers: &[&str]) -> Vec<u8> {
    let limits = Limits::default();
    let plain = pipeline::decode(
        JSON.as_bytes(),
        &Recipe::new(vec![Step::bare("format.json")]),
        reg,
        &limits,
    )
    .unwrap();
    let codec_params = plain.recipe.steps.last().unwrap().params.clone();
    let mut steps: Vec<Step> = layers.iter().map(|id| Step::bare(*id)).collect();
    steps.push(Step::new("format.json", codec_params));
    pipeline::encode(&plain.doc, &Recipe::new(steps), reg).unwrap()
}

/// Detects `input`, checks the recipe kinds and that decode then encode gives the input back.
fn check(reg: &Registry, input: &[u8], expected: &[&str]) {
    let limits = Limits::default();
    let detection = detect(input, reg, &limits, &DetectOptions::default()).unwrap();
    assert_eq!(detection.best.recipe.kinds(), expected);
    let decoded = pipeline::decode(input, &detection.best.recipe, reg, &limits).unwrap();
    let again = pipeline::encode(&decoded.doc, &decoded.recipe, reg).unwrap();
    assert_eq!(again, input);
}

#[test]
fn plain_json() {
    let reg = registry();
    let input = make_input(&reg, &[]);
    assert_eq!(input, JSON.as_bytes());
    check(&reg, &input, &["format.json"]);
}

#[test]
fn base64_json() {
    let reg = registry();
    let input = make_input(&reg, &["encoding.base64"]);
    check(&reg, &input, &["encoding.base64", "format.json"]);
}

#[test]
fn gzip_json() {
    let reg = registry();
    let input = make_input(&reg, &["compression.gzip"]);
    check(&reg, &input, &["compression.gzip", "format.json"]);
}

#[test]
fn zlib_json() {
    let reg = registry();
    let input = make_input(&reg, &["compression.zlib"]);
    check(&reg, &input, &["compression.zlib", "format.json"]);
}

#[test]
fn lz_string_json_is_not_taken_for_base64() {
    let reg = registry();
    let input = make_input(&reg, &["compression.lz-string"]);
    check(&reg, &input, &["compression.lz-string", "format.json"]);
}

#[test]
fn url_escaped_json_is_a_tyrano_save() {
    let reg = registry();
    let input = make_input(&reg, &["encoding.url-escape"]);
    check(&reg, &input, &["encoding.url-escape", "format.json"]);
}

#[test]
fn base64_of_gzip_json() {
    let reg = registry();
    let input = make_input(&reg, &["encoding.base64", "compression.gzip"]);
    check(
        &reg,
        &input,
        &["encoding.base64", "compression.gzip", "format.json"],
    );
}

#[test]
fn small_byte_limit_still_finds_the_right_recipe() {
    let reg = registry();
    let input = make_input(&reg, &["encoding.base64", "compression.gzip"]);
    // Only a little above the input size: the frontier must stay within this budget.
    let limits = Limits {
        max_decoded_bytes: 1024,
        ..Limits::default()
    };
    let detection = detect(&input, &reg, &limits, &DetectOptions::default()).unwrap();
    assert_eq!(
        detection.best.recipe.kinds(),
        ["encoding.base64", "compression.gzip", "format.json"]
    );
}

#[test]
fn random_bytes_match_nothing() {
    let reg = registry();
    // Deterministic pseudo-random bytes; the first byte rules out every sniffer.
    let mut state: u32 = 0x1234_5678;
    let mut input = vec![0xFF_u8];
    for _ in 0..512 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        input.push(state.to_le_bytes()[3]);
    }
    let error = detect(&input, &reg, &Limits::default(), &DetectOptions::default()).unwrap_err();
    assert_eq!(error.code(), codes::DET_NO_MATCH);
}

/// Every fixture under `fixtures/` must still detect as the recipe recorded in
/// `fixtures/expected/`; a new structured-text sniff must not steal another codec's files.
#[test]
fn no_existing_fixture_is_stolen_by_a_new_sniffer() {
    use std::fs;
    use std::path::{Path, PathBuf};

    fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name.starts_with("README") {
                continue;
            }
            let path = entry.path();
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => collect(&path, files),
                Ok(kind) if kind.is_file() => files.push(path),
                _ => {}
            }
        }
    }

    fn expected_recipe(path: &Path) -> Option<String> {
        let text = fs::read_to_string(path).ok()?;
        text.lines()
            .find_map(|line| line.trim().strip_prefix("recipe = \""))
            .and_then(|rest| rest.strip_suffix('"'))
            .map(str::to_owned)
    }

    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let reg = registry();
    let limits = Limits::default();
    let mut checked = 0usize;
    for root in ["samples", "synthetic"] {
        let dir = fixtures.join(root);
        let mut files = Vec::new();
        collect(&dir, &mut files);
        files.sort();
        for file in files {
            let rel = file
                .strip_prefix(&dir)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            let expected_path = fixtures
                .join("expected")
                .join(root)
                .join(format!("{rel}.toml"));
            let Some(expected) = expected_recipe(&expected_path) else {
                continue;
            };
            let bytes = fs::read(&file).expect("fixture readable");
            let name = file
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let opts = DetectOptions {
                file_name: Some(name),
                ..DetectOptions::default()
            };
            let Ok(detection) = detect(&bytes, &reg, &limits, &opts) else {
                panic!("{root}/{rel}: nothing detected");
            };
            let found = detection.best.recipe.kinds().join(" > ");
            assert_eq!(found, expected, "{root}/{rel} changed codec");
            checked += 1;
        }
    }
    assert!(
        checked > 50,
        "expected to check the whole corpus, got {checked}"
    );
}

#[test]
fn structured_text_is_recognised_by_content() {
    let reg = registry();
    let cases: &[(&str, &[u8], &str)] = &[
        (
            "save.dat",
            b"<?xml version=\"1.0\"?>\n<save><gold>7</gold></save>\n",
            "format.xml",
        ),
        (
            "binary.plist",
            include_bytes!("../../../fixtures/synthetic/plist/binary.plist"),
            "format.plist",
        ),
        (
            "save",
            b"# yaml\nplayer:\n  name: Mira\n  level: 4\n",
            "format.yaml",
        ),
        (
            "save",
            b"name,class,level\nMira,Ranger,42\nRune,Mage,7\n",
            "format.csv",
        ),
        ("save", b"[Audio]\nvolume = 0.75\nmusic=on\n", "format.ini"),
        (
            "save",
            b"# properties\nplayer.name = Mira\nplayer.level=4\n",
            "format.properties",
        ),
        (
            "[Player]",
            b"[Player]\nlevel = 4\nname = \"Mira\"\n",
            "format.godot-config",
        ),
    ];
    for (name, bytes, expected) in cases {
        let opts = DetectOptions {
            file_name: Some((*name).to_owned()),
            ..DetectOptions::default()
        };
        let detection = detect(bytes, &reg, &Limits::default(), &opts)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert_eq!(
            detection.best.recipe.kinds(),
            [*expected],
            "{name} detected as the wrong codec"
        );
    }
}

#[test]
fn godot_resources_are_detected_by_content_and_config_files_stay_config() {
    let reg = registry();
    let cases: &[(&str, &[u8], &str)] = &[
        (
            "player.tres",
            include_bytes!("../../../fixtures/synthetic/godot/resource4.tres"),
            "format.godot-resource-text",
        ),
        (
            "player.tres",
            include_bytes!("../../../fixtures/synthetic/godot/resource3.tres"),
            "format.godot-resource-text",
        ),
        (
            "save.res",
            include_bytes!("../../../fixtures/synthetic/godot/resource4.res"),
            "format.godot-resource",
        ),
        (
            "save.res",
            include_bytes!("../../../fixtures/synthetic/godot/resource3.res"),
            "format.godot-resource",
        ),
        (
            "save.res",
            include_bytes!("../../../fixtures/synthetic/godot/resource4-deflate.res"),
            "format.godot-resource",
        ),
        (
            "settings.cfg",
            b"[Player]\nlevel = 4\nname = \"Mira\"\n",
            "format.godot-config",
        ),
    ];
    for (name, bytes, expected) in cases {
        let opts = DetectOptions {
            file_name: Some((*name).to_owned()),
            ..DetectOptions::default()
        };
        let detection = detect(bytes, &reg, &Limits::default(), &opts)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert_eq!(
            detection.best.recipe.kinds(),
            [*expected],
            "{name} detected as the wrong codec"
        );
    }
}

#[test]
fn plain_text_and_json_are_not_taken_by_structured_text() {
    let reg = registry();
    for (name, bytes) in [
        ("save.txt", &b"{\"gold\": 7, \"name\": \"Mira\"}"[..]),
        ("save.json", &b"[1, 2, 3]"[..]),
    ] {
        let opts = DetectOptions {
            file_name: Some(name.to_owned()),
            ..DetectOptions::default()
        };
        let detection = detect(bytes, &reg, &Limits::default(), &opts).expect("detected");
        assert_eq!(detection.best.recipe.kinds(), ["format.json"], "{name}");
    }
}

#[test]
fn layers_without_a_magic_are_found_by_content() {
    let reg = registry();
    let detect_named = |name: &str, bytes: &[u8]| {
        let opts = DetectOptions {
            file_name: Some(name.to_owned()),
            ..DetectOptions::default()
        };
        detect(bytes, &reg, &Limits::default(), &opts).expect("detected")
    };

    // Brotli with a name that says nothing about the format.
    let brotli = include_bytes!("../../../fixtures/synthetic/layers/save-json-brotli.dat");
    assert_eq!(
        detect_named("save.dat", brotli).best.recipe.kinds(),
        ["compression.brotli", "format.json"]
    );

    // Raw deflate offered as a plain .dat file (Naninovel only used .nson before).
    let deflate = make_input(&reg, &["compression.deflate"]);
    assert_eq!(
        detect_named("save.dat", &deflate).best.recipe.kinds(),
        ["compression.deflate", "format.json"]
    );

    // LZF raw and framed, and a repeating-key XOR around JSON.
    let lzf = include_bytes!("../../../fixtures/synthetic/lzf/save.json.lzf");
    assert_eq!(
        detect_named("save.bin", lzf).best.recipe.kinds(),
        ["compression.lzf", "format.json"]
    );
    let framed = include_bytes!("../../../fixtures/synthetic/lzf/save.json.zv");
    assert_eq!(
        detect_named("save.bin", framed).best.recipe.kinds(),
        ["compression.lzf", "format.json"]
    );
    let xor = include_bytes!("../../../fixtures/synthetic/xor/save-json-key1.bin");
    assert_eq!(
        detect_named("save.bin", xor).best.recipe.kinds(),
        ["obfuscation.xor", "format.json"]
    );
}

#[test]
fn kirikiri_and_gamemaker_flavors_are_recognised() {
    let reg = registry();
    let cases: &[(&[u8], &[&str])] = &[
        (
            include_bytes!("../../../fixtures/synthetic/kirikiri/save01.ksd"),
            &["format.tjs2"],
        ),
        (
            include_bytes!("../../../fixtures/synthetic/kirikiri/system.kdt"),
            &["format.tjs2"],
        ),
        (
            include_bytes!("../../../fixtures/synthetic/kirikiri/scrambled.ksd"),
            &["obfuscation.kirikiri", "format.tjs2"],
        ),
        (
            include_bytes!("../../../fixtures/synthetic/kirikiri/compressed.ksd"),
            &["obfuscation.kirikiri", "format.tjs2"],
        ),
        (
            include_bytes!("../../../fixtures/synthetic/gamemaker/save.ini"),
            &["format.ini"],
        ),
        (
            include_bytes!("../../../fixtures/synthetic/gamemaker/save.json"),
            &["format.json"],
        ),
        (
            include_bytes!("../../../fixtures/synthetic/gamemaker/save-base64.txt"),
            &["encoding.base64", "format.json"],
        ),
        (
            include_bytes!("../../../fixtures/synthetic/gamemaker/ds-map.txt"),
            &["format.gamemaker-ds-map"],
        ),
        (
            include_bytes!("../../../fixtures/synthetic/gamemaker/ds-map-v403.txt"),
            &["format.gamemaker-ds-map"],
        ),
        (
            include_bytes!("../../../fixtures/synthetic/gamemaker/ds-list.txt"),
            &["format.gamemaker-ds-list"],
        ),
        (
            include_bytes!("../../../fixtures/synthetic/gamemaker/ds-list-legacy.txt"),
            &["format.gamemaker-ds-list"],
        ),
        (
            include_bytes!("../../../fixtures/synthetic/pixel-game-maker-mv/play_LOADFILE1.json"),
            &["format.json"],
        ),
    ];
    for (bytes, expected) in cases {
        check(&reg, bytes, expected);
    }
}

#[test]
fn bioware_fixtures_detect_as_gff_and_erf() {
    let reg = registry();
    let limits = Limits::default();
    let cases: &[(&str, &[&str])] = &[
        ("character.bic", &["format.gff"]),
        ("module.ifo", &["format.gff"]),
        ("kotor-save.sav", &["container.erf", "format.gff"]),
        ("dao-save.das", &["format.gff4"]),
    ];
    for (name, expected) in cases {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/synthetic/bioware")
            .join(name);
        let bytes = std::fs::read(&path).expect("fixture readable");
        let opts = DetectOptions {
            file_name: Some((*name).to_owned()),
            ..DetectOptions::default()
        };
        let detection = detect(&bytes, &reg, &limits, &opts)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert_eq!(detection.best.recipe.kinds(), *expected, "{name}");
        let decoded = pipeline::decode(&bytes, &detection.best.recipe, &reg, &limits)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let again = pipeline::encode(&decoded.doc, &decoded.recipe, &reg)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert_eq!(again, bytes, "{name} does not round-trip");
    }
}

#[test]
fn unity_asset_saves_are_claimed_by_their_specific_codec() {
    let reg = registry();
    let detect_named = |name: &str, bytes: &[u8]| {
        let opts = DetectOptions {
            file_name: Some(name.to_owned()),
            ..DetectOptions::default()
        };
        detect(bytes, &reg, &Limits::default(), &opts).expect("detected")
    };

    // Adventure Creator joins its parts with `||`; plain JSON or XML without it stays plain.
    let ac = [
        (
            "village.json.save",
            include_bytes!("../../../fixtures/synthetic/adventure-creator/village.json.save")
                .as_slice(),
        ),
        (
            "village.xml.save",
            include_bytes!("../../../fixtures/synthetic/adventure-creator/village.xml.save")
                .as_slice(),
        ),
        (
            "village.binary.save",
            include_bytes!("../../../fixtures/synthetic/adventure-creator/village.binary.save")
                .as_slice(),
        ),
    ];
    for (name, bytes) in ac {
        assert_eq!(
            detect_named(name, bytes).best.recipe.kinds(),
            ["format.adventure-creator"],
            "{name}"
        );
    }
    for (name, bytes) in [
        (
            "save.json",
            b"{\"mainData\":{\"currentPlayerID\":1},\"playerData\":[]}".as_slice(),
        ),
        (
            "save.xml",
            b"<SaveData><mainData><currentPlayerID>1</currentPlayerID></mainData></SaveData>"
                .as_slice(),
        ),
    ] {
        let kinds = detect_named(name, bytes).best.recipe.kinds().join(" > ");
        assert_ne!(kinds, "format.adventure-creator", "{name}");
    }

    // Pixel Crushers writes SavedGameData JSON with save records and nested strings.
    for (name, bytes) in [
        (
            "save0.dat",
            include_bytes!("../../../fixtures/synthetic/pixelcrushers/save0.dat").as_slice(),
        ),
        (
            "save1.dat",
            include_bytes!("../../../fixtures/synthetic/pixelcrushers/save1.dat").as_slice(),
        ),
    ] {
        assert_eq!(
            detect_named(name, bytes).best.recipe.kinds(),
            ["format.pixel-crushers"],
            "{name}"
        );
    }
    // A plain JSON object with a `records` key is not a Pixel Crushers save.
    for name in ["save.json", "save.dat"] {
        let bytes = b"{\"records\":[{\"key\":\"a\",\"data\":\"x\"}],\"items\":[1,2]}";
        assert_eq!(
            detect_named(name, bytes).best.recipe.kinds(),
            ["format.json"],
            "{name}"
        );
    }
}
