// Object, array, template, and pattern conversions

use super::super::{internal, public};
use super::{convert_expression, convert_type_annotation, create_location};
use string_interner::DefaultStringInterner;
use tsv_lang::{LocationTracker, Span};

pub(in crate::ast) fn convert_template_literal(
    template: &internal::TemplateLiteral,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TemplateLiteral {
    let _ = (source, interner); // Suppress unused warnings - available for future use
    public::TemplateLiteral {
        node_type: "TemplateLiteral".to_string(),
        start: template.span.start,
        end: template.span.end,
        loc: create_location(template.span, loc, offset),
        quasis: template
            .quasis
            .iter()
            .map(|q| convert_template_element(q, loc, offset))
            .collect(),
        expressions: template
            .expressions
            .iter()
            .map(|e| convert_expression(e, source, loc, interner, offset))
            .collect(),
    }
}

pub(in crate::ast::convert) fn convert_template_element(
    element: &internal::TemplateElement,
    loc: &LocationTracker,
    offset: usize,
) -> public::TemplateElement {
    // Acorn excludes delimiters from TemplateElement spans:
    // - start: skip opening ` or } (+1)
    // - end: skip closing ` (-1 if tail) or ${ (-2 if not tail)
    let adjusted_start = element.span.start + 1;
    let adjusted_end = if element.tail {
        element.span.end - 1
    } else {
        element.span.end - 2
    };
    let adjusted_span = Span::new(adjusted_start, adjusted_end);
    public::TemplateElement {
        node_type: "TemplateElement".to_string(),
        start: adjusted_start,
        end: adjusted_end,
        loc: create_location(adjusted_span, loc, offset),
        value: public::TemplateElementValue {
            raw: element.raw.clone(),
            cooked: element.cooked.clone(),
        },
        tail: element.tail,
    }
}

pub(in crate::ast) fn convert_object_pattern(
    obj: &internal::ObjectPattern,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ObjectPattern {
    public::ObjectPattern {
        node_type: "ObjectPattern".to_string(),
        start: obj.span.start,
        end: obj.span.end,
        loc: create_location(obj.span, loc, offset),
        properties: obj
            .properties
            .iter()
            .map(|p| convert_object_pattern_property(p, source, loc, interner, offset))
            .collect(),
        type_annotation: obj
            .type_annotation
            .as_ref()
            .map(|ta| convert_type_annotation(ta, source, loc, interner, offset)),
    }
}

fn convert_object_pattern_property(
    prop: &internal::ObjectPatternProperty,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ObjectPatternProperty {
    match prop {
        internal::ObjectPatternProperty::Property(p) => public::ObjectPatternProperty::Property(
            convert_property(p, source, loc, interner, offset),
        ),
        internal::ObjectPatternProperty::RestElement(r) => {
            public::ObjectPatternProperty::RestElement(public::RestElement {
                node_type: "RestElement".to_string(),
                start: r.span.start,
                end: r.span.end,
                loc: create_location(r.span, loc, offset),
                argument: Box::new(convert_expression(
                    &r.argument,
                    source,
                    loc,
                    interner,
                    offset,
                )),
                type_annotation: r
                    .type_annotation
                    .as_ref()
                    .map(|ta| convert_type_annotation(ta, source, loc, interner, offset)),
            })
        }
    }
}

// TODO: Support property decorators in conversion
// Convert decorator AST nodes from internal to public format
// Needed when internal::Property gains decorators field
pub(in crate::ast) fn convert_property(
    prop: &internal::Property,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::Property {
    // TODO: Handle PropertyKind enum when refactored
    // Currently: Direct field access (method, shorthand, computed)
    // After refactor: Match on PropertyKind to extract fields
    // Also needed: Support for Get/Set property kinds (change kind: String field)
    public::Property {
        node_type: "Property".to_string(),
        start: prop.span.start,
        end: prop.span.end,
        loc: create_location(prop.span, loc, offset),
        method: prop.method,
        shorthand: prop.shorthand,
        computed: prop.computed,
        key: Box::new(convert_expression(&prop.key, source, loc, interner, offset)),
        value: Box::new(convert_expression(
            &prop.value,
            source,
            loc,
            interner,
            offset,
        )),
        kind: prop.kind.as_str().to_string(),
    }
}
