//! Ignore-directive honoring audit (Arm A) — the mechanized discovery of unhonored
//! `// prettier-ignore` / `format-ignore` positions.
//!
//! ## Why this exists
//!
//! Recognition of the ignore directive is centralized and correct
//! (`tsv_lang::is_format_ignore_directive`), but **consumption is a per-node opt-in the printer
//! makes at each position** — ~15 scattered sites. A construct whose printer position is in that
//! set is emitted raw; any position NOT in it silently *reformats* an ignored construct, breaking
//! the drop-in `prettier-ignore` contract. One instance is confirmed
//! (`TSUnionType.types` — `| a1&a2` reformats to `(a1 & a2)`), and §1.3 of the ignore-honoring
//! plan lists a dozen *suspected* positions. This audit replaces that guess-list with a computed
//! ledger, the same way `comments:audit` (the print-once ledger) structurally guards the per-site
//! `owned_by_node` comment model rather than trusting each site by inspection.
//!
//! ## The invariant (per injected directive)
//!
//! For a node preceded by an ignore directive, the node's original source slice must appear
//! **verbatim** in the output (modulo the directive's own line). Since a seed file is a format
//! fixed point, every node is already canonical — so honoring and reformatting would be
//! indistinguishable. To make the difference observable, the node's interior whitespace is
//! **perturbed** (every structural space doubled): a doubled space can only be removed by
//! reformatting, never by preservation. So:
//!
//! - **honored** → the perturbed slice survives verbatim → it is a substring of the output.
//! - **not honored** → the perturbation collapses → the slice is NOT a substring = an
//!   [`IgnoreKind::Unhonored`] finding at that node's AST position.
//!
//! ## The four graded checks (per injected directive)
//!
//! 1. **Honoring** ([`IgnoreKind::Unhonored`]) — the original check above: the perturbed slice
//!    must survive the format verbatim.
//! 2. **Second-pass stability** ([`IgnoreKind::Unstable`]) — the accepted output is formatted
//!    once more and must be byte-identical: the mechanical guard on the relocation-transient
//!    class (a directive honored on pass 1 that an emitter relocates into a placement pass 2
//!    reads as inert — the annotation-head `␣⟨⟩//` transient's shape).
//! 3. **Scope / sibling control** ([`IgnoreKind::Overfrozen`]) — the same injection with every
//!    structural space OUTSIDE the node also doubled (in-region, exclusion-free) must format
//!    byte-identically to the primary output. A surviving doubled space outside the node means
//!    the freeze over-extends — over-freezing previously read as "honored" (the cluster-1
//!    escape's blind spot).
//! 4. **Trailing inertness** ([`IgnoreKind::TrailingFrozen`]) — the directive appended to the
//!    END of the preceding line instead must freeze nothing: the decided placement floor
//!    (content before a directive on its line ⇒ inert). Skipped after an opening delimiter
//!    (`{` / `[` / `(` / `<`), where a trailing directive is decided FORWARD-BINDING, not
//!    inert — both tools freeze the first member after `{ // prettier-ignore` (prettier
//!    relocates the directive own-line, tsv preserves its position).
//!
//! The companion checks (2–4) run only on the **span-maximal node beginning on each line**: the
//! directive is inserted above the whole line, so it binds to the OUTERMOST construct beginning
//! there (a statement-head directive ignores the whole statement, in both tools), and a narrower
//! same-line candidate would grade that decided wider freeze as a finding. The honoring check
//! stays per-candidate.
//!
//! [`IgnoreKind::Panic`] (the injected directive crashed the formatter — production WASM is
//! `panic = "abort"`, so a crash is a DoS) is NEVER pinnable and always fails the gate; the four
//! graded kinds are the ratcheted position ledger. The honoring check is a deliberately
//! **cheaper single-format check** than `blank_audit`'s F1 battery (see
//! `[[gap-audit-f1-cost-and-cheaper-alternative]]`): one format per injection, one substring
//! test. The companion checks add at most three more formats per injection — still far under an
//! F1 battery, and the whole audit stays in the `deno task check` budget.
//!
//! ## The finding key — AST position, not a token shape
//!
//! Unlike `gap_audit` / `blank_audit` (which key by [`site_shape`], a flat token key), this audit
//! keys by the node's **AST position** — `{enclosing-node-type}.{child-field}`, e.g.
//! `TSUnionType.types`, `TSTupleType.elementTypes`, `Program.body`. Honoring is a per-*position*
//! property (a position either has the printer opt-in or it doesn't), so the ledger is a ledger of
//! **positions**, which is exactly what the plan's §1.3 wants. A position that honors (check 1)
//! can still appear via a companion-check finding — `TRAILING_FROZEN` / `OVERFROZEN` / `UNSTABLE`
//! at the same shape — so "covered" means passing all four graded checks; the ledger names every
//! `(kind, position)` pair that fails one.
//!
//! ## Design
//!
//! Pure Rust, no sidecar, no new deps — the `fuzz` / `gap_audit` / `blank_audit` direction. Sites
//! come from a walk of the wire AST tree keyed to `code_regions` (the spans the AST says are JS),
//! so the perturbation lands only in JS. Each candidate node must (a) lie fully inside a JS region,
//! (b) **lead its own line** (modulo a single leading `|`/`&` union/intersection separator — so the
//! directive binds to it), and (c) have at least one **perturbable** structural space (a space
//! outside a string/template/comment/regex-literal interior). A node with none is skipped: it
//! reformats to itself, so honoring is untestable and uninteresting.
//!
//! ## Scope — what a green run does NOT prove
//!
//! - **JS positions only.** The TS/JS `//` directive is injected into `code_regions` — standalone
//!   `.ts`/`.svelte.ts` (whole file) and a `.svelte` component's `<script>` / `{expr}` slots. CSS
//!   (`/* prettier-ignore */`) and Svelte template markup (`<!-- prettier-ignore -->`) use different
//!   directive spellings and are a deliberate follow-up (the plan's open Q (b) — CSS/Svelte parity),
//!   the same CSS deferral `blank_audit` makes.
//! - **Whitespace-reformatting positions only.** The perturbation is space-doubling, so a position
//!   whose only reformatting is non-whitespace (quote normalization, paren strip) is invisible to
//!   Arm A. Arm B (the curated control matrix) backstops specific such positions.
//! - **Only format fixed points are injected into.** A seed that isn't idempotent / doesn't reparse
//!   as authored is reported once and skipped (over `tests/fixtures` these are the variant /
//!   unformatted fixture files by design; the real yield is external corpora).
//! - **A seed already bearing an ignore directive is skipped** whole (coarse substring exemption) —
//!   an injected directive interacting with a pre-existing one is fragile, and the confirmed gaps
//!   are reached through the many directive-free fixtures regardless.
//! - **Union/Intersection candidate nodes are skipped** (paren-transparently — `is_composite_type`):
//!   at a single-child head, an own-line directive before a composite freezes only the FIRST member
//!   by decided semantics (Rule A; prettier's `types[0]` redirect agrees), so the whole-node
//!   substring check is the wrong expectation there, and the member-level positions carry those
//!   placements. A single-child position's ledger line therefore measures only its non-composite
//!   sites — a closed line means those sites honor, not that composite-valued ones were exercised.
//!   The skip keys on the candidate's own type, not its position, so a union-typed LIST member
//!   (where the printer does freeze the whole member) also goes untested here — that narrower loss
//!   is accepted and carried by the member-freeze fixtures. The skipped span still feeds the
//!   line-maximality map, so the union's own first member (sharing its line) doesn't run the
//!   companion checks against a narrower span than the decided item-level freeze.

use argh::FromArgs;
use std::collections::{BTreeMap, BTreeSet};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};

use serde_json::Value;

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;
use tsv_lang::comment_ledger;

use crate::audit::examples::{ExampleOrd, ExampleSet};
use crate::audit::node_edge::is_non_structural_key;
use crate::audit::parallel::{ArmedRun, run_pool};
use crate::audit::properties::{Pristine, Utf16ToByte, pristine_format, tsv_parse_to_value};
use crate::audit::ratchet::{
    GateDiff, Ratchet, SnapshotKey, print_ratchet_skipped, refuse_narrowed_update,
    report_unpinned_panics,
};
use crate::audit::report::{
    self, Detail, Finding, IgnoreDetail, ReportExample, RunSummary, Severity,
};
use crate::audit::sites::{
    code_regions, regex_literal_spans, snippet, source_has_ignore_directive,
    string_and_template_spans,
};
use crate::audit::tally::CappedPaths;
use crate::cli::CliError;

use super::profile::{is_input_invalid_fixture, resolve_seed_files};

/// Inject a `// prettier-ignore` directive before every JS node and grade four checks: honoring,
/// second-pass stability, freeze scope, and trailing inertness (see the module docs).
///
/// For each seed file, at each candidate node position (one at a time), prepends the directive on
/// its own line with the node's interior structural spaces doubled, formats, and grades the four
/// checks; reports every `(kind, position)` that fails one, and every position that crashes the
/// formatter. Pure Rust — no Deno. Defaults to `tests/fixtures`; the real yield is external corpora.
/// Exits 1 on a new / stale / panic finding shape (a ratchet, like `gap_audit` / `blank_audit`).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "ignore_audit")]
#[allow(clippy::struct_excessive_bools)] // independent CLI flags
pub struct IgnoreAuditCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// print the full per-shape report even when the ratchet holds. A passing gate is summary-only
    /// by default — the positions it already knows about are noise in `deno task check`
    #[argh(switch)]
    report: bool,

    /// worker threads (default: available parallelism). Each file's whole inject loop stays on one
    /// thread
    #[argh(option)]
    jobs: Option<usize>,

    /// cap the number of seed files (0 = unlimited)
    #[argh(option, default = "0")]
    limit: usize,

    /// rewrite the committed shape snapshot from this run. Only valid on a FULL default run — the
    /// snapshot describes the directive payload over `tests/fixtures` and nothing else, so any
    /// narrowing flag is refused rather than silently pinning a partial set
    #[argh(switch)]
    update: bool,

    /// seed file paths / directories (default: tests/fixtures)
    #[argh(positional)]
    paths: Vec<String>,
}

/// The TS/JS directive, on its own line. CSS (`/* prettier-ignore */`) and Svelte template markup
/// (`<!-- prettier-ignore -->`) are a follow-up (see the module docs).
const DIRECTIVE: &str = "// prettier-ignore\n";

/// The operators the printer's break styles put at the START of a broken line, so a node they lead
/// still counts as "leading its line" for the directive to bind to it: union `|`, intersection `&`,
/// and the ternary `?` / `:`. Named and justified together rather than grown ad hoc per construct —
/// the point of the audit is a *computed* position ledger, so this gate must not re-introduce a
/// curated, construct-specific guess. (Member-chain `.` / `?.` are deliberately excluded: they lead
/// a DIFFERENT node — the member's property — which is its own position, not handled here.)
const LINE_LEAD_OPERATORS: [&str; 4] = ["|", "&", "?", ":"];

/// Why an injected directive is a finding. `Panic` is the one absolute break (never pinnable);
/// the other four are the ratcheted position ledger — one per graded check (see the module docs).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum IgnoreKind {
    /// The formatter crashed on the injected directive — NEVER pinnable, always fails the gate.
    Panic,
    /// The ignored node's perturbed source did NOT survive verbatim — the position silently
    /// reformatted an ignored construct (the drop-in `prettier-ignore` contract violation).
    Unhonored,
    /// A directive TRAILING the preceding line (content before it on its line) froze the node
    /// anyway — a violation of the decided inert-placement floor (the misbinding class).
    TrailingFrozen,
    /// MORE than the ignored node was frozen — a doubled structural space OUTSIDE the node
    /// survived the format (the scope check's sibling control).
    Overfrozen,
    /// The injection's accepted output is not a format fixed point — pass 2 changed it (the
    /// freeze-on-pass-1 / inert-on-pass-2 relocation-transient class).
    Unstable,
}

impl IgnoreKind {
    fn label(self) -> &'static str {
        match self {
            Self::Panic => "PANIC",
            Self::Unhonored => "UNHONORED",
            Self::TrailingFrozen => "TRAILING_FROZEN",
            Self::Overfrozen => "OVERFROZEN",
            Self::Unstable => "UNSTABLE",
        }
    }

    fn from_label(s: &str) -> Option<Self> {
        [
            Self::Panic,
            Self::Unhonored,
            Self::TrailingFrozen,
            Self::Overfrozen,
            Self::Unstable,
        ]
        .into_iter()
        .find(|k| k.label() == s)
    }
}

/// Whether a shape may be **pinned** into the snapshot — everything but a [`IgnoreKind::Panic`].
///
/// The audit is a **ratchet over a live bug family** born RED: the confirmed union gap and its
/// suspected siblings are day-one findings, so they must be pinnable or the gate would hard-block
/// `deno task check` on landing. Only a crash stays absolute.
fn is_pinnable(kind: IgnoreKind) -> bool {
    kind != IgnoreKind::Panic
}

/// How many of `shapes` crash the formatter — kept out of the snapshot, accounted separately.
fn count_panics(shapes: &BTreeMap<(IgnoreKind, String), ShapeAgg>) -> usize {
    shapes
        .keys()
        .filter(|(k, _)| *k == IgnoreKind::Panic)
        .count()
}

/// The command that re-pins the snapshot — quoted by the ratchet's read-failure message.
const REPIN_HINT: &str = "deno task ignore:audit:update";

/// The `#`-comment header the snapshot file opens with — machine-generated, do NOT hand-edit.
const SNAPSHOT_HEADER: &str = "# Generated by `deno task ignore:audit:update` — do NOT hand-edit.\n\
     #\n\
     # Every line is a KNOWN BUG at an AST position (`{parent}.{field}`), one line per graded\n\
     # check an injected `// prettier-ignore` fails there:\n\
     #\n\
     #   UNHONORED       — an own-line directive is NOT honored: the formatter reformats the\n\
     #                     ignored node instead of emitting it verbatim (the drop-in contract\n\
     #                     violation).\n\
     #   TRAILING_FROZEN — a directive TRAILING the preceding line (content before it on its\n\
     #                     line) freezes the node anyway, violating the decided inert-placement\n\
     #                     floor.\n\
     #   OVERFROZEN      — MORE than the ignored node is frozen: a doubled structural space\n\
     #                     outside the node survives the format (over-freezing otherwise reads\n\
     #                     as \"honored\").\n\
     #   UNSTABLE        — the injection's output is not a format fixed point: pass 2 changes\n\
     #                     it (the freeze-on-pass-1 / inert-on-pass-2 relocation class).\n\
     #\n\
     # The gate fails on a line that is NOT here (a newly-discovered finding), on a line here\n\
     # that no longer fires (a stale entry — delete it when you fix the position), and on any\n\
     # PANIC (never listed; that invariant is absolute).\n\
     #\n\
     # Counts are deliberately not pinned: they churn with every ordinary fixture PR. Positions\n\
     # ARE pinned (this is a ratchet over a live bug family, born red).\n\
     #\n\
     # Format: KIND<TAB>POSITION\n";

/// Where the committed shape snapshot lives — the ratchet `deno task check` gates on. Colocated
/// with this module, read at runtime by the [`Ratchet`].
fn known_shapes_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/commands/ignore_audit_known.txt")
}

/// The ratchet over [`known_shapes_path`], carrying this audit's header + re-pin hint.
fn ratchet() -> Ratchet {
    Ratchet::new(known_shapes_path(), SNAPSHOT_HEADER, REPIN_HINT)
}

/// One snapshot line: `KIND<TAB>POSITION`.
///
/// [`IgnoreKind`] leads the key, so its derived [`Ord`] matches the `shapes` map's order — the
/// snapshot renders in exactly that order, a stable minimal-diff file.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct IgnoreKey {
    kind: IgnoreKind,
    shape: String,
}

impl SnapshotKey for IgnoreKey {
    fn to_line(&self) -> String {
        format!("{}\t{}", self.kind.label(), self.shape)
    }

    fn from_line(line: &str) -> Option<Self> {
        let mut cols = line.split('\t');
        let kind = IgnoreKind::from_label(cols.next()?)?;
        let shape = cols.next()?.to_string();
        Some(Self { kind, shape })
    }

    fn is_pinnable(&self) -> bool {
        is_pinnable(self.kind)
    }
}

/// The graded shapes as [`IgnoreKey`]s — the set the [`Ratchet`] sees (includes the unpinnable
/// `PANIC` ones; the ratchet splits those off via [`SnapshotKey::is_pinnable`]).
fn snapshot_keys(shapes: &BTreeMap<(IgnoreKind, String), ShapeAgg>) -> BTreeSet<IgnoreKey> {
    shapes
        .keys()
        .map(|(kind, shape)| IgnoreKey {
            kind: *kind,
            shape: shape.clone(),
        })
        .collect()
}

/// One reproducible instance of a position — kept as the single smallest by `(path, offset)`
/// (an [`ExampleSet`] at `N = 1`), so the chosen example is thread-count independent.
#[derive(Clone)]
struct Example {
    path: String,
    /// The byte offset in the seed the node begins at — the injection point.
    offset: usize,
    node_type: String,
    snippet: String,
}

impl ExampleOrd for Example {
    fn sort_key(&self) -> (&str, usize) {
        (&self.path, self.offset)
    }
}

/// Everything a position accumulates. Counts stay exact; only the single smallest example is kept.
#[derive(Default)]
struct ShapeAgg {
    count: usize,
    /// Distinct seed files the position fired in.
    files: BTreeSet<String>,
    /// The smallest example by `(path, offset)`.
    examples: ExampleSet<Example, 1>,
}

/// One thread's slice of the work.
#[derive(Default)]
struct Tally {
    shapes: BTreeMap<(IgnoreKind, String), ShapeAgg>,
    candidates: usize,
    injections: usize,
    honored: usize,
    /// Injections at an UNHONORED position — the ratcheted findings, counted for the summary line
    /// (only the UNHONORED-kind entries of `shapes` sum to this).
    unhonored: usize,
    /// Honoring/scope/trailing mutants that did not parse/format — the offset named no valid
    /// directive position. (A stability-check rejection is graded UNSTABLE instead: the accepted
    /// output failing to re-parse is definitionally not a fixed point.)
    rejected: usize,
    /// Second-pass stability checks run (= span-maximal primary injections that formatted).
    stability_checks: usize,
    unstable: usize,
    /// Scope (sibling-control) checks run — span-maximal primary-accepted injections with at
    /// least one doubleable structural space outside the node.
    sibling_checks: usize,
    overfrozen: usize,
    /// Trailing-inertness injections attempted (the placement-eligible subset of the span-maximal
    /// injections).
    trailing_injections: usize,
    trailing_frozen: usize,
    files_done: usize,
    parse_skipped: usize,
    /// Files not a clean format fixed point AS AUTHORED (or already directive-bearing) — reported
    /// and skipped, exact count + bounded path sample. Over `tests/fixtures` these are the
    /// variant / unformatted / format-ignore files.
    not_clean: CappedPaths,
}

impl Tally {
    fn record(&mut self, kind: IgnoreKind, cand: &Candidate, path: &str, source: &str) {
        let example = Example {
            path: path.to_string(),
            offset: cand.start,
            node_type: cand.node_type.clone(),
            snippet: snippet(source, cand.start),
        };
        let e = self.shapes.entry((kind, cand.shape.clone())).or_default();
        e.count += 1;
        e.files.insert(path.to_string());
        e.examples.offer(example);
    }

    /// Bump the graded kind's summary counter and record the finding — the paired bookkeeping
    /// every check performs, centralized so the kind→counter mapping can't drift per call site
    /// (the exhaustive match forces an update when a kind is added).
    fn count_and_record(&mut self, kind: IgnoreKind, cand: &Candidate, path: &str, source: &str) {
        match kind {
            IgnoreKind::Unhonored => self.unhonored += 1,
            IgnoreKind::TrailingFrozen => self.trailing_frozen += 1,
            IgnoreKind::Overfrozen => self.overfrozen += 1,
            IgnoreKind::Unstable => self.unstable += 1,
            // A panic has no paired counter — the shape map alone carries it (unpinnable class),
            // so recording is the whole bookkeeping. Call sites use `record` directly for it.
            IgnoreKind::Panic => {}
        }
        self.record(kind, cand, path, source);
    }

    fn record_not_clean(&mut self, display: String) {
        self.not_clean.push(display);
    }

    fn merge(&mut self, other: Tally) {
        self.candidates += other.candidates;
        self.injections += other.injections;
        self.honored += other.honored;
        self.unhonored += other.unhonored;
        self.rejected += other.rejected;
        self.stability_checks += other.stability_checks;
        self.unstable += other.unstable;
        self.sibling_checks += other.sibling_checks;
        self.overfrozen += other.overfrozen;
        self.trailing_injections += other.trailing_injections;
        self.trailing_frozen += other.trailing_frozen;
        self.files_done += other.files_done;
        self.parse_skipped += other.parse_skipped;
        self.not_clean.merge(other.not_clean);
        for (k, v) in other.shapes {
            match self.shapes.get_mut(&k) {
                Some(e) => {
                    e.count += v.count;
                    e.files.extend(v.files);
                    e.examples.merge(v.examples);
                }
                None => {
                    self.shapes.insert(k, v);
                }
            }
        }
    }
}

/// A candidate node position: a JS node that leads its line and can be perturbed.
struct Candidate {
    /// Byte span of the node in the seed source.
    start: usize,
    end: usize,
    /// Byte offset of the start of the node's line — where the directive line is inserted.
    line_start: usize,
    /// The AST position key: `{parent-type}.{child-field}`.
    shape: String,
    /// The node's own type (for the example / triage; not part of the ratchet key).
    node_type: String,
}

/// Whether byte offset `p` falls inside any span in `spans` (sorted or not).
fn in_any_span(p: usize, spans: &[(usize, usize)]) -> bool {
    spans.iter().any(|&(a, b)| a <= p && p < b)
}

/// Whether a wire node is a `TSUnionType`/`TSIntersectionType`, looking through
/// `TSParenthesizedType` shells — the directive binding is paren-transparent, so a
/// parenthesized composite declines injection the same way a bare one does.
fn is_composite_type(node: &Value, nt: &str) -> bool {
    if matches!(nt, "TSUnionType" | "TSIntersectionType") {
        return true;
    }
    let mut cur = node;
    loop {
        match cur.get("type").and_then(Value::as_str) {
            Some("TSParenthesizedType") => match cur.get("typeAnnotation") {
                Some(inner) => cur = inner,
                None => return false,
            },
            Some("TSUnionType" | "TSIntersectionType") => return true,
            _ => return false,
        }
    }
}

/// The node's slice with every **structural** space (a `' '` outside `exclusions`) doubled, or
/// `None` when there is nothing to double — a no-op perturbation would make honoring untestable
/// (the slice survives both honoring and reformatting), a masked miss, so such a node is skipped.
fn perturb(
    source: &str,
    start: usize,
    end: usize,
    exclusions: &[(usize, usize)],
) -> Option<String> {
    let slice = &source[start..end];
    let mut out = String::with_capacity(slice.len() + 8);
    let mut changed = false;
    for (rel, ch) in slice.char_indices() {
        out.push(ch);
        if ch == ' ' && !in_any_span(start + rel, exclusions) {
            out.push(' ');
            changed = true;
        }
    }
    changed.then_some(out)
}

/// The outcome of one panic-safe format of a mutant.
enum FormatOutcome {
    /// The formatter crashed — an [`IgnoreKind::Panic`] wherever it happens.
    Panicked,
    /// The mutant did not parse/format — the offset named no valid directive position.
    Rejected,
    Output(String),
}

/// Format `mutant` panic-safely, draining any comment-ledger state the format left (armed
/// globally) so it can't grow unbounded.
fn format_checked(mutant: &str, parser: ParserType) -> FormatOutcome {
    let formatted = std::panic::catch_unwind(AssertUnwindSafe(|| format_source(mutant, parser)));
    #[cfg(feature = "comment_check")]
    let _ = comment_ledger::take_comment_ledger();
    match formatted {
        Err(_) => FormatOutcome::Panicked,
        Ok(Err(_)) => FormatOutcome::Rejected,
        Ok(Ok(output)) => FormatOutcome::Output(output),
    }
}

/// The read-only inputs the mutant builders share — bundled so the two identically-typed span
/// slices can't be transposed at a call site (the same argument-budget move as [`Walk`]).
struct MutantCtx<'a> {
    source: &'a str,
    /// JS code regions — a space outside them is markup, never doubled.
    regions: &'a [(usize, usize)],
    /// String/template/comment/regex interiors — content, never doubled, never an append point.
    exclusions: &'a [(usize, usize)],
}

/// Append `ctx.source[from..to]` to `out`, doubling every structural space — a `' '` inside a JS
/// region and outside the exclusions — and reporting via the return whether anything was doubled.
/// The sibling-mutant builder's segment primitive: markup (out-of-region), strings, templates,
/// comments, and regex bodies keep their spaces single.
fn push_doubled(out: &mut String, ctx: &MutantCtx<'_>, from: usize, to: usize) -> bool {
    let mut changed = false;
    for (rel, ch) in ctx.source[from..to].char_indices() {
        out.push(ch);
        let p = from + rel;
        if ch == ' ' && in_any_span(p, ctx.regions) && !in_any_span(p, ctx.exclusions) {
            out.push(' ');
            changed = true;
        }
    }
    changed
}

/// The scope-check (sibling-control) mutant: the primary injection with every structural space
/// OUTSIDE the candidate node ALSO doubled. If the freeze is scoped exactly to the node, every
/// outside doubling normalizes and the output is byte-identical to the primary injection's; a
/// surviving doubled space means the freeze over-extends ([`IgnoreKind::Overfrozen`]). `None`
/// when nothing outside the node is doubleable (the check would be vacuous — the comparison
/// holds trivially).
fn sibling_mutant(
    ctx: &MutantCtx<'_>,
    cand: &Candidate,
    perturbed: &str,
    indent: &str,
) -> Option<String> {
    let mut out = String::with_capacity(ctx.source.len() * 2 + DIRECTIVE.len() + 16);
    let mut changed = push_doubled(&mut out, ctx, 0, cand.line_start);
    out.push_str(indent);
    out.push_str(DIRECTIVE);
    changed |= push_doubled(&mut out, ctx, cand.line_start, cand.start);
    out.push_str(perturbed);
    changed |= push_doubled(&mut out, ctx, cand.end, ctx.source.len());
    changed.then_some(out)
}

/// The characters a preceding line may END with that make a trailing directive FORWARD-BINDING
/// rather than inert: after an opening delimiter there is no preceding sibling on the line, and
/// both tools bind the directive to the FIRST member (`{ // prettier-ignore` freezes the first
/// statement/property — prettier relocates the directive own-line, tsv preserves its position).
/// The trailing-inertness check skips these placements; everything else — separators, complete
/// statements, a trailing `=` (the fixture-pinned after-equals decision) — asserts inert.
const FORWARD_BINDING_LINE_ENDS: [char; 4] = ['{', '[', '(', '<'];

/// The trailing-inertness mutant: the directive appended to the END of the line preceding the
/// candidate (` // prettier-ignore`), a placement the decided floor classifies as INERT (content
/// before a directive on its line ⇒ inert; a `//` comment can never be glued forward). The
/// perturbed node must therefore NOT survive — if it does, the position misbinds a trailing
/// directive ([`IgnoreKind::TrailingFrozen`]). `None` when the placement is ineligible:
///
/// - no preceding line, or a blank one (the directive would be own-line — the primary check's
///   placement class, not this one's);
/// - the preceding line ends with an opening delimiter ([`FORWARD_BINDING_LINE_ENDS`]);
/// - the preceding line is not wholly inside one JS region (appending `//` into markup or a
///   `<script>` tag line would not lex as a JS comment);
/// - an exclusion span covers the append point — inside a string/template/comment. A line
///   comment's span ends exactly at EOL, so appending there would extend that comment's text,
///   not create a directive; the `<= ce` below catches it. (A block comment or string ending
///   exactly at EOL is also skipped — over-conservative but safe: a missed test, never a wrong
///   verdict.)
///
/// Assumes LF line endings: the append lands at the byte before the `\n`, which for a CRLF line
/// would strand the `\r` mid-line. Guaranteed by the pristine fixed-point gate — the formatter
/// emits LF only, so a CRLF seed never reaches injection.
fn trailing_mutant(ctx: &MutantCtx<'_>, cand: &Candidate, perturbed: &str) -> Option<String> {
    if cand.line_start == 0 {
        return None;
    }
    let source = ctx.source;
    let eol = cand.line_start - 1;
    let prev_line_start = source[..eol].rfind('\n').map_or(0, |i| i + 1);
    let prev_content = source[prev_line_start..eol].trim();
    let last = prev_content.chars().last()?;
    if FORWARD_BINDING_LINE_ENDS.contains(&last) {
        return None;
    }
    if !ctx
        .regions
        .iter()
        .any(|&(a, b)| a <= prev_line_start && eol < b)
    {
        return None;
    }
    if ctx
        .exclusions
        .iter()
        .any(|&(cs, ce)| cs <= eol && eol <= ce)
    {
        return None;
    }
    let mut out = String::with_capacity(source.len() + DIRECTIVE.len() + 16);
    out.push_str(&source[..eol]);
    out.push(' ');
    out.push_str(DIRECTIVE.trim_end());
    out.push_str(&source[eol..cand.start]);
    out.push_str(perturbed);
    out.push_str(&source[cand.end..]);
    Some(out)
}

/// The per-file walk context — bundles the read-only lookups and the growing candidate list so the
/// recursive walk stays within the argument budget.
struct Walk<'a> {
    map: &'a Utf16ToByte,
    source: &'a str,
    regions: &'a [(usize, usize)],
    out: Vec<Candidate>,
    /// `(line_start, end)` of composite (Union/Intersection) nodes that passed every candidate
    /// filter but were skipped by the composite rule — kept so the line-maximality map (see
    /// [`audit_file`]) still knows the outermost node beginning on their line: a directive above
    /// a union-valued list item binds to the ITEM (the printer freezes the whole member), so the
    /// item's first member — which begins on the same line — must not run the companion checks
    /// against its own narrower span.
    skipped_composites: Vec<(usize, usize)>,
}

impl Walk<'_> {
    /// Walk the wire tree collecting every candidate node keyed to its `{parent}.{field}` position.
    ///
    /// `parent` carries the enclosing typed node's type and the field the current subtree hangs off,
    /// so a typed node found under it is a candidate at that position. Descending into a typed node
    /// resets `parent` for its own children; an array or untyped container passes `parent` through
    /// (an array element sits at the same position as the array).
    fn collect(&mut self, node: &Value, parent: Option<(&str, &str)>) {
        match node {
            Value::Object(obj) => {
                if let Some(nt) = obj.get("type").and_then(Value::as_str) {
                    if let Some((pt, field)) = parent {
                        self.consider(node, nt, pt, field);
                    }
                    for (k, v) in obj {
                        if is_non_structural_key(k) {
                            continue;
                        }
                        self.collect(v, Some((nt, k)));
                    }
                } else {
                    // An untyped container (e.g. a Svelte spanless wrapper) — recurse through it,
                    // keeping the position of whatever encloses it.
                    for v in obj.values() {
                        self.collect(v, parent);
                    }
                }
            }
            Value::Array(items) => {
                for v in items {
                    self.collect(v, parent);
                }
            }
            _ => {}
        }
    }

    /// Whether a typed `node` at position `{pt}.{field}` is an injectable candidate: inside a JS
    /// region and leading its own line (modulo a single leading `|`/`&` union/intersection
    /// separator, so the directive binds to it).
    fn consider(&mut self, node: &Value, nt: &str, pt: &str, field: &str) {
        let Some((s, e)) = self.map.node_byte_span(node) else {
            return;
        };
        if e <= s || e > self.source.len() {
            return;
        }
        // Trim trailing whitespace to match `raw_source_range` (some wire spans over-extend past
        // the node into the next line's indentation), so an honored node's verbatim slice matches.
        let e = s + self.source[s..e].trim_end().len();
        if e <= s {
            return;
        }
        // The node's whole span must sit inside a JS region (so a TS `//` directive is the right one).
        if !self.regions.iter().any(|&(a, b)| a <= s && e <= b) {
            return;
        }
        // Lead-its-line: everything on the node's line before it is indentation, optionally one
        // leading break operator (`LINE_LEAD_OPERATORS`). Otherwise the directive binds to a sibling.
        let line_start = self.source[..s].rfind('\n').map_or(0, |i| i + 1);
        let stripped = self.source[line_start..s].trim();
        if !(stripped.is_empty() || LINE_LEAD_OPERATORS.contains(&stripped)) {
            return;
        }
        // A (paren-transparently) Union/Intersection candidate is skipped: at a
        // single-child head, the decided semantics for an own-line directive before a
        // composite are the MEMBER rules — Rule A freezes only the FIRST member
        // (prettier's own `types[0]` redirect agrees) — so this audit's whole-node
        // substring check is the wrong expectation there by design, and the
        // member-level positions (`TSUnionType.types`, `TSIntersectionType.types`)
        // already carry those placements. Without the skip, every single-child
        // position whose fixture sites hold union-valued children (annotation, alias
        // RHS, …) would stay pinned forever on decided behavior. The skip is keyed on
        // the candidate's own type, not its position, so it ALSO drops a union-typed
        // LIST member (a `[A | B, c]` tuple element) — there the printer freezes the
        // whole member (the deliberate Rule-A list-item scope; a cataloged divergence
        // from prettier's `types[0]` redirect), so the whole-node check WOULD have
        // been right, but the honoring signal rides on the member-freeze fixtures
        // instead. The skipped span still feeds the span-maximality map so the
        // union's own first member doesn't run companion checks against a narrower
        // span than the decided item-level freeze.
        if is_composite_type(node, nt) {
            self.skipped_composites.push((line_start, e));
            return;
        }
        self.out.push(Candidate {
            start: s,
            end: e,
            line_start,
            shape: format!("{pt}.{field}"),
            node_type: nt.to_string(),
        });
    }
}

/// Audit one file: verify it is a clean fixed point AS AUTHORED, then inject a directive before
/// every candidate node and assert the perturbed node survives verbatim.
fn audit_file(path: &Path, tally: &mut Tally) {
    let display = path.to_string_lossy().into_owned();
    if is_input_invalid_fixture(path) {
        return;
    }
    let Ok(source) = std::fs::read_to_string(path) else {
        return;
    };
    let parser = ParserType::from_extension(&display);
    // CSS uses a different directive spelling — a deliberate follow-up (see the module docs).
    if parser == ParserType::Css {
        return;
    }
    // A seed already bearing an ignore directive is exempt (injected + pre-existing interaction).
    if source_has_ignore_directive(&source) {
        return;
    }

    // Pristine 1/2 — ledger-clean, and capture the seed's own comment spans (a comment interior is
    // excluded from perturbation: a formatter preserves comment text, so a doubled space there
    // would survive reformatting and read as a false "honored").
    let (comment_spans, pristine_output) = match pristine_format(&source, parser) {
        Pristine::Skip { dirty: false } => {
            tally.parse_skipped += 1;
            return;
        }
        Pristine::Skip { dirty: true } => {
            tally.record_not_clean(display);
            return;
        }
        Pristine::Clean {
            comment_spans,
            output,
        } => (comment_spans, output),
    };
    // Pristine 2/2 — a TRUE fixed point AS AUTHORED (`format(source) == source`, read off the
    // output the pristine format above already computed — no second format). Stricter than
    // `f1_check`'s "idempotent after the first pass", which every `unformatted_*` variant fixture
    // also satisfies: the audit needs the node's canonical form to BE its source form, so honoring
    // (the perturbed slice survives verbatim) and reformatting (the perturbation collapses) are
    // cleanly distinguishable — a non-canonical seed muddies that. Being a fixed point also proves
    // the source reparses and that formatting corrupts nothing (output == input).
    if pristine_output != source {
        tally.record_not_clean(display);
        return;
    }

    let Some(wire) = tsv_parse_to_value(&source, parser) else {
        tally.record_not_clean(display);
        return;
    };
    let map = Utf16ToByte::new(&source);

    // Perturbation exclusions: the seed's comment interiors PLUS its string / template interiors
    // (a doubled space there is content, preserved by both honoring and reformatting).
    let mut exclusions: Vec<(usize, usize)> = comment_spans
        .iter()
        .map(|s| (s.start as usize, s.end as usize))
        .collect();
    exclusions.extend(
        string_and_template_spans(&source, &wire)
            .iter()
            .map(|s| (s.start as usize, s.end as usize)),
    );
    // Regex bodies too: a regex's spaces are pattern CONTENT, preserved by any formatter, so a
    // doubled space there survives reformatting and reads as a false "preserved" (masking a real
    // miss in the honoring check, or a false OVERFROZEN in the scope check).
    exclusions.extend(
        regex_literal_spans(&source, &wire)
            .iter()
            .map(|s| (s.start as usize, s.end as usize)),
    );

    let regions = code_regions(&source, parser);
    let mut walk = Walk {
        map: &map,
        source: &source,
        regions: &regions,
        out: Vec::new(),
        skipped_composites: Vec::new(),
    };
    walk.collect(&wire, None);
    let candidates = walk.out;
    tally.candidates += candidates.len();
    tally.files_done += 1;

    let maximal = maximal_by_line(&candidates, &walk.skipped_composites);
    let ctx = MutantCtx {
        source: &source,
        regions: &regions,
        exclusions: &exclusions,
    };

    let mut mutant = String::with_capacity(source.len() + DIRECTIVE.len() + 16);
    for (cand_index, cand) in candidates.iter().enumerate() {
        // The perturbed node — skip a node with no structural space (untestable, see `perturb`).
        let Some(perturbed) = perturb(&source, cand.start, cand.end, &exclusions) else {
            continue;
        };
        tally.injections += 1;

        // Build the mutant: the directive on its own line above the node's line (at the node's
        // indent, which `source[line_start..start]` already carries), then the perturbed node.
        let indent_end = source[cand.line_start..cand.start]
            .find(|c: char| !c.is_whitespace())
            .map_or(cand.start - cand.line_start, |off| off);
        let indent = &source[cand.line_start..cand.line_start + indent_end];
        mutant.clear();
        mutant.push_str(&source[..cand.line_start]);
        mutant.push_str(indent);
        mutant.push_str(DIRECTIVE);
        mutant.push_str(&source[cand.line_start..cand.start]);
        mutant.push_str(&perturbed);
        mutant.push_str(&source[cand.end..]);

        // Check 1 — honoring (panic-safe: production WASM is panic=abort).
        let primary_output = match format_checked(&mutant, parser) {
            FormatOutcome::Panicked => {
                tally.record(IgnoreKind::Panic, cand, &display, &source);
                // A crashing injection gets no companion checks — fix the crash first.
                continue;
            }
            FormatOutcome::Rejected => {
                tally.rejected += 1;
                None
            }
            FormatOutcome::Output(output) => {
                if output.contains(&perturbed) {
                    tally.honored += 1;
                } else {
                    tally.count_and_record(IgnoreKind::Unhonored, cand, &display, &source);
                }
                Some(output)
            }
        };

        if !maximal[cand_index] {
            continue;
        }

        if let Some(primary_output) = &primary_output {
            // Check 2 — second-pass stability: the accepted output must be a fixed point. A
            // pass-2 rejection (the output no longer parses) is definitionally not one either.
            tally.stability_checks += 1;
            match format_checked(primary_output, parser) {
                FormatOutcome::Panicked => tally.record(IgnoreKind::Panic, cand, &display, &source),
                FormatOutcome::Output(ref second) if second == primary_output => {}
                _ => tally.count_and_record(IgnoreKind::Unstable, cand, &display, &source),
            }

            // Check 3 — scope (sibling control): doubling OUTSIDE the node must all normalize.
            if let Some(sibling) = sibling_mutant(&ctx, cand, &perturbed, indent) {
                tally.sibling_checks += 1;
                match format_checked(&sibling, parser) {
                    FormatOutcome::Panicked => {
                        tally.record(IgnoreKind::Panic, cand, &display, &source);
                    }
                    FormatOutcome::Rejected => tally.rejected += 1,
                    FormatOutcome::Output(output) => {
                        if output != *primary_output {
                            tally.count_and_record(IgnoreKind::Overfrozen, cand, &display, &source);
                        }
                    }
                }
            }
        }

        // Check 4 — trailing inertness: a trailing directive must freeze nothing.
        if let Some(trailing) = trailing_mutant(&ctx, cand, &perturbed) {
            tally.trailing_injections += 1;
            match format_checked(&trailing, parser) {
                FormatOutcome::Panicked => tally.record(IgnoreKind::Panic, cand, &display, &source),
                FormatOutcome::Rejected => tally.rejected += 1,
                FormatOutcome::Output(output) => {
                    if output.contains(&perturbed) {
                        tally.count_and_record(IgnoreKind::TrailingFrozen, cand, &display, &source);
                    }
                }
            }
        }
    }
}

/// Which candidates are span-maximal on their line — the ones the companion checks run on.
///
/// The companion checks (stability / scope / trailing) run only on the SPAN-MAXIMAL node
/// beginning on each LINE: the directive is inserted above the whole line, so it binds to the
/// OUTERMOST construct beginning there (a statement-head directive ignores the whole statement,
/// in both tools; an alias-head directive above `| {a} … extends U ? 1 : 2` freezes the whole
/// conditional the union checks) — a narrower candidate sharing the line (a nested same-start
/// node, or a first member after a lead operator) would grade that decided wider freeze as a
/// finding (scope) or duplicate the outer result under a nested key (stability / trailing). The
/// honoring check stays per-candidate — a nested slice surviving inside the wider freeze is
/// exactly what "honored" means there. Skipped composites participate so a union-valued list
/// item's first member defers to the item-level freeze scope. Ties keep the first-collected
/// candidate (the walk visits parents before children, so the outer position keys the companion
/// findings).
fn maximal_by_line(candidates: &[Candidate], skipped_composites: &[(usize, usize)]) -> Vec<bool> {
    let mut maximal = vec![false; candidates.len()];
    let mut best: BTreeMap<usize, (usize, Option<usize>)> = BTreeMap::new();
    for (i, c) in candidates.iter().enumerate() {
        let e = best.entry(c.line_start).or_insert((c.end, Some(i)));
        if c.end > e.0 {
            *e = (c.end, Some(i));
        }
    }
    for &(line_start, e) in skipped_composites {
        let entry = best.entry(line_start).or_insert((e, None));
        if e > entry.0 {
            *entry = (e, None);
        }
    }
    for (_, (_, i)) in best {
        if let Some(i) = i {
            maximal[i] = true;
        }
    }
    maximal
}

impl IgnoreAuditCommand {
    /// The flags that make this run reach a shape set OTHER than the one the snapshot describes.
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
            "the directive payload over tests/fixtures",
            "SUBSET",
        )?;
        let files = resolve_seed_files(&self.paths, self.limit)?;

        // Armed so `pristine_format`'s dirty check works; this audit reads no ledger findings.
        let armed = ArmedRun::arm(false);
        let total = run_pool(&files, self.jobs, audit_file, Tally::merge)?;
        drop(armed);

        if self.update {
            ratchet().write_pinned(&snapshot_keys(&total.shapes), "position")?;
            print_companions(&total);
            report_not_clean(&total, false, true);
            return report_unpinned_panics(
                count_panics(&total.shapes),
                "position",
                "an injected directive",
            );
        }

        // Grade only on the full default corpus — the snapshot describes exactly that.
        let graded = if default_paths && narrowed.is_empty() {
            Some(ratchet().grade(&snapshot_keys(&total.shapes))?)
        } else {
            None
        };

        let (summary, findings) = build_report(&total);
        if self.json {
            report::print_json(&summary, &findings, &companion_extras(&total));
        } else if graded.as_ref().is_some_and(GateDiff::holds) && !self.report {
            // A passing gate is summary-only — the pinned positions are noise in `deno task check`.
            report::print_summary(&summary, &findings);
        } else {
            report::print_report(&summary, &findings);
        }
        if !self.json {
            print_companions(&total);
        }
        report_not_clean(&total, self.json, self.report || !default_paths);

        // Off the default corpus the snapshot doesn't apply — every finding is news.
        if !default_paths {
            return if total.shapes.is_empty() {
                Ok(())
            } else {
                Err(CliError::Failed)
            };
        }
        if !narrowed.is_empty() {
            print_ratchet_skipped(&narrowed);
            return Ok(());
        }
        match &graded {
            Some(diff) => self.report_gate(diff, &total),
            None => Ok(()),
        }
    }

    /// Report a [`GateDiff`] and turn it into an exit status.
    fn report_gate(&self, diff: &GateDiff<IgnoreKey>, total: &Tally) -> Result<(), CliError> {
        let panics: Vec<_> = total
            .shapes
            .iter()
            .filter(|((k, _), _)| *k == IgnoreKind::Panic)
            .collect();
        if !panics.is_empty() {
            eprintln!(
                "\n✗ {} position(s) CRASH the formatter on an injected directive — not pinnable \
                 and not a ratchet entry: fix the crash.",
                panics.len()
            );
            for ((_, shape), agg) in panics.iter().take(40) {
                let ex = agg.examples.canonical();
                eprintln!("    {shape:<28} e.g. {}:{}", ex.path, ex.offset);
            }
        }
        if !diff.new.is_empty() {
            eprintln!(
                "\n✗ {} NEW finding(s) — a `// prettier-ignore` here fails a graded check \
                 (honoring / stability / scope / trailing inertness) the snapshot has never \
                 seen it fail:",
                diff.new.len()
            );
            for k in diff.new.iter().take(40) {
                eprintln!("    {:<12} {}", k.kind.label(), k.shape);
            }
            if diff.new.len() > 40 {
                eprintln!("    … and {} more", diff.new.len() - 40);
            }
            eprintln!(
                "  Fix the position (printer opt-in for UNHONORED, the misbinding for \
                 TRAILING_FROZEN, scope narrowing for OVERFROZEN, the relocation transient for \
                 UNSTABLE), or — if it is genuinely pre-existing and merely newly REACHED by a \
                 fixture — re-run `{REPIN_HINT}`."
            );
        }
        if !diff.stale.is_empty() {
            eprintln!(
                "\n✗ {} STALE snapshot entry/entries — these position/check pairs no longer \
                 fire. Drop the lines (`{REPIN_HINT}`):",
                diff.stale.len()
            );
            for k in diff.stale.iter().take(40) {
                eprintln!("    {:<12} {}", k.kind.label(), k.shape);
            }
            if diff.stale.len() > 40 {
                eprintln!("    … and {} more", diff.stale.len() - 40);
            }
        }
        if diff.holds() {
            println!(
                "\n✓ ratchet holds — every finding is a known gap ({} pinned); no directive \
                 position newly fails a graded check",
                diff.known
            );
            Ok(())
        } else {
            Err(CliError::Failed)
        }
    }
}

/// Translate a run's [`Tally`] into the shared reporting envelope (the `audit::report` printers —
/// worst-first ordering + a `--json` shape uniform with `gap_audit` / `blank_audit`). Every kind
/// maps on: `PANIC` is `GateFailing` (absolute); every graded kind (`UNHONORED` /
/// `TRAILING_FROZEN` / `OVERFROZEN` / `UNSTABLE`) is `Informational` (the ratchet decides
/// fatality). There is no report-only class, so every finding is `gated`.
fn build_report(total: &Tally) -> (RunSummary, Vec<Finding>) {
    let summary = RunSummary {
        audit: "ignore_audit",
        files_done: total.files_done,
        // The envelope's "sites" is this audit's candidate node positions; "accepted" is the
        // injections that formatted (honored + unhonored), the analog of blank's non-rejected set.
        sites: total.candidates,
        injections: total.injections,
        accepted: total.honored + total.unhonored,
        parse_skipped: total.parse_skipped,
        // ignore_audit reports its own not-clean bucket (with paths) via `report_not_clean`; the
        // envelope's dirty-file notice (a `comments:audit` overlap) is unused here.
        dirty_files: Vec::new(),
        payload_labels: vec!["prettier-ignore"],
    };
    let findings = total
        .shapes
        .iter()
        .map(|((kind, shape), agg)| {
            let ex = agg.examples.canonical();
            Finding {
                audit: "ignore_audit",
                severity: if *kind == IgnoreKind::Panic {
                    Severity::GateFailing
                } else {
                    Severity::Informational
                },
                confidence: None,
                site: shape.clone(),
                example: ReportExample {
                    payload: "prettier-ignore",
                    path: ex.path.clone(),
                    injection_offset: ex.offset,
                    attribution_offset: ex.offset,
                    snippet: ex.snippet.clone(),
                    text: "// prettier-ignore".to_string(),
                    injected: true,
                },
                verdict_string: String::new(),
                detail: Detail::Ignore(IgnoreDetail {
                    kind_label: kind.label(),
                    count: agg.count,
                    files: agg.files.len(),
                    node_type: ex.node_type.clone(),
                    gated: true,
                }),
            }
        })
        .collect();
    (summary, findings)
}

/// The companion-check counters as `--json` extras — the shared envelope's summary carries only
/// the primary-injection counts.
fn companion_extras(total: &Tally) -> serde_json::Map<String, Value> {
    let mut extras = serde_json::Map::new();
    extras.insert("stability_checks".into(), total.stability_checks.into());
    extras.insert("unstable".into(), total.unstable.into());
    extras.insert("sibling_checks".into(), total.sibling_checks.into());
    extras.insert("overfrozen".into(), total.overfrozen.into());
    extras.insert(
        "trailing_injections".into(),
        total.trailing_injections.into(),
    );
    extras.insert("trailing_frozen".into(), total.trailing_frozen.into());
    extras.insert("rejected".into(), total.rejected.into());
    extras
}

/// One line of companion-check accounting under the envelope summary (human output only — the
/// `--json` shape carries the same counts via [`companion_extras`]).
fn print_companions(total: &Tally) {
    println!(
        "○ companion checks — second-pass {}/{} unstable · scope {}/{} overfrozen · trailing \
         {}/{} frozen",
        total.unstable,
        total.stability_checks,
        total.overfrozen,
        total.sibling_checks,
        total.trailing_frozen,
        total.trailing_injections
    );
}

/// Print the "skipped — not a clean fixed point as authored" bucket. The COUNT always prints (a
/// coverage fact a graded gate must not silently drop); the sampled PATHS print only when
/// `show_paths` — over `tests/fixtures` the skips are the expected `unformatted_*` variants (pure
/// noise in `deno task check`), but over a real corpus (an explicit path / `--report`) they are the
/// triage list, matching `blank_audit`'s `report_not_clean`.
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
    let paths = if show_paths && !total.not_clean.sample().is_empty() {
        let sample: Vec<String> = total
            .not_clean
            .sample()
            .iter()
            .map(|p| format!("    {p}"))
            .collect();
        let more = total
            .not_clean
            .count()
            .saturating_sub(total.not_clean.sample().len());
        let tail = if more > 0 {
            format!("\n    … and {more} more")
        } else {
            String::new()
        };
        format!(":\n{}{tail}", sample.join("\n"))
    } else {
        String::new()
    };
    line(format!(
        "\n○ {} file(s) skipped — not a clean format fixed point AS AUTHORED (or already \
         directive-bearing). Over tests/fixtures this is expected (variant / unformatted / \
         format-ignore fixtures); over a real-code corpus each wants triage{paths}",
        total.not_clean.count()
    ));
}
