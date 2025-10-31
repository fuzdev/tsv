/// Helpers for managing test fixtures
use std::fs;
use std::path::{Path, PathBuf};

/// A test fixture with its input file
#[derive(Debug, Clone)]
pub struct Fixture {
    /// Full path to the fixture directory
    pub path: PathBuf,
    /// Relative path from fixtures root (e.g., "svelte/elements/block_text")
    pub relative_path: String,
    /// Input filename (e.g., "input.svelte", "input.ts", "input.css")
    pub input_file: String,
}

impl Fixture {
    /// Get the full path to the input file
    pub fn input_path(&self) -> PathBuf {
        self.path.join(&self.input_file)
    }

    /// Get the full path to expected.json
    pub fn expected_path(&self) -> PathBuf {
        self.path.join("expected.json")
    }

    /// Get the full path to formatted file
    pub fn formatted_path(&self) -> PathBuf {
        let extension = Path::new(&self.input_file)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        self.path.join(format!("formatted.{}", extension))
    }

    /// Determine the file type from the input filename
    pub fn file_type(&self) -> FileType {
        if self.input_file.ends_with(".svelte") {
            FileType::Svelte
        } else if self.input_file.ends_with(".ts") {
            FileType::TypeScript
        } else if self.input_file.ends_with(".css") {
            FileType::Css
        } else {
            FileType::Unknown
        }
    }

    /// Check if this fixture matches all the given filter terms
    pub fn matches_filters(&self, filters: &[String]) -> bool {
        if filters.is_empty() {
            return true;
        }
        let lower_path = self.relative_path.to_lowercase();
        filters.iter().all(|filter| lower_path.contains(&filter.to_lowercase()))
    }
}

/// File type for a fixture
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Svelte,
    TypeScript,
    Css,
    Unknown,
}

/// Walk the fixtures directory and collect all fixtures
///
/// # Arguments
/// * `fixtures_dir` - Path to the fixtures directory (e.g., "tests/fixtures")
///
/// # Returns
/// A vector of all discovered fixtures
pub fn walk_fixtures(fixtures_dir: &Path) -> Result<Vec<Fixture>, String> {
    let mut fixtures = Vec::new();
    walk_fixtures_recursive(fixtures_dir, fixtures_dir, "", &mut fixtures)?;
    Ok(fixtures)
}

fn walk_fixtures_recursive(
    root: &Path,
    current: &Path,
    relative_base: &str,
    fixtures: &mut Vec<Fixture>,
) -> Result<(), String> {
    let entries = fs::read_dir(current)
        .map_err(|e| format!("Failed to read directory {:?}: {}", current, e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            let dir_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| format!("Invalid directory name: {:?}", path))?;

            let new_relative = if relative_base.is_empty() {
                dir_name.to_string()
            } else {
                format!("{}/{}", relative_base, dir_name)
            };

            // Check for input files in this directory
            let input_files = ["input.svelte", "input.ts", "input.css"];
            let mut found_input = false;

            for input_file in &input_files {
                let input_path = path.join(input_file);
                if input_path.exists() {
                    fixtures.push(Fixture {
                        path: path.clone(),
                        relative_path: new_relative.clone(),
                        input_file: input_file.to_string(),
                    });
                    found_input = true;
                    break; // Only one input file per fixture
                }
            }

            // If no input file found, recurse into subdirectories
            if !found_input {
                walk_fixtures_recursive(root, &path, &new_relative, fixtures)?;
            }
        }
    }

    Ok(())
}

/// Read file contents
pub fn read_file(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|e| format!("Failed to read file {:?}: {}", path, e))
}

/// Write file contents
pub fn write_file(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content)
        .map_err(|e| format!("Failed to write file {:?}: {}", path, e))
}

/// Delete file if it exists
pub fn delete_file_if_exists(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path)
            .map_err(|e| format!("Failed to delete file {:?}: {}", path, e))?;
    }
    Ok(())
}
