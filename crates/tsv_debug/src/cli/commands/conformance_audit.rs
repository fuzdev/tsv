use crate::fixtures;
use argh::FromArgs;
use std::collections::BTreeSet;
use std::path::Path;

/// Audit that every `_prettier_divergence` fixture is linked in
/// `docs/conformance_prettier.md`.
///
/// Walks `tests/fixtures/` for directories with a prettier-divergence suffix
/// (`_prettier_divergence` or `_svelte_prettier_divergence`) and verifies each is
/// referenced by a `tests/fixtures/<path>` link in `docs/conformance_prettier.md`.
/// The suffix asserts a deliberate formatting difference from Prettier; that claim
/// must be cataloged in the conformance doc so the divergence is sanctioned and
/// discoverable (and reviewers can find the rationale). A `_svelte_prettier_divergence`
/// fixture has a prettier aspect too, so it must appear here as well as in
/// `conformance_svelte.md`. Exits non-zero on any unlinked fixture. Part of
/// `deno task check`.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "conformance_audit")]
pub struct ConformanceAuditCommand {
    /// emit a machine-readable JSON report
    #[argh(switch)]
    json: bool,
}

const FIXTURES_DIR: &str = "tests/fixtures";
const DOC_PATH: &str = "docs/conformance_prettier.md";

impl ConformanceAuditCommand {
    pub fn run(self) {
        let fixtures_dir = Path::new(FIXTURES_DIR);
        if !fixtures_dir.exists() {
            eprintln!("Error: fixtures directory not found: {FIXTURES_DIR}");
            std::process::exit(1);
        }

        let all = match fixtures::walk_fixtures(fixtures_dir) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error walking fixtures: {e}");
                std::process::exit(1);
            }
        };

        let doc = match std::fs::read_to_string(DOC_PATH) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error reading {DOC_PATH}: {e}");
                std::process::exit(1);
            }
        };
        let linked = extract_linked_fixtures(&doc);

        // Every prettier-divergence fixture (sorted/deduped) that must be cataloged.
        let divergence: BTreeSet<String> = all
            .iter()
            .filter(|f| f.is_prettier_divergence())
            .map(|f| normalize_fixture_path(&f.relative_path))
            .collect();

        let unlinked: Vec<&String> = divergence.iter().filter(|p| !linked.contains(*p)).collect();

        if self.json {
            print_json(divergence.len(), &unlinked);
        } else {
            print_human(divergence.len(), &unlinked);
        }

        if unlinked.is_empty() {
            std::process::exit(0);
        } else {
            std::process::exit(1);
        }
    }
}

/// Strip a fixture's `relative_path` (`./tests/fixtures/<p>`) down to `<p>`.
fn normalize_fixture_path(rel: &str) -> String {
    rel.rsplit_once("tests/fixtures/")
        .map_or(rel, |(_, p)| p)
        .trim_end_matches('/')
        .to_string()
}

/// Extract every `tests/fixtures/<path>` reference in the conformance doc,
/// normalized to `<path>` (trailing slash stripped).
///
/// Captures any link or prose form — `(../tests/fixtures/foo/)`, multiple links
/// in one table cell, etc. — since we only need set membership, not link
/// well-formedness. A path ends at the first `)`, `]`, backtick, `|`, or
/// whitespace.
fn extract_linked_fixtures(doc: &str) -> BTreeSet<String> {
    const MARKER: &str = "tests/fixtures/";
    let mut set = BTreeSet::new();
    let mut rest = doc;
    while let Some(idx) = rest.find(MARKER) {
        let after = &rest[idx + MARKER.len()..];
        let end = after
            .find(|c: char| c == ')' || c == ']' || c == '`' || c == '|' || c.is_whitespace())
            .unwrap_or(after.len());
        let path = after[..end].trim_end_matches('/');
        if !path.is_empty() {
            set.insert(path.to_string());
        }
        rest = &after[end..];
    }
    set
}

fn print_human(total: usize, unlinked: &[&String]) {
    if unlinked.is_empty() {
        println!("✓ all {total} _prettier_divergence fixtures linked in {DOC_PATH}");
        return;
    }
    eprintln!(
        "✗ {} of {total} _prettier_divergence fixtures NOT linked in {DOC_PATH}:\n",
        unlinked.len(),
    );
    for p in unlinked {
        eprintln!("  - {p}");
    }
    eprintln!(
        "\nEach _prettier_divergence fixture asserts a deliberate difference from Prettier.\n\
         Add a `tests/fixtures/<path>` link in {DOC_PATH} §Catalog (e.g. the §Comment\n\
         relocation table for comment-position divergences) so the divergence is\n\
         sanctioned and discoverable."
    );
}

fn print_json(total: usize, unlinked: &[&String]) {
    let report = serde_json::json!({
        "doc": DOC_PATH,
        "total": total,
        "unlinked_count": unlinked.len(),
        "unlinked": unlinked,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).unwrap_or_default()
    );
}
