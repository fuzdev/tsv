// Svelte attribute conversions
//
// Converts internal attribute nodes to public format:
// - Attribute: Regular HTML attributes
// - SpreadAttribute: {...spread}
// - AttachTag: [attach] directive (new in Svelte 5)
// - Various directives (delegated to directives.rs)

use crate::ast::{internal, public};
use string_interner::DefaultStringInterner;
use tsv_lang::{InfallibleResolve, LocationTracker};

use super::{
    convert_animate_directive, convert_bind_directive, convert_class_directive,
    convert_expression_tag, convert_let_directive, convert_on_directive, convert_style_directive,
    convert_text, convert_transition_directive, convert_use_directive, to_json_value,
};

pub(super) fn convert_attribute_node(
    node: &internal::AttributeNode,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::AttributeNode {
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

fn convert_attach_tag(
    tag: &internal::AttachTag,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::AttachTag {
    let expression =
        tsv_ts::ast::convert::convert_expression(&tag.expression, source, loc, interner, 0);

    public::AttachTag {
        node_type: "AttachTag".to_string(),
        start: tag.span.start,
        end: tag.span.end,
        expression,
    }
}

fn convert_spread_attribute(
    spread: &internal::SpreadAttribute,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::SpreadAttribute {
    let expression =
        tsv_ts::ast::convert::convert_expression(&spread.expression, source, loc, interner, 0);

    public::SpreadAttribute {
        node_type: "SpreadAttribute".to_string(),
        start: spread.span.start,
        end: spread.span.end,
        expression,
    }
}

fn convert_attribute(
    attr: &internal::Attribute,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::Attribute {
    // Extract attribute name from interner
    let name = interner.resolve_infallible(attr.name).to_string();

    // Convert attribute value following Svelte's JSON format:
    // - Boolean attributes (no value): serialize as `true`
    // - Text attributes (contain Text nodes): serialize as array
    // - Pure expression (single ExpressionTag): serialize as object
    // - Multiple expressions: serialize as array
    let value = match &attr.value {
        None => Some(serde_json::Value::Bool(true)), // Boolean attribute
        Some(values) => {
            // Check if any value is Text (string content)
            let has_text = values
                .iter()
                .any(|v| matches!(v, internal::AttributeValue::Text(_)));

            if has_text {
                // Has text content: always serialize as array (even if single Text value)
                let converted: Vec<_> = values
                    .iter()
                    .map(|v| convert_attribute_value(v, source, loc, interner))
                    .collect();
                Some(to_json_value(&converted))
            } else if values.len() == 1 {
                // Single expression only: serialize as object
                let converted = convert_attribute_value(&values[0], source, loc, interner);
                Some(to_json_value(&converted))
            } else {
                // Multiple expressions: serialize as array
                let converted: Vec<_> = values
                    .iter()
                    .map(|v| convert_attribute_value(v, source, loc, interner))
                    .collect();
                Some(to_json_value(&converted))
            }
        }
    };

    public::Attribute {
        node_type: "Attribute".to_string(),
        start: attr.span.start,
        end: attr.span.end,
        name,
        value,
    }
}

pub(super) fn convert_attribute_value(
    value: &internal::AttributeValue,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::AttributeValue {
    match value {
        internal::AttributeValue::Text(text) => public::AttributeValue::Text(convert_text(text)),
        internal::AttributeValue::ExpressionTag(tag) => public::AttributeValue::ExpressionTag(
            convert_expression_tag(tag, source, loc, interner),
        ),
    }
}
