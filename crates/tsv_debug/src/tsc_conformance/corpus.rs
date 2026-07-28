//! Index the tsc corpus inputs under `tests/cases/{compiler,conformance}`.
//!
//! The corpus lives in the (now-materialized) TypeScript submodule; a test's
//! identity is its path. Baselines are flat per suite, so the baseline↔test join
//! is by basename within a suite — basename collisions across nesting are counted
//! and reported, never silently merged.
//
// tsgo: internal/testrunner/compiler_runner.go NewCompilerBaselineRunner (basePath)
// tsgo: internal/vfs/internal/internal.go decodeBytes (BOM/UTF-16 handling)

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The corpus input tree, relative to a typescript-go checkout.
const CORPUS_SUBDIR: &str = "_submodules/TypeScript/tests/cases";

/// The two in-scope suites (`compiler` = regression, `conformance`), each a flat
/// baseline namespace.
pub const SUITES: [&str; 2] = ["compiler", "conformance"];

/// One discovered corpus test file.
#[derive(Debug, Clone)]
pub struct CorpusTest {
    /// Suite (`compiler` or `conformance`) — the baseline namespace.
    pub suite: &'static str,
    /// Path relative to the suite root, `/`-separated.
    pub relative_path: String,
    /// The file's basename (e.g. `foo.ts`) — the baseline-join key.
    pub basename: String,
    /// The lowercased extension without the dot (`ts` / `tsx` / `js`).
    pub extension: String,
    /// Absolute path on disk.
    pub path: PathBuf,
}

/// The corpus directory inside a typescript-go checkout.
pub fn corpus_dir(checkout: &Path) -> PathBuf {
    checkout.join(CORPUS_SUBDIR)
}

/// Read a corpus file, decoding a BOM exactly as tsgo's `decodeBytes` does:
/// UTF-16 LE/BE (`FF FE` / `FE FF`) are decoded and the BOM dropped; a UTF-8 BOM
/// (`EF BB BF`) is stripped; everything else is treated as UTF-8. Invalid
/// sequences become the replacement character (the harness never rejects on
/// encoding).
pub fn read_corpus_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    Ok(decode_bytes(&bytes))
}

/// Decode file bytes per tsgo's `decodeBytes`.
fn decode_bytes(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        return decode_utf16(&bytes[2..], false);
    }
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        return decode_utf16(&bytes[2..], true);
    }
    let body = if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        &bytes[3..]
    } else {
        bytes
    };
    String::from_utf8_lossy(body).into_owned()
}

/// Decode a UTF-16 byte stream (`big_endian` selects the order), an odd trailing
/// byte dropped as tsgo's `binary.Read` would leave it out of the `uint16` slice.
fn decode_utf16(bytes: &[u8], big_endian: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| {
            if big_endian {
                u16::from_be_bytes([c[0], c[1]])
            } else {
                u16::from_le_bytes([c[0], c[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

/// Walk both suites and index every `.ts` / `.tsx` / `.js` corpus file, sorted by
/// `(suite, relative_path)`. Safety-net directories are not present in the corpus,
/// so no pruning is needed.
pub fn discover_corpus(checkout: &Path) -> Result<Vec<CorpusTest>, String> {
    let base = corpus_dir(checkout);
    if !base.exists() {
        return Err(format!("corpus directory not found: {}", base.display()));
    }
    let mut out = Vec::new();
    for suite in SUITES {
        let root = base.join(suite);
        if root.exists() {
            walk(&root, &root, suite, &mut out)?;
        }
    }
    out.sort_by(|a, b| (a.suite, &a.relative_path).cmp(&(b.suite, &b.relative_path)));
    Ok(out)
}

fn walk(
    dir: &Path,
    root: &Path,
    suite: &'static str,
    out: &mut Vec<CorpusTest>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("read dir {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read entry: {e}"))?;
        let path = entry.path();
        if path.is_dir() {
            walk(&path, root, suite, out)?;
        } else if path.is_file() {
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let ext = match name.rsplit_once('.') {
                Some((_, e)) => e.to_ascii_lowercase(),
                None => continue,
            };
            if ext != "ts" && ext != "tsx" && ext != "js" {
                continue;
            }
            let relative_path = path
                .strip_prefix(root)
                .map_or_else(
                    |_| path.to_string_lossy().into_owned(),
                    |p| p.to_string_lossy().into_owned(),
                )
                .replace('\\', "/");
            out.push(CorpusTest {
                suite,
                relative_path,
                basename: name.to_string(),
                extension: ext,
                path,
            });
        }
    }
    Ok(())
}

/// A basename shared by more than one corpus test in the same suite — a join
/// ambiguity (the baseline join is by `(suite, basename)`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct BasenameCollision {
    /// The suite the collision is in.
    pub suite: String,
    /// The shared basename.
    pub basename: String,
    /// The colliding tests' relative paths.
    pub paths: Vec<String>,
}

/// Find every `(suite, basename)` shared by more than one corpus test.
pub fn basename_collisions(tests: &[CorpusTest]) -> Vec<BasenameCollision> {
    let mut by_key: BTreeMap<(&str, &str), Vec<String>> = BTreeMap::new();
    for t in tests {
        by_key
            .entry((t.suite, t.basename.as_str()))
            .or_default()
            .push(t.relative_path.clone());
    }
    by_key
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|((suite, basename), paths)| BasenameCollision {
            suite: suite.to_string(),
            basename: basename.to_string(),
            paths,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf8_bom() {
        let bytes = [0xEF, 0xBB, 0xBF, b'/', b'/', b'@'];
        assert_eq!(decode_bytes(&bytes), "//@");
    }

    #[test]
    fn decodes_utf16_le() {
        // "//" in UTF-16 LE with BOM.
        let bytes = [0xFF, 0xFE, b'/', 0x00, b'/', 0x00];
        assert_eq!(decode_bytes(&bytes), "//");
    }

    #[test]
    fn decodes_utf16_be() {
        let bytes = [0xFE, 0xFF, 0x00, b'/', 0x00, b'/'];
        assert_eq!(decode_bytes(&bytes), "//");
    }

    #[test]
    fn plain_utf8_passthrough() {
        assert_eq!(decode_bytes(b"// @target: es5"), "// @target: es5");
    }
}
