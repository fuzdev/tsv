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
const DIVERGENCE_SUFFIX: &str = "_compiled_divergence";

#[derive(serde::Serialize)]
struct AuditReport {
    divergence_fixtures: usize,
    orphans: Vec<String>,
    missing_backlinks: Vec<String>,
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

        let mut report = AuditReport {
            divergence_fixtures: 0,
            orphans: Vec::new(),
            missing_backlinks: Vec::new(),
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

        let findings = report.orphans.len() + report.missing_backlinks.len();

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
            println!(
                "compile_conformance_audit: {} divergence fixture(s), {} finding(s)",
                report.divergence_fixtures, findings
            );
        }

        if findings > 0 {
            Err(CliError::Failed)
        } else {
            Ok(())
        }
    }
}
