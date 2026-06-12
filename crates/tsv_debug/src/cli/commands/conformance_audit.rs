use crate::fixtures;
use argh::FromArgs;
use std::collections::BTreeSet;
use std::path::Path;

/// Audit that every divergence-suffixed fixture is linked in its conformance doc.
///
/// Walks `tests/fixtures/` and verifies each divergence directory is referenced by a
/// `tests/fixtures/<path>` link in the doc that sanctions its claim:
///
/// - `_prettier_divergence` (incl. `_svelte_prettier_divergence`) → `docs/conformance_prettier.md`
/// - `_svelte_divergence` (incl. `_svelte_prettier_divergence`) → `docs/conformance_svelte.md`
///
/// A divergence suffix asserts a deliberate difference from a canonical tool; that
/// claim must be cataloged in the conformance doc so the divergence is sanctioned and
/// discoverable (and reviewers can find the rationale). A `_svelte_prettier_divergence`
/// fixture asserts both, so it must appear in both docs. Exits non-zero on any
/// unlinked fixture. Part of `deno task check`.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "conformance_audit")]
pub struct ConformanceAuditCommand {
    /// emit a machine-readable JSON report
    #[argh(switch)]
    json: bool,
}

const FIXTURES_DIR: &str = "tests/fixtures";

/// One doc-coverage audit: which suffix class must be linked in which doc.
struct Audit {
    doc_path: &'static str,
    suffix_label: &'static str,
    total: usize,
    unlinked: Vec<String>,
}

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

        let audits = [
            run_audit(
                &all,
                "docs/conformance_prettier.md",
                "_prettier_divergence",
                fixtures::Fixture::is_prettier_divergence,
            ),
            run_audit(
                &all,
                "docs/conformance_svelte.md",
                "_svelte_divergence",
                fixtures::Fixture::is_svelte_divergence,
            ),
        ];

        if self.json {
            print_json(&audits);
        } else {
            for audit in &audits {
                print_human(audit);
            }
        }

        if audits.iter().all(|a| a.unlinked.is_empty()) {
            std::process::exit(0);
        } else {
            std::process::exit(1);
        }
    }
}

fn run_audit(
    all: &[fixtures::Fixture],
    doc_path: &'static str,
    suffix_label: &'static str,
    is_in_class: impl Fn(&fixtures::Fixture) -> bool,
) -> Audit {
    let doc = match std::fs::read_to_string(doc_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {doc_path}: {e}");
            std::process::exit(1);
        }
    };
    let linked = extract_linked_fixtures(&doc);

    // Every divergence fixture (sorted/deduped) that must be cataloged.
    let divergence: BTreeSet<String> = all
        .iter()
        .filter(|f| is_in_class(f))
        .map(|f| normalize_fixture_path(&f.relative_path))
        .collect();

    let total = divergence.len();
    let unlinked: Vec<String> = divergence
        .into_iter()
        .filter(|p| !linked.contains(p))
        .collect();

    Audit {
        doc_path,
        suffix_label,
        total,
        unlinked,
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

fn print_human(audit: &Audit) {
    let Audit {
        doc_path,
        suffix_label,
        total,
        unlinked,
    } = audit;
    if unlinked.is_empty() {
        println!("✓ all {total} {suffix_label} fixtures linked in {doc_path}");
        return;
    }
    eprintln!(
        "✗ {} of {total} {suffix_label} fixtures NOT linked in {doc_path}:\n",
        unlinked.len(),
    );
    for p in unlinked {
        eprintln!("  - {p}");
    }
    eprintln!(
        "\nEach {suffix_label} fixture asserts a deliberate difference from the canonical tool.\n\
         Add a `tests/fixtures/<path>` link in {doc_path} so the divergence is\n\
         sanctioned and discoverable."
    );
}

fn print_json(audits: &[Audit]) {
    let report: Vec<_> = audits
        .iter()
        .map(|a| {
            serde_json::json!({
                "doc": a.doc_path,
                "suffix": a.suffix_label,
                "total": a.total,
                "unlinked_count": a.unlinked.len(),
                "unlinked": a.unlinked,
            })
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&report).unwrap_or_default()
    );
}
