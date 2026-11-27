//! The wire writer's JSON-copy helpers: the writer functions whose JSON form has no tape
//! counterpart worth sharing a body with — the staged node heads (an element's, an attribute's,
//! a text's), the name fields' fixed-width windows, a text's `raw` / `data` pair written as one
//! escape, a root comment's two position shapes, the `loc` wire's `name_loc`, and
//! `<svelte:options>`'s `customElement` data. The JSON mount (`mod.rs`) provides them to
//! `write.rs` by name, as the tape mount (`write_tape.rs`) provides its own; the shared body
//! calls each by that name, so neither copy carries the other's form.

use std::borrow::Cow;

use super::write::{Ctx, name_bytes};
use crate::ast;
use crate::ast::convert::special::find_option_values;
use tsv_lang::{Comment, JsonWriter, LocationMapper, Span};
use tsv_ts::__private::{CommentMode, write_expression_embedded};

/// An element's head — `{"type":"Component"` or `{"type":"RegularElement"`, `start`, `end`
/// (and `loc`) — as one staged burst, which ends before the `name` field (`write.rs`'s module
/// doc, Staged runs). The opening literal is whole per kind, so each arm's copy is one
/// constant's width.
#[expect(clippy::inline_always)]
#[inline(always)]
pub(super) fn write_element_header(w: &mut JsonWriter, elem: &ast::Element, ctx: &Ctx<'_>) {
    let mut run = w.stage_run();
    match elem.kind {
        ast::ElementKind::Component => run.raw("{\"type\":\"Component\",\"start\":"),
        ast::ElementKind::Html => run.raw("{\"type\":\"RegularElement\",\"start\":"),
    }
    run.u32(ctx.pos(elem.span.start));
    run.raw(",\"end\":");
    run.u32(ctx.pos(elem.span.end));
    run.flush();
    w.span_loc(ctx.positions, elem.span.start, elem.span.end);
}

/// An attribute's head — `{"type":"Attribute"`, `start`, `end` (and `loc`) — as one staged
/// burst, which ends before the `name` field.
#[expect(clippy::inline_always)]
#[inline(always)]
pub(super) fn write_attribute_header(w: &mut JsonWriter, attr: &ast::Attribute, ctx: &Ctx<'_>) {
    let mut run = w.stage_run();
    run.raw("{\"type\":\"Attribute\",\"start\":");
    run.u32(ctx.pos(attr.span.start));
    run.raw(",\"end\":");
    run.u32(ctx.pos(attr.span.end));
    run.flush();
    w.span_loc(ctx.positions, attr.span.start, attr.span.end);
}

/// A text's head — `{"type":"Text"`, `start`, `end` (and `loc`), `,"raw":` — as one staged
/// burst, which ends at the dynamic `raw`.
#[expect(clippy::inline_always)]
#[inline(always)]
pub(super) fn write_text_header(w: &mut JsonWriter, text: &ast::Text, ctx: &Ctx<'_>) {
    let mut run = w.stage_run();
    run.raw("{\"type\":\"Text\",\"start\":");
    run.u32(ctx.pos(text.span.start));
    run.raw(",\"end\":");
    run.u32(ctx.pos(text.span.end));
    if ctx.positions.has_locations() {
        run.flush();
        w.span_loc(ctx.positions, text.span.start, text.span.end);
        w.raw(",\"raw\":");
    } else {
        run.raw(",\"raw\":");
        run.flush();
    }
}

/// A root-level comment, emitted fused in final char space: `{type, value, start, end,
/// loc}`, the literal acorn's `onComment` wrapper builds (a `<script>` or
/// template-expression comment). Svelte's template reader builds an in-tag comment
/// `{type, start, end, value, loc}`; tsv's wire keeps the one order, and the
/// `from_template_reader` axis keys the one difference left — `character` in such a
/// comment's `loc`. tsv writes `loc` immediately after `end`, from the document's one
/// line table.
pub(super) fn write_root_comment(w: &mut JsonWriter, comment: &Comment, ctx: &Ctx<'_>) {
    let span = comment.span;
    w.raw("{\"type\":\"");
    w.raw(if comment.is_block { "Block" } else { "Line" });
    w.raw("\",\"value\":");
    w.string(&comment.wire_value(ctx.source));
    if comment.from_template_reader {
        w.raw(",\"start\":");
        w.start_end(ctx.pos(span.start), ctx.pos(span.end));
        if let Some(lines) = ctx.positions.lines() {
            // `character` in each point, the shape Svelte's own `locate-character`
            // positions take
            let (start, end) = lines.span_positions(span.start, span.end);
            w.loc_field::<true>(start, end);
        }
    } else {
        w.span_start_end_field(ctx.positions, span.start, span.end);
    }
    w.raw("}");
}

/// The `,"name_loc":` field and the shared `NameLocation` shape it holds:
/// `start`/`end` each `{line, column, character}` (all three, always) — the `loc`
/// object's character-bearing form, spelled once for every writer by
/// `StageRun::loc_object`. Char-space via one fused translation per endpoint. Only the
/// loc-bearing wire has one: the span-only wire every binding ships carries no
/// `name_loc`.
///
/// Emitted as one **staged run** (`JsonWriter::stage_run`), the field **key inside
/// it** rather than a `raw` at the call sites: every caller emits the same key, which
/// is also why the callers name this `_field` — it emits the key, so it may not be
/// spliced into a position where some other key precedes the object.
///
/// A name never crosses a line, so only its start is resolved to one
/// ([`LocationMapper::one_line_span_positions`]): the line table here is Svelte's,
/// where only `\n` starts a line, and every name span stops at whitespace, which
/// includes `\n` — an element or component name, an attribute name and a directive's
/// head (`on:click|once`) are each read to Svelte whitespace, `/`, `>` or `=` at the
/// latest, and a shorthand attribute's (`{x}`) is its identifier's span, which holds
/// no whitespace at all. So the end's line is the start's, which `loc_object` appends
/// again from the digits already in hand rather than converting it a second time.
pub(super) fn write_name_loc_field(w: &mut JsonWriter, span: Span, lines: LocationMapper<'_>) {
    let (start, end) = lines.one_line_span_positions(span.start, span.end);
    debug_assert_eq!(start.1.line, end.1.line, "a name span lies on one line");
    let mut run = w.stage_run();
    run.loc_object::<true>(",\"name_loc\":", start, end);
    run.flush();
}

/// The `,"name":` key a Svelte node's name follows.
const NAME_KEY: &[u8; 8] = b",\"name\":";

/// Emit an element's `,"name":` field.
///
/// A `RegularElement`'s name is escape-free by the grammar the parser admits it under, so
/// it is written as the key and the quoted slice in one fixed-width append
/// ([`JsonWriter::string_escape_free_led`]), with no escape scan. The element parser's name
/// gate (`is_valid_tag_name`) admits a name that is not component-shaped only as a valid
/// element name: an ASCII letter followed by ASCII letters and digits and a `-`-led
/// custom-element tail of `PCENChar`s, a `![a-zA-Z]+` declaration, or an ASCII
/// `prefix:local` name — and no character of those is a `"`, a `\` or a control. A
/// component name — a name with no `:` whose lead is uppercase or that holds a `.` — is read
/// to whitespace, `/` or `>`, so it CAN hold one (`<A\b/>`), and is scanned
/// ([`write_scanned_name_field`]).
///
/// `inline(never)`: one out-of-line copy of the window write, whose arms leave by tail
/// call, so it saves no register.
#[inline(never)]
pub(super) fn write_element_name_field(w: &mut JsonWriter, elem: &ast::Element, ctx: &Ctx<'_>) {
    let name = name_bytes(elem.name_span, ctx);
    match elem.kind {
        ast::ElementKind::Html => w.string_escape_free_led(NAME_KEY, name),
        ast::ElementKind::Component => write_scanned_name_field(w, name),
    }
}

/// A `,"name":` field for a name no grammar vouches for — an attribute's, a component's:
/// the escape scan, then the key and the quoted name as one fixed-width append when it
/// comes back clean ([`JsonWriter::string_led`]).
///
/// `inline(never)`: one shared copy of the short scan and the window write for its two
/// callers (the attribute and the component name); the word loop and the escaping arm leave
/// by tail call.
#[inline(never)]
pub(super) fn write_scanned_name_field(w: &mut JsonWriter, name: &[u8]) {
    w.string_led(NAME_KEY, name);
}

/// A `Text`'s `raw` value, `,"data":`, then its `data` value — the tail every
/// `raw`-first `Text` shape shares.
///
/// `data` is `raw` whenever the text holds no `&` (nearly every template text), and
/// then the two values are the same bytes: [`JsonWriter::string_pair`] escapes them
/// once and copies the escaped form. Only a text whose `data` is decoded escapes two
/// strings.
///
/// A text made only of collapsible whitespace — the indentation between two tags,
/// over half of all texts — takes [`JsonWriter::string_pair_whitespace`], which knows
/// every byte's escape: `[ \t\n\r]` holds no `&`, so its `data` is its `raw`.
///
/// A flag test ahead of one of two out-of-line bodies, so neither pays for
/// the other's registers: inlined here, either body took callee-saved registers that
/// were saved at entry, ahead of the test, on every text.
pub(super) fn write_raw_then_data(w: &mut JsonWriter, text: &ast::Text, ctx: &Ctx<'_>) {
    if text.is_collapsible_ws_only {
        write_whitespace_raw_then_data(w, text, ctx);
    } else {
        write_content_raw_then_data(w, text, ctx);
    }
}

/// [`write_raw_then_data`] for a text made only of collapsible whitespace: `raw` and
/// `data` as one [`JsonWriter::string_pair_whitespace`] window.
#[inline(never)]
fn write_whitespace_raw_then_data(w: &mut JsonWriter, text: &ast::Text, ctx: &Ctx<'_>) {
    // Bytes, not `raw`: a whitespace-only text is ASCII, so the `str` slice's
    // char-boundary checks buy nothing.
    w.string_pair_whitespace(&ctx.source.as_bytes()[text.raw_span.range()], b",\"data\":");
}

/// [`write_raw_then_data`] for a text that holds content: `raw` escaped once and copied
/// when `data` borrows it, two strings when `data` is decoded.
#[inline(never)]
fn write_content_raw_then_data(w: &mut JsonWriter, text: &ast::Text, ctx: &Ctx<'_>) {
    let raw = text.raw(ctx.source);
    // Asked inline rather than through `data()`: this is every `Text` with content the
    // writer emits.
    if text.data_is_raw(raw) {
        w.string_pair(raw, ",\"data\":");
    } else {
        w.string(raw);
        w.raw(",\"data\":");
        w.string(&text.data(ctx.source));
    }
}

/// Emit a leading comma before the second and later members of a JSON object/array being
/// hand-assembled: no-op on the first member, `,` thereafter (flips `first` to `false`).
fn json_comma(w: &mut JsonWriter, first: &mut bool) {
    if *first {
        *first = false;
    } else {
        w.raw(",");
    }
}

/// Emit `customElement.props` as Svelte's statically-*evaluated* plain object
/// (`read_options`, `1-parse/read/options.js`): `{ [name]: { reflect?, type?, attribute? } }`,
/// reading each nested string/boolean literal's value in source order. Not an AST node — no
/// positions.
fn write_custom_element_props(
    w: &mut JsonWriter,
    props_obj: &tsv_ts::ast::ObjectExpression,
    ctx: &Ctx<'_>,
) {
    use tsv_ts::ast::{Expression, ExpressionKind, LiteralValue, ObjectProperty};
    w.raw("{");
    let mut first_prop = true;
    for prop in ctx.view.obj_props(props_obj.properties) {
        let ObjectProperty::Property(p) = prop else {
            continue;
        };
        let (
            Expression {
                kind: ExpressionKind::Identifier(key),
                ..
            },
            Expression {
                kind: ExpressionKind::ObjectExpression(inner),
                ..
            },
        ) = (ctx.view.expr(p.key), ctx.view.expr(p.value))
        else {
            continue;
        };
        json_comma(w, &mut first_prop);
        w.string(key.name(ctx.source, ctx.view));
        w.raw(":{");
        let mut first_attr = true;
        for inner_prop in ctx.view.obj_props(inner.properties) {
            let ObjectProperty::Property(ip) = inner_prop else {
                continue;
            };
            let (
                Expression {
                    kind: ExpressionKind::Identifier(ikey),
                    ..
                },
                Expression {
                    kind: ExpressionKind::Literal(lit),
                    ..
                },
            ) = (ctx.view.expr(ip.key), ctx.view.expr(ip.value))
            else {
                continue;
            };
            let key_name = ikey.name(ctx.source, ctx.view);
            match &lit.value {
                LiteralValue::String(cooked) => {
                    json_comma(w, &mut first_attr);
                    w.string(key_name);
                    w.raw(":");
                    w.string(cooked.resolve(lit.span, ctx.source, ctx.view));
                }
                LiteralValue::Boolean(b) => {
                    json_comma(w, &mut first_attr);
                    w.string(key_name);
                    w.raw(":");
                    w.bool(*b);
                }
                _ => {}
            }
        }
        w.raw("}");
    }
    w.raw("}");
}

/// Emit the skip-if-none `customElement` option, mirroring Svelte's `read_options`
/// (`1-parse/read/options.js`). The first attribute value that is an object expression
/// (`{ tag, props, shadow, extend }`) or a plain string (`"tag-name"` → `{tag}`) produces the
/// field. Only those four recognized keys are extracted (first-wins on a duplicate, like Svelte's
/// `properties.find`), emitted in the fixed order `tag, props, shadow, extend` regardless of
/// source order — Svelte assembles `ce` in that order. `tag` is a string, `props` a
/// statically-evaluated plain object, `shadow` either the string `'open'`/`'none'` or the raw
/// `ObjectExpression` AST, and `extend` the raw expression AST (both via the shared expression
/// writer, so their offsets translate like any template `{expr}`).
pub(super) fn write_custom_element_field(
    w: &mut JsonWriter,
    attrs: &[ast::AttributeNode],
    ctx: &Ctx<'_>,
) {
    use tsv_ts::ast::{Expression, ExpressionKind, LiteralValue, ObjectProperty};
    let Some(values) = find_option_values(ctx.svelte, attrs, "customElement", ctx.source) else {
        return;
    };
    for v in values {
        // `customElement={{ tag: '…', props: {…}, shadow: …, extend: … }}`
        if let ast::AttributeValue::ExpressionTag(expr) = v
            && let ExpressionKind::ObjectExpression(obj) = &ctx.view.expr(expr.expression).kind
        {
            let mut tag: Option<&Expression> = None;
            let mut props: Option<&Expression> = None;
            let mut shadow: Option<&Expression> = None;
            let mut extend: Option<&Expression> = None;
            for prop in ctx.view.obj_props(obj.properties) {
                if let ObjectProperty::Property(p) = prop
                    && let ExpressionKind::Identifier(key) = &ctx.view.expr(p.key).kind
                {
                    let slot = match key.name(ctx.source, ctx.view) {
                        "tag" => &mut tag,
                        "props" => &mut props,
                        "shadow" => &mut shadow,
                        "extend" => &mut extend,
                        _ => continue,
                    };
                    if slot.is_none() {
                        *slot = Some(ctx.view.expr(p.value));
                    }
                }
            }

            w.raw(",\"customElement\":{");
            let mut first = true;
            // `tag`: the string-literal value.
            if let Some(Expression {
                kind: ExpressionKind::Literal(lit),
                ..
            }) = tag
                && let LiteralValue::String(cooked) = &lit.value
            {
                json_comma(w, &mut first);
                w.raw("\"tag\":");
                w.string(cooked.resolve(lit.span, ctx.source, ctx.view));
            }
            // `props`: statically-evaluated plain object.
            if let Some(Expression {
                kind: ExpressionKind::ObjectExpression(props_obj),
                ..
            }) = props
            {
                json_comma(w, &mut first);
                w.raw("\"props\":");
                write_custom_element_props(w, props_obj, ctx);
            }
            // `shadow`: the string `'open'`/`'none'`, or the raw `ObjectExpression` AST.
            match shadow {
                Some(Expression {
                    kind: ExpressionKind::Literal(lit),
                    ..
                }) => {
                    if let LiteralValue::String(cooked) = &lit.value {
                        json_comma(w, &mut first);
                        w.raw("\"shadow\":");
                        w.string(cooked.resolve(lit.span, ctx.source, ctx.view));
                    }
                }
                Some(
                    shadow_expr @ Expression {
                        kind: ExpressionKind::ObjectExpression(_),
                        ..
                    },
                ) => {
                    json_comma(w, &mut first);
                    w.raw("\"shadow\":");
                    write_expression_embedded(w, shadow_expr, ctx.embed(CommentMode::Off));
                }
                _ => {}
            }
            // `extend`: the raw expression AST.
            if let Some(extend_expr) = extend {
                json_comma(w, &mut first);
                w.raw("\"extend\":");
                write_expression_embedded(w, extend_expr, ctx.embed(CommentMode::Off));
            }
            w.raw("}");
            return;
        }
        // Plain text or string literal: `customElement="tag-name"` → `{tag}`.
        let tag_str = match v {
            ast::AttributeValue::Text(text) => Some(text.data(ctx.source)),
            ast::AttributeValue::ExpressionTag(expr) => {
                if let ExpressionKind::Literal(lit) = &ctx.view.expr(expr.expression).kind
                    && let LiteralValue::String(cooked) = &lit.value
                {
                    Some(Cow::Borrowed(
                        cooked.resolve(lit.span, ctx.source, ctx.view),
                    ))
                } else {
                    None
                }
            }
        };
        if let Some(tag) = tag_str {
            w.raw(",\"customElement\":{\"tag\":");
            w.string(&tag);
            w.raw("}");
            return;
        }
    }
}
