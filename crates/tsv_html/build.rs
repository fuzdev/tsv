// Build scripts should panic on failure - that's how they signal build errors.
// Using expect/unwrap is appropriate here.
#![allow(clippy::expect_used, clippy::unwrap_used)]

//! Build script to generate HTML entity map from entities.json
//!
//! This script parses our simplified entities.json file (derived from the canonical
//! HTML5 spec data) and generates a compile-time perfect hash map (phf::Map) for O(1)
//! entity lookup.
//!
//! Our simplified format matches Svelte's implementation: each entity maps to a single
//! Unicode codepoint (the first codepoint from the spec). Multi-codepoint entities with
//! combining marks are simplified to just the base character.
//!
//! The generated map contains 2,231 named entities with their Unicode codepoints.
//! Numeric entities (&#65;, &#x41;) are NOT in the map - they're decoded algorithmically.
//!
//! See: scripts/generate_simplified_entities.ts for the simplification process

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

fn main() {
    // Tell cargo to rerun if entities.json changes
    println!("cargo:rerun-if-changed=src/entities.json");

    // Read and parse entities.json (simplified format: { "entity": codepoint })
    let entities_json =
        fs::read_to_string("src/entities.json").expect("Failed to read src/entities.json");

    let entities: BTreeMap<String, u32> =
        serde_json::from_str(&entities_json).expect("Failed to parse entities.json");

    // Generate Rust code for the entity map
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("entities_map.rs");
    let mut f = fs::File::create(&dest_path).unwrap();

    writeln!(f, "// Auto-generated from entities.json").unwrap();
    writeln!(f, "// Simplified format matching Svelte's implementation").unwrap();
    writeln!(
        f,
        "// Source: https://html.spec.whatwg.org/entities.json (first codepoint only)"
    )
    .unwrap();
    writeln!(f, "// Total entities: {}", entities.len()).unwrap();
    writeln!(f).unwrap();
    writeln!(
        f,
        "static ENTITIES: phf::Map<&'static str, u32> = phf_map! {{"
    )
    .unwrap();

    for (entity_name, codepoint) in &entities {
        // Entity names already have '&' stripped in the JSON file
        writeln!(f, "    \"{entity_name}\" => {codepoint},").unwrap();
    }

    writeln!(f, "}};").unwrap();

    // No cargo:warning here — it would print on every build of every consumer.
    // A broken/truncated entities.json should fail instead (full list is ~2231).
    assert!(
        entities.len() > 2000,
        "entity map suspiciously small: {} entries",
        entities.len()
    );
}
