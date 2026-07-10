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

/// Whether the corpus *input* submodule is materialized (has any entries).
///
/// The core queries never touch it, but precise JSX detection and the (deferred)
/// pin-diff query would — so callers note when it's empty and skip rather than
/// pretend. A missing or empty directory both read as not materialized.
pub fn corpus_materialized(checkout: &Path) -> bool {
    let dir = checkout.join(CORPUS_SUBDIR);
    fs::read_dir(&dir).is_ok_and(|mut it| it.next().is_some())
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
