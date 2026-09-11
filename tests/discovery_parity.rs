//! Discovery parity: drive the shared `tests/discovery/scenarios.json` table
//! through the native walker (`tsv_cli::cli::discover::discover_files`) and
//! assert each case's in-scope set, exact and ordered.
//!
//! The SAME table is run against the WASM CLI (`cli.js`) by
//! `scripts/test_npm.ts`, so the two discovery walkers can't silently drift:
//! a divergence fails one side or the other. The matcher itself is pinned
//! against real `git check-ignore` by `tsv_ignore`'s `git_oracle`; this suite
//! pins the *walk* — repo-root detection, hierarchical layering, the
//! build-output heuristic, explicit-arg scope, and the `.git` boundary —
//! which has no external oracle. `expected` is hand-authored (not generated
//! from this impl), so it pins correctness for both surfaces, not mere
//! agreement.
//!
//! Each scenario materializes its `tree` in a fresh tempdir (string = file,
//! null = empty dir, `{"symlink": target}` = a symlink, unix only; a `.git` entry
//! makes a dir look like a repo root without a real git binary), then for each
//! case calls `discover_files` on
//! `<root>/<target>`. A case carries **either** `expected` — the discovered
//! files, relative to the tempdir root and `/`-joined — **or** `error`, a
//! substring of the argument error that must fail the run upfront with nothing
//! discovered.

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
/// empty directory, `{"symlink": target}` = a symbolic link to `target`, resolved
/// from the link's own directory (a scenario holding one is skipped off unix — see
/// [`holds_symlinks`]).
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
            Value::Object(link) => {
                let target = link
                    .get("symlink")
                    .and_then(Value::as_str)
                    .unwrap_or_else(|| {
                        panic!("tree object for {rel:?} must be {{\"symlink\": target}}")
                    });
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                symlink(target, &path);
            }
            other => {
                panic!("tree value for {rel:?} must be a string, null or a symlink, got {other}")
            }
        }
    }
}

/// Whether a scenario's tree holds a symlink — which needs unix, so the scenario is
/// skipped elsewhere.
fn holds_symlinks(tree: &serde_json::Map<String, Value>) -> bool {
    tree.values().any(Value::is_object)
}

#[cfg(unix)]
fn symlink(target: &str, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(not(unix))]
fn symlink(_target: &str, link: &Path) {
    panic!("{link:?}: a scenario holding a symlink is skipped off unix");
}

/// Run one case: discover under `<root>/<target>`. `Ok` carries the in-scope
/// files as root-relative, `/`-joined strings in discovery (sorted) order; `Err`
/// carries the **argument** errors that failed the run upfront (an unresolvable
/// path, or a named file whose extension tsv doesn't format), which an `error`
/// case asserts against.
fn discover_case(root: &Path, target: &str) -> Result<(Vec<String>, Vec<String>), Vec<String>> {
    let arg = if target.is_empty() {
        root.to_path_buf()
    } else {
        root.join(target)
    };
    let discovered = discover_files(&[arg.to_string_lossy().into_owned()])?;
    // Normalize separators on BOTH sides before stripping: discovery emits
    // native separators, so on Windows the root prefix ends in `\` where this
    // pattern expects `/` — stripping the un-normalized string never matches
    // and every case reads back absolute.
    let prefix = format!("{}/", root.to_string_lossy().replace('\\', "/"));
    let files = discovered
        .files
        .iter()
        .map(|p| {
            let s = p.to_string_lossy().replace('\\', "/");
            s.strip_prefix(&prefix).unwrap_or(&s).to_string()
        })
        .collect();
    Ok((files, discovered.diagnostics.warnings))
}

/// The substrings a case lists under `key` (`warns` / `no_warns`) — none when absent.
fn needles<'a>(case: &'a Value, key: &str) -> impl Iterator<Item = &'a str> {
    case.get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|needle| needle.as_str().unwrap())
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
        if cfg!(not(unix)) && holds_symlinks(tree) {
            eprintln!("discovery parity [{name}]: holds a symlink, which needs unix — skipped");
            continue;
        }
        let root = fresh_dir(name);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        materialize(&root, tree);

        for case in scenario["cases"].as_array().unwrap() {
            let target = case["target"].as_str().unwrap();
            let actual = discover_case(&root, target);
            // A case carries either `expected` (the in-scope set) or `error` (a
            // substring of the argument error that must fail the run upfront).
            match (case.get("error").and_then(Value::as_str), actual) {
                (Some(needle), Err(errors)) => {
                    if !errors.iter().any(|e| e.contains(needle)) {
                        failures.push(format!(
                            "[{name}] target={target:?}\n     expected error containing: {needle:?}\n     actual errors:            {errors:?}"
                        ));
                    }
                }
                (Some(needle), Ok((files, _))) => failures.push(format!(
                    "[{name}] target={target:?}\n     expected error containing: {needle:?}\n     actual: discovered {files:?}"
                )),
                (None, Err(errors)) => failures.push(format!(
                    "[{name}] target={target:?}\n     expected: {:?}\n     actual: run failed with {errors:?}",
                    expected_list(case)
                )),
                (None, Ok((files, warnings))) => {
                    let expected = expected_list(case);
                    if files != expected {
                        failures.push(format!(
                            "[{name}] target={target:?}\n     expected: {expected:?}\n     actual:   {files:?}"
                        ));
                    }
                    // `warns`: substrings each of which some warning must carry — the
                    // only pin on the WARNINGS channel as a walk product (the texts
                    // themselves are `tsv_discover`'s unit tests')
                    for needle in needles(case, "warns") {
                        if !warnings.iter().any(|w| w.contains(needle)) {
                            failures.push(format!(
                                "[{name}] target={target:?}\n     expected a warning containing: {needle:?}\n     actual warnings:                {warnings:?}"
                            ));
                        }
                    }
                    // `no_warns`: substrings no warning may carry
                    for needle in needles(case, "no_warns") {
                        if let Some(warning) = warnings.iter().find(|w| w.contains(needle)) {
                            failures.push(format!(
                                "[{name}] target={target:?}\n     expected no warning containing: {needle:?}\n     actual warning:                    {warning:?}"
                            ));
                        }
                    }
                }
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
