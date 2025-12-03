// TypeScript type conversions

use super::super::{internal, public};
use super::create_location;
use tsv_lang::LocationTracker;

pub(in crate::ast) fn convert_type_annotation(
    type_annotation: &internal::TSTypeAnnotation,
    loc: &LocationTracker,
    offset: usize,
) -> public::TSTypeAnnotation {
    public::TSTypeAnnotation {
        node_type: "TSTypeAnnotation".to_string(),
        start: type_annotation.span.start,
        end: type_annotation.span.end,
        loc: create_location(type_annotation.span, loc, offset),
        type_annotation: Box::new(convert_type(&type_annotation.type_annotation, loc, offset)),
    }
}

pub(in crate::ast) fn convert_type(
    ts_type: &internal::TSType,
    loc: &LocationTracker,
    offset: usize,
) -> public::TSType {
    match ts_type {
        internal::TSType::Keyword(kw) => convert_keyword_type(kw, loc, offset),
        internal::TSType::Literal(lit) => convert_literal_type(lit, loc, offset),
        internal::TSType::Array(arr) => public::TSType::TSArrayType(public::TSArrayType {
            node_type: "TSArrayType".to_string(),
            start: arr.span.start,
            end: arr.span.end,
            loc: create_location(arr.span, loc, offset),
            element_type: Box::new(convert_type(&arr.element_type, loc, offset)),
        }),
    }
}

fn convert_literal_type(
    lit: &internal::TSLiteralType,
    loc: &LocationTracker,
    offset: usize,
) -> public::TSType {
    match lit {
        internal::TSLiteralType::TemplateLiteral(template) => {
            public::TSType::TSLiteralType(public::TSLiteralType {
                node_type: "TSLiteralType".to_string(),
                start: template.span.start,
                end: template.span.end,
                loc: create_location(template.span, loc, offset),
                literal: public::TSLiteralTypeLiteral::TemplateLiteral(
                    convert_template_literal_type(template, loc, offset),
                ),
            })
        }
    }
}

fn convert_template_literal_type(
    template: &internal::TemplateLiteralType,
    loc: &LocationTracker,
    offset: usize,
) -> public::TemplateLiteralType {
    public::TemplateLiteralType {
        node_type: "TemplateLiteral".to_string(),
        start: template.span.start,
        end: template.span.end,
        loc: create_location(template.span, loc, offset),
        quasis: template
            .quasis
            .iter()
            .map(|q| convert_template_element_type(q, loc, offset))
            .collect(),
        expressions: template
            .types
            .iter()
            .map(|t| convert_type(t, loc, offset))
            .collect(),
    }
}

fn convert_template_element_type(
    elem: &internal::TemplateElement,
    loc: &LocationTracker,
    offset: usize,
) -> public::TemplateElement {
    public::TemplateElement {
        node_type: "TemplateElement".to_string(),
        start: elem.span.start,
        end: elem.span.end,
        loc: create_location(elem.span, loc, offset),
        value: public::TemplateElementValue {
            raw: elem.raw.clone(),
            cooked: elem.cooked.clone(),
        },
        tail: elem.tail,
    }
}

/// Convert internal TSKeywordType to the appropriate public type variant
fn convert_keyword_type(
    kw: &internal::TSKeywordType,
    loc: &LocationTracker,
    offset: usize,
) -> public::TSType {
    use internal::TSKeywordKind;

    // Helper macro to reduce boilerplate - creates the public type struct
    macro_rules! make_public {
        ($variant:ident) => {{
            public::TSType::$variant(public::$variant {
                node_type: kw.kind.node_type_name().to_string(),
                start: kw.span.start,
                end: kw.span.end,
                loc: create_location(kw.span, loc, offset),
            })
        }};
    }

    match kw.kind {
        TSKeywordKind::Number => make_public!(TSNumberKeyword),
        TSKeywordKind::String => make_public!(TSStringKeyword),
        TSKeywordKind::Boolean => make_public!(TSBooleanKeyword),
        TSKeywordKind::Any => make_public!(TSAnyKeyword),
        TSKeywordKind::Void => make_public!(TSVoidKeyword),
        TSKeywordKind::Undefined => make_public!(TSUndefinedKeyword),
        TSKeywordKind::Null => make_public!(TSNullKeyword),
        TSKeywordKind::Never => make_public!(TSNeverKeyword),
        TSKeywordKind::Unknown => make_public!(TSUnknownKeyword),
        TSKeywordKind::Object => make_public!(TSObjectKeyword),
        TSKeywordKind::Symbol => make_public!(TSSymbolKeyword),
        TSKeywordKind::BigInt => make_public!(TSBigIntKeyword),
    }
}
