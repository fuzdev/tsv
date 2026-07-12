use crate::cli::CliError;
use argh::FromArgs;
use std::path::{Path, PathBuf};
use tsv_cli::json_utils::to_json_with_tabs;
use tsv_svelte_compile::{CanonicalizeError, canonicalize_js};

/// Audit `canonicalize_js` at corpus scale: idempotence + output validity.
///
/// Walks the given paths (default `tests/fixtures`) for TS/JS sources and runs
/// the canonicalizer twice on each file. Buckets:
///
/// - **input-rejected** — the file doesn't parse as a strict TS module
///   (deliberately-invalid fixtures, script-goal JS). Informational skip.
/// - **NON-IDEMPOTENT** — canonicalize(canonicalize(x)) != canonicalize(x).
///   A canonicalizer bug (failure).
/// - **CORRUPT-OUTPUT** — the canonical reprint failed to reparse (the
///   canonicalizer's self-validation fired, or the second pass rejected the
///   first's output). A canonicalizer bug (failure).
///
/// Pure Rust, no sidecar. Exits 1 on any failure. The `canonicalize:audit` deno
/// task gates this over `tests/fixtures` in `deno task check`; point it at real
/// corpora on demand (`canonicalize_audit ~/dev/zzz/src ~/dev/gro/src`).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "canonicalize_audit")]
pub struct CanonicalizeAuditCommand {
    /// emit a machine-readable JSON report
    #[argh(switch)]
    json: bool,

    /// directories or files to audit (default: tests/fixtures)
    #[argh(positional)]
    paths: Vec<String>,
}

/// The audit's aggregate outcome (also the `--json` shape).
#[derive(Default, serde::Serialize)]
struct AuditReport {
    files: usize,
    clean: usize,
    input_rejected: usize,
    non_idempotent: Vec<String>,
    corrupt_output: Vec<CorruptEntry>,
}

#[derive(serde::Serialize)]
struct CorruptEntry {
    path: String,
    error: String,
}

impl CanonicalizeAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let paths = if self.paths.is_empty() {
            vec!["tests/fixtures".to_string()]
        } else {
            self.paths.clone()
        };

        let mut files = Vec::new();
        for path in &paths {
            let p = Path::new(path);
            if !p.exists() {
                eprintln!("Error: path not found: {path}");
                return Err(CliError::Failed);
            }
            collect_sources(p, &mut files);
        }
        files.sort();

        let mut report = AuditReport {
            files: files.len(),
            ..AuditReport::default()
        };

        for file in &files {
            let Ok(source) = std::fs::read_to_string(file) else {
                // Unreadable (permissions, non-UTF-8): treat like a rejected input.
                report.input_rejected += 1;
                continue;
            };
            match canonicalize_js(&source) {
                Ok(once) => match canonicalize_js(&once) {
                    Ok(twice) if twice == once => report.clean += 1,
                    Ok(_) => report.non_idempotent.push(display(file)),
                    Err(e) => report.corrupt_output.push(CorruptEntry {
                        path: display(file),
                        error: e.to_string(),
                    }),
                },
                // Self-validation catches corruption inside the first call.
                Err(CanonicalizeError::CorruptOutput(e)) => {
                    report.corrupt_output.push(CorruptEntry {
                        path: display(file),
                        error: e.to_string(),
                    });
                }
                Err(CanonicalizeError::Parse(_)) => report.input_rejected += 1,
            }
        }

        let failures = report.non_idempotent.len() + report.corrupt_output.len();

        if self.json {
            match to_json_with_tabs(&report) {
                Ok(json) => println!("{json}"),
                Err(e) => {
                    eprintln!("Error serializing report: {e}");
                    return Err(CliError::Failed);
                }
            }
        } else {
            for path in &report.non_idempotent {
                println!("NON-IDEMPOTENT {path}");
            }
            for entry in &report.corrupt_output {
                println!("CORRUPT-OUTPUT {}  ({})", entry.path, entry.error);
            }
            println!(
                "canonicalize_audit: {} files — {} clean, {} input-rejected, {} non-idempotent, {} corrupt-output",
                report.files,
                report.clean,
                report.input_rejected,
                report.non_idempotent.len(),
                report.corrupt_output.len()
            );
        }

        if failures > 0 {
            Err(CliError::Failed)
        } else {
            Ok(())
        }
    }
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

/// Recursively collect auditable sources: `.ts` / `.js` / `.mts` / `.cts`
/// (`.svelte.ts` is `.ts`-suffixed and included). Skips the usual non-source
/// directories so the audit can point at real repos.
fn collect_sources(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        if is_auditable(path) {
            out.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let child = entry.path();
        if child.is_dir() {
            let name = child.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.')
                || name == "node_modules"
                || name == "dist"
                || name == "build"
                || name == "target"
            {
                continue;
            }
            collect_sources(&child, out);
        } else if is_auditable(&child) {
            out.push(child);
        }
    }
}

fn is_auditable(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name.ends_with(".ts")
        || name.ends_with(".js")
        || name.ends_with(".mts")
        || name.ends_with(".cts")
        || name.ends_with(".mjs")
        || name.ends_with(".cjs")
}
