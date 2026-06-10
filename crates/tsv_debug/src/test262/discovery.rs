//! Discover test262 test files.

use std::fs;
use std::path::{Path, PathBuf};

/// A discovered test262 test file.
#[derive(Debug, Clone)]
pub struct TestFile {
    /// Full path to the test file
    pub path: PathBuf,
    /// Relative path from test262 root (e.g., "test/language/expressions/array/11.1.4-0.js")
    pub relative_path: String,
}

impl TestFile {
    /// Check if this test file matches any of the given filter terms.
    pub fn matches_filters(&self, filters: &[String]) -> bool {
        if filters.is_empty() {
            return true;
        }
        let lower_path = self.relative_path.to_lowercase();
        filters
            .iter()
            .any(|filter| lower_path.contains(&filter.to_lowercase()))
    }
}

/// Options for test discovery.
#[derive(Debug, Default)]
pub struct DiscoveryOptions {
    /// Only discover tests in these subdirectories (relative to test262/test/)
    pub subdirs: Vec<String>,
}

/// Walk the test262 directory and discover all test files.
///
/// Excludes:
/// - `*_FIXTURE.js` files (module dependencies, not standalone tests)
/// - `test/staging/` (in-progress proposals)
/// - `test/harness/` (harness infrastructure tests)
pub fn discover_tests(
    test262_root: &Path,
    options: &DiscoveryOptions,
) -> Result<Vec<TestFile>, String> {
    let test_dir = test262_root.join("test");

    if !test_dir.exists() {
        return Err(format!(
            "test262 test directory not found: {}\n\
			Make sure the test262 repository is cloned at the expected location.",
            test_dir.display()
        ));
    }

    let mut tests = Vec::new();

    // Determine which directories to scan
    let scan_dirs: Vec<PathBuf> = if options.subdirs.is_empty() {
        // Scan all standard test directories (exclude staging and harness)
        let mut dirs = Vec::new();
        if let Ok(entries) = fs::read_dir(&test_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    // Skip staging (in-progress proposals) and harness (infrastructure tests)
                    if name != "staging" && name != "harness" {
                        dirs.push(path);
                    }
                }
            }
        }
        dirs
    } else {
        // Scan only specified subdirectories
        options
            .subdirs
            .iter()
            .map(|subdir| test_dir.join(subdir))
            .filter(|path| path.exists())
            .collect()
    };

    for dir in scan_dirs {
        discover_tests_recursive(&dir, test262_root, &mut tests)?;
    }

    // Sort by path for consistent ordering
    tests.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    Ok(tests)
}

fn discover_tests_recursive(
    dir: &Path,
    root: &Path,
    tests: &mut Vec<TestFile>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory {}: {e}", dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let path = entry.path();

        if path.is_dir() {
            discover_tests_recursive(&path, root, tests)?;
        } else if path.is_file() {
            let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };

            // Only .js files
            if !filename.ends_with(".js") {
                continue;
            }

            // Skip fixture files (module dependencies)
            if filename.ends_with("_FIXTURE.js") {
                continue;
            }

            // Compute relative path from test262 root
            let relative_path = path.strip_prefix(root).map_or_else(
                |_| path.to_string_lossy().to_string(),
                |p| p.to_string_lossy().to_string(),
            );

            tests.push(TestFile {
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
    fn test_matches_filters_empty() {
        let test = TestFile {
            path: PathBuf::from("/test262/test/language/expressions/array/test.js"),
            relative_path: "test/language/expressions/array/test.js".to_string(),
        };
        assert!(test.matches_filters(&[]));
    }

    #[test]
    fn test_matches_filters_match() {
        let test = TestFile {
            path: PathBuf::from("/test262/test/language/expressions/array/test.js"),
            relative_path: "test/language/expressions/array/test.js".to_string(),
        };
        assert!(test.matches_filters(&["expressions".to_string()]));
        assert!(test.matches_filters(&["EXPRESSIONS".to_string()])); // case insensitive
        assert!(test.matches_filters(&["array".to_string()]));
    }

    #[test]
    fn test_matches_filters_no_match() {
        let test = TestFile {
            path: PathBuf::from("/test262/test/language/expressions/array/test.js"),
            relative_path: "test/language/expressions/array/test.js".to_string(),
        };
        assert!(!test.matches_filters(&["built-ins".to_string()]));
    }
}
