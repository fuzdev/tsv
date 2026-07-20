//! The refusal catalog's completeness guard: every [`Refusal`] variant must have
//! a representative in [`Refusal::every_variant`].
//!
//! `every_variant` is hand-maintained and, unlike
//! [`bucket_key`](Refusal::bucket_key), is not an exhaustive `match` — a new
//! variant compiles fine while missing from it, and the declared bucket universe
//! ([`Refusal::all_bucket_keys`]) then silently narrows.
//!
//! `tsv_debug`'s `all_bucket_keys_covers_the_catalog` pins that universe as a
//! full key SET, which catches a rename, a deletion, a wrong placeholder, and two
//! representatives collapsing onto one key. It cannot catch an **omission**: the
//! pin sits downstream of `every_variant`, so a variant absent from both changes
//! no key and passes. This closes that one hole from the other side, deriving the
//! variant list from the enum's own source rather than from a second hand-written
//! mirror.
//!
//! ⚠️ The derivation is a textual scan of `refusal.rs`, not a parse — it is sound
//! only while the enum body keeps one variant per line at four-space indent, which
//! is what `cargo fmt` produces. A scan that stopped finding variants would fail
//! loudly here (the derived set would shrink against `every_variant`), never
//! silently pass.

use crate::Refusal;

/// Every variant name declared by the `Refusal` enum, read from its source.
///
/// Scans the `pub enum Refusal {` body for lines that open at the enum's own
/// indent with an identifier — a variant. Field lines sit one level deeper, and
/// doc comments, attributes, section rules, and a struct variant's closing brace
/// all fail the leading-uppercase test.
fn variant_names_from_source() -> Vec<String> {
    const SOURCE: &str = include_str!("../refusal.rs");

    SOURCE
        .lines()
        .skip_while(|line| !line.starts_with("pub enum Refusal {"))
        .skip(1)
        .take_while(|line| *line != "}")
        .filter_map(|line| {
            let name = line.strip_prefix("    ")?;
            if !name.starts_with(|c: char| c.is_ascii_uppercase()) {
                return None;
            }
            Some(
                name.chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect(),
            )
        })
        .collect()
}

/// The variant name of a representative, via its `Debug` derive.
///
/// `Debug` prints the variant name first for every shape — a unit variant is the
/// bare name, a struct variant is `Name { … }`, a tuple variant `Name(…)`.
fn variant_name_of(refusal: &Refusal) -> String {
    format!("{refusal:?}")
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}

#[test]
fn every_variant_covers_the_enum() {
    let declared = variant_names_from_source();
    assert!(
        declared.len() > 100,
        "the source scan found only {} variant(s) — the enum's layout changed and \
         the scan no longer reads it",
        declared.len()
    );

    let represented: Vec<String> = Refusal::every_variant()
        .iter()
        .map(variant_name_of)
        .collect();

    let missing: Vec<&String> = declared
        .iter()
        .filter(|name| !represented.contains(name))
        .collect();
    assert!(
        missing.is_empty(),
        "`Refusal` variant(s) with no `every_variant` representative: {missing:#?}\n\
         Add one per variant (parameters spelled as the placeholder the bucket key \
         collapses to, e.g. `name: \"{{name}}\".to_string()`), then regenerate \
         `EXPECTED_BUCKET_KEYS` in tsv_debug's compile_conformance_audit."
    );

    // The reverse direction is a scan failure rather than a catalog one: a
    // represented name the source has no variant for means the scan misread the
    // enum, since a nonexistent variant could not have been constructed.
    let unknown: Vec<&String> = represented
        .iter()
        .filter(|name| !declared.contains(name))
        .collect();
    assert!(
        unknown.is_empty(),
        "the source scan missed variant(s) `every_variant` constructs: {unknown:#?}"
    );
}
