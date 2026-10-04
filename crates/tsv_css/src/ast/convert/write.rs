//! Writer-mode conversion: emit compact wire JSON directly from the internal
//! CSS AST.
//!
//! The CSS sibling of `tsv_ts`'s `ast/convert/write/` — the **sole emission
//! path** for the CSS wire JSON. It walks the *internal* AST once and writes the
//! final JSON bytes as it goes, never materializing a typed public tree. It
//! writes both wires — the span-only one behind `convert_ast_json_bytes`
//! (every parse binding, the CLI's default output) and the loc-bearing one behind
//! `convert_ast_json_bytes_with_locations` (`tsv parse --locations`) — and is the
//! entry the Svelte writer composes for embedded `<style>` blocks.
//!
//! **Byte-identity**: the wire JSON is a faithful emission of the `parseCss()`
//! quirk catalog — node field order (including the `AttributeSelector`
//! `start`/`end`-before-`name` and `Rule`
//! `prelude`/`block`-before-`start`/`end` quirks), the skip rules (`metadata` on
//! standalone CSS only; `namespace`/`Nth.selector` skipped when absent), the
//! `null`s for absent-but-present `Option`s (`combinator`, `matcher`/`value`/
//! `flags`, `PseudoClass.args`), and scalar formatting all match `parseCss`'s
//! JSON exactly — the shape the canonical `parseCss` `expected.json` records.
//! The writer **reuses the raw-source reconstruction helpers** in the sibling
//! `mod.rs` (`strip_css_comments_collecting`, `split_declaration_svelte_compat`,
//! `raw_selector_name`, …) so the Svelte scan semantics are defined once.
//!
//! `parseCss` emits only `start`/`end`; tsv adds `loc` to every object that carries
//! them, on the wire that asks for it — the one `loc` definition all three writers
//! share (`tsv_lang::WirePositions`), in the coordinates `crate::WIRE_COORDINATES`
//! states. Each position is translated via the byte→UTF-16 map (identity on ASCII). Dynamic
//! strings are escaped by
//! [`write_string`] (byte-identical to `serde_json`, and to `JsonWriter::string`:
//! one shared copy of `JsonWriter::string_led_words` with an empty lead, whose short clean
//! path is a single window append, which pays on this writer's many short strings); static
//! structure/tokens are written verbatim; integers are hand-formatted.
//!
//! Node-header prefixes are single pre-fused literals per site, deliberately NOT
//! extracted into a shared `open_node` helper: the helper — even `#[inline]`
//! taking the pre-fused prefix — shifted fat-LTO inlining across the crate
//! (`write_block`/`write_atrule` de-inlined) and measured +0.45% instructions on
//! the CSS parse-JSON path. CSS nodes are small enough that per-node call
//! structure is visible; keep the literals at the site, whether a site hands its
//! literal to `w.raw` or to the head window below (which is `inline(always)` and
//! const-generic over the literal's length, so it is the site's own code).
//!
//! # The `start`/`end` bursts
//!
//! Every node here ends with the same burst — `,"start":` N `,"end":` M, then a
//! `}` or a constant `metadata` payload — and the nodes whose positions lead open
//! with its twin (`{"type":"Block","start":` N `,"end":` M `,"children":[`). Each
//! is written into the output buffer directly, in one of three shapes:
//!
//! - The emitters that carry ~37% of the corpus's nodes — the rule
//!   (`write_rule`), the relative selector (`write_relative_selector`), and the
//!   named selectors (`write_named_selector`, and the namespaced arm of
//!   `write_type_selector`, which writes the same tail itself) — close through
//!   [`JsonWriter::start_end_tail`]: the lead, the key, both integers and the
//!   closing byte stored into one fixed-width window of the buffer, inline at the
//!   site.
//! - The four heads that open most nodes — `Block`, `Declaration`, `SelectorList`
//!   and `ComplexSelector` — open through [`JsonWriter::start_end_head`], the
//!   tail window's twin: the opening literal, both integers, and the constant that
//!   follows them (`,"children":[`, so the array's opener costs no append of its
//!   own) in one window, inline at the site. It has to be inline to pay: the same
//!   burst behind a call is the call below with more arguments.
//! - Every other pair is one call. The remaining head bursts (`StyleSheetFile`, `Atrule`,
//!   `AttributeSelector`, the synthesized selector list), and the synthesized relative selector's
//!   tail (whose `],"start":` is a literal of its own), take [`JsonWriter::start_end`] after a
//!   literal ending in `"start":`; the remaining tails (combinator, the pseudo selectors, `Nth`,
//!   `Percentage`, `CSSComment`) take [`JsonWriter::start_end_field`]. Each writes both integers
//!   and the key between them into such a window. Those bodies are deliberately `inline(never)` for
//!   WASM size, so the call forces the output buffer's pointer/length/capacity out of registers
//!   around it — the price the hot tails and heads do not pay.
//!
//! ⚠️ **None of these bursts is a staged run ([`JsonWriter::stage_run`]), and the two ends are
//! refused for different reasons.** A staged run copies its static fragments twice — once into the
//! scratch, once through the flush — so the trade is *appends removed* against *static bytes in the
//! run*. A *head* run's static fragments are ~50 bytes against a tail's ~17, and staging the heads
//! is a loss: it removes more instructions than staging the tails does and buys no cycles for them.
//! That verdict is about the double copy, not about bursting a head: the head window stores its
//! literal once, at a constant width, into the output buffer, which is the half of the trade a
//! staged run cannot have. The *tails* do pay as staged runs against the out-of-line pair (a staged
//! run inlines its integer emission, the call does not), and the window keeps exactly that — the
//! integers inline, nothing spilled around a call — while writing the digits and the closing
//! constant straight into the output buffer: no scratch round trip and no runtime-length flush, so
//! fewer instructions than the staged form. **Grade any change to this on `cycles`/wall as well as
//! instructions** — the two channels have ranked this file's scopes in opposite orders.
//!
//! The windows are the span-only wire's shape. On the wire that carries `loc`, which sits
//! between `end` and the closing constant, [`Ctx::head`] / [`Ctx::tail`] write the same burst
//! piecewise with the `loc` field in place.

use super::super::internal;
use super::{
    WireComment, convert_prelude_to_string, raw_selector_name, selector_contains_invalid,
    split_declaration_svelte_compat, strip_css_comments_collecting, trim_wire_end, trim_wire_start,
};
use std::borrow::Cow;
use tsv_lang::{JsonWriter, Span, Wire, WirePositions, WireTables, write_array, write_or_null};

/// Declares one `parseCss()` metadata payload twice from a single literal: bare
/// (`$bare`), and as the constant burst that closes its node (`$closing` — `$lead`,
/// the payload, the node's `}`), so a walked node ends in one append.
macro_rules! metadata_payload {
    ($bare:ident, $closing:ident = $lead:literal, $payload:literal) => {
        const $bare: &str = $payload;
        const $closing: &str = concat!($lead, $payload, "}");
    };
}

// `parseCss()` constant metadata payloads — always the `Default` (all-`false`,
// `null` unit) shapes, emitted only on standalone CSS (`Ctx::has_metadata`).
// The `,"metadata":…` prefix folds the leading comma into the constant.

/// A `Rule`'s metadata and its closing `}`.
const RULE_META_CLOSE: &str = ",\"metadata\":{\"parent_rule\":null,\"has_local_selectors\":false,\"has_global_selectors\":false,\"is_global_block\":false}}";
// `COMPLEX_META_CLOSE` leads with the `]` that ends the selector's `children`.
metadata_payload!(
    COMPLEX_META,
    COMPLEX_META_CLOSE = "]",
    ",\"metadata\":{\"rule\":null,\"is_global\":false,\"used\":false}"
);
metadata_payload!(
    RELATIVE_META,
    RELATIVE_META_CLOSE = "",
    ",\"metadata\":{\"is_global\":false,\"is_global_like\":false,\"scoped\":false}"
);

/// An array's opening and items: `open` — a literal ending in the array's `[`, or empty where the
/// node's head burst already wrote it — then the comma-separated items. The caller writes the `]`,
/// as the first byte of whatever constant follows the array, so a node's array costs it no append
/// of its own on either side (`tsv_lang::write_array` spends one on each bracket).
#[inline]
fn write_array_open<T, const N: usize>(
    w: &mut JsonWriter,
    open: &[u8; N],
    items: impl IntoIterator<Item = T>,
    mut f: impl FnMut(&mut JsonWriter, T),
) {
    w.raw_fixed(open);
    let mut first = true;
    for item in items {
        if !first {
            w.raw(",");
        }
        first = false;
        f(w, item);
    }
}

/// The per-document environment every writer function shares.
#[derive(Clone, Copy)]
struct Ctx<'a> {
    source: &'a str,
    positions: WirePositions<'a>,
    /// Whether to attach `parseCss()` `metadata` (standalone `.css`) or omit it
    /// (embedded `<style>`). Precomputed once — the two shapes are otherwise the
    /// same walk — so the per-node metadata sites are a bare bool test.
    has_metadata: bool,
}

impl Ctx<'_> {
    /// Byte offset → emitted (UTF-16 code unit) offset; identity on ASCII.
    #[inline]
    fn pos(&self, byte: u32) -> u32 {
        self.positions.pos(byte)
    }

    /// A node's closing burst (`lead`, `"start":` N `,"end":` M, `close`) as
    /// [`JsonWriter::start_end_tail`]'s one window — or, on a wire that carries `loc`,
    /// the same bytes with `loc` after `end`, written piecewise.
    #[expect(clippy::inline_always)]
    #[inline(always)]
    fn tail<const L: usize, const C: usize>(
        &self,
        w: &mut JsonWriter,
        lead: &[u8; L],
        start: u32,
        end: u32,
        close: &[u8; C],
    ) {
        if self.positions.has_locations() {
            w.raw_fixed(lead);
            w.raw("\"start\":");
            w.span_start_end(self.positions, start, end);
            w.raw_fixed(close);
        } else {
            w.start_end_tail(lead, self.pos(start), self.pos(end), close);
        }
    }

    /// A node's opening burst (`lead` ending in `"start":`, N `,"end":` M, `close`) as
    /// [`JsonWriter::start_end_head`]'s one window — or, on a wire that carries `loc`,
    /// the same bytes with `loc` after `end`, written piecewise.
    #[expect(clippy::inline_always)]
    #[inline(always)]
    fn head<const L: usize, const C: usize>(
        &self,
        w: &mut JsonWriter,
        lead: &[u8; L],
        start: u32,
        end: u32,
        close: &[u8; C],
    ) {
        if self.positions.has_locations() {
            w.raw_fixed(lead);
            w.span_start_end(self.positions, start, end);
            w.raw_fixed(close);
        } else {
            w.start_end_head(lead, self.pos(start), self.pos(end), close);
        }
    }
}

/// Convert the internal CSS nodes straight to standalone-`StyleSheetFile` wire
/// bytes on `wire` — one AST walk, with byte→char offset translation fused in, in
/// the coordinates `crate::WIRE_COORDINATES` states. (An embedded `<style>` takes the
/// Svelte writer's positions.)
pub(crate) fn write_stylesheet_file_bytes(
    stylesheet: &internal::CssStyleSheet<'_>,
    source: &str,
    wire: Wire,
) -> Vec<u8> {
    let tables = WireTables::new(source, crate::WIRE_COORDINATES, wire);
    let ctx = Ctx {
        source,
        positions: tables.positions(),
        has_metadata: true,
    };
    let mut w = JsonWriter::with_capacity(tsv_lang::estimated_json_capacity(source.len(), wire));
    write_stylesheet_file(&mut w, stylesheet, &ctx);
    w.into_bytes()
}

/// Emit an embedded-`<style>` stylesheet's `children` array (no `metadata`) into
/// a caller-owned writer, handing back the `comments` run the same walk gathered
/// — the composition entry the Svelte writer uses for a `<style>` element, whose
/// wire puts those two arrays side by side. `positions` must be the host
/// document's (spans are in host-file coordinates, and the line table is the
/// Svelte document's).
pub fn write_css_children(
    w: &mut JsonWriter,
    stylesheet: &internal::CssStyleSheet<'_>,
    source: &str,
    positions: WirePositions<'_>,
) -> CssComments {
    let ctx = Ctx {
        source,
        positions,
        has_metadata: false,
    };
    write_children(w, stylesheet, &ctx)
}

/// Emit the `comments` array `write_css_children` collected. Split from it
/// because the wire writes `children` first and the two are separate keys.
pub fn write_css_comments(
    w: &mut JsonWriter,
    comments: &CssComments,
    source: &str,
    positions: WirePositions<'_>,
) {
    let ctx = Ctx {
        source,
        positions,
        has_metadata: false,
    };
    write_comments(w, comments, &ctx);
}

/// A dynamic string value, escaped byte-identical to `serde_json`: one shared out-of-line
/// copy of [`JsonWriter::string_led_words`] with an empty lead, so a clean string shorter
/// than a word is written as one window append with no call, and one of up to two words by
/// a tail call that saves no register.
#[inline(never)]
fn write_string(w: &mut JsonWriter, s: &str) {
    w.string_led_words(&[], s.as_bytes());
}

/// [`write_string`] behind its field's key `lead` (`,"property":`), written as part of the
/// string's own window append rather than as an append of its own — one out-of-line copy per
/// key, for the two fields every declaration carries.
#[inline(never)]
fn write_keyed_string<const K: usize>(w: &mut JsonWriter, lead: &[u8; K], s: &str) {
    w.string_led_words(lead, s.as_bytes());
}

/// The standalone `StyleSheetFile` root: `type`, `start` (0), `end` (source
/// length), `children`, `comments`.
fn write_stylesheet_file(
    w: &mut JsonWriter,
    stylesheet: &internal::CssStyleSheet<'_>,
    ctx: &Ctx<'_>,
) {
    w.raw("{\"type\":\"StyleSheetFile\",\"start\":");
    w.span_start_end(ctx.positions, 0, ctx.source.len() as u32);
    w.raw(",\"children\":");
    let comments = write_children(w, stylesheet, ctx);
    w.raw(",\"comments\":");
    write_comments(w, &comments, ctx);
    w.raw("}");
}

/// Every comment Svelte's CSS parser captures on a stylesheet root, in source
/// order — gathered by the `children` walk, opaque to its caller, spent by
/// [`write_css_comments`].
///
/// `#[must_use]` because it is the *only* way to reach those comments: the walk
/// that gathers them has already written its children, so dropping the value
/// silently omits the sibling array rather than failing to compile.
#[must_use]
pub struct CssComments(Vec<WireComment>);

/// Emit the stylesheet's `children` array, gathering the wire `CSSComment` run
/// as the walk goes.
///
/// tsv keeps those comments in three disjoint places, because each serves a
/// different printer need: the detached `CssStyleSheet.comments` (top level,
/// selector gaps, structured at-rule preludes), the in-block
/// `CssBlockChild::Comment` children, and — for a declaration value or an at-rule
/// prelude — nowhere at all, since those are never lexed as `Comment`s and are
/// re-derived from source. Svelte has one flat list, so this walk rebuilds it.
///
/// It rides the emission walk rather than running its own, so a declaration's
/// reading comes from the **same** `strip_css_comments_collecting` call its
/// emitted `value` comes from — a second scan would drift the recorded offsets
/// from the string they index — and a comment-free stylesheet pays nothing for
/// the question.
///
/// A prelude comment arrives twice — once registered by the parser, once
/// stripped out of the prelude text — so the merge is a dedupe on `start` that
/// keeps the **positioned** reading: `position` is the field only the strip can
/// supply, and Svelte captures such a comment through `read_value`, which always
/// sets it.
fn write_children(
    w: &mut JsonWriter,
    stylesheet: &internal::CssStyleSheet<'_>,
    ctx: &Ctx<'_>,
) -> CssComments {
    let mut out = Vec::new();
    write_array(w, stylesheet.nodes, |w, n| write_node(w, n, ctx, &mut out));
    out.extend(stylesheet.comments.iter().map(|c| WireComment {
        span: c.span,
        position: None,
    }));
    out.sort_unstable_by_key(|c| (c.span.start, c.position.is_none()));
    out.dedup_by_key(|c| c.span.start);
    CssComments(out)
}

fn write_comments(w: &mut JsonWriter, comments: &CssComments, ctx: &Ctx<'_>) {
    write_array(w, &comments.0, |w, c| {
        // `value` is the comment's interior — Svelte's `read_comment` reads it
        // between the delimiters it has already eaten.
        let interior = Span::new(c.span.start + 2, c.span.end - 2);
        w.raw("{\"type\":\"CSSComment\",\"value\":");
        write_string(w, interior.extract(ctx.source));
        w.span_start_end_field(ctx.positions, c.span.start, c.span.end);
        if let Some(position) = c.position {
            w.raw(",\"position\":");
            w.u32(position);
        }
        w.raw("}");
    });
}

/// Emit a CSS node (a `Rule` or an `Atrule`).
fn write_node(
    w: &mut JsonWriter,
    node: &internal::CssNode<'_>,
    ctx: &Ctx<'_>,
    comments: &mut Vec<WireComment>,
) {
    match node {
        internal::CssNode::Rule(rule) => write_rule(w, rule, ctx, comments),
        internal::CssNode::Atrule(atrule) => write_atrule(w, atrule, ctx, comments),
    }
}

/// Emits a `Rule` node. Field order: `type`, `prelude`, `block`, `start`,
/// `end`, then `metadata` (standalone only).
///
/// The trailing `start`/`end` burst is one in-place window (module doc, §The
/// `start`/`end` bursts); the closing constant — `metadata` and the `}`, or the `}`
/// alone — is an append of its own, since which one it is varies by document.
fn write_rule(
    w: &mut JsonWriter,
    rule: &internal::CssRule<'_>,
    ctx: &Ctx<'_>,
    comments: &mut Vec<WireComment>,
) {
    w.raw("{\"type\":\"Rule\",\"prelude\":");
    write_selector_list(w, &rule.selector, ctx);
    w.raw(",\"block\":");
    write_block(w, rule.block_span, rule.declarations, ctx, comments);
    ctx.tail(w, b",", rule.span.start, rule.span.end, b"");
    if ctx.has_metadata {
        w.raw(RULE_META_CLOSE);
    } else {
        w.raw("}");
    }
}

/// Emits an `Atrule` node. Field order: `type`, `start`, `end`, `name`,
/// `prelude`, `block` (unlike `Rule`, whose positions trail — parseCss
/// constructs the two literals differently). `Atrule` carries no `metadata`.
fn write_atrule(
    w: &mut JsonWriter,
    atrule: &internal::CssAtrule<'_>,
    ctx: &Ctx<'_>,
    comments: &mut Vec<WireComment>,
) {
    w.raw("{\"type\":\"Atrule\",\"start\":");
    w.span_start_end(ctx.positions, atrule.span.start, atrule.span.end);
    w.raw(",\"name\":");
    // Half-decoded from source, like a selector name: parseCss reads both with
    // `read_identifier`, so an identity escape keeps its backslash (`@a\?b` → `a\?b`)
    // where the internal `name` is fully decoded.
    write_string(w, &raw_selector_name(ctx.source, atrule.name_span, 0));
    w.raw(",\"prelude\":");
    let prelude = convert_prelude_to_string(&atrule.prelude, ctx.source);
    write_string(w, &prelude);
    collect_prelude_comments(atrule, ctx.source, comments);
    w.raw(",\"block\":");
    write_or_null(w, atrule.block.as_ref(), |w, b| {
        write_block(w, b.span, b.children, ctx, comments);
    });
    w.raw("}");
}

/// Record the comments Svelte lifts out of an at-rule prelude, which the emitted
/// `prelude` string above cannot supply: it strips `prelude.span()`, and Svelte
/// reads the prelude with `read_value` from right after the name to the `{` / `;`
/// terminator — a WIDER region. The difference is whitespace and comments only,
/// so both strip to the same string, but scanning the narrow one would miss a
/// trailing `@import url(x) /* c */;` comment and measure the leading one's
/// `position` from the wrong origin.
///
/// Both ends come from the parser rather than from a byte scan, because the
/// region can hold strings and `url()`s: Svelte's `read_value` is quote- and
/// url-aware, so a `;`/`}`/`{` inside one is not a terminator to it either. A
/// block at-rule ends at its own `{`; a blockless one ends at the `;` the parser
/// required, one byte before `span.end`.
///
// TODO: the prelude is derived twice — this wide region and the narrow
// `prelude.span()` `convert_prelude_to_string` strips for the emitted string. They
// are claimed to differ only by whitespace and comments, so the narrow one could go
// and this region could supply both. That moves how `prelude` itself is derived,
// which the CSS fixtures and the parse-corpus gate stand on, so it wants its own
// change and its own verdict on that claim.
fn collect_prelude_comments(
    atrule: &internal::CssAtrule<'_>,
    source: &str,
    comments: &mut Vec<WireComment>,
) {
    let start = atrule.name_span.end;
    let end = atrule
        .block
        .as_ref()
        .map_or(atrule.span.end - 1, |b| b.span.start);
    let region = Span::new(start, end.max(start));
    strip_css_comments_collecting(region.extract(source), region.start, comments);
}

/// Emits a `Block` node. A `Comment` child produces no output — it is collected
/// for the stylesheet's `comments` array and filtered out of `children`.
fn write_block(
    w: &mut JsonWriter,
    block_span: Span,
    children: &[internal::CssBlockChild<'_>],
    ctx: &Ctx<'_>,
    comments: &mut Vec<WireComment>,
) {
    comments.extend(children.iter().filter_map(|c| match c {
        internal::CssBlockChild::Comment(c) => Some(WireComment {
            span: c.span,
            position: None,
        }),
        _ => None,
    }));
    ctx.head(
        w,
        b"{\"type\":\"Block\",\"start\":",
        block_span.start,
        block_span.end,
        b",\"children\":[",
    );
    write_array_open(
        w,
        b"",
        children
            .iter()
            .filter(|c| !matches!(c, internal::CssBlockChild::Comment(_))),
        |w, c| write_block_child(w, c, ctx, comments),
    );
    w.raw("]}");
}

fn write_block_child(
    w: &mut JsonWriter,
    child: &internal::CssBlockChild<'_>,
    ctx: &Ctx<'_>,
    comments: &mut Vec<WireComment>,
) {
    match child {
        internal::CssBlockChild::Declaration(d) => write_declaration(w, d, ctx, comments),
        internal::CssBlockChild::Rule(r) => write_rule(w, r, ctx, comments),
        internal::CssBlockChild::Atrule(a) => write_atrule(w, a, ctx, comments),
        // Comments are filtered out before this call (see `write_block`).
        internal::CssBlockChild::Comment(_) => {}
    }
}

/// Svelte's `read_declaration` view of one declaration, in raw source: the
/// terminator it stops at and the property/value halves it splits into.
///
/// The single derivation behind both readings of a declaration — the emitted
/// `Declaration` node and the comments lifted out of its value. Sharing it is
/// what keeps a recorded `position` indexing the very string it was measured
/// against; a second copy of these four lines is a copy that can drift.
struct DeclarationSplit<'a> {
    /// Byte offset of the `;`/`}` that ends the declaration — the wire `end`.
    end: u32,
    /// Pre-colon text; the wire `property` is this `trim_end`ed.
    property: &'a str,
    /// Post-colon text, a `trim_start`ed **suffix** of the declaration's extent;
    /// the wire `value` is this with block comments stripped.
    value: &'a str,
    /// `value`'s own byte offset in the document, so a comment span recorded
    /// inside it comes back in document coordinates.
    value_start: u32,
}

fn split_declaration<'a>(
    decl: &internal::CssDeclaration<'_>,
    source: &'a str,
) -> DeclarationSplit<'a> {
    let content_end = decl
        .important_end
        .map_or(decl.span.end, |e| e.max(decl.span.end));
    let end = crate::comments::scan_to_terminator(source, content_end as usize);
    let decl_source = &source[decl.span.start as usize..end];
    let colon = decl.colon_pos();
    let (property, value) = if decl.has_block_comment {
        // Rare: a block comment sits somewhere in the declaration — apply the
        // Svelte property/value split quirk.
        split_declaration_svelte_compat(decl_source, colon)
    } else {
        // Common: no comments anywhere → split at the recorded colon. No re-scans.
        (
            &decl_source[..colon],
            trim_wire_start(&decl_source[colon + 1..]),
        )
    };
    DeclarationSplit {
        end: end as u32,
        property,
        // Both arms leave `value` a suffix of `decl_source` (each slices from an
        // offset and only ever trims the front), so the length difference is it.
        value_start: decl.span.start + (decl_source.len() - value.len()) as u32,
        value,
    }
}

/// Emits a `Declaration` node: `end` is the `;`/`}` terminator, `property`
/// the trimmed pre-colon text, `value` the post-colon source with block
/// comments stripped.
///
/// That strip is also the only place a declaration's comments exist — they are
/// never lexed as `Comment`s — so it doubles as their collector, which is what
/// keeps each recorded `position` indexing the very `value` emitted here.
fn write_declaration(
    w: &mut JsonWriter,
    decl: &internal::CssDeclaration<'_>,
    ctx: &Ctx<'_>,
    comments: &mut Vec<WireComment>,
) {
    let split = split_declaration(decl, ctx.source);
    let value: Cow<'_, str> = if decl.has_block_comment {
        strip_css_comments_collecting(split.value, split.value_start, comments)
    } else {
        // Nothing to strip, so the strip reduces to the trim it ends with — and
        // the front is already trimmed.
        Cow::Borrowed(trim_wire_end(split.value))
    };

    ctx.head(
        w,
        b"{\"type\":\"Declaration\",\"start\":",
        decl.span.start,
        split.end,
        b"",
    );
    write_keyed_string(w, b",\"property\":", trim_wire_end(split.property));
    write_keyed_string(w, b",\"value\":", &value);
    w.raw("}");
}

/// Emits a `SelectorList` node (rule preludes — parsed non-forgivingly, no
/// `Invalid`).
fn write_selector_list(w: &mut JsonWriter, sl: &internal::SelectorList<'_>, ctx: &Ctx<'_>) {
    write_selector_list_inner(w, sl, ctx, false);
}

/// Emits a `SelectorList` node for pseudo-class args — drops complex selectors
/// containing a forgiving-parse `Invalid`.
fn write_selector_list_filtered(
    w: &mut JsonWriter,
    sl: &internal::SelectorList<'_>,
    ctx: &Ctx<'_>,
) {
    write_selector_list_inner(w, sl, ctx, true);
}

fn write_selector_list_inner(
    w: &mut JsonWriter,
    sl: &internal::SelectorList<'_>,
    ctx: &Ctx<'_>,
    filter_invalid: bool,
) {
    ctx.head(
        w,
        b"{\"type\":\"SelectorList\",\"start\":",
        sl.span.start,
        sl.span.end,
        b",\"children\":[",
    );
    write_array_open(
        w,
        b"",
        sl.selectors
            .iter()
            .filter(|c| !filter_invalid || !selector_contains_invalid(c)),
        |w, c| write_complex_selector(w, c, ctx),
    );
    w.raw("]}");
}

/// Emits a `ComplexSelector` node.
fn write_complex_selector(w: &mut JsonWriter, c: &internal::ComplexSelector<'_>, ctx: &Ctx<'_>) {
    ctx.head(
        w,
        b"{\"type\":\"ComplexSelector\",\"start\":",
        c.span.start,
        c.span.end,
        b",\"children\":[",
    );
    write_array_open(w, b"", c.children, |w, r| {
        write_relative_selector(w, r, ctx);
    });
    if ctx.has_metadata {
        w.raw(COMPLEX_META_CLOSE);
    } else {
        w.raw("]}");
    }
}

/// Emits a `RelativeSelector` node. `combinator` is `null` (no skip) when
/// absent; field order is `combinator`, `selectors`, `start`, `end`, `metadata`.
///
/// The trailing `start`/`end` burst is one in-place window, led by the `]` that
/// closes `selectors` and stopping before `metadata` for `write_rule`'s reason (module
/// doc, §The `start`/`end` bursts).
fn write_relative_selector(w: &mut JsonWriter, r: &internal::RelativeSelector<'_>, ctx: &Ctx<'_>) {
    // The combinator-less head — the first compound of every selector — is one literal
    // through the `selectors` array's `[`.
    match (&r.combinator, &r.combinator_span) {
        (Some(comb), Some(span)) => {
            w.raw("{\"type\":\"RelativeSelector\",\"combinator\":");
            write_combinator(w, comb.as_str(), *span, ctx);
            w.raw(",\"selectors\":[");
        }
        _ => w.raw("{\"type\":\"RelativeSelector\",\"combinator\":null,\"selectors\":["),
    }
    write_array_open(w, b"", r.selectors, |w, s| write_simple_selector(w, s, ctx));
    ctx.tail(w, b"],", r.span.start, r.span.end, b"");
    if ctx.has_metadata {
        w.raw(RELATIVE_META_CLOSE);
    } else {
        w.raw("}");
    }
}

fn write_combinator(w: &mut JsonWriter, name: &'static str, span: Span, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"Combinator\",\"name\":");
    w.token(name); // ` ` / `>` / `+` / `~` / `||` — escape-free
    w.span_start_end_field(ctx.positions, span.start, span.end);
    w.raw("}");
}

/// Emit a simple selector (type/universal/class/id/nesting/attribute/pseudo/percentage).
fn write_simple_selector(w: &mut JsonWriter, simple: &internal::SimpleSelector<'_>, ctx: &Ctx<'_>) {
    match simple {
        internal::SimpleSelector::Type {
            namespace_span,
            span,
        } => {
            let name = match namespace_span {
                None => raw_selector_name(ctx.source, *span, 0),
                // Step forward from the prefix token rather than scanning for a `|`: the
                // separator is the next non-trivia byte after the prefix, and the element
                // name the next one after that. Both junctures may hold a comment
                // (`svg/* c */|rect`, `svg|/* c */rect`), and the prefix itself may hold an
                // escaped `|` (`a\|b|rect`) — a scan for the first `|` reads one as the
                // separator and the rest as the name.
                Some(prefix) => {
                    let name_start = wq_name_start(ctx.source, *prefix, span.end);
                    raw_selector_name(ctx.source, *span, name_start - span.start as usize)
                }
            };
            write_type_selector(w, &name, *namespace_span, *span, ctx);
        }
        internal::SimpleSelector::Universal {
            namespace_span,
            span,
        } => {
            write_type_selector(w, "*", *namespace_span, *span, ctx);
        }
        internal::SimpleSelector::Class { span } => {
            // Past the `.` AND any comment glued to it (`./* c */cls`) — never a bare `1`.
            let name_start = crate::comments::class_name_start(ctx.source.as_bytes(), span.start);
            let name = raw_selector_name(ctx.source, *span, (name_start - span.start) as usize);
            w.raw("{\"type\":\"ClassSelector\",\"name\":");
            write_named_selector(w, &name, *span, ctx);
        }
        internal::SimpleSelector::Id { span } => {
            let name = raw_selector_name(ctx.source, *span, 1);
            w.raw("{\"type\":\"IdSelector\",\"name\":");
            write_named_selector(w, &name, *span, ctx);
        }
        internal::SimpleSelector::Nesting { span } => {
            w.raw("{\"type\":\"NestingSelector\",\"name\":");
            write_named_selector(w, "&", *span, ctx);
        }
        internal::SimpleSelector::Attribute {
            namespace_span,
            name_span,
            matcher,
            value_span,
            flags,
            span,
        } => {
            let name = raw_selector_name(ctx.source, *name_span, 0);
            let matcher = *matcher;
            // Svelte's `value` is the raw token with a string's quotes stripped —
            // escapes stay encoded (`[a=x\27]` → `x\27`), so no decode here.
            let value = value_span.map(|s| internal::attribute_value_text(ctx.source, s));
            let flags = *flags;
            // Half-decoded from the prefix's own span, like every other selector name —
            // `*` and the empty prefix fall out of the same slice.
            let namespace = namespace_span.map(|ns| raw_selector_name(ctx.source, ns, 0));
            w.raw("{\"type\":\"AttributeSelector\",\"start\":");
            w.span_start_end(ctx.positions, span.start, span.end);
            w.raw(",\"name\":");
            write_string(w, &name);
            w.raw(",\"matcher\":");
            // `as_str()` is a static escape-free operator (`=`/`~=`/`|=`/…), like
            // the sibling `Combinator` name — skip the escape scan.
            write_or_null(w, matcher.as_ref(), |w, m| w.token(m.as_str()));
            w.raw(",\"value\":");
            write_or_null(w, value.as_ref(), |w, v| write_string(w, v));
            w.raw(",\"flags\":");
            write_or_null(w, flags.as_ref(), |w, f| write_string(w, f));
            if let Some(ns) = &namespace {
                w.raw(",\"namespace\":");
                write_string(w, ns);
            }
            w.raw("}");
        }
        internal::SimpleSelector::PseudoClass {
            args,
            name_end,
            span,
        } => {
            let name_span = Span {
                start: crate::comments::pseudo_name_start(ctx.source.as_bytes(), span.start),
                end: *name_end,
            };
            let name = raw_selector_name(ctx.source, name_span, 0);
            w.raw("{\"type\":\"PseudoClassSelector\",\"name\":");
            write_string(w, &name);
            w.raw(",\"args\":");
            write_or_null(w, args.as_ref(), |w, a| write_pseudo_args(w, a, ctx));
            w.span_start_end_field(ctx.positions, span.start, span.end);
            w.raw("}");
        }
        internal::SimpleSelector::PseudoElement {
            args,
            name_end,
            span,
        } => {
            let name_end = *name_end;
            // Past BOTH colons and any comment glued to either (`:/* c */:before`) —
            // never a bare `2`.
            let name = raw_selector_name(
                ctx.source,
                Span {
                    start: crate::comments::pseudo_name_start(ctx.source.as_bytes(), span.start),
                    end: name_end,
                },
                0,
            );
            w.raw("{\"type\":\"PseudoElementSelector\",\"name\":");
            write_string(w, &name);
            w.span_start_end_field(ctx.positions, span.start, span.end);
            // `args` is emitted only when present — Svelte spreads the key in
            // conditionally (`...(args && { args })`), so an argument-less
            // `::before` carries no `args` at all, unlike a pseudo-CLASS (which
            // always emits `args`, `null` when absent).
            if let Some(args) = args {
                w.raw(",\"args\":");
                write_pseudo_args(w, args, ctx);
            }
            w.raw("}");
        }
        internal::SimpleSelector::Percentage { value, span } => {
            let value_str = if value.fract() == 0.0 {
                format!("{}%", *value as i64)
            } else {
                format!("{value}%")
            };
            w.raw("{\"type\":\"Percentage\",\"value\":");
            write_string(w, &value_str);
            w.span_start_end_field(ctx.positions, span.start, span.end);
            w.raw("}");
        }
        internal::SimpleSelector::Nth { span } => {
            // An An+B term inside pseudo-class args. parseCss stores the value
            // verbatim (the raw source slice — never operator-normalized like the
            // printer's output). For an `An+B of S` term the span folds in the
            // ` of ` (`"2n of "`), matching Svelte, which reads `S` as sibling
            // selectors rather than a nested list — so no `selector` is emitted
            // here (only the dedicated `:nth-*()` path nests `S` under
            // `Nth.selector`).
            w.raw("{\"type\":\"Nth\",\"value\":");
            write_string(w, span.extract(ctx.source));
            w.span_start_end_field(ctx.positions, span.start, span.end);
            w.raw("}");
        }
        // Forgiving-list `Invalid`s are filtered before convert (see
        // `write_selector_list_filtered`); the non-filtering path (rule preludes)
        // never contains them.
        #[expect(clippy::unreachable)]
        internal::SimpleSelector::Invalid { .. } => {
            unreachable!("Invalid selectors should be filtered in write_selector_list_filtered")
        }
    }
}

/// Where a `<wq-name>`'s element name begins, given its `<ns-prefix>`'s leading-token
/// span: past the `|` that follows the prefix, and past any comment run on either side of
/// it. Bounded by `limit` (the selector's own end).
///
/// Two [`crate::comments::skip_trivia_forward`] steps rather than a scan for `|`, because
/// only the *first* `|` outside the prefix is the separator — an escaped one inside it
/// (`a\|b|rect`) is part of the prefix's name.
fn wq_name_start(source: &str, prefix: Span, limit: u32) -> usize {
    let bytes = source.as_bytes();
    let limit = limit as usize;
    let pipe = crate::comments::skip_trivia_forward(bytes, prefix.end as usize, limit);
    crate::comments::skip_trivia_forward(bytes, (pipe + 1).min(limit), limit)
}

/// A `TypeSelector` (a type or universal selector): the [`write_named_selector`] shape
/// with Svelte's `namespace` between `name` and `start`, present only when the source
/// has a `<ns-prefix>`. The prefix is half-decoded like every selector name, so `*|a`
/// gives `"*"` and the no-namespace `|a` gives `""` — distinct from an absent key, which
/// is a bare `a` (any namespace, or the default one).
fn write_type_selector(
    w: &mut JsonWriter,
    name: &str,
    namespace_span: Option<Span>,
    span: Span,
    ctx: &Ctx<'_>,
) {
    w.raw("{\"type\":\"TypeSelector\",\"name\":");
    let Some(prefix) = namespace_span else {
        write_named_selector(w, name, span, ctx);
        return;
    };
    write_string(w, name);
    w.raw(",\"namespace\":");
    write_string(w, &raw_selector_name(ctx.source, prefix, 0));
    ctx.tail(w, b",", span.start, span.end, b"}");
}

/// The shared `{type, name, start, end}` shape (Class/Id/Nesting, and a `TypeSelector`
/// with no namespace), from the name on: the caller has written the node's head,
/// `{"type":"…","name":`, as one literal of its own — a constant-width store at each
/// site, where a node type handed down here would be three appends, the middle one a
/// runtime-length copy.
///
/// The trailing `start`/`end`/`}` burst is one in-place window (module doc, §The
/// `start`/`end` bursts) — this node carries no `metadata`, so the closing brace joins
/// it.
fn write_named_selector(w: &mut JsonWriter, name: &str, span: Span, ctx: &Ctx<'_>) {
    write_string(w, name);
    ctx.tail(w, b",", span.start, span.end, b"}");
}

/// Emit a functional pseudo-class's or pseudo-element's args (an `Nth` node, a
/// nested `SelectorList`, or a `::part()` ident run projected onto one).
fn write_pseudo_args(w: &mut JsonWriter, args: &internal::PseudoClassArgs<'_>, ctx: &Ctx<'_>) {
    match args {
        internal::PseudoClassArgs::Nth {
            value,
            of_selector,
            value_span,
            ..
        } => {
            // The public span is `[value_span.start, content_end)` — both ends
            // pulled in from the internal `span`, which spans the whole `(…)` for
            // the printer's benefit:
            //  - START at the An+B token (not `(`), so a leading comment
            //    (`:nth-child(/* c */ 2n)`) isn't absorbed — matching parseCss and
            //    tsv's own selector-list args (`:is(/* c */ .a)`).
            //  - END at the last content token — the `of S` selector list's end
            //    when present, else the trimmed An+B value — not `span.end` (which
            //    reaches `)`). Matches Svelte's `read_selector_list`, which captures
            //    its end before `allow_comment_or_whitespace`.
            // The internal `span` is untouched (it reaches `)` so the printer can
            // find leading/trailing gap comments via `[span.start, value_span.start)`
            // and `[content_end, span.end)`), so only the wire offsets move.
            let content_end = of_selector
                .as_ref()
                .map_or(value_span.end, |sel| sel.span.end);
            let public_span = Span::new(value_span.start, content_end);
            write_wrap_single_selector(w, public_span, ctx, |w, ctx| {
                w.raw("{\"type\":\"Nth\",\"value\":");
                write_string(w, value);
                w.span_start_end_field(ctx.positions, public_span.start, public_span.end);
                if let Some(sel) = of_selector {
                    w.raw(",\"selector\":");
                    write_selector_list_filtered(w, sel, ctx);
                }
                w.raw("}");
            });
        }
        internal::PseudoClassArgs::SelectorList { selectors, .. } => {
            write_selector_list_filtered(w, selectors, ctx);
        }
        internal::PseudoClassArgs::Part { ident_spans, .. } => {
            write_part_args(w, ident_spans, ctx);
        }
    }
}

/// Emit a `::part( <ident>+ )` argument as the `SelectorList` Svelte produces for
/// it. Svelte has no `::part` grammar — it reads *every* pseudo-element argument
/// with `read_selector_list`, so a space-separated part run comes back as one
/// `ComplexSelector` whose names are `TypeSelector`s joined by descendant
/// combinators. tsv keeps the run as `PseudoClassArgs::Part` (the printer needs the
/// per-ident spans to place comments), so the wire shape is synthesized here from
/// `ident_spans` rather than walking a selector tree that was never built.
///
/// The gaps come out of the spans: each name after the first takes a `' '`
/// combinator covering `[prev_end, start)` — Svelte's `read_combinator` returns
/// exactly the whitespace run it consumed — and its `RelativeSelector` starts at
/// that combinator. `ident_spans` is always non-empty (the parser rejects an empty
/// `::part()`).
fn write_part_args(w: &mut JsonWriter, ident_spans: &[Span], ctx: &Ctx<'_>) {
    #[expect(clippy::unreachable)]
    let (Some(first), Some(last)) = (ident_spans.first(), ident_spans.last()) else {
        unreachable!("::part() always names at least one part — the parser rejects an empty arg")
    };
    write_synth_selector_list(w, Span::new(first.start, last.end), ctx, |w, ctx| {
        write_array(w, ident_spans.iter().enumerate(), |w, (i, span)| {
            // The compound's own extent starts at the combinator, so the first name
            // starts at itself and every later one at the end of its predecessor.
            let rel_start = if i == 0 {
                span.start
            } else {
                ident_spans[i - 1].end
            };
            let combinator = (i > 0).then(|| Span::new(rel_start, span.start));
            write_synth_relative_selector(
                w,
                combinator,
                Span::new(rel_start, span.end),
                ctx,
                |w, ctx| {
                    w.raw("{\"type\":\"TypeSelector\",\"name\":");
                    write_named_selector(w, &raw_selector_name(ctx.source, *span, 0), *span, ctx);
                },
            );
        });
    });
}

/// Wrap a single simple selector in the full nesting `parseCss` emits:
/// SelectorList → ComplexSelector → RelativeSelector → `[<simple>]`, all sharing
/// `span`. The inner simple selector is emitted by `emit_simple`.
fn write_wrap_single_selector(
    w: &mut JsonWriter,
    span: Span,
    ctx: &Ctx<'_>,
    emit_simple: impl FnOnce(&mut JsonWriter, &Ctx<'_>),
) {
    write_synth_selector_list(w, span, ctx, |w, ctx| {
        w.raw("[");
        write_synth_relative_selector(w, None, span, ctx, emit_simple);
        w.raw("]");
    });
}

/// The `SelectorList` → `ComplexSelector` shell `parseCss` wraps a *synthesized*
/// pseudo argument in — both nodes spanning `span`, `emit_relatives` writing the
/// `RelativeSelector` array beneath them.
///
/// Two arguments are synthesized rather than walked (an `Nth` term, a `::part()`
/// ident run) and they differ only in that array, so the shell is stated once.
fn write_synth_selector_list(
    w: &mut JsonWriter,
    span: Span,
    ctx: &Ctx<'_>,
    emit_relatives: impl FnOnce(&mut JsonWriter, &Ctx<'_>),
) {
    w.raw("{\"type\":\"SelectorList\",\"start\":");
    w.span_start_end(ctx.positions, span.start, span.end);
    w.raw(",\"children\":[{\"type\":\"ComplexSelector\",\"start\":");
    w.span_start_end(ctx.positions, span.start, span.end);
    w.raw(",\"children\":");
    emit_relatives(w, ctx);
    if ctx.has_metadata {
        w.raw(COMPLEX_META);
    }
    w.raw("}]}"); // close ComplexSelector, SelectorList.children, SelectorList
}

/// One synthesized `RelativeSelector` holding the single simple selector
/// `emit_simple` writes. `span` is the compound's extent, which begins at its
/// **combinator** rather than at the selector; `combinator` is the gap that
/// combinator covers, `None` for the first compound in a chain.
fn write_synth_relative_selector(
    w: &mut JsonWriter,
    combinator: Option<Span>,
    span: Span,
    ctx: &Ctx<'_>,
    emit_simple: impl FnOnce(&mut JsonWriter, &Ctx<'_>),
) {
    w.raw("{\"type\":\"RelativeSelector\",\"combinator\":");
    match combinator {
        // Descendant is the only combinator a synthesized argument produces —
        // Svelte's `read_combinator` hands back exactly the whitespace run it ate.
        Some(gap) => write_combinator(w, " ", gap, ctx),
        None => w.null(),
    }
    w.raw(",\"selectors\":[");
    emit_simple(w, ctx);
    w.raw("],\"start\":");
    w.span_start_end(ctx.positions, span.start, span.end);
    if ctx.has_metadata {
        w.raw(RELATIVE_META);
    }
    w.raw("}");
}
