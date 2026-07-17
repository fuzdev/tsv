//! Corpus-scale format→reparse round-trip audit (the escape/delimiter
//! data-corruption gate).
//!
//! ## Why this exists
//!
//! The existing corpus gates cannot see a whole class of formatter bug:
//! **output that mis-delimits but loses no characters.** `corpus:compare:format`
//! guards output against prettier with a differential char-frequency SAFETY
//! check — which is *blind* to delimiter/structure corruption (a value re-quoted
//! `attr='a"b'` → `attr="a"b"` preserves every char `a b " =`, only the delimiter
//! STRUCTURE is wrong, so the frequencies match). `corpus:compare:parse` diffs
//! tsv's parse of the *input* against the canonical parsers — it never reparses
//! tsv's *formatted output*. The single-file `ast_diff` command *does* the right
//! round-trip (parse → format → parse → compare, optionally `--render`), but only
//! one file at a time.
//!
//! This command is the missing corpus-scale runner: for every file, does
//! `format(src)` **reparse** to the same document? A `no` is the strongest
//! correctness signal there is — the drop-in-replacement contract broken.
//!
//! ## Two-phase oracle (tsv-self pre-filter → canonical confirm)
//!
//! 1. **tsv-self** (pure Rust, no sidecar — fast, runs over every file): parse
//!    input and formatted output with tsv's own parser, render-normalize, compare.
//!    - output tsv can't reparse → [`TsvVerdict::Unreparseable`] (always a bug:
//!      the formatter emitted something its own parser rejects);
//!    - reparses but the AST diverges → [`TsvVerdict::Divergent`] (a suspect).
//! 2. **canonical confirm** (Svelte / acorn-typescript / parseCss via the Deno
//!    sidecar): the drop-in contract oracle. Runs on the tsv-self suspects by
//!    default, or on **every** input-accepted file with `--canonical-all` (the
//!    thorough gate mode — closes tsv-self's blind spot, where tsv's own parser
//!    happens to accept a corruption identically).
//!    - canonical throws on tsv's output → [`CanVerdict::Unreparseable`] (invalid
//!      per the real language — the prize);
//!    - reparses but diverges under render-equivalence → [`CanVerdict::Divergent`].
//!
//! Canonical is authoritative where it runs: a canonical-Clean overrules a
//! tsv-self `Divergent` (a tsv wire-shape quirk, not a real corruption), but never
//! a tsv-self `Unreparseable` (that is a genuine tsv-parser-on-own-output bug).
//!
//! The four finding buckets (`{tsv,canonical}_unreparseable`,
//! `{tsv,canonical}_divergent`) are the work-list; `format_error` (tsv rejects the
//! input — a parse-gap for other gates) and `canonical_rejects_input` (an invalid /
//! error fixture) are counted and skipped, not findings.
//!
//! ## `--gate` (the `deno task check` guard)
//!
//! `--gate` fails on the `*_unreparseable` buckets only (the divergent buckets
//! are render-model noise over `tests/fixtures`). Over `tests/fixtures` that guard
//! is a **cheap tripwire**: the fixture idempotency/normalization invariants
//! (`fixtures_validate` F1/N, also in `deno task check`) already make every
//! formatted output reparse, so the bucket is ~always 0 there and a regression
//! that broke it would trip those checks too. The real yield is on **external
//! corpora** — point it at `../prettier/tests/format/*` and real repos, where it
//! surfaces corruption no fixture covers. Kept in `check` as a fast (~1.4s),
//! pure-Rust backstop, not the primary detector.

use argh::FromArgs;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use futures_util::{StreamExt, stream};

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;

use crate::audit::properties::{structurally_equivalent, tsv_parse_to_value};
use crate::cli::CliError;
use crate::deno;

use super::profile::resolve_files;

/// Audit whether every file's formatted output reparses to the same document.
///
/// Phase 1 (pure Rust) round-trips each file through tsv's own parser; phase 2
/// confirms the suspects (or every file, with `--canonical-all`) against the
/// canonical parsers via the sidecar. Defaults to `tests/fixtures` when no paths
/// are given — point it at the corpus (`../prettier/tests/format/{css,js,typescript,html}`,
/// `../zzz/src`, `../svelte/packages/svelte/src`, …) to generate the work-list.
///
/// `--gate` restricts the failing set to the reliable `*_unreparseable` buckets
/// (the divergent buckets are render-model noise over `tests/fixtures`); a bare
/// `--gate` runs phase 1 only, the fast pure-Rust `deno task check` guard, while
/// `--gate --canonical-all` is the thorough release-cadence form that also guards
/// `canonical_unreparseable` (tsv's own parser accepting output the real parser
/// rejects).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "roundtrip_audit")]
#[allow(clippy::struct_excessive_bools)] // independent CLI flags
pub struct RoundtripAuditCommand {
    /// gate mode: fail (exit 1) ONLY on the {tsv,canonical}_unreparseable
    /// buckets — the reliable half. The divergent buckets (render-model noise)
    /// are still counted but non-fatal. Bare `--gate` runs phase 1 only
    /// (pure Rust, no sidecar); add `--canonical-all` for the canonical
    /// unreparseable guard too. This is the `deno task check` regression-guard
    /// mode.
    #[argh(switch)]
    gate: bool,

    /// run the canonical reparse on EVERY input-accepted file, not just the
    /// tsv-self-flagged suspects (thorough gate mode; slower — one sidecar
    /// round-trip per file)
    #[argh(switch)]
    canonical_all: bool,

    /// disable Svelte-5 render-time whitespace normalization before comparing
    /// (default: on, matching `ast_diff --render`; no effect under a bare
    /// `--gate`, which skips the comparison)
    #[argh(switch)]
    no_render: bool,

    /// print the AST diff for each divergent finding
    #[argh(switch)]
    verbose: bool,

    /// cap the number of files audited (0 = unlimited)
    #[argh(option, default = "0")]
    limit: usize,

    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// file paths / directories (default: tests/fixtures)
    #[argh(positional)]
    paths: Vec<String>,
}

/// tsv-self round-trip verdict (phase 1, pure Rust).
#[derive(Clone, Copy, PartialEq, Eq)]
enum TsvVerdict {
    /// Formatted output reparses to the same (render-normalized) AST.
    Clean,
    /// tsv could not format the input (a parse-gap; out of scope here).
    FormatError,
    /// tsv's own parser rejects tsv's own formatted output.
    Unreparseable,
    /// Output reparses but the AST diverges.
    Divergent,
}

/// Canonical round-trip verdict (phase 2, via the sidecar).
#[derive(Clone, Copy, PartialEq, Eq)]
enum CanVerdict {
    /// Canonical parse of input and output are render-equivalent.
    Clean,
    /// The canonical parser rejects the *input* (invalid / error fixture).
    RejectsInput,
    /// The canonical parser throws on tsv's *output* (invalid per the real language).
    Unreparseable,
    /// Output reparses but the AST diverges.
    Divergent,
}

/// The single reported bucket for a file, resolved by severity from the two
/// phases' verdicts (canonical authoritative where present, save for a tsv-self
/// `Unreparseable`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Bucket {
    Clean,
    FormatError,
    CanonicalRejectsInput,
    CanonicalUnreparseable,
    TsvUnreparseable,
    CanonicalDivergent,
    TsvDivergent,
}

impl Bucket {
    /// A bucket that fails the audit (a real or suspected corruption).
    fn is_finding(self) -> bool {
        matches!(
            self,
            Self::CanonicalUnreparseable
                | Self::TsvUnreparseable
                | Self::CanonicalDivergent
                | Self::TsvDivergent
        )
    }

    /// The reliable half — output the parser rejects. These are the only
    /// buckets `--gate` mode fails on (the divergent buckets are the noisy
    /// render-model half, reported but non-fatal there).
    fn is_unreparseable(self) -> bool {
        matches!(self, Self::CanonicalUnreparseable | Self::TsvUnreparseable)
    }

    fn label(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::FormatError => "format_error",
            Self::CanonicalRejectsInput => "canonical_rejects_input",
            Self::CanonicalUnreparseable => "canonical_unreparseable",
            Self::TsvUnreparseable => "tsv_unreparseable",
            Self::CanonicalDivergent => "canonical_divergent",
            Self::TsvDivergent => "tsv_divergent",
        }
    }

    /// A stable severity rank for sorting findings worst-first.
    fn severity(self) -> u8 {
        match self {
            Self::CanonicalUnreparseable => 0,
            Self::TsvUnreparseable => 1,
            Self::CanonicalDivergent => 2,
            Self::TsvDivergent => 3,
            _ => 9,
        }
    }
}

/// One audited file's outcome across both phases.
struct FileResult {
    display: String,
    path: PathBuf,
    parser: ParserType,
    tsv: TsvVerdict,
    canonical: Option<CanVerdict>,
    /// Captured AST diff (only with `--verbose` on a divergence).
    diff: Option<String>,
}

impl FileResult {
    /// Resolve the two phases' verdicts into one reported bucket, worst-first.
    fn bucket(&self) -> Bucket {
        // Canonical rejecting tsv's output is the headline drop-in violation.
        if self.canonical == Some(CanVerdict::Unreparseable) {
            return Bucket::CanonicalUnreparseable;
        }
        // tsv rejecting its own output is always a real bug (never masked).
        if self.tsv == TsvVerdict::Unreparseable {
            return Bucket::TsvUnreparseable;
        }
        if self.canonical == Some(CanVerdict::Divergent) {
            return Bucket::CanonicalDivergent;
        }
        if self.tsv == TsvVerdict::Divergent {
            // A canonical-Clean overrules a tsv-self divergence (a tsv wire-shape
            // quirk, not a corruption). Otherwise the suspect stands.
            if self.canonical == Some(CanVerdict::Clean) {
                return Bucket::Clean;
            }
            return Bucket::TsvDivergent;
        }
        if self.canonical == Some(CanVerdict::RejectsInput) {
            return Bucket::CanonicalRejectsInput;
        }
        if self.tsv == TsvVerdict::FormatError {
            return Bucket::FormatError;
        }
        Bucket::Clean
    }
}

impl RoundtripAuditCommand {
    /// Whether the canonical (sidecar) phase runs. `--gate` skips it unless
    /// `--canonical-all` is also given, so a bare `--gate` is a pure-Rust
    /// phase-1 gate; every non-gate run confirms against canonical.
    fn runs_canonical(&self) -> bool {
        !self.gate || self.canonical_all
    }

    /// The phase-1 fast path — check only that the output *reparses*, skipping
    /// the wire-JSON convert + skeleton compare. Sound **exactly when** the
    /// canonical phase won't run (`!runs_canonical`): then the divergent verdict
    /// is never consumed (a bare `--gate` fails on `tsv_unreparseable` alone), so
    /// computing it is dead weight. Deriving it from `runs_canonical` keeps that
    /// invariant in one place — the fast path can never outlive its safety
    /// condition.
    fn reparse_only(&self) -> bool {
        !self.runs_canonical()
    }

    pub(crate) fn run(self) -> Result<(), CliError> {
        let paths = if self.paths.is_empty() {
            vec!["tests/fixtures".to_string()]
        } else {
            self.paths.clone()
        };
        let mut files = match resolve_files(&paths) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error: {e}");
                return Err(CliError::Failed);
            }
        };
        // Intentionally-invalid fixture inputs aren't round-trip subjects.
        files.retain(|p| {
            !p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("input_invalid"))
        });
        if self.limit > 0 {
            files.truncate(self.limit);
        }

        let render = !self.no_render;
        let reparse_only = self.reparse_only();

        // Phase 1: tsv-self round-trip (pure Rust, serial — parse+format is fast).
        let mut results: Vec<FileResult> = files
            .iter()
            .map(|p| tsv_self_roundtrip(p, render, self.verbose, reparse_only))
            .collect();

        // Phase 2: canonical confirm over the sidecar (skipped by a bare `--gate`).
        if self.runs_canonical() {
            let rt = super::create_runtime();
            rt.block_on(canonical_phase(
                &mut results,
                self.canonical_all,
                render,
                self.verbose,
            ));
        }

        self.report(&results)
    }

    fn report(&self, results: &[FileResult]) -> Result<(), CliError> {
        // In `--gate` mode only the unreparseable buckets fail the run; the
        // divergent buckets are counted but non-fatal (render-model noise).
        let is_fail = |b: Bucket| {
            if self.gate {
                b.is_unreparseable()
            } else {
                b.is_finding()
            }
        };

        let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut findings: Vec<&FileResult> = Vec::new();
        for r in results {
            let b = r.bucket();
            *counts.entry(b.label()).or_default() += 1;
            if is_fail(b) {
                findings.push(r);
            }
        }
        // Most-severe findings first.
        findings.sort_by_key(|r| r.bucket().severity());

        if self.json {
            let findings_json: Vec<Value> = findings
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "path": r.display,
                        "parser": r.parser.name(),
                        "bucket": r.bucket().label(),
                    })
                })
                .collect();
            let out = serde_json::json!({
                "scanned": results.len(),
                "gate": self.gate,
                "counts": counts,
                "findings": findings_json,
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
            return if findings.is_empty() {
                Ok(())
            } else {
                Err(CliError::Failed)
            };
        }

        println!(
            "format→reparse round-trip audit — {} files\n",
            results.len()
        );
        for (label, n) in &counts {
            println!("  {n:>6}  {label}");
        }
        println!();
        if self.reparse_only() {
            // Bare `--gate`: the fast path never classified divergence, so
            // `clean` here means "output reparses", not "reparses + equivalent".
            println!(
                "(gate mode, phase 1 only: `clean` = output reparses; divergence not classified — only *_unreparseable fails)\n"
            );
        } else if self.gate {
            println!(
                "(gate mode: only *_unreparseable buckets fail; divergent counts are informational)\n"
            );
        }

        if findings.is_empty() {
            println!("✓ no round-trip findings (every formatted output reparses equivalent)");
            return Ok(());
        }

        println!("✗ {} finding(s):\n", findings.len());
        for r in &findings {
            println!(
                "  [{}] {} ({})",
                r.bucket().label(),
                r.display,
                r.parser.name()
            );
            if self.verbose
                && let Some(diff) = &r.diff
            {
                println!("{diff}");
            }
        }
        Err(CliError::Failed)
    }
}

/// Phase 1: parse the input and the formatted output with **tsv's own** parser
/// and compare them under render-equivalence.
fn tsv_self_roundtrip(path: &Path, render: bool, verbose: bool, reparse_only: bool) -> FileResult {
    let display = path.to_string_lossy().into_owned();
    let parser = ParserType::from_extension(&display);
    let mk = |tsv: TsvVerdict, diff: Option<String>| FileResult {
        display: display.clone(),
        path: path.to_path_buf(),
        parser,
        tsv,
        canonical: None,
        diff,
    };

    let Ok(source) = std::fs::read_to_string(path) else {
        return mk(TsvVerdict::FormatError, None);
    };
    let Ok(formatted) = format_source(&source, parser) else {
        return mk(TsvVerdict::FormatError, None);
    };

    if reparse_only {
        // Gate fast path: format already proved the input parses, so the only
        // question is whether the *output* reparses. No wire-JSON convert, no
        // normalization, no comparison — the divergent verdict is unused here.
        let verdict = if tsv_reparses(&formatted, parser) {
            TsvVerdict::Clean
        } else {
            TsvVerdict::Unreparseable
        };
        return mk(verdict, None);
    }

    let Some(wire_in) = tsv_parse_to_value(&source, parser) else {
        // Format parsed it, so this should not happen — treat as a parse gap.
        return mk(TsvVerdict::FormatError, None);
    };
    let Some(wire_out) = tsv_parse_to_value(&formatted, parser) else {
        return mk(TsvVerdict::Unreparseable, None);
    };

    let (equal, diff) = structurally_equivalent(wire_in, wire_out, render, verbose);
    if equal {
        mk(TsvVerdict::Clean, None)
    } else {
        mk(TsvVerdict::Divergent, diff)
    }
}

/// Whether tsv's own parser accepts `source` (parse only — no wire-JSON
/// convert). The gate fast path's reparseability test.
fn tsv_reparses(source: &str, parser: ParserType) -> bool {
    let arena = bumpalo::Bump::new();
    match parser {
        ParserType::TypeScript => tsv_ts::parse(source, &arena).is_ok(),
        ParserType::Svelte => tsv_svelte::parse(source, &arena).is_ok(),
        ParserType::Css => tsv_css::parse(source, &arena).is_ok(),
    }
}

/// Phase 2: for the selected files, reparse input and output with the canonical
/// parsers and record a [`CanVerdict`]. Fans out over the sidecar pool.
async fn canonical_phase(
    results: &mut [FileResult],
    canonical_all: bool,
    render: bool,
    verbose: bool,
) {
    // A file is checked when it's a tsv-self suspect, or unconditionally with
    // --canonical-all. A FormatError has no output to reparse.
    let jobs: Vec<(usize, PathBuf, ParserType)> = results
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r.tsv != TsvVerdict::FormatError
                && (canonical_all
                    || matches!(r.tsv, TsvVerdict::Unreparseable | TsvVerdict::Divergent))
        })
        .map(|(i, r)| (i, r.path.clone(), r.parser))
        .collect();

    if jobs.is_empty() {
        return;
    }

    let concurrency = deno::init_bulk_pool();
    let checked: Vec<(usize, CanVerdict, Option<String>)> = stream::iter(jobs)
        .map(|(i, path, parser)| async move {
            let (verdict, diff) = canonical_roundtrip(&path, parser, render, verbose).await;
            (i, verdict, diff)
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    for (i, verdict, diff) in checked {
        results[i].canonical = Some(verdict);
        if results[i].diff.is_none() {
            results[i].diff = diff;
        }
    }
}

/// Reparse `path`'s input and tsv-formatted output with the canonical parser.
async fn canonical_roundtrip(
    path: &Path,
    parser: ParserType,
    render: bool,
    verbose: bool,
) -> (CanVerdict, Option<String>) {
    // Phase 1 already proved these succeed for non-FormatError files.
    let Ok(source) = std::fs::read_to_string(path) else {
        return (CanVerdict::Clean, None);
    };
    let Ok(formatted) = format_source(&source, parser) else {
        return (CanVerdict::Clean, None);
    };

    let Ok(canon_in) = deno::parse_by_type(&source, parser).await else {
        // The canonical parser rejects the input — an invalid / error fixture,
        // not a round-trip subject.
        return (CanVerdict::RejectsInput, None);
    };
    let Ok(canon_out) = deno::parse_by_type(&formatted, parser).await else {
        return (CanVerdict::Unreparseable, None);
    };

    let (equal, diff) = structurally_equivalent(canon_in, canon_out, render, verbose);
    if equal {
        (CanVerdict::Clean, None)
    } else {
        (CanVerdict::Divergent, diff)
    }
}
