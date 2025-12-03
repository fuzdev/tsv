// Svelte directive conversions
//
// Converts internal directive nodes to public format.
// All directives share common structure: name, expression, modifiers.

use crate::ast::{internal, public};
use string_interner::DefaultStringInterner;
use tsv_lang::LocationTracker;

use super::{convert_attribute_value, convert_expression_tag, to_json_value};

pub(super) fn convert_on_directive(
    d: &internal::OnDirective,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::OnDirective {
    let expression = d
        .expression
        .as_ref()
        .map(|e| tsv_ts::ast::convert::convert_expression(e, source, loc, interner, 0));

    public::OnDirective {
        node_type: "OnDirective".to_string(),
        start: d.span.start,
        end: d.span.end,
        name: d.name.clone(),
        expression,
        modifiers: d.modifiers.clone(),
    }
}

pub(super) fn convert_bind_directive(
    d: &internal::BindDirective,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::BindDirective {
    let expression =
        tsv_ts::ast::convert::convert_expression(&d.expression, source, loc, interner, 0);

    public::BindDirective {
        node_type: "BindDirective".to_string(),
        start: d.span.start,
        end: d.span.end,
        name: d.name.clone(),
        expression,
        modifiers: d.modifiers.clone(),
    }
}

pub(super) fn convert_class_directive(
    d: &internal::ClassDirective,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::ClassDirective {
    let expression =
        tsv_ts::ast::convert::convert_expression(&d.expression, source, loc, interner, 0);

    public::ClassDirective {
        node_type: "ClassDirective".to_string(),
        start: d.span.start,
        end: d.span.end,
        name: d.name.clone(),
        expression,
        modifiers: d.modifiers.clone(),
    }
}

pub(super) fn convert_style_directive(
    d: &internal::StyleDirective,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::StyleDirective {
    // Convert value based on its type
    let value = match &d.value {
        internal::StyleDirectiveValue::True => serde_json::Value::Bool(true),
        internal::StyleDirectiveValue::ExpressionTag(tag) => {
            let expr_tag = convert_expression_tag(tag, source, loc, interner);
            to_json_value(&expr_tag)
        }
        internal::StyleDirectiveValue::Parts(parts) => {
            let converted: Vec<_> = parts
                .iter()
                .map(|p| convert_attribute_value(p, source, loc, interner))
                .collect();
            to_json_value(&converted)
        }
    };

    public::StyleDirective {
        node_type: "StyleDirective".to_string(),
        start: d.span.start,
        end: d.span.end,
        name: d.name.clone(),
        modifiers: d.modifiers.clone(),
        value,
    }
}

pub(super) fn convert_use_directive(
    d: &internal::UseDirective,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::UseDirective {
    let expression = d
        .expression
        .as_ref()
        .map(|e| tsv_ts::ast::convert::convert_expression(e, source, loc, interner, 0));

    public::UseDirective {
        node_type: "UseDirective".to_string(),
        start: d.span.start,
        end: d.span.end,
        name: d.name.clone(),
        expression,
        modifiers: d.modifiers.clone(),
    }
}

pub(super) fn convert_transition_directive(
    d: &internal::TransitionDirective,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::TransitionDirective {
    let expression = d
        .expression
        .as_ref()
        .map(|e| tsv_ts::ast::convert::convert_expression(e, source, loc, interner, 0));

    public::TransitionDirective {
        node_type: "TransitionDirective".to_string(),
        start: d.span.start,
        end: d.span.end,
        name: d.name.clone(),
        expression,
        modifiers: d.modifiers.clone(),
        intro: d.intro,
        outro: d.outro,
    }
}

pub(super) fn convert_animate_directive(
    d: &internal::AnimateDirective,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::AnimateDirective {
    let expression = d
        .expression
        .as_ref()
        .map(|e| tsv_ts::ast::convert::convert_expression(e, source, loc, interner, 0));

    public::AnimateDirective {
        node_type: "AnimateDirective".to_string(),
        start: d.span.start,
        end: d.span.end,
        name: d.name.clone(),
        expression,
        modifiers: d.modifiers.clone(),
    }
}

pub(super) fn convert_let_directive(
    d: &internal::LetDirective,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::LetDirective {
    let expression = d
        .expression
        .as_ref()
        .map(|e| tsv_ts::ast::convert::convert_expression(e, source, loc, interner, 0));

    public::LetDirective {
        node_type: "LetDirective".to_string(),
        start: d.span.start,
        end: d.span.end,
        name: d.name.clone(),
        expression,
        modifiers: d.modifiers.clone(),
    }
}
