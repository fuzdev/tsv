//! Discover tsgo conformance baseline files (`*.errors.txt`).
//!
//! Mirrors `test262/discovery.rs`: a plain `std::fs` recursion, no extra
//! dependency. The baselines are the committed oracle; the corpus *inputs* live
//! in a separate (often unmaterialized) submodule that these queries don't need.

use std::fs;
use std::path::{Path, PathBuf};

/// The committed baselines live here, relative to a typescript-go checkout.
const BASELINES_SUBDIR: &str = "testdata/baselines/reference/submodule";

/// The corpus *input* files live here — a git submodule, often unmaterialized.
const CORPUS_SUBDIR: &str = "_submodules/TypeScript";

/// A discovered baseline file (`*.errors.txt`).
#[derive(Debug, Clone)]
pub struct Baseline {
    /// Path to the `.errors.txt` file on disk.
    pub path: PathBuf,
    /// Path relative to the baselines root, `/`-separated (e.g.
    /// `compiler/foo.errors.txt`). The stable identity used by every query.
    pub relative_path: String,
}

/// The baselines directory inside a typescript-go checkout.
pub fn baselines_dir(checkout: &Path) -> PathBuf {
    checkout.join(BASELINES_SUBDIR)
}

/// Whether the corpus *input* submodule is materialized (its known test tree is
/// populated).
///
/// The core queries never touch it, but the `index` / `run` legs (and precise JSX
/// detection) do — so callers note when it's absent and skip rather than pretend. A
/// bare submodule directory can hold only a `.git` gitlink or a stray file on a
/// partial checkout, so the probe reaches for a **known corpus directory**
/// (`tests/cases/compiler`) and requires real entries: a missing, empty, or
/// partially-checked-out submodule all read as not materialized.
pub fn corpus_materialized(checkout: &Path) -> bool {
    // `_submodules/TypeScript/tests/cases/compiler` — the largest corpus suite; a
    // materialized submodule always populates it, a partial one does not.
    let cases_compiler = checkout
        .join(CORPUS_SUBDIR)
        .join("tests")
        .join("cases")
        .join("compiler");
    fs::read_dir(&cases_compiler).is_ok_and(|mut it| it.next().is_some())
}

/// Walk the baselines directory and discover every `.errors.txt` file, sorted by
/// relative path. The sibling `.types` / `.symbols` baselines are ignored — only
/// files ending in `.errors.txt` are collected.
pub fn discover_baselines(baselines_dir: &Path) -> Result<Vec<Baseline>, String> {
    if !baselines_dir.exists() {
        return Err(format!(
            "baselines directory not found: {}",
            baselines_dir.display()
        ));
    }

    let mut out = Vec::new();
    discover_recursive(baselines_dir, baselines_dir, &mut out)?;
    out.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(out)
}

fn discover_recursive(dir: &Path, root: &Path, out: &mut Vec<Baseline>) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory {}: {e}", dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let path = entry.path();

        if path.is_dir() {
            discover_recursive(&path, root, out)?;
        } else if path.is_file() {
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.ends_with(".errors.txt") {
                continue;
            }

            let relative_path = path
                .strip_prefix(root)
                .map_or_else(
                    |_| path.to_string_lossy().into_owned(),
                    |p| p.to_string_lossy().into_owned(),
                )
                .replace('\\', "/");

            out.push(Baseline {
                path,
                relative_path,
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_materialized_requires_the_known_case_tree() {
        let root = std::env::temp_dir().join(format!("tsv_corpus_probe_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        // Absent submodule → not materialized.
        assert!(!corpus_materialized(&root));

        // A bare submodule dir with only a stray file (a partial checkout) → still not
        // materialized: the deep `tests/cases/compiler` tree is what counts, so a
        // gitlink-or-README-only checkout can't pass.
        let submodule = root.join(CORPUS_SUBDIR);
        fs::create_dir_all(&submodule).unwrap();
        fs::write(submodule.join("README.md"), "partial\n").unwrap();
        assert!(!corpus_materialized(&root));

        // An empty `tests/cases/compiler` dir → not materialized (no test entries).
        let cases = submodule.join("tests").join("cases").join("compiler");
        fs::create_dir_all(&cases).unwrap();
        assert!(!corpus_materialized(&root));

        // A populated corpus suite → materialized.
        fs::write(cases.join("someTest.ts"), "export {};\n").unwrap();
        assert!(corpus_materialized(&root));

        let _ = fs::remove_dir_all(&root);
    }
}
