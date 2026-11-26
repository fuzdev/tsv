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
//! `write_string` (byte-identical to `serde_json`, and to `JsonWriter::string`:
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
//!   named selectors (`write_named_selector`, the class / id / type / pseudo names
//!   `write_selector_name` writes before the same tail, and the namespaced arm of
//!   `write_type_selector`, which writes that tail itself) — close through
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
//! # Shared span words
//!
//! A selector list of one complex selector — nearly every rule prelude — has that selector's span,
//! and a complex selector of one relative selector has that one's: three bursts, one `start`/`end`.
//! `write_selector_list_inner` turns the pair into digit words once ([`JsonWriter::span_words`]) and
//! hands them down, so the complex selector's head and the relative selector's tail write them
//! through [`JsonWriter::start_end_head_words`] / [`JsonWriter::start_end_tail_words`] without
//! generating the digits again. Each hand-off is guarded by a span comparison at the parent — the
//! words' reuse contract ([`SpanWords`]) — and restated against the node's own span in debug builds.
//! The chain pays only while the three emitters are one function: `write_complex_selector` and
//! `write_relative_selector` are `inline(always)` and the complex selector's loop over its
//! relative selectors is open-coded (each says why).
//!
//! The chain reaches one node further down and one further up. Down: a compound of one class or
//! one unprefixed type selector (`.a`, `div` — most compounds) usually spans its relative
//! selector, so `write_relative_selector` writes that node itself, ahead of the general loop, and
//! closes it on the same words. Up: a rule opens where its prelude opens and closes where its
//! block closes, so the list hands back its `start` word and the block its `end` word
//! ([`SpanHalf`], one register pair each) and `write_rule` joins them ([`SpanWords::join`]) for
//! its own tail. Both are guarded by span comparisons like the rest of the chain. Handing the
//! words down to every simple selector's arm instead measured worse on instructions, and widened
//! `write_selector_list_inner`'s frame, which the pseudo-class argument recursion repeats.
//!
//! The windows are the span-only wire's shape. On the wire that carries `loc`, which sits
//! between `end` and the closing constant, [`Ctx::head`] / [`Ctx::tail`] write the same burst
//! piecewise with the `loc` field in place.
//!
//! # One walk, two products
//!
//! This file is mounted twice: as `convert::write`, the JSON copy, writing into a
//! [`tsv_lang::JsonWriter`]; and behind `_tape` as `convert::write_tape::write`, the tape copy,
//! writing the tape (`docs/tape_format.md`) into a `tsv_lang::tape::TapeWriter` (`write_tape.rs`).
//! Each copy is concrete — `Sink` names its sink — so the JSON copy is the code it is with no
//! tape at all, item for item: a writer generic over the sink inlined its JSON instance
//! differently. What differs between the two copies is stated in the file, in two macros each
//! mount defines before mounting it: `tape! { … }` holds what only the tape copy compiles (its
//! sink, its string sites, its entry point, and every site's tape-structure call — through the
//! site macros, `write_macros.rs`), and `json_only! { … }` what only the JSON copy does (its
//! sink, its escapers, its entry points). The JSON mount expands `tape!` to nothing, so the
//! JSON copy holds no tape call. Code inside `tape! {}` / `json_only! {}` is invisible to
//! rustfmt (a brace macro's body is left as written), so keep those blocks to aliases and entry
//! points, and their lines within the width by hand.
//!
//! A site states its bytes ONCE, as a token spec (`raw!(w; open Rule key Rule.prelude)`): the
//! JSON copy writes the `concat!`'d literal, the tape copy the record's open and the field's
//! mask bit, from the same tokens, so the two cannot drift. Every entry, key and keyword a spec
//! names is looked up in the tape's tables in a `const` — a name the tables lack fails the
//! tape copy's build. A data call (`w.span_start_end(…)`, `put_string(w, …)`) is the same call
//! in both copies: `tsv_lang::WireSink` is the contract the two sinks keep, and the tape writes
//! the data's words. The span-words chain is the JSON copy's alone — the tape sink makes no
//! words, so its explicit-offset arms are the tape's path.
//!
//! ⚠️ **To edit a site, edit its spec, never one half.** A literal written straight to
//! `w.raw` with no spec is a JSON byte the tape cannot see; a new helper both copies call
//! moves the JSON copy's codegen when the tape copy is built beside it (the reason
//! `declaration_split.rs` and `prelude_string.rs` are mounted once per copy). `cargo test
//! --test tape_roundtrip` grades the tape copy against the JSON copy over every CSS fixture
//! document and `<style>` body.
//!
//! [`JsonWriter::span_words`]: tsv_lang::JsonWriter::span_words
//! [`JsonWriter::stage_run`]: tsv_lang::JsonWriter::stage_run
//! [`JsonWriter::start_end`]: tsv_lang::JsonWriter::start_end
//! [`JsonWriter::start_end_field`]: tsv_lang::JsonWriter::start_end_field
//! [`JsonWriter::start_end_head`]: tsv_lang::JsonWriter::start_end_head
//! [`JsonWriter::start_end_head_words`]: tsv_lang::JsonWriter::start_end_head_words
//! [`JsonWriter::start_end_tail`]: tsv_lang::JsonWriter::start_end_tail
//! [`JsonWriter::start_end_tail_words`]: tsv_lang::JsonWriter::start_end_tail_words

use super::{convert_prelude_to_string, split_declaration_svelte_compat};
use crate::ast;
use crate::ast::convert::{
    WireComment, raw_selector_name, selector_contains_invalid, strip_css_comments_collecting,
    trim_wire_end, trim_wire_start,
};
use std::borrow::Cow;
use tsv_lang::{Span, SpanHalf, SpanWords, Wire, WirePositions, WireTables};

json_only! {
    /// The JSON copy's sink and its string sites: this module's out-of-line escapers and
    /// `tsv_lang`'s list helpers.
    type Sink<'a> = JsonWriter;
    use self::{
        write_keyed_string as put_keyed_string, write_selector_name as put_selector_name,
        write_string as put_string,
    };
    use tsv_lang::{
        JsonWriter, write_array as put_list, write_or_null as put_record_or_null,
        write_or_null as put_ref_or_null,
    };
}

tape! {
    /// The tape copy's sink, and its string sites and list helpers (`write_tape.rs`).
    type Sink<'a> = tsv_lang::tape::TapeWriter<'a>;
    // the JSON-only literals (`raw`, `raw_fixed`) are the sink trait's alone
    use tsv_lang::WireSink as _;
    use super::{
        put_keyed_string, put_list, put_record_or_null, put_ref_or_null, put_selector_name,
        put_string,
    };
}

/// An array's opening and items: `open`, then the comma-separated items. The caller writes the
/// `]`, as the first byte of whatever constant follows the array, so a node's array costs it no
/// append of its own on either side (`tsv_lang::write_array` spends one on each bracket).
///
/// `open` is always empty — the node's head burst writes the `[` as a spec (`lopen`), which the
/// tape sees, where a literal here would be a JSON byte the tape cannot see — and a compile-time
/// assertion holds every caller to it.
#[inline]
fn write_array_open<T, const N: usize>(
    w: &mut Sink<'_>,
    open: &[u8; N],
    items: impl IntoIterator<Item = T>,
    mut f: impl FnMut(&mut Sink<'_>, T),
) {
    const {
        assert!(
            N == 0,
            "a literal `open` is a JSON byte the tape cannot see"
        );
    };
    w.raw_fixed(open);
    let mut first = true;
    for item in items {
        if !first {
            raw!(w; sep);
        }
        first = false;
        f(w, item);
    }
}

/// The per-document environment every writer function shares.
#[derive(Clone, Copy)]
struct Ctx<'a> {
    source: &'a str,
    /// Reads what the stores of the tree being written hold.
    view: ast::CssView<'a>,
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
    /// [`tsv_lang::JsonWriter::start_end_tail`]'s one window — or, on a wire that carries `loc`,
    /// the same bytes with `loc` after `end`, written piecewise.
    #[expect(clippy::inline_always)]
    #[inline(always)]
    fn tail<const L: usize, const C: usize>(
        &self,
        w: &mut Sink<'_>,
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
    /// [`tsv_lang::JsonWriter::start_end_head`]'s one window — or, on a wire that carries `loc`,
    /// the same bytes with `loc` after `end`, written piecewise.
    #[expect(clippy::inline_always)]
    #[inline(always)]
    fn head<const L: usize, const C: usize>(
        &self,
        w: &mut Sink<'_>,
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

json_only! {
    /// Convert the internal CSS nodes straight to standalone-`StyleSheetFile` wire
    /// bytes on `wire` — one AST walk, with byte→char offset translation fused in, in
    /// the coordinates `crate::WIRE_COORDINATES` states. (An embedded `<style>` takes the
    /// Svelte writer's positions.)
    pub(crate) fn write_stylesheet_file_bytes(
        parsed: ast::CssParsedRef<'_>,
        source: &str,
        wire: Wire,
    ) -> Vec<u8> {
        let tables = WireTables::new(source, crate::WIRE_COORDINATES, wire);
        let ctx = Ctx {
            source,
            view: parsed.view,
            positions: tables.positions(),
            has_metadata: true,
        };
        let mut w =
            JsonWriter::with_capacity(tsv_lang::estimated_json_capacity(source.len(), wire));
        write_stylesheet_file(&mut w, parsed.stylesheet, &ctx);
        w.into_bytes()
    }

    /// Emit an embedded-`<style>` stylesheet's `children` array (no `metadata`) into
    /// a caller-owned writer, handing back the `comments` run the same walk gathered
    /// — the composition entry the Svelte writer uses for a `<style>` element, whose
    /// wire puts those two arrays side by side. `positions` must be the host
    /// document's (spans are in host-file coordinates, and the line table is the
    /// Svelte document's).
    pub fn write_css_children<'a>(
        w: &mut JsonWriter,
        stylesheet: impl Into<ast::CssParsedRef<'a>>,
        source: &str,
        positions: WirePositions<'_>,
    ) -> CssComments {
        let parsed = stylesheet.into();
        let ctx = Ctx {
            source,
            view: parsed.view,
            positions,
            has_metadata: false,
        };
        write_children(w, parsed.stylesheet, &ctx)
    }

    /// Emit the `comments` array `write_css_children` collected. Split from it
    /// because the wire writes `children` first and the two are separate keys.
    pub fn write_css_comments(
        w: &mut JsonWriter,
        comments: &CssComments,
        source: &str,
        positions: WirePositions<'_>,
    ) {
        write_comments(w, comments, source, positions);
    }
}

json_only! {
    /// A dynamic string value, escaped byte-identical to `serde_json`: one shared out-of-line
    /// copy of [`JsonWriter::string_led_words`] with an empty lead, so a clean string shorter
    /// than a word is written as one window append with no call, and one of up to two words by
    /// a tail call that saves no register.
    #[inline(never)]
    fn write_string(w: &mut JsonWriter, s: &str) {
        w.string_led_words(&[], s.as_bytes());
    }

    /// A selector name written from source: `span` less `prefix_len` bytes of sigil, with
    /// [`raw_selector_name`]'s escape scan and [`write_string`]'s JSON-escape scan asked as
    /// one. A name with no `\`, `"`, control or non-ASCII byte is written straight.
    #[inline(never)]
    fn write_selector_name(w: &mut JsonWriter, source: &str, span: Span, prefix_len: usize) {
        let (start, end) = (span.start as usize + prefix_len, span.end as usize);
        let bytes = source.as_bytes();
        if start <= end
            && end <= bytes.len()
            && tsv_lang::swar::next_byte_below_or_of(bytes, start, end, 0x20, [b'\\', b'"']) == end
        {
            w.string_escape_free_led(b"", &bytes[start..end]);
            return;
        }
        write_string(w, &raw_selector_name(source, span, prefix_len));
    }

    /// [`write_string`] behind its field's key `lead` (`,"property":`), written as part of the
    /// string's own window append rather than as an append of its own — one out-of-line copy per
    /// key, for the two fields every declaration carries.
    #[inline(never)]
    fn write_keyed_string<const K: usize>(w: &mut JsonWriter, lead: &[u8; K], s: &[u8]) {
        w.string_led_words(lead, s);
    }
}

/// The standalone `StyleSheetFile` root: `type`, `start` (0), `end` (source
/// length), `children`, `comments`.
fn write_stylesheet_file(w: &mut Sink<'_>, stylesheet: &ast::CssStyleSheet, ctx: &Ctx<'_>) {
    raw!(w; open StyleSheetFile key StyleSheetFile.start);
    w.span_start_end(ctx.positions, 0, ctx.source.len() as u32);
    raw!(w; key StyleSheetFile.children);
    let comments = write_children(w, stylesheet, ctx);
    raw!(w; key StyleSheetFile.comments);
    write_comments(w, &comments, ctx.source, ctx.positions);
    raw!(w; close);
}

/// Every comment Svelte's CSS parser captures on a stylesheet root, in source
/// order — gathered by the `children` walk, opaque to its caller, spent by
/// `write_css_comments`.
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
fn write_children(w: &mut Sink<'_>, stylesheet: &ast::CssStyleSheet, ctx: &Ctx<'_>) -> CssComments {
    let mut out = Vec::new();
    put_list(w, ctx.view.nodes(stylesheet.nodes), |w, n| {
        write_node(w, n, ctx, &mut out);
    });
    out.extend(stylesheet.comments.iter().map(|c| WireComment {
        span: c.span,
        position: None,
    }));
    out.sort_unstable_by_key(|c| (c.span.start, c.position.is_none()));
    out.dedup_by_key(|c| c.span.start);
    CssComments(out)
}

/// Emit `comments` as the wire's `CSSComment` array. It reads no tree, only the run the
/// `children` walk gathered, so it takes the source and the positions and no view.
fn write_comments(
    w: &mut Sink<'_>,
    comments: &CssComments,
    source: &str,
    positions: WirePositions<'_>,
) {
    put_list(w, &comments.0, |w, c| {
        // `value` is the comment's interior — Svelte's `read_comment` reads it
        // between the delimiters it has already eaten.
        let interior = Span::new(c.span.start + 2, c.span.end - 2);
        raw!(w; open CSSComment key CSSComment.value);
        put_string(w, interior.extract(source));
        w.span_start_end_field(positions, c.span.start, c.span.end);
        if let Some(position) = c.position {
            raw!(w; key CSSComment.position);
            w.u32(position);
        }
        raw!(w; close);
    });
}

/// Emit a CSS node (a `Rule` or an `Atrule`).
fn write_node(
    w: &mut Sink<'_>,
    node: &ast::CssNode,
    ctx: &Ctx<'_>,
    comments: &mut Vec<WireComment>,
) {
    match node {
        ast::CssNode::Rule(rule) => write_rule(w, rule, ctx, comments),
        ast::CssNode::Atrule(atrule) => write_atrule(w, atrule, ctx, comments),
    }
}

/// Emits a `Rule` node. Field order: `type`, `prelude`, `block`, `start`,
/// `end`, then `metadata` (standalone only).
///
/// The trailing `start`/`end` burst is one in-place window (module doc, §The
/// `start`/`end` bursts); the closing constant — `metadata` and the `}`, or the `}`
/// alone — is an append of its own, since which one it is varies by document. A rule
/// opens where its prelude does and closes where its block does, so the window is
/// written from the two halves' words when both made them (module doc, §Shared span
/// words).
fn write_rule(
    w: &mut Sink<'_>,
    rule: &ast::CssRule,
    ctx: &Ctx<'_>,
    comments: &mut Vec<WireComment>,
) {
    raw!(w; open Rule key Rule.prelude);
    let prelude_start = write_selector_list(w, &rule.selector, ctx);
    raw!(w; key Rule.block);
    let block_end = write_block(
        w,
        rule.block_span,
        ctx.view.block_children(rule.declarations),
        ctx,
        comments,
    );
    let shared =
        if rule.span.start == rule.selector.span.start && rule.span.end == rule.block_span.end {
            SpanWords::join(prelude_start, block_end)
        } else {
            None
        };
    match shared {
        Some(words) => {
            #[cfg(debug_assertions)]
            assert_words_of(words, rule.span, ctx, "rule");
            tail_words!(w; [sep] words; []);
        }
        None => tail!(ctx, w; [sep] rule.span.start, rule.span.end; []),
    }
    if ctx.has_metadata {
        raw!(w; rule_meta close);
    } else {
        raw!(w; close);
    }
}

/// Emits an `Atrule` node. Field order: `type`, `start`, `end`, `name`,
/// `prelude`, `block` (unlike `Rule`, whose positions trail — parseCss
/// constructs the two literals differently). `Atrule` carries no `metadata`.
fn write_atrule(
    w: &mut Sink<'_>,
    atrule: &ast::CssAtrule,
    ctx: &Ctx<'_>,
    comments: &mut Vec<WireComment>,
) {
    raw!(w; open Atrule key Atrule.start);
    w.span_start_end(ctx.positions, atrule.span.start, atrule.span.end);
    raw!(w; key Atrule.name);
    // Half-decoded from source, like a selector name: parseCss reads both with
    // `read_identifier`, so an identity escape keeps its backslash (`@a\?b` → `a\?b`)
    // where the internal `name` is fully decoded.
    put_string(w, &raw_selector_name(ctx.source, atrule.name_span, 0));
    raw!(w; key Atrule.prelude);
    let prelude = convert_prelude_to_string(&atrule.prelude, ctx.source);
    put_string(w, &prelude);
    collect_prelude_comments(atrule, ctx.source, comments);
    raw!(w; key Atrule.block);
    put_record_or_null(w, atrule.block.as_ref(), |w, b| {
        // An at-rule's positions lead, so nothing it writes after the block shares its end.
        let _ = write_block(
            w,
            b.span,
            ctx.view.block_children(b.children),
            ctx,
            comments,
        );
    });
    raw!(w; close);
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
    atrule: &ast::CssAtrule,
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
///
/// Returns the `end` half of the words its head was written from, for the rule that
/// closes where the block does ([`SpanHalf::NONE`] when the head made none).
fn write_block(
    w: &mut Sink<'_>,
    block_span: Span,
    children: &[ast::CssBlockChild],
    ctx: &Ctx<'_>,
    comments: &mut Vec<WireComment>,
) -> SpanHalf {
    let words = if ctx.positions.has_locations() {
        None
    } else {
        Sink::span_words(ctx.pos(block_span.start), ctx.pos(block_span.end))
    };
    match words {
        Some(words) => head_words!(w;
            [open Block key Block.start] words; [key Block.children lopen]),
        None => head!(ctx, w;
            [open Block key Block.start] block_span.start, block_span.end;
            [key Block.children lopen]),
    }
    let mut first = true;
    for child in children {
        if let ast::CssBlockChild::Comment(c) = child {
            comments.push(WireComment {
                span: c.span,
                position: None,
            });
            continue;
        }
        if !first {
            raw!(w; sep);
        }
        first = false;
        write_block_child(w, child, ctx, comments);
    }
    raw!(w; lclose close);
    words.map_or(SpanHalf::NONE, SpanWords::end_half)
}

fn write_block_child(
    w: &mut Sink<'_>,
    child: &ast::CssBlockChild,
    ctx: &Ctx<'_>,
    comments: &mut Vec<WireComment>,
) {
    match child {
        ast::CssBlockChild::Declaration(d) => write_declaration(w, d, ctx, comments),
        ast::CssBlockChild::Rule(r) => write_rule(w, r, ctx, comments),
        ast::CssBlockChild::Atrule(a) => write_atrule(w, a, ctx, comments),
        // Comments are filtered out before this call (see `write_block`).
        ast::CssBlockChild::Comment(_) => {}
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

fn split_declaration<'a>(decl: &ast::CssDeclaration, source: &'a str) -> DeclarationSplit<'a> {
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
    w: &mut Sink<'_>,
    decl: &ast::CssDeclaration,
    ctx: &Ctx<'_>,
    comments: &mut Vec<WireComment>,
) {
    // A comment-free declaration whose trims stop on plain ASCII bytes is written straight
    // from the source bytes: its `property` and `value` are the slices `split_declaration`
    // and the wire trims (`trim_wire_end`, `trim_wire_start`, the class `trim_wire` names)
    // produce, asked on bytes. A trim that stops where the class may go on — `<VT>` or a
    // non-ASCII byte (`tsv_lang::stop_byte_may_be_js_whitespace`) — takes the `str` path.
    if !decl.has_block_comment {
        let bytes = ctx.source.as_bytes();
        let content_end = decl
            .important_end
            .map_or(decl.span.end, |e| e.max(decl.span.end));
        let end = crate::comments::scan_to_terminator(ctx.source, content_end as usize);
        let colon = decl.colon_offset as usize;
        if let (Some(property), Some(value)) = (
            bytes.get(decl.span.start as usize..colon),
            bytes.get(colon + 1..end),
        ) {
            let property = property.trim_ascii_end();
            let value = value.trim_ascii();
            if !(tsv_lang::stop_byte_may_be_js_whitespace(property.last())
                || tsv_lang::stop_byte_may_be_js_whitespace(value.first())
                || tsv_lang::stop_byte_may_be_js_whitespace(value.last()))
            {
                head!(ctx, w;
                    [open Declaration key Declaration.start]
                    decl.span.start, end as u32; []);
                keyed!(w; [key Declaration.property] property);
                keyed!(w; [key Declaration.value] value);
                raw!(w; close);
                return;
            }
        }
    }
    let split = split_declaration(decl, ctx.source);
    let value: Cow<'_, str> = if decl.has_block_comment {
        strip_css_comments_collecting(split.value, split.value_start, comments)
    } else {
        // Nothing to strip, so the strip reduces to the trim it ends with — and
        // the front is already trimmed.
        Cow::Borrowed(trim_wire_end(split.value))
    };

    head!(ctx, w;
        [open Declaration key Declaration.start]
        decl.span.start, split.end; []);
    keyed!(w; [key Declaration.property] trim_wire_end(split.property).as_bytes());
    keyed!(w; [key Declaration.value] value.as_bytes());
    raw!(w; close);
}

/// Emits a `SelectorList` node (rule preludes — parsed non-forgivingly, no
/// `Invalid`), returning the `start` half of its words for the rule that opens where
/// the list does.
fn write_selector_list(w: &mut Sink<'_>, sl: &ast::SelectorList, ctx: &Ctx<'_>) -> SpanHalf {
    write_selector_list_inner(w, sl, ctx, false)
}

/// Emits a `SelectorList` node for pseudo-class args — drops complex selectors
/// containing a forgiving-parse `Invalid`.
fn write_selector_list_filtered(w: &mut Sink<'_>, sl: &ast::SelectorList, ctx: &Ctx<'_>) {
    let _ = write_selector_list_inner(w, sl, ctx, true);
}

/// Returns the `start` half of the list's words ([`SpanHalf::NONE`] when it made none).
fn write_selector_list_inner(
    w: &mut Sink<'_>,
    sl: &ast::SelectorList,
    ctx: &Ctx<'_>,
    filter_invalid: bool,
) -> SpanHalf {
    // The list's digit words, made once (module doc, §Shared span words): a list of one
    // complex selector — nearly every list — shares that selector's span, so the
    // selector's head (and, through it, its one relative selector's tail) writes the
    // same words again. The `loc` wire writes its positions piecewise and takes none.
    let selectors = ctx.view.complex_selectors(sl.selectors);
    let words = if ctx.positions.has_locations() {
        None
    } else {
        Sink::span_words(ctx.pos(sl.span.start), ctx.pos(sl.span.end))
    };
    match words {
        Some(words) => head_words!(w;
            [open SelectorList key SelectorList.start] words;
            [key SelectorList.children lopen]),
        None => head!(ctx, w;
            [open SelectorList key SelectorList.start] sl.span.start, sl.span.end;
            [key SelectorList.children lopen]),
    }
    let shared = match selectors {
        [c] if c.span == sl.span => words,
        _ => None,
    };
    write_array_open(
        w,
        b"",
        selectors
            .iter()
            .filter(|c| !filter_invalid || !selector_contains_invalid(c, ctx.view)),
        |w, c| write_complex_selector(w, c, ctx, shared),
    );
    raw!(w; lclose close);
    words.map_or(SpanHalf::NONE, SpanWords::start_half)
}

/// Emits a `ComplexSelector` node. `shared` is its list's [`SpanWords`] when the
/// list proved the two spans equal (a one-selector list), else `None`.
///
/// `inline(always)`, as is [`write_relative_selector`]: the words ride registers
/// from the list's head to the relative selector's tail only while all three bodies
/// are one function. Measured outlined (with the words still threaded), the list → complex
/// call costs more than the shared words save.
#[expect(clippy::inline_always)]
#[inline(always)]
fn write_complex_selector(
    w: &mut Sink<'_>,
    c: &ast::ComplexSelector,
    ctx: &Ctx<'_>,
    shared: Option<SpanWords>,
) {
    #[cfg(debug_assertions)]
    if let Some(words) = shared {
        assert_words_of(words, c.span, ctx, "complex selector");
    }
    let words = match shared {
        Some(words) => Some(words),
        None if ctx.positions.has_locations() => None,
        None => Sink::span_words(ctx.pos(c.span.start), ctx.pos(c.span.end)),
    };
    match words {
        Some(words) => head_words!(w;
            [open ComplexSelector key ComplexSelector.start] words;
            [key ComplexSelector.children lopen]),
        None => head!(ctx, w;
            [open ComplexSelector key ComplexSelector.start] c.span.start, c.span.end;
            [key ComplexSelector.children lopen]),
    }
    let children = ctx.view.relative_selectors(c.children);
    let shared = match children {
        [r] if r.span == c.span => words,
        _ => None,
    };
    // Open-coded, not `write_array_open`: through its closure, LLVM outlined the
    // relative selector's body (and the words with it) out of this function — measured
    // a loss — where the loop keeps it inline.
    let mut first = true;
    for r in children {
        if !first {
            raw!(w; sep);
        }
        first = false;
        write_relative_selector(w, r, ctx, shared);
    }
    if ctx.has_metadata {
        raw!(w; lclose complex_meta close);
    } else {
        raw!(w; lclose close);
    }
}

/// Emits a `RelativeSelector` node. `combinator` is `null` (no skip) when
/// absent; field order is `combinator`, `selectors`, `start`, `end`, `metadata`.
///
/// The trailing `start`/`end` burst is one in-place window, led by the `]` that
/// closes `selectors` and stopping before `metadata` for `write_rule`'s reason (module
/// doc, §The `start`/`end` bursts) — written from `shared`, its complex selector's
/// [`SpanWords`], when the complex proved the two spans equal (its one relative
/// selector), and `inline(always)` for [`write_complex_selector`]'s reason.
#[expect(clippy::inline_always)]
#[inline(always)]
fn write_relative_selector(
    w: &mut Sink<'_>,
    r: &ast::RelativeSelector,
    ctx: &Ctx<'_>,
    shared: Option<SpanWords>,
) {
    #[cfg(debug_assertions)]
    if let Some(words) = shared {
        assert_words_of(words, r.span, ctx, "relative selector");
    }
    // The combinator-less head — the first compound of every selector — is one literal
    // through the `selectors` array's `[`.
    match (&r.combinator, &r.combinator_span) {
        (Some(comb), Some(span)) => {
            raw!(w; open RelativeSelector key RelativeSelector.combinator);
            write_combinator(w, comb.as_str(), *span, ctx);
            raw!(w; key RelativeSelector.selectors lopen);
        }
        _ => raw!(w; open RelativeSelector key RelativeSelector.combinator nul_rec
            key RelativeSelector.selectors lopen),
    }
    // A compound of one class or one unprefixed type selector — `.a`, `div`, the
    // commonest compounds — that spans the whole relative selector closes on the same
    // words, so it is written here, ahead of the general loop.
    match (shared, ctx.view.simple_selectors(r.selectors)) {
        (Some(words), [ast::SimpleSelector::Class { span }]) if *span == r.span => {
            #[cfg(debug_assertions)]
            assert_words_of(words, *span, ctx, "class selector");
            write_class_selector_head(w, *span, ctx);
            tail_words!(w; [sep] words; [close]);
        }
        (
            Some(words),
            [
                ast::SimpleSelector::Type {
                    namespace_span: None,
                    span,
                },
            ],
        ) if *span == r.span => {
            #[cfg(debug_assertions)]
            assert_words_of(words, *span, ctx, "type selector");
            write_type_selector_head(w, *span, ctx);
            tail_words!(w; [sep] words; [close]);
        }
        (_, simples) => write_array_open(w, b"", simples, |w, s| {
            write_simple_selector(w, s, ctx);
        }),
    }
    match shared {
        Some(words) => tail_words!(w; [lclose sep] words; []),
        None => tail!(ctx, w; [lclose sep] r.span.start, r.span.end; []),
    }
    if ctx.has_metadata {
        raw!(w; relative_meta close);
    } else {
        raw!(w; close);
    }
}

/// Restates the [`SpanWords`] reuse contract at a reuse site, in debug builds: the shared
/// `words` are the ones the node's own span makes.
#[cfg(debug_assertions)]
fn assert_words_of(words: SpanWords, span: Span, ctx: &Ctx<'_>, node: &str) {
    assert_eq!(
        Some(words),
        Sink::span_words(ctx.pos(span.start), ctx.pos(span.end)),
        "shared span words reused for a {node} of another span"
    );
}

/// A `ClassSelector` through its `name` — the name read from source past the `.` AND any
/// comment glued to it (`./* c */cls`), never a bare `1`. The caller writes the tail.
///
/// `inline(always)`, as is [`write_type_selector_head`]: each has two callers, the general
/// simple-selector arm and `write_relative_selector`'s lone-compound arm, and the class
/// head came out of line between them — a call on the commonest selector of all.
#[expect(clippy::inline_always)]
#[inline(always)]
fn write_class_selector_head(w: &mut Sink<'_>, span: Span, ctx: &Ctx<'_>) {
    let name_start = crate::comments::class_name_start(ctx.source.as_bytes(), span.start);
    raw!(w; open ClassSelector key ClassSelector.name);
    put_selector_name(w, ctx.source, span, (name_start - span.start) as usize);
}

/// An unprefixed `TypeSelector` through its `name`, read from source. The caller writes
/// the tail.
#[expect(clippy::inline_always)]
#[inline(always)]
fn write_type_selector_head(w: &mut Sink<'_>, span: Span, ctx: &Ctx<'_>) {
    raw!(w; open TypeSelector key TypeSelector.name);
    put_selector_name(w, ctx.source, span, 0);
}

fn write_combinator(w: &mut Sink<'_>, name: &'static str, span: Span, ctx: &Ctx<'_>) {
    raw!(w; open Combinator key Combinator.name);
    keyword!(w; Combinator.name, name); // ` ` / `>` / `+` / `~` / `||` — escape-free
    w.span_start_end_field(ctx.positions, span.start, span.end);
    raw!(w; close);
}

/// Emit a simple selector (type/universal/class/id/nesting/attribute/pseudo/percentage).
///
/// `inline(never)` is a placement pin. Its one caller is the general loop of
/// `write_relative_selector`, which sits inside `write_selector_list_inner` with the
/// lone-compound arm ahead of it; out of line, this body's frame is paid only by a
/// compound that reaches it, and the list's own frame, which the pseudo-class argument
/// recursion repeats, stays narrow. Inline, the list function is twice the size.
#[inline(never)]
fn write_simple_selector(w: &mut Sink<'_>, simple: &ast::SimpleSelector, ctx: &Ctx<'_>) {
    match simple {
        ast::SimpleSelector::Type {
            namespace_span,
            span,
        } => {
            let name = match namespace_span {
                None => {
                    write_type_selector_head(w, *span, ctx);
                    tail!(ctx, w; [sep] span.start, span.end; [close]);
                    return;
                }
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
        ast::SimpleSelector::Universal {
            namespace_span,
            span,
        } => {
            write_type_selector(w, "*", *namespace_span, *span, ctx);
        }
        ast::SimpleSelector::Class { span } => {
            write_class_selector_head(w, *span, ctx);
            tail!(ctx, w; [sep] span.start, span.end; [close]);
        }
        ast::SimpleSelector::Id { span } => {
            raw!(w; open IdSelector key IdSelector.name);
            put_selector_name(w, ctx.source, *span, 1);
            tail!(ctx, w; [sep] span.start, span.end; [close]);
        }
        ast::SimpleSelector::Nesting { span } => {
            raw!(w; open NestingSelector key NestingSelector.name);
            write_named_selector(w, "&", *span, ctx);
        }
        ast::SimpleSelector::Attribute {
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
            let value = value_span.map(|s| ast::attribute_value_text(ctx.source, s));
            let flags = *flags;
            // Half-decoded from the prefix's own span, like every other selector name —
            // `*` and the empty prefix fall out of the same slice.
            let namespace = namespace_span.map(|ns| raw_selector_name(ctx.source, ns, 0));
            raw!(w; open AttributeSelector key AttributeSelector.start);
            w.span_start_end(ctx.positions, span.start, span.end);
            raw!(w; key AttributeSelector.name);
            put_string(w, &name);
            raw!(w; key AttributeSelector.matcher);
            // `as_str()` is a static escape-free operator (`=`/`~=`/`|=`/…), like
            // the sibling `Combinator` name — skip the escape scan.
            put_ref_or_null(w, matcher.as_ref(), |w, m| token_ref!(w; m.as_str()));
            raw!(w; key AttributeSelector.value);
            put_ref_or_null(w, value.as_ref(), |w, v| put_string(w, v));
            raw!(w; key AttributeSelector.flags);
            put_ref_or_null(w, flags.as_ref(), |w, &f| {
                put_string(w, ctx.view.text(f));
            });
            if let Some(ns) = &namespace {
                raw!(w; key AttributeSelector.namespace);
                put_string(w, ns);
            }
            raw!(w; close);
        }
        ast::SimpleSelector::PseudoClass {
            args,
            name_end,
            span,
        } => {
            let name_span = Span {
                start: crate::comments::pseudo_name_start(ctx.source.as_bytes(), span.start),
                end: *name_end,
            };
            raw!(w; open PseudoClassSelector key PseudoClassSelector.name);
            put_selector_name(w, ctx.source, name_span, 0);
            raw!(w; key PseudoClassSelector.args);
            put_record_or_null(w, args.as_ref(), |w, a| write_pseudo_args(w, a, ctx));
            w.span_start_end_field(ctx.positions, span.start, span.end);
            raw!(w; close);
        }
        ast::SimpleSelector::PseudoElement {
            args,
            name_end,
            span,
        } => {
            let name_end = *name_end;
            // Past BOTH colons and any comment glued to either (`:/* c */:before`) —
            // never a bare `2`.
            let name_span = Span {
                start: crate::comments::pseudo_name_start(ctx.source.as_bytes(), span.start),
                end: name_end,
            };
            raw!(w; open PseudoElementSelector key PseudoElementSelector.name);
            put_selector_name(w, ctx.source, name_span, 0);
            w.span_start_end_field(ctx.positions, span.start, span.end);
            // `args` is emitted only when present — Svelte spreads the key in
            // conditionally (`...(args && { args })`), so an argument-less
            // `::before` carries no `args` at all, unlike a pseudo-CLASS (which
            // always emits `args`, `null` when absent).
            // A kept `<any-value>` has no wire node, so it carries no key either.
            let args = args
                .as_ref()
                .filter(|a| !matches!(a, ast::PseudoClassArgs::AnyValue { .. }));
            if let Some(args) = args {
                raw!(w; key PseudoElementSelector.args);
                write_pseudo_args(w, args, ctx);
            }
            raw!(w; close);
        }
        ast::SimpleSelector::Percentage { value, span } => {
            let value = value.get();
            let value_str = if value.fract() == 0.0 {
                format!("{}%", value as i64)
            } else {
                format!("{value}%")
            };
            raw!(w; open Percentage key Percentage.value);
            put_string(w, &value_str);
            w.span_start_end_field(ctx.positions, span.start, span.end);
            raw!(w; close);
        }
        ast::SimpleSelector::Nth { span } => {
            // An An+B term inside pseudo-class args. parseCss stores the value
            // verbatim (the raw source slice — never operator-normalized like the
            // printer's output). For an `An+B of S` term the span folds in the
            // ` of ` (`"2n of "`), matching Svelte, which reads `S` as sibling
            // selectors rather than a nested list — so no `selector` is emitted
            // here (only the dedicated `:nth-*()` path nests `S` under
            // `Nth.selector`).
            raw!(w; open Nth key Nth.value);
            put_string(w, span.extract(ctx.source));
            w.span_start_end_field(ctx.positions, span.start, span.end);
            raw!(w; close);
        }
        // Forgiving-list `Invalid`s are filtered before convert (see
        // `write_selector_list_filtered`); the non-filtering path (rule preludes)
        // never contains them.
        #[expect(clippy::unreachable)]
        ast::SimpleSelector::Invalid { .. } => {
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
    w: &mut Sink<'_>,
    name: &str,
    namespace_span: Option<Span>,
    span: Span,
    ctx: &Ctx<'_>,
) {
    raw!(w; open TypeSelector key TypeSelector.name);
    let Some(prefix) = namespace_span else {
        write_named_selector(w, name, span, ctx);
        return;
    };
    put_string(w, name);
    raw!(w; key TypeSelector.namespace);
    put_string(w, &raw_selector_name(ctx.source, prefix, 0));
    tail!(ctx, w; [sep] span.start, span.end; [close]);
}

/// The shared `{type, name, start, end}` shape for a name already in hand — the nesting
/// `&` and a universal `*` with no namespace — from the name on (a name read from source
/// is `write_selector_name`'s, followed by the same tail): the caller has written the node's head,
/// `{"type":"…","name":`, as one literal of its own — a constant-width store at each
/// site, where a node type handed down here would be three appends, the middle one a
/// runtime-length copy.
///
/// The trailing `start`/`end`/`}` burst is one in-place window (module doc, §The
/// `start`/`end` bursts) — this node carries no `metadata`, so the closing brace joins
/// it.
fn write_named_selector(w: &mut Sink<'_>, name: &str, span: Span, ctx: &Ctx<'_>) {
    put_string(w, name);
    tail!(ctx, w; [sep] span.start, span.end; [close]);
}

/// Emit a functional pseudo-class's or pseudo-element's args (an `Nth` node, a
/// nested `SelectorList`, or a `::part()` ident run projected onto one).
fn write_pseudo_args(w: &mut Sink<'_>, args: &ast::PseudoClassArgs, ctx: &Ctx<'_>) {
    match args {
        // A kept `<any-value>` has no wire node: parseCss rejects a non-selector argument,
        // and tsv keeps Svelte's shape for the accepted one — `args: null` on a
        // pseudo-class (a pseudo-element drops the key, filtered at its call site).
        ast::PseudoClassArgs::AnyValue { .. } => null_record!(w),
        ast::PseudoClassArgs::Nth {
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
                raw!(w; open Nth key Nth.value);
                put_string(w, ctx.view.text(*value));
                w.span_start_end_field(ctx.positions, public_span.start, public_span.end);
                if let Some(sel) = of_selector {
                    raw!(w; key Nth.selector);
                    write_selector_list_filtered(w, sel, ctx);
                }
                raw!(w; close);
            });
        }
        ast::PseudoClassArgs::SelectorList { selectors, .. } => {
            write_selector_list_filtered(w, selectors, ctx);
        }
        ast::PseudoClassArgs::Part { ident_spans, .. } => {
            write_part_args(w, ctx.view.part_spans(*ident_spans), ctx);
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
fn write_part_args(w: &mut Sink<'_>, ident_spans: &[Span], ctx: &Ctx<'_>) {
    #[expect(clippy::unreachable)]
    let (Some(first), Some(last)) = (ident_spans.first(), ident_spans.last()) else {
        unreachable!("::part() always names at least one part — the parser rejects an empty arg")
    };
    write_synth_selector_list(w, Span::new(first.start, last.end), ctx, |w, ctx| {
        put_list(w, ident_spans.iter().enumerate(), |w, (i, span)| {
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
                    raw!(w; open TypeSelector key TypeSelector.name);
                    put_selector_name(w, ctx.source, *span, 0);
                    tail!(ctx, w; [sep] span.start, span.end; [close]);
                },
            );
        });
    });
}

/// Wrap a single simple selector in the full nesting `parseCss` emits:
/// SelectorList → ComplexSelector → RelativeSelector → `[<simple>]`, all sharing
/// `span`. The inner simple selector is emitted by `emit_simple`.
fn write_wrap_single_selector(
    w: &mut Sink<'_>,
    span: Span,
    ctx: &Ctx<'_>,
    emit_simple: impl FnOnce(&mut Sink<'_>, &Ctx<'_>),
) {
    write_synth_selector_list(w, span, ctx, |w, ctx| {
        raw!(w; lopen);
        write_synth_relative_selector(w, None, span, ctx, emit_simple);
        raw!(w; lclose);
    });
}

/// The `SelectorList` → `ComplexSelector` shell `parseCss` wraps a *synthesized*
/// pseudo argument in — both nodes spanning `span`, `emit_relatives` writing the
/// `RelativeSelector` array beneath them.
///
/// Two arguments are synthesized rather than walked (an `Nth` term, a `::part()`
/// ident run) and they differ only in that array, so the shell is stated once.
fn write_synth_selector_list(
    w: &mut Sink<'_>,
    span: Span,
    ctx: &Ctx<'_>,
    emit_relatives: impl FnOnce(&mut Sink<'_>, &Ctx<'_>),
) {
    raw!(w; open SelectorList key SelectorList.start);
    w.span_start_end(ctx.positions, span.start, span.end);
    raw!(w; key SelectorList.children lopen open ComplexSelector key ComplexSelector.start);
    w.span_start_end(ctx.positions, span.start, span.end);
    raw!(w; key ComplexSelector.children);
    emit_relatives(w, ctx);
    if ctx.has_metadata {
        raw!(w; complex_meta);
    }
    raw!(w; close lclose close); // close ComplexSelector, SelectorList.children, SelectorList
}

/// One synthesized `RelativeSelector` holding the single simple selector
/// `emit_simple` writes. `span` is the compound's extent, which begins at its
/// **combinator** rather than at the selector; `combinator` is the gap that
/// combinator covers, `None` for the first compound in a chain.
fn write_synth_relative_selector(
    w: &mut Sink<'_>,
    combinator: Option<Span>,
    span: Span,
    ctx: &Ctx<'_>,
    emit_simple: impl FnOnce(&mut Sink<'_>, &Ctx<'_>),
) {
    raw!(w; open RelativeSelector key RelativeSelector.combinator);
    match combinator {
        // Descendant is the only combinator a synthesized argument produces —
        // Svelte's `read_combinator` hands back exactly the whitespace run it ate.
        Some(gap) => write_combinator(w, " ", gap, ctx),
        None => null_record!(w),
    }
    raw!(w; key RelativeSelector.selectors lopen);
    emit_simple(w, ctx);
    raw!(w; lclose key RelativeSelector.start);
    w.span_start_end(ctx.positions, span.start, span.end);
    if ctx.has_metadata {
        raw!(w; relative_meta);
    }
    raw!(w; close);
}

tape! {
    /// The standalone `StyleSheetFile` TAPE of `parsed`, written into `sink` — the same walk
    /// as `write_stylesheet_file_bytes` on the span-only wire, in the coordinates
    /// `crate::WIRE_COORDINATES` states. Finish it with `sink.finish_words()` /
    /// `sink.finish_bytes()`.
    pub(crate) fn write_stylesheet_file_tape(
        parsed: ast::CssParsedRef<'_>,
        source: &str,
        sink: &mut tsv_lang::tape::TapeSink,
    ) {
        let tables = WireTables::new(source, crate::WIRE_COORDINATES, Wire::Span);
        let ctx = Ctx {
            source,
            view: parsed.view,
            positions: tables.positions(),
            has_metadata: true,
        };
        let mut w = sink.writer(source, ctx.positions, crate::WIRE_COORDINATES.bom);
        write_stylesheet_file(&mut w, parsed.stylesheet, &ctx);
    }
}
