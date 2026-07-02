// Svelte attribute conversions
//
// Converts internal attribute nodes to public format:
// - Attribute: Regular HTML attributes
// - SpreadAttribute: {...spread}
// - AttachTag: [attach] directive (new in Svelte 5)
// - Various directives (delegated to directives.rs)

use crate::ast::{internal, public};
use std::borrow::Cow;
use string_interner::DefaultStringInterner;
use tsv_lang::{LocationMapper, LocationTracker};
use tsv_ts::ast::convert::convert_expression;
use tsv_ts::ast::public::name_cow;

use super::{
    convert_animate_directive, convert_bind_directive, convert_class_directive,
    convert_expression_tag, convert_let_directive, convert_on_directive, convert_style_directive,
    convert_transition_directive, convert_use_directive, span_to_name_loc,
};

pub(super) fn convert_attribute_node<'src>(
    node: &internal::AttributeNode<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::AttributeNode<'src> {
    match node {
        internal::AttributeNode::Attribute(attr) => {
            public::AttributeNode::Attribute(convert_attribute(attr, source, loc, interner))
        }
        internal::AttributeNode::SpreadAttribute(spread) => public::AttributeNode::SpreadAttribute(
            convert_spread_attribute(spread, source, loc, interner),
        ),
        internal::AttributeNode::AttachTag(tag) => {
            public::AttributeNode::AttachTag(convert_attach_tag(tag, source, loc, interner))
        }
        internal::AttributeNode::OnDirective(d) => {
            public::AttributeNode::OnDirective(convert_on_directive(d, source, loc, interner))
        }
        internal::AttributeNode::BindDirective(d) => {
            public::AttributeNode::BindDirective(convert_bind_directive(d, source, loc, interner))
        }
        internal::AttributeNode::ClassDirective(d) => {
            public::AttributeNode::ClassDirective(convert_class_directive(d, source, loc, interner))
        }
        internal::AttributeNode::StyleDirective(d) => {
            public::AttributeNode::StyleDirective(convert_style_directive(d, source, loc, interner))
        }
        internal::AttributeNode::UseDirective(d) => {
            public::AttributeNode::UseDirective(convert_use_directive(d, source, loc, interner))
        }
        internal::AttributeNode::TransitionDirective(d) => {
            public::AttributeNode::TransitionDirective(convert_transition_directive(
                d, source, loc, interner,
            ))
        }
        internal::AttributeNode::AnimateDirective(d) => public::AttributeNode::AnimateDirective(
            convert_animate_directive(d, source, loc, interner),
        ),
        internal::AttributeNode::LetDirective(d) => {
            public::AttributeNode::LetDirective(convert_let_directive(d, source, loc, interner))
        }
    }
}

fn convert_attach_tag<'src>(
    tag: &internal::AttachTag<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::AttachTag<'src> {
    let expression = convert_expression(
        &tag.expression,
        source,
        LocationMapper::identity(loc),
        interner,
    );

    public::AttachTag {
        node_type: "AttachTag",
        start: tag.span.start,
        end: tag.span.end,
        expression: expression.into(),
    }
}

fn convert_spread_attribute<'src>(
    spread: &internal::SpreadAttribute<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::SpreadAttribute<'src> {
    let expression = convert_expression(
        &spread.expression,
        source,
        LocationMapper::identity(loc),
        interner,
    );

    public::SpreadAttribute {
        node_type: "SpreadAttribute",
        start: spread.span.start,
        end: spread.span.end,
        expression: expression.into(),
    }
}

fn convert_attribute<'src>(
    attr: &internal::Attribute<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::Attribute<'src> {
    // Extract attribute name from interner, borrowing from source when possible
    let name = name_cow(attr.name_span, source, attr.name, interner);

    // Convert attribute value following Svelte's JSON format:
    // - Boolean attributes (no value): serialize as `true`
    // - Text attributes (contain Text nodes): serialize as array
    // - Pure expression (single ExpressionTag): serialize as object
    // - Multiple expressions: serialize as array
    let value = match &attr.value {
        None => public::AttributeValueField::True(true), // Boolean attribute
        Some(values) => {
            // Check if any value is Text (string content)
            let has_text = values
                .iter()
                .any(|v| matches!(v, internal::AttributeValue::Text(_)));

            // A quoted single expression (`value="{expr}"`) serializes as an array like
            // any quoted sequence; only the bare form (`value={expr}`) is a plain object.
            // The value region directly abuts the opening quote, so the byte before the
            // tag's `{` discriminates the two forms.
            let quoted = values.len() == 1
                && matches!(&values[0], internal::AttributeValue::ExpressionTag(tag)
                if matches!(
                    (tag.span.start as usize)
                        .checked_sub(1)
                        .and_then(|i| source.as_bytes().get(i)),
                    Some(b'"' | b'\'')
                ));

            if has_text || quoted {
                // Text content or quoted expression: always serialize as array
                let converted: Vec<_> = values
                    .iter()
                    .map(|v| convert_attribute_value(v, source, loc, interner))
                    .collect();
                public::AttributeValueField::Sequence(converted)
            } else if values.len() == 1 {
                // Single bare expression: serialize as object
                let mut converted = convert_attribute_value(&values[0], source, loc, interner);

                // Shorthand attributes ({name}): Svelte's parser creates the Identifier via
                // read_identifier() which includes `character` in loc. Detect shorthand by
                // checking if the ExpressionTag and its Identifier expression share the same span.
                if let public::AttributeValue::ExpressionTag(ref mut et) = converted
                    && let public::ExpressionIsland::Typed(ref mut expression) = et.expression
                    && let internal::AttributeValue::ExpressionTag(ref internal_tag) = values[0]
                    && let tsv_ts::ast::internal::Expression::Identifier(ref id) =
                        internal_tag.expression
                    && internal_tag.span == id.span
                {
                    expression.inject_loc_character();
                }

                public::AttributeValueField::Single(converted)
            } else {
                // Multiple expressions: serialize as array
                let converted: Vec<_> = values
                    .iter()
                    .map(|v| convert_attribute_value(v, source, loc, interner))
                    .collect();
                public::AttributeValueField::Sequence(converted)
            }
        }
    };

    public::Attribute {
        node_type: "Attribute",
        start: attr.span.start,
        end: attr.span.end,
        name,
        name_loc: span_to_name_loc(attr.name_span, loc),
        value,
    }
}

pub(super) fn convert_attribute_value<'src>(
    value: &internal::AttributeValue<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::AttributeValue<'src> {
    match value {
        internal::AttributeValue::Text(text) => {
            public::AttributeValue::Text(convert_attribute_text(text, source))
        }
        internal::AttributeValue::ExpressionTag(tag) => public::AttributeValue::ExpressionTag(
            convert_expression_tag(tag, source, loc, interner),
        ),
    }
}

fn convert_attribute_text<'src>(
    text: &internal::Text,
    source: &'src str,
) -> public::AttributeText<'src> {
    public::AttributeText {
        start: text.span.start,
        end: text.span.end,
        node_type: "Text",
        raw: Cow::Borrowed(text.raw(source)),
        data: text.data(source),
    }
}
