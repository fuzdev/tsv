//! Discovery parity: drive the shared `tests/discovery/scenarios.json` table
//! through the native walker (`tsv_cli::cli::discover::discover_files`) and
//! assert each case's in-scope set, exact and ordered.
//!
//! The SAME table is run against the WASM CLI (`cli.js`) by
//! `scripts/test_npm.ts`, so the two discovery walkers can't silently drift:
//! a divergence fails one side or the other. The matcher itself is pinned
//! against real `git check-ignore` by `tsv_ignore`'s `git_oracle`; this suite
//! pins the *walk* — repo-root detection, hierarchical layering, the
//! build-output heuristic, explicit-arg bypass, and the `.git` boundary —
//! which has no external oracle. `expected` is hand-authored (not generated
//! from this impl), so it pins correctness for both surfaces, not mere
//! agreement.
//!
//! Each scenario materializes its `tree` in a fresh tempdir (string = file,
//! null = empty dir; a `.git` entry makes a dir look like a repo root without
//! a real git binary), then for each case calls `discover_files` on
//! `<root>/<target>` and compares the discovered files — relative to the
//! tempdir root, `/`-joined — to `expected`.

// Test harness: unwrap/expect/panic on setup failure is the desired behavior.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use tsv_cli::cli::discover::discover_files;

/// A unique temp dir path (no temp-dir dependency), mirroring the git_oracle harness.
fn fresh_dir(tag: &str) -> PathBuf {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("tsv_discovery_{tag}_{}_{n}", std::process::id()))
}

/// Materialize a scenario `tree`: string value = file (parents created), null =
/// empty directory.
fn materialize(root: &Path, tree: &serde_json::Map<String, Value>) {
    for (rel, value) in tree {
        let path = root.join(rel);
        match value {
            Value::Null => {
                fs::create_dir_all(&path).unwrap();
            }
            Value::String(contents) => {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, contents).unwrap();
            }
            other => panic!("tree value for {rel:?} must be a string or null, got {other}"),
        }
    }
}

/// Run one case: discover under `<root>/<target>` and return the in-scope files
/// as root-relative, `/`-joined strings in discovery (sorted) order.
fn discover_case(root: &Path, target: &str) -> Vec<String> {
    let arg = if target.is_empty() {
        root.to_path_buf()
    } else {
        root.join(target)
    };
    let discovered = discover_files(&[arg.to_string_lossy().into_owned()])
        .expect("scenario args always resolve to a real path");
    let prefix = format!("{}/", root.to_string_lossy());
    discovered
        .files
        .iter()
        .map(|p| {
            let s = p.to_string_lossy();
            s.strip_prefix(&prefix).unwrap_or(&s).replace('\\', "/")
        })
        .collect()
}

fn expected_list(case: &Value) -> Vec<String> {
    case["expected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn discovery_matches_shared_scenarios() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/discovery/scenarios.json"
    );
    let table: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let scenarios = table["scenarios"].as_array().unwrap();

    let mut failures = Vec::new();
    for scenario in scenarios {
        let name = scenario["name"].as_str().unwrap();
        let tree = scenario["tree"].as_object().unwrap();
        let root = fresh_dir(name);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        materialize(&root, tree);

        for case in scenario["cases"].as_array().unwrap() {
            let target = case["target"].as_str().unwrap();
            let expected = expected_list(case);
            let actual = discover_case(&root, target);
            if actual != expected {
                failures.push(format!(
                    "[{name}] target={target:?}\n     expected: {expected:?}\n     actual:   {actual:?}"
                ));
            }
        }
        let _ = fs::remove_dir_all(&root);
    }

    assert!(
        failures.is_empty(),
        "{} discovery-parity mismatch(es):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
