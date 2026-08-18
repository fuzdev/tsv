//! Writer-mode conversion: emit compact wire JSON directly from the internal
//! CSS AST.
//!
//! The CSS sibling of `tsv_ts`'s `ast/convert/write/` — the **sole emission
//! path** for the CSS wire JSON. It walks the *internal* AST once and writes the
//! final JSON bytes as it goes, never materializing a typed public tree — the
//! hot path behind `convert_ast_json_bytes` (FFI/CLI compact output) and the
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
//! CSS public nodes carry only `start`/`end` (no `loc`/columns), so there is no
//! `LocationTracker`: each position is translated independently via a
//! `ByteToCharMap` (identity on ASCII). Dynamic strings delegate to
//! `serde_json` (via `JsonWriter::string`); static structure/tokens are written
//! verbatim; integers are hand-formatted.
//!
//! Node-header prefixes are single pre-fused `w.raw` literals per site,
//! deliberately NOT extracted into a shared `open_node` helper: the helper —
//! even `#[inline]` taking the pre-fused prefix — shifted fat-LTO inlining
//! across the crate (`write_block`/`write_atrule` de-inlined) and measured
//! +0.45% instructions on the CSS parse-JSON path. CSS nodes are small enough
//! that per-node call structure is visible; keep the literals inline.

use super::super::internal;
use super::{
    WireComment, convert_prelude_to_string, pseudo_name_end, raw_selector_name, scan_to_terminator,
    selector_contains_invalid, split_declaration_svelte_compat, strip_css_comments_collecting,
};
use std::borrow::Cow;
use tsv_lang::{ByteToCharMap, JsonWriter, Span, write_array, write_or_null};

/// `parseCss()` constant metadata payloads — always the `Default` (all-`false`,
/// `null` unit) shapes, emitted only on standalone CSS (`Ctx::has_metadata`).
/// The `,"metadata":…` prefix folds the leading comma into the constant.
const RULE_META: &str = ",\"metadata\":{\"parent_rule\":null,\"has_local_selectors\":false,\"has_global_selectors\":false,\"is_global_block\":false}";
const COMPLEX_META: &str = ",\"metadata\":{\"rule\":null,\"is_global\":false,\"used\":false}";
const RELATIVE_META: &str =
    ",\"metadata\":{\"is_global\":false,\"is_global_like\":false,\"scoped\":false}";

/// The per-document environment every writer function shares.
#[derive(Clone, Copy)]
struct Ctx<'a> {
    source: &'a str,
    map: &'a ByteToCharMap,
    /// Whether to attach `parseCss()` `metadata` (standalone `.css`) or omit it
    /// (embedded `<style>`). Precomputed once — the two shapes are otherwise the
    /// same walk — so the per-node metadata sites are a bare bool test.
    has_metadata: bool,
}

impl Ctx<'_> {
    /// Byte offset → emitted (UTF-16 code unit) offset; identity on ASCII.
    #[inline]
    fn pos(&self, byte: u32) -> u32 {
        self.map.byte_to_char(byte)
    }
}

/// Convert the internal CSS nodes straight to standalone-`StyleSheetFile` wire
/// bytes — one AST walk, with byte→char offset translation fused in.
pub(crate) fn write_stylesheet_file_bytes(
    stylesheet: &internal::CssStyleSheet<'_>,
    source: &str,
) -> Vec<u8> {
    let map = ByteToCharMap::new(source);
    let ctx = Ctx {
        source,
        map: &map,
        has_metadata: true,
    };
    let mut w = JsonWriter::with_capacity(tsv_lang::estimated_json_capacity(source.len()));
    write_stylesheet_file(&mut w, stylesheet, &ctx);
    w.into_bytes()
}

/// Emit an embedded-`<style>` stylesheet's `children` array (no `metadata`) into
/// a caller-owned writer, handing back the `comments` run the same walk gathered
/// — the composition entry the Svelte writer uses for a `<style>` element, whose
/// wire puts those two arrays side by side. `map` must be built from the host
/// document (spans are in host-file coordinates).
pub fn write_css_children(
    w: &mut JsonWriter,
    stylesheet: &internal::CssStyleSheet<'_>,
    source: &str,
    map: &ByteToCharMap,
) -> CssComments {
    let ctx = Ctx {
        source,
        map,
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
    map: &ByteToCharMap,
) {
    let ctx = Ctx {
        source,
        map,
        has_metadata: false,
    };
    write_comments(w, comments, &ctx);
}

/// The standalone `StyleSheetFile` root: `type`, `start` (0), `end` (source
/// length), `children`, `comments`.
fn write_stylesheet_file(
    w: &mut JsonWriter,
    stylesheet: &internal::CssStyleSheet<'_>,
    ctx: &Ctx<'_>,
) {
    w.raw("{\"type\":\"StyleSheetFile\",\"start\":");
    w.u32(ctx.pos(0));
    w.raw(",\"end\":");
    w.u32(ctx.pos(ctx.source.len() as u32));
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
        w.string(interior.extract(ctx.source));
        w.raw(",\"start\":");
        w.u32(ctx.pos(c.span.start));
        w.raw(",\"end\":");
        w.u32(ctx.pos(c.span.end));
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
    w.raw(",\"start\":");
    w.u32(ctx.pos(rule.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(rule.span.end));
    if ctx.has_metadata {
        w.raw(RULE_META);
    }
    w.raw("}");
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
    w.u32(ctx.pos(atrule.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(atrule.span.end));
    w.raw(",\"name\":");
    w.string(atrule.name);
    w.raw(",\"prelude\":");
    let prelude = convert_prelude_to_string(&atrule.prelude, ctx.source);
    w.string(&prelude);
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
    w.raw("{\"type\":\"Block\",\"start\":");
    w.u32(ctx.pos(block_span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(block_span.end));
    w.raw(",\"children\":");
    write_array(
        w,
        children
            .iter()
            .filter(|c| !matches!(c, internal::CssBlockChild::Comment(_))),
        |w, c| write_block_child(w, c, ctx, comments),
    );
    w.raw("}");
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
    let end = scan_to_terminator(source, content_end as usize);
    let decl_source = &source[decl.span.start as usize..end];
    let colon = decl.colon_pos();
    let (property, value) = if decl.has_block_comment {
        // Rare: a block comment sits somewhere in the declaration — apply the
        // Svelte property/value split quirk.
        split_declaration_svelte_compat(decl_source, colon)
    } else {
        // Common: no comments anywhere → split at the recorded colon. No re-scans.
        (&decl_source[..colon], decl_source[colon + 1..].trim_start())
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
        Cow::Borrowed(split.value.trim_end())
    };

    w.raw("{\"type\":\"Declaration\",\"start\":");
    w.u32(ctx.pos(decl.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(split.end));
    w.raw(",\"property\":");
    w.string(split.property.trim_end());
    w.raw(",\"value\":");
    w.string(&value);
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
    w.raw("{\"type\":\"SelectorList\",\"start\":");
    w.u32(ctx.pos(sl.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(sl.span.end));
    w.raw(",\"children\":");
    write_array(
        w,
        sl.selectors
            .iter()
            .filter(|c| !filter_invalid || !selector_contains_invalid(c)),
        |w, c| write_complex_selector(w, c, ctx),
    );
    w.raw("}");
}

/// Emits a `ComplexSelector` node.
fn write_complex_selector(w: &mut JsonWriter, c: &internal::ComplexSelector<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"ComplexSelector\",\"start\":");
    w.u32(ctx.pos(c.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(c.span.end));
    w.raw(",\"children\":");
    write_array(w, c.children, |w, r| write_relative_selector(w, r, ctx));
    if ctx.has_metadata {
        w.raw(COMPLEX_META);
    }
    w.raw("}");
}

/// Emits a `RelativeSelector` node. `combinator` is `null` (no skip) when
/// absent; field order is `combinator`, `selectors`, `start`, `end`, `metadata`.
fn write_relative_selector(w: &mut JsonWriter, r: &internal::RelativeSelector<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"RelativeSelector\",\"combinator\":");
    match (&r.combinator, &r.combinator_span) {
        (Some(comb), Some(span)) => write_combinator(w, comb.as_str(), *span, ctx),
        _ => w.null(),
    }
    w.raw(",\"selectors\":");
    write_array(w, r.selectors, |w, s| write_simple_selector(w, s, ctx));
    w.raw(",\"start\":");
    w.u32(ctx.pos(r.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(r.span.end));
    if ctx.has_metadata {
        w.raw(RELATIVE_META);
    }
    w.raw("}");
}

fn write_combinator(w: &mut JsonWriter, name: &'static str, span: Span, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"Combinator\",\"name\":");
    w.token(name); // ` ` / `>` / `+` / `~` / `||` — escape-free
    w.raw(",\"start\":");
    w.u32(ctx.pos(span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(span.end));
    w.raw("}");
}

/// Emit a simple selector (type/universal/class/id/nesting/attribute/pseudo/percentage).
fn write_simple_selector(w: &mut JsonWriter, simple: &internal::SimpleSelector<'_>, ctx: &Ctx<'_>) {
    match simple {
        internal::SimpleSelector::Type { namespace, span } => {
            let name = if namespace.is_none() {
                raw_selector_name(ctx.source, *span, 0)
            } else {
                let raw = &ctx.source[span.start as usize..span.end as usize];
                let prefix = raw.find('|').map_or(0, |i| i + 1);
                raw_selector_name(ctx.source, *span, prefix)
            };
            write_named_selector(w, "TypeSelector", &name, *span, ctx);
        }
        internal::SimpleSelector::Universal { namespace: _, span } => {
            write_named_selector(w, "TypeSelector", "*", *span, ctx);
        }
        internal::SimpleSelector::Class { span } => {
            let name = raw_selector_name(ctx.source, *span, 1);
            write_named_selector(w, "ClassSelector", &name, *span, ctx);
        }
        internal::SimpleSelector::Id { span } => {
            let name = raw_selector_name(ctx.source, *span, 1);
            write_named_selector(w, "IdSelector", &name, *span, ctx);
        }
        internal::SimpleSelector::Nesting { span } => {
            write_named_selector(w, "NestingSelector", "&", *span, ctx);
        }
        internal::SimpleSelector::Attribute {
            namespace,
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
            let namespace = *namespace;
            w.raw("{\"type\":\"AttributeSelector\",\"start\":");
            w.u32(ctx.pos(span.start));
            w.raw(",\"end\":");
            w.u32(ctx.pos(span.end));
            w.raw(",\"name\":");
            w.string(&name);
            w.raw(",\"matcher\":");
            // `as_str()` is a static escape-free operator (`=`/`~=`/`|=`/…), like
            // the sibling `Combinator` name — skip the serde escape scan.
            write_or_null(w, matcher.as_ref(), |w, m| w.token(m.as_str()));
            w.raw(",\"value\":");
            write_or_null(w, value.as_ref(), |w, v| w.string(v));
            w.raw(",\"flags\":");
            write_or_null(w, flags.as_ref(), |w, f| w.string(f));
            if let Some(ns) = namespace {
                w.raw(",\"namespace\":");
                w.string(ns);
            }
            w.raw("}");
        }
        internal::SimpleSelector::PseudoClass { args, span } => {
            let name_span = Span {
                start: span.start,
                end: pseudo_name_end(ctx.source, *span, args.is_some()),
            };
            let name = raw_selector_name(ctx.source, name_span, 1);
            w.raw("{\"type\":\"PseudoClassSelector\",\"name\":");
            w.string(&name);
            w.raw(",\"args\":");
            write_or_null(w, args.as_ref(), |w, a| write_pseudo_args(w, a, ctx));
            w.raw(",\"start\":");
            w.u32(ctx.pos(span.start));
            w.raw(",\"end\":");
            w.u32(ctx.pos(span.end));
            w.raw("}");
        }
        internal::SimpleSelector::PseudoElement { args, span } => {
            let name_end = pseudo_name_end(ctx.source, *span, args.is_some());
            let name = raw_selector_name(
                ctx.source,
                Span {
                    start: span.start,
                    end: name_end,
                },
                2,
            );
            w.raw("{\"type\":\"PseudoElementSelector\",\"name\":");
            w.string(&name);
            w.raw(",\"start\":");
            w.u32(ctx.pos(span.start));
            w.raw(",\"end\":");
            w.u32(ctx.pos(span.end));
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
            w.string(&value_str);
            w.raw(",\"start\":");
            w.u32(ctx.pos(span.start));
            w.raw(",\"end\":");
            w.u32(ctx.pos(span.end));
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
            w.string(span.extract(ctx.source));
            w.raw(",\"start\":");
            w.u32(ctx.pos(span.start));
            w.raw(",\"end\":");
            w.u32(ctx.pos(span.end));
            w.raw("}");
        }
        // Forgiving-list `Invalid`s are filtered before convert (see
        // `write_selector_list_filtered`); the non-filtering path (rule preludes)
        // never contains them.
        #[allow(clippy::unreachable)]
        internal::SimpleSelector::Invalid { .. } => {
            unreachable!("Invalid selectors should be filtered in write_selector_list_filtered")
        }
    }
}

/// The shared `{type, name, start, end}` shape (Type/Universal/Class/Id/Nesting).
fn write_named_selector(
    w: &mut JsonWriter,
    node_type: &str,
    name: &str,
    span: Span,
    ctx: &Ctx<'_>,
) {
    w.raw("{\"type\":\"");
    w.raw(node_type);
    w.raw("\",\"name\":");
    w.string(name);
    w.raw(",\"start\":");
    w.u32(ctx.pos(span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(span.end));
    w.raw("}");
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
                w.string(value);
                w.raw(",\"start\":");
                w.u32(ctx.pos(public_span.start));
                w.raw(",\"end\":");
                w.u32(ctx.pos(public_span.end));
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
    #[allow(clippy::unreachable)]
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
                    write_named_selector(
                        w,
                        "TypeSelector",
                        &raw_selector_name(ctx.source, *span, 0),
                        *span,
                        ctx,
                    );
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
    let s = ctx.pos(span.start);
    let e = ctx.pos(span.end);
    w.raw("{\"type\":\"SelectorList\",\"start\":");
    w.u32(s);
    w.raw(",\"end\":");
    w.u32(e);
    w.raw(",\"children\":[{\"type\":\"ComplexSelector\",\"start\":");
    w.u32(s);
    w.raw(",\"end\":");
    w.u32(e);
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
    w.u32(ctx.pos(span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(span.end));
    if ctx.has_metadata {
        w.raw(RELATIVE_META);
    }
    w.raw("}");
}
