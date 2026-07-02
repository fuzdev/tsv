// TypeScript type conversions

use super::super::{internal, public};
use super::declarations::convert_type_parameter;
use super::{convert_expression, convert_identifier, convert_literal, create_location};
use internal::TSKeywordKind;
use std::borrow::Cow;
use string_interner::DefaultStringInterner;
use tsv_lang::{InfallibleResolve, LocationMapper};

pub(in crate::ast) fn convert_type_annotation<'src>(
    type_annotation: &internal::TSTypeAnnotation<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSTypeAnnotation<'src> {
    public::TSTypeAnnotation {
        node_type: "TSTypeAnnotation",
        start: loc.pos(type_annotation.span.start),
        end: loc.pos(type_annotation.span.end),
        loc: create_location(type_annotation.span, loc),
        type_annotation: Box::new(convert_type(
            type_annotation.type_annotation,
            source,
            loc,
            interner,
        )),
    }
}

pub(in crate::ast) fn convert_type<'src>(
    ts_type: &internal::TSType<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSType<'src> {
    match ts_type {
        internal::TSType::Keyword(kw) => convert_keyword_type(kw, loc),
        internal::TSType::Literal(lit) => convert_literal_type(lit, source, loc, interner),
        internal::TSType::Array(arr) => public::TSType::TSArrayType(public::TSArrayType {
            node_type: "TSArrayType",
            start: loc.pos(arr.span.start),
            end: loc.pos(arr.span.end),
            loc: create_location(arr.span, loc),
            element_type: Box::new(convert_type(arr.element_type, source, loc, interner)),
        }),
        internal::TSType::Union(u) => public::TSType::TSUnionType(public::TSUnionType {
            node_type: "TSUnionType",
            start: loc.pos(u.span.start),
            end: loc.pos(u.span.end),
            loc: create_location(u.span, loc),
            types: u
                .types
                .iter()
                .map(|t| convert_type(t, source, loc, interner))
                .collect(),
        }),
        internal::TSType::Intersection(i) => {
            public::TSType::TSIntersectionType(public::TSIntersectionType {
                node_type: "TSIntersectionType",
                start: loc.pos(i.span.start),
                end: loc.pos(i.span.end),
                loc: create_location(i.span, loc),
                types: i
                    .types
                    .iter()
                    .map(|t| convert_type(t, source, loc, interner))
                    .collect(),
            })
        }
        internal::TSType::TypeReference(r) => {
            public::TSType::TSTypeReference(public::TSTypeReference {
                node_type: "TSTypeReference",
                start: loc.pos(r.span.start),
                end: loc.pos(r.span.end),
                loc: create_location(r.span, loc),
                type_name: convert_entity_name(&r.type_name, source, loc, interner),
                type_arguments: r
                    .type_arguments
                    .as_ref()
                    .map(|ta| convert_type_parameter_instantiation(ta, source, loc, interner)),
            })
        }
        internal::TSType::TypeLiteral(t) => public::TSType::TSTypeLiteral(public::TSTypeLiteral {
            node_type: "TSTypeLiteral",
            start: loc.pos(t.span.start),
            end: loc.pos(t.span.end),
            loc: create_location(t.span, loc),
            members: t
                .members
                .iter()
                .map(|m| convert_type_element(m, source, loc, interner))
                .collect(),
        }),
        internal::TSType::Function(f) => public::TSType::TSFunctionType(public::TSFunctionType {
            node_type: "TSFunctionType",
            start: loc.pos(f.span.start),
            end: loc.pos(f.span.end),
            loc: create_location(f.span, loc),
            type_parameters: f
                .type_parameters
                .as_ref()
                .map(|tp| convert_type_parameter_declaration_simple(tp, source, loc, interner)),
            params: f
                .params
                .iter()
                .map(|p| convert_expression(p, source, loc, interner))
                .collect(),
            return_type: Box::new(convert_type_annotation(
                &f.return_type,
                source,
                loc,
                interner,
            )),
        }),
        internal::TSType::Constructor(c) => {
            public::TSType::TSConstructorType(public::TSConstructorType {
                node_type: "TSConstructorType",
                start: loc.pos(c.span.start),
                end: loc.pos(c.span.end),
                loc: create_location(c.span, loc),
                abstract_: c.abstract_,
                type_parameters: c
                    .type_parameters
                    .as_ref()
                    .map(|tp| convert_type_parameter_declaration_simple(tp, source, loc, interner)),
                params: c
                    .params
                    .iter()
                    .map(|p| convert_expression(p, source, loc, interner))
                    .collect(),
                return_type: Box::new(convert_type_annotation(
                    &c.return_type,
                    source,
                    loc,
                    interner,
                )),
            })
        }
        internal::TSType::Tuple(t) => public::TSType::TSTupleType(public::TSTupleType {
            node_type: "TSTupleType",
            start: loc.pos(t.span.start),
            end: loc.pos(t.span.end),
            loc: create_location(t.span, loc),
            element_types: t
                .element_types
                .iter()
                .map(|e| convert_type(e, source, loc, interner))
                .collect(),
        }),
        internal::TSType::Parenthesized(p) => {
            public::TSType::TSParenthesizedType(public::TSParenthesizedType {
                node_type: "TSParenthesizedType",
                start: loc.pos(p.span.start),
                end: loc.pos(p.span.end),
                loc: create_location(p.span, loc),
                type_annotation: Box::new(convert_type(p.type_annotation, source, loc, interner)),
            })
        }
        internal::TSType::TypePredicate(p) => {
            let name = interner.resolve_infallible(p.parameter_name.name);
            let parameter_name = if name == "this" {
                public::TSTypePredicateParameterName::TSThisType(public::TSThisType {
                    node_type: "TSThisType",
                    start: loc.pos(p.parameter_name.span.start),
                    end: loc.pos(p.parameter_name.span.end),
                    loc: create_location(p.parameter_name.span, loc),
                })
            } else {
                public::TSTypePredicateParameterName::Identifier(convert_identifier(
                    &p.parameter_name,
                    source,
                    loc,
                    interner,
                ))
            };
            public::TSType::TSTypePredicate(public::TSTypePredicate {
                node_type: "TSTypePredicate",
                start: loc.pos(p.span.start),
                end: loc.pos(p.span.end),
                loc: create_location(p.span, loc),
                parameter_name,
                type_annotation: p.type_annotation.as_ref().map(|t| {
                    Box::new(public::TSTypeAnnotation {
                        node_type: "TSTypeAnnotation",
                        start: loc.pos(t.span().start),
                        end: loc.pos(t.span().end),
                        loc: create_location(t.span(), loc),
                        type_annotation: Box::new(convert_type(t, source, loc, interner)),
                    })
                }),
                asserts: p.asserts,
            })
        }
        internal::TSType::Conditional(c) => {
            public::TSType::TSConditionalType(public::TSConditionalType {
                node_type: "TSConditionalType",
                start: loc.pos(c.span.start),
                end: loc.pos(c.span.end),
                loc: create_location(c.span, loc),
                check_type: Box::new(convert_type(c.check_type, source, loc, interner)),
                extends_type: Box::new(convert_type(c.extends_type, source, loc, interner)),
                true_type: Box::new(convert_type(c.true_type, source, loc, interner)),
                false_type: Box::new(convert_type(c.false_type, source, loc, interner)),
            })
        }
        internal::TSType::Mapped(m) => public::TSType::TSMappedType(public::TSMappedType {
            node_type: "TSMappedType",
            start: loc.pos(m.span.start),
            end: loc.pos(m.span.end),
            loc: create_location(m.span, loc),
            type_parameter: public::TSMappedTypeParameter {
                node_type: "TSTypeParameter",
                start: loc.pos(m.type_parameter.span.start),
                end: loc.pos(m.type_parameter.span.end),
                loc: create_location(m.type_parameter.span, loc),
                // Mapped-type parameter name is a bare symbol with no name-only
                // span (the struct span covers `K in C`), so it stays owned.
                name: Cow::Owned(
                    interner
                        .resolve_infallible(m.type_parameter.name)
                        .to_string(),
                ),
                constraint: Some(Box::new(convert_type(
                    m.type_parameter.constraint,
                    source,
                    loc,
                    interner,
                ))),
            },
            name_type: m
                .name_type
                .as_ref()
                .map(|t| Box::new(convert_type(t, source, loc, interner))),
            type_annotation: m
                .type_annotation
                .as_ref()
                .map(|t| Box::new(convert_type(t, source, loc, interner))),
            readonly: m.readonly.map(convert_mapped_type_modifier),
            optional: m.optional.map(convert_mapped_type_modifier),
        }),
        internal::TSType::TypeOperator(o) => {
            public::TSType::TSTypeOperator(public::TSTypeOperator {
                node_type: "TSTypeOperator",
                start: loc.pos(o.span.start),
                end: loc.pos(o.span.end),
                loc: create_location(o.span, loc),
                operator: o.operator.as_str(),
                type_annotation: Box::new(convert_type(o.type_annotation, source, loc, interner)),
            })
        }
        internal::TSType::Import(i) => public::TSType::TSImportType(public::TSImportType {
            node_type: "TSImportType",
            start: loc.pos(i.span.start),
            end: loc.pos(i.span.end),
            loc: create_location(i.span, loc),
            argument: convert_literal(&i.argument, source, loc),
            options: i
                .options
                .as_ref()
                .map(|o| Box::new(convert_expression(o, source, loc, interner))),
            qualifier: i
                .qualifier
                .as_ref()
                .map(|q| convert_entity_name(q, source, loc, interner)),
            type_arguments: i
                .type_arguments
                .as_ref()
                .map(|ta| convert_type_parameter_instantiation(ta, source, loc, interner)),
        }),
        internal::TSType::TypeQuery(q) => public::TSType::TSTypeQuery(public::TSTypeQuery {
            node_type: "TSTypeQuery",
            start: loc.pos(q.span.start),
            end: loc.pos(q.span.end),
            loc: create_location(q.span, loc),
            expr_name: convert_type_query_expr_name(&q.expr_name, source, loc, interner),
            type_arguments: q
                .type_arguments
                .as_ref()
                .map(|ta| convert_type_parameter_instantiation(ta, source, loc, interner)),
        }),
        internal::TSType::IndexedAccess(i) => {
            public::TSType::TSIndexedAccessType(public::TSIndexedAccessType {
                node_type: "TSIndexedAccessType",
                start: loc.pos(i.span.start),
                end: loc.pos(i.span.end),
                loc: create_location(i.span, loc),
                object_type: Box::new(convert_type(i.object_type, source, loc, interner)),
                index_type: Box::new(convert_type(i.index_type, source, loc, interner)),
            })
        }
        internal::TSType::Rest(r) => public::TSType::TSRestType(public::TSRestType {
            node_type: "TSRestType",
            start: loc.pos(r.span.start),
            end: loc.pos(r.span.end),
            loc: create_location(r.span, loc),
            type_annotation: Box::new(convert_type(r.type_annotation, source, loc, interner)),
        }),
        internal::TSType::Optional(o) => public::TSType::TSOptionalType(public::TSOptionalType {
            node_type: "TSOptionalType",
            start: loc.pos(o.span.start),
            end: loc.pos(o.span.end),
            loc: create_location(o.span, loc),
            type_annotation: Box::new(convert_type(o.type_annotation, source, loc, interner)),
        }),
        internal::TSType::NamedTupleMember(n) => {
            public::TSType::TSNamedTupleMember(public::TSNamedTupleMember {
                node_type: "TSNamedTupleMember",
                start: loc.pos(n.span.start),
                end: loc.pos(n.span.end),
                loc: create_location(n.span, loc),
                label: convert_identifier(&n.label, source, loc, interner),
                element_type: Box::new(convert_type(n.element_type, source, loc, interner)),
                optional: n.optional,
            })
        }
        internal::TSType::Infer(i) => public::TSType::TSInferType(public::TSInferType {
            node_type: "TSInferType",
            start: loc.pos(i.span.start),
            end: loc.pos(i.span.end),
            loc: create_location(i.span, loc),
            // `infer U` / `infer U extends C` — its `type_parameter` is a regular
            // type parameter (modifiers always absent, no default), so reuse the
            // shared converter rather than rebuilding it inline.
            type_parameter: convert_type_parameter(&i.type_parameter, source, loc, interner),
        }),
        internal::TSType::ThisType(t) => public::TSType::TSThisType(public::TSThisType {
            node_type: "TSThisType",
            start: loc.pos(t.span.start),
            end: loc.pos(t.span.end),
            loc: create_location(t.span, loc),
        }),
    }
}

fn convert_type_query_expr_name<'src>(
    expr_name: &internal::TSTypeQueryExprName<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSTypeQueryExprName<'src> {
    match expr_name {
        internal::TSTypeQueryExprName::EntityName(entity) => match entity {
            internal::TSEntityName::Identifier(id) => public::TSTypeQueryExprName::Identifier(
                convert_identifier(id, source, loc, interner),
            ),
            internal::TSEntityName::QualifiedName(qn) => {
                public::TSTypeQueryExprName::QualifiedName(public::TSQualifiedName {
                    node_type: "TSQualifiedName",
                    start: loc.pos(qn.span.start),
                    end: loc.pos(qn.span.end),
                    loc: create_location(qn.span, loc),
                    left: Box::new(convert_entity_name(&qn.left, source, loc, interner)),
                    right: convert_identifier(&qn.right, source, loc, interner),
                })
            }
        },
        internal::TSTypeQueryExprName::Import(i) => {
            public::TSTypeQueryExprName::Import(public::TSImportType {
                node_type: "TSImportType",
                start: loc.pos(i.span.start),
                end: loc.pos(i.span.end),
                loc: create_location(i.span, loc),
                argument: convert_literal(&i.argument, source, loc),
                options: i
                    .options
                    .as_ref()
                    .map(|o| Box::new(convert_expression(o, source, loc, interner))),
                qualifier: i
                    .qualifier
                    .as_ref()
                    .map(|q| convert_entity_name(q, source, loc, interner)),
                type_arguments: i
                    .type_arguments
                    .as_ref()
                    .map(|ta| convert_type_parameter_instantiation(ta, source, loc, interner)),
            })
        }
    }
}

fn convert_literal_type<'src>(
    lit: &internal::TSLiteralType<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSType<'src> {
    match lit {
        internal::TSLiteralType::TemplateLiteral(template) => {
            public::TSType::TSLiteralType(public::TSLiteralType {
                node_type: "TSLiteralType",
                start: loc.pos(template.span.start),
                end: loc.pos(template.span.end),
                loc: create_location(template.span, loc),
                literal: public::TSLiteralTypeLiteral::TemplateLiteral(
                    convert_template_literal_type(template, source, loc, interner),
                ),
            })
        }
        internal::TSLiteralType::String(literal)
        | internal::TSLiteralType::Number(literal)
        | internal::TSLiteralType::BigInt(literal) => {
            // `convert_literal` derives value/raw/bigint from the literal's variant.
            public::TSType::TSLiteralType(public::TSLiteralType {
                node_type: "TSLiteralType",
                start: loc.pos(literal.span.start),
                end: loc.pos(literal.span.end),
                loc: create_location(literal.span, loc),
                literal: public::TSLiteralTypeLiteral::Literal(convert_literal(
                    literal, source, loc,
                )),
            })
        }
        internal::TSLiteralType::UnaryExpression(unary) => {
            // Convert UnaryExpression for negative number types like `-1`
            // Get the argument literal (parser guarantees this is always a Literal)
            #[allow(clippy::unreachable)] // parser builds this variant only with a Literal argument
            let internal::Expression::Literal(arg_lit) = unary.argument else {
                unreachable!(
                    "parser only creates TSLiteralType::UnaryExpression with Literal argument"
                )
            };
            public::TSType::TSLiteralType(public::TSLiteralType {
                node_type: "TSLiteralType",
                start: loc.pos(unary.span.start),
                end: loc.pos(unary.span.end),
                loc: create_location(unary.span, loc),
                literal: public::TSLiteralTypeLiteral::UnaryExpression(public::UnaryExpression {
                    node_type: "UnaryExpression",
                    start: loc.pos(unary.span.start),
                    end: loc.pos(unary.span.end),
                    loc: create_location(unary.span, loc),
                    operator: unary.operator.as_str(),
                    prefix: unary.prefix,
                    argument: Box::new(public::Expression::Literal(convert_literal(
                        arg_lit, source, loc,
                    ))),
                }),
            })
        }
    }
}

fn convert_template_literal_type<'src>(
    template: &internal::TemplateLiteralType<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TemplateLiteralType<'src> {
    public::TemplateLiteralType {
        node_type: "TemplateLiteral",
        start: loc.pos(template.span.start),
        end: loc.pos(template.span.end),
        loc: create_location(template.span, loc),
        quasis: template
            .quasis
            .iter()
            .map(|q| super::convert_template_element(q, source, loc))
            .collect(),
        expressions: template
            .types
            .iter()
            .map(|t| convert_type(t, source, loc, interner))
            .collect(),
    }
}

// Template element conversion reuses convert_template_element from patterns module

/// Convert internal TSKeywordType to the appropriate public type variant
fn convert_keyword_type<'src>(
    kw: &internal::TSKeywordType,
    loc: LocationMapper<'_>,
) -> public::TSType<'src> {
    // Helper macro to reduce boilerplate - creates the public type struct
    macro_rules! make_public {
        ($variant:ident) => {{
            public::TSType::$variant(public::$variant {
                node_type: kw.kind.node_type_name(),
                start: loc.pos(kw.span.start),
                end: loc.pos(kw.span.end),
                loc: create_location(kw.span, loc),
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
                start: loc.pos(kw.span.start),
                end: loc.pos(kw.span.end),
                loc: create_location(kw.span, loc),
                literal: public::TSLiteralTypeLiteral::Literal(public::Literal {
                    node_type: "Literal",
                    start: loc.pos(kw.span.start),
                    end: loc.pos(kw.span.end),
                    loc: create_location(kw.span, loc),
                    value: serde_json::Value::Bool(is_true),
                    raw: Cow::Borrowed(if is_true { "true" } else { "false" }),
                    bigint: None,
                }),
            })
        }
    }
}

// Entity name conversion
pub(super) fn convert_entity_name<'src>(
    name: &internal::TSEntityName<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSEntityName<'src> {
    match name {
        internal::TSEntityName::Identifier(id) => {
            public::TSEntityName::Identifier(convert_identifier(id, source, loc, interner))
        }
        internal::TSEntityName::QualifiedName(qn) => {
            public::TSEntityName::QualifiedName(public::TSQualifiedName {
                node_type: "TSQualifiedName",
                start: loc.pos(qn.span.start),
                end: loc.pos(qn.span.end),
                loc: create_location(qn.span, loc),
                left: Box::new(convert_entity_name(&qn.left, source, loc, interner)),
                right: convert_identifier(&qn.right, source, loc, interner),
            })
        }
    }
}

fn convert_type_parameter_instantiation<'src>(
    params: &internal::TSTypeParameterInstantiation<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSTypeParameterInstantiation<'src> {
    public::TSTypeParameterInstantiation {
        node_type: "TSTypeParameterInstantiation",
        start: loc.pos(params.span.start),
        end: loc.pos(params.span.end),
        loc: create_location(params.span, loc),
        params: params
            .params
            .iter()
            .map(|p| convert_type(p, source, loc, interner))
            .collect(),
    }
}

/// Simplified version that passes through interner but uses placeholder identifier names
fn convert_type_parameter_declaration_simple<'src>(
    params: &internal::TSTypeParameterDeclaration<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSTypeParameterDeclaration<'src> {
    public::TSTypeParameterDeclaration {
        node_type: "TSTypeParameterDeclaration",
        start: loc.pos(params.span.start),
        end: loc.pos(params.span.end),
        loc: create_location(params.span, loc),
        params: params
            .params
            .iter()
            .map(|p| convert_type_parameter(p, source, loc, interner))
            .collect(),
        extra: params
            .trailing_comma
            .map(|pos| public::TSTypeParameterExtra {
                // Emitted like `start`/`end`: acorn reports it in UTF-16 units.
                trailing_comma: loc.pos(pos),
            }),
    }
}

fn convert_type_element<'src>(
    elem: &internal::TSTypeElement<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSTypeElement<'src> {
    match elem {
        internal::TSTypeElement::PropertySignature(p) => {
            public::TSTypeElement::PropertySignature(public::TSPropertySignature {
                node_type: "TSPropertySignature",
                start: loc.pos(p.span.start),
                end: loc.pos(p.span.end),
                loc: create_location(p.span, loc),
                key: convert_expression(&p.key, source, loc, interner),
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
                    .map(|ta| convert_type_annotation(ta, source, loc, interner)),
            })
        }
        internal::TSTypeElement::MethodSignature(m) => {
            let kind = match m.kind {
                internal::MethodKind::Get => Some("get"),
                internal::MethodKind::Set => Some("set"),
                _ => Some("method"),
            };
            public::TSTypeElement::MethodSignature(public::TSMethodSignature {
                node_type: "TSMethodSignature",
                start: loc.pos(m.span.start),
                end: loc.pos(m.span.end),
                loc: create_location(m.span, loc),
                computed: m.computed,
                key: convert_expression(&m.key, source, loc, interner),
                optional: m.optional,
                kind,
                type_parameters: m
                    .type_parameters
                    .as_ref()
                    .map(|tp| convert_type_parameter_declaration_simple(tp, source, loc, interner)),
                parameters: m
                    .params
                    .iter()
                    .map(|p| convert_expression(p, source, loc, interner))
                    .collect(),
                return_type: m
                    .return_type
                    .as_ref()
                    .map(|rt| convert_type_annotation(rt, source, loc, interner)),
            })
        }
        internal::TSTypeElement::CallSignature(c) => {
            public::TSTypeElement::CallSignature(public::TSCallSignatureDeclaration {
                node_type: "TSCallSignatureDeclaration",
                start: loc.pos(c.span.start),
                end: loc.pos(c.span.end),
                loc: create_location(c.span, loc),
                type_parameters: c
                    .type_parameters
                    .as_ref()
                    .map(|tp| convert_type_parameter_declaration_simple(tp, source, loc, interner)),
                params: c
                    .params
                    .iter()
                    .map(|p| convert_expression(p, source, loc, interner))
                    .collect(),
                return_type: c
                    .return_type
                    .as_ref()
                    .map(|rt| convert_type_annotation(rt, source, loc, interner)),
            })
        }
        internal::TSTypeElement::ConstructSignature(c) => {
            public::TSTypeElement::ConstructSignature(public::TSConstructSignatureDeclaration {
                node_type: "TSConstructSignatureDeclaration",
                start: loc.pos(c.span.start),
                end: loc.pos(c.span.end),
                loc: create_location(c.span, loc),
                type_parameters: c
                    .type_parameters
                    .as_ref()
                    .map(|tp| convert_type_parameter_declaration_simple(tp, source, loc, interner)),
                params: c
                    .params
                    .iter()
                    .map(|p| convert_expression(p, source, loc, interner))
                    .collect(),
                return_type: c
                    .return_type
                    .as_ref()
                    .map(|rt| convert_type_annotation(rt, source, loc, interner)),
            })
        }
        internal::TSTypeElement::IndexSignature(i) => {
            public::TSTypeElement::IndexSignature(public::TSIndexSignature {
                node_type: "TSIndexSignature",
                start: loc.pos(i.span.start),
                end: loc.pos(i.span.end),
                loc: create_location(i.span, loc),
                parameters: i
                    .parameters
                    .iter()
                    .map(|p| public::Identifier {
                        node_type: "Identifier",
                        start: loc.pos(p.span.start),
                        end: loc.pos(p.span.end),
                        loc: create_location(p.span, loc),
                        name: public::name_cow(p.span, source, p.name, interner),
                        optional: p.optional,
                        type_annotation: p
                            .type_annotation()
                            .map(|ta| convert_type_annotation(ta, source, loc, interner)),
                        decorators: Vec::new(),
                    })
                    .collect(),
                type_annotation: convert_type_annotation(&i.type_annotation, source, loc, interner),
                is_static: i.is_static,
                readonly: i.readonly,
            })
        }
    }
}

// Interface declaration conversion
pub(in crate::ast) fn convert_interface_declaration<'src>(
    iface: &internal::TSInterfaceDeclaration<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSInterfaceDeclaration<'src> {
    public::TSInterfaceDeclaration {
        node_type: "TSInterfaceDeclaration",
        start: loc.pos(iface.span.start),
        end: loc.pos(iface.span.end),
        loc: create_location(iface.span, loc),
        id: convert_identifier(&iface.id, source, loc, interner),
        type_parameters: iface
            .type_parameters
            .as_ref()
            .map(|tp| convert_type_parameter_declaration_simple(tp, source, loc, interner)),
        extends: iface
            .extends
            .iter()
            .map(|h| convert_interface_heritage(h, source, loc, interner))
            .collect(),
        body: convert_interface_body(&iface.body, source, loc, interner),
        declare: iface.declare,
    }
}

fn convert_interface_heritage<'src>(
    heritage: &internal::TSInterfaceHeritage<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSInterfaceHeritage<'src> {
    public::TSInterfaceHeritage {
        node_type: "TSExpressionWithTypeArguments",
        start: loc.pos(heritage.span.start),
        end: loc.pos(heritage.span.end),
        loc: create_location(heritage.span, loc),
        expression: convert_entity_name(&heritage.expression, source, loc, interner),
        type_parameters: heritage
            .type_arguments
            .as_ref()
            .map(|ta| convert_type_parameter_instantiation(ta, source, loc, interner)),
    }
}

fn convert_interface_body<'src>(
    body: &internal::TSInterfaceBody<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSInterfaceBody<'src> {
    public::TSInterfaceBody {
        node_type: "TSInterfaceBody",
        start: loc.pos(body.span.start),
        end: loc.pos(body.span.end),
        loc: create_location(body.span, loc),
        body: body
            .body
            .iter()
            .map(|m| convert_type_element(m, source, loc, interner))
            .collect(),
    }
}

// Declare function conversion
pub(in crate::ast) fn convert_declare_function<'src>(
    func: &internal::TSDeclareFunction<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSDeclareFunction<'src> {
    public::TSDeclareFunction {
        node_type: "TSDeclareFunction",
        start: loc.pos(func.span.start),
        end: loc.pos(func.span.end),
        loc: create_location(func.span, loc),
        declare: func.declare,
        id: convert_identifier(&func.id, source, loc, interner),
        expression: false, // Always false for declarations
        generator: func.generator,
        is_async: func.r#async,
        type_parameters: func.type_parameters.as_ref().map(|tp| {
            super::declarations::convert_type_parameter_declaration(tp, source, loc, interner)
        }),
        params: func
            .params
            .iter()
            .map(|p| convert_expression(p, source, loc, interner))
            .collect(),
        return_type: func
            .return_type
            .as_ref()
            .map(|rt| convert_type_annotation(rt, source, loc, interner)),
    }
}

/// Convert TSEnumDeclaration to public AST
pub(in crate::ast) fn convert_enum_declaration<'src>(
    enum_decl: &internal::TSEnumDeclaration<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSEnumDeclaration<'src> {
    public::TSEnumDeclaration {
        node_type: "TSEnumDeclaration",
        start: loc.pos(enum_decl.span.start),
        end: loc.pos(enum_decl.span.end),
        loc: create_location(enum_decl.span, loc),
        id: convert_identifier(&enum_decl.id, source, loc, interner),
        members: enum_decl
            .members
            .iter()
            .map(|m| convert_enum_member(m, source, loc, interner))
            .collect(),
        is_const: enum_decl.r#const,
        declare: enum_decl.declare,
    }
}

/// Convert a single TSEnumMember
fn convert_enum_member<'src>(
    member: &internal::TSEnumMember<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TSEnumMember<'src> {
    let id = match &member.id {
        internal::TSEnumMemberId::Identifier(id) => {
            public::TSEnumMemberId::Identifier(convert_identifier(id, source, loc, interner))
        }
        internal::TSEnumMemberId::String(lit) => {
            public::TSEnumMemberId::Literal(convert_literal(lit, source, loc))
        }
    };

    public::TSEnumMember {
        node_type: "TSEnumMember",
        start: loc.pos(member.span.start),
        end: loc.pos(member.span.end),
        loc: create_location(member.span, loc),
        id,
        initializer: member
            .initializer
            .as_ref()
            .map(|expr| convert_expression(expr, source, loc, interner)),
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
