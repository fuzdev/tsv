use crate::cli::CliError;
use crate::compile_fixtures::{COMPILE_FIXTURES_DIR, walk_compile_fixtures};
use argh::FromArgs;
use std::path::Path;
use tsv_cli::json_utils::to_json_with_tabs;

/// Audit compile-fixture divergence integrity (the compiler analog of
/// `conformance_audit`, deliberately minimal).
///
/// A `_compiled_divergence`-suffixed compile fixture asserts a deliberate,
/// sanctioned difference from the canonical Svelte compiler's output. That
/// catalog (`docs/conformance_svelte_compiler.md`) is expected to stay empty —
/// it is a safety valve, not a tolerance budget — so this audit mostly asserts
/// emptiness:
///
/// 1. **Orphans** — every `_compiled_divergence` fixture must be linked in the
///    catalog document.
/// 2. **Missing back-links** — every such fixture must carry a `README.md`
///    containing a link that resolves to the catalog document.
///
/// Both are per-fixture, so with an empty catalog they check nothing; they are a
/// tripwire armed for a future entry. The third check does not depend on one:
///
/// 3. **Checklist ↔ `Refusal` drift** — every bucket key
///    `docs/checklist_svelte_compiler.md` quotes must be one the refusal catalog
///    can actually produce (see [`audit_refusal_keys`] for why only this
///    direction gates).
///
/// Pure Rust, no sidecar. Exits non-zero on any finding. Gated in
/// `deno task check`.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "compile_conformance_audit")]
pub struct CompileConformanceAuditCommand {
    /// emit a machine-readable JSON report
    #[argh(switch)]
    json: bool,
}

const CATALOG_DOC: &str = "docs/conformance_svelte_compiler.md";
const CHECKLIST_DOC: &str = "docs/checklist_svelte_compiler.md";
const DIVERGENCE_SUFFIX: &str = "_compiled_divergence";

#[derive(serde::Serialize)]
struct AuditReport {
    divergence_fixtures: usize,
    orphans: Vec<String>,
    missing_backlinks: Vec<String>,
    /// Keys the checklist quotes that no `Refusal` variant produces (gating).
    stale_refusal_keys: Vec<String>,
    /// Bucket keys the checklist never quotes (report-only — see the check's docs).
    unmentioned_refusal_keys: Vec<String>,
}

impl CompileConformanceAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let doc = match std::fs::read_to_string(CATALOG_DOC) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error reading {CATALOG_DOC}: {e}");
                return Err(CliError::Failed);
            }
        };

        // A missing tree is an empty tree (nothing to audit).
        let root = Path::new(COMPILE_FIXTURES_DIR);
        let fixtures = if root.exists() {
            walk_compile_fixtures(root).map_err(|e| {
                eprintln!("Error walking compile fixtures: {e}");
                CliError::Failed
            })?
        } else {
            Vec::new()
        };

        let checklist = match std::fs::read_to_string(CHECKLIST_DOC) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error reading {CHECKLIST_DOC}: {e}");
                return Err(CliError::Failed);
            }
        };
        let (stale_refusal_keys, unmentioned_refusal_keys) = audit_refusal_keys(&checklist);

        let mut report = AuditReport {
            divergence_fixtures: 0,
            orphans: Vec::new(),
            missing_backlinks: Vec::new(),
            stale_refusal_keys,
            unmentioned_refusal_keys,
        };

        for fixture in &fixtures {
            if !fixture.relative_path.ends_with(DIVERGENCE_SUFFIX) {
                continue;
            }
            report.divergence_fixtures += 1;
            if !doc.contains(&fixture.relative_path) {
                report.orphans.push(fixture.relative_path.clone());
            }
            let readme = fixture.path.join("README.md");
            let has_backlink = std::fs::read_to_string(&readme)
                .is_ok_and(|content| content.contains("conformance_svelte_compiler.md"));
            if !has_backlink {
                report.missing_backlinks.push(fixture.relative_path.clone());
            }
        }

        let findings =
            report.orphans.len() + report.missing_backlinks.len() + report.stale_refusal_keys.len();

        if self.json {
            match to_json_with_tabs(&report) {
                Ok(json) => println!("{json}"),
                Err(e) => {
                    eprintln!("Error serializing report: {e}");
                    return Err(CliError::Failed);
                }
            }
        } else {
            for orphan in &report.orphans {
                println!("ORPHAN: {orphan} not cataloged in {CATALOG_DOC}");
            }
            for missing in &report.missing_backlinks {
                println!("MISSING BACK-LINK: {missing} README must link {CATALOG_DOC}");
            }
            for stale in &report.stale_refusal_keys {
                println!(
                    "STALE REFUSAL KEY: {CHECKLIST_DOC} quotes `{stale}`, which no Refusal variant produces"
                );
            }
            if !report.unmentioned_refusal_keys.is_empty() {
                println!(
                    "note: {} bucket key(s) have no `**Refused**:` bullet in {CHECKLIST_DOC} (report-only)",
                    report.unmentioned_refusal_keys.len()
                );
            }
            println!(
                "compile_conformance_audit: {} divergence fixture(s), {} refusal key(s) quoted of {} produced, {} finding(s)",
                report.divergence_fixtures,
                quoted_refusal_keys(&checklist).len(),
                tsv_svelte_compile::Refusal::all_bucket_keys().len(),
                findings
            );
        }

        if findings > 0 {
            Err(CliError::Failed)
        } else {
            Ok(())
        }
    }
}

//
// Check 3 — checklist ↔ `Refusal` drift
//

/// Compare the refusal bucket keys `docs/checklist_svelte_compiler.md` quotes
/// against the keys the [`Refusal`](tsv_svelte_compile::Refusal) catalog actually
/// produces, returning `(stale, unmentioned)`.
///
/// The checklist's Coverage section claims its **Refused** bullets quote bucket
/// keys verbatim, so the document "maps one-to-one onto corpus runs". Nothing
/// verified that, and a renamed or deleted variant leaves the claim quietly false.
///
/// The two directions are graded differently on purpose:
///
/// - **stale** (a quoted key no variant produces) is a **gating** finding — the
///   doc asserts a bucket a corpus run can never report, which is the claim being
///   false in the direction a reader is misled by.
/// - **unmentioned** (a producible key the doc never quotes) is **report-only**.
///   A variant needs no checklist row to be correct: several are internal or are
///   covered by a prose paragraph rather than a `**Refused**:` bullet, so gating
///   this direction would be born red and would pressure the doc toward a
///   mechanical key dump rather than a coverage map.
fn audit_refusal_keys(checklist: &str) -> (Vec<String>, Vec<String>) {
    let produced: std::collections::BTreeSet<String> =
        tsv_svelte_compile::Refusal::all_bucket_keys()
            .iter()
            .map(|k| strip_backticks(k))
            .collect();
    let quoted = quoted_refusal_keys(checklist);

    let stale = quoted
        .iter()
        .filter(|k| !produced.contains(*k))
        .cloned()
        .collect();
    let unmentioned = produced
        .iter()
        .filter(|k| !quoted.contains(*k))
        .cloned()
        .collect();
    (stale, unmentioned)
}

/// Drop every backtick from a key, the one normalization applied to both sides.
fn strip_backticks(key: &str) -> String {
    key.replace('`', "").trim().to_string()
}

/// Every bucket key quoted by a `- **Refused**: `-prefixed checklist bullet.
///
/// The key is the bullet's leading code span (a ``` `` ```-fenced span when the
/// span contains a backtick). Both sides are compared with backticks stripped
/// ([`strip_backticks`]): a key may legitimately *contain* them (the `` `$:` ``
/// in the legacy-reactive-statement key), while the doc also decorates some
/// placeholders it quotes (`` `{name}` ``) — stripping is the one normalization
/// that reads both spellings the same. A bullet that opens with prose instead of
/// a code span makes no verbatim-key claim and is skipped.
fn quoted_refusal_keys(checklist: &str) -> std::collections::BTreeSet<String> {
    const MARKER: &str = "- **Refused**:";
    checklist
        .lines()
        .filter_map(|line| {
            let rest = line.trim_start().strip_prefix(MARKER)?.trim_start();
            let (fence, body) = if let Some(b) = rest.strip_prefix("``") {
                ("``", b)
            } else {
                ("`", rest.strip_prefix('`')?)
            };
            let key = body.split_once(fence)?.0;
            Some(strip_backticks(key))
        })
        .filter(|k| !k.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_refusal_keys_reads_both_fence_widths() {
        let md = "\
- **Refused**: `plain key {name}` — prose after the key.\n\
- **Refused**: `` key with a `$:` backtick `` — prose.\n\
- **Refused**: prose-first bullet with no code span\n\
- **Supported**: `not a refusal bullet`\n";
        let keys = quoted_refusal_keys(md);
        let got: Vec<&str> = keys.iter().map(String::as_str).collect();
        assert_eq!(got, vec!["key with a $: backtick", "plain key {name}"]);
    }

    #[test]
    fn every_quoted_key_is_producible() {
        // The gating direction, exercised on a synthetic doc so the test states
        // the rule rather than re-reading the live checklist.
        let real = tsv_svelte_compile::Refusal::ClientGeneration
            .bucket_key()
            .into_owned();
        let md = format!("- **Refused**: `{real}`\n- **Refused**: `no variant emits this`\n");
        let (stale, _) = audit_refusal_keys(&md);
        assert_eq!(stale, vec!["no variant emits this".to_string()]);
    }

    #[test]
    fn all_bucket_keys_covers_the_catalog() {
        // A representative per variant, so the audit's oracle is the whole catalog
        // rather than whichever variants happen to be constructed elsewhere.
        assert_eq!(
            tsv_svelte_compile::Refusal::every_variant().len(),
            126,
            "add the new Refusal variant to `every_variant` (it is not compiler-enforced)"
        );
    }
}
