use crate::cli::CliError;
use argh::FromArgs;
use std::path::{Path, PathBuf};
use tsv_cli::json_utils::to_json_with_tabs;
use tsv_svelte_compile::{CanonicalizeError, canonicalize_js};

/// Audit `canonicalize_js` at corpus scale: idempotence + output validity +
/// comment losslessness.
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
/// - **COMMENT-LOSS** — the reprint dropped, merged, duplicated, or reordered a
///   comment. A canonicalizer bug (failure), and the one the other two buckets
///   are structurally blind to: a swallowed comment leaves valid JS *and* is
///   idempotent (once it's gone it stays gone, so pass 2 reproduces pass 1). The
///   canonical path routes comment classification through its own line-break
///   table precisely to prevent this; this bucket is what proves the routing is
///   still intact at corpus scale.
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
    comment_loss: Vec<CommentLossEntry>,
}

#[derive(serde::Serialize)]
struct CorruptEntry {
    path: String,
    error: String,
}

#[derive(serde::Serialize)]
struct CommentLossEntry {
    path: String,
    detail: String,
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
                Ok(once) => {
                    let mut clean = true;
                    match canonicalize_js(&once) {
                        Ok(twice) if twice == once => {}
                        Ok(_) => {
                            report.non_idempotent.push(display(file));
                            clean = false;
                        }
                        Err(e) => {
                            report.corrupt_output.push(CorruptEntry {
                                path: display(file),
                                error: e.to_string(),
                            });
                            clean = false;
                        }
                    }
                    // Independent of idempotence: a swallowed comment is idempotent.
                    if let Some(detail) = comment_loss(&source, &once) {
                        report.comment_loss.push(CommentLossEntry {
                            path: display(file),
                            detail,
                        });
                        clean = false;
                    }
                    if clean {
                        report.clean += 1;
                    }
                }
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

        let failures =
            report.non_idempotent.len() + report.corrupt_output.len() + report.comment_loss.len();

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
            for entry in &report.comment_loss {
                println!("COMMENT-LOSS {}  ({})", entry.path, entry.detail);
            }
            println!(
                "canonicalize_audit: {} files — {} clean, {} input-rejected, {} non-idempotent, {} corrupt-output, {} comment-loss",
                report.files,
                report.clean,
                report.input_rejected,
                report.non_idempotent.len(),
                report.corrupt_output.len(),
                report.comment_loss.len()
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

/// Compare an input's comments against its canonical reprint's.
///
/// The canonical reprint may *move* a comment (an own-line comment can become a
/// trailing comment of the preceding node) but must never drop, merge, duplicate,
/// or reorder one. Comment text is whitespace-normalized before comparison because
/// re-indenting the interior of a multi-line block comment is legitimate
/// reformatting; every real failure still surfaces:
///
/// - **dropped** — the list shortens.
/// - **swallowed / merged** — the text itself changes: the tokens that got glued
///   onto the `//` comment's output line land *inside* the comment on reparse.
/// - **duplicated** — the list lengthens.
/// - **reordered** — the list permutes.
///
/// Returns `None` when the comments survive intact, `Some(detail)` naming the first
/// divergence otherwise. A parse failure yields `None`: the input was already
/// accepted by `canonicalize_js`, and a bad output is the CORRUPT-OUTPUT bucket's.
fn comment_loss(input: &str, output: &str) -> Option<String> {
    let before = comment_texts(input)?;
    let after = comment_texts(output)?;
    if before == after {
        return None;
    }
    // A common prefix that matches means the divergence is a count change.
    match before.iter().zip(&after).position(|(b, a)| b != a) {
        Some(i) => Some(format!(
            "comment {}/{} changed: {} -> {}",
            i + 1,
            before.len(),
            truncate(&before[i]),
            truncate(&after[i])
        )),
        None => Some(format!("comment count {} -> {}", before.len(), after.len())),
    }
}

/// Every comment in `source`, in source order, whitespace-normalized.
///
/// `None` if the source doesn't parse as a strict module.
fn comment_texts(source: &str) -> Option<Vec<String>> {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).ok()?;
    Some(
        program
            .comments
            .iter()
            .map(|c| {
                let text = &source[c.span.start as usize..c.span.end as usize];
                text.split_whitespace().collect::<Vec<_>>().join(" ")
            })
            .collect(),
    )
}

/// Clip a comment's text for a one-line report entry.
fn truncate(text: &str) -> String {
    const MAX: usize = 48;
    if text.chars().count() <= MAX {
        return format!("{text:?}");
    }
    let clipped: String = text.chars().take(MAX).collect();
    format!("{clipped:?}...")
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
