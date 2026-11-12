//! Lint-table parity: pin `tsv_ffi`'s and `tsv_napi`'s hand-mirrored lint
//! tables against the workspace's.
//!
//! Those two crates can't inherit the workspace's `unsafe_code = "forbid"` (the
//! FFI / N-API boundary), and Cargo replaces the **whole** `[lints]` table on
//! override — there is no partial inherit — so each re-declares every
//! `[workspace.lints.*]` entry verbatim. A hand-kept copy silently drifts when a
//! lint is added to the workspace and not mirrored.
//!
//! This test is the guard: it extracts the `[lints.rust]` / `[lints.rustdoc]` /
//! `[lints.clippy]` tables from both crates and the `[workspace.lints.*]` tables
//! from the root manifest, then asserts they are identical — except
//! `unsafe_code`, pinned to `deny` in both, and `tsv_ffi`'s added
//! `clippy::undocumented_unsafe_blocks` (the one crate hand-writing `unsafe`, so
//! the one whose every block must say why it is sound). It also asserts each
//! crate re-declares exactly the workspace's set of lint tools (a
//! `[workspace.lints.<tool>]` added later, or one a crate alone carries, fails),
//! and that neither crate's `lib.rs` carries an inner `unsafe_code` attribute,
//! which would lift the lint crate-wide and undo the per-item enumeration. A
//! drift fails here with the offending lint named, so the mirrored tables can't
//! rot unnoticed.
//!
//! Pure string parsing (section header → next `[`), no TOML dependency.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// Extract a lint table (`name = value` lines under `header`) into a map.
///
/// A section runs from its exact header line to the next `[…]` line. Trailing
/// `# comments` and blank lines are dropped, and each value's internal
/// whitespace is normalized so verbatim copies compare equal regardless of
/// spacing. `name` is everything before the first `=`; lint values never
/// contain `#` or are split by the first `#`.
fn lint_table(contents: &str, header: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut in_section = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == header;
            continue;
        }
        if !in_section {
            continue;
        }
        let code = trimmed.split('#').next().unwrap_or("").trim();
        if let Some((name, value)) = code.split_once('=') {
            let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
            map.insert(name.trim().to_string(), value);
        }
    }
    map
}

/// The lint tools a manifest declares tables for: each `[<prefix>.<tool>]`
/// header line's `<tool>`.
fn lint_tools(contents: &str, prefix: &str) -> BTreeSet<String> {
    contents
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix('[')?
                .strip_suffix(']')?
                .strip_prefix(prefix)?
                .strip_prefix('.')
                .map(str::to_string)
        })
        .collect()
}

/// Every inner attribute (`#![…]`) in `source` that names `unsafe_code`, read
/// from the `#![` to the first `)]` after it so a multi-line attribute counts.
/// Comment lines are dropped first, so prose quoting one does not.
fn inner_unsafe_code_attrs(source: &str) -> Vec<String> {
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    code.match_indices("#![")
        .filter_map(|(start, _)| {
            let rest = &code[start..];
            let attr = &rest[..rest.find(")]").map_or(rest.len(), |end| end + 2)];
            attr.contains("unsafe_code").then(|| attr.to_string())
        })
        .collect()
}

#[test]
fn ffi_napi_lint_tables_match_workspace() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_toml = fs::read_to_string(root.join("Cargo.toml")).unwrap();

    let root_clippy = lint_table(&root_toml, "[workspace.lints.clippy]");
    let root_rust = lint_table(&root_toml, "[workspace.lints.rust]");
    let root_rustdoc = lint_table(&root_toml, "[workspace.lints.rustdoc]");
    assert!(
        root_clippy.len() > 10,
        "did not find [workspace.lints.clippy] (got {} entries) — test extraction is broken",
        root_clippy.len()
    );
    assert_eq!(
        root_rust.get("unsafe_code").map(String::as_str),
        Some("\"forbid\""),
        "workspace [lints.rust] should forbid unsafe_code"
    );

    assert!(
        !root_rustdoc.is_empty(),
        "did not find [workspace.lints.rustdoc] — test extraction is broken"
    );
    let root_tools = lint_tools(&root_toml, "workspace.lints");

    // Pinned to the *exact* level each crate relaxes to, not merely "weaker
    // than forbid"; each crate's own manifest comment carries the reasoning.
    // `extra_clippy` is the lint a crate adds over the workspace's table.
    for (crate_name, expected_unsafe_code, extra_clippy) in [
        (
            "tsv_ffi",
            "\"deny\"",
            Some(("undocumented_unsafe_blocks", "\"warn\"")),
        ),
        ("tsv_napi", "\"deny\"", None),
    ] {
        let toml =
            fs::read_to_string(root.join("crates").join(crate_name).join("Cargo.toml")).unwrap();
        let clippy = lint_table(&toml, "[lints.clippy]");
        let rustdoc = lint_table(&toml, "[lints.rustdoc]");
        let mut rust = lint_table(&toml, "[lints.rust]");

        // The same lint tools on both sides: a whole table one side lacks is
        // drift the per-table comparisons below can't see.
        assert_eq!(
            lint_tools(&toml, "lints"),
            root_tools,
            "{crate_name}'s [lints.*] tables name different tools than \
             [workspace.lints.*] — mirror the whole table"
        );

        // No crate-wide lift: each `unsafe` site carries its own `#[expect]`
        // (tsv_ffi) or comes from macro output (tsv_napi).
        let lib =
            fs::read_to_string(root.join("crates").join(crate_name).join("src/lib.rs")).unwrap();
        assert_eq!(
            inner_unsafe_code_attrs(&lib),
            Vec::<String>::new(),
            "{crate_name}'s lib.rs lifts unsafe_code crate-wide — lift it per item instead"
        );

        // Clippy table must mirror the workspace exactly, plus the crate's addition.
        let mut expected_clippy = root_clippy.clone();
        if let Some((lint, level)) = extra_clippy {
            assert!(
                expected_clippy
                    .insert(lint.to_string(), level.to_string())
                    .is_none(),
                "{crate_name}'s added clippy::{lint} is already in the workspace table"
            );
        }
        assert_eq!(
            clippy, expected_clippy,
            "{crate_name}'s [lints.clippy] has drifted from [workspace.lints.clippy] — \
             re-sync it verbatim (Cargo replaces the whole table on override)"
        );

        assert_eq!(
            rustdoc, root_rustdoc,
            "{crate_name}'s [lints.rustdoc] has drifted from [workspace.lints.rustdoc] — \
             re-sync it verbatim"
        );

        // Rust table mirrors the workspace too, except `unsafe_code`, which
        // each crate relaxes from `forbid` to its own pinned level.
        assert_eq!(
            rust.remove("unsafe_code").as_deref(),
            Some(expected_unsafe_code),
            "{crate_name} should set unsafe_code = {expected_unsafe_code}"
        );
        let mut root_rust_sans_unsafe = root_rust.clone();
        root_rust_sans_unsafe.remove("unsafe_code");
        assert_eq!(
            rust, root_rust_sans_unsafe,
            "{crate_name}'s [lints.rust] has drifted from [workspace.lints.rust] \
             (ignoring the intentional unsafe_code relaxation) — re-sync it"
        );
    }
}
