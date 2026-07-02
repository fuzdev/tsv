// Object, array, template, and pattern conversions

use super::super::{internal, public};
use super::{convert_expression, convert_type_annotation, create_location};
use std::borrow::Cow;
use string_interner::DefaultStringInterner;
use tsv_lang::{LocationMapper, Span};

pub(in crate::ast) fn convert_template_literal<'src>(
    template: &internal::TemplateLiteral<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TemplateLiteral<'src> {
    public::TemplateLiteral {
        node_type: "TemplateLiteral",
        start: loc.pos(template.span.start),
        end: loc.pos(template.span.end),
        loc: create_location(template.span, loc),
        quasis: template
            .quasis
            .iter()
            .map(|q| convert_template_element(q, source, loc))
            .collect(),
        expressions: template
            .expressions
            .iter()
            .map(|e| convert_expression(e, source, loc, interner))
            .collect(),
    }
}

pub(in crate::ast::convert) fn convert_template_element<'src>(
    element: &internal::TemplateElement<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
) -> public::TemplateElement<'src> {
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
    // `raw` is a verbatim source slice (borrowed). `cooked` borrows the same
    // slice for the no-escape (`Verbatim`) case; only a genuinely decoded value
    // owns, and an invalid escape yields `null`.
    let cooked = match element.cooked {
        internal::TemplateCooked::Verbatim => Some(Cow::Borrowed(element.raw(source))),
        internal::TemplateCooked::Decoded(decoded) => Some(Cow::Owned(decoded.to_string())),
        internal::TemplateCooked::Invalid => None,
    };
    public::TemplateElement {
        node_type: "TemplateElement",
        start: loc.pos(adjusted_start),
        end: loc.pos(adjusted_end),
        loc: create_location(adjusted_span, loc),
        value: public::TemplateElementValue {
            raw: Cow::Borrowed(element.raw(source)),
            cooked,
        },
        tail: element.tail,
    }
}

pub(in crate::ast) fn convert_object_pattern<'src>(
    obj: &internal::ObjectPattern<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::ObjectPattern<'src> {
    public::ObjectPattern {
        node_type: "ObjectPattern",
        start: loc.pos(obj.span.start),
        end: loc.pos(obj.span.end),
        loc: create_location(obj.span, loc),
        properties: obj
            .properties
            .iter()
            .map(|p| convert_object_pattern_property(p, source, loc, interner))
            .collect(),
        optional: obj.optional,
        type_annotation: obj
            .type_annotation
            .as_ref()
            .map(|ta| convert_type_annotation(ta, source, loc, interner)),
    }
}

fn convert_object_pattern_property<'src>(
    prop: &internal::ObjectPatternProperty<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::ObjectPatternProperty<'src> {
    match prop {
        internal::ObjectPatternProperty::Property(p) => {
            public::ObjectPatternProperty::Property(convert_property(p, source, loc, interner))
        }
        internal::ObjectPatternProperty::RestElement(r) => {
            public::ObjectPatternProperty::RestElement(convert_rest_element(
                r, source, loc, interner,
            ))
        }
    }
}

/// Convert an internal `RestElement` to its public node. Shared by the
/// object-pattern rest (`{...r}`) and the expression rest (`[...r]` / call rest).
pub(in crate::ast) fn convert_rest_element<'src>(
    rest: &internal::RestElement<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::RestElement<'src> {
    public::RestElement {
        node_type: "RestElement",
        start: loc.pos(rest.span.start),
        end: loc.pos(rest.span.end),
        loc: create_location(rest.span, loc),
        argument: Box::new(convert_expression(rest.argument, source, loc, interner)),
        type_annotation: rest
            .type_annotation
            .as_ref()
            .map(|ta| convert_type_annotation(ta, source, loc, interner)),
    }
}

// TODO: Support property decorators in conversion
// Convert decorator AST nodes from internal to public format
// Needed when internal::Property gains decorators field
pub(in crate::ast) fn convert_property<'src>(
    prop: &internal::Property<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::Property<'src> {
    // TODO: Handle PropertyKind enum when refactored
    // Currently: Direct field access (method, shorthand, computed)
    // After refactor: Match on PropertyKind to extract fields
    // Also needed: Support for Get/Set property kinds (change kind: String field)
    public::Property {
        node_type: "Property",
        start: loc.pos(prop.span.start),
        end: loc.pos(prop.span.end),
        loc: create_location(prop.span, loc),
        method: prop.method,
        shorthand: prop.shorthand,
        computed: prop.computed,
        key: Box::new(convert_expression(&prop.key, source, loc, interner)),
        value: Box::new(convert_expression(&prop.value, source, loc, interner)),
        kind: prop.kind.as_str(),
    }
}
