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
//! So sites come from [`code_regions`] — the spans the AST says are JS or CSS — and inside
//! those two existing layers filter for free:
//!
//! - **inside a word** (`fo/* c */o` → `fo o`) — the parser rejects it, so the site is
//!   skipped. Correctly: that gap exists in no real document.
//! - **inside a string literal** (`"fo/* c */o"`) — parses, but the injected text is never
//!   *lexed* as a comment, so the ledger registers nothing and reports nothing.
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
//! The audit inherits **the ledger's scope** exactly: only the **detached** comments a
//! format entry registers. A Svelte `<!-- … -->` and a CSS in-block comment are AST nodes
//! carried by the tree, and a CSS declaration's *value* comments are never lexed as
//! `Comment`s at all — all outside the model by construction (see [`comment_ledger`]'s
//! module docs). So this speaks for the detached-comment model — the class that bit us
//! eight times — and not for CSS values. CSS also has no line comments, so the `line`
//! payload is inert in a `.css` file (harmless: it simply never registers).
//!
//! It also inherits **[`code_regions`]' reach**: a gap the region walk doesn't name is a
//! gap never probed. Today that means a `.svelte` file's `<style>` content is unprobed, so
//! a Svelte file containing only a `<style>` block yields **zero sites** — see that
//! function's TODO for why the ledger's scope, not difficulty, is what holds it back.

use argh::FromArgs;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::cli::CliError;
use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;
use tsv_lang::comment_ledger::{self, CommentFinding, CommentFindingKind};

use super::profile::resolve_files;
use super::roundtrip_audit::tsv_parse_to_value;

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

    /// inject at EVERY char boundary, including positions strictly inside a word.
    /// A diagnostic, not a stricter mode: it does surface extra shapes, but on the
    /// corpus they are artifacts of mutilating an existing comment, not gap bugs
    /// (see `injection_sites`)
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

/// The committed snapshot of every finding shape `tests/fixtures` currently produces —
/// the ratchet `deno task check` gates on.
///
/// **Machine-generated** (`deno task gaps:audit:update`), unlike
/// [`scan_audit`](super::scan_audit)'s hand-curated `ALLOW`: at ~700 shapes a per-entry
/// rationale is not a thing a human can keep honest, so this deliberately carries none. It
/// is a ratchet, not a sanction — every line is a **known bug**, and the file shrinking is
/// the goal.
///
/// What it buys: a shape that is not on the list fails the gate, so no *new* kind of drop
/// lands silently. What it does not: a new drop at an **existing** shape is invisible (the
/// key is the shape, not the count — counts churn with every ordinary fixture PR, and a
/// gate that fails per added fixture would just be turned off). Fixing a shape must remove
/// its line; a stale entry fails the gate too, so the list can't rot.
const KNOWN_SHAPES: &str = include_str!("gap_audit_known.txt");

/// Where [`KNOWN_SHAPES`] lives, for `--update` to rewrite.
fn known_shapes_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/commands/gap_audit_known.txt")
}

/// One snapshot line: what the ratchet actually pins.
///
/// The payload set is part of the key, not decoration. A shape that drops only a `line`
/// comment today and starts dropping a `block` one tomorrow is a **new bug on a new
/// ownership path** — keyed on the shape alone it would land inside an existing entry and
/// never be seen. It is also stable in the way a count is not: it changes when the bug's
/// character changes, not when a fixture is added.
type KnownKey = (String, String, String);

/// Render a payload set into its snapshot column.
fn payload_column(payloads: &BTreeSet<&'static str>) -> String {
    payloads.iter().copied().collect::<Vec<_>>().join(",")
}

/// Whether a shape is something the snapshot may pin — everything but a [`Kind::Panic`].
///
/// A panic is not a "known bug" to ratchet alongside the drops. The invariant it breaks is
/// **absolute** (a comment in a gap must never crash the formatter), so it always fails the
/// gate and is never pinnable — otherwise `--update` would quietly absorb a crash into the
/// same list whose shrinking is the goal. [`render_known`] and [`found_keys`] share this
/// filter so the two stay in lockstep: a panic can't be written, so it can never read back
/// as stale.
fn is_pinnable(kind: Kind) -> bool {
    kind != Kind::Panic
}

/// How many of `shapes` crash the formatter — the shapes [`is_pinnable`] keeps out of the
/// snapshot, and which therefore need their own accounting on every exit path.
fn count_panics(shapes: &BTreeMap<(Kind, String), ShapeAgg>) -> usize {
    shapes.keys().filter(|(k, _)| *k == Kind::Panic).count()
}

/// What the ratchet found, computed **before** anything prints.
///
/// Grading is split from reporting for one reason: a ratchet that holds has nothing to act
/// on, so printing its ~700 known shapes is thousands of lines of noise inside
/// `deno task check` — and whether it holds is only knowable after the diff. Deciding first
/// lets a clean gate print a summary instead.
struct GateDiff {
    /// How many shapes the snapshot pins — the denominator in the ✓ line.
    known: usize,
    /// Shapes the snapshot has never seen: a new kind of drop.
    new: Vec<KnownKey>,
    /// Pinned shapes that no longer fire — a fixed bug, or a rotting list.
    stale: Vec<KnownKey>,
    /// Crashes. Never pinned (see [`is_pinnable`]), so they are graded on their own.
    panics: usize,
}

impl GateDiff {
    fn holds(&self) -> bool {
        self.new.is_empty() && self.stale.is_empty() && self.panics == 0
    }
}

/// Diff a run's shapes against the committed snapshot. Pure — it prints nothing and decides
/// nothing; see [`GateDiff`].
fn grade_against_snapshot(shapes: &BTreeMap<(Kind, String), ShapeAgg>) -> GateDiff {
    let known = parse_known();
    let found = found_keys(shapes);
    GateDiff {
        new: found.difference(&known).cloned().collect(),
        stale: known.difference(&found).cloned().collect(),
        panics: count_panics(shapes),
        known: known.len(),
    }
}

/// Parse the snapshot into its keys.
fn parse_known() -> BTreeSet<KnownKey> {
    KNOWN_SHAPES
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut cols = l.split('\t');
            let kind = cols.next()?.to_string();
            let shape = cols.next()?.to_string();
            let payloads = cols.next()?.to_string();
            Some((kind, shape, payloads))
        })
        .collect()
}

/// The keys a run's shapes produce — the pinnable ones (see [`is_pinnable`]).
fn found_keys(shapes: &BTreeMap<(Kind, String), ShapeAgg>) -> BTreeSet<KnownKey> {
    shapes
        .iter()
        .filter(|((kind, _), _)| is_pinnable(*kind))
        .map(|((kind, shape), agg)| {
            (
                kind.label().to_string(),
                shape.clone(),
                payload_column(&agg.payloads),
            )
        })
        .collect()
}

/// Render the snapshot file for `shapes`.
fn render_known(shapes: &BTreeMap<(Kind, String), ShapeAgg>) -> String {
    let mut out = String::new();
    out.push_str(
        "# Generated by `deno task gaps:audit:update` — do NOT hand-edit.\n\
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
         # Format: KIND<TAB>SHAPE<TAB>PAYLOADS\n",
    );
    for ((kind, shape), agg) in shapes.iter().filter(|((k, _), _)| is_pinnable(*k)) {
        out.push_str(kind.label());
        out.push('\t');
        out.push_str(shape);
        out.push('\t');
        out.push_str(&payload_column(&agg.payloads));
        out.push('\n');
    }
    out
}

/// Whether `c` can sit inside an identifier — the site filter's notion of "in a word".
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// `(start, end)` of a wire node, when it carries a span — in the wire's own position
/// space, **not** bytes. See [`Utf16ToByte`].
fn span_of(node: &serde_json::Value) -> Option<(usize, usize)> {
    let s = node.get("start")?.as_u64()? as usize;
    let e = node.get("end")?.as_u64()? as usize;
    (e >= s).then_some((s, e))
}

/// Translates the wire AST's positions into byte offsets.
///
/// The wire emits **UTF-16 code-unit** offsets (`tsv_lang::location::ByteToCharMap`), not
/// byte offsets — they coincide on ASCII and diverge the moment a file holds a `é` or an
/// emoji. Slicing `source` with a raw wire offset is then off by the multi-byte count:
/// wrong regions, or a panic on a non-char-boundary. Nothing downstream can catch that —
/// an ASCII-only corpus grades identical either way — so the map is unit-tested against a
/// direct `char_indices` walk instead.
struct Utf16ToByte {
    /// `None` for an all-ASCII source, where the two spaces are identical and the table is
    /// pure overhead (the overwhelmingly common case).
    table: Option<Vec<usize>>,
    len: usize,
}

impl Utf16ToByte {
    fn new(source: &str) -> Self {
        if source.is_ascii() {
            return Self {
                table: None,
                len: source.len(),
            };
        }
        // One entry per UTF-16 code unit; an astral char spans two units and both map to
        // the char's byte start, so a boundary offset always lands on a char boundary.
        let mut table = Vec::with_capacity(source.len() + 1);
        for (byte, ch) in source.char_indices() {
            for _ in 0..ch.len_utf16() {
                table.push(byte);
            }
        }
        table.push(source.len());
        Self {
            table: Some(table),
            len: source.len(),
        }
    }

    /// The byte offset for a wire offset, or `None` if it is out of range.
    fn byte(&self, wire: usize) -> Option<usize> {
        match &self.table {
            None => (wire <= self.len).then_some(wire),
            Some(t) => t.get(wire).copied(),
        }
    }
}

/// Collect the ranges the payload would actually be **lexed as a comment** in.
///
/// For a `.ts` / `.css` file that is the whole file. For `.svelte` it must be asked of the
/// AST, because the markup around the code is *not* a comment context — see the module docs
/// on why tsv's own acceptance can't be used for this.
///
/// Svelte regions come from **two walks**, because no one AST expresses them all:
///
/// - [`collect_regions`] over the **wire** shape names the two carriers a canonical node's
///   own span already is: a `Script`'s `content` (the `Program` span is exactly the
///   `>`-to-`</script>` region), and an `ExpressionTag`'s brace interior
///   (`{ /* c */ x.y }` is legal). It finds them by recursive walk, so it cannot miss a
///   path an `ExpressionTag` hides in (an attribute value, a `<svelte:element this={…}>`).
/// - [`svelte_only_regions`] over tsv's **internal** AST names the rest, which exist only
///   as tsv's own parse bookkeeping: a block's `opening_tag_span` and a directive's
///   `head_span`. Svelte's AST carries neither, so the wire cannot express them — an
///   `IfBlock`'s span covers the whole block (body included) and its `test` span is the
///   expression alone, so the head is not derivable from either without a scan.
///
/// TODO: `<style>` content is still unnamed, so no comment is probed there. `Style` carries
/// a `content_span` that names it in one line — the reason to hold off is **yield**, not
/// difficulty: measured over `tests/fixtures` it is +154k sites (+20% gate runtime) for 3
/// finding shapes, all `@import`-prelude double-prints. That thinness is structural, not
/// incidental: the ledger only registers **detached** comments, while a CSS in-block comment
/// is a `CssBlockChild::Comment` AST node and a declaration-VALUE comment is never lexed as
/// a `Comment` at all. Probing `<style>` mostly tests the registration gap, so the honest
/// prerequisite is extending the ledger to AST-node comments (see `comment_ledger`'s own
/// TODO) — after which this region earns its cost.
fn code_regions(source: &str, parser: ParserType) -> Vec<(usize, usize)> {
    match parser {
        ParserType::TypeScript | ParserType::Css => vec![(0, source.len())],
        ParserType::Svelte => {
            let Some(wire) = tsv_parse_to_value(source, parser) else {
                return Vec::new();
            };
            let mut wire_spans = Vec::new();
            collect_regions(&wire, &mut wire_spans);
            let map = Utf16ToByte::new(source);
            let mut byte_spans: Vec<(usize, usize)> = wire_spans
                .into_iter()
                .filter_map(|(s, e)| Some((map.byte(s)?, map.byte(e)?)))
                .collect();
            byte_spans.extend(svelte_only_regions(source));
            merge_regions(byte_spans)
        }
    }
}

/// The Svelte regions the wire shape cannot express — read off tsv's internal AST.
///
/// Every one is a **head**: the run from a construct's opening delimiter to the code it
/// introduces. Svelte's public AST records only the finished expression, so a head is not
/// derivable from it; tsv's parser already keeps the two spans this needs
/// (`opening_tag_span`, `head_span`) for its own comment lookup.
///
/// **Interiors only** — never the enclosing delimiter's outside. A tag's `}` is where the
/// code region ends; the byte *after* it is markup (harmless, but noise), and for a
/// directive it is the middle of an element tag, where tsv over-accepts a comment Svelte
/// would reject (the `<script lang="ts"/* c */>` class the module docs name). So a block /
/// tag contributes `span` minus its two delimiters, and a directive contributes
/// `head_span.end ..= span.end - 1`, which stops on the closing `}`.
///
/// What this deliberately does **not** do is filter the positions within a head where a
/// comment is illegal — `{#each list as ⟨⟩item}` and `{#await p then ⟨⟩v}` are Svelte's
/// own hand-read pattern slots, not acorn's, and it rejects a comment in them. No
/// whitelist is needed because **tsv rejects there too**, so `Formatted::Rejected` filters
/// them exactly as it does a word interior. The same covers `{⟨⟩#if` and `{#⟨⟩if`.
fn svelte_only_regions(source: &str) -> Vec<(usize, usize)> {
    use tsv_svelte::ast::internal::{AttributeNode, Fragment, FragmentNode, SpecialElementKind};

    /// A `{…}`-delimited construct: its interior, both delimiters excluded.
    fn interior(span: tsv_lang::Span, out: &mut Vec<(usize, usize)>) {
        let (s, e) = (span.start as usize, span.end as usize);
        if e > s + 1 {
            out.push((s + 1, e - 1));
        }
    }

    /// A span taken as-is (a bare expression already bounded by its delimiters).
    fn span_of(span: tsv_lang::Span, out: &mut Vec<(usize, usize)>) {
        if span.end > span.start {
            out.push((span.start as usize, span.end as usize));
        }
    }

    fn attributes(attrs: &[AttributeNode<'_>], out: &mut Vec<(usize, usize)>) {
        for a in attrs {
            match a {
                // `{...rest}` / `{@attach f()}` — brace-delimited, like an ExpressionTag.
                AttributeNode::SpreadAttribute(x) => interior(x.span, out),
                AttributeNode::AttachTag(x) => interior(x.span, out),
                // A directive's value: `on:click⟨={handler}⟩`. Bounded below by the head
                // (`on:click|once` is not a comment context) and above by the closing `}`.
                AttributeNode::OnDirective(x) => directive_value(x.head_span, x.span, out),
                AttributeNode::BindDirective(x) => directive_value(x.head_span, x.span, out),
                AttributeNode::ClassDirective(x) => directive_value(x.head_span, x.span, out),
                AttributeNode::StyleDirective(x) => directive_value(x.head_span, x.span, out),
                AttributeNode::UseDirective(x) => directive_value(x.head_span, x.span, out),
                AttributeNode::TransitionDirective(x) => directive_value(x.head_span, x.span, out),
                AttributeNode::AnimateDirective(x) => directive_value(x.head_span, x.span, out),
                AttributeNode::LetDirective(x) => directive_value(x.head_span, x.span, out),
                // A plain attribute's expression value is an `ExpressionTag`, which the
                // wire walk already names.
                AttributeNode::Attribute(_) => {}
            }
        }
    }

    /// `head_span.end ..= span.end - 1` — the `={expr}` run, stopping on the `}`. Empty for
    /// a shorthand directive (`bind:value`), which has no value to probe.
    fn directive_value(head: tsv_lang::Span, span: tsv_lang::Span, out: &mut Vec<(usize, usize)>) {
        let (s, e) = (head.end as usize, span.end as usize);
        if e > s + 1 {
            out.push((s, e - 1));
        }
    }

    fn walk(frag: &Fragment<'_>, out: &mut Vec<(usize, usize)>) {
        for node in frag.nodes {
            match node {
                FragmentNode::IfBlock(b) => {
                    interior(b.opening_tag_span, out);
                    walk(&b.consequent, out);
                    if let Some(alt) = &b.alternate {
                        walk(alt, out);
                    }
                }
                FragmentNode::EachBlock(b) => {
                    interior(b.opening_tag_span, out);
                    walk(&b.body, out);
                    if let Some(fallback) = &b.fallback {
                        walk(fallback, out);
                    }
                }
                FragmentNode::AwaitBlock(b) => {
                    interior(b.opening_tag_span, out);
                    for f in [&b.pending, &b.then, &b.catch].into_iter().flatten() {
                        walk(f, out);
                    }
                }
                FragmentNode::KeyBlock(b) => {
                    interior(b.opening_tag_span, out);
                    walk(&b.fragment, out);
                }
                FragmentNode::SnippetBlock(b) => {
                    interior(b.opening_tag_span, out);
                    walk(&b.body, out);
                }
                // `{@html x}` / `{@const a = b}` / `{@render f()}` / `{@debug a}` — the
                // whole tag is one brace-delimited head, expression included.
                FragmentNode::HtmlTag(t) => interior(t.span, out),
                FragmentNode::ConstTag(t) => interior(t.span, out),
                FragmentNode::DeclarationTag(t) => interior(t.span, out),
                FragmentNode::DebugTag(t) => interior(t.span, out),
                FragmentNode::RenderTag(t) => interior(t.span, out),
                FragmentNode::Element(e) => {
                    attributes(e.attributes, out);
                    walk(&e.fragment, out);
                }
                FragmentNode::SpecialElement(e) => {
                    // `<svelte:element this={tag}>` / `<svelte:component this={x}>` hold
                    // their expression **bare**, not wrapped in an `ExpressionTag` — so the
                    // wire walk does not name it and this is its only cover. The expression
                    // span alone is the region: its ends already sit against the two
                    // braces, so the brace-adjacent gaps come along.
                    // Listed exhaustively rather than with a `_` arm, deliberately: a
                    // future variant that carries an expression would otherwise go
                    // unprobed **silently**, which is the exact failure this walk exists to
                    // fix. Let it break the build instead.
                    match &e.kind {
                        SpecialElementKind::SvelteElement { tag } => span_of(tag.span(), out),
                        SpecialElementKind::SvelteComponent { expression } => {
                            span_of(expression.span(), out);
                        }
                        SpecialElementKind::SvelteHead
                        | SpecialElementKind::SvelteWindow
                        | SpecialElementKind::SvelteBody
                        | SpecialElementKind::SvelteDocument
                        | SpecialElementKind::SvelteSelf
                        | SpecialElementKind::SlotElement
                        | SpecialElementKind::SvelteFragment
                        | SpecialElementKind::SvelteBoundary
                        | SpecialElementKind::TitleElement => {}
                    }
                    attributes(e.attributes, out);
                    walk(&e.fragment, out);
                }
                FragmentNode::ExpressionTag(_)
                | FragmentNode::Text(_)
                | FragmentNode::Comment(_) => {}
            }
        }
    }

    let arena = bumpalo::Bump::new();
    // A parse failure is not this function's business to report: the caller already skipped
    // any seed file tsv rejects, and an injected source that stops parsing is a `Rejected`
    // the inject loop drops on the floor.
    let Ok(root) = tsv_svelte::parse(source, &arena) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    walk(&root.fragment, &mut out);
    out
}

/// Walk the wire AST accumulating [`code_regions`]' carriers.
fn collect_regions(node: &serde_json::Value, out: &mut Vec<(usize, usize)>) {
    match node {
        serde_json::Value::Object(obj) => {
            match obj.get("type").and_then(serde_json::Value::as_str) {
                Some("Script") => {
                    if let Some(span) = obj.get("content").and_then(span_of) {
                        out.push(span);
                    }
                }
                // The braces themselves aren't a comment context; their interior is.
                Some("ExpressionTag") => {
                    if let Some((s, e)) = span_of(node)
                        && e > s + 1
                    {
                        out.push((s + 1, e - 1));
                    }
                }
                _ => {}
            }
            for (k, v) in obj {
                if k != "loc" {
                    collect_regions(v, out);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for v in items {
                collect_regions(v, out);
            }
        }
        _ => {}
    }
}

/// Sort and coalesce overlapping/adjacent ranges, so a site is never injected twice.
fn merge_regions(mut regions: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    regions.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(regions.len());
    for (s, e) in regions {
        match merged.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    merged
}

/// The byte offsets to inject at: every `char` boundary within a code region, minus the
/// ones strictly **inside a word**.
///
/// The word filter keeps every punctuator- and whitespace-adjacent offset, so `describe`
/// `.` `only` retains **both** dot gaps — the exact positions the class hides in — while the
/// two word interiors go. Splitting `describe` into `desc/* c */ribe` probes a gap that
/// exists in no real document. Worth ~2.2× on real source.
///
/// **It is a heuristic, not a proof, and `--all-bytes` disagrees with it.** On the corpus
/// the extra sites yield a handful of extra shapes, and every one inspected was an artifact
/// of injecting *into an existing comment's text* — `// after empty i⟨⟩nit` with the `line`
/// payload terminates the author's comment early and turns `nit` into code, which then reads
/// as that comment being dropped. Junk, correctly excluded, but by accident: the filter
/// screens word interiors and comment prose is mostly words.
///
/// TODO: exclude sites inside an existing comment's span outright. The word filter misses
/// the punctuator boundaries within one (`/* c1 ⟨⟩*/`), so that artifact class is only
/// mostly suppressed. It needs the parsed comment list, which the ledger holds but does not
/// expose.
fn injection_sites(source: &str, regions: &[(usize, usize)], all_bytes: bool) -> Vec<usize> {
    let mut sites = Vec::new();
    for &(start, end) in regions {
        let mut prev: Option<char> = source[..start].chars().next_back();
        // Inclusive of `end`: the last offset of a region is a gap like any other (the
        // position just before `</script>` is where a trailing comment goes).
        for (i, ch) in source[start..end].char_indices() {
            if all_bytes || !(prev.is_some_and(is_word) && is_word(ch)) {
                sites.push(start + i);
            }
            prev = Some(ch);
        }
        let tail_is_word = source[end..].chars().next().is_some_and(is_word);
        if all_bytes || !(prev.is_some_and(is_word) && tail_is_word) {
            sites.push(end);
        }
    }
    sites
}

/// Words kept verbatim in a [`site_shape`] rather than abstracted to `IDENT`.
///
/// Heuristic and deliberately generous: a shape is a **report/dedup key**, not a parse. The
/// point is that `import⟨⟩.` and `IDENT⟨⟩.` name different bugs — the meta-property/import-
/// phase header versus every member access in the corpus — so a keyword that heads a
/// concatenated construct must survive. `source` / `defer` / `meta` / `target` are here for
/// exactly that reason despite being contextual keywords.
const SHAPE_KEYWORDS: &[&str] = &[
    "abstract",
    "as",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "constructor",
    "continue",
    "declare",
    "default",
    "defer",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "finally",
    "for",
    "from",
    "function",
    "get",
    "global",
    "if",
    "implements",
    "import",
    "in",
    "infer",
    "instanceof",
    "interface",
    "is",
    "keyof",
    "let",
    "meta",
    "module",
    "namespace",
    "new",
    "of",
    "out",
    "private",
    "protected",
    "public",
    "readonly",
    "require",
    "return",
    "satisfies",
    "set",
    "source",
    "static",
    "super",
    "switch",
    "target",
    "this",
    "throw",
    "try",
    "type",
    "typeof",
    "unique",
    "var",
    "void",
    "while",
    "yield",
];

/// The word ending at `end`, or `None` when the char before `end` isn't identifier-ish.
fn word_before(source: &str, end: usize) -> Option<&str> {
    let start = source[..end]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c))
        .map(|(i, _)| i)
        .last()?;
    Some(&source[start..end])
}

/// The word starting at `start`, or `None` when the char at `start` isn't identifier-ish.
fn word_after(source: &str, start: usize) -> Option<&str> {
    let len: usize = source[start..]
        .chars()
        .take_while(|c| is_word(*c))
        .map(char::len_utf8)
        .sum();
    (len > 0).then(|| &source[start..start + len])
}

/// Render one side of a shape: a keyword verbatim, any other word as `IDENT`.
fn shape_word(w: &str) -> String {
    if SHAPE_KEYWORDS.contains(&w) {
        w.to_string()
    } else if w.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        "NUM".to_string()
    } else {
        "IDENT".to_string()
    }
}

/// The non-word, non-whitespace run (a punctuator) ending at `end`, capped at 3 chars.
fn punct_before(source: &str, end: usize) -> String {
    let s: String = source[..end]
        .chars()
        .rev()
        .take_while(|c| !is_word(*c) && !c.is_whitespace())
        .take(3)
        .collect();
    s.chars().rev().collect()
}

/// The non-word, non-whitespace run (a punctuator) starting at `start`, capped at 3 chars.
fn punct_after(source: &str, start: usize) -> String {
    source[start..]
        .chars()
        .take_while(|c| !is_word(*c) && !c.is_whitespace())
        .take(3)
        .collect()
}

/// A compact, **file-independent** name for an injection position: the source token on
/// each side, with identifiers abstracted.
///
/// This is the dedup key of the whole report. One bug fires at every site that reaches it —
/// a member-access drop would land thousands of times across the corpus — so raw
/// `(file, offset)` findings are unreadable and, as a ratchet key, would go stale on the
/// next fixture edit. A shape collapses those to one line: `import⟨⟩.`, `IDENT⟨⟩=`,
/// `.⟨⟩IDENT`. Whitespace is elided rather than represented, since a gap's *width* is not
/// what distinguishes the position.
fn site_shape(source: &str, offset: usize) -> String {
    let before = word_before(source, offset).map_or_else(
        || {
            let p = punct_before(source, offset);
            if p.is_empty() { "␣".to_string() } else { p }
        },
        shape_word,
    );
    let after = word_after(source, offset).map_or_else(
        || {
            let p = punct_after(source, offset);
            if p.is_empty() { "␣".to_string() } else { p }
        },
        shape_word,
    );
    format!("{before}⟨⟩{after}")
}

/// A readable source window around an injection point — the eyeball companion to the
/// abstracted [`site_shape`], so a finding can be reproduced by hand.
fn snippet(source: &str, offset: usize) -> String {
    let lo = (0..=offset)
        .rev()
        .find(|i| source.is_char_boundary(*i) && offset - i >= 28)
        .unwrap_or(0);
    let hi = (offset..=source.len())
        .find(|i| source.is_char_boundary(*i) && i - offset >= 28)
        .unwrap_or(source.len());
    let one_line = |s: &str| s.replace('\n', "⏎").replace('\t', "→");
    format!(
        "{}⟨⟩{}",
        one_line(&source[lo..offset]),
        one_line(&source[offset..hi])
    )
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
    offset: usize,
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
    /// picking the smallest `(path, offset)` instead of "whoever merged first" keeps a
    /// report (and any diff of one) stable across `--jobs 1` and `--jobs 12`.
    fn sort_key(&self) -> (&str, usize) {
        (&self.path, self.offset)
    }
}

/// Everything a shape accumulates. Counts stay exact; only one example is kept, so a
/// corpus that fires a bug a million times still reports in constant memory.
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
    example: Example,
    /// The in-run self-verification verdict — `None` until the verify pass runs.
    verdict: Option<Verdict>,
}

/// Whether a shape's example survives an **observational** re-check, independent of the
/// ledger that reported it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Verdict {
    /// Re-formatting really does lose (or duplicate) a comment. The ledger's claim is
    /// visible in the output.
    Confirmed,
    /// The ledger says a comment was never emitted, yet the output holds just as many
    /// comments as its input. Something printed it without recording the emit — or printed
    /// a *mangled* rebuild of it (`/* a⏎b */` → `/* ab */`, one comment either way). Real
    /// either way, but not the plain drop it is filed as.
    Unconfirmed,
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
    offset: usize,
    /// The offending comment's text, which is the *injected* payload only when
    /// [`Self::injected`] holds.
    text: String,
    /// Whether the offending comment is the injected one rather than a bystander.
    injected: bool,
}

impl Tally {
    fn record(&mut self, hit: Hit<'_>) {
        let shape = site_shape(hit.source, hit.offset);
        let candidate = Example {
            payload: hit.payload.label(),
            path: hit.path.to_string(),
            offset: hit.offset,
            snippet: snippet(hit.source, hit.offset),
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
                example: candidate.clone(),
                verdict: None,
            });
        e.count += 1;
        if !hit.injected {
            e.bystander_hits += 1;
        }
        e.payloads.insert(hit.payload.label());
        e.files.insert(hit.path.to_string());
        if candidate.sort_key() < e.example.sort_key() {
            e.example = candidate;
        }
    }

    fn merge(&mut self, other: Tally) {
        self.sites += other.sites;
        self.injections += other.injections;
        self.accepted += other.accepted;
        self.files_done += other.files_done;
        self.parse_skipped += other.parse_skipped;
        self.dirty_files.extend(other.dirty_files);
        for (k, v) in other.shapes {
            match self.shapes.get_mut(&k) {
                Some(e) => {
                    e.count += v.count;
                    e.bystander_hits += v.bystander_hits;
                    e.payloads.extend(v.payloads);
                    e.files.extend(v.files);
                    // Smallest (path, offset) wins, NOT whoever merged first — see
                    // `Example::sort_key`.
                    if v.example.sort_key() < e.example.sort_key() {
                        e.example = v.example;
                    }
                }
                None => {
                    self.shapes.insert(k, v);
                }
            }
        }
    }
}

/// What one ledger-armed format did.
enum Formatted {
    /// The parser or printer panicked — a finding in its own right (a comment in a gap
    /// must never crash the formatter).
    Panicked,
    /// The source did not parse, so the injection is not a legal comment here. The
    /// overwhelmingly common case, and **not** a finding: it means the offset names no gap.
    Rejected,
    /// Formatted.
    Ok {
        /// The ledger's findings — normally empty.
        findings: Vec<CommentFinding>,
        /// How many comments the document registered. Doubles as a needle-free "how many
        /// comments are in this text" measure: `ledger_format(text).parsed` counts them
        /// with the real lexer, so [`verify_example`] never has to string-match a comment
        /// whose text the printer may legitimately re-indent.
        parsed: usize,
        /// The formatted text, already built by `format_source` — free to carry.
        output: String,
    },
}

/// Format `src` with the ledger armed and drain it.
///
/// Drains on every path, including the failing ones: the ledger is thread-local and keyed
/// on source identity, so a straggler left by a rejected parse could otherwise be attributed
/// to the next injection.
fn ledger_format(src: &str, parser: ParserType) -> Formatted {
    let _ = comment_ledger::take_comment_ledger();
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| format_source(src, parser)));
    match result {
        Err(_) => {
            let _ = comment_ledger::take_comment_ledger();
            Formatted::Panicked
        }
        Ok(Err(_)) => {
            let _ = comment_ledger::take_comment_ledger();
            Formatted::Rejected
        }
        Ok(Ok(output)) => {
            let ledger = comment_ledger::take_comment_ledger();
            Formatted::Ok {
                findings: ledger.findings,
                parsed: ledger.parsed,
                output,
            }
        }
    }
}

/// Re-derive a finding's **observable** claim, independently of the ledger that made it.
///
/// The ledger is an instrument, and an instrument that only ever agrees with itself is not
/// evidence — every mistake found while building this audit was of exactly that shape (a
/// stale needle, a char-vs-byte offset, checking the injected comment when the finding was
/// about a bystander). So each shape's example is re-run and the ledger is made to *predict*
/// something falsifiable: if it says this format drops `d` comments and double-prints `p`,
/// then the output must reparse to exactly `parsed - d + p` comments. Anything else means
/// the ledger's account and the actual output disagree.
///
/// Counting via a reparse rather than by matching the comment's text is what makes this
/// sound: a printer may legitimately re-indent a multi-line comment, so text matching
/// produces false alarms, while the count is exact. It is also what leaves the
/// [`Verdict::Unconfirmed`] bucket meaningful — a *mangled* rebuild is still one comment, so
/// it shows up as "the ledger says dropped but nothing is missing", which is precisely the
/// signal wanted.
///
/// The residual blind spot, named rather than hidden: counts can balance. A format that
/// drops one comment and duplicates another nets to zero and reads as confirmed-ish. No
/// example in the corpus does this today, and the per-shape example is a sample of the
/// shape's hits, never a proof about all of them.
fn verify_example(agg: &ShapeAgg, kind: Kind, parser: ParserType) -> Verdict {
    // A panic is self-evident: it either happens or it doesn't, and it was caught to get here.
    if kind == Kind::Panic {
        return Verdict::Confirmed;
    }
    let Ok(source) = std::fs::read_to_string(&agg.example.path) else {
        return Verdict::Unconfirmed;
    };
    let Some(payload) = Payload::from_label(agg.example.payload) else {
        return Verdict::Unconfirmed;
    };
    let offset = agg.example.offset;
    if offset > source.len() || !source.is_char_boundary(offset) {
        return Verdict::Unconfirmed;
    }
    let mut injected = String::with_capacity(source.len() + 24);
    injected.push_str(&source[..offset]);
    injected.push_str(payload.text());
    injected.push_str(&source[offset..]);

    let Formatted::Ok {
        findings,
        parsed,
        output,
    } = ledger_format(&injected, parser)
    else {
        return Verdict::Unconfirmed;
    };
    if findings.is_empty() {
        // The example no longer fires at all — the ledger and the re-run disagree outright.
        return Verdict::Unconfirmed;
    }
    let Formatted::Ok {
        parsed: reparsed, ..
    } = ledger_format(&output, parser)
    else {
        // The formatter's own output doesn't parse. A real bug, but `roundtrip_audit`'s.
        return Verdict::Unconfirmed;
    };

    if reparsed == predict_comment_count(parsed, &findings) {
        Verdict::Confirmed
    } else {
        Verdict::Unconfirmed
    }
}

/// How many comments the output must hold, if the ledger's account of `findings` is true.
///
/// Each dropped comment removes one. Each double-printed one adds `emitted - 1` — **not**
/// one: `CommentFinding::emitted` is documented as `>= 2`, so a comment printed three times
/// adds two, and assuming "double" means exactly twice would mispredict it as
/// [`Verdict::Unconfirmed`].
///
/// Split out and unit-tested because it is arithmetic: an off-by-one here changes a verdict
/// and nothing else, and no corpus run would show it — the audit would simply file a
/// confirmed finding under the wrong bucket.
fn predict_comment_count(parsed: usize, findings: &[CommentFinding]) -> usize {
    let dropped = findings
        .iter()
        .filter(|f| f.kind == CommentFindingKind::Dropped)
        .count();
    let extra: usize = findings
        .iter()
        .filter(|f| f.kind == CommentFindingKind::DoublePrinted)
        .map(|f| f.emitted.saturating_sub(1))
        .sum();
    // `dropped` counts registered comments, so it can never exceed `parsed`; saturate
    // rather than risk a panic on a ledger that ever breaks that invariant.
    parsed.saturating_sub(dropped) + extra
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

    match ledger_format(&source, parser) {
        Formatted::Panicked | Formatted::Rejected => {
            tally.parse_skipped += 1;
            return;
        }
        Formatted::Ok { findings, .. } if !findings.is_empty() => {
            tally.dirty_files.push(display);
            return;
        }
        Formatted::Ok { .. } => {}
    }
    tally.files_done += 1;

    let regions = code_regions(&source, parser);
    let sites = injection_sites(&source, &regions, all_bytes);
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
                        offset,
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
                tally.record(Hit {
                    kind: match f.kind {
                        CommentFindingKind::Dropped => Kind::Dropped,
                        CommentFindingKind::DoublePrinted => Kind::DoublePrinted,
                    },
                    payload,
                    path: &display,
                    source: &source,
                    offset,
                    text: f.text,
                    // The injected comment starts exactly at the injection point; anything
                    // else is a bystander the injection knocked out.
                    injected: f.span.start as usize == offset,
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

        // Self-verify each shape's example against the output (cheap: once per shape, not
        // per site — ~700 formats against millions). Single-threaded and after the join, so
        // it can't interleave with a worker's thread-local ledger.
        let verdicts: Vec<((Kind, String), Verdict)> = total
            .shapes
            .iter()
            .map(|((kind, shape), agg)| {
                let parser = ParserType::from_extension(&agg.example.path);
                ((*kind, shape.clone()), verify_example(agg, *kind, parser))
            })
            .collect();
        for (key, verdict) in verdicts {
            if let Some(agg) = total.shapes.get_mut(&key) {
                agg.verdict = Some(verdict);
            }
        }

        std::panic::set_hook(prev_hook);
        comment_ledger::set_comment_check(false);

        if self.update {
            let path = known_shapes_path();
            let rendered = render_known(&total.shapes);
            if let Err(e) = std::fs::write(&path, &rendered) {
                eprintln!("Error: cannot write {}: {e}", path.display());
                return Err(CliError::Failed);
            }
            // Count what was actually written, not every shape — a panic is not pinned
            // (see `is_pinnable`), so reporting `total.shapes.len()` would overstate the
            // file by exactly the crashes it deliberately omits.
            println!(
                "✓ wrote {} shape(s) to {}",
                found_keys(&total.shapes).len(),
                path.display()
            );
            // Spend the verify pass rather than discarding it. Pinning is the moment ~700
            // claims get frozen, so it is exactly when it's worth saying which ones the
            // audit could not reproduce. A WARNING, not a refusal: an unconfirmed shape is
            // still a real finding, and the verdict describes the shape's one sampled
            // example rather than the shape, so refusing on it would both block `--update`
            // and flip with which fixture happens to sort first.
            let unconfirmed = count_unconfirmed(&total.shapes);
            if unconfirmed > 0 {
                println!(
                    "  ⚠ {unconfirmed} of them UNCONFIRMED — filed as dropped/double-printed, \
                     yet the output reparses to just as many comments as its input. Likely \
                     MANGLES (a rebuilt comment) rather than plain drops; see \
                     docs/gap_audit.md."
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
            Some(grade_against_snapshot(&total.shapes))
        } else {
            None
        };

        if self.json {
            print_json(&total, &payloads);
        } else if graded.as_ref().is_some_and(GateDiff::holds) && !self.report {
            // Nothing to act on: every shape is one the snapshot already pins, so the
            // per-shape report is thousands of lines of noise in `deno task check`.
            print_summary(&total, &payloads);
        } else {
            print_report(&total, &payloads);
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

    /// Report a [`GateDiff`] and turn it into an exit status. See [`KNOWN_SHAPES`] for why
    /// the key is the shape and not the count.
    fn report_gate(&self, diff: &GateDiff, total: &Tally) -> Result<(), CliError> {
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
                eprintln!(
                    "    {shape:<20} e.g. inject {} at {}:{}",
                    agg.example.payload, agg.example.path, agg.example.offset
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
            for (kind, shape, payloads) in new.iter().take(40) {
                eprintln!("    {kind:<14} {shape:<20} [{payloads}]");
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
            for (kind, shape, payloads) in stale.iter().take(40) {
                eprintln!("    {kind:<14} {shape:<20} [{payloads}]");
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

/// How many pinnable shapes carry [`Verdict::Unconfirmed`] — the ledger's account of them
/// couldn't be reproduced against the output.
fn count_unconfirmed(shapes: &BTreeMap<(Kind, String), ShapeAgg>) -> usize {
    shapes
        .iter()
        .filter(|((kind, _), agg)| is_pinnable(*kind) && agg.verdict == Some(Verdict::Unconfirmed))
        .count()
}

/// The header every report opens with — totals, then any file that was **skipped**.
///
/// The skip notice lives here, not in [`print_report`], because it is a statement about
/// COVERAGE, not a finding: a dirty file is one this audit never probed. Quiet modes may
/// drop findings the snapshot already pins; they must never drop the fact that a file went
/// unprobed, or a shrinking corpus reads as a passing gate.
fn print_header(t: &Tally, payloads: &[Payload]) {
    let labels: Vec<&str> = payloads.iter().map(|p| p.label()).collect();
    println!(
        "gap_audit — {} files · {} sites · {} injections ({} accepted) · payloads: {}\n",
        t.files_done,
        t.sites,
        t.injections,
        t.accepted,
        labels.join(", ")
    );

    if !t.dirty_files.is_empty() {
        println!(
            "○ {} file(s) already had ledger findings AS AUTHORED — reported by \
             `comments:audit`, not injected into here:",
            t.dirty_files.len()
        );
        for p in t.dirty_files.iter().take(10) {
            println!("    {p}");
        }
        if t.dirty_files.len() > 10 {
            println!("    … and {} more", t.dirty_files.len() - 10);
        }
        println!();
    }
}

/// What a run with nothing to act on prints: the header, the totals, and nothing else.
///
/// The per-shape report is for shapes you might *do* something about. When the ratchet
/// holds, every one is already pinned — printing all ~700 buries the `✓` under thousands of
/// lines in `deno task check`. `--report` brings them back.
fn print_summary(t: &Tally, payloads: &[Payload]) {
    print_header(t, payloads);
    let findings: usize = t.shapes.values().map(|s| s.count).sum();
    println!(
        "○ {findings} finding(s) across {} known site shape(s) — all pinned; re-run with \
         --report for the per-shape detail",
        t.shapes.len()
    );
}

fn print_report(t: &Tally, payloads: &[Payload]) {
    print_header(t, payloads);

    if t.shapes.is_empty() {
        println!(
            "✓ every injected comment printed exactly once — no gap drops a comment across \
             {} injections",
            t.accepted
        );
        return;
    }

    let total: usize = t.shapes.values().map(|s| s.count).sum();
    println!(
        "✗ {total} finding(s) across {} distinct site shape(s)\n",
        t.shapes.len()
    );

    // Worst-first: a shape firing everywhere is one bug on a hot path, and fixing it
    // collapses the whole list.
    let mut rows: Vec<(&(Kind, String), &ShapeAgg)> = t.shapes.iter().collect();
    rows.sort_by(|a, b| b.1.count.cmp(&a.1.count).then(a.0.cmp(b.0)));

    for ((kind, shape), agg) in &rows {
        let verdict = match agg.verdict {
            Some(Verdict::Confirmed) | None => String::new(),
            Some(Verdict::Unconfirmed) => "  ⚠ UNCONFIRMED".to_string(),
        };
        println!(
            "  {:>7}×  {:<14} {}{}",
            agg.count,
            kind.label(),
            shape,
            verdict
        );
        println!(
            "            {} file(s) · payloads: {}{}",
            agg.files.len(),
            agg.payloads.iter().copied().collect::<Vec<_>>().join(", "),
            match agg.bystander_hits {
                0 => String::new(),
                n if n == agg.count => "  (ALL hits knock out a BYSTANDER comment)".to_string(),
                n => format!("  ({n} of {} hits knock out a bystander)", agg.count),
            }
        );
        println!(
            "            e.g. inject {} at {}:{}  {}",
            agg.example.payload, agg.example.path, agg.example.offset, agg.example.snippet
        );
        println!("            comment: {:?}", agg.example.text);
        println!();
    }

    let unconfirmed = rows
        .iter()
        .filter(|(_, a)| a.verdict == Some(Verdict::Unconfirmed))
        .count();
    if unconfirmed > 0 {
        println!(
            "⚠ {unconfirmed} shape(s) UNCONFIRMED: the ledger says a comment was never emitted, \
             yet the\n  output reparses to just as many comments as its input. Something printed \
             it without\n  recording the emit — or printed a MANGLED rebuild (`/* a⏎b */` → \
             `/* ab */`, one\n  comment either way). Real either way, but not the plain drop it \
             is filed as.\n"
        );
    }
}

fn print_json(t: &Tally, payloads: &[Payload]) {
    let shapes: Vec<serde_json::Value> = t
        .shapes
        .iter()
        .map(|((kind, shape), agg)| {
            serde_json::json!({
                "kind": kind.label(),
                "shape": shape,
                "count": agg.count,
                "files": agg.files.len(),
                "payloads": agg.payloads.iter().copied().collect::<Vec<_>>(),
                "bystander_hits": agg.bystander_hits,
                "verdict": match agg.verdict {
                    Some(Verdict::Confirmed) => "confirmed",
                    Some(Verdict::Unconfirmed) => "unconfirmed",
                    None => "unverified",
                },
                "example_payload": agg.example.payload,
                "example_path": agg.example.path,
                "example_offset": agg.example.offset,
                "example_snippet": agg.example.snippet,
                "example_text": agg.example.text,
                "example_injected": agg.example.injected,
            })
        })
        .collect();
    let out = serde_json::json!({
        "files": t.files_done,
        "sites": t.sites,
        "injections": t.injections,
        "accepted": t.accepted,
        "parse_skipped": t.parse_skipped,
        "dirty_files": t.dirty_files,
        "payloads": payloads.iter().map(|p| p.label()).collect::<Vec<_>>(),
        "findings": t.shapes.values().map(|s| s.count).sum::<usize>(),
        "shapes": shapes,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wire→byte map, graded against a direct walk on every prefix of strings covering
    /// each width class: ASCII (1 byte / 1 unit), 2- and 3-byte BMP (n bytes / 1 unit), and
    /// astral (4 bytes / **2** units — the arm an "offset == char index" reading gets wrong).
    ///
    /// This is the only thing that can fail on a bad map: the corpus is ~all ASCII, where
    /// every arm is the identity, so a broken translation formats byte-identically.
    #[test]
    fn utf16_to_byte_matches_a_direct_walk() {
        for src in [
            "",
            "abc",
            "é",
            "aéb",
            "日本語",
            "a😀b",
            "😀😀",
            "const é = 1; // 日本\nx😀y",
        ] {
            let map = Utf16ToByte::new(src);

            // Every char boundary must round-trip: the char's UTF-16 offset maps back to
            // exactly its byte offset.
            let mut units = 0usize;
            for (byte, ch) in src.char_indices() {
                assert_eq!(
                    map.byte(units),
                    Some(byte),
                    "src {src:?}: utf16 offset {units} should be byte {byte}"
                );
                units += ch.len_utf16();
            }
            // The end offset maps to the source length, and one past it is out of range.
            assert_eq!(map.byte(units), Some(src.len()), "src {src:?}: end offset");
            assert_eq!(map.byte(units + 1), None, "src {src:?}: past the end");

            // Every produced offset is a char boundary — the property that keeps slicing
            // from panicking.
            for u in 0..=units {
                let b = map.byte(u).expect("in range");
                assert!(src.is_char_boundary(b), "src {src:?}: byte {b} mid-char");
            }
        }
    }

    /// The ASCII fast path must be indistinguishable from the table, not merely close.
    #[test]
    fn utf16_to_byte_ascii_fast_path_matches_the_table() {
        let src = "const a = 1;\n\tb();";
        let fast = Utf16ToByte::new(src);
        assert!(fast.table.is_none(), "ASCII source should skip the table");
        for u in 0..=src.len() + 1 {
            let table_answer = if u <= src.len() { Some(u) } else { None };
            assert_eq!(fast.byte(u), table_answer, "offset {u}");
        }
    }

    /// A region is only useful if it names a spot a comment can actually go: the `Program`
    /// span must start after `>` and end before `</script>`.
    #[test]
    fn code_regions_name_the_script_body_only() {
        let src = "<script lang=\"ts\">\n\tconst a = 1;\n</script>\n";
        let regions = code_regions(src, ParserType::Svelte);
        assert_eq!(regions.len(), 1, "one script body: {regions:?}");
        let (s, e) = regions[0];
        assert_eq!(&src[s..e], "\n\tconst a = 1;\n");
    }

    /// Every region as a source slice, for tests that care about *what* was named rather
    /// than where it sits.
    fn named(src: &str) -> Vec<&str> {
        code_regions(src, ParserType::Svelte)
            .into_iter()
            .map(|(s, e)| &src[s..e])
            .collect()
    }

    /// A block's head is the region the wire shape cannot express: `IfBlock`'s own span
    /// covers the whole block (body included) and its `test` span is the expression alone,
    /// so neither names `#if cond`. The head is where the `{#if ⟨here⟩ a.b}` class lives.
    #[test]
    fn code_regions_name_a_block_head_without_its_body() {
        // One region, and it stops at the head's `}` — the body is markup, not a comment
        // context, and `x` must not appear in it.
        assert_eq!(named("{#if a.b}x{/if}"), ["#if a.b"]);
        // `{:else if}` is a nested IfBlock and gets its own head.
        assert_eq!(named("{#if a}x{:else if b}y{/if}"), ["#if a", ":else if b"]);
    }

    /// Each head kind, plus the tags — one case per construct, because each carries its
    /// span on a different field and a typo in the walk would silently name nothing.
    #[test]
    fn code_regions_name_every_head_kind() {
        assert_eq!(named("{#each xs as x}{/each}"), ["#each xs as x"]);
        assert_eq!(named("{#await p}{/await}"), ["#await p"]);
        assert_eq!(named("{#key k}{/key}"), ["#key k"]);
        assert_eq!(named("{#snippet f(a)}{/snippet}"), ["#snippet f(a)"]);
        assert_eq!(named("{@html x}"), ["@html x"]);
        assert_eq!(named("{@render f()}"), ["@render f()"]);
        assert_eq!(named("{@const a = b}"), ["@const a = b"]);
        assert_eq!(named("{@debug a}"), ["@debug a"]);
    }

    /// A directive's value is named from `head_span.end` to the closing `}` — never the
    /// head itself (`on:click|once` is not a comment context) and never the byte *after*
    /// the `}`, which is the middle of an element tag: tsv over-accepts a comment there
    /// while Svelte rejects it, so naming it would manufacture the junk shapes the module
    /// docs warn about. A shorthand directive has no value and contributes nothing.
    #[test]
    fn code_regions_name_a_directive_value_not_its_head() {
        // The slice stops *before* the `}` because a range's end is exclusive — but a
        // region's end is an injection site (see `injection_sites`), so the closing `}` is
        // still probed. That inclusive end is what reaches `on:click={h/* c */}`; the byte
        // after it — inside the element tag — is what stays out.
        let src = "<div on:click={h}></div>";
        let regions = code_regions(src, ParserType::Svelte);
        assert_eq!(regions.len(), 1, "one directive value: {regions:?}");
        let (s, e) = regions[0];
        assert_eq!(&src[s..e], "={h", "the head `on:click` is not named");
        assert_eq!(
            &src[e..=e],
            "}",
            "the last site is the closing brace, not past it"
        );

        assert_eq!(named("<div on:click|once={h}></div>"), ["={h"]);
        assert!(
            named("<input bind:value />").is_empty(),
            "a shorthand directive has no value to probe"
        );
        // A plain attribute's value is an `ExpressionTag`, which the wire walk names —
        // the interior only, so the braces stay out.
        assert_eq!(named("<div class={c}></div>"), ["c"]);
        // `{...rest}` is brace-delimited like an ExpressionTag.
        assert_eq!(named("<div {...rest}></div>"), ["...rest"]);
    }

    /// `<svelte:element this={tag}>` holds its expression **bare** — Svelte's AST has no
    /// `ExpressionTag` around it — so the wire walk never names it and this is its only
    /// cover. Regression guard for a real drop: the comment survives in `{'a' + 'b'}` and
    /// vanished in `this={'a' + 'b'}`.
    #[test]
    fn code_regions_name_a_bare_special_element_expression() {
        assert_eq!(named("<svelte:element this={tag} />"), ["tag"]);
        assert_eq!(named("<svelte:component this={C} />"), ["C"]);
    }

    /// The walk names a head **whole**, including the slots where Svelte hand-reads a
    /// pattern and rejects a comment (`{#each xs as ⟨here⟩ x}`). That is deliberate: tsv
    /// rejects in exactly those slots too, so `Formatted::Rejected` filters them the same
    /// way it filters a word interior — no whitelist to keep in sync with Svelte's parser.
    #[test]
    fn a_head_region_covers_slots_the_parser_filters() {
        let src = "{#each xs as x}{/each}";
        let (s, e) = code_regions(src, ParserType::Svelte)[0];
        let as_slot = src.find(" x}").expect("the pattern slot") + 1;
        assert!(
            (s..=e).contains(&as_slot),
            "the `as` pattern slot is inside the named head"
        );
        // ...and tsv rejects a comment there, so no site survives to a finding.
        let injected = format!("{}/* c */{}", &src[..as_slot], &src[as_slot..]);
        assert!(
            matches!(
                ledger_format(&injected, ParserType::Svelte),
                Formatted::Rejected
            ),
            "tsv must reject a comment in Svelte's pattern slot, as Svelte does"
        );
    }

    /// The shape is the **ratchet key** — the thing the gate diffs against the snapshot —
    /// so what it abstracts and what it keeps is a contract, not a formatting choice.
    #[test]
    fn site_shape_keeps_keywords_and_abstracts_identifiers() {
        // A keyword must survive verbatim on both sides: `import⟨⟩.` names the
        // meta-property/import-phase header, while `IDENT⟨⟩.` names every member access in
        // the corpus. Collapsing the two would hide one bug inside the other's entry.
        assert_eq!(site_shape("import.meta", 6), "import⟨⟩.");
        assert_eq!(site_shape("import.meta", 7), ".⟨⟩meta");
        assert_eq!(site_shape("new.target", 3), "new⟨⟩.");

        // A non-keyword word abstracts, so one bug is one line however many identifiers
        // reach it.
        assert_eq!(site_shape("foo.bar", 3), "IDENT⟨⟩.");
        assert_eq!(site_shape("foo.bar", 4), ".⟨⟩IDENT");
        assert_eq!(site_shape("x9.y", 2), "IDENT⟨⟩.");

        // Digits are their own class — `1⟨⟩.` (a float's point) is not `IDENT⟨⟩.`.
        assert_eq!(site_shape("1.5", 1), "NUM⟨⟩.");

        // Whitespace is elided rather than represented: a gap's WIDTH doesn't distinguish
        // the position, so `a  =` and `a =` must land on one shape.
        assert_eq!(site_shape("a = 1", 2), "␣⟨⟩=");
        assert_eq!(site_shape("a  = 1", 2), "␣⟨⟩␣");

        // Punctuator runs are kept literally, capped, and read in source order.
        assert_eq!(site_shape("a);", 1), "IDENT⟨⟩);");
        assert_eq!(site_shape("f(x)", 2), "(⟨⟩IDENT");

        // The ends of a file are gaps too and must not panic.
        assert_eq!(site_shape("ab", 0), "␣⟨⟩IDENT");
        assert_eq!(site_shape("ab", 2), "IDENT⟨⟩␣");

        // Non-ASCII must not panic or slice mid-char.
        assert_eq!(site_shape("é.b", 2), "IDENT⟨⟩.");
    }

    /// A minimal shape carrying `payloads` — only the snapshot columns matter here, so the
    /// example is filler.
    fn mk_agg(payloads: &[&'static str]) -> ShapeAgg {
        ShapeAgg {
            count: 1,
            payloads: payloads.iter().copied().collect(),
            bystander_hits: 0,
            files: BTreeSet::new(),
            example: Example {
                payload: "block",
                path: "p.svelte".to_string(),
                offset: 0,
                snippet: String::new(),
                text: "/* c */".to_string(),
                injected: true,
            },
            verdict: None,
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

        let rendered = render_known(&shapes);
        // Every non-comment line must be a complete key — a dropped column would make the
        // gate silently compare fewer fields than it pins.
        let parsed: BTreeSet<KnownKey> = rendered
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| {
                let mut c = l.split('\t');
                (
                    c.next().unwrap().to_string(),
                    c.next().unwrap().to_string(),
                    c.next().unwrap().to_string(),
                )
            })
            .collect();

        assert_eq!(
            parsed,
            found_keys(&shapes),
            "render → parse must round-trip"
        );
        // The payload column is part of the key: same shape, different payload set ⇒
        // different entry, so a shape that starts dropping a new comment kind fails the gate.
        assert!(rendered.contains("DROPPED\timport⟨⟩.\tblock\n"));
        assert!(rendered.contains("DOUBLE-PRINTED\tIDENT⟨⟩=\tblock,line\n"));
    }

    /// A panic must never reach the snapshot — not via `--update`, and not as a key the
    /// gate diffs. The corpus cannot grade this: `tests/fixtures` panics nowhere today, so
    /// both arms are vacuously green there and would stay green if the filter were dropped.
    #[test]
    fn a_panic_is_never_pinned() {
        let mut shapes: BTreeMap<(Kind, String), ShapeAgg> = BTreeMap::new();
        shapes.insert((Kind::Dropped, "import⟨⟩.".to_string()), mk_agg(&["block"]));
        shapes.insert((Kind::Panic, "IDENT⟨⟩(".to_string()), mk_agg(&["block"]));

        // Not written: a crash must not land in the list whose shrinking is the goal.
        // Checked over the DATA lines, not the whole file — the header explains the panic
        // rule in prose, so a substring search over it matches that and proves nothing.
        let rendered = render_known(&shapes);
        let data: Vec<&str> = rendered
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        assert_eq!(
            data,
            vec!["DROPPED\timport⟨⟩.\tblock"],
            "only the pinnable shape is written"
        );

        // Nor diffed: were it a found key it would read as NEW, and `--update` would then
        // "fix" the gate by pinning it — the exact laundering this prevents.
        let keys = found_keys(&shapes);
        assert_eq!(keys.len(), 1, "only the pinnable shape is a key: {keys:?}");
        assert!(keys.iter().all(|(kind, _, _)| kind != "PANIC"));

        assert_eq!(
            count_panics(&shapes),
            1,
            "but it is still counted, and fails"
        );
    }

    /// The ledger's falsifiable prediction. Arithmetic, so the corpus cannot grade it.
    #[test]
    fn predicted_comment_count_accounts_for_each_finding() {
        let f = |kind, emitted| CommentFinding {
            kind,
            text: "/* c */".to_string(),
            span: tsv_lang::Span { start: 0, end: 7 },
            emitted,
        };
        // No findings ⇒ the output keeps every comment.
        assert_eq!(predict_comment_count(5, &[]), 5);
        // A drop removes exactly one.
        assert_eq!(
            predict_comment_count(5, &[f(CommentFindingKind::Dropped, 0)]),
            4
        );
        // A comment printed TWICE adds one — but one printed THREE times adds two. This is
        // the case a "double means 2" reading gets wrong.
        assert_eq!(
            predict_comment_count(5, &[f(CommentFindingKind::DoublePrinted, 2)]),
            6
        );
        assert_eq!(
            predict_comment_count(5, &[f(CommentFindingKind::DoublePrinted, 3)]),
            7
        );
        // Mixed findings compose.
        assert_eq!(
            predict_comment_count(
                5,
                &[
                    f(CommentFindingKind::Dropped, 0),
                    f(CommentFindingKind::DoublePrinted, 2)
                ]
            ),
            5
        );
        // A ledger that broke its own invariant must not panic the audit.
        assert_eq!(
            predict_comment_count(
                0,
                &[
                    f(CommentFindingKind::Dropped, 0),
                    f(CommentFindingKind::Dropped, 0)
                ]
            ),
            0
        );
    }

    /// A full default run, as a baseline for the narrowing cases below.
    fn full_run() -> GapAuditCommand {
        GapAuditCommand {
            json: false,
            report: false,
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

        // `--json` / `--report` / `--jobs` change how a run is REPORTED and scheduled, never
        // which sites it reaches, so they must not disqualify one — a gate you can't run
        // under --json, or on a fixed thread count, would just get bypassed.
        let reporting_only = GapAuditCommand {
            json: true,
            report: true,
            jobs: Some(1),
            ..full_run()
        };
        assert!(
            reporting_only.narrowing_flags().is_empty(),
            "--json / --report / --jobs don't change the shape set"
        );

        // They compose, so the error message can name every offender at once.
        let both = GapAuditCommand {
            limit: 30,
            payload: Some("block".to_string()),
            ..full_run()
        };
        assert_eq!(both.narrowing_flags(), vec!["--limit", "--payload"]);
    }

    /// The word filter must keep both dot gaps of a punctuator-joined header — the exact
    /// positions the whole audit exists to probe — while dropping word interiors.
    #[test]
    fn injection_sites_keep_dot_gaps_and_drop_word_interiors() {
        let src = "a.b";
        let sites = injection_sites(src, &[(0, src.len())], false);
        assert_eq!(sites, vec![0, 1, 2, 3], "every gap around `.` is a site");

        let src = "ab";
        let sites = injection_sites(src, &[(0, src.len())], false);
        assert_eq!(sites, vec![0, 2], "the interior of `ab` is not a gap");
        let all = injection_sites(src, &[(0, src.len())], true);
        assert_eq!(all, vec![0, 1, 2], "--all-bytes keeps the interior");
    }
}
