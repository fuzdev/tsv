//! Compiler fixture discovery and file conventions.
//!
//! Compile fixtures live in their own tree — `tests/fixtures_compile/` — so the
//! parser/formatter fixture counts and validation in `tests/fixtures/` stay
//! unperturbed. Each fixture directory holds:
//!
//! - `input.svelte` — a runes component (the canonical Svelte compiler rejects
//!   legacy syntax), prettier-formatted by `compile_fixture_init`.
//! - `expected_server.js` — the **canonicalized** oracle server output:
//!   `canonicalize_js(svelte_compile(input, Server, dev=false).js)`. Always
//!   oracle-generated, never hand-written.
//! - `expected.css` — the raw oracle CSS, present only when the component has a
//!   `<style>` that produces output (deterministic `svelte-tsvhash` scoping class).
//!
//! The same hierarchy rule as the main tree applies: every directory is either a
//! fixture (has `input.svelte`) or a container (has subdirectories), never both,
//! never neither.

use std::fs;
use std::path::{Path, PathBuf};

/// The compile fixture tree root, relative to the repo root.
pub const COMPILE_FIXTURES_DIR: &str = "tests/fixtures_compile";

/// The fixture's component file name.
pub const INPUT_FILE: &str = "input.svelte";

/// The canonicalized oracle server-JS file name.
pub const EXPECTED_SERVER_JS: &str = "expected_server.js";

/// The raw oracle CSS file name (present only for styled components).
pub const EXPECTED_CSS: &str = "expected.css";

/// A discovered compile fixture.
#[derive(Debug, Clone)]
pub struct CompileFixture {
    /// Absolute-or-cwd-relative path to the fixture directory.
    pub path: PathBuf,
    /// Path relative to the compile fixture root (e.g. `text/hello_world`).
    pub relative_path: String,
}

impl CompileFixture {
    /// Whether this fixture matches any of the given case-insensitive substring
    /// filters (empty filters match everything). Mirrors the main tree's
    /// `Fixture::matches_filters`.
    pub fn matches_filters(&self, filters: &[String]) -> bool {
        if filters.is_empty() {
            return true;
        }
        let lower = self.relative_path.to_lowercase();
        filters.iter().any(|f| lower.contains(&f.to_lowercase()))
    }

    /// The fixture's `input.svelte` path.
    pub fn input_path(&self) -> PathBuf {
        self.path.join(INPUT_FILE)
    }

    /// The fixture's `expected_server.js` path.
    pub fn expected_server_js_path(&self) -> PathBuf {
        self.path.join(EXPECTED_SERVER_JS)
    }

    /// The fixture's `expected.css` path.
    pub fn expected_css_path(&self) -> PathBuf {
        self.path.join(EXPECTED_CSS)
    }
}

/// Walk the compile fixture tree, enforcing the fixture-or-container hierarchy
/// rule. Returns fixtures sorted by relative path (deterministic output order).
///
/// # Errors
///
/// Returns an error when a directory has both `input.svelte` and
/// subdirectories, or neither, or when a directory read fails.
pub fn walk_compile_fixtures(root: &Path) -> Result<Vec<CompileFixture>, String> {
    let mut fixtures = Vec::new();
    walk_recursive(root, "", &mut fixtures)?;
    fixtures.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(fixtures)
}

fn walk_recursive(
    current: &Path,
    relative: &str,
    fixtures: &mut Vec<CompileFixture>,
) -> Result<(), String> {
    let has_input = current.join(INPUT_FILE).exists();
    let mut subdirs: Vec<PathBuf> = fs::read_dir(current)
        .map_err(|e| format!("Failed to read directory {current:?}: {e}"))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    subdirs.sort();

    if has_input && !subdirs.is_empty() {
        return Err(format!(
            "Directory {current:?} has both {INPUT_FILE} and subdirectories — \
             every directory must be a fixture or a container, not both"
        ));
    }
    if has_input {
        fixtures.push(CompileFixture {
            path: current.to_path_buf(),
            relative_path: relative.to_string(),
        });
        return Ok(());
    }
    // The root itself may be empty (no fixtures yet); a non-root leaf with
    // neither input nor children is an orphan.
    if subdirs.is_empty() && !relative.is_empty() {
        return Err(format!(
            "Directory {current:?} has neither {INPUT_FILE} nor subdirectories (orphan)"
        ));
    }
    for subdir in subdirs {
        let name = subdir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("Invalid directory name: {subdir:?}"))?;
        let child_relative = if relative.is_empty() {
            name.to_string()
        } else {
            format!("{relative}/{name}")
        };
        walk_recursive(&subdir, &child_relative, fixtures)?;
    }
    Ok(())
}

/// Ensure `content` ends with exactly the bytes it has plus a trailing newline
/// when missing — the normalization applied both when writing an expected file
/// and when comparing a freshly generated value against it.
pub fn with_trailing_newline(content: &str) -> String {
    if content.ends_with('\n') {
        content.to_string()
    } else {
        format!("{content}\n")
    }
}
