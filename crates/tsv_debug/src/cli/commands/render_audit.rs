//! Corpus-scale render-equivalence audit: does `tsv format` change what a Svelte
//! component RENDERS?
//!
//! ## Why this exists
//!
//! The fixture render-equivalence check (the R rules, `deno task fixtures:validate`)
//! asks this question of `tests/fixtures` — a **curated** corpus whose whitespace
//! variants are hand-authored to be render-equivalent in the first place. That is a
//! regression guard, not a discovery tool: it is close to the least likely place for
//! a render change to hide. The exposure is **real code**, where the formatter meets
//! authorings nobody curated — the same gap `audit:corpus` exists to close for the
//! content-loss class ("the extension-robustness bar `deno task check`'s fixture-only
//! scope is structurally blind to").
//!
//! This command is that corpus-scale arm: for every `.svelte` file, compare the
//! browser-visible **render key** of the source against the render key of
//! `tsv format(source)`. A difference means formatting changed what the page renders
//! — silent, user-visible corruption that no other gate sees. `corpus:compare:format`
//! is a char-frequency SAFETY check (blind: the characters only MOVE),
//! `roundtrip_audit`'s skeleton erases the whitespace that carries the meaning, and
//! `authoring_audit` asks the *convergence* question (do two authorings reach one
//! fixed point), never whether that fixed point renders like the input.
//!
//! ## Oracle
//!
//! `deno::svelte_render_key` — `svelte compile --generate server` reduced to its
//! browser-visible render (baked template text, `${…}` holed out,
//! `<script>`/`<style>`/comments stripped, whitespace collapsed with block-boundary
//! whitespace dropped). Equal keys prove equal renders. Because the key is
//! baked-template-only, a `<script>`/`<style>` reformatting that leaves the template
//! alone is correctly ignored.
//!
//! Files whose format is a no-op are skipped (trivially render-equal), and files
//! Svelte's semantic **analyzer** rejects are counted as compile-skipped — that arm
//! cannot run there, exactly as in the fixture check.
//!
//! This is the in-repo, any-corpus form of
//! `../test-svelte-prettier-whitespace/whitespace-safety-check.mjs`, which asks the
//! same question of a git working tree vs `HEAD`.

use std::path::{Path, PathBuf};

use argh::FromArgs;
use futures_util::stream::{self, StreamExt};

use crate::audit::vacuity::check_graded_nonzero;
use crate::cli::CliError;
use crate::deno;
use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;

use super::profile::{is_input_invalid_fixture, is_svelte, resolve_seed_files_named};

/// Audit whether `tsv format` changes what a Svelte component renders.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "render_audit")]
pub struct RenderAuditCommand {
    /// exit 1 on any finding (a formatted file that renders differently)
    #[argh(switch)]
    gate: bool,

    /// machine-readable JSON report
    #[argh(switch)]
    json: bool,

    /// cap the number of files audited (0 = unlimited)
    #[argh(option, default = "0")]
    limit: usize,

    /// files or directories to audit (default: tests/fixtures)
    #[argh(positional)]
    files: Vec<String>,
}

/// What happened to one file.
enum Outcome {
    /// `format` is a no-op — trivially render-equal, nothing to check.
    Unchanged,
    /// Formatted, and the render key is unchanged. The good case.
    Preserved,
    /// Formatted, and the render key CHANGED — the finding.
    Changed { before: String, after: String },
    /// tsv could not format the file (a parse gap other gates own).
    FormatError,
    /// Svelte's analyzer rejects one side, so the oracle cannot run here.
    CompileSkipped,
}

struct Tally {
    unchanged: usize,
    preserved: usize,
    format_error: usize,
    compile_skipped: usize,
    findings: Vec<(PathBuf, String, String)>,
}

impl RenderAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        // Svelte templates only — the render question is meaningless elsewhere.
        // `input_invalid_*` fixtures are deliberately unparseable.
        let files = resolve_seed_files_named(&self.files, self.limit, ".svelte files", |p| {
            is_svelte(p) && !is_input_invalid_fixture(p)
        })?;

        let concurrency = deno::init_bulk_pool();
        let rt = super::create_runtime();
        let tally = rt.block_on(audit_files(&files, concurrency));

        self.report(&tally, files.len());

        if self.gate && !tally.findings.is_empty() {
            return Err(CliError::Failed);
        }
        // The floor UNDER the non-empty resolution: resolution proves `.svelte` files
        // were found, not that the render question was ever put. The two skip buckets
        // are the ones that could not put it — tsv failed to format, or the analyzer
        // rejected a side — and a run that is entirely those reports "✓ no findings"
        // and exits 0, indistinguishable from a clean sweep. A no-op format IS a
        // verdict (render equality by identity), so it counts toward the floor even
        // though the report calls it skipped: it skips the ORACLE, not the question.
        check_graded_nonzero(
            tally.unchanged + tally.preserved + tally.findings.len(),
            ".svelte files render-checked",
        )
    }

    fn report(&self, tally: &Tally, total: usize) {
        if self.json {
            let findings: Vec<_> = tally
                .findings
                .iter()
                .map(|(p, before, after)| {
                    serde_json::json!({
                        "file": p.display().to_string(),
                        "render_key_before": before,
                        "render_key_after": after,
                    })
                })
                .collect();
            let report = serde_json::json!({
                "scanned": total,
                "unchanged": tally.unchanged,
                "preserved": tally.preserved,
                "format_error": tally.format_error,
                "compile_skipped": tally.compile_skipped,
                "findings": findings,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&report).unwrap_or_default()
            );
            return;
        }

        for (path, before, after) in &tally.findings {
            println!("⚠️  RENDER CHANGED  {}", path.display());
            println!("    before: {}", truncate(before));
            println!("    after : {}", truncate(after));
            println!();
        }

        println!("Render audit — {total} .svelte file(s)");
        println!("  render preserved across format : {}", tally.preserved);
        println!("  format is a no-op (skipped)    : {}", tally.unchanged);
        println!(
            "  compile-blind (analyzer reject): {}",
            tally.compile_skipped
        );
        println!("  tsv format errors (skipped)    : {}", tally.format_error);
        if tally.findings.is_empty() {
            println!("✓ no findings — formatting preserved the render on every checked file");
        } else {
            println!(
                "✗ {} file(s) RENDER DIFFERENTLY after formatting",
                tally.findings.len()
            );
        }
    }
}

/// Truncate a render key for display — they can be long.
fn truncate(s: &str) -> String {
    const MAX: usize = 240;
    if s.chars().count() <= MAX {
        return s.to_string();
    }
    let head: String = s.chars().take(MAX).collect();
    format!("{head}… ({} chars)", s.chars().count())
}

async fn audit_files(files: &[PathBuf], concurrency: usize) -> Tally {
    let results = stream::iter(files.iter().map(|path| async move {
        let outcome = audit_file(path).await;
        (path.clone(), outcome)
    }))
    .buffer_unordered(concurrency)
    .collect::<Vec<_>>()
    .await;

    let mut tally = Tally {
        unchanged: 0,
        preserved: 0,
        format_error: 0,
        compile_skipped: 0,
        findings: Vec::new(),
    };
    for (path, outcome) in results {
        match outcome {
            Outcome::Unchanged => tally.unchanged += 1,
            Outcome::Preserved => tally.preserved += 1,
            Outcome::CompileSkipped => tally.compile_skipped += 1,
            Outcome::FormatError => tally.format_error += 1,
            Outcome::Changed { before, after } => tally.findings.push((path, before, after)),
        }
    }
    tally.findings.sort_by(|a, b| a.0.cmp(&b.0));
    tally
}

async fn audit_file(path: &Path) -> Outcome {
    let Ok(source) = std::fs::read_to_string(path) else {
        return Outcome::FormatError;
    };

    let Ok(formatted) = format_source(&source, ParserType::Svelte) else {
        return Outcome::FormatError;
    };
    if formatted == source {
        return Outcome::Unchanged;
    }

    // Both sides must be analyzable for the oracle to speak.
    let (Ok(before), Ok(after)) = (
        deno::svelte_render_key(&source).await,
        deno::svelte_render_key(&formatted).await,
    ) else {
        return Outcome::CompileSkipped;
    };

    if before == after {
        Outcome::Preserved
    } else {
        Outcome::Changed { before, after }
    }
}
