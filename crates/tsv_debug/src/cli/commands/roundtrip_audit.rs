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
//! The six finding buckets (`{tsv,canonical}_unreparseable`,
//! `{tsv,canonical}_leaf_corruption`, `{tsv,canonical}_divergent`) are the
//! work-list; `format_error` (tsv rejects the input — a parse-gap for other gates)
//! and `canonical_rejects_input` (an invalid / error fixture) are counted and
//! skipped, not findings.
//!
//! **Leaf-value corruption** is the class the structural skeleton is blind to:
//! `structural_skeleton` erases every scalar leaf, so output that reparses to an
//! **equal shape** but with a changed decode-invariant value (a mis-decoded string,
//! a miscanonicalized number, a mangled multi-line comment) reads as Clean. The
//! [`leaf_conservation_diff`](crate::audit::properties::leaf_conservation_diff)
//! check compares the multiset of conserved leaves (values / names / cooked chunks
//! / regex body+flags, never `raw`) input-vs-output and, **when the skeleton is
//! otherwise equal**, files a `*_leaf_corruption` finding — gate-fatal like
//! `*_unreparseable`. It is a refinement of Clean, not a competitor to the
//! divergent bucket: a shape change stays a Divergence (the skeleton owns it), so a
//! corruption that also changes shape (an ASI merge, a comment swallow) is reported
//! as divergent, and leaf-corruption is reserved for the genuinely skeleton-blind
//! same-shape value change.
//!
//! ## `--gate` (the `deno task check` guard)
//!
//! `--gate` fails on the `*_unreparseable` and `*_leaf_corruption` buckets (the
//! divergent buckets are render-model noise over `tests/fixtures`). A **bare**
//! `--gate` runs phase 1 only via the reparse-only fast path, which classifies
//! neither divergence nor leaf corruption (only `tsv_unreparseable`); the leaf
//! check rides `--gate --canonical-all` and every non-gate run, the same tier as
//! divergence. Over `tests/fixtures` that guard
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

use crate::audit::properties::{
    leaf_conservation_diff, structurally_equivalent, tsv_parse_to_value,
};
use crate::cli::CliError;
use crate::deno;

use super::profile::{is_input_invalid_fixture, resolve_files};

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
    /// Output reparses with an **equal skeleton** but a decode-invariant leaf value changed
    /// (a mis-decoded string, a miscanonicalized number, a mangled comment). The skeleton-blind
    /// class — a refinement of Clean, distinct from Divergent (a shape change). Gate-fatal.
    LeafCorruption,
    /// Output reparses but the AST diverges (a shape change — the skeleton catches it).
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
    /// A decode-invariant leaf value changed under the canonical parser with the skeleton
    /// otherwise equal (the drop-in-oracle confirmation of a skeleton-blind leaf corruption).
    LeafCorruption,
    /// Output reparses but the AST diverges (a shape change).
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
    CanonicalLeafCorruption,
    TsvLeafCorruption,
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
                | Self::CanonicalLeafCorruption
                | Self::TsvLeafCorruption
                | Self::CanonicalDivergent
                | Self::TsvDivergent
        )
    }

    /// The reliable half — output the parser rejects, plus leaf-value corruption (a
    /// still-parses value change the skeleton is blind to). These are the buckets `--gate`
    /// mode fails on; the divergent buckets are the noisy render-model half, reported but
    /// non-fatal there.
    fn is_gate_fatal(self) -> bool {
        matches!(
            self,
            Self::CanonicalUnreparseable
                | Self::TsvUnreparseable
                | Self::CanonicalLeafCorruption
                | Self::TsvLeafCorruption
        )
    }

    fn label(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::FormatError => "format_error",
            Self::CanonicalRejectsInput => "canonical_rejects_input",
            Self::CanonicalUnreparseable => "canonical_unreparseable",
            Self::TsvUnreparseable => "tsv_unreparseable",
            Self::CanonicalLeafCorruption => "canonical_leaf_corruption",
            Self::TsvLeafCorruption => "tsv_leaf_corruption",
            Self::CanonicalDivergent => "canonical_divergent",
            Self::TsvDivergent => "tsv_divergent",
        }
    }

    /// A stable severity rank for sorting findings worst-first.
    fn severity(self) -> u8 {
        match self {
            Self::CanonicalUnreparseable => 0,
            Self::TsvUnreparseable => 1,
            Self::CanonicalLeafCorruption => 2,
            Self::TsvLeafCorruption => 3,
            Self::CanonicalDivergent => 4,
            Self::TsvDivergent => 5,
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
        // Leaf-value corruption — a still-parses value change. Like unreparseable, a tsv-self
        // leaf change is never masked by a canonical-Clean (tsv's own formatter changed a
        // value tsv's own parser reads), so both leaf verdicts outrank the divergent ones.
        if self.canonical == Some(CanVerdict::LeafCorruption) {
            return Bucket::CanonicalLeafCorruption;
        }
        if self.tsv == TsvVerdict::LeafCorruption {
            return Bucket::TsvLeafCorruption;
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
    /// the wire-JSON convert + skeleton compare + leaf-value check. Sound
    /// **exactly when** the canonical phase won't run (`!runs_canonical`): then
    /// neither the divergent nor the leaf-corruption verdict is consumed (a bare
    /// `--gate` fails on `tsv_unreparseable` alone — the only verdict this fast
    /// path produces), so computing them is dead weight. Deriving it from
    /// `runs_canonical` keeps that invariant in one place — the fast path can
    /// never outlive its safety condition. Leaf conservation therefore rides the
    /// same tier as divergence: caught by `--gate --canonical-all` and non-gate
    /// runs, not by the bare phase-1 `--gate`.
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
        files.retain(|p| !is_input_invalid_fixture(p));
        // A scan with nothing in it must not read as a pass: `--gate` reports
        // "no round-trip findings" and exits 0 on an empty set, so a typo'd path or
        // a corpus that silently stopped resolving would look identical to a clean
        // run. Fail loud instead, matching `gap_audit`/`blank_audit`/`fuzz`/`render_audit`.
        if files.is_empty() {
            eprintln!("Error: no round-trip subjects found (searched {paths:?})");
            return Err(CliError::Failed);
        }
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
        // In `--gate` mode only the gate-fatal buckets fail the run (the unreparseable +
        // leaf-corruption half); the divergent buckets are counted but non-fatal (render-model
        // noise).
        let is_fail = |b: Bucket| {
            if self.gate {
                b.is_gate_fatal()
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
                "(gate mode: only *_unreparseable + *_leaf_corruption buckets fail; divergent counts are informational)\n"
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

    // Leaf-value conservation is the skeleton-BLIND class: a *same-shape* output whose decoded
    // leaf changed (a mis-decoded string, a miscanonicalized number, a mangled comment). So it
    // refines the skeleton-Clean verdict rather than competing with Divergent — a shape change
    // is a Divergence (which the skeleton owns), and only a shape-EQUAL output with a changed
    // leaf is a LeafCorruption. Computed before the move-consuming structural compare. (Not run
    // under a bare `--gate`, which takes the `reparse_only` fast path above.)
    let leaf_diff = leaf_conservation_diff(&wire_in, &wire_out);
    let (equal, diff) = structurally_equivalent(wire_in, wire_out, render, verbose);
    if !equal {
        return mk(TsvVerdict::Divergent, diff);
    }
    if let Some(detail) = leaf_diff {
        return mk(TsvVerdict::LeafCorruption, verbose.then_some(detail));
    }
    mk(TsvVerdict::Clean, None)
}

/// Whether tsv's own parser accepts `source` (parse only — no wire-JSON
/// convert). The gate fast path's reparseability test.
fn tsv_reparses(source: &str, parser: ParserType) -> bool {
    let arena = bumpalo::Bump::new();
    let mut interner = tsv_lang::Interner::new();
    match parser {
        ParserType::TypeScript => tsv_ts::parse(source, &arena, &mut interner).is_ok(),
        ParserType::Svelte => tsv_svelte::parse(source, &arena, &mut interner).is_ok(),
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
                    || matches!(
                        r.tsv,
                        TsvVerdict::Unreparseable
                            | TsvVerdict::LeafCorruption
                            | TsvVerdict::Divergent
                    ))
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

    // Leaf-value conservation under the canonical parser — the skeleton-blind class (a
    // shape-equal output with a changed leaf), the drop-in-oracle confirmation. A shape change
    // is a Divergence; only a shape-equal leaf change is a LeafCorruption.
    let leaf_diff = leaf_conservation_diff(&canon_in, &canon_out);
    let (equal, diff) = structurally_equivalent(canon_in, canon_out, render, verbose);
    if !equal {
        return (CanVerdict::Divergent, diff);
    }
    if let Some(detail) = leaf_diff {
        return (CanVerdict::LeafCorruption, verbose.then_some(detail));
    }
    (CanVerdict::Clean, None)
}
