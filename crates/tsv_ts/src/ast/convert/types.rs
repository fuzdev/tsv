// TypeScript type conversions

use super::super::{internal, public};
use super::declarations::convert_type_parameter;
use super::{convert_expression, create_location};
use internal::TSKeywordKind;
use string_interner::DefaultStringInterner;
use tsv_lang::{InfallibleResolve, LocationTracker};

pub(in crate::ast) fn convert_type_annotation(
    type_annotation: &internal::TSTypeAnnotation<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSTypeAnnotation {
    public::TSTypeAnnotation {
        node_type: "TSTypeAnnotation",
        start: type_annotation.span.start,
        end: type_annotation.span.end,
        loc: create_location(type_annotation.span, loc, offset),
        type_annotation: Box::new(convert_type(
            type_annotation.type_annotation,
            source,
            loc,
            interner,
            offset,
        )),
    }
}

pub(in crate::ast) fn convert_type(
    ts_type: &internal::TSType<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSType {
    match ts_type {
        internal::TSType::Keyword(kw) => convert_keyword_type(kw, loc, offset),
        internal::TSType::Literal(lit) => convert_literal_type(lit, source, loc, interner, offset),
        internal::TSType::Array(arr) => public::TSType::TSArrayType(public::TSArrayType {
            node_type: "TSArrayType",
            start: arr.span.start,
            end: arr.span.end,
            loc: create_location(arr.span, loc, offset),
            element_type: Box::new(convert_type(
                arr.element_type,
                source,
                loc,
                interner,
                offset,
            )),
        }),
        internal::TSType::Union(u) => public::TSType::TSUnionType(public::TSUnionType {
            node_type: "TSUnionType",
            start: u.span.start,
            end: u.span.end,
            loc: create_location(u.span, loc, offset),
            types: u
                .types
                .iter()
                .map(|t| convert_type(t, source, loc, interner, offset))
                .collect(),
        }),
        internal::TSType::Intersection(i) => {
            public::TSType::TSIntersectionType(public::TSIntersectionType {
                node_type: "TSIntersectionType",
                start: i.span.start,
                end: i.span.end,
                loc: create_location(i.span, loc, offset),
                types: i
                    .types
                    .iter()
                    .map(|t| convert_type(t, source, loc, interner, offset))
                    .collect(),
            })
        }
        internal::TSType::TypeReference(r) => {
            public::TSType::TSTypeReference(public::TSTypeReference {
                node_type: "TSTypeReference",
                start: r.span.start,
                end: r.span.end,
                loc: create_location(r.span, loc, offset),
                type_name: convert_entity_name(&r.type_name, loc, interner, offset),
                type_arguments: r.type_arguments.as_ref().map(|ta| {
                    convert_type_parameter_instantiation(ta, source, loc, interner, offset)
                }),
            })
        }
        internal::TSType::TypeLiteral(t) => public::TSType::TSTypeLiteral(public::TSTypeLiteral {
            node_type: "TSTypeLiteral",
            start: t.span.start,
            end: t.span.end,
            loc: create_location(t.span, loc, offset),
            members: t
                .members
                .iter()
                .map(|m| convert_type_element(m, source, loc, interner, offset))
                .collect(),
        }),
        internal::TSType::Function(f) => public::TSType::TSFunctionType(public::TSFunctionType {
            node_type: "TSFunctionType",
            start: f.span.start,
            end: f.span.end,
            loc: create_location(f.span, loc, offset),
            type_parameters: f.type_parameters.as_ref().map(|tp| {
                convert_type_parameter_declaration_simple(tp, source, loc, interner, offset)
            }),
            params: f
                .params
                .iter()
                .map(|p| convert_expression(p, source, loc, interner, offset))
                .collect(),
            return_type: Box::new(convert_type_annotation(
                &f.return_type,
                source,
                loc,
                interner,
                offset,
            )),
        }),
        internal::TSType::Constructor(c) => {
            public::TSType::TSConstructorType(public::TSConstructorType {
                node_type: "TSConstructorType",
                start: c.span.start,
                end: c.span.end,
                loc: create_location(c.span, loc, offset),
                abstract_: c.abstract_,
                type_parameters: c.type_parameters.as_ref().map(|tp| {
                    convert_type_parameter_declaration_simple(tp, source, loc, interner, offset)
                }),
                params: c
                    .params
                    .iter()
                    .map(|p| convert_expression(p, source, loc, interner, offset))
                    .collect(),
                return_type: Box::new(convert_type_annotation(
                    &c.return_type,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::TSType::Tuple(t) => public::TSType::TSTupleType(public::TSTupleType {
            node_type: "TSTupleType",
            start: t.span.start,
            end: t.span.end,
            loc: create_location(t.span, loc, offset),
            element_types: t
                .element_types
                .iter()
                .map(|e| convert_type(e, source, loc, interner, offset))
                .collect(),
        }),
        internal::TSType::Parenthesized(p) => {
            public::TSType::TSParenthesizedType(public::TSParenthesizedType {
                node_type: "TSParenthesizedType",
                start: p.span.start,
                end: p.span.end,
                loc: create_location(p.span, loc, offset),
                type_annotation: Box::new(convert_type(
                    p.type_annotation,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::TSType::TypePredicate(p) => {
            let name = interner
                .resolve_infallible(p.parameter_name.name)
                .to_string();
            let parameter_name = if name == "this" {
                public::TSTypePredicateParameterName::TSThisType(public::TSThisType {
                    node_type: "TSThisType",
                    start: p.parameter_name.span.start,
                    end: p.parameter_name.span.end,
                    loc: create_location(p.parameter_name.span, loc, offset),
                })
            } else {
                public::TSTypePredicateParameterName::Identifier(super::convert_identifier(
                    &p.parameter_name,
                    loc,
                    interner,
                    offset,
                ))
            };
            public::TSType::TSTypePredicate(public::TSTypePredicate {
                node_type: "TSTypePredicate",
                start: p.span.start,
                end: p.span.end,
                loc: create_location(p.span, loc, offset),
                parameter_name,
                type_annotation: p.type_annotation.as_ref().map(|t| {
                    Box::new(public::TSTypeAnnotation {
                        node_type: "TSTypeAnnotation",
                        start: t.span().start,
                        end: t.span().end,
                        loc: create_location(t.span(), loc, offset),
                        type_annotation: Box::new(convert_type(t, source, loc, interner, offset)),
                    })
                }),
                asserts: p.asserts,
            })
        }
        internal::TSType::Conditional(c) => {
            public::TSType::TSConditionalType(public::TSConditionalType {
                node_type: "TSConditionalType",
                start: c.span.start,
                end: c.span.end,
                loc: create_location(c.span, loc, offset),
                check_type: Box::new(convert_type(c.check_type, source, loc, interner, offset)),
                extends_type: Box::new(convert_type(c.extends_type, source, loc, interner, offset)),
                true_type: Box::new(convert_type(c.true_type, source, loc, interner, offset)),
                false_type: Box::new(convert_type(c.false_type, source, loc, interner, offset)),
            })
        }
        internal::TSType::Mapped(m) => public::TSType::TSMappedType(public::TSMappedType {
            node_type: "TSMappedType",
            start: m.span.start,
            end: m.span.end,
            loc: create_location(m.span, loc, offset),
            type_parameter: public::TSMappedTypeParameter {
                node_type: "TSTypeParameter",
                start: m.type_parameter.span.start,
                end: m.type_parameter.span.end,
                loc: create_location(m.type_parameter.span, loc, offset),
                name: interner
                    .resolve_infallible(m.type_parameter.name)
                    .to_string(),
                constraint: Some(Box::new(convert_type(
                    m.type_parameter.constraint,
                    source,
                    loc,
                    interner,
                    offset,
                ))),
            },
            name_type: m
                .name_type
                .as_ref()
                .map(|t| Box::new(convert_type(t, source, loc, interner, offset))),
            type_annotation: m
                .type_annotation
                .as_ref()
                .map(|t| Box::new(convert_type(t, source, loc, interner, offset))),
            readonly: m.readonly.map(convert_mapped_type_modifier),
            optional: m.optional.map(convert_mapped_type_modifier),
        }),
        internal::TSType::TypeOperator(o) => {
            public::TSType::TSTypeOperator(public::TSTypeOperator {
                node_type: "TSTypeOperator",
                start: o.span.start,
                end: o.span.end,
                loc: create_location(o.span, loc, offset),
                operator: o.operator.as_str().to_string(),
                type_annotation: Box::new(convert_type(
                    o.type_annotation,
                    source,
                    loc,
                    interner,
                    offset,
                )),
            })
        }
        internal::TSType::Import(i) => public::TSType::TSImportType(public::TSImportType {
            node_type: "TSImportType",
            start: i.span.start,
            end: i.span.end,
            loc: create_location(i.span, loc, offset),
            argument: super::convert_literal(&i.argument, source, loc, offset),
            options: i
                .options
                .as_ref()
                .map(|o| Box::new(convert_expression(o, source, loc, interner, offset))),
            qualifier: i
                .qualifier
                .as_ref()
                .map(|q| convert_entity_name(q, loc, interner, offset)),
            type_arguments: i
                .type_arguments
                .as_ref()
                .map(|ta| convert_type_parameter_instantiation(ta, source, loc, interner, offset)),
        }),
        internal::TSType::TypeQuery(q) => public::TSType::TSTypeQuery(public::TSTypeQuery {
            node_type: "TSTypeQuery",
            start: q.span.start,
            end: q.span.end,
            loc: create_location(q.span, loc, offset),
            expr_name: convert_type_query_expr_name(&q.expr_name, source, loc, interner, offset),
            type_arguments: q
                .type_arguments
                .as_ref()
                .map(|ta| convert_type_parameter_instantiation(ta, source, loc, interner, offset)),
        }),
        internal::TSType::IndexedAccess(i) => {
            public::TSType::TSIndexedAccessType(public::TSIndexedAccessType {
                node_type: "TSIndexedAccessType",
                start: i.span.start,
                end: i.span.end,
                loc: create_location(i.span, loc, offset),
                object_type: Box::new(convert_type(i.object_type, source, loc, interner, offset)),
                index_type: Box::new(convert_type(i.index_type, source, loc, interner, offset)),
            })
        }
        internal::TSType::Rest(r) => public::TSType::TSRestType(public::TSRestType {
            node_type: "TSRestType",
            start: r.span.start,
            end: r.span.end,
            loc: create_location(r.span, loc, offset),
            type_annotation: Box::new(convert_type(
                r.type_annotation,
                source,
                loc,
                interner,
                offset,
            )),
        }),
        internal::TSType::Optional(o) => public::TSType::TSOptionalType(public::TSOptionalType {
            node_type: "TSOptionalType",
            start: o.span.start,
            end: o.span.end,
            loc: create_location(o.span, loc, offset),
            type_annotation: Box::new(convert_type(
                o.type_annotation,
                source,
                loc,
                interner,
                offset,
            )),
        }),
        internal::TSType::NamedTupleMember(n) => {
            public::TSType::TSNamedTupleMember(public::TSNamedTupleMember {
                node_type: "TSNamedTupleMember",
                start: n.span.start,
                end: n.span.end,
                loc: create_location(n.span, loc, offset),
                label: super::convert_identifier(&n.label, loc, interner, offset),
                element_type: Box::new(convert_type(n.element_type, source, loc, interner, offset)),
                optional: n.optional,
            })
        }
        internal::TSType::Infer(i) => public::TSType::TSInferType(public::TSInferType {
            node_type: "TSInferType",
            start: i.span.start,
            end: i.span.end,
            loc: create_location(i.span, loc, offset),
            // `infer U` / `infer U extends C` — its `type_parameter` is a regular
            // type parameter (modifiers always absent, no default), so reuse the
            // shared converter rather than rebuilding it inline.
            type_parameter: convert_type_parameter(
                &i.type_parameter,
                source,
                loc,
                interner,
                offset,
            ),
        }),
        internal::TSType::ThisType(t) => public::TSType::TSThisType(public::TSThisType {
            node_type: "TSThisType",
            start: t.span.start,
            end: t.span.end,
            loc: create_location(t.span, loc, offset),
        }),
    }
}

fn convert_type_query_expr_name(
    expr_name: &internal::TSTypeQueryExprName<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSTypeQueryExprName {
    match expr_name {
        internal::TSTypeQueryExprName::EntityName(entity) => match entity {
            internal::TSEntityName::Identifier(id) => public::TSTypeQueryExprName::Identifier(
                super::convert_identifier(id, loc, interner, offset),
            ),
            internal::TSEntityName::QualifiedName(qn) => {
                public::TSTypeQueryExprName::QualifiedName(public::TSQualifiedName {
                    node_type: "TSQualifiedName",
                    start: qn.span.start,
                    end: qn.span.end,
                    loc: create_location(qn.span, loc, offset),
                    left: Box::new(convert_entity_name(&qn.left, loc, interner, offset)),
                    right: super::convert_identifier(&qn.right, loc, interner, offset),
                })
            }
        },
        internal::TSTypeQueryExprName::Import(i) => {
            public::TSTypeQueryExprName::Import(public::TSImportType {
                node_type: "TSImportType",
                start: i.span.start,
                end: i.span.end,
                loc: create_location(i.span, loc, offset),
                argument: super::convert_literal(&i.argument, source, loc, offset),
                options: i
                    .options
                    .as_ref()
                    .map(|o| Box::new(convert_expression(o, source, loc, interner, offset))),
                qualifier: i
                    .qualifier
                    .as_ref()
                    .map(|q| convert_entity_name(q, loc, interner, offset)),
                type_arguments: i.type_arguments.as_ref().map(|ta| {
                    convert_type_parameter_instantiation(ta, source, loc, interner, offset)
                }),
            })
        }
    }
}

fn convert_literal_type(
    lit: &internal::TSLiteralType<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSType {
    match lit {
        internal::TSLiteralType::TemplateLiteral(template) => {
            public::TSType::TSLiteralType(public::TSLiteralType {
                node_type: "TSLiteralType",
                start: template.span.start,
                end: template.span.end,
                loc: create_location(template.span, loc, offset),
                literal: public::TSLiteralTypeLiteral::TemplateLiteral(
                    convert_template_literal_type(template, source, loc, interner, offset),
                ),
            })
        }
        internal::TSLiteralType::String(literal)
        | internal::TSLiteralType::Number(literal)
        | internal::TSLiteralType::BigInt(literal) => {
            // `convert_literal` derives value/raw/bigint from the literal's variant.
            public::TSType::TSLiteralType(public::TSLiteralType {
                node_type: "TSLiteralType",
                start: literal.span.start,
                end: literal.span.end,
                loc: create_location(literal.span, loc, offset),
                literal: public::TSLiteralTypeLiteral::Literal(super::convert_literal(
                    literal, source, loc, offset,
                )),
            })
        }
        internal::TSLiteralType::UnaryExpression(unary) => {
            // Convert UnaryExpression for negative number types like `-1`
            // Get the argument literal (parser guarantees this is always a Literal)
            let internal::Expression::Literal(arg_lit) = unary.argument else {
                unreachable!(
                    "parser only creates TSLiteralType::UnaryExpression with Literal argument"
                )
            };
            public::TSType::TSLiteralType(public::TSLiteralType {
                node_type: "TSLiteralType",
                start: unary.span.start,
                end: unary.span.end,
                loc: create_location(unary.span, loc, offset),
                literal: public::TSLiteralTypeLiteral::UnaryExpression(public::UnaryExpression {
                    node_type: "UnaryExpression",
                    start: unary.span.start,
                    end: unary.span.end,
                    loc: create_location(unary.span, loc, offset),
                    operator: unary.operator.as_str().to_string(),
                    prefix: unary.prefix,
                    argument: Box::new(public::Expression::Literal(super::convert_literal(
                        arg_lit, source, loc, offset,
                    ))),
                }),
            })
        }
    }
}

fn convert_template_literal_type(
    template: &internal::TemplateLiteralType<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TemplateLiteralType {
    public::TemplateLiteralType {
        node_type: "TemplateLiteral",
        start: template.span.start,
        end: template.span.end,
        loc: create_location(template.span, loc, offset),
        quasis: template
            .quasis
            .iter()
            .map(|q| super::convert_template_element(q, source, loc, offset))
            .collect(),
        expressions: template
            .types
            .iter()
            .map(|t| convert_type(t, source, loc, interner, offset))
            .collect(),
    }
}

// Template element conversion reuses convert_template_element from patterns module

/// Convert internal TSKeywordType to the appropriate public type variant
fn convert_keyword_type(
    kw: &internal::TSKeywordType,
    loc: &LocationTracker,
    offset: usize,
) -> public::TSType {
    // Helper macro to reduce boilerplate - creates the public type struct
    macro_rules! make_public {
        ($variant:ident) => {{
            public::TSType::$variant(public::$variant {
                node_type: kw.kind.node_type_name(),
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
        // Boolean literal types: `true` and `false` as types → TSLiteralType with Literal
        TSKeywordKind::True | TSKeywordKind::False => {
            let is_true = matches!(kw.kind, TSKeywordKind::True);
            public::TSType::TSLiteralType(public::TSLiteralType {
                node_type: "TSLiteralType",
                start: kw.span.start,
                end: kw.span.end,
                loc: create_location(kw.span, loc, offset),
                literal: public::TSLiteralTypeLiteral::Literal(public::Literal {
                    node_type: "Literal",
                    start: kw.span.start,
                    end: kw.span.end,
                    loc: create_location(kw.span, loc, offset),
                    value: serde_json::Value::Bool(is_true),
                    raw: if is_true { "true" } else { "false" }.to_string(),
                    bigint: None,
                }),
            })
        }
    }
}

// Entity name conversion
pub(super) fn convert_entity_name(
    name: &internal::TSEntityName<'_>,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSEntityName {
    match name {
        internal::TSEntityName::Identifier(id) => {
            public::TSEntityName::Identifier(super::convert_identifier(id, loc, interner, offset))
        }
        internal::TSEntityName::QualifiedName(qn) => {
            public::TSEntityName::QualifiedName(public::TSQualifiedName {
                node_type: "TSQualifiedName",
                start: qn.span.start,
                end: qn.span.end,
                loc: create_location(qn.span, loc, offset),
                left: Box::new(convert_entity_name(&qn.left, loc, interner, offset)),
                right: super::convert_identifier(&qn.right, loc, interner, offset),
            })
        }
    }
}

fn convert_type_parameter_instantiation(
    params: &internal::TSTypeParameterInstantiation<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSTypeParameterInstantiation {
    public::TSTypeParameterInstantiation {
        node_type: "TSTypeParameterInstantiation",
        start: params.span.start,
        end: params.span.end,
        loc: create_location(params.span, loc, offset),
        params: params
            .params
            .iter()
            .map(|p| convert_type(p, source, loc, interner, offset))
            .collect(),
    }
}

/// Simplified version that passes through interner but uses placeholder identifier names
fn convert_type_parameter_declaration_simple(
    params: &internal::TSTypeParameterDeclaration<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSTypeParameterDeclaration {
    public::TSTypeParameterDeclaration {
        node_type: "TSTypeParameterDeclaration",
        start: params.span.start,
        end: params.span.end,
        loc: create_location(params.span, loc, offset),
        params: params
            .params
            .iter()
            .map(|p| convert_type_parameter(p, source, loc, interner, offset))
            .collect(),
        extra: params
            .trailing_comma
            .map(|pos| public::TSTypeParameterExtra {
                trailing_comma: pos + offset as u32,
            }),
    }
}

fn convert_type_element(
    elem: &internal::TSTypeElement<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSTypeElement {
    match elem {
        internal::TSTypeElement::PropertySignature(p) => {
            public::TSTypeElement::PropertySignature(public::TSPropertySignature {
                node_type: "TSPropertySignature",
                start: p.span.start,
                end: p.span.end,
                loc: create_location(p.span, loc, offset),
                key: convert_expression(&p.key, source, loc, interner, offset),
                computed: {
                    let is_new_key = matches!(&p.key, internal::Expression::Identifier(id)
                        if interner.resolve_infallible(id.name) == "new");
                    // acorn quirk: omits `computed` when key is `new` and not readonly
                    if !p.computed && !p.readonly && is_new_key {
                        None
                    } else {
                        Some(p.computed)
                    }
                },
                optional: p.optional,
                readonly: p.readonly,
                type_annotation: p
                    .type_annotation
                    .as_ref()
                    .map(|ta| convert_type_annotation(ta, source, loc, interner, offset)),
            })
        }
        internal::TSTypeElement::MethodSignature(m) => {
            let kind = match m.kind {
                internal::MethodKind::Get => Some("get".to_string()),
                internal::MethodKind::Set => Some("set".to_string()),
                _ => Some("method".to_string()),
            };
            public::TSTypeElement::MethodSignature(public::TSMethodSignature {
                node_type: "TSMethodSignature",
                start: m.span.start,
                end: m.span.end,
                loc: create_location(m.span, loc, offset),
                computed: m.computed,
                key: convert_expression(&m.key, source, loc, interner, offset),
                optional: m.optional,
                kind,
                type_parameters: m.type_parameters.as_ref().map(|tp| {
                    convert_type_parameter_declaration_simple(tp, source, loc, interner, offset)
                }),
                parameters: m
                    .params
                    .iter()
                    .map(|p| convert_expression(p, source, loc, interner, offset))
                    .collect(),
                return_type: m
                    .return_type
                    .as_ref()
                    .map(|rt| convert_type_annotation(rt, source, loc, interner, offset)),
            })
        }
        internal::TSTypeElement::CallSignature(c) => {
            public::TSTypeElement::CallSignature(public::TSCallSignatureDeclaration {
                node_type: "TSCallSignatureDeclaration",
                start: c.span.start,
                end: c.span.end,
                loc: create_location(c.span, loc, offset),
                type_parameters: c.type_parameters.as_ref().map(|tp| {
                    convert_type_parameter_declaration_simple(tp, source, loc, interner, offset)
                }),
                params: c
                    .params
                    .iter()
                    .map(|p| convert_expression(p, source, loc, interner, offset))
                    .collect(),
                return_type: c
                    .return_type
                    .as_ref()
                    .map(|rt| convert_type_annotation(rt, source, loc, interner, offset)),
            })
        }
        internal::TSTypeElement::ConstructSignature(c) => {
            public::TSTypeElement::ConstructSignature(public::TSConstructSignatureDeclaration {
                node_type: "TSConstructSignatureDeclaration",
                start: c.span.start,
                end: c.span.end,
                loc: create_location(c.span, loc, offset),
                type_parameters: c.type_parameters.as_ref().map(|tp| {
                    convert_type_parameter_declaration_simple(tp, source, loc, interner, offset)
                }),
                params: c
                    .params
                    .iter()
                    .map(|p| convert_expression(p, source, loc, interner, offset))
                    .collect(),
                return_type: c
                    .return_type
                    .as_ref()
                    .map(|rt| convert_type_annotation(rt, source, loc, interner, offset)),
            })
        }
        internal::TSTypeElement::IndexSignature(i) => {
            public::TSTypeElement::IndexSignature(public::TSIndexSignature {
                node_type: "TSIndexSignature",
                start: i.span.start,
                end: i.span.end,
                loc: create_location(i.span, loc, offset),
                parameters: i
                    .parameters
                    .iter()
                    .map(|p| public::Identifier {
                        node_type: "Identifier",
                        start: p.span.start,
                        end: p.span.end,
                        loc: create_location(p.span, loc, offset),
                        name: interner.resolve_infallible(p.name).to_string(),
                        optional: p.optional,
                        type_annotation: p
                            .type_annotation()
                            .map(|ta| convert_type_annotation(ta, source, loc, interner, offset)),
                        decorators: Vec::new(),
                    })
                    .collect(),
                type_annotation: convert_type_annotation(
                    &i.type_annotation,
                    source,
                    loc,
                    interner,
                    offset,
                ),
                is_static: i.is_static,
                readonly: i.readonly,
            })
        }
    }
}

// Interface declaration conversion
pub(in crate::ast) fn convert_interface_declaration(
    iface: &internal::TSInterfaceDeclaration<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSInterfaceDeclaration {
    public::TSInterfaceDeclaration {
        node_type: "TSInterfaceDeclaration",
        start: iface.span.start,
        end: iface.span.end,
        loc: create_location(iface.span, loc, offset),
        id: super::convert_identifier(&iface.id, loc, interner, offset),
        type_parameters: iface
            .type_parameters
            .as_ref()
            .map(|tp| convert_type_parameter_declaration_simple(tp, source, loc, interner, offset)),
        extends: iface
            .extends
            .iter()
            .map(|h| convert_interface_heritage(h, source, loc, interner, offset))
            .collect(),
        body: convert_interface_body(&iface.body, source, loc, interner, offset),
        declare: iface.declare,
    }
}

fn convert_interface_heritage(
    heritage: &internal::TSInterfaceHeritage<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSInterfaceHeritage {
    public::TSInterfaceHeritage {
        node_type: "TSExpressionWithTypeArguments",
        start: heritage.span.start,
        end: heritage.span.end,
        loc: create_location(heritage.span, loc, offset),
        expression: convert_entity_name(&heritage.expression, loc, interner, offset),
        type_parameters: heritage
            .type_arguments
            .as_ref()
            .map(|ta| convert_type_parameter_instantiation(ta, source, loc, interner, offset)),
    }
}

fn convert_interface_body(
    body: &internal::TSInterfaceBody<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSInterfaceBody {
    public::TSInterfaceBody {
        node_type: "TSInterfaceBody",
        start: body.span.start,
        end: body.span.end,
        loc: create_location(body.span, loc, offset),
        body: body
            .body
            .iter()
            .map(|m| convert_type_element(m, source, loc, interner, offset))
            .collect(),
    }
}

// Declare function conversion
pub(in crate::ast) fn convert_declare_function(
    func: &internal::TSDeclareFunction<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSDeclareFunction {
    public::TSDeclareFunction {
        node_type: "TSDeclareFunction",
        start: func.span.start,
        end: func.span.end,
        loc: create_location(func.span, loc, offset),
        declare: func.declare,
        id: super::convert_identifier(&func.id, loc, interner, offset),
        expression: false, // Always false for declarations
        generator: func.generator,
        is_async: func.r#async,
        type_parameters: func.type_parameters.as_ref().map(|tp| {
            super::declarations::convert_type_parameter_declaration(
                tp, source, loc, interner, offset,
            )
        }),
        params: func
            .params
            .iter()
            .map(|p| convert_expression(p, source, loc, interner, offset))
            .collect(),
        return_type: func
            .return_type
            .as_ref()
            .map(|rt| convert_type_annotation(rt, source, loc, interner, offset)),
    }
}

/// Convert TSEnumDeclaration to public AST
pub(in crate::ast) fn convert_enum_declaration(
    enum_decl: &internal::TSEnumDeclaration<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSEnumDeclaration {
    public::TSEnumDeclaration {
        node_type: "TSEnumDeclaration",
        start: enum_decl.span.start,
        end: enum_decl.span.end,
        loc: create_location(enum_decl.span, loc, offset),
        id: super::convert_identifier(&enum_decl.id, loc, interner, offset),
        members: enum_decl
            .members
            .iter()
            .map(|m| convert_enum_member(m, source, loc, interner, offset))
            .collect(),
        is_const: enum_decl.r#const,
        declare: enum_decl.declare,
    }
}

/// Convert a single TSEnumMember
fn convert_enum_member(
    member: &internal::TSEnumMember<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TSEnumMember {
    let id = match &member.id {
        internal::TSEnumMemberId::Identifier(id) => {
            public::TSEnumMemberId::Identifier(super::convert_identifier(id, loc, interner, offset))
        }
        internal::TSEnumMemberId::String(lit) => {
            public::TSEnumMemberId::Literal(super::convert_literal(lit, source, loc, offset))
        }
    };

    public::TSEnumMember {
        node_type: "TSEnumMember",
        start: member.span.start,
        end: member.span.end,
        loc: create_location(member.span, loc, offset),
        id,
        initializer: member
            .initializer
            .as_ref()
            .map(|expr| convert_expression(expr, source, loc, interner, offset)),
    }
}

/// Convert a mapped-type `+`/`-`/`true` modifier to its public form. Shared by
/// the `readonly` and `optional` fields of a mapped type.
fn convert_mapped_type_modifier(m: internal::TSMappedTypeModifier) -> public::TSMappedTypeModifier {
    match m {
        internal::TSMappedTypeModifier::True => public::TSMappedTypeModifier::True,
        internal::TSMappedTypeModifier::Plus => public::TSMappedTypeModifier::Plus,
        internal::TSMappedTypeModifier::Minus => public::TSMappedTypeModifier::Minus,
    }
}
