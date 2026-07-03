// TypeScript type writers.

use super::super::super::internal;
use super::declarations::write_type_parameter;
use super::expressions::{write_expression, write_expressions};
use super::patterns::write_template_element;
use super::{
    Ctx, JsonWriter, close_node, node_header, write_array, write_bare_node, write_identifier_parts,
    write_identifier_plain, write_literal, write_or_null, write_type_annotation_field,
    write_type_arguments_field, write_type_parameters_field,
};
use internal::TSKeywordKind;
use tsv_lang::InfallibleResolve;

/// Emits a `TSTypeAnnotation` node. A Svelte block pattern's top-level
/// annotation (`ctx.pattern_ann_span`) omits the `loc` field — Svelte's
/// `read_context` synthesizes that node itself, without `loc`; nested
/// annotations keep theirs.
pub(super) fn write_type_annotation(
    w: &mut JsonWriter,
    type_annotation: &internal::TSTypeAnnotation<'_>,
    ctx: &Ctx<'_>,
) {
    if type_annotation.span == ctx.pattern_ann_span {
        // `{type, start, end, typeAnnotation}` — no `loc` (the block-pattern quirk).
        let span = type_annotation.span;
        w.raw("{\"type\":\"TSTypeAnnotation\",\"start\":");
        w.u32(ctx.loc.pos(span.start));
        w.raw(",\"end\":");
        w.u32(ctx.loc.pos(span.end));
    } else {
        node_header(w, "TSTypeAnnotation", type_annotation.span, ctx);
    }
    w.raw(",\"typeAnnotation\":");
    write_type(w, type_annotation.type_annotation, ctx);
    close_node(w, "TSTypeAnnotation", type_annotation.span, ctx);
}

/// Emit a `TSType`, dispatching on its variant.
pub(super) fn write_type(w: &mut JsonWriter, ts_type: &internal::TSType<'_>, ctx: &Ctx<'_>) {
    match ts_type {
        internal::TSType::Keyword(kw) => write_keyword_type(w, kw, ctx),
        internal::TSType::Literal(lit) => write_literal_type(w, lit, ctx),
        internal::TSType::Array(arr) => {
            node_header(w, "TSArrayType", arr.span, ctx);
            w.raw(",\"elementType\":");
            write_type(w, arr.element_type, ctx);
            close_node(w, "TSArrayType", arr.span, ctx);
        }
        internal::TSType::Union(u) => {
            node_header(w, "TSUnionType", u.span, ctx);
            w.raw(",\"types\":");
            write_array(w, u.types, |w, t| write_type(w, t, ctx));
            close_node(w, "TSUnionType", u.span, ctx);
        }
        internal::TSType::Intersection(i) => {
            node_header(w, "TSIntersectionType", i.span, ctx);
            w.raw(",\"types\":");
            write_array(w, i.types, |w, t| write_type(w, t, ctx));
            close_node(w, "TSIntersectionType", i.span, ctx);
        }
        internal::TSType::TypeReference(r) => {
            node_header(w, "TSTypeReference", r.span, ctx);
            w.raw(",\"typeName\":");
            write_entity_name(w, &r.type_name, ctx);
            write_type_arguments_field(w, r.type_arguments.as_ref(), ctx);
            close_node(w, "TSTypeReference", r.span, ctx);
        }
        internal::TSType::TypeLiteral(t) => {
            node_header(w, "TSTypeLiteral", t.span, ctx);
            w.raw(",\"members\":");
            write_array(w, t.members, |w, m| write_type_element(w, m, ctx));
            close_node(w, "TSTypeLiteral", t.span, ctx);
        }
        internal::TSType::Function(f) => {
            node_header(w, "TSFunctionType", f.span, ctx);
            write_type_parameters_field(w, f.type_parameters.as_ref(), ctx);
            w.raw(",\"parameters\":");
            write_expressions(w, f.params, ctx);
            w.raw(",\"typeAnnotation\":");
            write_type_annotation(w, &f.return_type, ctx);
            close_node(w, "TSFunctionType", f.span, ctx);
        }
        internal::TSType::Constructor(c) => {
            node_header(w, "TSConstructorType", c.span, ctx);
            w.raw(",\"abstract\":");
            w.bool(c.abstract_);
            write_type_parameters_field(w, c.type_parameters.as_ref(), ctx);
            w.raw(",\"parameters\":");
            write_expressions(w, c.params, ctx);
            w.raw(",\"typeAnnotation\":");
            write_type_annotation(w, &c.return_type, ctx);
            close_node(w, "TSConstructorType", c.span, ctx);
        }
        internal::TSType::Tuple(t) => {
            node_header(w, "TSTupleType", t.span, ctx);
            w.raw(",\"elementTypes\":");
            write_array(w, t.element_types, |w, e| write_type(w, e, ctx));
            close_node(w, "TSTupleType", t.span, ctx);
        }
        internal::TSType::Parenthesized(p) => {
            node_header(w, "TSParenthesizedType", p.span, ctx);
            w.raw(",\"typeAnnotation\":");
            write_type(w, p.type_annotation, ctx);
            close_node(w, "TSParenthesizedType", p.span, ctx);
        }
        internal::TSType::TypePredicate(p) => {
            node_header(w, "TSTypePredicate", p.span, ctx);
            w.raw(",\"parameterName\":");
            if ctx.interner.resolve_infallible(p.parameter_name.name) == "this" {
                write_bare_node(w, "TSThisType", p.parameter_name.span, ctx);
            } else {
                write_identifier_plain(w, &p.parameter_name, ctx);
            }
            // acorn-typescript's annotation-less forms (`asserts foo` /
            // `asserts this`) assign `asserts` before the null annotation;
            // the `is T` forms assign the annotation first and stamp
            // `asserts` last. The annotation node is synthesized around the
            // type's own span (there is no `:` in `x is T`).
            match &p.type_annotation {
                Some(t) => {
                    w.raw(",\"typeAnnotation\":");
                    node_header(w, "TSTypeAnnotation", t.span(), ctx);
                    w.raw(",\"typeAnnotation\":");
                    write_type(w, t, ctx);
                    close_node(w, "TSTypeAnnotation", t.span(), ctx);
                    w.raw(",\"asserts\":");
                    w.bool(p.asserts);
                }
                None => {
                    w.raw(",\"asserts\":");
                    w.bool(p.asserts);
                    w.raw(",\"typeAnnotation\":null");
                }
            }
            close_node(w, "TSTypePredicate", p.span, ctx);
        }
        internal::TSType::Conditional(c) => {
            node_header(w, "TSConditionalType", c.span, ctx);
            w.raw(",\"checkType\":");
            write_type(w, c.check_type, ctx);
            w.raw(",\"extendsType\":");
            write_type(w, c.extends_type, ctx);
            w.raw(",\"trueType\":");
            write_type(w, c.true_type, ctx);
            w.raw(",\"falseType\":");
            write_type(w, c.false_type, ctx);
            close_node(w, "TSConditionalType", c.span, ctx);
        }
        internal::TSType::Mapped(m) => {
            node_header(w, "TSMappedType", m.span, ctx);
            if let Some(modifier) = m.readonly {
                w.raw(",\"readonly\":");
                write_mapped_type_modifier(w, modifier);
            }
            // The `TSMappedTypeParameter`: the parameter name is a bare symbol
            // with no name-only span (the struct span covers `K in C`), so it
            // resolves from the interner.
            w.raw(",\"typeParameter\":");
            node_header(w, "TSTypeParameter", m.type_parameter.span, ctx);
            w.raw(",\"name\":");
            w.string(ctx.interner.resolve_infallible(m.type_parameter.name));
            w.raw(",\"constraint\":");
            write_type(w, m.type_parameter.constraint, ctx);
            close_node(w, "TSTypeParameter", m.type_parameter.span, ctx);
            w.raw(",\"nameType\":");
            write_or_null(w, m.name_type.as_ref(), |w, t| write_type(w, t, ctx));
            if let Some(modifier) = m.optional {
                w.raw(",\"optional\":");
                write_mapped_type_modifier(w, modifier);
            }
            if let Some(t) = &m.type_annotation {
                w.raw(",\"typeAnnotation\":");
                write_type(w, t, ctx);
            }
            close_node(w, "TSMappedType", m.span, ctx);
        }
        internal::TSType::TypeOperator(o) => {
            node_header(w, "TSTypeOperator", o.span, ctx);
            w.raw(",\"operator\":");
            w.token(o.operator.as_str());
            w.raw(",\"typeAnnotation\":");
            write_type(w, o.type_annotation, ctx);
            close_node(w, "TSTypeOperator", o.span, ctx);
        }
        internal::TSType::Import(i) => write_import_type(w, i, ctx),
        internal::TSType::TypeQuery(q) => {
            node_header(w, "TSTypeQuery", q.span, ctx);
            w.raw(",\"exprName\":");
            match &q.expr_name {
                internal::TSTypeQueryExprName::EntityName(entity) => match entity {
                    internal::TSEntityName::Identifier(id) => write_identifier_plain(w, id, ctx),
                    internal::TSEntityName::QualifiedName(qn) => write_qualified_name(w, qn, ctx),
                },
                internal::TSTypeQueryExprName::Import(i) => write_import_type(w, i, ctx),
            }
            write_type_arguments_field(w, q.type_arguments.as_ref(), ctx);
            close_node(w, "TSTypeQuery", q.span, ctx);
        }
        internal::TSType::IndexedAccess(i) => {
            node_header(w, "TSIndexedAccessType", i.span, ctx);
            w.raw(",\"objectType\":");
            write_type(w, i.object_type, ctx);
            w.raw(",\"indexType\":");
            write_type(w, i.index_type, ctx);
            close_node(w, "TSIndexedAccessType", i.span, ctx);
        }
        internal::TSType::Rest(r) => {
            node_header(w, "TSRestType", r.span, ctx);
            w.raw(",\"typeAnnotation\":");
            write_type(w, r.type_annotation, ctx);
            close_node(w, "TSRestType", r.span, ctx);
        }
        internal::TSType::Optional(o) => {
            node_header(w, "TSOptionalType", o.span, ctx);
            w.raw(",\"typeAnnotation\":");
            write_type(w, o.type_annotation, ctx);
            close_node(w, "TSOptionalType", o.span, ctx);
        }
        internal::TSType::NamedTupleMember(n) => {
            // Field order: `optional`, `label`, `elementType`.
            node_header(w, "TSNamedTupleMember", n.span, ctx);
            w.raw(",\"optional\":");
            w.bool(n.optional);
            w.raw(",\"label\":");
            write_identifier_plain(w, &n.label, ctx);
            w.raw(",\"elementType\":");
            write_type(w, n.element_type, ctx);
            close_node(w, "TSNamedTupleMember", n.span, ctx);
        }
        internal::TSType::Infer(i) => {
            node_header(w, "TSInferType", i.span, ctx);
            w.raw(",\"typeParameter\":");
            write_type_parameter(w, &i.type_parameter, ctx);
            close_node(w, "TSInferType", i.span, ctx);
        }
        internal::TSType::ThisType(t) => {
            write_bare_node(w, "TSThisType", t.span, ctx);
        }
    }
}

/// Emit a mapped-type modifier (`TSMappedTypeModifier`): `true`, `"+"`, or
/// `"-"`.
fn write_mapped_type_modifier(w: &mut JsonWriter, m: internal::TSMappedTypeModifier) {
    match m {
        internal::TSMappedTypeModifier::True => w.raw("true"),
        internal::TSMappedTypeModifier::Plus => w.raw("\"+\""),
        internal::TSMappedTypeModifier::Minus => w.raw("\"-\""),
    }
}

/// Emits a `TSImportType` node (used in both type position and
/// `typeof import(...)`). Field order: `argument`, `options?`, `qualifier?`,
/// `typeArguments?`.
fn write_import_type(w: &mut JsonWriter, i: &internal::TSImportType<'_>, ctx: &Ctx<'_>) {
    node_header(w, "TSImportType", i.span, ctx);
    w.raw(",\"argument\":");
    write_literal(w, &i.argument, ctx);
    if let Some(o) = &i.options {
        w.raw(",\"options\":");
        write_expression(w, o, ctx);
    }
    if let Some(q) = &i.qualifier {
        w.raw(",\"qualifier\":");
        write_entity_name(w, q, ctx);
    }
    write_type_arguments_field(w, i.type_arguments.as_ref(), ctx);
    close_node(w, "TSImportType", i.span, ctx);
}

/// Emits a `TSLiteralType` node.
fn write_literal_type(w: &mut JsonWriter, lit: &internal::TSLiteralType<'_>, ctx: &Ctx<'_>) {
    match lit {
        internal::TSLiteralType::TemplateLiteral(template) => {
            node_header(w, "TSLiteralType", template.span, ctx);
            w.raw(",\"literal\":");
            // A template-literal type: node type "TemplateLiteral", but its
            // expressions are types.
            node_header(w, "TemplateLiteral", template.span, ctx);
            w.raw(",\"expressions\":");
            write_array(w, template.types, |w, t| write_type(w, t, ctx));
            w.raw(",\"quasis\":");
            write_array(w, template.quasis, |w, q| write_template_element(w, q, ctx));
            // Close the inner `TemplateLiteral`, then the `TSLiteralType`.
            close_node(w, "TemplateLiteral", template.span, ctx);
            close_node(w, "TSLiteralType", template.span, ctx);
        }
        internal::TSLiteralType::String(literal)
        | internal::TSLiteralType::Number(literal)
        | internal::TSLiteralType::BigInt(literal) => {
            node_header(w, "TSLiteralType", literal.span, ctx);
            w.raw(",\"literal\":");
            write_literal(w, literal, ctx);
            close_node(w, "TSLiteralType", literal.span, ctx);
        }
        internal::TSLiteralType::UnaryExpression(unary) => {
            // Negative number types like `-1`; the parser guarantees the
            // argument is a Literal.
            #[allow(clippy::unreachable)]
            let internal::Expression::Literal(arg_lit) = unary.argument else {
                unreachable!(
                    "parser only creates TSLiteralType::UnaryExpression with Literal argument"
                )
            };
            node_header(w, "TSLiteralType", unary.span, ctx);
            w.raw(",\"literal\":");
            node_header(w, "UnaryExpression", unary.span, ctx);
            w.raw(",\"operator\":");
            w.token(unary.operator.as_str());
            w.raw(",\"prefix\":");
            w.bool(unary.prefix);
            w.raw(",\"argument\":");
            write_literal(w, arg_lit, ctx);
            // Close the inner `UnaryExpression`, then the `TSLiteralType`.
            close_node(w, "UnaryExpression", unary.span, ctx);
            close_node(w, "TSLiteralType", unary.span, ctx);
        }
    }
}

/// Emit a keyword type: keyword nodes are bare; boolean literal types
/// (`true`/`false` as types) emit a `TSLiteralType` with a synthesized
/// `Literal`.
fn write_keyword_type(w: &mut JsonWriter, kw: &internal::TSKeywordType, ctx: &Ctx<'_>) {
    match kw.kind {
        TSKeywordKind::True | TSKeywordKind::False => {
            let is_true = matches!(kw.kind, TSKeywordKind::True);
            node_header(w, "TSLiteralType", kw.span, ctx);
            w.raw(",\"literal\":");
            node_header(w, "Literal", kw.span, ctx);
            w.raw(",\"value\":");
            w.bool(is_true);
            w.raw(",\"raw\":");
            w.token(if is_true { "true" } else { "false" });
            // Close the inner `Literal`, then the `TSLiteralType`.
            close_node(w, "Literal", kw.span, ctx);
            close_node(w, "TSLiteralType", kw.span, ctx);
        }
        _ => write_bare_node(w, kw.kind.node_type_name(), kw.span, ctx),
    }
}

/// Emit an entity name (an `Identifier` or a `TSQualifiedName`).
pub(super) fn write_entity_name(
    w: &mut JsonWriter,
    name: &internal::TSEntityName<'_>,
    ctx: &Ctx<'_>,
) {
    match name {
        internal::TSEntityName::Identifier(id) => write_identifier_plain(w, id, ctx),
        internal::TSEntityName::QualifiedName(qn) => write_qualified_name(w, qn, ctx),
    }
}

/// Emits a `TSQualifiedName` node (`left`, `right`).
fn write_qualified_name(w: &mut JsonWriter, qn: &internal::TSQualifiedName<'_>, ctx: &Ctx<'_>) {
    node_header(w, "TSQualifiedName", qn.span, ctx);
    w.raw(",\"left\":");
    write_entity_name(w, &qn.left, ctx);
    w.raw(",\"right\":");
    write_identifier_plain(w, &qn.right, ctx);
    close_node(w, "TSQualifiedName", qn.span, ctx);
}

/// Emits a `TSTypeParameterInstantiation` node.
pub(super) fn write_type_parameter_instantiation(
    w: &mut JsonWriter,
    params: &internal::TSTypeParameterInstantiation<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "TSTypeParameterInstantiation", params.span, ctx);
    w.raw(",\"params\":");
    write_array(w, params.params, |w, p| write_type(w, p, ctx));
    close_node(w, "TSTypeParameterInstantiation", params.span, ctx);
}

/// Emit a `TSTypeElement` (property/method/call/construct/index signature).
fn write_type_element(w: &mut JsonWriter, elem: &internal::TSTypeElement<'_>, ctx: &Ctx<'_>) {
    match elem {
        internal::TSTypeElement::PropertySignature(p) => {
            // Field order: `readonly` (only when true), `computed?`, `key`,
            // `optional` (only when true), `typeAnnotation?`.
            node_header(w, "TSPropertySignature", p.span, ctx);
            if p.readonly {
                w.raw(",\"readonly\":true");
            }
            // acorn quirk: omits `computed` when the key is the `new` keyword
            // and the signature is neither computed nor readonly.
            let is_new_key = matches!(&p.key, internal::Expression::Identifier(id)
                if ctx.interner.resolve_infallible(id.name) == "new");
            if !(!p.computed && !p.readonly && is_new_key) {
                w.raw(",\"computed\":");
                w.bool(p.computed);
            }
            w.raw(",\"key\":");
            write_expression(w, &p.key, ctx);
            if p.optional {
                w.raw(",\"optional\":true");
            }
            write_type_annotation_field(w, p.type_annotation.as_ref(), ctx);
            close_node(w, "TSPropertySignature", p.span, ctx);
        }
        internal::TSTypeElement::MethodSignature(m) => {
            // Field order: `computed`, `key`, then per kind — a get/set
            // signature's `kind` is assigned between the two
            // `parsePropertyName` calls (right after the key); a plain
            // method's `kind` lands on the finished signature (last). Then
            // `optional` (only when true), `typeParameters?`, `parameters`,
            // `typeAnnotation?` (the return type's wire name).
            node_header(w, "TSMethodSignature", m.span, ctx);
            w.raw(",\"computed\":");
            w.bool(m.computed);
            w.raw(",\"key\":");
            write_expression(w, &m.key, ctx);
            let getset = matches!(
                m.kind,
                internal::MethodKind::Get | internal::MethodKind::Set
            );
            if getset {
                w.raw(",\"kind\":");
                w.token(match m.kind {
                    internal::MethodKind::Get => "get",
                    _ => "set",
                });
            }
            if m.optional {
                w.raw(",\"optional\":true");
            }
            write_type_parameters_field(w, m.type_parameters.as_ref(), ctx);
            w.raw(",\"parameters\":");
            write_expressions(w, m.params, ctx);
            write_type_annotation_field(w, m.return_type.as_ref(), ctx);
            if !getset {
                w.raw(",\"kind\":\"method\"");
            }
            close_node(w, "TSMethodSignature", m.span, ctx);
        }
        internal::TSTypeElement::CallSignature(c) => {
            write_signature_declaration(
                w,
                "TSCallSignatureDeclaration",
                c.span,
                c.type_parameters.as_ref(),
                c.params,
                c.return_type.as_ref(),
                ctx,
            );
        }
        internal::TSTypeElement::ConstructSignature(c) => {
            write_signature_declaration(
                w,
                "TSConstructSignatureDeclaration",
                c.span,
                c.type_parameters.as_ref(),
                c.params,
                c.return_type.as_ref(),
                ctx,
            );
        }
        internal::TSTypeElement::IndexSignature(i) => write_index_signature(w, i, ctx),
    }
}

/// Shared call/construct signature shape: `typeParameters?`, `parameters`,
/// `typeAnnotation?` (the return type's wire name).
fn write_signature_declaration(
    w: &mut JsonWriter,
    node_type: &str,
    span: tsv_lang::Span,
    type_parameters: Option<&internal::TSTypeParameterDeclaration<'_>>,
    params: &[internal::Expression<'_>],
    return_type: Option<&internal::TSTypeAnnotation<'_>>,
    ctx: &Ctx<'_>,
) {
    node_header(w, node_type, span, ctx);
    write_type_parameters_field(w, type_parameters, ctx);
    w.raw(",\"parameters\":");
    write_expressions(w, params, ctx);
    write_type_annotation_field(w, return_type, ctx);
    close_node(w, node_type, span, ctx);
}

/// Emits a `TSIndexSignature` node (identical for a class member and a type
/// element). Field order: `static` (only when true), `readonly` (only when
/// true), `parameters`, `typeAnnotation`.
pub(super) fn write_index_signature(
    w: &mut JsonWriter,
    sig: &internal::TSIndexSignature<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "TSIndexSignature", sig.span, ctx);
    if sig.is_static {
        w.raw(",\"static\":true");
    }
    if sig.readonly {
        w.raw(",\"readonly\":true");
    }
    w.raw(",\"parameters\":");
    write_array(w, sig.parameters, |w, p| {
        // Index-signature parameters carry the binding's optional flag and
        // type annotation, never decorators.
        write_identifier_parts(
            w,
            p.span,
            p.name,
            p.optional,
            p.type_annotation(),
            None,
            ctx,
        );
    });
    w.raw(",\"typeAnnotation\":");
    write_type_annotation(w, &sig.type_annotation, ctx);
    close_node(w, "TSIndexSignature", sig.span, ctx);
}

/// Emits a `TSInterfaceDeclaration` node (its `extends` clauses as
/// `TSExpressionWithTypeArguments`, its body a `TSInterfaceBody`). Field order:
/// `declare?` (statement position), `id`, `typeParameters?`, `extends` (only
/// when non-empty), `body`, `declare?` (export position) — acorn-typescript's
/// statement-level `declare interface` passes `declare` into the interface
/// parse (assigned before `id`), while `export declare` stamps the finished
/// node, so the field's position depends on `exported`.
pub(super) fn write_interface_declaration(
    w: &mut JsonWriter,
    iface: &internal::TSInterfaceDeclaration<'_>,
    ctx: &Ctx<'_>,
    exported: bool,
) {
    node_header(w, "TSInterfaceDeclaration", iface.span, ctx);
    if iface.declare && !exported {
        w.raw(",\"declare\":true");
    }
    w.raw(",\"id\":");
    write_identifier_plain(w, &iface.id, ctx);
    write_type_parameters_field(w, iface.type_parameters.as_ref(), ctx);
    if !iface.extends.is_empty() {
        w.raw(",\"extends\":");
        write_array(w, iface.extends, |w, h| {
            // Node type "TSExpressionWithTypeArguments"; the expression is an
            // entity name.
            node_header(w, "TSExpressionWithTypeArguments", h.span, ctx);
            w.raw(",\"expression\":");
            write_entity_name(w, &h.expression, ctx);
            if let Some(ta) = &h.type_arguments {
                w.raw(",\"typeParameters\":");
                write_type_parameter_instantiation(w, ta, ctx);
            }
            close_node(w, "TSExpressionWithTypeArguments", h.span, ctx);
        });
    }
    w.raw(",\"body\":");
    node_header(w, "TSInterfaceBody", iface.body.span, ctx);
    w.raw(",\"body\":");
    write_array(w, iface.body.body, |w, m| write_type_element(w, m, ctx));
    close_node(w, "TSInterfaceBody", iface.body.span, ctx);
    if iface.declare && exported {
        w.raw(",\"declare\":true");
    }
    close_node(w, "TSInterfaceDeclaration", iface.span, ctx);
}

/// Emits a `TSDeclareFunction` node. Field order: `declare?` (statement
/// position), `id`, `expression` (always false), `generator`, `async`,
/// `typeParameters?`, `params`, `returnType?`, `declare?` (export position) —
/// acorn-typescript's statement-level `declare function` stamps `declare`
/// before the function parses (`tsTryParseDeclare`), while `export declare`
/// stamps the finished node, so the field's position depends on `exported`.
pub(super) fn write_declare_function(
    w: &mut JsonWriter,
    func: &internal::TSDeclareFunction<'_>,
    ctx: &Ctx<'_>,
    exported: bool,
) {
    node_header(w, "TSDeclareFunction", func.span, ctx);
    if func.declare && !exported {
        w.raw(",\"declare\":true");
    }
    w.raw(",\"id\":");
    write_identifier_plain(w, &func.id, ctx);
    w.raw(",\"expression\":false,\"generator\":");
    w.bool(func.generator);
    w.raw(",\"async\":");
    w.bool(func.r#async);
    write_type_parameters_field(w, func.type_parameters.as_ref(), ctx);
    w.raw(",\"params\":");
    write_expressions(w, func.params, ctx);
    super::write_return_type_field(w, func.return_type.as_ref(), ctx);
    if func.declare && exported {
        w.raw(",\"declare\":true");
    }
    close_node(w, "TSDeclareFunction", func.span, ctx);
}

/// Emits a `TSEnumDeclaration` node (each member a `TSEnumMember`). Field
/// order: `const` (only when true), `declare` (only when true), `id`, `members`
/// (each: `id`, `initializer?`).
pub(super) fn write_enum_declaration(
    w: &mut JsonWriter,
    enum_decl: &internal::TSEnumDeclaration<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "TSEnumDeclaration", enum_decl.span, ctx);
    if enum_decl.r#const {
        w.raw(",\"const\":true");
    }
    if enum_decl.declare {
        w.raw(",\"declare\":true");
    }
    w.raw(",\"id\":");
    write_identifier_plain(w, &enum_decl.id, ctx);
    w.raw(",\"members\":");
    write_array(w, enum_decl.members, |w, member| {
        node_header(w, "TSEnumMember", member.span, ctx);
        w.raw(",\"id\":");
        match &member.id {
            internal::TSEnumMemberId::Identifier(id) => write_identifier_plain(w, id, ctx),
            internal::TSEnumMemberId::String(lit) => write_literal(w, lit, ctx),
        }
        if let Some(init) = &member.initializer {
            w.raw(",\"initializer\":");
            write_expression(w, init, ctx);
        }
        close_node(w, "TSEnumMember", member.span, ctx);
    });
    close_node(w, "TSEnumDeclaration", enum_decl.span, ctx);
}
