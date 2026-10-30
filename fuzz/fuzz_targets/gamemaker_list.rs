#![no_main]

use libfuzzer_sys::fuzz_target;
use saveprism_codecs::register_all;
use saveprism_core::{pipeline, Limits, Recipe, Registry, Step};

fuzz_target!(|data: &[u8]| {
    let mut registry = Registry::default();
    register_all(&mut registry);
    let limits = Limits {
        max_input_bytes: 2 * 1024 * 1024,
        max_decoded_bytes: 4 * 1024 * 1024,
        ..Limits::default()
    };
    let recipe = Recipe::new(vec![Step::bare("format.gamemaker-ds-list")]);
    if let Ok(decoded) = pipeline::decode(data, &recipe, &registry, &limits) {
        let encoded = pipeline::encode(&decoded.doc, &decoded.recipe, &registry)
            .expect("decoded GameMaker ds_list re-encodes");
        let again = pipeline::decode(&encoded, &decoded.recipe, &registry, &limits)
            .expect("encoded GameMaker ds_list decodes");
        assert!(decoded.doc.tree_eq(&again.doc));
    }
});
