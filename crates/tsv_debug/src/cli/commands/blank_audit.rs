//! Blank-line injection audit — the mechanized form of hunting the blank-line handling class.
//!
//! ## Why this exists
//!
//! A recurring bug family: **a printer that reflows a list, a pattern, or a block mishandles a
//! blank line an author left in a gap** — it fails to collapse a 2+ blank run to the one prettier
//! keeps, or it settles on a *different* output on the second pass (a non-idempotent fixed point),
//! or the reflow knocks out a nearby comment. The specifier-list and array-pattern blank-line bugs
//! are the named instances.
//!
//! Nothing else probes it. `fuzz`'s byte mutation essentially never synthesizes a *blank line* in
//! a gap (its inserts are single interesting bytes / tokens); `gap_audit` injects comments, not
//! blanks; and the fixture suite only ever formats each file **as authored**, so a gap no fixture
//! puts a blank in is a gap never checked. This audit closes that hole the same way `gap_audit`
//! closes the dropped-comment one: for each seed file it injects a **blank line** into every
//! candidate gap, one at a time, formats, and grades a fixed, policy-free set of invariants on the
//! result.
//!
//! ## The invariants (per injected blank)
//!
//! Six, keyed by the [`site_shape`] of the injection offset:
//!
//! 1. **no panic** — the formatter must never crash on a blank in a gap (production WASM is
//!    `panic = "abort"`, so a panic is a DoS). Caught under [`catch_unwind`](std::panic::catch_unwind);
//!    NEVER pinnable, like `gap_audit`'s `PANIC`.
//! 2. **F1 idempotency** — pass 1 may keep or drop the blank, but pass 2 must be a fixed point
//!    (`format(format(injected)) == format(injected)`). The specifier-list / array-pattern class.
//! 3. **structural reparse** — `format(injected)` must reparse to the same document skeleton
//!    (`Unreparseable` gates; `StructuralDivergence` is **report-only**, see below).
//! 4. **leaf conservation** — no decode-invariant leaf value may change (`LeafCorruption`).
//! 5. **ledger-clean** — the injected blank must not make the format DROP or DOUBLE-PRINT a
//!    comment the author already had (`Dropped` / `DoublePrinted`) — the blank-triggered
//!    comment-drop class, read off the print-once ledger.
//! 6. **blank-run ≤ 1** — the formatted output must never hold a run of ≥2 consecutive blank
//!    lines, *except* inside a template-literal quasi, a `<pre>` / `<textarea>`, or a
//!    format-ignore region (`BlankRun`).
//!
//! Invariants 1–4 are the shared [`f1_check`]; 5 is the ledger; 6 is a region-scoped output scan.
//!
//! **`STRUCTURAL-DIVERGENCE` is held report-only** (fuzz-soft parity — fuzz's `structural_divergence`
//! is its soft, canonical-confirmation-wanting bucket): a blank-induced structural change over
//! Svelte is render-model noisy, so it is REPORTED but never gated. Every OTHER policy kind is
//! pinned (this is a ratchet over a live bug family, born red); only `PANIC` always fails. The
//! carve-out is a filter on the graded key set ([`is_graded`] / [`snapshot_keys`]), **not**
//! [`is_pinnable`] — a report-only kind must be *absent* from the ratchet, not *failing* it.
//!
//! ## The absorb pin — the blank-DROP class the invariants deliberately don't grade
//!
//! Invariant 2 lets pass 1 keep OR drop the injected blank (the invariants are policy-free), and
//! the ABSORB fast path below grades every clean drop as success — so a formatter that silently
//! EATS a blank at some gap passes all six invariants forever: the dropped-blank output is its
//! own fixed point, so F1, fuzz, and round-trip are structurally blind too, and only a prettier
//! compare on the authored shape can see it (the #759 class: an authored blank before a
//! last-child declaration, deleted). The absorb pin closes the *silent* half of that hole: every
//! **node-edge class** ([`node_edge_key_with_map`] — the innermost AST node holding the offset
//! plus the child-role edge the gap sits in) whose injected blank is absorbed is pinned in a
//! SECOND snapshot (`blank_absorb_known.txt`), a **behavior pin, not a bug list**
//! (`width_audit`'s stance — most absorption is sanctioned; prettier collapses a blank in an
//! expression or an argument list too). The key is deliberately the coarse node-edge class, not
//! the fine [`site_shape`]: ~81% of injections absorb, so token granularity pins the fixture
//! tree's whole token-adjacency vocabulary (~5.7k shapes, measured) and mints new ones on
//! ordinary fixture PRs — the exact churn the snapshot header warns turns gates off — while a
//! node-edge class ≈ one emitter decision, the grain a triage verdict actually covers. One
//! absorption is exempt from the pin: an injection beside an already-authored blank
//! ([`gap_holds_blank`]) reproduces the pristine output via the sanctioned 2+→1 run collapse,
//! not a drop, and recording it would pin classes where blanks otherwise survive. The gate
//! fails on a NEW absorbing class — the formatter newly eats a blank at a kind of gap the pin
//! has never seen, which must be triaged against prettier rather than land silently — and on a
//! stale one. Graded only on the full default corpus (absorption is normal behavior, so
//! off-corpus absorb classes are not news the way findings are).
//!
//! ## Design
//!
//! Pure Rust, no sidecar, no new deps — the [`fuzz`](super::fuzz) / [`gap_audit`](super::gap_audit)
//! direction. Deliberately **targeted, not random**: a blank line is a specific, structurally
//! meaningful mutation the fuzzer's byte ops don't reach.
//!
//! **Sites** come from [`code_regions`] (the spans the AST says are JS) minus three classes
//! [`injection_sites`] and [`string_and_template_spans`] filter for free: a **word interior** (the
//! blank splits a token → parse rejects), a **comment interior** (mutilates the author's comment),
//! and — the class unique to blanks — a **string / template interior**, where tsv's permissive
//! lexer accepts a raw newline as string content rather than rejecting it, so the blank becomes
//! content and reads as a false finding. See [`string_and_template_spans`] for why that third one
//! isn't covered by `Formatted::Rejected` the way the first is.
//!
//! **The single payload is a blank line** (`"\n\n"` — two newlines). Injected between two tokens on
//! one line it forces exactly one empty line into the gap; injected next to an existing newline it
//! forms a longer run — which is precisely the input the ≤1-blank-run invariant must collapse. One
//! payload, not five (a comment's ownership paths don't apply): a blank line has one meaning.
//!
//! ## Scope — what a green run does NOT prove
//!
//! - **CSS is deferred.** A `.css` seed is skipped outright, and a `.svelte` file's `<style>` is
//!   unprobed ([`code_regions`] doesn't name it) — CSS's whole-file region is the most exposed to
//!   the string-interior class, and its blank-line behavior is a separate follow-up.
//! - **Only format fixed points are injected into.** A seed that isn't idempotent, doesn't
//!   reparse, or already violates a blank-run AS AUTHORED is reported once and skipped — injecting
//!   into it would report the base problem at every site. Over `tests/fixtures` that skips the
//!   variant / unformatted / prettier-output fixture files (which are not tsv fixed points by
//!   design); the real yield is external corpora, where every file should be a fixed point.
//! - **A format-ignore-bearing file is exempt from invariant 6** (whole-file), since locating the
//!   verbatim ignore range from the output alone is fragile; the other five still run.
//!
//! ## Structure
//!
//! Thin orchestration over the [`audit`](crate::audit) substrate: site enumeration in
//! [`audit::sites`](crate::audit::sites), the panic-free property core ([`f1_check`]) and the
//! ledger format in [`audit::properties`](crate::audit::properties), the snapshot ratchet in
//! [`audit::ratchet`](crate::audit::ratchet), and the reporting envelope in
//! [`audit::report`](crate::audit::report). This module owns the command, the per-file inject
//! loop, the blank-run scan, and the gate/exit decision.

use argh::FromArgs;
use std::collections::{BTreeMap, BTreeSet};
use std::panic::AssertUnwindSafe;

use serde_json::Value;

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;
use tsv_lang::comment_ledger::{self, CommentFindingKind};

use crate::audit::examples::{ExampleOrd, ExampleSet};
use crate::audit::node_edge::node_edge_key_with_map;
use crate::audit::parallel::{ArmedRun, run_pool};
use crate::audit::properties::{
    F1Outcome, Formatted, Pristine, Utf16ToByte, f1_check, leaf_conservation_diff, ledger_format,
    pristine_format, structurally_equivalent, tsv_parse_to_value,
};
use crate::audit::ratchet::{
    GateDiff, Ratchet, SnapshotKey, print_ratchet_skipped, refuse_narrowed_update,
    report_unpinned_panics,
};
use crate::audit::report::{
    self, BlankDetail, Detail, Finding, ReportExample, RunSummary, Severity,
};
use crate::audit::sites::{
    code_regions, injection_sites, site_shape, snippet, source_has_ignore_directive,
    string_and_template_spans,
};
use crate::audit::tally::CappedPaths;
use crate::cli::CliError;

use super::profile::{is_input_invalid_fixture, resolve_seed_files};

/// Inject a blank line into every gap and assert format stays a well-behaved fixed point.
///
/// For each seed file, injects a blank line at each candidate byte offset (one at a time),
/// formats, and reports every injection that panics, breaks idempotency, fails to reparse,
/// corrupts a leaf, drops/double-prints a comment, or emits a 2+ blank run — plus the absorb
/// pin over the silently-eaten blanks (the drop class the invariants don't grade). Pure Rust —
/// no Deno. Defaults to `tests/fixtures`; the real yield is external corpora. Exits 1 on a new
/// / stale / panic finding shape (a ratchet, like `gap_audit`) and on a new / stale absorb
/// class.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "blank_audit")]
#[allow(clippy::struct_excessive_bools)] // independent CLI flags
pub struct BlankAuditCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// print the full per-shape report (plus the per-class absorb rows, each with its
    /// reproducer) even when the gates hold. A passing gate is summary-only by default — the
    /// shapes it already knows about are noise in `deno task check`. Any run with something to
    /// act on reports regardless
    #[argh(switch)]
    report: bool,

    /// worker threads (default: available parallelism). Each file's whole inject loop stays on
    /// one thread — the ledger is thread-local
    #[argh(option)]
    jobs: Option<usize>,

    /// cap the number of seed files (0 = unlimited)
    #[argh(option, default = "0")]
    limit: usize,

    /// rewrite BOTH committed snapshots from this run (the bug shapes and the absorb pin).
    /// Only valid on a FULL default run — each snapshot describes the blank payload over
    /// `tests/fixtures` and nothing else, so any narrowing flag is refused rather than
    /// silently pinning a partial set
    #[argh(switch)]
    update: bool,

    /// seed file paths / directories (default: tests/fixtures)
    #[argh(positional)]
    paths: Vec<String>,
}

/// The one payload — a blank line: two newlines. Injected between two same-line tokens it forms
/// exactly one empty line; adjacent to an existing newline it forms a longer run, the input the
/// ≤1-blank-run invariant must collapse.
const PAYLOAD: &str = "\n\n";

/// Why an injected blank is a finding. `Panic` is the one absolute break (never pinnable); the
/// rest are policy invariants the ratchet grades.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum BlankKind {
    /// The formatter crashed on the injected blank — a comment in a gap must never do this.
    /// NEVER pinnable.
    Panic,
    /// `format(format(x)) != format(x)` — the specifier-list / array-pattern class.
    NonIdempotent,
    /// `format`'s output does not reparse.
    Unreparseable,
    /// Output reparses but the document structure changed.
    StructuralDivergence,
    /// A decode-invariant leaf value changed (skeleton-blind corruption).
    LeafCorruption,
    /// `format` errored on a parsed input (should be impossible — it re-parses internally).
    FormatError,
    /// The injected blank made the format DROP a comment the author already had.
    Dropped,
    /// … or DOUBLE-PRINT one.
    DoublePrinted,
    /// The formatted output holds a run of ≥2 consecutive blank lines outside a verbatim region.
    BlankRun,
}

impl BlankKind {
    fn label(self) -> &'static str {
        match self {
            Self::Panic => "PANIC",
            Self::NonIdempotent => "NON-IDEMPOTENT",
            Self::Unreparseable => "UNREPARSEABLE",
            Self::StructuralDivergence => "STRUCTURAL-DIVERGENCE",
            Self::LeafCorruption => "LEAF-CORRUPTION",
            Self::FormatError => "FORMAT-ERROR",
            Self::Dropped => "DROPPED",
            Self::DoublePrinted => "DOUBLE-PRINTED",
            Self::BlankRun => "BLANK-RUN",
        }
    }

    fn from_label(s: &str) -> Option<Self> {
        [
            Self::Panic,
            Self::NonIdempotent,
            Self::Unreparseable,
            Self::StructuralDivergence,
            Self::LeafCorruption,
            Self::FormatError,
            Self::Dropped,
            Self::DoublePrinted,
            Self::BlankRun,
        ]
        .into_iter()
        .find(|k| k.label() == s)
    }
}

/// Whether a kind is part of the RATCHET-GRADED set at all — the report-only carve-out.
///
/// `STRUCTURAL-DIVERGENCE` is held **report-only** (fuzz-soft parity: fuzz's `structural_divergence`
/// is its soft, non-fatal bucket that wants canonical confirmation). It is neither pinned nor
/// gate-failing — achieved by FILTERING its `(kind, shape)` keys out of the set the [`Ratchet`]
/// ever sees ([`snapshot_keys`]), so the grade produces no new / stale / unpinnable for it. This is
/// deliberately **not** [`is_pinnable`]: making struct-div un-pinnable would make it FAIL the gate
/// like a panic (the opposite of report-only). Two orthogonal filters — `is_graded` decides what
/// the ratchet sees, `is_pinnable` decides, within that, what may be written vs always-fails.
fn is_graded(kind: BlankKind) -> bool {
    kind != BlankKind::StructuralDivergence
}

/// Whether a graded shape may be **pinned** into the snapshot — everything the ratchet grades but a
/// [`BlankKind::Panic`].
///
/// This is a deliberate divergence from `fuzz` / `roundtrip_audit`, where non-idempotency is an
/// absolute (never-pinnable) gate: here it IS pinned. The audit is a **ratchet over a live bug
/// family** born RED — its baseline is a snapshot of known bugs whose shrinking is the goal, so
/// day-one findings must be pinnable or the gate would hard-block `deno task check` on landing.
/// Only a crash stays absolute. (`STRUCTURAL-DIVERGENCE` is the one report-only carve-out, handled a
/// layer up by [`is_graded`] — it never reaches this predicate.)
fn is_pinnable(kind: BlankKind) -> bool {
    kind != BlankKind::Panic
}

/// How many of `shapes` crash the formatter — the shapes [`is_pinnable`] keeps out of the
/// snapshot, accounted separately on every exit path.
fn count_panics(shapes: &BTreeMap<(BlankKind, String), ShapeAgg>) -> usize {
    shapes
        .keys()
        .filter(|(k, _)| *k == BlankKind::Panic)
        .count()
}

/// How many of `shapes` are the report-only `STRUCTURAL-DIVERGENCE` class — held soft, excluded
/// from the ratchet ([`is_graded`]).
fn count_soft(shapes: &BTreeMap<(BlankKind, String), ShapeAgg>) -> usize {
    shapes.keys().filter(|(k, _)| !is_graded(*k)).count()
}

/// The command that re-pins the snapshot — quoted by the ratchet's read-failure message.
const REPIN_HINT: &str = "deno task blanks:audit:update";

/// The `#`-comment header the snapshot file opens with — machine-generated, do NOT hand-edit.
const SNAPSHOT_HEADER: &str = "# Generated by `deno task blanks:audit:update` — do NOT hand-edit.\n\
     #\n\
     # Every line is a KNOWN BUG: a site shape where injecting a blank line makes the\n\
     # formatter break an invariant — leave a 2+ blank run, settle on a non-idempotent\n\
     # fixed point, fail to reparse, corrupt a leaf, or drop/double-print a comment. The\n\
     # gate fails on a line that is NOT here (a new kind of break), on a line here that no\n\
     # longer fires (a stale entry — delete it when you fix one), and on any PANIC.\n\
     #\n\
     # Counts are deliberately not pinned: they churn with every ordinary fixture PR, and a\n\
     # gate that fails per added fixture gets turned off. NON-IDEMPOTENT and every policy\n\
     # kind ARE pinned (this is a ratchet over a live bug family, born red); only a PANIC is\n\
     # never listed — that invariant is absolute, so it always fails the gate.\n\
     #\n\
     # STRUCTURAL-DIVERGENCE is NOT here at all: it is held REPORT-ONLY (fuzz-soft parity), so\n\
     # it is reported but never gated. It is excluded from this file by construction.\n\
     #\n\
     # ABSORBED shapes are not here either: they live in blank_absorb_known.txt — a\n\
     # BEHAVIOR pin (not a bug list) over the gaps where an injected blank is silently\n\
     # deleted, the blank-DROP class every invariant above is blind to.\n\
     #\n\
     # Format: KIND<TAB>SHAPE\n";

/// The ratchet over this audit's colocated snapshot — the one `deno task check` gates on —
/// carrying its header + re-pin hint.
fn ratchet() -> Ratchet {
    Ratchet::colocated("blank_audit_known.txt", SNAPSHOT_HEADER, REPIN_HINT)
}

/// The `#`-comment header of the ABSORB pin file — machine-generated, do NOT hand-edit.
const ABSORB_HEADER: &str = "# Generated by `deno task blanks:audit:update` — do NOT hand-edit.\n\
     #\n\
     # ⚠️ NOT a bug list — a BEHAVIOR PIN (width_audit's stance). Each line is a\n\
     # NODE-EDGE class — `(node_type, left_role→right_role)`, the innermost AST node\n\
     # whose span holds the injection offset and the child-role edge the gap sits in —\n\
     # where the formatter currently ABSORBS an injected blank line: the output is\n\
     # byte-identical to the pristine output, i.e. the blank is silently deleted.\n\
     # Most absorption is sanctioned — prettier collapses a blank in an expression, an\n\
     # argument list, a parameter list too. But a blank eaten at a gap where authors'\n\
     # blanks are meaningful is the blank-DROP bug class (#759's family), and every\n\
     # other gate is blind to it by construction: the dropped-blank output is its own\n\
     # fixed point, so the six invariants, F1, fuzz, and round-trip all pass. Only a\n\
     # prettier compare on the authored shape can grade a pinned line, which is why a\n\
     # NEW class here is a QUESTION to triage against prettier, never a silent pass.\n\
     #\n\
     # The key is deliberately the COARSE node-edge class, not the fine token shape the\n\
     # bug ratchet uses: ~81% of injections absorb, so token granularity pins the whole\n\
     # token-adjacency vocabulary of the fixture tree (~5.7k shapes, minting new ones on\n\
     # ordinary fixture PRs — a gate that fails per added fixture gets turned off). A\n\
     # node-edge class ≈ one emitter decision, which is the grain a triage verdict\n\
     # actually covers. An injection beside an ALREADY-authored blank is exempt: there\n\
     # the byte-identical output is the sanctioned 2+→1 run collapse, not a drop, and\n\
     # recording it would pin classes where blanks otherwise survive.\n\
     #\n\
     # ⚠️ A class graded ABSORBS on ONE reproducer is NOT cleared. The key is a node\n\
     # edge; the `--report` reproducer is one textual shape, and a class routinely spans\n\
     # many (inline comment, glued run, own-line run all key the same line). So a sweep\n\
     # over the reproducers under-reports, and — the other way round — a REAL fix will\n\
     # often leave its line here, because a line goes stale only when NO injection at\n\
     # that edge absorbs anymore. Do not `:update` that away. See docs/blank_audit.md\n\
     # for the prettier-compare harness and its three standing traps.\n\
     #\n\
     # The gate fails on a class NOT here (the formatter newly eats a blank at a new\n\
     # kind of gap) and on a listed class that no longer absorbs (a kept blank — a\n\
     # fix — or a vanished site; re-pin either way).\n\
     #\n\
     # Format: (NODE_TYPE, LEFT→RIGHT)\n";

/// The ratchet over the absorb pin file — the SECOND snapshot this audit grades. Same
/// `--update` regenerates both.
fn absorb_ratchet() -> Ratchet {
    Ratchet::colocated("blank_absorb_known.txt", ABSORB_HEADER, REPIN_HINT)
}

/// One snapshot line: `KIND<TAB>SHAPE`. No payload dimension — there is one payload.
///
/// [`BlankKind`] leads the key, so its derived [`Ord`] matches the `shapes` map's
/// `(BlankKind, shape)` order — the snapshot renders in exactly that order, a stable minimal-diff
/// file.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct BlankKey {
    kind: BlankKind,
    shape: String,
}

impl SnapshotKey for BlankKey {
    fn to_line(&self) -> String {
        format!("{}\t{}", self.kind.label(), self.shape)
    }

    fn from_line(line: &str) -> Option<Self> {
        let mut cols = line.split('\t');
        let kind = BlankKind::from_label(cols.next()?)?;
        let shape = cols.next()?.to_string();
        Some(Self { kind, shape })
    }

    fn is_pinnable(&self) -> bool {
        is_pinnable(self.kind)
    }
}

/// The GRADED shapes as [`BlankKey`]s — the set the [`Ratchet`] sees.
///
/// Excludes the report-only `STRUCTURAL-DIVERGENCE` class ([`is_graded`]) entirely, so it is
/// neither written to the snapshot nor diffed as new / stale. Still includes the unpinnable
/// (`PANIC`) ones — the ratchet splits those off via [`is_pinnable`] and always fails on them.
fn snapshot_keys(shapes: &BTreeMap<(BlankKind, String), ShapeAgg>) -> BTreeSet<BlankKey> {
    shapes
        .keys()
        .filter(|(kind, _)| is_graded(*kind))
        .map(|(kind, shape)| BlankKey {
            kind: *kind,
            shape: shape.clone(),
        })
        .collect()
}

/// One absorb-pin snapshot line: a rendered node-edge CLASS (`(node_type, left→right)`).
/// Always pinnable — an absorbed blank is a behavior being pinned, not an invariant violation
/// (the pin file's header states the stance).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct AbsorbKey(String);

impl SnapshotKey for AbsorbKey {
    fn to_line(&self) -> String {
        self.0.clone()
    }

    fn from_line(line: &str) -> Option<Self> {
        Some(Self(line.to_string()))
    }

    fn is_pinnable(&self) -> bool {
        true
    }
}

/// The absorbed node-edge classes as [`AbsorbKey`]s — the set the absorb [`Ratchet`] sees.
fn absorb_keys(absorb_shapes: &BTreeMap<String, ShapeAgg>) -> BTreeSet<AbsorbKey> {
    absorb_shapes.keys().cloned().map(AbsorbKey).collect()
}

/// One reproducible instance of a shape — kept as the single smallest by `(path, offset)`
/// (an [`ExampleSet`] at `N = 1`), so the chosen example is thread-count independent.
#[derive(Clone)]
struct Example {
    path: String,
    /// The byte offset in the seed the blank was injected at — the splice that reproduces it.
    offset: usize,
    snippet: String,
}

impl ExampleOrd for Example {
    fn sort_key(&self) -> (&str, usize) {
        (&self.path, self.offset)
    }
}

/// Everything a shape accumulates. Counts stay exact; only the single smallest example is kept.
#[derive(Default)]
struct ShapeAgg {
    count: usize,
    /// Distinct seed files the shape fired in.
    files: BTreeSet<String>,
    /// The smallest example by `(path, offset)`.
    examples: ExampleSet<Example, 1>,
}

impl ShapeAgg {
    /// Record one hit: count, distinct file, bounded example. The `contains` probe before the
    /// insert skips the owned-`String` alloc on the common repeat-file hit — this runs on the
    /// absorb fast path too (~80% of accepted injections), not just on findings.
    fn record_hit(&mut self, path: &str, candidate: Example) {
        self.count += 1;
        if !self.files.contains(path) {
            self.files.insert(path.to_string());
        }
        self.examples.offer(candidate);
    }

    /// Fold another aggregate in — the worker-pool merge.
    fn merge(&mut self, other: Self) {
        self.count += other.count;
        self.files.extend(other.files);
        self.examples.merge(other.examples);
    }
}

/// Merge one worker's per-key aggregates into the total's — the shared body behind both of
/// [`Tally::merge`]'s shape maps (the finding shapes and the absorb classes).
fn merge_shape_map<K: Ord>(dst: &mut BTreeMap<K, ShapeAgg>, src: BTreeMap<K, ShapeAgg>) {
    for (k, v) in src {
        match dst.get_mut(&k) {
            Some(e) => e.merge(v),
            None => {
                dst.insert(k, v);
            }
        }
    }
}

/// One thread's slice of the work.
#[derive(Default)]
struct Tally {
    shapes: BTreeMap<(BlankKind, String), ShapeAgg>,
    /// The absorb pin's aggregation: every node-edge CLASS (rendered [`NodeEdgeKey`]
    /// — see [`node_edge_key_with_map`]) whose injected blank the formatter ABSORBED, with the
    /// same per-shape bookkeeping as `shapes` (the example is the reproducer a NEW absorbing
    /// class is triaged from). Keyed by class alone — absorption has one kind.
    ///
    /// [`NodeEdgeKey`]: crate::audit::node_edge::NodeEdgeKey
    absorb_shapes: BTreeMap<String, ShapeAgg>,
    sites: usize,
    injections: usize,
    accepted: usize,
    /// Accepted injections the formatter ABSORBED (output byte-identical to the pristine output) —
    /// the fast path, graded by transitivity without the property battery. The rest ran the full
    /// battery.
    absorbed: usize,
    files_done: usize,
    parse_skipped: usize,
    /// Files that dropped/double-printed a comment AS AUTHORED (ledger-dirty) — reported by
    /// `comments:audit`, not injected into. ~0 over `tests/fixtures`.
    dirty_files: Vec<String>,
    /// Files that aren't a clean format fixed point AS AUTHORED (non-idempotent, unreparseable,
    /// or already blank-run-violating) — reported and skipped, exact count + bounded path
    /// sample. Over `tests/fixtures` these are the variant / unformatted fixture files
    /// (expected).
    not_clean: CappedPaths,
}

impl Tally {
    /// Record one finding at `offset` in `source` (path `path`) — keyed by its [`site_shape`].
    fn record(&mut self, kind: BlankKind, offset: usize, source: &str, path: &str) {
        let shape = site_shape(source, offset);
        let candidate = Example {
            path: path.to_string(),
            offset,
            snippet: snippet(source, offset),
        };
        self.shapes
            .entry((kind, shape))
            .or_default()
            .record_hit(path, candidate);
    }

    /// Record one ABSORBED injection at `offset` under its precomputed node-edge `class` — the
    /// absorb pin's aggregation. Runs on the fast path (~80% of accepted injections), so the
    /// per-hit work is one containment descent (paid by the caller) and the bounded example
    /// bookkeeping — trivial beside the format each injection already paid.
    fn record_absorbed(&mut self, class: String, offset: usize, source: &str, path: &str) {
        let candidate = Example {
            path: path.to_string(),
            offset,
            snippet: snippet(source, offset),
        };
        self.absorb_shapes
            .entry(class)
            .or_default()
            .record_hit(path, candidate);
    }

    /// Record a file skipped for not being a clean fixed point as authored.
    fn record_not_clean(&mut self, display: String) {
        self.not_clean.push(display);
    }

    fn merge(&mut self, other: Tally) {
        self.sites += other.sites;
        self.injections += other.injections;
        self.accepted += other.accepted;
        self.absorbed += other.absorbed;
        self.files_done += other.files_done;
        self.parse_skipped += other.parse_skipped;
        self.dirty_files.extend(other.dirty_files);
        self.not_clean.merge(other.not_clean);
        merge_shape_map(&mut self.shapes, other.shapes);
        merge_shape_map(&mut self.absorb_shapes, other.absorb_shapes);
    }
}

/// Whether the whitespace run containing `offset` already holds a blank line (≥2 newlines) in
/// the SOURCE — the absorb pin's run-collapse exemption.
///
/// An injection adjacent to an authored blank forms a 2+ blank run, which the formatter
/// collapses back to the single blank the pristine output already had — byte-identical output,
/// so the fast path calls it ABSORBED. That absorption is the sanctioned 2+→1 collapse
/// (invariant 6's own requirement), not a drop, and recording it would pin a class where blanks
/// otherwise SURVIVE (a `Program.body` statement gap) just because one seed authors a blank
/// beside one eligible site. A pristine seed is blank-run-clean, so "the run holds ≥2 newlines"
/// is exactly "the gap already carries an authored blank".
fn gap_holds_blank(source: &str, offset: usize) -> bool {
    let bytes = source.as_bytes();
    let is_ws = |b: u8| matches!(b, b' ' | b'\t' | b'\r' | b'\n');
    let mut newlines = 0usize;
    let mut i = offset;
    while i > 0 && is_ws(bytes[i - 1]) {
        i -= 1;
        if bytes[i] == b'\n' {
            newlines += 1;
        }
    }
    let mut j = offset;
    while j < bytes.len() && is_ws(bytes[j]) {
        if bytes[j] == b'\n' {
            newlines += 1;
        }
        j += 1;
    }
    newlines >= 2
}

/// Walk `node` collecting the verbatim-blank regions — template-literal quasis (verbatim text)
/// and Svelte `<pre>` / `<textarea>` (whitespace-preserving elements), in byte space via `map`.
/// A format-ignore region is NOT found here (locating its range from the output is fragile) — a
/// format-ignore-bearing file is exempted whole (see
/// [`source_has_ignore_directive`]).
///
/// A multi-line **string literal** is deliberately NOT a skip region here, even though it too can
/// hold a verbatim blank run: that omission is safe because a base file already carrying such a run
/// fails the pristine blank-run check and is skipped (never injected into), and an injected blank
/// never lands in a string interior (`string_and_template_spans` excludes those sites) — so the
/// only strings this scan meets are pristine-clean, and none can produce a false finding.
fn collect_blank_skip(node: &Value, map: &Utf16ToByte, out: &mut Vec<(usize, usize)>) {
    match node {
        Value::Object(obj) => {
            match obj.get("type").and_then(Value::as_str) {
                // A template quasi's text is verbatim — a blank run inside is content, not a bug.
                Some("TemplateElement") => {
                    if let Some(span) = map.node_byte_span(node) {
                        out.push(span);
                    }
                }
                // A Svelte `<pre>` / `<textarea>` preserves whitespace — over-skip the whole
                // element (sound: over-skipping only ever suppresses a finding, never invents one).
                Some("RegularElement")
                    if obj
                        .get("name")
                        .and_then(Value::as_str)
                        .is_some_and(tsv_html::preserves_whitespace) =>
                {
                    if let Some(span) = map.node_byte_span(node) {
                        out.push(span);
                    }
                }
                // The wire spelling of the one special kind in that same class
                // (`SpecialElementKind::preserves_content_whitespace`): a `<svelte:head>`
                // `<title>`, whose children the compiler pushes verbatim.
                Some("TitleElement") => {
                    if let Some(span) = map.node_byte_span(node) {
                        out.push(span);
                    }
                }
                _ => {}
            }
            for (k, v) in obj {
                if k != "loc" {
                    collect_blank_skip(v, map, out);
                }
            }
        }
        Value::Array(items) => {
            for v in items {
                collect_blank_skip(v, map, out);
            }
        }
        _ => {}
    }
}

/// The byte offset of the first ≥2-blank-line run in `output` that falls OUTSIDE a legitimate
/// verbatim region, or `None` when the output honors the blank-run-≤1 invariant.
///
/// `wire` is the parse of `output` (passed in so the caller's single output parse is reused).
/// `skip` exempts the whole output (a format-ignore-bearing file). A "blank line" is one that is
/// empty after trimming; a run of ≥2 adjacent blank lines whose start is not covered by a
/// verbatim region (template quasi / `<pre>` / `<textarea>`) is a violation.
fn blank_run_violation(output: &str, wire: &Value, skip: bool) -> Option<usize> {
    if skip {
        return None;
    }
    let map = Utf16ToByte::new(output);
    let mut regions = Vec::new();
    collect_blank_skip(wire, &map, &mut regions);
    let in_skip = |pos: usize| regions.iter().any(|&(s, e)| s <= pos && pos < e);

    let mut consecutive_blank = 0usize;
    let mut run_start = 0usize;
    let mut offset = 0usize;
    for line in output.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        if content.trim().is_empty() {
            if consecutive_blank == 0 {
                run_start = offset;
            }
            consecutive_blank += 1;
            if consecutive_blank >= 2 && !in_skip(run_start) {
                return Some(run_start);
            }
        } else {
            consecutive_blank = 0;
        }
        offset += line.len();
    }
    None
}

/// Audit one file: verify it is a clean fixed point AS AUTHORED, then inject a blank at every site.
///
/// The pristine gate is load-bearing: a file that already isn't a fixed point (or is ledger-dirty,
/// or already blank-run-violating) would re-report that base problem at every one of its sites, so
/// such a file is reported once and skipped.
fn audit_file(path: &std::path::Path, render: bool, tally: &mut Tally) {
    let display = path.to_string_lossy().into_owned();
    // Intentionally-invalid fixtures don't parse — nothing to inject into.
    if is_input_invalid_fixture(path) {
        return;
    }
    let Ok(source) = std::fs::read_to_string(path) else {
        return;
    };
    let parser = ParserType::from_extension(&display);
    // CSS is deferred (see the module docs) — skip `.css` seeds outright.
    if parser == ParserType::Css {
        return;
    }

    // Pristine 1/3 — ledger-clean; capture the seed's own comment spans (to exclude a site
    // inside one) and the pristine output the same format already paid for.
    let (comment_spans, pristine_output) = match pristine_format(&source, parser) {
        Pristine::Skip { dirty: false } => {
            tally.parse_skipped += 1;
            return;
        }
        Pristine::Skip { dirty: true } => {
            tally.dirty_files.push(display);
            return;
        }
        Pristine::Clean {
            comment_spans,
            output,
        } => (comment_spans, output),
    };
    // Pristine 2/3 — a format fixed point (idempotency / reparse / leaf) as authored.
    match std::panic::catch_unwind(AssertUnwindSafe(|| f1_check(&source, parser, render))) {
        Ok(F1Outcome::Ok) => {}
        _ => {
            tally.record_not_clean(display);
            return;
        }
    }
    // Pristine 3/3 — reject a file already holding a 2+ blank run as authored. The pristine
    // output (carried by `pristine_format` above — `format_source(&source)`, no second format)
    // is the fast-path oracle below: an injection the formatter ABSORBS reproduces it
    // byte-for-byte, and it is already a proven fixed point, so nothing needs checking. (Only
    // the byte-identity of the fast path is a "proxy" — the slow path grades each changed
    // output exactly against the injected input, not against the pristine.)
    let has_format_ignore = source_has_ignore_directive(&source);
    let Some(pristine_wire) = tsv_parse_to_value(&pristine_output, parser) else {
        // Unreachable — the f1 check just proved the output reparses — but total.
        tally.record_not_clean(display);
        return;
    };
    if blank_run_violation(&pristine_output, &pristine_wire, has_format_ignore).is_some() {
        tally.record_not_clean(display);
        return;
    }
    tally.files_done += 1;

    // Exclusion spans: the seed's own comments PLUS its string / template interiors (where a
    // blank would be lexed as content, not a gap). Non-overlapping, so one combined list feeds
    // `injection_sites` directly. The wire is kept — it also keys the absorb pin's node-edge
    // classes below.
    let source_wire = tsv_parse_to_value(&source, parser);
    let mut exclusion = comment_spans;
    if let Some(wire) = &source_wire {
        exclusion.extend(string_and_template_spans(&source, wire));
    }
    let regions = code_regions(&source, parser);
    let sites = injection_sites(&source, &regions, &exclusion, false);
    tally.sites += sites.len();
    // The absorb keyer's wire→byte map, built ONCE per file and reused across every absorbed
    // hit (`node_edge_key_with_map`'s contract — the per-offset wrapper must not enter a loop).
    let byte_map = Utf16ToByte::new(&source);

    let mut injected = String::with_capacity(source.len() + PAYLOAD.len());
    for &offset in &sites {
        injected.clear();
        injected.push_str(&source[..offset]);
        injected.push_str(PAYLOAD);
        injected.push_str(&source[offset..]);
        tally.injections += 1;

        // Format once (armed + panic-safe) for the output + the ledger findings (invariants 5, 6).
        let (output, ledger_findings) = match ledger_format(&injected, parser) {
            Formatted::Panicked => {
                tally.record(BlankKind::Panic, offset, &source, &display);
                continue;
            }
            // The injected blank broke the syntax — the offset names no valid gap.
            Formatted::Rejected => continue,
            Formatted::Ok {
                findings, output, ..
            } => (output, findings),
        };
        tally.accepted += 1;

        // FAST PATH — the formatter ABSORBED the blank: the output is byte-identical to the
        // pristine output, which the pristine gate already proved is an idempotent, reparseable,
        // blank-run-clean, ledger-clean fixed point. So every invariant holds by transitivity and
        // there is nothing to check — the overwhelmingly common case (most gaps collapse a blank),
        // and what keeps the audit near gap_audit's one-format-per-site cost rather than paying the
        // full property battery on every injection.
        if output == pristine_output {
            tally.absorbed += 1;
            // The pin records only the DROP subset of absorption: an injection beside an
            // authored blank is the sanctioned 2+→1 run collapse (`gap_holds_blank`), not a
            // silently-eaten blank, and pinning it would dilute the class semantics.
            if !gap_holds_blank(&source, offset) {
                // `(unkeyed)` is the total fallback for an offset the wire walk cannot place
                // (no parse, offset outside the root span) — pinned like any class rather than
                // dropped, so keyer coverage is itself visible in the pin file.
                let class = source_wire
                    .as_ref()
                    .and_then(|w| node_edge_key_with_map(w, &byte_map, offset))
                    .map_or_else(|| "(unkeyed)".to_string(), |k| k.to_string());
                tally.record_absorbed(class, offset, &source, &display);
            }
            continue;
        }

        // SLOW PATH — the blank CHANGED the output.
        // Invariant 5 — the injected blank must not drop/double a comment.
        for cf in ledger_findings {
            let kind = match cf.kind {
                CommentFindingKind::Dropped => BlankKind::Dropped,
                CommentFindingKind::DoublePrinted => BlankKind::DoublePrinted,
            };
            tally.record(kind, offset, &source, &display);
        }
        // Invariants 2, 3, 4, 6 — reusing the ledger's `output` (no re-format of `injected` the
        // way `f1_check` would) but grading it EXACTLY against the injected input. Wrapped in
        // `catch_unwind` (the first format didn't panic, so any panic here is the reparse or the
        // idempotency-format's).
        let graded = std::panic::catch_unwind(AssertUnwindSafe(|| {
            grade_changed(&injected, &output, parser, render, has_format_ignore)
        }));
        match graded {
            Err(_) => tally.record(BlankKind::Panic, offset, &source, &display),
            Ok(kinds) => {
                for k in kinds {
                    tally.record(k, offset, &source, &display);
                }
            }
        }
        // The armed `grade_changed` formats leave ledger state; drain it so the next
        // `ledger_format`'s read starts clean (it also drains-before, so this is insurance).
        let _ = comment_ledger::take_comment_ledger();
    }
}

/// Grade a NON-absorbed injection (its output differs from the pristine output) against invariants
/// 2, 3, 4, 6 — the same checks [`f1_check`] runs, but reusing the ledger's already-computed
/// `output` (= `format_source(injected)`) so no second format of the injected input is paid.
///
/// The grade is EXACT: `output` is compared against the INJECTED input (not the pristine one),
/// because a blank line is not always pure whitespace — it can split a token (`1.5` → `1⏎⏎.5`
/// retokenizes to two numbers) or trigger ASI, changing the injected input's own leaves and
/// structure. Only the fast-path byte-identity check (in the caller) uses the pristine output as
/// an oracle, and that IS exact (byte-equal to a proven fixed point). This is why the pristine
/// skeleton is NOT a per-injection proxy: `format` normalizes comment attachment / ASI, so an
/// injection that changes the injected input's structure yet formats back to the pristine shape is
/// a genuine `StructuralDivergence` the injected-relative grade catches and a pristine-relative one
/// would miss.
///
/// Returns the violated invariants (`BlankRun` composes with at most one of the exclusive
/// structure / leaf / idempotency kinds, in `f1_check`'s priority order).
fn grade_changed(
    injected: &str,
    output: &str,
    parser: ParserType,
    render: bool,
    has_format_ignore: bool,
) -> Vec<BlankKind> {
    let mut kinds = Vec::new();
    let wire_in = tsv_parse_to_value(injected, parser);
    // Invariant 3 (reparse) — the output must parse.
    let Some(wire_out) = tsv_parse_to_value(output, parser) else {
        kinds.push(BlankKind::Unreparseable);
        return kinds;
    };
    // Invariant 6 — no 2+ blank run (needs `wire_out` for the verbatim-region skip; done before the
    // structural compare consumes it).
    if blank_run_violation(output, &wire_out, has_format_ignore).is_some() {
        kinds.push(BlankKind::BlankRun);
    }
    // The injected input parsed (it formatted), so this is total; guard rather than unwrap.
    let Some(wire_in) = wire_in else {
        return kinds;
    };
    // Invariant 4 (leaf) — computed before the move-consuming structural compare.
    let leaf_changed = leaf_conservation_diff(&wire_in, &wire_out).is_some();
    // Invariant 3 (structure) — the reparse-skeleton compare. Structure / leaf / idempotency are
    // exclusive, in `f1_check`'s priority order.
    let (equal, _) = structurally_equivalent(wire_in, wire_out, render, false);
    if !equal {
        kinds.push(BlankKind::StructuralDivergence);
    } else if leaf_changed {
        kinds.push(BlankKind::LeafCorruption);
    } else {
        // Invariant 2 (idempotency) — pass 2 must be a fixed point.
        match format_source(output, parser) {
            Ok(f2) if f2 == output => {}
            Ok(_) => kinds.push(BlankKind::NonIdempotent),
            Err(_) => kinds.push(BlankKind::FormatError),
        }
    }
    kinds
}

impl BlankAuditCommand {
    /// The flags that make this run reach a shape set OTHER than the one the snapshot describes
    /// (the blank payload at every non-word/string site over all of `tests/fixtures`).
    fn narrowing_flags(&self) -> Vec<&'static str> {
        let mut flags = Vec::new();
        if !self.paths.is_empty() {
            flags.push("<paths>");
        }
        if self.limit > 0 {
            flags.push("--limit");
        }
        flags
    }

    pub(crate) fn run(self) -> Result<(), CliError> {
        let default_paths = self.paths.is_empty();
        let narrowed = self.narrowing_flags();
        refuse_narrowed_update(
            self.update,
            &narrowed,
            "the blank payload over tests/fixtures",
            "SUBSET",
        )?;
        let files = resolve_seed_files(&self.paths, self.limit)?;

        let armed = ArmedRun::arm(false);
        let render = true;
        // Stride-chunked worker pool (see `audit::parallel::run_pool`); a worker panic outside the
        // per-injection catch fails the run rather than silently dropping a tally.
        let total = run_pool(
            &files,
            self.jobs,
            |path, tally| audit_file(path, render, tally),
            Tally::merge,
        )?;
        drop(armed);

        if self.update {
            ratchet().write_pinned(&snapshot_keys(&total.shapes), "shape")?;
            absorb_ratchet().write_pinned(&absorb_keys(&total.absorb_shapes), "absorb class")?;
            // The report-only STRUCTURAL-DIVERGENCE shapes are deliberately NOT in the file — name
            // the count at re-pin so it's clear they were seen and held soft, not lost.
            let soft = count_soft(&total.shapes);
            if soft > 0 {
                println!(
                    "  ○ {soft} STRUCTURAL-DIVERGENCE shape(s) held report-only (fuzz-soft parity) \
                     — reported, NOT pinned."
                );
            }
            report_not_clean(&total, false, true);
            return report_unpinned_panics(
                count_panics(&total.shapes),
                "shape",
                "a blank in a gap",
            );
        }

        // Grade BEFORE printing (only a graded run can be quiet). The absorb pin is graded only
        // here too — and ONLY on the full default corpus: absorption is normal behavior, so an
        // off-corpus absorbed shape is not news the way an off-corpus finding is.
        let (graded, absorb_graded) = if default_paths && narrowed.is_empty() {
            (
                Some(ratchet().grade(&snapshot_keys(&total.shapes))?),
                Some(absorb_ratchet().grade(&absorb_keys(&total.absorb_shapes))?),
            )
        } else {
            (None, None)
        };

        let (summary, findings) = build_report(&total);
        let show_detail = self.report || !default_paths;
        if self.json {
            // The FULL finding set — each shape carries `gated`, so a consumer sees the report-only
            // STRUCTURAL-DIVERGENCE shapes flagged rather than dropped. The absorb pin rides the
            // extras as counts (the shapes themselves live in blank_absorb_known.txt).
            let mut extras = serde_json::Map::new();
            extras.insert(
                "absorb_shapes".to_string(),
                Value::from(total.absorb_shapes.len()),
            );
            report::print_json(&summary, &findings, &extras);
        } else {
            // Split the shared (ratchet-graded) findings from the report-only STRUCTURAL-DIVERGENCE
            // ones: the graded set drives the shared printers (so their "all pinned" is honest),
            // and the soft set prints in its own labeled, not-gated section (fuzz-soft parity).
            let (soft, graded_findings): (Vec<Finding>, Vec<Finding>) =
                findings.into_iter().partition(is_soft_finding);
            if graded.as_ref().is_some_and(GateDiff::holds) && !self.report {
                report::print_summary(&summary, &graded_findings);
            } else {
                report::print_report(&summary, &graded_findings);
            }
            report_soft(&soft, show_detail);
        }
        report_not_clean(&total, self.json, show_detail);
        // The fast-path share — how much of the corpus the formatter simply absorbs — is the
        // audit's cost story, worth surfacing on a non-JSON run. The distinct-class count is the
        // absorb pin's size in the same breath.
        if !self.json && total.accepted > 0 {
            println!(
                "\n○ {} of {} accepted injections were absorbed (the blank collapsed to the \
                 pristine output — fast path; {} distinct DROP node-edge class(es) after the \
                 run-collapse exemption, pinned by blank_absorb_known.txt); {} ran the full \
                 property battery.",
                total.absorbed,
                total.accepted,
                total.absorb_shapes.len(),
                total.accepted - total.absorbed
            );
            // The per-class rows are the triage view of the pin file (each class with its
            // reproducer) — `--report` / explicit-path only, like the other detail sections.
            if show_detail {
                let mut rows: Vec<(&String, &ShapeAgg)> = total.absorb_shapes.iter().collect();
                rows.sort_by_key(|(_, agg)| std::cmp::Reverse(agg.count));
                for (class, agg) in rows {
                    let ex = agg.examples.canonical();
                    println!(
                        "  {:>7}×  {:<48} e.g. inject blank at {}:{}  {}",
                        agg.count, class, ex.path, ex.offset, ex.snippet
                    );
                }
            }
        }

        // Off the default corpus the snapshot doesn't apply — every GRADED finding is news (a
        // report-only STRUCTURAL-DIVERGENCE shape never fails, on or off corpus, and an absorbed
        // shape is graded only against the default corpus's pin).
        if !default_paths {
            let has_graded = total.shapes.keys().any(|(k, _)| is_graded(*k));
            return if has_graded {
                Err(CliError::Failed)
            } else {
                Ok(())
            };
        }
        // A narrowed default run reaches only part of the snapshot's shape set, so grading it
        // would report every unreached shape as stale — report and stop rather than fail.
        if !narrowed.is_empty() {
            print_ratchet_skipped(&narrowed);
            return Ok(());
        }
        // Report BOTH diffs before deciding the exit, so a run failing on each prints each.
        let bug_gate = match &graded {
            Some(diff) => self.report_gate(diff, &total),
            None => Ok(()),
        };
        let absorb_gate = match &absorb_graded {
            Some(diff) => report_absorb_gate(diff, &total),
            None => Ok(()),
        };
        bug_gate.and(absorb_gate)
    }

    /// Report a [`GateDiff`] and turn it into an exit status.
    fn report_gate(&self, diff: &GateDiff<BlankKey>, total: &Tally) -> Result<(), CliError> {
        let GateDiff { new, stale, .. } = diff;

        // Panics are graded on their own, never against the snapshot.
        let panics: Vec<_> = total
            .shapes
            .iter()
            .filter(|((kind, _), _)| *kind == BlankKind::Panic)
            .collect();
        if !panics.is_empty() {
            eprintln!(
                "\n✗ {} shape(s) CRASH the formatter — a blank in a gap must never panic it. Not \
                 pinnable and not a ratchet entry: fix the crash.",
                panics.len()
            );
            for ((_, shape), agg) in panics.iter().take(40) {
                let ex = agg.examples.canonical();
                eprintln!(
                    "    {shape:<20} e.g. inject blank at {}:{}",
                    ex.path, ex.offset
                );
            }
            if panics.len() > 40 {
                eprintln!("    … and {} more", panics.len() - 40);
            }
        }

        if !new.is_empty() {
            eprintln!(
                "\n✗ {} NEW finding shape(s) — a blank in one of these gaps breaks an invariant \
                 the snapshot has never seen:",
                new.len()
            );
            for k in new.iter().take(40) {
                eprintln!("    {:<22} {}", k.kind.label(), k.shape);
            }
            if new.len() > 40 {
                eprintln!("    … and {} more", new.len() - 40);
            }
            eprintln!(
                "  Fix it, or — if it is genuinely pre-existing and merely newly REACHED by a \
                 fixture — re-run `deno task blanks:audit:update`."
            );
        }
        if !stale.is_empty() {
            eprintln!(
                "\n✗ {} STALE snapshot entry/entries — these no longer fire. If you fixed them, \
                 drop the lines (`deno task blanks:audit:update`):",
                stale.len()
            );
            for k in stale.iter().take(40) {
                eprintln!("    {:<22} {}", k.kind.label(), k.shape);
            }
            if stale.len() > 40 {
                eprintln!("    … and {} more", stale.len() - 40);
            }
        }

        if diff.holds() {
            let msg = format!(
                "\n✓ ratchet holds — every finding shape is a known bug ({} pinned); no new blank \
                 breaks an invariant",
                diff.known
            );
            if self.json {
                eprintln!("{msg}");
            } else {
                println!("{msg}");
            }
            Ok(())
        } else {
            Err(CliError::Failed)
        }
    }
}

/// Report the absorb pin's [`GateDiff`] and turn it into an exit status — the second gate a
/// full default run grades ([`absorb_ratchet`]). A NEW class is a **question, not a verdict**
/// (the pin file's header states the stance): the formatter now eats a blank at a kind of gap
/// the pin has never seen, and only a prettier compare on the authored shape can grade it —
/// which is exactly why it must not land silently.
fn report_absorb_gate(diff: &GateDiff<AbsorbKey>, total: &Tally) -> Result<(), CliError> {
    let GateDiff { new, stale, .. } = diff;

    if !new.is_empty() {
        eprintln!(
            "\n✗ {} NEW ABSORB class(es) — the formatter EATS an injected blank at a kind of gap \
             the pin has never seen. A silently deleted blank is its own fixed point (no other \
             invariant can see it — the blank-DROP class), so triage against prettier: if \
             prettier collapses the blank there too, re-pin (`{REPIN_HINT}`); if prettier keeps \
             it, this is a blank-DROP bug — fix it instead.",
            new.len()
        );
        for k in new.iter().take(40) {
            match total.absorb_shapes.get(&k.0) {
                Some(agg) => {
                    let ex = agg.examples.canonical();
                    eprintln!(
                        "    {:<40} e.g. inject blank at {}:{}  {}",
                        k.0, ex.path, ex.offset, ex.snippet
                    );
                }
                None => eprintln!("    {}", k.0),
            }
        }
        if new.len() > 40 {
            eprintln!("    … and {} more", new.len() - 40);
        }
    }
    if !stale.is_empty() {
        eprintln!(
            "\n✗ {} STALE absorb pin entry/entries — these classes no longer absorb (a kept blank \
             — a fix — or a vanished site). Re-pin (`{REPIN_HINT}`):",
            stale.len()
        );
        for k in stale.iter().take(40) {
            eprintln!("    {}", k.0);
        }
        if stale.len() > 40 {
            eprintln!("    … and {} more", stale.len() - 40);
        }
    }

    if diff.holds() {
        println!(
            "✓ absorb pin holds — no new kind of silently-eaten blank ({} classes pinned)",
            diff.known
        );
        Ok(())
    } else {
        Err(CliError::Failed)
    }
}

/// Translate a run's [`Tally`] into the shared reporting envelope.
fn build_report(total: &Tally) -> (RunSummary, Vec<Finding>) {
    let summary = RunSummary {
        audit: "blank_audit",
        files_done: total.files_done,
        sites: total.sites,
        injections: total.injections,
        accepted: total.accepted,
        parse_skipped: total.parse_skipped,
        dirty_files: total.dirty_files.clone(),
        payload_labels: vec!["blank"],
    };
    let findings = total
        .shapes
        .iter()
        .map(|((kind, shape), agg)| {
            let ex = agg.examples.canonical();
            Finding {
                audit: "blank_audit",
                severity: if *kind == BlankKind::Panic {
                    Severity::GateFailing
                } else {
                    Severity::Informational
                },
                confidence: None,
                site: shape.clone(),
                verdict_string: String::new(),
                example: ReportExample {
                    payload: "blank",
                    path: ex.path.clone(),
                    injection_offset: ex.offset,
                    attribution_offset: ex.offset,
                    snippet: ex.snippet.clone(),
                    text: "\\n\\n".to_string(),
                    injected: true,
                    skip_sites: Vec::new(),
                },
                detail: Detail::Blank(BlankDetail {
                    kind_label: kind.label(),
                    count: agg.count,
                    files: agg.files.len(),
                    gated: is_graded(*kind),
                }),
            }
        })
        .collect();
    (summary, findings)
}

/// Print the "skipped — not a clean fixed point as authored" bucket (blank-specific, so it lives
/// here rather than in the shared envelope). Under `--json` it goes to stderr, leaving the JSON
/// document the sole parseable stdout.
///
/// The COUNT always prints — a file the audit couldn't grade is a coverage fact, and a graded gate
/// must never silently drop it. The path SAMPLE prints only when `show_paths` (a `--report` run,
/// an explicit-path run, or `--update`): over `tests/fixtures` the ~170 skipped files are the
/// expected `unformatted_*` variants, so the list is pure noise inside `deno task check`, but over
/// a real corpus (an explicit path) it is the triage list.
fn report_not_clean(total: &Tally, json: bool, show_paths: bool) {
    if total.not_clean.is_empty() {
        return;
    }
    let line = |s: String| {
        if json {
            eprintln!("{s}");
        } else {
            println!("{s}");
        }
    };
    line(format!(
        "\n○ {} file(s) skipped — not a clean format fixed point AS AUTHORED (non-idempotent, \
         unreparseable, or already blank-run-violating). Over tests/fixtures this is expected \
         (the variant / unformatted / prettier-output fixture files are not tsv fixed points); \
         over a real-code corpus each wants triage{}",
        total.not_clean.count(),
        if show_paths {
            ":"
        } else {
            " (--report to list)"
        }
    ));
    if !show_paths {
        return;
    }
    for l in total.not_clean.sample_lines("    ") {
        line(l);
    }
}

/// Whether a finding is REPORT-ONLY (the ungraded `STRUCTURAL-DIVERGENCE` class) — the partition
/// predicate that splits the shared graded printers from [`report_soft`].
fn is_soft_finding(f: &Finding) -> bool {
    !f.gated()
}

/// Print the report-only `STRUCTURAL-DIVERGENCE` section — reported, never gated (fuzz-soft
/// parity). The COUNT always prints (a report-only finding is still a finding); the per-shape rows
/// print only with `show_rows` (a `--report` or explicit-path run), like the not-clean bucket.
fn report_soft(soft: &[Finding], show_rows: bool) {
    if soft.is_empty() {
        return;
    }
    let hits: usize = soft.iter().map(Finding::count).sum();
    println!(
        "\n○ {} STRUCTURAL-DIVERGENCE shape(s) ({hits} hit(s)) — reported, NOT gated (soft, like \
         fuzz's structural_divergence: a blank-induced structural change over Svelte is \
         render-model noisy and\n  wants canonical confirmation — `roundtrip_audit \
         --canonical-all <input>` — before it earns a pin, so it is excluded from the ratchet){}",
        soft.len(),
        if show_rows {
            ":"
        } else {
            " (--report to list)"
        }
    );
    if !show_rows {
        return;
    }
    let mut rows: Vec<&Finding> = soft.iter().collect();
    rows.sort_by_key(|f| std::cmp::Reverse(f.count()));
    for f in rows.iter().take(40) {
        println!("  {:>7}×  {}", f.count(), f.site);
        let ex = &f.example;
        println!(
            "            e.g. inject blank at {}:{}  {}",
            ex.path, ex.injection_offset, ex.snippet
        );
    }
    if rows.len() > 40 {
        println!("  … and {} more", rows.len() - 40);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A minimal shape agg carrying one example — for the snapshot tests.
    fn mk_agg() -> ShapeAgg {
        let mut examples = ExampleSet::default();
        examples.offer(Example {
            path: "p.svelte".to_string(),
            offset: 0,
            snippet: String::new(),
        });
        ShapeAgg {
            count: 1,
            files: BTreeSet::new(),
            examples,
        }
    }

    /// The snapshot is the gate's on-disk contract: whatever `--update` writes, the gate must
    /// read back as the identical key set. Kind leads the key, so the render is in `BlankKind`
    /// enum order (`NON-IDEMPOTENT` before `BLANK-RUN`) — NOT label-string order.
    #[test]
    fn snapshot_render_and_parse_round_trip() {
        let mut shapes: BTreeMap<(BlankKind, String), ShapeAgg> = BTreeMap::new();
        shapes.insert((BlankKind::NonIdempotent, "IDENT⟨⟩=".to_string()), mk_agg());
        shapes.insert((BlankKind::BlankRun, "{⟨⟩IDENT".to_string()), mk_agg());

        let r = Ratchet::new(PathBuf::from("/unused"), SNAPSHOT_HEADER, REPIN_HINT);
        let found = snapshot_keys(&shapes);
        let rendered = r.render(&found);
        let parsed: BTreeSet<BlankKey> = rendered
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(BlankKey::from_line)
            .collect();
        assert_eq!(parsed, found, "render → parse must round-trip");

        // Enum order: NonIdempotent (1) before BlankRun (8), regardless of label-string order.
        let data: Vec<&str> = rendered
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        assert_eq!(
            data,
            vec!["NON-IDEMPOTENT\tIDENT⟨⟩=", "BLANK-RUN\t{⟨⟩IDENT"],
            "renders in BlankKind-enum order; each line is KIND<TAB>SHAPE"
        );
    }

    /// A panic must never reach the snapshot — not via `--update`, and not as a diffed key.
    #[test]
    fn a_panic_is_never_pinned() {
        let mut shapes: BTreeMap<(BlankKind, String), ShapeAgg> = BTreeMap::new();
        shapes.insert((BlankKind::NonIdempotent, "IDENT⟨⟩=".to_string()), mk_agg());
        shapes.insert((BlankKind::Panic, "IDENT⟨⟩(".to_string()), mk_agg());

        let r = Ratchet::new(PathBuf::from("/unused"), SNAPSHOT_HEADER, REPIN_HINT);
        let found = snapshot_keys(&shapes);
        let rendered = r.render(&found);
        let data: Vec<&str> = rendered
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        assert_eq!(
            data,
            vec!["NON-IDEMPOTENT\tIDENT⟨⟩="],
            "only the pinnable shape is written"
        );
        assert_eq!(count_panics(&shapes), 1, "but the panic is still counted");
        assert!(
            found
                .iter()
                .filter(|k| k.is_pinnable())
                .all(|k| k.kind != BlankKind::Panic)
        );
    }

    /// STRUCTURAL-DIVERGENCE is REPORT-ONLY: it must be filtered OUT of the graded key set
    /// entirely (so the ratchet never pins it and never fails on it) — the third category. This is
    /// the mechanism `is_graded` implements, NOT `is_pinnable` (which would make it FAIL like a
    /// panic). The corpus can't grade the distinction: both a report-only shape and a would-fail
    /// one are absent from the snapshot file, so only this pins that struct-div produces no
    /// new/stale key.
    #[test]
    fn structural_divergence_is_report_only_not_graded() {
        let mut shapes: BTreeMap<(BlankKind, String), ShapeAgg> = BTreeMap::new();
        shapes.insert((BlankKind::NonIdempotent, "IDENT⟨⟩=".to_string()), mk_agg());
        shapes.insert(
            (BlankKind::StructuralDivergence, "␣⟨⟩/*".to_string()),
            mk_agg(),
        );

        // The graded set the ratchet sees excludes struct-div entirely — only the NonIdempotent
        // key is a graded key.
        let found = snapshot_keys(&shapes);
        assert_eq!(found.len(), 1, "struct-div is not a graded key");
        assert!(
            found
                .iter()
                .all(|k| k.kind != BlankKind::StructuralDivergence)
        );

        // So grading against a snapshot pinning only NonIdempotent HOLDS — struct-div contributes
        // no NEW key (it's not in `found`) and no STALE key (it's not in the snapshot).
        let t = TempRatchet::new();
        t.ratchet.write(&found).expect("write");
        let diff = t.ratchet.grade(&found).expect("grade");
        assert!(diff.holds(), "struct-div excluded ⇒ the graded set holds");
        assert_eq!(diff.known, 1, "only the NonIdempotent shape is pinned");

        // And it is counted as a report-only shape, distinct from a panic.
        assert_eq!(count_soft(&shapes), 1);
        assert_eq!(count_panics(&shapes), 0);
    }

    /// A ratchet over a fresh temp file, cleaned up on drop — for the grade test above.
    struct TempRatchet {
        ratchet: Ratchet,
        path: PathBuf,
    }
    impl TempRatchet {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static NONCE: AtomicU32 = AtomicU32::new(0);
            let n = NONCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "tsv_blank_ratchet_test_{}_{n}.txt",
                std::process::id()
            ));
            Self {
                ratchet: Ratchet::new(path.clone(), SNAPSHOT_HEADER, REPIN_HINT),
                path,
            }
        }
    }
    impl Drop for TempRatchet {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn full_run() -> BlankAuditCommand {
        BlankAuditCommand {
            json: false,
            report: false,
            jobs: None,
            limit: 0,
            update: false,
            paths: Vec::new(),
        }
    }

    /// Every flag that changes which shapes a run reaches must disqualify `--update` / grading.
    #[test]
    fn every_narrowing_flag_disqualifies_a_run() {
        assert!(full_run().narrowing_flags().is_empty());

        let paths = BlankAuditCommand {
            paths: vec!["src".to_string()],
            ..full_run()
        };
        assert_eq!(paths.narrowing_flags(), vec!["<paths>"]);

        let limit = BlankAuditCommand {
            limit: 30,
            ..full_run()
        };
        assert_eq!(limit.narrowing_flags(), vec!["--limit"]);

        // Reporting flags don't change the shape set.
        let reporting_only = BlankAuditCommand {
            json: true,
            report: true,
            jobs: Some(1),
            ..full_run()
        };
        assert!(reporting_only.narrowing_flags().is_empty());
    }

    /// The blank-run scanner: 2+ blank lines outside a verbatim region is a violation; one blank
    /// is fine; a run inside a template quasi is exempt; and the format-ignore `skip` exempts all.
    #[test]
    fn blank_run_violation_flags_only_real_runs() {
        let scan = |src: &str, skip: bool| {
            let wire = tsv_parse_to_value(src, ParserType::TypeScript).expect("parses");
            blank_run_violation(src, &wire, skip)
        };
        // Two blank lines between statements → a violation at the first blank line's byte start.
        assert_eq!(
            scan("a;\n\n\nb;", false),
            Some(3),
            "2+ blank lines outside any verbatim region is a violation"
        );
        // A single blank line is allowed.
        assert_eq!(scan("a;\n\nb;", false), None);
        // The same run inside a template quasi is legitimate content — exempt.
        assert_eq!(
            scan("const t = `x\n\n\ny`;", false),
            None,
            "a blank run inside a template quasi is verbatim content"
        );
        // A format-ignore-bearing file is exempt whole (via the `skip` flag).
        assert_eq!(scan("a;\n\n\nb;", true), None);
    }

    /// The absorb pin file round-trips through its rendered-class line format, and every absorb
    /// key is pinnable (absorption is a behavior pin, never an always-fails class).
    #[test]
    fn absorb_snapshot_render_and_parse_round_trip() {
        let mut absorb: BTreeMap<String, ShapeAgg> = BTreeMap::new();
        absorb.insert(
            "(CallExpression, arguments→arguments)".to_string(),
            mk_agg(),
        );
        absorb.insert("(VariableDeclarator, id→init)".to_string(), mk_agg());

        let r = Ratchet::new(PathBuf::from("/unused"), ABSORB_HEADER, REPIN_HINT);
        let found = absorb_keys(&absorb);
        assert!(found.iter().all(SnapshotKey::is_pinnable));
        let rendered = r.render(&found);
        let parsed: BTreeSet<AbsorbKey> = rendered
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(AbsorbKey::from_line)
            .collect();
        assert_eq!(parsed, found, "render → parse must round-trip");
    }

    /// The run-collapse exemption: a gap already holding an authored blank (≥2 newlines in its
    /// whitespace run) is exempt from the absorb pin; a same-line gap, a single-newline gap,
    /// and an offset mid-run all read their whole run.
    #[test]
    fn gap_holds_blank_reads_the_whole_ws_run() {
        // Same-line gap — no newline at all.
        let src = "const x = 1;";
        assert!(!gap_holds_blank(src, src.find('=').unwrap()));
        // A single newline is not a blank.
        let src = "a;\nb;";
        assert!(!gap_holds_blank(src, 3));
        // An authored blank (two newlines) — exempt, wherever in the run the offset sits.
        let src = "a;\n\nb;";
        assert!(gap_holds_blank(src, 2), "run start sees both newlines");
        assert!(gap_holds_blank(src, 3), "mid-run sees one back, one ahead");
        assert!(gap_holds_blank(src, 4), "run end sees both behind");
        // Indentation between the newlines still counts as one run.
        let src = "a;\n\t \nb;";
        assert!(gap_holds_blank(src, 3));
    }

    /// An absorbed injection aggregates by its precomputed node-edge class (one kind), with the
    /// same smallest-example bookkeeping as a finding — the reproducer a NEW class is triaged
    /// from.
    #[test]
    fn record_absorbed_keys_on_class() {
        let src = "const x = 1;";
        let eq = src.find('=').unwrap();
        let class = "(VariableDeclarator, id→init)";
        let mut tally = Tally::default();
        tally.record_absorbed(class.to_string(), eq, src, "b.ts");
        tally.record_absorbed(class.to_string(), eq, src, "a.ts");
        let agg = tally
            .absorb_shapes
            .get(class)
            .expect("keyed on the precomputed class");
        assert_eq!(agg.count, 2);
        assert_eq!(agg.files.len(), 2);
        assert_eq!(
            agg.examples.canonical().path,
            "a.ts",
            "the smallest (path, offset) is canonical"
        );
    }

    /// A finding keys on the injection offset's [`site_shape`], and the kept example is the
    /// smallest by `(path, offset)`.
    #[test]
    fn record_keys_on_site_shape_and_keeps_smallest_example() {
        let src = "const x = 1;";
        let eq = src.find('=').unwrap();
        let mut tally = Tally::default();
        tally.record(BlankKind::NonIdempotent, eq, src, "b.ts");
        tally.record(BlankKind::NonIdempotent, eq, src, "a.ts");
        let agg = tally
            .shapes
            .get(&(BlankKind::NonIdempotent, site_shape(src, eq)))
            .expect("keyed on the `=` gap's shape");
        assert_eq!(agg.count, 2);
        assert_eq!(agg.files.len(), 2);
        assert_eq!(
            agg.examples.canonical().path,
            "a.ts",
            "the smallest (path, offset) is canonical"
        );
    }
}
