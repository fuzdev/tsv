//! Writer-mode conversion: emit compact wire JSON directly from the internal
//! CSS AST.
//!
//! The CSS sibling of `tsv_ts`'s `ast/convert/write/` — it walks the *internal*
//! AST once and writes the final JSON bytes as it goes, never materializing the
//! typed public tree (`ast::public`). The hot path behind
//! `convert_ast_json_bytes` (FFI/CLI compact output) and the entry the Svelte
//! writer composes for embedded `<style>` blocks.
//!
//! **Byte-identity contract**: every function here emits exactly the bytes
//! `serde_json::to_string` produces for the corresponding `convert_*` result —
//! same field order (the public struct's declaration order, including the
//! `AttributeSelector` `start`/`end`-before-`name` and `Rule`
//! `prelude`/`block`-before-`start`/`end` quirks), same `skip_serializing_if`
//! behavior (`metadata` on standalone CSS only; `namespace`/`Nth.selector`
//! skipped when absent), same `null`s for the non-skipped `Option`s
//! (`combinator`, `matcher`/`value`/`flags`, `PseudoClass.args`), same scalar
//! formatting. Each `write_*` mirrors its `convert_*` twin in the sibling module
//! and **reuses its raw-source reconstruction helpers** (`strip_css_comments`,
//! `split_declaration_svelte_compat`, `raw_selector_name`, …) so the Svelte scan
//! semantics are defined once; change them in lockstep.
//!
//! CSS public nodes carry only `start`/`end` (no `loc`/columns), so there is no
//! `LocationTracker`: each position is translated independently via a
//! `ByteToCharMap` (identity on ASCII). Dynamic strings delegate to
//! `serde_json` (via `JsonWriter::string`); static structure/tokens are written
//! verbatim; integers are hand-formatted.

use super::super::internal;
use super::AstScope;
use super::{
    convert_prelude_to_string, pseudo_name_end, raw_selector_name, scan_to_terminator,
    selector_contains_invalid, split_declaration_svelte_compat, strip_css_comments,
};
use tsv_lang::{ByteToCharMap, JsonWriter, Span, write_array};

/// `parseCss()` constant metadata payloads — always the `Default` (all-`false`,
/// `null` unit) shapes, emitted only on standalone CSS (`AstScope::Standalone`).
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
    scope: AstScope,
}

impl Ctx<'_> {
    /// Byte offset → emitted (UTF-16 code unit) offset; identity on ASCII.
    #[inline]
    fn pos(&self, byte: u32) -> u32 {
        self.map.byte_to_char(byte)
    }
}

/// Convert the internal CSS nodes straight to standalone-`StyleSheetFile` wire
/// bytes. The writer twin of `convert_stylesheet_file` + `serde_json::to_string`
/// (+ the multibyte translate walk): byte-identical output, one AST walk.
pub(crate) fn write_stylesheet_file_bytes(
    nodes: &[internal::CssNode<'_>],
    source: &str,
) -> Vec<u8> {
    let map = ByteToCharMap::new(source);
    let ctx = Ctx {
        source,
        map: &map,
        scope: AstScope::Standalone,
    };
    let mut w = JsonWriter::with_capacity(tsv_lang::estimated_json_capacity(source.len()));
    write_stylesheet_file(&mut w, nodes, &ctx);
    w.into_bytes()
}

/// Emit one embedded-`<style>` CSS node (`AstScope::Embedded`, no `metadata`)
/// into a caller-owned writer — the composition entry the Svelte writer uses for
/// a `<style>` element's `children`. `map` must be built from the host document
/// (spans are in host-file coordinates).
pub fn write_css_node(
    w: &mut JsonWriter,
    node: &internal::CssNode<'_>,
    source: &str,
    map: &ByteToCharMap,
) {
    let ctx = Ctx {
        source,
        map,
        scope: AstScope::Embedded,
    };
    write_node(w, node, &ctx);
}

/// Mirrors `convert_stylesheet_file`: `type`, `start` (0), `end` (source length),
/// `children`.
fn write_stylesheet_file(w: &mut JsonWriter, nodes: &[internal::CssNode<'_>], ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"StyleSheetFile\",\"start\":");
    w.u32(ctx.pos(0));
    w.raw(",\"end\":");
    w.u32(ctx.pos(ctx.source.len() as u32));
    w.raw(",\"children\":");
    write_array(w, nodes, |w, n| write_node(w, n, ctx));
    w.raw("}");
}

/// Mirrors `convert_node`.
fn write_node(w: &mut JsonWriter, node: &internal::CssNode<'_>, ctx: &Ctx<'_>) {
    match node {
        internal::CssNode::Rule(rule) => write_rule(w, rule, ctx),
        internal::CssNode::Atrule(atrule) => write_atrule(w, atrule, ctx),
    }
}

/// Mirrors `convert_rule`. Field order: `type`, `prelude`, `block`, `start`,
/// `end`, then `metadata` (standalone only).
fn write_rule(w: &mut JsonWriter, rule: &internal::CssRule<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"Rule\",\"prelude\":");
    write_selector_list(w, &rule.selector, ctx);
    w.raw(",\"block\":");
    write_block(w, rule.block_span, rule.declarations, ctx);
    w.raw(",\"start\":");
    w.u32(ctx.pos(rule.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(rule.span.end));
    if ctx.scope.has_metadata() {
        w.raw(RULE_META);
    }
    w.raw("}");
}

/// Mirrors `convert_atrule`. `Atrule` carries no `metadata`.
fn write_atrule(w: &mut JsonWriter, atrule: &internal::CssAtrule<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"Atrule\",\"name\":");
    w.string(atrule.name);
    w.raw(",\"prelude\":");
    let prelude = convert_prelude_to_string(&atrule.prelude, ctx.source);
    w.string(&prelude);
    w.raw(",\"block\":");
    match &atrule.block {
        Some(b) => write_block(w, b.span, b.children, ctx),
        None => w.null(),
    }
    w.raw(",\"start\":");
    w.u32(ctx.pos(atrule.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(atrule.span.end));
    w.raw("}");
}

/// Mirrors the `public::Block` construction (comments dropped, like
/// `convert_block_child`'s `None` on `Comment`).
fn write_block(
    w: &mut JsonWriter,
    block_span: Span,
    children: &[internal::CssBlockChild<'_>],
    ctx: &Ctx<'_>,
) {
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
        |w, c| write_block_child(w, c, ctx),
    );
    w.raw("}");
}

fn write_block_child(w: &mut JsonWriter, child: &internal::CssBlockChild<'_>, ctx: &Ctx<'_>) {
    match child {
        internal::CssBlockChild::Declaration(d) => write_declaration(w, d, ctx),
        internal::CssBlockChild::Rule(r) => write_rule(w, r, ctx),
        internal::CssBlockChild::Atrule(a) => write_atrule(w, a, ctx),
        // Comments are filtered out before this call (see `write_block`).
        internal::CssBlockChild::Comment(_) => {}
    }
}

/// Mirrors `convert_declaration`: `end` is the `;`/`}` terminator, `property`
/// the trimmed pre-colon text, `value` the post-colon source with block
/// comments stripped.
fn write_declaration(w: &mut JsonWriter, decl: &internal::CssDeclaration<'_>, ctx: &Ctx<'_>) {
    let content_end = decl
        .important_end
        .map_or(decl.span.end, |e| e.max(decl.span.end));
    let end = scan_to_terminator(ctx.source, content_end as usize);
    let decl_source = &ctx.source[decl.span.start as usize..end];
    let (property_source, value_source) = split_declaration_svelte_compat(decl_source);
    let value = strip_css_comments(value_source);

    w.raw("{\"type\":\"Declaration\",\"start\":");
    w.u32(ctx.pos(decl.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(end as u32));
    w.raw(",\"property\":");
    w.string(property_source.trim_end());
    w.raw(",\"value\":");
    w.string(&value);
    w.raw("}");
}

/// Mirrors `convert_selector_list` (rule preludes — parsed non-forgivingly, no
/// `Invalid`).
fn write_selector_list(w: &mut JsonWriter, sl: &internal::SelectorList<'_>, ctx: &Ctx<'_>) {
    write_selector_list_inner(w, sl, ctx, false);
}

/// Mirrors `convert_selector_list_filtered` (pseudo-class args — drops complex
/// selectors containing a forgiving-parse `Invalid`).
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

/// Mirrors `convert_complex_selector`.
fn write_complex_selector(w: &mut JsonWriter, c: &internal::ComplexSelector<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"ComplexSelector\",\"start\":");
    w.u32(ctx.pos(c.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(c.span.end));
    w.raw(",\"children\":");
    write_array(w, c.children, |w, r| write_relative_selector(w, r, ctx));
    if ctx.scope.has_metadata() {
        w.raw(COMPLEX_META);
    }
    w.raw("}");
}

/// Mirrors `convert_relative_selector`. `combinator` is `null` (no skip) when
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
    if ctx.scope.has_metadata() {
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

/// Mirrors `convert_simple_selector`.
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
            value,
            flags,
            span,
        } => {
            let name = raw_selector_name(ctx.source, *name_span, 0);
            let matcher = *matcher;
            let value = *value;
            let flags = *flags;
            let namespace = *namespace;
            w.raw("{\"type\":\"AttributeSelector\",\"start\":");
            w.u32(ctx.pos(span.start));
            w.raw(",\"end\":");
            w.u32(ctx.pos(span.end));
            w.raw(",\"name\":");
            w.string(&name);
            w.raw(",\"matcher\":");
            match matcher {
                Some(m) => w.string(m.as_str()),
                None => w.null(),
            }
            w.raw(",\"value\":");
            match value {
                Some(v) => w.string(v),
                None => w.null(),
            }
            w.raw(",\"flags\":");
            match flags {
                Some(f) => w.string(f),
                None => w.null(),
            }
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
            match args {
                Some(a) => write_pseudo_class_args(w, a, ctx),
                None => w.null(),
            }
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
            w.u32(ctx.pos(name_end)); // name only, excluding args (matches Svelte)
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

/// Mirrors `convert_pseudo_class_args`.
fn write_pseudo_class_args(
    w: &mut JsonWriter,
    args: &internal::PseudoClassArgs<'_>,
    ctx: &Ctx<'_>,
) {
    match args {
        internal::PseudoClassArgs::Nth {
            value,
            of_selector,
            span,
            value_span: _,
        } => {
            write_wrap_single_selector(w, *span, ctx, |w, ctx| {
                w.raw("{\"type\":\"Nth\",\"value\":");
                w.string(value);
                w.raw(",\"start\":");
                w.u32(ctx.pos(span.start));
                w.raw(",\"end\":");
                w.u32(ctx.pos(span.end));
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
        #[allow(clippy::unreachable)]
        internal::PseudoClassArgs::Slotted { .. } | internal::PseudoClassArgs::Part { .. } => {
            unreachable!("Pseudo-element args (Slotted/Part) never attach to a pseudo-class")
        }
    }
}

/// Mirrors `wrap_single_selector`: SelectorList → ComplexSelector →
/// RelativeSelector → `[<simple>]`, all sharing `span`. The inner simple selector
/// is emitted by `emit_simple`.
fn write_wrap_single_selector(
    w: &mut JsonWriter,
    span: Span,
    ctx: &Ctx<'_>,
    emit_simple: impl FnOnce(&mut JsonWriter, &Ctx<'_>),
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
    w.raw(",\"children\":[{\"type\":\"RelativeSelector\",\"combinator\":null,\"selectors\":[");
    emit_simple(w, ctx);
    w.raw("],\"start\":");
    w.u32(s);
    w.raw(",\"end\":");
    w.u32(e);
    if ctx.scope.has_metadata() {
        w.raw(RELATIVE_META);
    }
    w.raw("}]"); // close RelativeSelector, ComplexSelector.children
    if ctx.scope.has_metadata() {
        w.raw(COMPLEX_META);
    }
    w.raw("}]}"); // close ComplexSelector, SelectorList.children, SelectorList
}
