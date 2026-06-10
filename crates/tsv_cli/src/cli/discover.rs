use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Directory names skipped during recursive discovery, in addition to hidden
/// directories (leading `.`), which are skipped unconditionally.
const EXCLUDED_DIRS: [&str; 4] = ["node_modules", "dist", "build", "target"];

/// Result of expanding path arguments: the files to format plus any non-fatal
/// traversal errors (unreadable directories/entries). Both are sorted.
pub struct Discovered {
    pub files: Vec<PathBuf>,
    /// `path: detail` messages — reported by the caller, counted as errors.
    pub errors: Vec<String>,
}

/// Whether a path has a formattable extension (`.ts`, `.svelte`, `.css` —
/// compound forms like `.svelte.ts` are covered by the `.ts` match).
fn is_formattable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| matches!(ext, "ts" | "svelte" | "css"))
}

/// Expand files and directories into a sorted, deduplicated list of files to format.
///
/// All root arguments are validated upfront: any that don't resolve to a file or
/// directory fail the whole run before anything is formatted (`Err` carries one
/// message per bad argument). Traversal errors below a valid root are non-fatal —
/// collected in `Discovered::errors` while the rest of the tree continues.
///
/// Explicit file arguments are always included regardless of extension (the
/// caller trusts them); directories recurse with the extension filter, skipping
/// hidden directories (generated output like `.svelte-kit`, VCS dirs) and
/// `EXCLUDED_DIRS`. Symlinks inside directories are not followed (cycle
/// safety) — pass them explicitly to format their targets.
///
/// With multiple roots, overlapping spellings of the same file (`src` vs
/// `./src`, absolute vs relative, symlink aliases) are deduplicated by canonical
/// path, keeping the first spelling in sorted order. A single root can't produce
/// duplicates (symlinks aren't followed), so the canonicalization cost is skipped.
pub fn discover_files(paths: &[String]) -> Result<Discovered, Vec<String>> {
    let bad: Vec<String> = paths
        .iter()
        .filter(|p| {
            let path = Path::new(p.as_str());
            !path.is_file() && !path.is_dir()
        })
        .map(|p| format!("{p}: not a file or directory"))
        .collect();
    if !bad.is_empty() {
        return Err(bad);
    }

    let mut files = Vec::new();
    let mut errors = Vec::new();
    for path_str in paths {
        let path = PathBuf::from(path_str);
        if path.is_file() {
            files.push(path);
        } else {
            collect_recursive(&path, &mut files, &mut errors);
        }
    }
    files.sort();
    files.dedup();
    if paths.len() > 1 {
        let mut seen: HashSet<PathBuf> = HashSet::with_capacity(files.len());
        files.retain(|p| seen.insert(fs::canonicalize(p).unwrap_or_else(|_| p.clone())));
    }
    errors.sort();
    Ok(Discovered { files, errors })
}

fn collect_recursive(dir: &Path, files: &mut Vec<PathBuf>, errors: &mut Vec<String>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            errors.push(format!("{}: read_dir failed: {e}", dir.display()));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                errors.push(format!("{}: read_dir entry failed: {e}", dir.display()));
                continue;
            }
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(e) => {
                errors.push(format!("{}: file_type failed: {e}", entry.path().display()));
                continue;
            }
        };
        let path = entry.path();
        if file_type.is_dir() {
            let name = entry.file_name();
            let hidden = name.as_encoded_bytes().first() == Some(&b'.');
            if hidden || name.to_str().is_some_and(|n| EXCLUDED_DIRS.contains(&n)) {
                continue;
            }
            collect_recursive(&path, files, errors);
        } else if file_type.is_file() && is_formattable(&path) {
            files.push(path);
        }
    }
}
