//! Fuzz target for the GTA III codec.
//!
//! The fuzz bytes are XORed over the invented file, starting from a valid save with the canonical
//! block layout. An empty input is the fixture itself, so the parser is always reachable.

#![no_main]

use libfuzzer_sys::fuzz_target;
use saveprism_codecs::register_all;
use saveprism_core::pipeline;
use saveprism_core::{Limits, Recipe, Registry, Step};

const FIXTURE: &[u8] = include_bytes!("../../fixtures/synthetic/gta-iii/invented.save");

fuzz_target!(|data: &[u8]| {
    let mut reg = Registry::default();
    register_all(&mut reg);
    let limits = Limits::default();
    let mut input = FIXTURE.to_vec();
    for (index, byte) in data.iter().take(65_536).enumerate() {
        let at = index % input.len();
        input[at] ^= byte;
    }
    let recipe = Recipe::new(vec![Step::bare("format.gta-iii")]);
    let Ok(out) = pipeline::decode(&input, &recipe, &reg, &limits) else {
        return;
    };
    let bytes = pipeline::encode(&out.doc, &out.recipe, &reg).expect("a decoded document encodes");
    assert_eq!(bytes, input, "an unchanged document writes the input back");
});
