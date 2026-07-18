//! Gap-injection audit — the mechanized form of hunting the dropped-comment class.
//!
//! ## Why this exists
//!
//! One recurring bug class: **a printer that concatenates fixed pieces without scanning
//! the gaps between them silently DROPS any comment an author wrote in a gap.** A header
//! built as `d.text("import.source(")` scans neither of its two dot gaps, so
//! `import./* c */source(x)` loses the comment — no error, no diff, just gone.
//!
//! The print-once ledger ([`comment_ledger`]) would catch every one of these. It just
//! never sees them: [`comment_audit`](super::comment_audit) formats each file **as
//! authored**, and a gap only becomes a finding once a comment is actually *in* it. Eight
//! such sites were found BY HAND, and every one was green on every gate — `cargo test`,
//! `comments:audit`, `roundtrip:audit`, the corpus diff — purely because no fixture
//! happened to put a comment in that position. The gates were not wrong; the corpus was
//! silent.
//!
//! This audit closes that hole: for each seed file it injects a comment into **every**
//! candidate gap, one at a time, formats, and runs the ledger over the result.
//!
//! ## Design
//!
//! Pure Rust, no sidecar, no new deps — the [`fuzz`](super::fuzz) /
//! [`comment_audit`](super::comment_audit) direction. Deliberately **targeted, not
//! random**: byte mutation would essentially never synthesize a valid comment in a dot
//! gap, which is the whole point of the class.
//!
//! **Sites are byte offsets, not tokens.** A token-stream enumeration would need a flat
//! Svelte token contract that doesn't exist — and `.svelte` is where this class lives,
//! since TS-only syntax is fixtured as `.svelte` + `lang="ts"` (a TS-only audit reaches 53
//! of 6,689 fixture files). Worse, it would carry exactly the blind spot the class
//! exploits: a punctuator-joined header is a **zero-width** gap, the first thing a
//! "between two tokens" abstraction elides. A byte offset has no such notion, so it cannot
//! miss one.
//!
//! **But an offset must first be somewhere the payload IS a comment**, and tsv's own parser
//! cannot answer that. It is deliberately more permissive than the canonical one, so
//! "tsv accepted it" does not mean "an author could write it": tsv parses
//! `<script lang="ts"/* c */>` — which Svelte rejects outright — lexes the `/* c */` in the
//! tag as a comment, and drops it. Real content loss, but an *over-acceptance* bug, not
//! this class; and `/* … */` is not a comment in Svelte markup under any reading, so
//! injecting one there tests nothing while burying the report in shapes like `IDENT⟨⟩␣`.
//!
//! So sites come from [`code_regions`](crate::audit::sites::code_regions) — the spans the
//! AST says are JS or CSS — and inside those two existing layers filter for free:
//!
//! - **inside a word** (`fo/* c */o` → `fo o`) — the parser rejects it, so the site is
//!   skipped. Correctly: that gap exists in no real document.
//! - **inside a string literal** (`"fo/* c */o"`) — parses, but the injected text is never
//!   *lexed* as a comment, so the ledger registers nothing and reports nothing.
//!
//! One class those two miss is an offset **inside an existing comment** (`/* c1 ⟨⟩*/`): it
//! parses, lexes, and *does* register — but injecting there mutilates the author's comment
//! rather than probing a gap, and reads as a false drop. That one is not free;
//! [`injection_sites`](crate::audit::sites::injection_sites) excludes it explicitly from
//! the seed's own parsed comment spans, under every mode.
//!
//! And because the ledger asks only "was each comment printed exactly once?" — never "did
//! the layout change?" — an injection that legitimately reflows the file, or even changes
//! the program via ASI (`return// c⏎ x`), cannot produce a false positive. That is why the
//! oracle here is the ledger and not an output diff.
//!
//! ## Scope — what a green run does NOT prove
//!
//! Two limits compose, and neither is visible in a `✓`.
//!
//! The audit inherits **the ledger's scope** exactly. That scope now covers both the
//! **detached** comments a format entry registers AND the **AST-node** comments — a Svelte
//! `<!-- … -->` and a CSS in-block `CssBlockChild::Comment`, which the ledger registers by
//! span (see [`comment_ledger`]'s module docs). A CSS declaration's *value* comments are
//! still never lexed as `Comment`s at all — outside the model by construction. So this
//! speaks for both comment models — the detached class that bit us eight times and the
//! tree-carried AST-node one — but not for CSS values. CSS also has no line comments, so the
//! `line` payload is inert in a `.css` file (harmless: it simply never registers).
//!
//! It also inherits **[`code_regions`](crate::audit::sites::code_regions)' reach**: a gap
//! the region walk doesn't name is a gap never probed. Today that means a `.svelte` file's
//! `<style>` content is unprobed, so a Svelte file containing only a `<style>` block yields
//! **zero sites** — now a yield/cost call rather than a scope one (the ledger guards CSS
//! in-block comments), see that function's TODO.
//!
//! ## Structure
//!
//! Thin orchestration over the [`audit`](crate::audit) substrate: site enumeration and
//! shape keying live in [`audit::sites`](crate::audit::sites), the panic-safe ledger format
//! and verify verdicts in [`audit::properties`](crate::audit::properties), the snapshot
//! ratchet in [`audit::ratchet`](crate::audit::ratchet), and the reporting envelope +
//! printers in [`audit::report`](crate::audit::report). This module owns the command, the
//! per-file inject loop, the finding aggregation, and the gate/exit decision.
//!
//! ## Attribution — where a bystander finding is filed
//!
//! Every injection perturbs one gap, but the ledger reports each finding by its comment's span
//! in the **formatted input**. When the finding IS the injected comment (`injected`), that span
//! starts at the injection offset and the two coincide. When it is a **bystander** — a
//! pre-existing seed comment the injection knocked out — its span is somewhere else entirely,
//! and after a width flip can be lines away. So a hit carries two seed offsets: the **injection
//! offset** (what [`verify_example`] re-splices to reproduce the drop) and the **attribution
//! offset** (the victim comment's own seed site, [`victim_seed_offset`]-mapped back across the
//! splice). The shape, snippet, canonical sort, and `--by-node` emitter edge all key on the
//! attribution offset — so a dropped bystander points at the emitter that dropped it, not at the
//! perturbation site the payload went in at.

use argh::FromArgs;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use crate::audit::properties::{
    Formatted, Pristine, Verdict, VerifyOutcome, VerifySummary, ledger_format,
    ledger_format_with_comments, pristine_format, tsv_parse_to_value,
};
use crate::audit::ratchet::{GateDiff, Ratchet, SnapshotKey};
use crate::audit::report::{
    self, Confidence, Detail, Finding, GapDetail, ReportExample, RunSummary, Severity,
};
use crate::audit::sites::{
    NodeEdgeKey, code_regions, injection_sites, node_edge_key, site_shape, snippet,
};
use crate::cli::CliError;
use tsv_cli::cli::input::ParserType;
use tsv_lang::comment_ledger::{self, CommentFindingKind};

use super::profile::resolve_files;

/// Inject a comment into every gap and assert the print-once ledger still holds.
///
/// For each seed file, injects each payload at each candidate byte offset (one at a time),
/// formats, and reports every comment the format DROPPED or DOUBLE-PRINTED. Pure Rust — no
/// Deno. Defaults to `tests/fixtures`; the real yield is external corpora. Exits 1 on any
/// finding.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "gap_audit")]
#[allow(clippy::struct_excessive_bools)] // independent CLI flags
pub struct GapAuditCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// print the full per-shape report even when the ratchet holds. A passing gate is
    /// summary-only by default — ~700 shapes it already knows about is noise in
    /// `deno task check`. Any run with something to act on reports regardless
    #[argh(switch)]
    report: bool,

    /// after the run, also print a COARSE by-(node, edge) rollup of the finding shapes: a
    /// ranked emitter work-list keyed on the enclosing AST node + child-role edge, folding
    /// the ~700 fine token shapes into the few dozen printer clusters. A report-only view —
    /// it never changes the ratchet grade or the exit code
    #[argh(switch)]
    by_node: bool,

    /// inject at EVERY char boundary, including positions strictly inside a WORD.
    /// A diagnostic, not a stricter mode: it relaxes only the word-interior filter,
    /// and the extra shapes are artifacts of splitting a word (`desc/* c */ribe`),
    /// not gap bugs. Comment interiors stay excluded under every mode (see
    /// `injection_sites`)
    #[argh(switch)]
    all_bytes: bool,

    /// only inject this payload (block | line | jsdoc_cast | annotation | multiline);
    /// default: all five
    #[argh(option)]
    payload: Option<String>,

    /// worker threads (default: available parallelism). Each file's whole inject
    /// loop stays on one thread — the ledger is thread-local
    #[argh(option)]
    jobs: Option<usize>,

    /// cap the number of seed files (0 = unlimited)
    #[argh(option, default = "0")]
    limit: usize,

    /// rewrite the committed shape snapshot from this run. Only valid on a FULL default
    /// run — the snapshot describes every payload over `tests/fixtures` and nothing else,
    /// so any narrowing flag is refused rather than silently pinning a partial set
    #[argh(switch)]
    update: bool,

    /// seed file paths / directories (default: tests/fixtures)
    #[argh(positional)]
    paths: Vec<String>,
}

/// One injected comment shape.
///
/// Each drives a **distinct path** through the ownership model (root `CLAUDE.md` §Comment
/// Handling), so a drop can live on one and not the others — which is the whole reason the
/// payload set is plural rather than just a block comment.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Payload {
    /// A plain glued block comment — `owned_by_node`, printed by the innermost node its
    /// token begins rather than by the enclosing gap.
    Block,
    /// A line comment — never owned (`owned ⇒ is_block`), and rides the `line_suffix`
    /// hoist-to-EOL path instead.
    Line,
    /// A JSDoc type cast — the **one** remaining content sniff
    /// (`is_jsdoc_type_cast_comment`), which governs paren retention by building a
    /// `JsdocCast` node that prints the comment itself.
    JsdocCast,
    /// A bundler annotation — owned exactly like any other glued block comment (no sniff).
    /// Called out because losing one is silently *semantic*: the marked call stops being
    /// droppable.
    Annotation,
    /// A multi-line block comment — sets `Comment::multiline`, the precomputed flag every
    /// multi-line-block expansion gate reads.
    Multiline,
}

impl Payload {
    const ALL: [Payload; 5] = [
        Self::Block,
        Self::Line,
        Self::JsdocCast,
        Self::Annotation,
        Self::Multiline,
    ];

    /// The exact text injected.
    fn text(self) -> &'static str {
        match self {
            // A trailing newline, so the payload comments out only itself and not the
            // rest of the author's line — `x// c⏎ + 1` is a line comment in a gap, while
            // `x// c + 1` is a line comment that ate an operand.
            Self::Line => "// c\n",
            Self::Block => "/* c */",
            Self::JsdocCast => "/** @type {T} */",
            Self::Annotation => "/* @__PURE__ */",
            Self::Multiline => "/* a\nb */",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Line => "line",
            Self::JsdocCast => "jsdoc_cast",
            Self::Annotation => "annotation",
            Self::Multiline => "multiline",
        }
    }

    fn from_label(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.label() == s)
    }
}

/// The command that re-pins the snapshot — quoted by the ratchet's read-failure message.
const REPIN_HINT: &str = "deno task gaps:audit:update";

/// The `#`-comment header the snapshot file opens with — machine-generated, do NOT
/// hand-edit. Owned here (not by the [`Ratchet`]) because it documents *this* audit's
/// ratchet: what a line means, why counts aren't pinned, why a panic is never listed.
const SNAPSHOT_HEADER: &str = "# Generated by `deno task gaps:audit:update` — do NOT hand-edit.\n\
     #\n\
     # Every line is a KNOWN BUG: a site shape where injecting a comment makes the\n\
     # formatter drop or double-print one. The gate fails on a line that is NOT here\n\
     # (a new kind of drop) and on a line here that no longer fires (a stale entry —\n\
     # delete it when you fix one). Counts are deliberately not pinned: they churn with\n\
     # every ordinary fixture PR, and a gate that fails per added fixture gets turned\n\
     # off. The PAYLOAD set is pinned, though: a shape that starts dropping a comment\n\
     # kind it used to survive is a new bug on a new ownership path.\n\
     #\n\
     # A PANIC is never listed here — that invariant is absolute, so it always fails\n\
     # the gate rather than being pinned.\n\
     #\n\
     # Format: KIND<TAB>SHAPE<TAB>PAYLOADS\n";

/// Where the committed shape snapshot lives — the ratchet `deno task check` gates on.
///
/// The snapshot is **machine-generated** (`deno task gaps:audit:update`), unlike
/// [`scan_audit`](super::scan_audit)'s hand-curated `ALLOW`: at ~700 shapes a per-entry
/// rationale is not a thing a human can keep honest, so it deliberately carries none. It is
/// a ratchet, not a sanction — every line is a **known bug**, and the file shrinking is the
/// goal.
///
/// Colocated with this module so it travels with the code that owns it. The path is the
/// only compile-time piece (`CARGO_MANIFEST_DIR`); the file itself is read at runtime by the
/// [`Ratchet`] — see [`audit::ratchet`](crate::audit::ratchet) for why not `include_str!`.
fn known_shapes_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/commands/gap_audit_known.txt")
}

/// The ratchet over [`known_shapes_path`], carrying this audit's header + re-pin hint.
fn ratchet() -> Ratchet {
    Ratchet::new(known_shapes_path(), SNAPSHOT_HEADER, REPIN_HINT)
}

/// One snapshot line: what the ratchet actually pins.
///
/// The payload set is part of the key, not decoration. A shape that drops only a `line`
/// comment today and starts dropping a `block` one tomorrow is a **new bug on a new
/// ownership path** — keyed on the shape alone it would land inside an existing entry and
/// never be seen. It is also stable in the way a count is not: it changes when the bug's
/// character changes, not when a fixture is added.
///
/// [`Kind`] leads the key, so its derived [`Ord`] matches the `shapes` map's `(Kind, shape)`
/// order — the snapshot renders in exactly that order, giving a stable, minimal-diff file.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct KnownKey {
    kind: Kind,
    shape: String,
    payloads: String,
}

impl SnapshotKey for KnownKey {
    fn to_line(&self) -> String {
        format!("{}\t{}\t{}", self.kind.label(), self.shape, self.payloads)
    }

    fn from_line(line: &str) -> Option<Self> {
        let mut cols = line.split('\t');
        let kind = Kind::from_label(cols.next()?)?;
        let shape = cols.next()?.to_string();
        let payloads = cols.next()?.to_string();
        Some(Self {
            kind,
            shape,
            payloads,
        })
    }

    fn is_pinnable(&self) -> bool {
        is_pinnable(self.kind)
    }
}

/// Render a payload set into its snapshot column.
fn payload_column(payloads: &BTreeSet<&'static str>) -> String {
    payloads.iter().copied().collect::<Vec<_>>().join(",")
}

/// Every shape as a [`KnownKey`] — **including** the unpinnable (`PANIC`) ones. The
/// [`Ratchet`] filters those out of the file and counts them on their own (see
/// [`SnapshotKey::is_pinnable`]), so the caller hands it the whole set and the split lives
/// in one place.
fn snapshot_keys(shapes: &BTreeMap<(Kind, String), ShapeAgg>) -> BTreeSet<KnownKey> {
    shapes
        .iter()
        .map(|((kind, shape), agg)| KnownKey {
            kind: *kind,
            shape: shape.clone(),
            payloads: payload_column(&agg.payloads),
        })
        .collect()
}

/// Whether a shape is something the snapshot may pin — everything but a [`Kind::Panic`].
///
/// A panic is not a "known bug" to ratchet alongside the drops. The invariant it breaks is
/// **absolute** (a comment in a gap must never crash the formatter), so it always fails the
/// gate and is never pinnable — otherwise `--update` would quietly absorb a crash into the
/// same list whose shrinking is the goal. [`KnownKey::is_pinnable`] routes through this, so
/// the ratchet's render/grade and the panic accounting below stay in lockstep.
fn is_pinnable(kind: Kind) -> bool {
    kind != Kind::Panic
}

/// How many of `shapes` crash the formatter — the shapes [`is_pinnable`] keeps out of the
/// snapshot, and which therefore need their own accounting on every exit path (the ratchet's
/// [`GateDiff::unpinnable`] is the abstract count; this is the concrete panic set gap reports
/// with examples).
fn count_panics(shapes: &BTreeMap<(Kind, String), ShapeAgg>) -> usize {
    shapes.keys().filter(|(k, _)| *k == Kind::Panic).count()
}

/// Why a site is a finding. `Dropped`/`DoublePrinted` mirror the ledger; `Panic` is this
/// audit's own (a comment in a gap must never crash the formatter).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Kind {
    Dropped,
    DoublePrinted,
    Panic,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::Dropped => "DROPPED",
            Self::DoublePrinted => "DOUBLE-PRINTED",
            Self::Panic => "PANIC",
        }
    }

    fn from_label(s: &str) -> Option<Self> {
        [Self::Dropped, Self::DoublePrinted, Self::Panic]
            .into_iter()
            .find(|k| k.label() == s)
    }
}

/// One reproducible instance of a shape — everything needed to re-create the finding by
/// hand, and nothing else.
///
/// Kept as a unit rather than as loose `example_*` fields on [`ShapeAgg`], because they are
/// only meaningful together: [`Self::payload`] at [`Self::offset`] in [`Self::path`] is a
/// *triple*, and mixing one shape's offset with another's payload reproduces nothing.
#[derive(Clone)]
struct Example {
    /// The payload that produced **this** example. Distinct from [`ShapeAgg::payloads`],
    /// which is the union over every hit: re-injecting some other payload of the union at
    /// this offset need not fire, or even parse.
    payload: &'static str,
    path: String,
    /// The byte offset in the seed the payload was **injected** at — the splice
    /// [`verify_example`] must reproduce. Equals [`Self::attribution_offset`] for an injected
    /// hit; for a bystander it is the perturbation site, a different position.
    injection_offset: usize,
    /// The byte offset the finding is **attributed** to: the victim comment's own site for a
    /// bystander (mapped back across the splice by [`victim_seed_offset`]), the injection site
    /// for the injected comment. The shape, snippet, by-node edge, and canonical sort all key
    /// on this — so a dropped bystander points at the emitter that dropped it, not at wherever
    /// the payload happened to go in.
    attribution_offset: usize,
    snippet: String,
    /// The offending comment's text — the injected payload only when [`Self::injected`].
    text: String,
    /// Whether the offending comment is the injected one rather than a bystander the
    /// injection knocked out.
    injected: bool,
}

impl Example {
    /// The tie-break that makes the chosen example **thread-count independent**.
    ///
    /// Threads take files by stride, so which one first sees a shape depends on `--jobs`;
    /// picking the smallest `(path, attribution_offset)` instead of "whoever merged first"
    /// keeps a report (and any diff of one) stable across `--jobs 1` and `--jobs 12`. The
    /// **attribution** offset (not the injection offset) is the sort key so the canonical
    /// example is the finding's own smallest *victim* site — the meaningful, shape-consistent
    /// locus. Two examples can now tie on `(path, attribution_offset)` while differing in
    /// injection offset (two injections dropping the same victim); ties only ever arise within
    /// one file (one worker), so [`ShapeAgg::offer_example`]'s first-seen tie-break stays
    /// deterministic across `--jobs`.
    fn sort_key(&self) -> (&str, usize) {
        (&self.path, self.attribution_offset)
    }
}

/// How many examples per shape the [`ShapeAgg`] keeps and the verify pass re-checks.
///
/// One example gives a single Confirmed/Unconfirmed bit, which cannot tell "uniformly an
/// instrument gap" (every example unconfirmed) from "a mixed real drop" (some confirmed) —
/// the distinction phase 0 of the gaps arc turns on. Keeping the N *smallest* by
/// [`Example::sort_key`] samples the shape while staying bounded in memory and cheap to
/// verify (each example is two extra formats, run once per shape — not per site). Five is
/// enough to separate all-vs-none-vs-mixed without inflating the verify pass.
const VERIFY_EXAMPLES: usize = 5;

/// Everything a shape accumulates. Counts stay exact; only the [`VERIFY_EXAMPLES`] smallest
/// examples are kept, so a corpus that fires a bug a million times still reports in bounded
/// memory.
#[derive(Clone)]
struct ShapeAgg {
    count: usize,
    /// Which payloads reach this shape — a drop on `line` but not `block` is a different
    /// bug from one on both, so this is part of the ratchet key.
    payloads: BTreeSet<&'static str>,
    /// Hits where the offending comment is a **bystander** — a comment the author already
    /// had, knocked out by an injection elsewhere. Tracked apart from the total because it
    /// is the scarier half: an existing comment vanishing because someone added another one
    /// nearby.
    bystander_hits: usize,
    /// Distinct seed files the shape fired in — separates "one weird fixture" from
    /// "everything with a dot in it". Bounded by the corpus, and shapes are few.
    files: BTreeSet<String>,
    /// The [`VERIFY_EXAMPLES`] smallest examples by [`Example::sort_key`], kept sorted
    /// ascending, so `examples[0]` is the canonical (smallest) one every report shows.
    examples: Vec<Example>,
    /// The in-run self-verification outcome — `None` until the verify pass runs.
    verify: Option<VerifyOutcome>,
}

impl ShapeAgg {
    /// The canonical example — the smallest by [`Example::sort_key`], shown in every report.
    ///
    /// A recorded shape always has at least one example (it is created *with* the hit that
    /// recorded it, via `or_insert_with` + [`Self::offer_example`]), so this never sees an
    /// empty set — an empty one is a construction bug.
    #[allow(clippy::expect_used)] // invariant: a recorded shape is created with its first example
    fn canonical(&self) -> &Example {
        self.examples
            .first()
            .expect("a recorded shape always carries at least one example")
    }

    /// Offer `candidate` to the bounded min-N set, keeping it sorted ascending by
    /// [`Example::sort_key`] and capped at [`VERIFY_EXAMPLES`].
    ///
    /// Thread-count independence rides on this keeping the N *smallest* by `(path, offset)`,
    /// exactly the tie-break the old single-example version used. A later candidate that
    /// *ties* an existing one on `sort_key` sorts **after** it (`<=` insertion point), so the
    /// first-seen among equal keys stays canonical — `examples[0]` never regresses to a
    /// later arrival. Ties only ever occur within one file (one worker/tally), so the final
    /// merged set is deterministic regardless of `--jobs`.
    fn offer_example(&mut self, candidate: Example) {
        let pos = self
            .examples
            .partition_point(|e| e.sort_key() <= candidate.sort_key());
        if pos >= VERIFY_EXAMPLES && self.examples.len() >= VERIFY_EXAMPLES {
            return; // larger than every kept example, and the set is already full
        }
        self.examples.insert(pos, candidate);
        self.examples.truncate(VERIFY_EXAMPLES);
    }
}

/// One thread's slice of the work.
#[derive(Default)]
struct Tally {
    shapes: BTreeMap<(Kind, String), ShapeAgg>,
    sites: usize,
    injections: usize,
    accepted: usize,
    files_done: usize,
    parse_skipped: usize,
    /// Bystander findings whose victim span could not be mapped back to seed coordinates
    /// across the splice (out of range / mid-`char`) — keyed on the injection offset as a
    /// fallback. Expected to be zero; a nonzero count means a reflow the linear span-shift
    /// can't place (see [`victim_seed_offset`]), surfaced rather than silently mis-keyed.
    victim_map_fallbacks: usize,
    /// Files already non-clean before injection — reported, never injected into (see
    /// [`audit_file`]).
    dirty_files: Vec<String>,
}

/// One finding at one site, before aggregation into its [`ShapeAgg`].
struct Hit<'a> {
    kind: Kind,
    payload: Payload,
    path: &'a str,
    /// The seed source — the shape and snippet are derived from it, so the caller never
    /// computes them for a site that turns out not to fire.
    source: &'a str,
    /// The byte offset in `source` the payload was **injected** at — the splice the verify
    /// pass reproduces.
    injection_offset: usize,
    /// The byte offset in `source` the finding is **attributed** to: the victim's own site for
    /// a bystander (mapped back across the splice), the injection site for the injected
    /// comment. The shape and snippet key on this.
    attribution_offset: usize,
    /// The offending comment's text, which is the *injected* payload only when
    /// [`Self::injected`] holds.
    text: String,
    /// Whether the offending comment is the injected one rather than a bystander.
    injected: bool,
}

impl Tally {
    fn record(&mut self, hit: Hit<'_>) {
        // Both the shape and the snippet key on the ATTRIBUTION offset — the victim's own site
        // for a bystander — so a bystander drop is filed under the emitter that dropped it,
        // never the perturbation site the payload went in at.
        let shape = site_shape(hit.source, hit.attribution_offset);
        let candidate = Example {
            payload: hit.payload.label(),
            path: hit.path.to_string(),
            injection_offset: hit.injection_offset,
            attribution_offset: hit.attribution_offset,
            snippet: snippet(hit.source, hit.attribution_offset),
            text: hit.text,
            injected: hit.injected,
        };
        let e = self
            .shapes
            .entry((hit.kind, shape))
            .or_insert_with(|| ShapeAgg {
                count: 0,
                payloads: BTreeSet::new(),
                bystander_hits: 0,
                files: BTreeSet::new(),
                examples: Vec::new(),
                verify: None,
            });
        e.count += 1;
        if !hit.injected {
            e.bystander_hits += 1;
        }
        e.payloads.insert(hit.payload.label());
        e.files.insert(hit.path.to_string());
        e.offer_example(candidate);
    }

    fn merge(&mut self, other: Tally) {
        self.sites += other.sites;
        self.injections += other.injections;
        self.accepted += other.accepted;
        self.files_done += other.files_done;
        self.parse_skipped += other.parse_skipped;
        self.victim_map_fallbacks += other.victim_map_fallbacks;
        self.dirty_files.extend(other.dirty_files);
        for (k, v) in other.shapes {
            match self.shapes.get_mut(&k) {
                Some(e) => {
                    e.count += v.count;
                    e.bystander_hits += v.bystander_hits;
                    e.payloads.extend(v.payloads);
                    e.files.extend(v.files);
                    // Keep the N smallest across both, NOT whoever merged first — see
                    // `Example::sort_key` / `ShapeAgg::offer_example`. Workers take disjoint
                    // files, so the two example sets never share a path (no cross-tally ties).
                    for ex in v.examples {
                        e.offer_example(ex);
                    }
                }
                None => {
                    self.shapes.insert(k, v);
                }
            }
        }
    }
}

/// Re-derive a finding's **observable** claim, independently of the ledger that made it.
///
/// The ledger is an instrument, and an instrument that only ever agrees with itself is not
/// evidence — every mistake found while building this audit was of exactly that shape (a
/// stale needle, a char-vs-byte offset, checking the injected comment when the finding was
/// about a bystander). So each kept example is re-run and the ledger is made to *predict*
/// something falsifiable: if it says this format drops `d` comments and double-prints `p`,
/// then the output must reparse to exactly `parsed - d + p` comments. Anything else means
/// the ledger's account and the actual output disagree.
///
/// The caller runs this over up to [`VERIFY_EXAMPLES`] examples per shape and reduces the
/// per-example verdicts into a [`VerifyOutcome`] ratio: all-confirmed is clean, all-unconfirmed
/// is a uniform instrument gap, and a split is a mixed real drop.
///
/// Deciding via the multiset of comment **contents** rather than a count is what makes this
/// both sound and decisive. A printer may legitimately re-indent a multi-line comment, which
/// a raw text match would false-alarm on — so each content is whitespace-normalized
/// ([`normalize_comment_text`]) before it becomes a multiset element: a re-indent
/// (`/* a⏎   b */` → `/* a⏎b */`) keeps the newline and normalizes equal, while a **mangle**
/// (`/* a⏎b */` → `/* ab */`) drops the newline and normalizes different. And unlike the
/// earlier `parsed - dropped + double` count, the multiset closes that count's two blind
/// spots: a balancing drop+duplicate nets zero (equal count, unequal multiset), and a
/// mangle is count-invariant (equal count, unequal content).
///
/// So: the injected source's comment contents vs the output's. Equal ⇒ every comment is
/// content-conserved, so a ledger finding here is contradicted by the output — a genuine
/// **instrument gap** ([`Verdict::Unconfirmed`], now provably so). Unequal ⇒ a content is
/// missing, mangled, or duplicated — real loss/corruption ([`Verdict::Confirmed`]).
///
/// The residual blind spot, named rather than hidden and far narrower than the count's: a
/// multiset can still balance if the SAME content is dropped in one place and duplicated in
/// another. No example in the corpus does this, and the kept examples are a sample of the
/// shape's hits, never a proof about all of them.
fn verify_example(example: &Example, kind: Kind, parser: ParserType) -> Verdict {
    // A panic is self-evident: it either happens or it doesn't, and it was caught to get here.
    if kind == Kind::Panic {
        return Verdict::Confirmed;
    }
    let Ok(source) = std::fs::read_to_string(&example.path) else {
        return Verdict::Unconfirmed;
    };
    let Some(payload) = Payload::from_label(example.payload) else {
        return Verdict::Unconfirmed;
    };
    // Re-create the finding by re-splicing at the INJECTION offset (never the attribution one)
    // — a bystander drop only reproduces from the perturbation that caused it.
    let offset = example.injection_offset;
    if offset > source.len() || !source.is_char_boundary(offset) {
        return Verdict::Unconfirmed;
    }
    let mut injected = String::with_capacity(source.len() + 24);
    injected.push_str(&source[..offset]);
    injected.push_str(payload.text());
    injected.push_str(&source[offset..]);

    let Formatted::Ok {
        findings,
        comments: input_comments,
        output,
    } = ledger_format_with_comments(&injected, parser)
    else {
        return Verdict::Unconfirmed;
    };
    if findings.is_empty() {
        // The example no longer fires at all — the ledger and the re-run disagree outright.
        return Verdict::Unconfirmed;
    }
    let Formatted::Ok {
        comments: output_comments,
        ..
    } = ledger_format_with_comments(&output, parser)
    else {
        // The formatter's own output doesn't parse. A real bug, but `roundtrip_audit`'s.
        return Verdict::Unconfirmed;
    };

    if comment_content_multiset(&input_comments) == comment_content_multiset(&output_comments) {
        // Content conserved: the ledger's drop/double-print claim is not observable in the
        // output — an instrument gap, not the content loss it is filed as.
        Verdict::Unconfirmed
    } else {
        // A content is missing, mangled, or duplicated — the ledger's claim is real.
        Verdict::Confirmed
    }
}

/// The multiset of comment **contents**, each whitespace-normalized so a legitimate re-indent
/// reads as conserved while a mangle reads as changed (see [`verify_example`]).
fn comment_content_multiset(texts: &[String]) -> BTreeMap<String, usize> {
    let mut ms: BTreeMap<String, usize> = BTreeMap::new();
    for text in texts {
        *ms.entry(normalize_comment_text(text)).or_insert(0) += 1;
    }
    ms
}

/// Split a comment's text on newlines, trim each line, and rejoin with `\n`. A re-indent of a
/// multi-line block comment changes per-line leading/trailing whitespace but keeps the
/// newline count, so it normalizes equal; a mangle that collapses the newlines yields fewer
/// lines and normalizes different. `trim` also drops a `\r`, so `\r\n` vs `\n` line endings
/// normalize alike.
fn normalize_comment_text(text: &str) -> String {
    text.split('\n')
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Map a bystander victim's span-start from the injected source's coordinates back to the
/// seed's, across the single-payload splice.
///
/// The inject loop builds `injected = seed[..injection_offset] + payload + seed[injection_offset..]`,
/// so `payload_len` bytes were inserted at `injection_offset`. A **bystander** finding's
/// comment — never the injected one — therefore sits either wholly *before* the splice (its
/// start unchanged) or wholly *at or after* it (its start shifted right by `payload_len`). Its
/// start never lands in `[injection_offset, injection_offset + payload_len)`: that range is the
/// injected comment, which the caller classifies `injected` and never routes here.
///
/// Returns the seed-space offset, or `None` — **checked, never a panic** — when the mapped
/// offset is out of the seed's range or lands mid-`char`-boundary (a reflow the linear
/// span-shift can't place, e.g. a multi-line comment re-indented across the splice). The caller
/// then falls back to injection-offset keying and counts it, so a stray victim is
/// mis-attributed rather than crashing the audit. This arithmetic is the "corpus can't grade
/// it" class — an off-by-one leaves every ASCII file byte-identical — so it is unit-tested
/// directly.
fn victim_seed_offset(
    seed: &str,
    injection_offset: usize,
    payload_len: usize,
    victim_start: usize,
) -> Option<usize> {
    let seed_offset = if victim_start < injection_offset {
        victim_start
    } else if victim_start >= injection_offset + payload_len {
        victim_start - payload_len
    } else {
        // Inside the injected payload — impossible for a bystander (that range IS the injected
        // comment). Refuse rather than fabricate an offset.
        return None;
    };
    (seed_offset <= seed.len() && seed.is_char_boundary(seed_offset)).then_some(seed_offset)
}

/// Audit one file: verify it is clean **as authored**, then inject at every site.
///
/// The pristine check is load-bearing, not a formality. A file that already drops a comment
/// would re-report that same drop at every one of its thousands of sites, drowning the
/// signal — so such a file is reported once and skipped. Over `tests/fixtures` this never
/// fires (`comments:audit` gates it green); over a real corpus it is the honest split
/// between "you already knew" and "the injection found it".
fn audit_file(path: &std::path::Path, payloads: &[Payload], all_bytes: bool, tally: &mut Tally) {
    let display = path.to_string_lossy().into_owned();
    // Intentionally-invalid fixtures don't parse — nothing to inject into.
    if path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with("input_invalid"))
    {
        return;
    }
    let Ok(source) = std::fs::read_to_string(path) else {
        return;
    };
    let parser = ParserType::from_extension(&display);

    let comment_spans = match pristine_format(&source, parser) {
        Pristine::Skip { dirty: false } => {
            tally.parse_skipped += 1;
            return;
        }
        Pristine::Skip { dirty: true } => {
            tally.dirty_files.push(display);
            return;
        }
        Pristine::Clean { comment_spans } => comment_spans,
    };
    tally.files_done += 1;

    let regions = code_regions(&source, parser);
    let sites = injection_sites(&source, &regions, &comment_spans, all_bytes);
    tally.sites += sites.len();

    let mut injected_src = String::with_capacity(source.len() + 24);
    for &payload in payloads {
        let text = payload.text();
        for &offset in &sites {
            injected_src.clear();
            injected_src.push_str(&source[..offset]);
            injected_src.push_str(text);
            injected_src.push_str(&source[offset..]);
            tally.injections += 1;

            let findings = match ledger_format(&injected_src, parser) {
                Formatted::Panicked => {
                    tally.record(Hit {
                        kind: Kind::Panic,
                        payload,
                        path: &display,
                        source: &source,
                        injection_offset: offset,
                        attribution_offset: offset,
                        text: text.to_string(),
                        injected: true,
                    });
                    continue;
                }
                // The injection isn't a legal comment here — the offset names no gap.
                Formatted::Rejected => continue,
                Formatted::Ok { findings, .. } => findings,
            };
            tally.accepted += 1;
            for f in findings {
                // The injected comment starts exactly at the injection point; anything else is
                // a bystander the injection knocked out. A bystander's finding span is in the
                // INJECTED source's coordinates, so map it back across the splice to the seed —
                // that seed offset is where the victim comment actually lived, and is what the
                // shape / snippet / by-node must key on (not the perturbation site).
                let victim_start = f.span.start as usize;
                let injected = victim_start == offset;
                let attribution_offset = if injected {
                    offset
                } else {
                    // TODO: island-relative-span hazard. This maps `f.span` back as if it were
                    // host-absolute over `source`, but a finding's span is in the coordinate space
                    // of the DOCUMENT it was registered against. A nested <script>/<style> ELEMENT
                    // is re-parsed against its own extracted content string, so an island finding's
                    // span is ISLAND-relative — mapping it across the splice would yield a bogus
                    // seed offset with `victim_map_fallbacks` staying 0, a SILENT mis-attribution.
                    // Safe TODAY only because `code_regions` injects host-only, so no island
                    // finding can arise. Naming <style>/nested-element raw content in
                    // `code_regions` (see `audit::sites::code_regions`'s TODO) opens the hole and
                    // MUST fix this first: thread the finding's `DocumentKey` (host source
                    // identity) through `CommentFinding` so the mapping can scope to the host key —
                    // as `comment_ledger::parsed_comment_spans` already does — or fall back.
                    match victim_seed_offset(&source, offset, text.len(), victim_start) {
                        Some(seed_offset) => seed_offset,
                        None => {
                            tally.victim_map_fallbacks += 1;
                            offset
                        }
                    }
                };
                tally.record(Hit {
                    kind: match f.kind {
                        CommentFindingKind::Dropped => Kind::Dropped,
                        CommentFindingKind::DoublePrinted => Kind::DoublePrinted,
                    },
                    payload,
                    path: &display,
                    source: &source,
                    injection_offset: offset,
                    attribution_offset,
                    text: f.text,
                    injected,
                });
            }
        }
    }
}

impl GapAuditCommand {
    /// The flags in effect that make this run something other than the one the snapshot
    /// describes: every payload, at every non-word site, over all of `tests/fixtures`.
    ///
    /// Empty ⇒ the run is both gradable against the snapshot and pinnable into it. Anything
    /// else reaches a different shape set — a subset for `--limit` / `--payload` / a path,
    /// a superset for `--all-bytes` — which is neither. One list, two uses (the `--update`
    /// refusal and the gate skip), so the two can't drift apart into disagreeing about what
    /// a full run is.
    fn narrowing_flags(&self) -> Vec<&'static str> {
        let mut flags = Vec::new();
        if !self.paths.is_empty() {
            flags.push("<paths>");
        }
        if self.limit > 0 {
            flags.push("--limit");
        }
        if self.payload.is_some() {
            flags.push("--payload");
        }
        if self.all_bytes {
            flags.push("--all-bytes");
        }
        flags
    }

    pub(crate) fn run(self) -> Result<(), CliError> {
        let payloads: Vec<Payload> = match &self.payload {
            None => Payload::ALL.to_vec(),
            Some(s) => match Payload::from_label(s) {
                Some(p) => vec![p],
                None => {
                    eprintln!(
                        "Error: unknown --payload {s:?} (expected one of: {})",
                        Payload::ALL
                            .iter()
                            .map(|p| p.label())
                            .collect::<Vec<_>>()
                            .join(" | ")
                    );
                    return Err(CliError::Failed);
                }
            },
        };

        let default_paths = self.paths.is_empty();
        let narrowed = self.narrowing_flags();
        if self.update && !narrowed.is_empty() {
            eprintln!(
                "Error: --update pins the FULL default run (all {} payloads over \
                 tests/fixtures). This run is narrowed by {}, so its shape set is a \
                 SUBSET (or, for --all-bytes, a superset) of what the snapshot means — \
                 writing it would silently unpin real bugs. Re-run without {}.",
                Payload::ALL.len(),
                narrowed.join(" / "),
                narrowed.join(" / ")
            );
            return Err(CliError::Failed);
        }
        let paths = if default_paths {
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
        if self.limit > 0 {
            files.truncate(self.limit);
        }
        if files.is_empty() {
            eprintln!("Error: no seed files found (searched {paths:?})");
            return Err(CliError::Failed);
        }

        // Process-global; the per-thread ledgers below are thread-local, so arming once
        // here covers every worker.
        comment_ledger::set_comment_check(true);

        // The audit provokes panics on purpose (a formatter crash IS a finding), so keep
        // the default hook from printing each one.
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        let jobs = self
            .jobs
            .filter(|j| *j > 0)
            .or_else(|| {
                std::thread::available_parallelism()
                    .ok()
                    .map(std::num::NonZero::get)
            })
            .unwrap_or(1)
            .min(files.len());

        let all_bytes = self.all_bytes;
        let mut total = Tally::default();
        // Chunk by stride rather than by block: fixture sizes cluster by directory, and
        // the work is QUADRATIC in file size, so contiguous blocks would leave one thread
        // holding every large file.
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..jobs)
                .map(|worker| {
                    let files = &files;
                    let payloads = &payloads;
                    scope.spawn(move || {
                        let mut tally = Tally::default();
                        for path in files.iter().skip(worker).step_by(jobs) {
                            audit_file(path, payloads, all_bytes, &mut tally);
                        }
                        tally
                    })
                })
                .collect();
            for h in handles {
                match h.join() {
                    Ok(t) => total.merge(t),
                    Err(_) => eprintln!("warning: a worker thread panicked outside the audit loop"),
                }
            }
        });

        // A bystander whose victim span couldn't be placed back in seed coordinates was keyed
        // on its injection offset instead — expected to be zero, so surface it rather than let
        // it silently pollute a shape.
        if total.victim_map_fallbacks > 0 {
            eprintln!(
                "warning: {} bystander finding(s) could not map a victim span back across the \
                 splice and fell back to injection-offset keying — a reflow the linear \
                 span-shift can't place (see `victim_seed_offset`). Expected zero.",
                total.victim_map_fallbacks
            );
        }

        // Self-verify each shape's kept examples against the output (cheap: up to
        // `VERIFY_EXAMPLES` per shape, not per site — a few thousand formats against
        // millions). Single-threaded and after the join, so it can't interleave with a
        // worker's thread-local ledger. Each example uses its own file's parser, since a
        // shape can fire across `.svelte` / `.ts` / `.css` alike.
        let outcomes: Vec<((Kind, String), VerifyOutcome)> = total
            .shapes
            .iter()
            .map(|((kind, shape), agg)| {
                let confirmed = agg
                    .examples
                    .iter()
                    .filter(|ex| {
                        let parser = ParserType::from_extension(&ex.path);
                        verify_example(ex, *kind, parser) == Verdict::Confirmed
                    })
                    .count();
                let outcome = VerifyOutcome {
                    confirmed,
                    total: agg.examples.len(),
                };
                ((*kind, shape.clone()), outcome)
            })
            .collect();
        for (key, outcome) in outcomes {
            if let Some(agg) = total.shapes.get_mut(&key) {
                agg.verify = Some(outcome);
            }
        }

        std::panic::set_hook(prev_hook);
        comment_ledger::set_comment_check(false);

        if self.update {
            let found = snapshot_keys(&total.shapes);
            ratchet().write(&found)?;
            // Count what was actually written (the pinnable keys), not every shape — a
            // panic is not pinned (see `is_pinnable`), so reporting `total.shapes.len()`
            // would overstate the file by exactly the crashes it deliberately omits.
            let written = found.iter().filter(|k| k.is_pinnable()).count();
            println!(
                "✓ wrote {} shape(s) to {}",
                written,
                known_shapes_path().display()
            );
            // Spend the verify pass rather than discarding it. Pinning is the moment ~700
            // claims get frozen, so it is exactly when it's worth saying which ones the
            // audit could not reproduce. A WARNING, not a refusal: an unconfirmed shape is
            // still a real finding, and the verdict describes the shape's one sampled
            // example rather than the shape, so refusing on it would both block `--update`
            // and flip with which fixture happens to sort first.
            let unconfirmed = count_by_summary(&total.shapes, VerifySummary::Unconfirmed);
            let partial = count_by_summary(&total.shapes, VerifySummary::Partial);
            if unconfirmed > 0 || partial > 0 {
                println!(
                    "  ⚠ verify: {unconfirmed} shape(s) UNCONFIRMED (no kept example \
                     reproduced) and {partial} PARTIAL (some did) — filed as \
                     dropped/double-printed, yet the output reparses to just as many comments \
                     as its input. Likely MANGLES (a rebuilt comment) rather than plain drops; \
                     see docs/gap_audit.md."
                );
            }
            let panics = count_panics(&total.shapes);
            if panics > 0 {
                eprintln!(
                    "\n✗ {panics} PANIC shape(s) were NOT pinned — a comment in a gap must \
                     never crash the formatter, so the gate will keep failing until they \
                     are fixed."
                );
                return Err(CliError::Failed);
            }
            return Ok(());
        }

        // Grade BEFORE printing. Only a run that is actually graded can be quiet — a
        // narrowed or off-corpus run has no verdict to be quiet about, so it always reports.
        let graded = if default_paths && narrowed.is_empty() {
            Some(ratchet().grade(&snapshot_keys(&total.shapes))?)
        } else {
            None
        };

        let (summary, findings) = build_report(&total, &payloads);
        // The by-node rollup + attribution-agreement measurement is report-only, and both its
        // consumers parse seed files — so compute it at most once, only when a consumer needs it:
        // `--json` always folds it in (per-slice tooling reads it to ask "did my fix move the
        // cluster?"), and the human `--by-node` view renders it as text.
        let rollup = (self.json || self.by_node).then(|| compute_by_node(&total));
        if self.json {
            let extra = rollup
                .as_ref()
                .map(by_node_json_sections)
                .unwrap_or_default();
            report::print_json(&summary, &findings, &extra);
        } else if graded.as_ref().is_some_and(GateDiff::holds) && !self.report {
            // Nothing to act on: every shape is one the snapshot already pins, so the
            // per-shape report is thousands of lines of noise in `deno task check`.
            report::print_summary(&summary, &findings);
        } else {
            report::print_report(&summary, &findings);
        }

        // The human by-node view — printed on every path (default or narrowed), after the report
        // and before the exit decision, so it never perturbs the grade or exit.
        if let Some(rollup) = &rollup
            && self.by_node
        {
            report_by_node(rollup, self.json);
        }

        // Off the default corpus the snapshot doesn't apply — every finding is news.
        if !default_paths {
            return if total.shapes.is_empty() {
                Ok(())
            } else {
                Err(CliError::Failed)
            };
        }
        // A narrowed default run reaches only part of the snapshot's shape set (or, under
        // --all-bytes, more than it), so grading it would report every shape the narrowing
        // simply didn't reach as a stale entry — a wall of noise that says nothing about
        // the code. These flags are diagnostics; report and stop rather than fail on the
        // narrowing itself.
        if !narrowed.is_empty() {
            eprintln!(
                "\n○ ratchet SKIPPED — {} narrows this run, and the snapshot pins the full \
                 default one. Findings above are reported, NOT graded: this is not a \
                 passing gate.",
                narrowed.join(" / ")
            );
            return Ok(());
        }
        match &graded {
            Some(diff) => self.report_gate(diff, &total),
            // Unreachable: `graded` is Some exactly when default-pathed and un-narrowed,
            // which the two returns above have just established.
            None => Ok(()),
        }
    }

    /// Report a [`GateDiff`] and turn it into an exit status. See [`known_shapes_path`] for
    /// why the key is the shape and not the count.
    fn report_gate(&self, diff: &GateDiff<KnownKey>, total: &Tally) -> Result<(), CliError> {
        let GateDiff { new, stale, .. } = diff;

        // Panics are graded on their own, never against the snapshot: `is_pinnable` keeps
        // them out of both sides of the diff, so without this arm a crash would fail
        // nothing at all.
        let panics: Vec<_> = total
            .shapes
            .iter()
            .filter(|((kind, _), _)| *kind == Kind::Panic)
            .collect();
        if !panics.is_empty() {
            eprintln!(
                "\n✗ {} shape(s) CRASH the formatter — a comment in a gap must never panic \
                 it. Not pinnable and not a ratchet entry: fix the crash.",
                panics.len()
            );
            for ((_, shape), agg) in panics.iter().take(40) {
                let ex = agg.canonical();
                // A panic hit is always the injected comment (injection == attribution), so this
                // "inject … at" line names the injection offset that reproduces the crash.
                eprintln!(
                    "    {shape:<20} e.g. inject {} at {}:{}",
                    ex.payload, ex.path, ex.injection_offset
                );
            }
            if panics.len() > 40 {
                eprintln!("    … and {} more", panics.len() - 40);
            }
        }

        if !new.is_empty() {
            eprintln!(
                "\n✗ {} NEW finding shape(s) — a comment in one of these gaps is dropped or \
                 double-printed, and the snapshot has never seen it:",
                new.len()
            );
            for k in new.iter().take(40) {
                eprintln!(
                    "    {:<14} {:<20} [{}]",
                    k.kind.label(),
                    k.shape,
                    k.payloads
                );
            }
            if new.len() > 40 {
                eprintln!("    … and {} more", new.len() - 40);
            }
            eprintln!(
                "  Fix the drop, or — if it is genuinely pre-existing and merely newly \
                 REACHED by a fixture — re-run `deno task gaps:audit:update`."
            );
        }
        if !stale.is_empty() {
            eprintln!(
                "\n✗ {} STALE snapshot entry/entries — these no longer fire. If you fixed \
                 them, drop the lines (`deno task gaps:audit:update`):",
                stale.len()
            );
            for k in stale.iter().take(40) {
                eprintln!(
                    "    {:<14} {:<20} [{}]",
                    k.kind.label(),
                    k.shape,
                    k.payloads
                );
            }
            if stale.len() > 40 {
                eprintln!("    … and {} more", stale.len() - 40);
            }
        }

        if diff.holds() {
            // Under `--json`, stdout is the report and nothing else — a trailing status line
            // makes it unparseable. Logs go to stderr (the `corpus:compare --json` contract).
            let msg = format!(
                "\n✓ ratchet holds — every finding shape is a known bug ({} pinned); no new \
                 gap drops a comment",
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

/// How many pinnable shapes carry the given verify [`VerifySummary`] — `Unconfirmed` (no
/// kept example reproduced) or `Partial` (some did). An unverified shape (`None`) matches
/// neither.
fn count_by_summary(shapes: &BTreeMap<(Kind, String), ShapeAgg>, want: VerifySummary) -> usize {
    shapes
        .iter()
        .filter(|((kind, _), agg)| {
            is_pinnable(*kind) && agg.verify.map(VerifyOutcome::summary) == Some(want)
        })
        .count()
}

/// Translate a run's [`Tally`] into the shared reporting envelope: the run totals and one
/// [`Finding`] per shape, in the `shapes` map's `(Kind, shape)` order (so the printers'
/// stable count-sort preserves the `(Kind, shape)` tie-break).
fn build_report(total: &Tally, payloads: &[Payload]) -> (RunSummary, Vec<Finding>) {
    let summary = RunSummary {
        audit: "gap_audit",
        files_done: total.files_done,
        sites: total.sites,
        injections: total.injections,
        accepted: total.accepted,
        parse_skipped: total.parse_skipped,
        dirty_files: total.dirty_files.clone(),
        payload_labels: payloads.iter().map(|p| p.label()).collect(),
    };
    let findings = total
        .shapes
        .iter()
        .map(|((kind, shape), agg)| {
            let ex = agg.canonical();
            Finding {
                audit: "gap_audit",
                severity: severity_of(*kind),
                confidence: agg.verify.map(|v| confidence_of(v.summary())),
                site: shape.clone(),
                verdict_string: agg
                    .verify
                    .map(VerifyOutcome::report_label)
                    .unwrap_or_default(),
                example: ReportExample {
                    payload: ex.payload,
                    path: ex.path.clone(),
                    injection_offset: ex.injection_offset,
                    attribution_offset: ex.attribution_offset,
                    snippet: ex.snippet.clone(),
                    text: ex.text.clone(),
                    injected: ex.injected,
                },
                detail: Detail::Gap(GapDetail {
                    kind_label: kind.label(),
                    count: agg.count,
                    files: agg.files.len(),
                    payloads: agg.payloads.iter().copied().collect(),
                    bystander_hits: agg.bystander_hits,
                    verify_confirmed: agg.verify.map(|v| v.confirmed),
                    verify_total: agg.verify.map(|v| v.total),
                }),
            }
        })
        .collect();
    (summary, findings)
}

/// A finding's [`Severity`]: a `PANIC` is an absolute break (gate-failing on its own); a
/// drop / double-print is informational, its fatality decided by the ratchet.
fn severity_of(kind: Kind) -> Severity {
    match kind {
        Kind::Panic => Severity::GateFailing,
        Kind::Dropped | Kind::DoublePrinted => Severity::Informational,
    }
}

/// Map the verify pass's [`VerifySummary`] onto the envelope's [`Confidence`] axis.
fn confidence_of(summary: VerifySummary) -> Confidence {
    match summary {
        VerifySummary::Clean => Confidence::Confirmed,
        VerifySummary::Partial => Confidence::Partial,
        VerifySummary::Unconfirmed => Confidence::Unconfirmed,
    }
}

/// One `(node_type, edge)` cluster in the by-node rollup — the finding shapes that share an
/// enclosing AST node-edge, and their summed count.
struct NodeCluster {
    /// Summed finding count across the cluster's shapes.
    total: usize,
    /// How many distinct site shapes rolled into it.
    shapes: usize,
    /// The lexicographically smallest shape, shown as the cluster's example.
    smallest_shape: String,
}

/// A seed parsed once for the by-node pass, so shapes sharing a file don't re-read/re-parse it.
struct ParsedSeed {
    source: String,
    wire: serde_json::Value,
}

/// One cluster row in the ranked (worst-first) by-node work-list — the human/JSON-ready form of
/// a [`NodeCluster`], carrying its own key.
struct ClusterRow {
    key: NodeEdgeKey,
    hits: usize,
    shapes: usize,
    smallest_shape: String,
}

/// One fat shape whose kept examples key to MORE than one `(node, edge)` cluster — the canonical
/// rollup attributes its whole count to just one, so it is where that approximation's error
/// concentrates.
struct Disagreer {
    kind_label: &'static str,
    shape: String,
    hits: usize,
    /// The distinct `(node, edge)` clusters this shape's kept examples span, as display strings.
    clusters: Vec<String>,
}

/// Whether the canonical-example rollup can be trusted: does a shape's WHOLE count belong on one
/// `(node, edge)` cluster, or does the shape span several? Measured by keying **every** kept
/// example (not just the canonical) and asking whether they all land on one cluster.
struct Agreement {
    /// Shapes whose kept examples all key to a single cluster (canonical attribution is exact).
    shapes_agreeing: usize,
    /// Shapes with at least one resolvable example — the denominator (a fully-unresolved shape is
    /// outside it, exactly as it is outside the cluster totals).
    shapes_measured: usize,
    /// Hits summed over the agreeing shapes — the hit-weighted numerator.
    hits_agreeing: usize,
    /// Hits summed over the measured shapes — the hit-weighted denominator.
    hits_measured: usize,
    /// The fattest disagreeing shapes, worst-first, capped at [`TOP_DISAGREERS`].
    top_disagreers: Vec<Disagreer>,
}

/// The whole by-node rollup + its trustworthiness measurement, computed once and shared by the
/// human `--by-node` view and the `--json` sections.
struct ByNodeRollup {
    /// Clusters ranked worst-first (hits desc, then key).
    clusters: Vec<ClusterRow>,
    grand_total: usize,
    unresolved_count: usize,
    unresolved_shapes: usize,
    total_shapes: usize,
    agreement: Agreement,
}

/// How many disagreeing shapes the report lists — enough to see where the canonical-attribution
/// error concentrates without dumping the whole tail.
const TOP_DISAGREERS: usize = 15;

/// One shape's resolved cluster membership — the pure input to [`aggregate_agreement`], split out
/// so the agreement arithmetic (the "corpus can't grade it" class — every share is exact yet no
/// corpus diff would flag an off-by-one) is unit-testable without file I/O.
struct ShapeClusters {
    kind_label: &'static str,
    shape: String,
    hits: usize,
    /// The distinct `(node, edge)` clusters the shape's kept examples resolved to, as display
    /// strings. Empty ⇒ the shape is fully unresolved and outside the agreement denominator.
    clusters: Vec<String>,
}

/// Fold each shape's resolved cluster membership into the [`Agreement`] tally: one resolved
/// cluster ⇒ agrees, more than one ⇒ disagrees (and is a candidate disagreer), none ⇒ unmeasured.
/// Pure — the caller does the file I/O to build the [`ShapeClusters`].
fn aggregate_agreement(shapes: &[ShapeClusters]) -> Agreement {
    let mut shapes_agreeing = 0usize;
    let mut shapes_measured = 0usize;
    let mut hits_agreeing = 0usize;
    let mut hits_measured = 0usize;
    let mut disagreers: Vec<Disagreer> = Vec::new();
    for s in shapes {
        match s.clusters.len() {
            0 => {} // fully unresolved — outside the measured denominator, like the cluster tail
            1 => {
                shapes_agreeing += 1;
                shapes_measured += 1;
                hits_agreeing += s.hits;
                hits_measured += s.hits;
            }
            _ => {
                shapes_measured += 1;
                hits_measured += s.hits;
                disagreers.push(Disagreer {
                    kind_label: s.kind_label,
                    shape: s.shape.clone(),
                    hits: s.hits,
                    clusters: s.clusters.clone(),
                });
            }
        }
    }
    // Fattest first — a high-hit shape disagreeing mis-ranks far more than a 2-hit one. Ties
    // break on the shape name so the list is deterministic.
    disagreers.sort_by(|a, b| b.hits.cmp(&a.hits).then_with(|| a.shape.cmp(&b.shape)));
    disagreers.truncate(TOP_DISAGREERS);
    Agreement {
        shapes_agreeing,
        shapes_measured,
        hits_agreeing,
        hits_measured,
        top_disagreers: disagreers,
    }
}

/// Resolve one seed offset to its `(node, edge)` cluster, parsing (and caching) the seed file.
///
/// A `node_edge_key` call rebuilds `Utf16ToByte` from the source each time — fine over the ≤5
/// kept examples per shape this is called on, but the reason it must never move into a loop over
/// all injection sites.
fn resolve_key(
    cache: &mut HashMap<String, Option<ParsedSeed>>,
    path: &str,
    attribution_offset: usize,
) -> Option<NodeEdgeKey> {
    let seed = cache.entry(path.to_string()).or_insert_with(|| {
        let source = std::fs::read_to_string(path).ok()?;
        let parser = ParserType::from_extension(path);
        let wire = tsv_parse_to_value(&source, parser)?;
        Some(ParsedSeed { source, wire })
    });
    seed.as_ref()
        .and_then(|s| node_edge_key(&s.wire, &s.source, attribution_offset))
}

/// Build the COARSE by-(node, edge) rollup AND measure how trustworthy it is.
///
/// The rollup keys each shape's **canonical** example by its [`NodeEdgeKey`] and sums the shapes'
/// finding counts per key — folding the ~700 fine token shapes into the few dozen printer
/// clusters a burn-down works through. But canonical = the smallest `(path, attribution_offset)`,
/// arbitrary w.r.t. structure, so attributing a shape's WHOLE count to it is only sound when the
/// shape occupies one cluster. So this also keys **every** kept example and measures per-shape
/// [`Agreement`]: a shape whose kept examples span more than one cluster is where the canonical
/// approximation mis-ranks, and the fattest such shapes are the error's concentration.
///
/// Report-only: it feeds neither the gate nor the exit code.
fn compute_by_node(total: &Tally) -> ByNodeRollup {
    let mut clusters: BTreeMap<NodeEdgeKey, NodeCluster> = BTreeMap::new();
    let mut cache: HashMap<String, Option<ParsedSeed>> = HashMap::new();
    let mut unresolved_count = 0usize;
    let mut unresolved_shapes = 0usize;
    let mut shape_clusters: Vec<ShapeClusters> = Vec::with_capacity(total.shapes.len());

    for ((kind, shape), agg) in &total.shapes {
        // Canonical attribution — the cluster rollup (unchanged from the T2 slice).
        //
        // TODO: record-time keying. Attributing a shape's WHOLE count to its canonical example's
        // cluster is only exact when the shape occupies one cluster, and the `agreement` measure
        // below shows it does not for the fattest, generic shapes (hit-weighted agreement ~77%
        // over `tests/fixtures`, the disagreement led by `␣⟨⟩␣` splitting across emitters) — so
        // the headline ranking a burn-down reads is mis-ranked. The exact fix is to key each hit
        // at RECORD time by its own `attribution_offset`: parse the seed's wire once per file in
        // `audit_file` (already done for Svelte in `code_regions`), build one `Utf16ToByte`, key
        // each `Hit` before `Tally::record`, and accumulate a `BTreeMap<NodeEdgeKey, …>` exactly —
        // retiring this post-hoc canonical approximation. Left as a follow-up: it is a data-model
        // change, not this measurement slice's scope.
        let ex = agg.canonical();
        match resolve_key(&mut cache, &ex.path, ex.attribution_offset) {
            Some(k) => {
                let c = clusters.entry(k).or_insert_with(|| NodeCluster {
                    total: 0,
                    shapes: 0,
                    smallest_shape: shape.clone(),
                });
                c.total += agg.count;
                c.shapes += 1;
                if shape.as_str() < c.smallest_shape.as_str() {
                    c.smallest_shape.clone_from(shape);
                }
            }
            None => {
                unresolved_count += agg.count;
                unresolved_shapes += 1;
            }
        }

        // Agreement — the distinct resolved clusters across ALL kept examples (≤5), not just the
        // canonical one.
        let mut distinct: BTreeSet<NodeEdgeKey> = BTreeSet::new();
        for ex in &agg.examples {
            if let Some(k) = resolve_key(&mut cache, &ex.path, ex.attribution_offset) {
                distinct.insert(k);
            }
        }
        shape_clusters.push(ShapeClusters {
            kind_label: kind.label(),
            shape: shape.clone(),
            hits: agg.count,
            clusters: distinct.iter().map(NodeEdgeKey::to_string).collect(),
        });
    }

    // Worst-first: the fattest emitter cluster is the highest-leverage fix. Ties break on the
    // key, so the ranking is deterministic.
    let grand_total: usize = clusters.values().map(|c| c.total).sum();
    let mut rows: Vec<(NodeEdgeKey, NodeCluster)> = clusters.into_iter().collect();
    rows.sort_by(|a, b| b.1.total.cmp(&a.1.total).then_with(|| a.0.cmp(&b.0)));
    let clusters: Vec<ClusterRow> = rows
        .into_iter()
        .map(|(key, c)| ClusterRow {
            key,
            hits: c.total,
            shapes: c.shapes,
            smallest_shape: c.smallest_shape,
        })
        .collect();

    ByNodeRollup {
        clusters,
        grand_total,
        unresolved_count,
        unresolved_shapes,
        total_shapes: total.shapes.len(),
        agreement: aggregate_agreement(&shape_clusters),
    }
}

/// `n/d` as a whole-percent, `0` when `d == 0` — the human view's share formatter.
fn pct_of(n: usize, d: usize) -> usize {
    if d > 0 { n * 100 / d } else { 0 }
}

/// `n/d` as a fraction rounded to four decimals, `0.0` when `d == 0` — the JSON view's share.
///
/// Both operands are finding COUNTS — comfortably under 2^52, so the `f64` cast is exact and the
/// precision-loss lint (the whole-corpus-scale caveat) does not apply, exactly as
/// [`metrics`](super::metrics) allows it for the same reason.
#[allow(clippy::cast_precision_loss)]
fn share_of(n: usize, d: usize) -> f64 {
    if d == 0 {
        0.0
    } else {
        ((n as f64 / d as f64) * 1e4).round() / 1e4
    }
}

/// The audit-specific top-level `--json` sections `report::print_json` folds in beside the
/// envelope: `by_node` (the ranked cluster work-list per-slice tooling consumes) and `agreement`
/// (whether that ranking can be trusted). Additive — the envelope's own fields are untouched.
fn by_node_json_sections(rollup: &ByNodeRollup) -> serde_json::Map<String, serde_json::Value> {
    let by_node: Vec<serde_json::Value> = rollup
        .clusters
        .iter()
        .map(|c| {
            serde_json::json!({
                "node": c.key.node_type,
                "edge": c.key.edge,
                "hits": c.hits,
                "shapes": c.shapes,
                "share": share_of(c.hits, rollup.grand_total),
                "example_shape": c.smallest_shape,
            })
        })
        .collect();

    let ag = &rollup.agreement;
    let top_disagreers: Vec<serde_json::Value> = ag
        .top_disagreers
        .iter()
        .map(|d| {
            serde_json::json!({
                "kind": d.kind_label,
                "shape": d.shape,
                "hits": d.hits,
                "clusters": d.clusters,
            })
        })
        .collect();
    let agreement = serde_json::json!({
        "shapes_agreeing": ag.shapes_agreeing,
        "shapes_total": ag.shapes_measured,
        "shapes_agreeing_share": share_of(ag.shapes_agreeing, ag.shapes_measured),
        "hits_agreeing": ag.hits_agreeing,
        "hits_total": ag.hits_measured,
        "hits_agreeing_share": share_of(ag.hits_agreeing, ag.hits_measured),
        "unresolved_shapes": rollup.unresolved_shapes,
        "unresolved_count": rollup.unresolved_count,
        "top_disagreers": top_disagreers,
    });

    let mut m = serde_json::Map::new();
    m.insert("by_node".to_string(), serde_json::Value::Array(by_node));
    m.insert("agreement".to_string(), agreement);
    m
}

/// Print the COARSE by-(node, edge) rollup — a ranked emitter work-list — plus the
/// attribution-agreement measurement of how trustworthy that ranking is.
///
/// A shape whose canonical example no longer reads/parses, or whose offset keys to no node, falls
/// into the `UNRESOLVED` tail (reported, never fatal). Report-only: computed after grading, it
/// feeds neither the gate nor the exit code. Under `--json` it prints to stderr, leaving the JSON
/// document on stdout the sole parseable output.
fn report_by_node(rollup: &ByNodeRollup, json: bool) {
    let mut lines: Vec<String> = Vec::new();
    let unresolved = if rollup.unresolved_shapes > 0 {
        format!(
            "  ·  {} finding(s) across {} shape(s) UNRESOLVED",
            rollup.unresolved_count, rollup.unresolved_shapes
        )
    } else {
        String::new()
    };
    lines.push(format!(
        "\nby-node — {} emitter cluster(s) over {} finding(s) across {} shape(s){unresolved}",
        rollup.clusters.len(),
        rollup.grand_total,
        rollup.total_shapes
    ));
    lines.push(String::new());
    for c in &rollup.clusters {
        let key = c.key.to_string();
        lines.push(format!(
            "  {:>7}×  {:>4} shape(s)  {key:<42}  e.g. {}",
            c.hits, c.shapes, c.smallest_shape
        ));
    }
    let top10: usize = rollup.clusters.iter().take(10).map(|c| c.hits).sum();
    lines.push(format!(
        "\ntop-10 cluster(s) cover {top10}/{} findings ({}%)",
        rollup.grand_total,
        pct_of(top10, rollup.grand_total)
    ));
    lines.push(
        "note: each shape's whole count is attributed to its canonical example's (node, edge), \
         so a generic shape spanning several structural contexts lands wholly in one — the \
         totals are a worst-first approximation, not an exact per-site tally."
            .to_string(),
    );

    // The attribution-agreement measurement — how much of that approximation to trust.
    let ag = &rollup.agreement;
    lines.push(String::new());
    lines.push(format!(
        "attribution agreement — {}/{} measured shape(s) key ALL kept examples to ONE cluster \
         ({}% unweighted); hit-weighted {}/{} ({}%)",
        ag.shapes_agreeing,
        ag.shapes_measured,
        pct_of(ag.shapes_agreeing, ag.shapes_measured),
        ag.hits_agreeing,
        ag.hits_measured,
        pct_of(ag.hits_agreeing, ag.hits_measured),
    ));
    if !ag.top_disagreers.is_empty() {
        lines.push(
            "  fattest DISAGREEING shape(s) — canonical attribution mis-ranks their whole count \
             onto one of these clusters:"
                .to_string(),
        );
        for d in &ag.top_disagreers {
            lines.push(format!(
                "    {:>7}×  {:<20} spans {}",
                d.hits,
                d.shape,
                d.clusters.join("  |  ")
            ));
        }
    }

    let out = lines.join("\n");
    if json {
        eprintln!("{out}");
    } else {
        println!("{out}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The verify decision: a re-indented multi-line comment normalizes EQUAL (not a
    /// finding), while a mangle that eats the newline normalizes DIFFERENT (a finding). This
    /// is the property the count-based verify was blind to, so no corpus run grades it.
    #[test]
    fn comment_content_multiset_normalizes_reindent_but_not_mangle() {
        // Re-indent: leading whitespace before `b` changes, newline kept ⇒ conserved.
        let injected = vec!["/* a\n   b */".to_string()];
        let reindented = vec!["/* a\nb */".to_string()];
        assert_eq!(
            comment_content_multiset(&injected),
            comment_content_multiset(&reindented),
            "a re-indent must not read as a change"
        );
        // Mangle: the newline is gone, so the line count drops ⇒ NOT conserved.
        let mangled = vec!["/* ab */".to_string()];
        assert_ne!(
            comment_content_multiset(&injected),
            comment_content_multiset(&mangled),
            "a mangle that collapses the newline must read as a change"
        );
        // A plain drop: the content is simply absent from the output multiset.
        assert_ne!(
            comment_content_multiset(&injected),
            comment_content_multiset(&[]),
            "a dropped comment must read as a change"
        );
        // A duplicate: the same content twice is a distinct multiset from once.
        let once = vec!["/* c */".to_string()];
        let twice = vec!["/* c */".to_string(), "/* c */".to_string()];
        assert_ne!(
            comment_content_multiset(&once),
            comment_content_multiset(&twice),
            "a double-print must read as a change"
        );
    }

    /// A minimal shape carrying `payloads` — only the snapshot columns matter here, so the
    /// example is filler.
    fn mk_agg(payloads: &[&'static str]) -> ShapeAgg {
        ShapeAgg {
            count: 1,
            payloads: payloads.iter().copied().collect(),
            bystander_hits: 0,
            files: BTreeSet::new(),
            examples: vec![Example {
                payload: "block",
                path: "p.svelte".to_string(),
                injection_offset: 0,
                attribution_offset: 0,
                snippet: String::new(),
                text: "/* c */".to_string(),
                injected: true,
            }],
            verify: None,
        }
    }

    /// The snapshot is the gate's on-disk contract: whatever `--update` writes, the gate
    /// must read back as the identical key set, or a green run means nothing.
    #[test]
    fn snapshot_render_and_parse_round_trip() {
        let mut shapes: BTreeMap<(Kind, String), ShapeAgg> = BTreeMap::new();
        shapes.insert((Kind::Dropped, "import⟨⟩.".to_string()), mk_agg(&["block"]));
        shapes.insert(
            (Kind::DoublePrinted, "IDENT⟨⟩=".to_string()),
            mk_agg(&["line", "block"]),
        );

        let r = Ratchet::new(PathBuf::from("/unused"), SNAPSHOT_HEADER, REPIN_HINT);
        let found = snapshot_keys(&shapes);
        let rendered = r.render(&found);
        // Every non-comment line must parse back to a complete key — a dropped column would
        // make the gate silently compare fewer fields than it pins.
        let parsed: BTreeSet<KnownKey> = rendered
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(KnownKey::from_line)
            .collect();

        assert_eq!(parsed, found, "render → parse must round-trip");
        // The payload column is part of the key: same shape, different payload set ⇒
        // different entry, so a shape that starts dropping a new comment kind fails the gate.
        // Kind leads the key, so the DROPPED shape renders before the DOUBLE-PRINTED one.
        assert!(rendered.contains("DROPPED\timport⟨⟩.\tblock\n"));
        assert!(rendered.contains("DOUBLE-PRINTED\tIDENT⟨⟩=\tblock,line\n"));
    }

    /// The committed snapshot's line ORDER is load-bearing for byte-identity: `--update`
    /// renders in [`KnownKey`]'s [`Ord`] order, and `gap_audit_known.txt` is committed in
    /// **`Kind`-enum** order — all `DROPPED`, then all `DOUBLE-PRINTED`. That is NOT
    /// label-string order, which would put `DOUBLE-PRINTED` first (`'O' < 'R'`). Two facts
    /// nothing else pins carry it: (1) `KnownKey.kind` is the [`Kind`] **enum**
    /// (`Dropped` = 0 < `DoublePrinted` = 1), not a label `String`; (2) `kind` is the
    /// **first** field of the derived `Ord`. Flip either — retype `kind` to a `String`, or
    /// reorder the struct fields — and this exact-vector assert fails, where
    /// `snapshot_render_and_parse_round_trip` (order-agnostic `contains`) and the gate
    /// (set-difference grade, order-independent) both stay green, and the break would only
    /// surface as a ~700-line reorder the next time someone runs `--update`. The line text
    /// also locks the `kind<TAB>shape<TAB>payloads` column order.
    #[test]
    fn render_orders_by_kind_enum_not_label() {
        let mut shapes: BTreeMap<(Kind, String), ShapeAgg> = BTreeMap::new();
        // Shapes chosen so enum-order and label-string-order DISAGREE, and a field reorder
        // (shape-first) also flips them: `'I' < 'i'`, so shape order would render the
        // DOUBLE-PRINTED `IDENT⟨⟩=` before the DROPPED `import⟨⟩.`.
        shapes.insert((Kind::Dropped, "import⟨⟩.".to_string()), mk_agg(&["block"]));
        shapes.insert(
            (Kind::DoublePrinted, "IDENT⟨⟩=".to_string()),
            mk_agg(&["line", "block"]),
        );

        let r = Ratchet::new(PathBuf::from("/unused"), SNAPSHOT_HEADER, REPIN_HINT);
        let rendered = r.render(&snapshot_keys(&shapes));
        let data: Vec<&str> = rendered
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        assert_eq!(
            data,
            vec![
                "DROPPED\timport⟨⟩.\tblock",
                "DOUBLE-PRINTED\tIDENT⟨⟩=\tblock,line",
            ],
            "snapshot renders in Kind-enum order (DROPPED before DOUBLE-PRINTED), not \
             label-string order; each line is kind<TAB>shape<TAB>payloads"
        );
    }

    /// A panic must never reach the snapshot — not via `--update`, and not as a key the
    /// gate diffs. The corpus cannot grade this: `tests/fixtures` panics nowhere today, so
    /// both arms are vacuously green there and would stay green if the filter were dropped.
    #[test]
    fn a_panic_is_never_pinned() {
        let mut shapes: BTreeMap<(Kind, String), ShapeAgg> = BTreeMap::new();
        shapes.insert((Kind::Dropped, "import⟨⟩.".to_string()), mk_agg(&["block"]));
        shapes.insert((Kind::Panic, "IDENT⟨⟩(".to_string()), mk_agg(&["block"]));

        let r = Ratchet::new(PathBuf::from("/unused"), SNAPSHOT_HEADER, REPIN_HINT);
        let found = snapshot_keys(&shapes);

        // Not written: a crash must not land in the list whose shrinking is the goal.
        // Checked over the DATA lines, not the whole file — the header explains the panic
        // rule in prose, so a substring search over it matches that and proves nothing.
        let rendered = r.render(&found);
        let data: Vec<&str> = rendered
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        assert_eq!(
            data,
            vec!["DROPPED\timport⟨⟩.\tblock"],
            "only the pinnable shape is written"
        );

        // Nor pinnable: the panic is in the found set but never a pinned key.
        let pinnable: Vec<_> = found.iter().filter(|k| k.is_pinnable()).collect();
        assert_eq!(pinnable.len(), 1, "only the pinnable shape is a key");
        assert!(pinnable.iter().all(|k| k.kind != Kind::Panic));

        assert_eq!(
            count_panics(&shapes),
            1,
            "but it is still counted, and fails"
        );
    }

    /// A full default run, as a baseline for the narrowing cases below.
    fn full_run() -> GapAuditCommand {
        GapAuditCommand {
            json: false,
            report: false,
            by_node: false,
            all_bytes: false,
            payload: None,
            jobs: None,
            limit: 0,
            update: false,
            paths: Vec::new(),
        }
    }

    /// Every flag that changes which shapes a run reaches must be named here, because this
    /// one list decides both whether `--update` may write the snapshot and whether the gate
    /// may grade against it. A flag missing from it silently pins (or grades) a shape set
    /// that isn't the one the snapshot means — `--limit 30 --payload block --update` wrote
    /// an EMPTY snapshot over 717 pinned bugs and reported success.
    #[test]
    fn every_narrowing_flag_disqualifies_a_run() {
        assert!(
            full_run().narrowing_flags().is_empty(),
            "the default run is the one the snapshot describes"
        );

        // Each flag must disqualify a run ON ITS OWN — spelled out rather than looped, so
        // a flag dropped from `narrowing_flags` fails as its own named assertion.
        let paths = GapAuditCommand {
            paths: vec!["src".to_string()],
            ..full_run()
        };
        assert_eq!(paths.narrowing_flags(), vec!["<paths>"]);

        let limit = GapAuditCommand {
            limit: 30,
            ..full_run()
        };
        assert_eq!(limit.narrowing_flags(), vec!["--limit"]);

        let payload = GapAuditCommand {
            payload: Some("block".to_string()),
            ..full_run()
        };
        assert_eq!(payload.narrowing_flags(), vec!["--payload"]);

        // A superset, not a subset — but equally not the pinned set.
        let all_bytes = GapAuditCommand {
            all_bytes: true,
            ..full_run()
        };
        assert_eq!(all_bytes.narrowing_flags(), vec!["--all-bytes"]);

        // `--json` / `--report` / `--by-node` / `--jobs` change how a run is REPORTED and
        // scheduled, never which sites it reaches, so they must not disqualify one — a gate
        // you can't run under --json, with the by-node view, or on a fixed thread count, would
        // just get bypassed.
        let reporting_only = GapAuditCommand {
            json: true,
            report: true,
            by_node: true,
            jobs: Some(1),
            ..full_run()
        };
        assert!(
            reporting_only.narrowing_flags().is_empty(),
            "--json / --report / --by-node / --jobs don't change the shape set"
        );

        // They compose, so the error message can name every offender at once.
        let both = GapAuditCommand {
            limit: 30,
            payload: Some("block".to_string()),
            ..full_run()
        };
        assert_eq!(both.narrowing_flags(), vec!["--limit", "--payload"]);
    }

    /// An [`Example`] at `(path, offset)`, only the fields [`Example::sort_key`] reads matter.
    fn mk_example(path: &str, offset: usize) -> Example {
        Example {
            payload: "block",
            path: path.to_string(),
            injection_offset: offset,
            attribution_offset: offset,
            snippet: String::new(),
            text: "/* c */".to_string(),
            injected: true,
        }
    }

    fn empty_agg() -> ShapeAgg {
        ShapeAgg {
            count: 0,
            payloads: BTreeSet::new(),
            bystander_hits: 0,
            files: BTreeSet::new(),
            examples: Vec::new(),
            verify: None,
        }
    }

    /// The bounded set keeps the `VERIFY_EXAMPLES` smallest by `sort_key`, whatever the
    /// arrival order — the property that makes the kept set (and any diff of it) independent
    /// of `--jobs`.
    #[test]
    fn offer_example_keeps_the_n_smallest_by_sort_key() {
        let mut agg = empty_agg();
        for off in [9, 3, 7, 1, 5, 8, 2, 6, 0, 4] {
            agg.offer_example(mk_example("a.svelte", off));
        }
        let offsets: Vec<usize> = agg.examples.iter().map(|e| e.attribution_offset).collect();
        assert_eq!(offsets, (0..VERIFY_EXAMPLES).collect::<Vec<_>>());
        assert_eq!(
            agg.canonical().attribution_offset,
            0,
            "canonical is the smallest"
        );
    }

    /// A later candidate that TIES an existing one on `sort_key` (same path + offset,
    /// different payload) sorts AFTER it, so the first-seen stays canonical — `examples[0]`
    /// never regresses to a later arrival, matching the old single-example tie-break.
    #[test]
    fn offer_example_ties_keep_the_first_seen_canonical() {
        let mut agg = empty_agg();
        let mut first = mk_example("a.svelte", 0);
        first.payload = "block";
        let mut second = mk_example("a.svelte", 0);
        second.payload = "line";
        agg.offer_example(first);
        agg.offer_example(second);
        assert_eq!(
            agg.examples.len(),
            2,
            "both ties are distinct examples, both kept"
        );
        assert_eq!(
            agg.canonical().payload,
            "block",
            "first-seen stays canonical"
        );
    }

    /// Merging two tallies keeps the `VERIFY_EXAMPLES` smallest across both. Workers take
    /// disjoint files, so the two example sets never share a path — the merged min-N is
    /// determined purely by the global `(path, offset)` set, independent of merge order.
    #[test]
    fn merge_keeps_the_n_smallest_examples_across_tallies() {
        let key = (Kind::Dropped, "IDENT⟨⟩.".to_string());
        let mut a = Tally::default();
        let mut b = Tally::default();
        {
            let mut agg = empty_agg();
            for off in [0, 2, 4, 6, 8] {
                agg.offer_example(mk_example("a.svelte", off));
            }
            a.shapes.insert(key.clone(), agg);
        }
        {
            let mut agg = empty_agg();
            for off in [1, 3, 5, 7, 9] {
                agg.offer_example(mk_example("b.svelte", off));
            }
            b.shapes.insert(key.clone(), agg);
        }
        a.merge(b);
        // `a.svelte` sorts before `b.svelte`, so the five smallest `(path, offset)` keys are
        // all of a's — a deterministic result no thread count changes.
        let got: Vec<(&str, usize)> = a.shapes[&key]
            .examples
            .iter()
            .map(|e| (e.path.as_str(), e.attribution_offset))
            .collect();
        assert_eq!(
            got,
            vec![
                ("a.svelte", 0),
                ("a.svelte", 2),
                ("a.svelte", 4),
                ("a.svelte", 6),
                ("a.svelte", 8),
            ]
        );
    }

    /// The splice-mapping arithmetic — the "corpus can't grade it" class (an offset error
    /// leaves every ASCII file byte-identical, so no corpus run grades it; only this does).
    /// A victim BEFORE the injection maps unchanged; one AT-OR-AFTER `injection + payload_len`
    /// maps back by `payload_len`; an offset inside the payload range, out of range, or
    /// mid-`char` falls back to `None` (the caller's injection-offset keying).
    #[test]
    fn victim_seed_offset_maps_across_the_splice() {
        // 8 ASCII bytes, every offset a char boundary. Injecting a 4-byte payload at offset 3
        // yields `injected = "abc" + PPPP + "defgh"` (length 12).
        let seed = "abcdefgh";
        let inj = 3;
        let plen = 4;

        // Before the splice: unchanged (injected 0..3 == seed 0..3).
        assert_eq!(victim_seed_offset(seed, inj, plen, 0), Some(0));
        assert_eq!(victim_seed_offset(seed, inj, plen, 2), Some(2));

        // At or after the splice: shift back by payload_len. Seed `d` sits at injected 7
        // (3 + 4) and maps back to 3; seed `h` at injected 11 → 7; the seed's end (injected
        // 12) → seed.len() 8.
        assert_eq!(victim_seed_offset(seed, inj, plen, 7), Some(3));
        assert_eq!(victim_seed_offset(seed, inj, plen, 11), Some(7));
        assert_eq!(victim_seed_offset(seed, inj, plen, 12), Some(8));

        // Inside the payload range [3, 7): impossible for a bystander ⇒ None (fallback). The
        // low end (== injection) is the injected comment, already classified `injected`.
        assert_eq!(victim_seed_offset(seed, inj, plen, 3), None);
        assert_eq!(victim_seed_offset(seed, inj, plen, 6), None);

        // Out of range past the seed's end ⇒ None (13 - 4 = 9 > 8).
        assert_eq!(victim_seed_offset(seed, inj, plen, 13), None);

        // Multibyte: a mapped offset that lands mid-`char` falls back to None. `é` is two
        // bytes at seed [1, 3). Injecting a 2-byte payload at 0 → `injected = "PP" + "aébc"`.
        let seed2 = "aébc";
        // Injected 3 → seed 1 (the start of `é`, a boundary) ⇒ mapped.
        assert_eq!(victim_seed_offset(seed2, 0, 2, 3), Some(1));
        // Injected 4 → seed 2, which is mid-`é` ⇒ None.
        assert_eq!(victim_seed_offset(seed2, 0, 2, 4), None);
    }

    /// A bystander hit keys on the VICTIM's site, not the perturbation site. Record two hits
    /// from one injection over `import.x = y.z`: the injected comment (attribution == injection
    /// at the `import.` dot) and a bystander whose victim sits at the `y.z` dot — a DIFFERENT
    /// shape. The bystander must be filed under the victim site's shape, carrying its own
    /// attribution offset while the injection offset survives for reproduction. The corpus
    /// can't grade this: a `record` that keyed on `injection_offset` would land the bystander
    /// under `import⟨⟩.` and the gate would still be green (a shape it already pins).
    #[test]
    fn record_keys_a_bystander_on_the_victim_site() {
        let src = "import.x = y.z";
        let import_dot = src.find(".x").expect("first dot"); // offset 6
        let member_dot = src.rfind(".z").expect("second dot"); // offset 12
        assert_eq!(site_shape(src, import_dot), "import⟨⟩.");
        assert_eq!(site_shape(src, member_dot), "IDENT⟨⟩.");

        let mut tally = Tally::default();
        // The injected comment: attribution == injection at the `import.` dot.
        tally.record(Hit {
            kind: Kind::Dropped,
            payload: Payload::Block,
            path: "p.ts",
            source: src,
            injection_offset: import_dot,
            attribution_offset: import_dot,
            text: "/* c */".to_string(),
            injected: true,
        });
        // A bystander the SAME injection knocked out, whose victim lived at the `y.z` dot.
        tally.record(Hit {
            kind: Kind::Dropped,
            payload: Payload::Block,
            path: "p.ts",
            source: src,
            injection_offset: import_dot,
            attribution_offset: member_dot,
            text: "/* pre-existing */".to_string(),
            injected: false,
        });

        assert!(
            tally
                .shapes
                .contains_key(&(Kind::Dropped, "import⟨⟩.".to_string())),
            "the injected hit keys on the injection site"
        );
        let victim = tally
            .shapes
            .get(&(Kind::Dropped, "IDENT⟨⟩.".to_string()))
            .expect("the bystander keys on the VICTIM site, not the injection site");
        assert_eq!(victim.bystander_hits, 1, "recorded as a bystander");
        assert_eq!(
            victim.canonical().attribution_offset,
            member_dot,
            "the attribution offset is the victim's own site"
        );
        assert_eq!(
            victim.canonical().injection_offset,
            import_dot,
            "the injection offset survives for reproduction"
        );
    }

    /// A shape carrying its resolved cluster set — filler for the agreement arithmetic.
    fn mk_shape(shape: &str, hits: usize, clusters: &[&str]) -> ShapeClusters {
        ShapeClusters {
            kind_label: "DROPPED",
            shape: shape.to_string(),
            hits,
            clusters: clusters.iter().map(|c| (*c).to_string()).collect(),
        }
    }

    /// The agreement tally — the "corpus can't grade it" class: every share is exact yet no
    /// corpus diff would flag an off-by-one, so it is unit-tested directly. A shape with one
    /// resolved cluster AGREES (its whole count belongs on that cluster); one with more DISAGREES
    /// (the canonical rollup mis-ranks it); a fully-unresolved shape is outside BOTH denominators,
    /// exactly as it is outside the cluster totals. Disagreers rank fattest-first, hit-weighted.
    #[test]
    fn aggregate_agreement_splits_agree_disagree_and_ranks_by_hits() {
        let shapes = [
            // Agrees: all kept examples key to one cluster.
            mk_shape("a", 100, &["(CallExpression, callee→arguments)"]),
            // Disagrees, FAT: spans two clusters — this is where mis-ranking concentrates.
            mk_shape(
                "b",
                500,
                &["(CallExpression, callee→arguments)", "(MemberExpression, object→property)"],
            ),
            // Disagrees, thin.
            mk_shape("c", 3, &["(Rule, prelude→block)", "(Identifier, ^→$)"]),
            // Fully unresolved: no example keyed to a node — outside the measured denominator.
            mk_shape("d", 9, &[]),
        ];
        let ag = aggregate_agreement(&shapes);

        // Only `a` agrees; `d` is not measured at all.
        assert_eq!(ag.shapes_agreeing, 1);
        assert_eq!(ag.shapes_measured, 3, "a, b, c — not the unresolved d");
        assert_eq!(ag.hits_agreeing, 100);
        assert_eq!(ag.hits_measured, 603, "100 + 500 + 3, excluding d's 9");

        // The two disagreers, fattest first.
        assert_eq!(ag.top_disagreers.len(), 2);
        assert_eq!(ag.top_disagreers[0].shape, "b");
        assert_eq!(ag.top_disagreers[0].hits, 500);
        assert_eq!(ag.top_disagreers[0].clusters.len(), 2);
        assert_eq!(ag.top_disagreers[1].shape, "c");

        // The shares the report headlines with.
        assert_eq!(pct_of(ag.shapes_agreeing, ag.shapes_measured), 33);
        assert!(approx(share_of(ag.hits_agreeing, ag.hits_measured), 0.1658));
    }

    /// A four-decimal share compares within one ULP-ish epsilon — `assert_eq!` on `f64` trips
    /// clippy's `float_cmp` and is brittle regardless.
    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// `share_of` guards its zero denominator and rounds to four decimals; `pct_of` likewise.
    #[test]
    fn share_and_pct_guard_zero_denominator() {
        assert!(approx(share_of(0, 0), 0.0));
        assert_eq!(pct_of(0, 0), 0);
        assert!(approx(share_of(1, 3), 0.3333));
        assert!(approx(share_of(2, 4), 0.5));
        assert_eq!(pct_of(1, 3), 33);
    }
}
