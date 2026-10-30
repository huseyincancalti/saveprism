#![no_main]

use libfuzzer_sys::fuzz_target;
use saveprism_codecs::register_all;
use saveprism_core::{pipeline, Limits, Recipe, Registry, Step};

fuzz_target!(|data: &[u8]| {
    let mut registry = Registry::default();
    register_all(&mut registry);
    let limits = Limits {
        max_input_bytes: 4 * 1024 * 1024,
        max_decoded_bytes: 16 * 1024 * 1024,
        max_nodes: 1_000_000,
        ..Limits::default()
    };
    let recipe = Recipe::new(vec![Step::bare("format.minecraft-region")]);
    let Ok(decoded) = pipeline::decode(data, &recipe, &registry, &limits) else {
        return;
    };
    let encoded = pipeline::encode(&decoded.doc, &decoded.recipe, &registry)
        .expect("a decoded Minecraft region re-encodes");
    assert_eq!(encoded, data, "an unchanged Minecraft region changed");
});
