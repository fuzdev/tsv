//! Lint-table parity: pin `tsv_ffi`'s and `tsv_napi`'s hand-mirrored lint
//! tables against the workspace's.
//!
//! Those two crates need `unsafe_code = "allow"` (the FFI / N-API boundary),
//! and Cargo replaces the **whole** `[lints]` table on override — there is no
//! partial inherit — so each re-declares every `[workspace.lints.*]` entry
//! verbatim. That copy silently drifts when a lint is added to the workspace
//! and not mirrored (it already had: `elided_lifetimes_in_paths` went missing).
//!
//! This test is the guard: it extracts the `[lints.rust]` / `[lints.clippy]`
//! tables from both crates and the `[workspace.lints.*]` tables from the root
//! manifest, then asserts they are identical — except `unsafe_code`, which the
//! two crates intentionally relax from `forbid` to `allow`. A drift fails here
//! with the offending lint named, so the mirrored tables can't rot unnoticed.
//!
//! Pure string parsing (section header → next `[`), no TOML dependency.

// Test harness: unwrap/expect/panic on setup failure is the desired behavior.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
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

#[test]
fn ffi_napi_lint_tables_match_workspace() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_toml = fs::read_to_string(root.join("Cargo.toml")).unwrap();

    let root_clippy = lint_table(&root_toml, "[workspace.lints.clippy]");
    let root_rust = lint_table(&root_toml, "[workspace.lints.rust]");
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

    for crate_name in ["tsv_ffi", "tsv_napi"] {
        let toml =
            fs::read_to_string(root.join("crates").join(crate_name).join("Cargo.toml")).unwrap();
        let clippy = lint_table(&toml, "[lints.clippy]");
        let mut rust = lint_table(&toml, "[lints.rust]");

        // Clippy table must mirror the workspace exactly.
        assert_eq!(
            clippy, root_clippy,
            "{crate_name}'s [lints.clippy] has drifted from [workspace.lints.clippy] — \
             re-sync it verbatim (Cargo replaces the whole table on override)"
        );

        // Rust table mirrors the workspace too, except `unsafe_code`: these
        // crates relax it from `forbid` to `allow` for the FFI / N-API boundary.
        assert_eq!(
            rust.remove("unsafe_code").as_deref(),
            Some("\"allow\""),
            "{crate_name} should set unsafe_code = \"allow\""
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
