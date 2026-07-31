// Build scripts should panic on failure - that's how they signal build errors.
// Using expect/unwrap is appropriate here.
#![allow(clippy::expect_used, clippy::unwrap_used)]

//! Build script to generate HTML entity map from entities.json
//!
//! This script parses entities.json — the WHATWG named character references list, one
//! entry per name holding the characters it stands for — and generates a compile-time
//! perfect hash map (phf::Map) for O(1) entity lookup.
//!
//! The generated map contains 2,231 named entities; 93 of them stand for two code
//! points (a combining mark, a variation selector, or a second character), which is why
//! the value is a string rather than a `char`. Numeric entities (&#65;, &#x41;) are NOT
//! in the map - they're decoded algorithmically.

use std::collections::BTreeMap;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::Path;

fn main() {
    // Tell cargo to rerun if entities.json changes
    println!("cargo:rerun-if-changed=src/entities.json");

    // Read and parse entities.json (format: { "entity": "characters" })
    let entities_json =
        fs::read_to_string("src/entities.json").expect("Failed to read src/entities.json");

    let entities: BTreeMap<String, String> =
        serde_json::from_str(&entities_json).expect("Failed to parse entities.json");

    // No cargo:warning here — it would print on every build of every consumer.
    // A broken/truncated entities.json should fail instead (full list is ~2231).
    assert!(
        entities.len() > 2000,
        "entity map suspiciously small: {} entries",
        entities.len()
    );
    assert!(
        entities.values().all(|characters| !characters.is_empty()),
        "an entity naming no characters cannot be decoded"
    );

    // Generate Rust code for the entity map
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("entities_map.rs");
    let mut f = fs::File::create(&dest_path).unwrap();

    writeln!(f, "// Auto-generated from entities.json").unwrap();
    writeln!(f, "// Source: https://html.spec.whatwg.org/entities.json").unwrap();
    writeln!(f, "// Total entities: {}", entities.len()).unwrap();
    writeln!(f).unwrap();
    writeln!(
        f,
        "static ENTITIES: phf::Map<&'static str, &'static str> = phf_map! {{"
    )
    .unwrap();

    let mut escaped = String::new();
    for (entity_name, characters) in &entities {
        // Entity names already have '&' stripped in the JSON file, and are ASCII. The
        // characters are escaped: one can be a combining mark or a variation selector,
        // which would be invisible in the generated source.
        escaped.clear();
        for c in characters.chars() {
            push_escaped(&mut escaped, c);
        }
        writeln!(f, "    \"{entity_name}\" => \"{escaped}\",").unwrap();
    }

    writeln!(f, "}};").unwrap();
}

/// Append one character to a Rust string literal body, escaped so the generated map
/// stays pure ASCII.
fn push_escaped(out: &mut String, c: char) {
    if c.is_ascii_graphic() && c != '"' && c != '\\' {
        out.push(c);
    } else {
        write!(out, "\\u{{{:x}}}", c as u32).unwrap();
    }
}
