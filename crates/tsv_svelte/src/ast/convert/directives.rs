// Svelte directive conversions
//
// Converts internal directive nodes to public format.
// All directive types carry a `modifiers` field populated from internal data.
// The five types without official modifier support (use/bind/class/animate/let)
// still preserve any `|mod` text verbatim, matching Svelte's permissive parser.

use crate::ast::{internal, public};
use std::borrow::Cow;
use string_interner::DefaultStringInterner;
use tsv_lang::{LocationMapper, LocationTracker, Span};
use tsv_ts::ast::convert::convert_expression;

use super::{convert_attribute_value, convert_expression_tag, span_to_name_loc, to_json_value};

/// A directive's name borrows straight from source: `name_span` covers exactly
/// the name, so no interner round-trip or borrow guard is needed.
fn directive_name<'src>(name_span: Span, source: &'src str) -> Cow<'src, str> {
    Cow::Borrowed(name_span.extract(source))
}

/// Directive modifiers stay owned: the internal AST stores them as arena
/// strings without spans, so there is no `'src` slice to borrow. They are rare
/// (only `|modifier` heads), so the allocation doesn't matter.
fn convert_modifiers(modifiers: &[&str]) -> Vec<String> {
    modifiers.iter().map(|m| (*m).to_string()).collect()
}

pub(super) fn convert_on_directive<'src>(
    d: &internal::OnDirective<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::OnDirective<'src> {
    let expression = d
        .expression
        .as_ref()
        .map(|e| convert_expression(e, source, LocationMapper::identity(loc), interner).into());

    public::OnDirective {
        start: d.span.start,
        end: d.span.end,
        node_type: "OnDirective",
        name: directive_name(d.name_span, source),
        name_loc: span_to_name_loc(d.head_span, loc),
        expression,
        modifiers: convert_modifiers(d.modifiers),
    }
}

pub(super) fn convert_bind_directive<'src>(
    d: &internal::BindDirective<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::BindDirective<'src> {
    let expression = convert_directive_expression(
        &d.expression,
        d.expression_tag_span.is_some(),
        source,
        loc,
        interner,
    );

    public::BindDirective {
        start: d.span.start,
        end: d.span.end,
        node_type: "BindDirective",
        name: directive_name(d.name_span, source),
        name_loc: span_to_name_loc(d.head_span, loc),
        expression,
        modifiers: convert_modifiers(d.modifiers),
    }
}

pub(super) fn convert_class_directive<'src>(
    d: &internal::ClassDirective<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::ClassDirective<'src> {
    let expression = convert_directive_expression(
        &d.expression,
        d.expression_tag_span.is_some(),
        source,
        loc,
        interner,
    );

    public::ClassDirective {
        start: d.span.start,
        end: d.span.end,
        node_type: "ClassDirective",
        name: directive_name(d.name_span, source),
        name_loc: span_to_name_loc(d.head_span, loc),
        expression,
        modifiers: convert_modifiers(d.modifiers),
    }
}

pub(super) fn convert_style_directive<'src>(
    d: &internal::StyleDirective<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::StyleDirective<'src> {
    // Convert value based on its type
    let value = match &d.value {
        internal::StyleDirectiveValue::True => public::AttributeValueField::True(true),
        internal::StyleDirectiveValue::ExpressionTag(tag) => {
            let expr_tag = convert_expression_tag(tag, source, loc, interner);
            // A quoted expression (`style:color="{expr}"`) serializes as an array like
            // any quoted sequence; only the bare form (`style:color={expr}`) is a plain
            // object. The byte before the tag's `{` discriminates (matching Svelte).
            let quoted = matches!(
                (tag.span.start as usize)
                    .checked_sub(1)
                    .and_then(|i| source.as_bytes().get(i)),
                Some(b'"' | b'\'')
            );
            let part = public::AttributeValue::ExpressionTag(expr_tag);
            if quoted {
                public::AttributeValueField::Sequence(vec![part])
            } else {
                public::AttributeValueField::Single(part)
            }
        }
        internal::StyleDirectiveValue::Parts(parts) => {
            let converted: Vec<_> = parts
                .iter()
                .map(|p| convert_attribute_value(p, source, loc, interner))
                .collect();
            public::AttributeValueField::Sequence(converted)
        }
    };

    public::StyleDirective {
        start: d.span.start,
        end: d.span.end,
        node_type: "StyleDirective",
        name: directive_name(d.name_span, source),
        name_loc: span_to_name_loc(d.head_span, loc),
        modifiers: convert_modifiers(d.modifiers),
        value,
    }
}

pub(super) fn convert_use_directive<'src>(
    d: &internal::UseDirective<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::UseDirective<'src> {
    let expression = d
        .expression
        .as_ref()
        .map(|e| convert_expression(e, source, LocationMapper::identity(loc), interner).into());

    public::UseDirective {
        start: d.span.start,
        end: d.span.end,
        node_type: "UseDirective",
        name: directive_name(d.name_span, source),
        name_loc: span_to_name_loc(d.head_span, loc),
        expression,
        modifiers: convert_modifiers(d.modifiers),
    }
}

pub(super) fn convert_transition_directive<'src>(
    d: &internal::TransitionDirective<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::TransitionDirective<'src> {
    let expression = d
        .expression
        .as_ref()
        .map(|e| convert_expression(e, source, LocationMapper::identity(loc), interner).into());

    public::TransitionDirective {
        start: d.span.start,
        end: d.span.end,
        node_type: "TransitionDirective",
        name: directive_name(d.name_span, source),
        name_loc: span_to_name_loc(d.head_span, loc),
        expression,
        modifiers: convert_modifiers(d.modifiers),
        intro: d.direction.has_intro(),
        outro: d.direction.has_outro(),
    }
}

pub(super) fn convert_animate_directive<'src>(
    d: &internal::AnimateDirective<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::AnimateDirective<'src> {
    let expression = d
        .expression
        .as_ref()
        .map(|e| convert_expression(e, source, LocationMapper::identity(loc), interner).into());

    public::AnimateDirective {
        start: d.span.start,
        end: d.span.end,
        node_type: "AnimateDirective",
        name: directive_name(d.name_span, source),
        name_loc: span_to_name_loc(d.head_span, loc),
        expression,
        modifiers: convert_modifiers(d.modifiers),
    }
}

pub(super) fn convert_let_directive<'src>(
    d: &internal::LetDirective<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::LetDirective<'src> {
    let expression = d
        .expression
        .as_ref()
        .map(|e| convert_expression(e, source, LocationMapper::identity(loc), interner).into());

    public::LetDirective {
        start: d.span.start,
        end: d.span.end,
        node_type: "LetDirective",
        name: directive_name(d.name_span, source),
        name_loc: span_to_name_loc(d.head_span, loc),
        expression,
        modifiers: convert_modifiers(d.modifiers),
    }
}

/// Convert a directive expression to JSON, handling shorthand vs explicit.
///
/// Shorthand directives (`bind:value`, `class:active`) produce a synthetic
/// Identifier without `loc` and with Svelte field ordering (`start, end, type, name`).
/// Explicit directives (`bind:value={a}`) use the normal acorn-style conversion.
fn convert_directive_expression(
    expr: &tsv_ts::ast::internal::Expression<'_>,
    has_expression_tag: bool,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> serde_json::Value {
    if has_expression_tag {
        // Explicit: normal conversion with loc
        let converted = convert_expression(expr, source, LocationMapper::identity(loc), interner);
        to_json_value(&converted)
    } else {
        // Shorthand: synthetic identifier without loc, Svelte field ordering.
        // The shorthand form (`bind:x` / `class:x`) is built with a synthetic
        // `make_shorthand_identifier`, so the expression is always an `Identifier`.
        #[allow(clippy::unreachable)] // shorthand directive expr is a synthetic Identifier
        let tsv_ts::ast::internal::Expression::Identifier(id) = expr else {
            unreachable!("shorthand directive expression is always an Identifier");
        };
        let name = interner.resolve(id.name).unwrap_or("").to_string();
        let mut map = serde_json::Map::new();
        map.insert(
            "start".into(),
            serde_json::Value::Number(id.span.start.into()),
        );
        map.insert("end".into(), serde_json::Value::Number(id.span.end.into()));
        map.insert(
            "type".into(),
            serde_json::Value::String("Identifier".into()),
        );
        map.insert("name".into(), serde_json::Value::String(name));
        serde_json::Value::Object(map)
    }
}
