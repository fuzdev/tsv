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
//! Two kinds only: [`IgnoreKind::Unhonored`] (the position silently reformatted an ignored node)
//! and [`IgnoreKind::Panic`] (the injected directive crashed the formatter — production WASM is
//! `panic = "abort"`, so a crash is a DoS; NEVER pinnable, always fails the gate). This is a
//! deliberately **cheaper single-format check** than `blank_audit`'s F1 battery (see
//! `[[gap-audit-f1-cost-and-cheaper-alternative]]`): one format per injection, one substring test.
//!
//! ## The finding key — AST position, not a token shape
//!
//! Unlike `gap_audit` / `blank_audit` (which key by [`site_shape`], a flat token key), this audit
//! keys by the node's **AST position** — `{enclosing-node-type}.{child-field}`, e.g.
//! `TSUnionType.types`, `TSTupleType.elementTypes`, `Program.body`. Honoring is a per-*position*
//! property (a position either has the printer opt-in or it doesn't), so the ledger is a ledger of
//! **positions**, which is exactly what the plan's §1.3 wants. A covered position never produces a
//! finding (the perturbed slice survives), so the ledger names only the uncovered ones.
//!
//! ## Design
//!
//! Pure Rust, no sidecar, no new deps — the `fuzz` / `gap_audit` / `blank_audit` direction. Sites
//! come from a walk of the wire AST tree keyed to `code_regions` (the spans the AST says are JS),
//! so the perturbation lands only in JS. Each candidate node must (a) lie fully inside a JS region,
//! (b) **lead its own line** (modulo a single leading `|`/`&` union/intersection separator — so the
//! directive binds to it), and (c) have at least one **perturbable** structural space (a space
//! outside a string/template/comment interior). A node with none is skipped: it reformats to itself,
//! so honoring is untestable and uninteresting.
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
    code_regions, snippet, source_has_ignore_directive, string_and_template_spans,
};
use crate::audit::tally::CappedPaths;
use crate::cli::CliError;

use super::profile::{is_input_invalid_fixture, resolve_seed_files};

/// Inject a `// prettier-ignore` directive before every JS node and assert its perturbed source
/// survives verbatim.
///
/// For each seed file, at each candidate node position (one at a time), prepends the directive on
/// its own line and doubles the node's interior structural spaces, formats, and reports every
/// position whose perturbed slice does NOT survive (the ignore was not honored) or that crashes the
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
/// `Unhonored` is the ratcheted position ledger.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum IgnoreKind {
    /// The formatter crashed on the injected directive — NEVER pinnable, always fails the gate.
    Panic,
    /// The ignored node's perturbed source did NOT survive verbatim — the position silently
    /// reformatted an ignored construct (the drop-in `prettier-ignore` contract violation).
    Unhonored,
}

impl IgnoreKind {
    fn label(self) -> &'static str {
        match self {
            Self::Panic => "PANIC",
            Self::Unhonored => "UNHONORED",
        }
    }

    fn from_label(s: &str) -> Option<Self> {
        [Self::Panic, Self::Unhonored]
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
     # Every line is a KNOWN BUG: an AST position (`{parent}.{field}`) where a `// prettier-ignore`\n\
     # directive is NOT honored — the formatter reformats the ignored node instead of emitting it\n\
     # verbatim, breaking the drop-in prettier-ignore contract. The gate fails on a line that is NOT\n\
     # here (a newly-discovered unhonored position), on a line here that no longer fires (a stale\n\
     # entry — delete it when you add the printer opt-in), and on any PANIC.\n\
     #\n\
     # Counts are deliberately not pinned: they churn with every ordinary fixture PR. UNHONORED\n\
     # positions ARE pinned (this is a ratchet over a live bug family, born red); only a PANIC is\n\
     # never listed — that invariant is absolute, so it always fails the gate.\n\
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
    /// (the map's per-shape counts sum to this, minus any panics, which are their own class).
    unhonored: usize,
    /// Injections whose mutant did not parse/format — the offset named no valid directive position.
    rejected: usize,
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

    fn record_not_clean(&mut self, display: String) {
        self.not_clean.push(display);
    }

    fn merge(&mut self, other: Tally) {
        self.candidates += other.candidates;
        self.injections += other.injections;
        self.honored += other.honored;
        self.unhonored += other.unhonored;
        self.rejected += other.rejected;
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

/// The per-file walk context — bundles the read-only lookups and the growing candidate list so the
/// recursive walk stays within the argument budget.
struct Walk<'a> {
    map: &'a Utf16ToByte,
    source: &'a str,
    regions: &'a [(usize, usize)],
    out: Vec<Candidate>,
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

    let regions = code_regions(&source, parser);
    let mut walk = Walk {
        map: &map,
        source: &source,
        regions: &regions,
        out: Vec::new(),
    };
    walk.collect(&wire, None);
    let candidates = walk.out;
    tally.candidates += candidates.len();
    tally.files_done += 1;

    let mut mutant = String::with_capacity(source.len() + DIRECTIVE.len() + 16);
    for cand in &candidates {
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

        // Format once, panic-safe (production WASM is panic=abort).
        let formatted =
            std::panic::catch_unwind(AssertUnwindSafe(|| format_source(&mutant, parser)));
        // Drain any ledger state the format left (armed globally) so it can't grow unbounded.
        #[cfg(feature = "comment_check")]
        let _ = comment_ledger::take_comment_ledger();

        match formatted {
            Err(_) => tally.record(IgnoreKind::Panic, cand, &display, &source),
            // The mutant did not parse/format — the offset named no valid directive position.
            Ok(Err(_)) => tally.rejected += 1,
            Ok(Ok(output)) => {
                if output.contains(&perturbed) {
                    tally.honored += 1;
                } else {
                    tally.unhonored += 1;
                    tally.record(IgnoreKind::Unhonored, cand, &display, &source);
                }
            }
        }
    }
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
            report::print_json(&summary, &findings, &serde_json::Map::new());
        } else if graded.as_ref().is_some_and(GateDiff::holds) && !self.report {
            // A passing gate is summary-only — the pinned positions are noise in `deno task check`.
            report::print_summary(&summary, &findings);
        } else {
            report::print_report(&summary, &findings);
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
                "\n✗ {} NEW unhonored position(s) — a `// prettier-ignore` here is silently \
                 reformatted, and the snapshot has never seen it:",
                diff.new.len()
            );
            for k in diff.new.iter().take(40) {
                eprintln!("    {:<12} {}", k.kind.label(), k.shape);
            }
            if diff.new.len() > 40 {
                eprintln!("    … and {} more", diff.new.len() - 40);
            }
            eprintln!(
                "  Add the printer opt-in (the per-child ignore helper), or — if it is genuinely \
                 pre-existing and merely newly REACHED by a fixture — re-run `{REPIN_HINT}`."
            );
        }
        if !diff.stale.is_empty() {
            eprintln!(
                "\n✗ {} STALE snapshot entry/entries — these positions now honor the directive. \
                 Drop the lines (`{REPIN_HINT}`):",
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
                "\n✓ ratchet holds — every unhonored position is a known gap ({} pinned); no new \
                 directive position is silently reformatted",
                diff.known
            );
            Ok(())
        } else {
            Err(CliError::Failed)
        }
    }
}

/// Translate a run's [`Tally`] into the shared reporting envelope (the `audit::report` printers —
/// worst-first ordering + a `--json` shape uniform with `gap_audit` / `blank_audit`). Both kinds
/// map on: `UNHONORED` is `Informational` (the ratchet decides fatality), `PANIC` is `GateFailing`
/// (absolute). There is no report-only class, so every finding is `gated`.
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
