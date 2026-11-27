//! The wire writer's tape copy: `write.rs` mounted a second time, writing the tape
//! (`docs/tape_format.md`) into a `tsv_lang::tape::TapeWriter` — `write.rs`'s module doc, §One
//! walk, two products. Here are what the copy needs that the JSON copy takes from elsewhere:
//! its `tape!` / `json_only!`, its list helpers, the embedded `tsv_ts` / `tsv_css` writers'
//! tape copies (so a document's `<script>`s, template expressions and `<style>` land in its one
//! tape), its own copies of the two helper modules the writer shares with nothing else
//! (`comment_attachment.rs`, `special.rs`), and its forms of the functions each copy writes its
//! own way (`write_json.rs` holds the JSON copy's).
//!
//! Most of those forms write a record's fields **positionally**: a field every record of the
//! entry carries (`start`, `end`, an element's `name`, a text's `data`) declares no key on the
//! tape, so its data call writes its words with no key pending.

use std::borrow::Cow;

use crate::ast;
use crate::ast::convert::write_macros::{assert_positional_field, assert_positional_span};
use tsv_lang::tape::Enc;
use tsv_lang::tape::{TAG_OBJECT, TapeWriter};
use tsv_lang::{Comment, LocationMapper, Span};
use tsv_ts::__private::CommentMode;
use write::{Ctx, name_bytes};

// The embedded writers' tape copies, under the names the body calls.
use tsv_css::__private::{
    write_css_children_tape as write_css_children, write_css_comments_tape as write_css_comments,
};
use tsv_ts::__private::{
    write_expression_embedded_tape as write_expression_embedded,
    write_identifier_expression_with_character_tape as write_identifier_expression_with_character,
    write_pattern_embedded_tape as write_pattern_embedded,
    write_program_embedded_tape as write_program_embedded,
    write_variable_declaration_embedded_tape as write_variable_declaration_embedded,
};

/// The tape copy compiles its tape code.
macro_rules! tape {
    ($($t:tt)*) => { $($t)* };
}

/// The tape copy drops the JSON copy's own code.
macro_rules! json_only {
    ($($t:tt)*) => {};
}

/// The tape copy's half of an expression each copy spells its own way.
macro_rules! copy_pick {
    ($json:expr, $tape:expr) => {
        $tape
    };
}

// The comment attach's island declarations and `<svelte:options>`' readers, mounted once per
// copy: shared, a helper both copies call was outlined out of the JSON copy once the tape
// copy called it too (`attach_script` out of `write_script`), as `tsv_css`'s shared helpers
// were.
#[path = "comment_attachment.rs"]
#[expect(
    clippy::duplicate_mod,
    reason = "a per-copy helper, mounted once per copy of the writer (module doc)"
)]
mod comment_attachment;
#[path = "special.rs"]
#[expect(
    clippy::duplicate_mod,
    reason = "a per-copy helper, mounted once per copy of the writer (module doc)"
)]
mod special;
use special::find_option_values;

#[path = "write.rs"]
#[expect(
    clippy::duplicate_mod,
    reason = "the writer's tape copy, mounted beside its JSON copy (`write.rs`'s module doc)"
)]
mod write;
pub(crate) use write::write_root_tape;

/// A list: its count, then its items — `tsv_lang::write_array` on the tape.
#[inline]
fn write_array<T>(
    w: &mut TapeWriter<'_>,
    items: impl IntoIterator<Item = T>,
    mut f: impl FnMut(&mut TapeWriter<'_>, T),
) {
    w.t_list_open();
    for item in items {
        f(w, item);
    }
    w.t_list_close();
}

/// An array of records in a tagged-value field (an attribute's text-and-tag sequence): the
/// array's tag and count, then each record behind its object tag.
#[inline]
fn write_value_array<T>(
    w: &mut TapeWriter<'_>,
    items: impl IntoIterator<Item = T>,
    mut f: impl FnMut(&mut TapeWriter<'_>, T),
) {
    w.t_value_list_open();
    for item in items {
        w.t_tag(TAG_OBJECT);
        f(w, item);
    }
    w.t_list_close();
}

/// A nullable child: the item through `f`, or the `null` its field holds — `tsv_lang::
/// write_or_null` on the tape.
#[inline]
fn write_or_null<T>(
    w: &mut TapeWriter<'_>,
    item: Option<&T>,
    f: impl FnOnce(&mut TapeWriter<'_>, &T),
) {
    match item {
        Some(v) => f(w, v),
        None => w.null(),
    }
}

/// An element's head: its record (`Component` or `RegularElement`) and its offsets. Returns
/// the entry, which the element's key literals name.
#[inline]
fn write_element_header(w: &mut TapeWriter<'_>, elem: &ast::Element, ctx: &Ctx<'_>) -> u16 {
    const {
        assert_positional_span(entry!("Component"));
        assert_positional_span(entry!("RegularElement"));
    };
    let e = match elem.kind {
        ast::ElementKind::Component => entry!("Component"),
        ast::ElementKind::Html => entry!("RegularElement"),
    };
    w.t_open(e);
    w.span_start_end(ctx.positions, elem.span.start, elem.span.end);
    e
}

/// An attribute's head: its record and its offsets.
#[inline]
fn write_attribute_header(w: &mut TapeWriter<'_>, attr: &ast::Attribute, ctx: &Ctx<'_>) {
    const { assert_positional_span(entry!("Attribute")) };
    w.t_open(entry!("Attribute"));
    w.span_start_end(ctx.positions, attr.span.start, attr.span.end);
}

/// A text's head: its record and its offsets (its `raw` is the record's own text, which the
/// tape carries as no words).
#[inline]
fn write_text_header(w: &mut TapeWriter<'_>, text: &ast::Text, ctx: &Ctx<'_>) {
    const { assert_positional_span(entry!("Text")) };
    w.t_open(entry!("Text"));
    w.span_start_end(ctx.positions, text.span.start, text.span.end);
}

/// An element's `name`: the source slice it occupies.
#[inline]
fn write_element_name_field(w: &mut TapeWriter<'_>, elem: &ast::Element, ctx: &Ctx<'_>) {
    const {
        assert_positional_field(entry!("Component"), "name", Enc::Ref);
        assert_positional_field(entry!("RegularElement"), "name", Enc::Ref);
    };
    w.t_ref(name_bytes(elem.name_span, ctx));
}

/// An attribute's or a component's `name`: the source slice it occupies.
#[inline]
fn write_scanned_name_field(w: &mut TapeWriter<'_>, name: &[u8]) {
    const {
        assert_positional_field(entry!("Attribute"), "name", Enc::Ref);
        assert_positional_field(entry!("Component"), "name", Enc::Ref);
    };
    w.t_ref(name);
}

/// `name_loc` is the loc wire's alone, which no tape is written from.
#[inline]
fn write_name_loc_field(_: &mut TapeWriter<'_>, _: Span, _: LocationMapper<'_>) {}

/// A text's `data` (its `raw` is the record's own text): the source slice when the decode is
/// identity — a whitespace-only text, or one holding no reference — else the decoded text,
/// pooled.
#[inline]
fn write_raw_then_data(w: &mut TapeWriter<'_>, text: &ast::Text, ctx: &Ctx<'_>) {
    const {
        assert_positional_field(entry!("Text"), "raw", Enc::Own);
        assert_positional_field(entry!("Text"), "data", Enc::Ref);
    };
    if text.is_collapsible_ws_only {
        w.t_ref(&ctx.source.as_bytes()[text.raw_span.range()]);
        return;
    }
    let raw = text.raw(ctx.source);
    if text.data_is_raw(raw) {
        w.t_ref(raw.as_bytes());
    } else {
        w.t_ref(text.data(ctx.source).as_bytes());
    }
}

/// A root-level comment: a `Block` or `Line` record, its `value` and offsets.
fn write_root_comment(w: &mut TapeWriter<'_>, comment: &Comment, ctx: &Ctx<'_>) {
    const {
        assert_positional_field(entry!("Block"), "value", Enc::Ref);
        assert_positional_field(entry!("Line"), "value", Enc::Ref);
        assert_positional_span(entry!("Block"));
        assert_positional_span(entry!("Line"));
    };
    w.t_open(if comment.is_block {
        entry!("Block")
    } else {
        entry!("Line")
    });
    w.t_ref(comment.wire_value(ctx.source).as_bytes());
    w.span_start_end_field(ctx.positions, comment.span.start, comment.span.end);
    w.t_close();
}

/// A string literal's value, when `e` is one.
fn string_literal<'a>(e: Option<&'a tsv_ts::ast::Expression>, ctx: &Ctx<'a>) -> Option<&'a str> {
    use tsv_ts::ast::{ExpressionKind, LiteralValue};
    match &e?.kind {
        ExpressionKind::Literal(lit) => match &lit.value {
            LiteralValue::String(cooked) => Some(cooked.resolve(lit.span, ctx.source, ctx.view)),
            _ => None,
        },
        _ => None,
    }
}

/// One key of the open self-describing record and its string value.
fn string_member(w: &mut TapeWriter<'_>, key: &str, value: &str) {
    w.t_object_key(key.as_bytes());
    w.string(value);
}

/// `customElement.props`: the statically-evaluated plain object `write_json.rs`'s twin
/// writes, as self-describing records — the property names are the author's — written as the
/// walk finds them, each record's key count made by its own writes.
fn write_custom_element_props(
    w: &mut TapeWriter<'_>,
    props_obj: &tsv_ts::ast::ObjectExpression,
    ctx: &Ctx<'_>,
) {
    use tsv_ts::ast::{Expression, ExpressionKind, LiteralValue, ObjectProperty};
    w.t_object_open();
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
        w.t_object_key(key.name(ctx.source, ctx.view).as_bytes());
        w.t_object_open();
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
                    string_member(w, key_name, cooked.resolve(lit.span, ctx.source, ctx.view));
                }
                LiteralValue::Boolean(b) => {
                    w.t_object_key(key_name.as_bytes());
                    w.bool(*b);
                }
                _ => {}
            }
        }
        w.t_object_close();
    }
    w.t_object_close();
}

/// The skip-if-none `customElement` option — `write_json.rs`'s twin, its selection rules
/// restated — as a self-describing record in the tagged-value field.
fn write_custom_element_field(w: &mut TapeWriter<'_>, attrs: &[ast::AttributeNode], ctx: &Ctx<'_>) {
    use tsv_ts::ast::{Expression, ExpressionKind, LiteralValue, ObjectProperty};
    let Some(values) = find_option_values(ctx.svelte, attrs, "customElement", ctx.source) else {
        return;
    };
    for v in values {
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
            raw!(w, "~SvelteOptions", ",\"customElement\":");
            w.t_object_open();
            // `tag`: the string-literal value.
            if let Some(tag) = string_literal(tag, ctx) {
                string_member(w, "tag", tag);
            }
            // `props`: statically-evaluated plain object.
            if let Some(Expression {
                kind: ExpressionKind::ObjectExpression(props_obj),
                ..
            }) = props
            {
                w.t_object_key(b"props");
                write_custom_element_props(w, props_obj, ctx);
            }
            // `shadow`: the string `'open'`/`'none'`, or the raw `ObjectExpression` AST.
            match shadow {
                Some(Expression {
                    kind: ExpressionKind::Literal(_),
                    ..
                }) => {
                    if let Some(shadow) = string_literal(shadow, ctx) {
                        string_member(w, "shadow", shadow);
                    }
                }
                Some(
                    shadow_expr @ Expression {
                        kind: ExpressionKind::ObjectExpression(_),
                        ..
                    },
                ) => {
                    w.t_object_key(b"shadow");
                    write_expression_embedded(w, shadow_expr, ctx.embed(CommentMode::Off));
                }
                _ => {}
            }
            // `extend`: the raw expression AST.
            if let Some(extend_expr) = extend {
                w.t_object_key(b"extend");
                write_expression_embedded(w, extend_expr, ctx.embed(CommentMode::Off));
            }
            w.t_object_close();
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
            raw!(w, "~SvelteOptions", ",\"customElement\":");
            w.t_object_open();
            string_member(w, "tag", &tag);
            w.t_object_close();
            return;
        }
    }
}
