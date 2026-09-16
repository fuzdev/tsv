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
//!    - reparses but a node was dropped / duplicated / re-typed →
//!      [`TsvVerdict::NodeLoss`] (always a bug — the population census);
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
//! The eight finding buckets (`{tsv,canonical}_unreparseable`,
//! `{tsv,canonical}_node_loss`, `{tsv,canonical}_leaf_corruption`,
//! `{tsv,canonical}_divergent`) are the work-list; the four skips — `read_error`,
//! `format_error` (tsv rejects the input — a parse-gap for other gates),
//! `canonical_rejects_input` (an invalid / error fixture) and
//! `canonical_diverges_on_input` (below) — are counted, not findings.
//!
//! **Standing to grade.** The canonical phase asks whether the formatter changed the
//! document *as the canonical parser reads it*, which is only well posed while both
//! parsers are reading the same document. So it first checks canonical's node census
//! of the INPUT against tsv's and skips a mismatch as
//! [`CanVerdict::DivergesOnInput`] — the same question `RejectsInput` answers in its
//! extreme form. Without it every deliberate parser divergence reads as node loss:
//! tsv formats the program *it* read into an output both parsers then agree on, so
//! canonical's delta across the format is the divergence closing, not a dropped node.
//! The class is cataloged in `docs/conformance_svelte.md` §TypeScript Corrections;
//! the skip's measured tightness and the blind spot it concedes are in
//! `docs/audits.md` §Round-trip.
//!
//! **Node loss** is the class the structural skeleton SEES but could not gate: a
//! dropped element is one more skeleton difference, filed into the same `divergent`
//! bucket as a whitespace `Text` appearing beside a block element — render-model
//! noise every consumer holds report-only. A glued element run before a block that
//! printed only its last member shipped that way. The node-population census
//! (`node_conservation_diff` — every node by `type`, minus the wrappers and
//! separators the formatter rewrites by design, plus every word of template text)
//! is asked FIRST, ahead of the skeleton, and files `*_node_loss` — gate-fatal, and
//! the one verdict beyond reparse that the bare-gate fast path computes, since it
//! is the class that shipped.
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
//! **Comment attachment** is not shape either: `structural_skeleton` drops each node's
//! `leadingComments` / `trailingComments` (Svelte's parser attaches them, tsv's wire
//! mirrors it), so a comment that legitimately trails past a `;` reads Clean, while its
//! presence, kind and text stay pinned by the root `comments` array and the leaf check.
//!
//! ## `--gate` (the `deno task check` guard)
//!
//! `--gate` fails on the `*_unreparseable`, `*_node_loss` and `*_leaf_corruption`
//! buckets (the divergent buckets are render-model noise over `tests/fixtures`). A
//! **bare** `--gate` runs phase 1 only via the population-only fast path, which
//! classifies neither divergence nor leaf corruption (`tsv_unreparseable` and
//! `tsv_node_loss`); the leaf check rides `--gate --canonical-all` and every
//! non-gate run, the same tier as divergence. Over `tests/fixtures` that guard
//! is a **cheap tripwire**: the fixture idempotency/normalization invariants
//! (`fixtures_validate` F1/N, also in `deno task check`) already make every
//! formatted output reparse, so the bucket is ~always 0 there and a regression
//! that broke it would trip those checks too. The real yield is on **external
//! corpora** — point it at `../prettier/tests/format/*` and real repos, where it
//! surfaces corruption no fixture covers. Kept in `check` as a pure-Rust backstop,
//! not the primary detector.
//!
//! The census is asked of the wire **bytes**: the fast path emits each side's wire
//! ([`tsv_parse_to_wire_bytes`]) and counts straight out of it
//! ([`streamed_node_census`](crate::audit::properties::streamed_node_census)), so it builds no
//! `Value` — the tree is what the skeleton and the leaf check need, and this path runs
//! neither. That is the whole cost of the census, not a detail of it: reading a wire into a
//! tree costs ~11x streaming the same bytes, and dropping the tree adds a fifth of the read
//! again, while the emit and the count are ~0.1 s each over the fixture corpus. Measured on the `corpus`
//! profile, 3 runs, as reparse-only → census-over-a-`Value` → census-over-the-bytes:
//! `tests/fixtures` (10,171 files) 0.37 → 3.0 → **0.9 s** (peak RSS 12.9 → 19.0 → 13.5 MB),
//! the prettier suites 0.07 → 0.53 → **0.17 s**, the `../corpora` snapshot (6,702 files)
//! 1.1 → 17.0 → **4.1 s** (RSS 17 → 83 → 23 MB). So the population census costs ~2.4x the
//! reparse-only gate it grew out of, rather than the ~8x it cost through a tree.

use argh::FromArgs;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use futures_util::StreamExt;

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;

use crate::audit::properties::{
    ReparseCompare, compare_reparsed, node_conservation_diff, streamed_node_conservation_diff,
    tsv_parse_to_value, tsv_parse_to_wire_bytes,
};
use crate::audit::vacuity::check_graded_nonzero;
use crate::cli::CliError;
use crate::deno;

use super::profile::{is_input_invalid_fixture, resolve_seed_files_named};
use super::{ResultOrder, spawn_work_stream, task_result};

/// Audit whether every file's formatted output reparses to the same document.
///
/// Phase 1 (pure Rust) round-trips each file through tsv's own parser; phase 2
/// confirms the suspects (or every file, with `--canonical-all`) against the
/// canonical parsers via the sidecar. Defaults to `tests/fixtures` when no paths
/// are given — point it at the corpus (`../prettier/tests/format/{css,js,typescript,html}`,
/// `../corpora/collections/zzz/src`, `../corpora/collections/svelte/packages/svelte/src`, …) to generate the work-list.
///
/// A bare run reports every finding bucket and exits 0. `--gate` reports only the
/// reliable `*_unreparseable`, `*_node_loss` and `*_leaf_corruption` buckets and exits
/// 1 on any (the divergent buckets are render-model noise over `tests/fixtures`); a
/// bare `--gate` runs phase 1 only, the fast pure-Rust `deno task check` guard, which
/// classifies `tsv_unreparseable` and `tsv_node_loss`, while `--gate --canonical-all` is the thorough
/// release-cadence form that also guards `canonical_unreparseable` (tsv's own parser
/// accepting output the real parser rejects) and both leaf-corruption buckets.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "roundtrip_audit")]
#[expect(clippy::struct_excessive_bools)] // independent CLI flags
pub struct RoundtripAuditCommand {
    /// gate mode: report only the gate-fatal buckets — the
    /// {tsv,canonical}_unreparseable, {tsv,canonical}_node_loss and
    /// {tsv,canonical}_leaf_corruption buckets — and exit 1 on any. The divergent
    /// buckets (render-model noise) are still counted but non-fatal. Without it
    /// the run reports every finding and exits 0. Bare `--gate` runs phase 1 only
    /// (pure Rust, no sidecar), which classifies tsv_unreparseable and
    /// tsv_node_loss; add `--canonical-all` for the canonical and leaf-corruption
    /// guards too. This is the `deno task check` regression-guard mode.
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
    /// The file could not be read.
    ReadError,
    /// tsv's own parser rejects tsv's own formatted output.
    Unreparseable,
    /// Output reparses but a conserved node (an element, a block, a statement, a word of
    /// template text) was dropped, duplicated or re-typed. The precise subset of Divergent —
    /// gate-fatal, and the one verdict the bare-gate fast path computes beyond reparse.
    NodeLoss,
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
    /// The canonical parser reads the *input* to a different node population than tsv does —
    /// a settled parser divergence, so canonical has no census of the document tsv's
    /// formatter was actually given. A skip, like [`Self::RejectsInput`], which is the same
    /// standing-to-grade question in its extreme form; the argument is in the module doc.
    DivergesOnInput,
    /// The canonical parser throws on tsv's *output* (invalid per the real language).
    Unreparseable,
    /// A conserved node was dropped, duplicated or re-typed under the canonical parser (the
    /// drop-in-oracle confirmation of a node loss).
    NodeLoss,
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
    ReadError,
    CanonicalRejectsInput,
    CanonicalDivergesOnInput,
    CanonicalUnreparseable,
    TsvUnreparseable,
    CanonicalNodeLoss,
    TsvNodeLoss,
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
                | Self::CanonicalNodeLoss
                | Self::TsvNodeLoss
                | Self::CanonicalLeafCorruption
                | Self::TsvLeafCorruption
                | Self::CanonicalDivergent
                | Self::TsvDivergent
        )
    }

    /// The reliable half — output the parser rejects, a dropped / duplicated node, and
    /// leaf-value corruption (a still-parses value change the skeleton is blind to). These are
    /// the buckets `--gate` mode fails on; the divergent buckets are the noisy render-model
    /// half, reported but non-fatal there.
    fn is_gate_fatal(self) -> bool {
        matches!(
            self,
            Self::CanonicalUnreparseable
                | Self::TsvUnreparseable
                | Self::CanonicalNodeLoss
                | Self::TsvNodeLoss
                | Self::CanonicalLeafCorruption
                | Self::TsvLeafCorruption
        )
    }

    fn label(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::FormatError => "format_error",
            Self::ReadError => "read_error",
            Self::CanonicalRejectsInput => "canonical_rejects_input",
            Self::CanonicalDivergesOnInput => "canonical_diverges_on_input",
            Self::CanonicalUnreparseable => "canonical_unreparseable",
            Self::TsvUnreparseable => "tsv_unreparseable",
            Self::CanonicalNodeLoss => "canonical_node_loss",
            Self::TsvNodeLoss => "tsv_node_loss",
            Self::CanonicalLeafCorruption => "canonical_leaf_corruption",
            Self::TsvLeafCorruption => "tsv_leaf_corruption",
            Self::CanonicalDivergent => "canonical_divergent",
            Self::TsvDivergent => "tsv_divergent",
        }
    }

    /// A bucket the round-trip property was actually EVALUATED on. The four
    /// negations are skips — the file couldn't be read, tsv couldn't format it, the
    /// canonical oracle rejected it, or the canonical oracle read it to a different
    /// document than tsv did — so none carries a verdict. The vacuity floor counts
    /// these: a run where every file skipped reports "no round-trip findings" and
    /// exits 0, which is what a clean run reports too.
    fn is_graded(self) -> bool {
        !matches!(
            self,
            Self::ReadError
                | Self::FormatError
                | Self::CanonicalRejectsInput
                | Self::CanonicalDivergesOnInput
        )
    }

    /// A stable severity rank for sorting findings worst-first.
    fn severity(self) -> u8 {
        match self {
            Self::CanonicalUnreparseable => 0,
            Self::TsvUnreparseable => 1,
            Self::CanonicalNodeLoss => 2,
            Self::TsvNodeLoss => 3,
            Self::CanonicalLeafCorruption => 4,
            Self::TsvLeafCorruption => 5,
            Self::CanonicalDivergent => 6,
            Self::TsvDivergent => 7,
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
        // A dropped / duplicated node — like unreparseable, a tsv-self loss is never masked by a
        // canonical-Clean: the formatter changed the population tsv's own parser reads.
        if self.canonical == Some(CanVerdict::NodeLoss) {
            return Bucket::CanonicalNodeLoss;
        }
        if self.tsv == TsvVerdict::NodeLoss {
            return Bucket::TsvNodeLoss;
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
        if self.canonical == Some(CanVerdict::DivergesOnInput) {
            return Bucket::CanonicalDivergesOnInput;
        }
        if self.tsv == TsvVerdict::ReadError {
            return Bucket::ReadError;
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

    /// The phase-1 fast path — check that the output *reparses* and that its node
    /// population is conserved, skipping the render normalization, the skeleton
    /// compare and the leaf-value check. Sound **exactly when** the canonical phase won't run
    /// (`!runs_canonical`): then neither the divergent nor the leaf-corruption
    /// verdict is consumed (a bare `--gate` fails on `tsv_unreparseable` and
    /// `tsv_node_loss` — the two verdicts this fast path produces), so computing
    /// them is dead weight. Deriving it from `runs_canonical` keeps that invariant
    /// in one place — the fast path can never outlive its safety condition. Leaf
    /// conservation therefore rides the same tier as divergence: caught by
    /// `--gate --canonical-all` and non-gate runs, not by the bare phase-1 `--gate`.
    /// Node conservation does NOT: it is the class that shipped (a glued element
    /// run before a block printed only its last member), so the cheap standing
    /// gate carries it — and cheaply, since the census reads the wire bytes and the
    /// skipped checks are exactly the ones that wanted a `Value` (see the module doc).
    fn population_only(&self) -> bool {
        !self.runs_canonical()
    }

    pub(crate) fn run(self) -> Result<(), CliError> {
        // Intentionally-invalid fixture inputs aren't round-trip subjects. A scan with
        // nothing in it must not read as a pass: `--gate` reports "no round-trip
        // findings" and exits 0 on an empty set, so a typo'd path or a corpus that
        // silently stopped resolving would look identical to a clean run. Resolution
        // fails loud on one, naming the subject rather than the raw walk.
        let files =
            resolve_seed_files_named(&self.paths, self.limit, "round-trip subjects", |p| {
                !is_input_invalid_fixture(p)
            })?;

        let render = !self.no_render;
        let population_only = self.population_only();

        // Phase 1: tsv-self round-trip (pure Rust, serial — parse+format is fast).
        let mut results: Vec<FileResult> = files
            .iter()
            .map(|p| tsv_self_roundtrip(p, render, self.verbose, population_only))
            .collect();

        // Phase 2: canonical confirm over the sidecar (skipped by a bare `--gate`).
        if self.runs_canonical() {
            let rt = super::create_runtime();
            rt.block_on(canonical_phase(
                &mut results,
                self.canonical_all,
                render,
                self.verbose,
            ))?;
        }

        let graded = results.iter().filter(|r| r.bucket().is_graded()).count();
        self.report(&results)?;
        // The floor UNDER the non-empty resolution: resolution proves files were found,
        // not that any was graded, and this audit has no `default_paths` pin above it at
        // any scope.
        check_graded_nonzero(graded, "round-trip subjects graded")
    }

    fn report(&self, results: &[FileResult]) -> Result<(), CliError> {
        // A bare run reports every finding bucket and exits 0. `--gate` reports only the
        // gate-fatal buckets (the unreparseable + leaf-corruption half) and fails on any; the
        // divergent buckets are counted but non-fatal there (render-model noise).
        let reported = |b: Bucket| {
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
            if reported(b) {
                findings.push(r);
            }
        }
        // Most-severe findings first.
        findings.sort_by_key(|r| r.bucket().severity());
        let verdict = if self.gate && !findings.is_empty() {
            Err(CliError::Failed)
        } else {
            Ok(())
        };

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
            super::print_json_pretty(&out);
            return verdict;
        }

        println!(
            "format→reparse round-trip audit — {} files\n",
            results.len()
        );
        for (label, n) in &counts {
            println!("  {n:>6}  {label}");
        }
        println!();
        if self.population_only() {
            // Bare `--gate`: the fast path never classified divergence, so `clean` here
            // means "output reparses with its node population conserved", not "reparses
            // + equivalent".
            println!(
                "(gate mode, phase 1 only: `clean` = output reparses + node population conserved; divergence not classified — only *_unreparseable + *_node_loss fail)\n"
            );
        } else if self.gate {
            println!(
                "(gate mode: only *_unreparseable + *_node_loss + *_leaf_corruption buckets fail; divergent counts are informational)\n"
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
        if !self.gate {
            println!(
                "\n(report-only: exits 0 — `--gate` fails on *_unreparseable + *_node_loss + *_leaf_corruption)"
            );
        }
        verdict
    }
}

/// Phase 1: parse the input and the formatted output with **tsv's own** parser
/// and compare them under render-equivalence.
fn tsv_self_roundtrip(
    path: &Path,
    render: bool,
    verbose: bool,
    population_only: bool,
) -> FileResult {
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
        return mk(TsvVerdict::ReadError, None);
    };
    let Ok(formatted) = format_source(&source, parser) else {
        return mk(TsvVerdict::FormatError, None);
    };

    if population_only {
        // Gate fast path: the output reparses, so the one remaining gate-fatal question
        // phase 1 alone can answer is whether its node population is conserved. No
        // normalization, no skeleton, no leaves — the divergent and leaf verdicts are
        // unused here, and neither is a wire TREE: the census reads the bytes.
        let Some(wire_in) = tsv_parse_to_wire_bytes(&source, parser) else {
            // Format parsed it, so this should not happen — treat as a parse gap.
            return mk(TsvVerdict::FormatError, None);
        };
        let Some(wire_out) = tsv_parse_to_wire_bytes(&formatted, parser) else {
            return mk(TsvVerdict::Unreparseable, None);
        };
        return match streamed_node_conservation_diff(&wire_in, &wire_out) {
            Some(detail) => mk(TsvVerdict::NodeLoss, verbose.then_some(detail)),
            None => mk(TsvVerdict::Clean, None),
        };
    }

    let Some(wire_in) = tsv_parse_to_value(&source, parser) else {
        // Format parsed it, so this should not happen — treat as a parse gap.
        return mk(TsvVerdict::FormatError, None);
    };
    let Some(wire_out) = tsv_parse_to_value(&formatted, parser) else {
        return mk(TsvVerdict::Unreparseable, None);
    };

    let (verdict, detail) = verdict_of(
        compare_reparsed(wire_in, wire_out, render, verbose),
        verbose,
    );
    mk(verdict, detail)
}

/// The tsv-self verdict of a reparse compare, with its detail under `--verbose`: a divergence's
/// AST diff (already gated by the compare) or a conservation delta (gated here).
fn verdict_of(compare: ReparseCompare, verbose: bool) -> (TsvVerdict, Option<String>) {
    match compare {
        ReparseCompare::Equal => (TsvVerdict::Clean, None),
        ReparseCompare::NodeLoss(detail) => (TsvVerdict::NodeLoss, verbose.then_some(detail)),
        ReparseCompare::Divergent(diff) => (TsvVerdict::Divergent, diff),
        ReparseCompare::LeafCorruption(detail) => {
            (TsvVerdict::LeafCorruption, verbose.then_some(detail))
        }
    }
}

/// Phase 2: for the selected files, reparse input and output with the canonical
/// parsers and record a [`CanVerdict`]. Fans out over the sidecar pool.
async fn canonical_phase(
    results: &mut [FileResult],
    canonical_all: bool,
    render: bool,
    verbose: bool,
) -> Result<(), CliError> {
    // A file is checked when it's a tsv-self suspect, or unconditionally with
    // --canonical-all. A read or format error has no output to reparse.
    let jobs: Vec<(usize, PathBuf, ParserType)> = results
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            !matches!(r.tsv, TsvVerdict::ReadError | TsvVerdict::FormatError)
                && (canonical_all
                    || matches!(
                        r.tsv,
                        TsvVerdict::Unreparseable
                            | TsvVerdict::NodeLoss
                            | TsvVerdict::LeafCorruption
                            | TsvVerdict::Divergent
                    ))
        })
        .map(|(i, r)| (i, r.path.clone(), r.parser))
        .collect();

    if jobs.is_empty() {
        return Ok(());
    }

    // Each check runs on its own spawned task, so the CPU-bound format and wire compare spread
    // across the runtime's workers while the canonical parses share the sidecar pool.
    let mut checked = spawn_work_stream(
        jobs,
        ResultOrder::Completion,
        move |(i, path, parser)| async move {
            let (verdict, diff) = canonical_roundtrip(&path, parser, render, verbose).await;
            (i, verdict, diff)
        },
    );
    while let Some(joined) = checked.next().await {
        let (i, verdict, diff) = task_result(joined, "round-trip canonical check")?;
        results[i].canonical = Some(verdict);
        if results[i].diff.is_none() {
            results[i].diff = diff;
        }
    }
    Ok(())
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

    // Standing to grade (module doc): canonical must read the INPUT to the same node
    // population tsv does, or the delta it reports across the format is the two parsers'
    // disagreement closing rather than a node the formatter lost. Three facts live here:
    // it is asked AFTER `Unreparseable`, which is a drop-in violation however canonical read
    // the input; it uses the same census `compare_reparsed` does, so the membership rule has
    // one spelling; and the `None` arm is unreachable — `format_source` above parses through
    // the same three functions, the TypeScript one at the same unnamed goal — so it falls
    // through to the compare rather than inventing a verdict for a state that cannot occur.
    if let Some(tsv_in) = tsv_parse_to_value(&source, parser)
        && node_conservation_diff(&tsv_in, &canon_in).is_some()
    {
        return (CanVerdict::DivergesOnInput, None);
    }

    // The same compare as phase 1, under the canonical parser — the drop-in-oracle
    // confirmation of each verdict.
    match compare_reparsed(canon_in, canon_out, render, verbose) {
        ReparseCompare::Equal => (CanVerdict::Clean, None),
        ReparseCompare::NodeLoss(detail) => (CanVerdict::NodeLoss, verbose.then_some(detail)),
        ReparseCompare::Divergent(diff) => (CanVerdict::Divergent, diff),
        ReparseCompare::LeafCorruption(detail) => {
            (CanVerdict::LeafCorruption, verbose.then_some(detail))
        }
    }
}
