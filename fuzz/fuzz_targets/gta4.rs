//! Fuzz target for the GTA IV codec.
//!
//! A fixed metadata and `SimpleVars` prefix puts the parser at block 1, so the fuzzer spends its
//! time on block sizes, the remaining blocks and the end block instead of rediscovering `SAVE`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use saveprism_codecs::register_all;
use saveprism_core::pipeline;
use saveprism_core::{Limits, Recipe, Registry, Step};

const FIXTURE: &[u8] = include_bytes!("../../fixtures/synthetic/gta-iv/invented.save");

fuzz_target!(|data: &[u8]| {
    let mut reg = Registry::default();
    register_all(&mut reg);
    let limits = Limits::default();
    let prefix = &FIXTURE[..0x1C9];
    let mut input = Vec::with_capacity(prefix.len() + data.len());
    input.extend_from_slice(prefix);
    input.extend_from_slice(data);
    let recipe = Recipe::new(vec![Step::bare("format.gta-iv")]);
    let Ok(out) = pipeline::decode(&input, &recipe, &reg, &limits) else {
        return;
    };
    let bytes = pipeline::encode(&out.doc, &out.recipe, &reg).expect("a decoded document encodes");
    assert_eq!(bytes, input, "an unchanged document writes the input back");
});
