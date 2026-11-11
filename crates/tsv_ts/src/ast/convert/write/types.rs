// TypeScript type writers.

use super::declarations::{write_decorators_field, write_type_parameter};
use super::expressions::{write_expression, write_expressions};
use super::patterns::write_template_element;
use super::{
    Ctx, JsonWriter, close_node, node_header, write_array, write_bare_node,
    write_function_flags_fields, write_identifier_parts, write_identifier_plain, write_literal,
    write_name_field, write_or_null, write_type_annotation_field, write_type_arguments_field,
    write_type_parameters_field,
};
use crate::ast;
use ast::TSKeywordKind;
use smallvec::SmallVec;

/// Emits a `TSTypeAnnotation` node. A Svelte block pattern's top-level
/// annotation (`ctx.pattern_ann_span`) is its own comment island
/// (`ctx.annotation_comments`), since Svelte reads it with a second acorn parse.
pub(super) fn write_type_annotation(
    w: &mut JsonWriter,
    type_annotation: &ast::TSTypeAnnotation,
    ctx: &Ctx<'_>,
) {
    let annotation_ctx;
    let ctx = if type_annotation.span == ctx.pattern_ann_span {
        annotation_ctx = Ctx {
            comments: ctx.annotation_comments,
            ..*ctx
        };
        &annotation_ctx
    } else {
        ctx
    };
    node_header(w, "TSTypeAnnotation", type_annotation.span, ctx);
    w.raw(",\"typeAnnotation\":");
    write_type(w, ctx.view.ty(type_annotation.type_annotation), ctx);
    close_node(w, "TSTypeAnnotation", type_annotation.span, ctx);
}

/// Emit a `TSType`, dispatching on its variant.
pub(super) fn write_type(w: &mut JsonWriter, ts_type: &ast::TSType<'_>, ctx: &Ctx<'_>) {
    match ts_type {
        ast::TSType::Keyword(kw) => write_keyword_type(w, kw, ctx),
        ast::TSType::Literal(lit) => write_literal_type(w, lit, ctx),
        ast::TSType::Array(_) | ast::TSType::IndexedAccess(_) => {
            write_postfix_type_chain(w, ts_type, ctx);
        }
        ast::TSType::Union(u) => {
            node_header(w, "TSUnionType", u.span, ctx);
            w.raw(",\"types\":");
            write_array(w, ctx.view.ty_list(u.types), |w, &t| {
                write_type(w, ctx.view.ty(t), ctx);
            });
            close_node(w, "TSUnionType", u.span, ctx);
        }
        ast::TSType::Intersection(i) => {
            node_header(w, "TSIntersectionType", i.span, ctx);
            w.raw(",\"types\":");
            write_array(w, ctx.view.ty_list(i.types), |w, &t| {
                write_type(w, ctx.view.ty(t), ctx);
            });
            close_node(w, "TSIntersectionType", i.span, ctx);
        }
        ast::TSType::TypeReference(r) => {
            node_header(w, "TSTypeReference", r.span, ctx);
            w.raw(",\"typeName\":");
            write_entity_name(w, &r.type_name, ctx);
            write_type_arguments_field(w, r.type_arguments.as_ref(), ctx);
            close_node(w, "TSTypeReference", r.span, ctx);
        }
        ast::TSType::TypeLiteral(t) => {
            node_header(w, "TSTypeLiteral", t.span, ctx);
            w.raw(",\"members\":");
            write_array(w, ctx.view.type_members(t.members), |w, m| {
                write_type_element(w, m, ctx);
            });
            close_node(w, "TSTypeLiteral", t.span, ctx);
        }
        ast::TSType::Function(f) => {
            node_header(w, "TSFunctionType", f.span, ctx);
            write_type_parameters_field(w, f.type_parameters.as_ref(), ctx);
            w.raw(",\"parameters\":");
            write_expressions(w, ctx.view.run(f.params), ctx);
            w.raw(",\"typeAnnotation\":");
            write_type_annotation(w, &f.return_type, ctx);
            close_node(w, "TSFunctionType", f.span, ctx);
        }
        &ast::TSType::Constructor(_, c) => {
            let c = ctx.view.ctor_type(c);
            node_header(w, "TSConstructorType", c.span, ctx);
            w.raw(",\"abstract\":");
            w.bool(c.abstract_);
            write_type_parameters_field(w, c.type_parameters.as_ref(), ctx);
            w.raw(",\"parameters\":");
            write_expressions(w, ctx.view.run(c.params), ctx);
            w.raw(",\"typeAnnotation\":");
            write_type_annotation(w, &c.return_type, ctx);
            close_node(w, "TSConstructorType", c.span, ctx);
        }
        ast::TSType::Tuple(t) => {
            node_header(w, "TSTupleType", t.span, ctx);
            w.raw(",\"elementTypes\":");
            write_array(w, ctx.view.ty_list(t.element_types), |w, &e| {
                write_type(w, ctx.view.ty(e), ctx);
            });
            close_node(w, "TSTupleType", t.span, ctx);
        }
        ast::TSType::Parenthesized(p) => {
            node_header(w, "TSParenthesizedType", p.span, ctx);
            w.raw(",\"typeAnnotation\":");
            write_type(w, ctx.view.ty(p.type_annotation), ctx);
            close_node(w, "TSParenthesizedType", p.span, ctx);
        }
        ast::TSType::TypePredicate(p) => {
            node_header(w, "TSTypePredicate", p.span, ctx);
            w.raw(",\"parameterName\":");
            if p.parameter_name.name(ctx.source, ctx.view) == "this" {
                write_bare_node(w, "TSThisType", p.parameter_name.span, ctx);
            } else {
                write_identifier_plain(w, &p.parameter_name, ctx);
            }
            // acorn-typescript's annotation-less forms (`asserts foo` /
            // `asserts this`) assign `asserts` before the null annotation;
            // the `is T` forms assign the annotation first and stamp
            // `asserts` last. The annotation node is synthesized around the
            // type's own span (there is no `:` in `x is T`).
            match p.type_annotation {
                Some(t) => {
                    w.raw(",\"typeAnnotation\":");
                    node_header(w, "TSTypeAnnotation", ctx.view.ty(t).span(), ctx);
                    w.raw(",\"typeAnnotation\":");
                    write_type(w, ctx.view.ty(t), ctx);
                    close_node(w, "TSTypeAnnotation", ctx.view.ty(t).span(), ctx);
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
        ast::TSType::Conditional(c) => {
            node_header(w, "TSConditionalType", c.span, ctx);
            w.raw(",\"checkType\":");
            write_type(w, ctx.view.ty(c.check_type), ctx);
            w.raw(",\"extendsType\":");
            write_type(w, ctx.view.ty(c.extends_type), ctx);
            w.raw(",\"trueType\":");
            write_type(w, ctx.view.ty(c.true_type), ctx);
            w.raw(",\"falseType\":");
            write_type(w, ctx.view.ty(c.false_type), ctx);
            close_node(w, "TSConditionalType", c.span, ctx);
        }
        ast::TSType::Mapped(m) => {
            node_header(w, "TSMappedType", m.span, ctx);
            if let Some(modifier) = m.readonly {
                w.raw(",\"readonly\":");
                write_mapped_type_modifier(w, modifier);
            }
            // The `TSMappedTypeParameter`: the parameter name is a bare name
            // channel; the name token is the leading `raw_len` bytes of the
            // struct span (which covers `K in C`).
            w.raw(",\"typeParameter\":");
            node_header(w, "TSTypeParameter", m.type_parameter.span, ctx);
            write_name_field(w, m.type_parameter.name, m.type_parameter.span.start, ctx);
            w.raw(",\"constraint\":");
            write_type(w, ctx.view.ty(m.type_parameter.constraint), ctx);
            close_node(w, "TSTypeParameter", m.type_parameter.span, ctx);
            w.raw(",\"nameType\":");
            write_or_null(w, m.name_type.as_ref(), |w, &t| {
                write_type(w, ctx.view.ty(t), ctx);
            });
            if let Some(modifier) = m.optional {
                w.raw(",\"optional\":");
                write_mapped_type_modifier(w, modifier);
            }
            if let Some(t) = ctx.view.opt_ty(m.type_annotation) {
                w.raw(",\"typeAnnotation\":");
                write_type(w, t, ctx);
            }
            close_node(w, "TSMappedType", m.span, ctx);
        }
        ast::TSType::TypeOperator(o) => {
            node_header(w, "TSTypeOperator", o.span, ctx);
            w.raw(",\"operator\":");
            w.token(o.operator.as_str());
            w.raw(",\"typeAnnotation\":");
            write_type(w, ctx.view.ty(o.type_annotation), ctx);
            close_node(w, "TSTypeOperator", o.span, ctx);
        }
        &ast::TSType::Import(_, i) => write_import_type(w, ctx.view.import_type(i), ctx),
        ast::TSType::TypeQuery(q) => {
            node_header(w, "TSTypeQuery", q.span, ctx);
            w.raw(",\"exprName\":");
            match &q.expr_name {
                ast::TSTypeQueryExprName::EntityName(entity) => match entity {
                    ast::TSEntityName::Identifier(id) => write_identifier_plain(w, id, ctx),
                    &ast::TSEntityName::QualifiedName(_, qn) => {
                        write_qualified_name(w, ctx.view.qual_name(qn), ctx);
                    }
                },
                &ast::TSTypeQueryExprName::Import(_, i) => {
                    write_import_type(w, ctx.view.import_type(i), ctx);
                }
            }
            write_type_arguments_field(w, q.type_arguments.as_ref(), ctx);
            close_node(w, "TSTypeQuery", q.span, ctx);
        }
        ast::TSType::Rest(r) => {
            node_header(w, "TSRestType", r.span, ctx);
            w.raw(",\"typeAnnotation\":");
            write_type(w, ctx.view.ty(r.type_annotation), ctx);
            close_node(w, "TSRestType", r.span, ctx);
        }
        ast::TSType::Optional(o) => {
            node_header(w, "TSOptionalType", o.span, ctx);
            w.raw(",\"typeAnnotation\":");
            write_type(w, ctx.view.ty(o.type_annotation), ctx);
            close_node(w, "TSOptionalType", o.span, ctx);
        }
        ast::TSType::NamedTupleMember(n) => {
            // Field order: `label`, `optional`, `elementType` — acorn-typescript stamps
            // the label first, reading it before the `?` it may carry.
            node_header(w, "TSNamedTupleMember", n.span, ctx);
            w.raw(",\"label\":");
            write_identifier_plain(w, &n.label, ctx);
            w.raw(",\"optional\":");
            w.bool(n.optional);
            w.raw(",\"elementType\":");
            write_type(w, ctx.view.ty(n.element_type), ctx);
            close_node(w, "TSNamedTupleMember", n.span, ctx);
        }
        &ast::TSType::Infer(_, i) => {
            let i = ctx.view.infer_type(i);
            node_header(w, "TSInferType", i.span, ctx);
            w.raw(",\"typeParameter\":");
            write_type_parameter(w, &i.type_parameter, ctx);
            close_node(w, "TSInferType", i.span, ctx);
        }
        ast::TSType::ThisType(t) => {
            write_bare_node(w, "TSThisType", t.span, ctx);
        }
    }
}

/// Emit a mapped-type modifier (`TSMappedTypeModifier`): `true`, `"+"`, or
/// `"-"`.
fn write_mapped_type_modifier(w: &mut JsonWriter, m: ast::TSMappedTypeModifier) {
    match m {
        ast::TSMappedTypeModifier::True => w.raw("true"),
        ast::TSMappedTypeModifier::Plus => w.raw("\"+\""),
        ast::TSMappedTypeModifier::Minus => w.raw("\"-\""),
    }
}

/// Emits a `TSImportType` node (used in both type position and
/// `typeof import(...)`). Field order: `argument`, `options?`, `qualifier?`,
/// `typeArguments?`.
fn write_import_type(w: &mut JsonWriter, i: &ast::TSImportType<'_>, ctx: &Ctx<'_>) {
    node_header(w, "TSImportType", i.span, ctx);
    w.raw(",\"argument\":");
    write_literal(w, &i.argument, ctx);
    if let Some(o) = ctx.view.opt(i.options) {
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
fn write_literal_type(w: &mut JsonWriter, lit: &ast::TSLiteralType<'_>, ctx: &Ctx<'_>) {
    match lit {
        ast::TSLiteralType::TemplateLiteral(template) => {
            node_header(w, "TSLiteralType", template.span, ctx);
            w.raw(",\"literal\":");
            // A template-literal type: node type "TemplateLiteral", but its
            // expressions are types.
            node_header(w, "TemplateLiteral", template.span, ctx);
            w.raw(",\"expressions\":");
            write_array(w, ctx.view.ty_list(template.types), |w, &t| {
                write_type(w, ctx.view.ty(t), ctx);
            });
            w.raw(",\"quasis\":");
            write_array(w, ctx.view.quasis(template.quasis), |w, q| {
                write_template_element(w, q, ctx);
            });
            // Close the inner `TemplateLiteral`, then the `TSLiteralType`.
            close_node(w, "TemplateLiteral", template.span, ctx);
            close_node(w, "TSLiteralType", template.span, ctx);
        }
        ast::TSLiteralType::String(literal)
        | ast::TSLiteralType::Number(literal)
        | ast::TSLiteralType::BigInt(literal) => {
            node_header(w, "TSLiteralType", literal.span, ctx);
            w.raw(",\"literal\":");
            write_literal(w, literal, ctx);
            close_node(w, "TSLiteralType", literal.span, ctx);
        }
        ast::TSLiteralType::UnaryExpression(unary) => {
            // Negative number types like `-1`; the parser guarantees the
            // argument is a Literal.
            #[expect(clippy::unreachable)]
            let ast::ExpressionKind::Literal(arg_lit) = &ctx.view.expr(unary.argument).kind else {
                unreachable!(
                    "parser only creates TSLiteralType::UnaryExpression with Literal argument"
                )
            };
            node_header(w, "TSLiteralType", unary.span, ctx);
            w.raw(",\"literal\":");
            node_header(w, "UnaryExpression", unary.span, ctx);
            w.raw(",\"operator\":");
            w.token(unary.operator.as_str());
            // Constant: acorn's `UnaryExpression` is always prefix.
            w.raw_fixed(b",\"prefix\":true,\"argument\":");
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
fn write_keyword_type(w: &mut JsonWriter, kw: &ast::TSKeywordType, ctx: &Ctx<'_>) {
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
pub(super) fn write_entity_name(w: &mut JsonWriter, name: &ast::TSEntityName<'_>, ctx: &Ctx<'_>) {
    match name {
        ast::TSEntityName::Identifier(id) => write_identifier_plain(w, id, ctx),
        &ast::TSEntityName::QualifiedName(_, qn) => {
            write_qualified_name(w, ctx.view.qual_name(qn), ctx);
        }
    }
}

/// Emits a `TSQualifiedName` node (`left`, `right`) and the qualified names down its
/// `left` side, written without recursing down that side — a dotted name's length is
/// bounded only by `SPINE_LIMIT`, so a recursion per link would spend a frame per `.`.
/// Each name's header and `left` key outermost first, then the leftmost identifier, then
/// each name's `right` and close innermost first: the bytes, and the order of the
/// comment-attach node events, of the recursion down the left.
#[inline(never)]
fn write_qualified_name(w: &mut JsonWriter, qn: &ast::TSQualifiedName<'_>, ctx: &Ctx<'_>) {
    let mut spine: SmallVec<[&ast::TSQualifiedName<'_>; 8]> = SmallVec::new();
    let mut node = qn;
    loop {
        node_header(w, "TSQualifiedName", node.span, ctx);
        w.raw(",\"left\":");
        spine.push(node);
        match &node.left {
            &ast::TSEntityName::QualifiedName(_, left) => node = ctx.view.qual_name(left),
            ast::TSEntityName::Identifier(id) => {
                write_identifier_plain(w, id, ctx);
                break;
            }
        }
    }
    for node in spine.iter().rev() {
        w.raw(",\"right\":");
        write_identifier_plain(w, &node.right, ctx);
        close_node(w, "TSQualifiedName", node.span, ctx);
    }
}

/// One link of an array / indexed-access type chain ([`write_postfix_type_chain`]).
#[derive(Clone, Copy)]
enum PostfixTypeLink<'a> {
    Array(&'a ast::TSArrayType),
    IndexedAccess(&'a ast::TSIndexedAccessType),
}

/// Emits an array or indexed-access type and the array and indexed-access types down its
/// element / object side (`A[][]`, `A['k']['k']`, mixed), written without recursing down
/// that side — the chain's length is bounded only by `SPINE_LIMIT`, so a recursion per
/// link would spend a frame per suffix. Each link's header and element / object key
/// outermost first, then the innermost element / object through [`write_type`], then
/// each link's index type (an indexed access) and close innermost first: the bytes, and
/// the order of the comment-attach node events, of the recursion down that side.
#[inline(never)]
fn write_postfix_type_chain(w: &mut JsonWriter, ts_type: &ast::TSType<'_>, ctx: &Ctx<'_>) {
    let mut spine: SmallVec<[PostfixTypeLink<'_>; 8]> = SmallVec::new();
    let mut node = ts_type;
    loop {
        match node {
            ast::TSType::Array(arr) => {
                node_header(w, "TSArrayType", arr.span, ctx);
                w.raw(",\"elementType\":");
                spine.push(PostfixTypeLink::Array(arr));
                node = ctx.view.ty(arr.element_type);
            }
            ast::TSType::IndexedAccess(i) => {
                node_header(w, "TSIndexedAccessType", i.span, ctx);
                w.raw(",\"objectType\":");
                spine.push(PostfixTypeLink::IndexedAccess(i));
                node = ctx.view.ty(i.object_type);
            }
            _ => {
                write_type(w, node, ctx);
                break;
            }
        }
    }
    for link in spine.iter().rev() {
        match *link {
            PostfixTypeLink::Array(arr) => close_node(w, "TSArrayType", arr.span, ctx),
            PostfixTypeLink::IndexedAccess(i) => {
                w.raw(",\"indexType\":");
                write_type(w, ctx.view.ty(i.index_type), ctx);
                close_node(w, "TSIndexedAccessType", i.span, ctx);
            }
        }
    }
}

/// Emits a `TSTypeParameterInstantiation` node.
pub(super) fn write_type_parameter_instantiation(
    w: &mut JsonWriter,
    params: &ast::TSTypeParameterInstantiation,
    ctx: &Ctx<'_>,
) {
    node_header(w, "TSTypeParameterInstantiation", params.span, ctx);
    w.raw(",\"params\":");
    write_array(w, ctx.view.ty_list(params.params), |w, &p| {
        write_type(w, ctx.view.ty(p), ctx);
    });
    close_node(w, "TSTypeParameterInstantiation", params.span, ctx);
}

/// Emit a `TSTypeElement` (property/method/call/construct/index signature).
fn write_type_element(w: &mut JsonWriter, elem: &ast::TSTypeElement, ctx: &Ctx<'_>) {
    match elem {
        ast::TSTypeElement::PropertySignature(p) => {
            // Field order: `readonly` (only when true), `computed?`, `key`,
            // `optional` (only when true), `typeAnnotation?`.
            node_header(w, "TSPropertySignature", p.span, ctx);
            if p.readonly {
                w.raw(",\"readonly\":true");
            }
            // acorn quirk: omits `computed` when the key is the `new` keyword
            // and the signature is neither computed nor readonly.
            let is_new_key = matches!(&ctx.view.expr(p.key).kind, ast::ExpressionKind::Identifier(id)
                if id.name(ctx.source, ctx.view) == "new");
            if !p.computed && !p.readonly && is_new_key {
                w.raw(",\"key\":");
            } else {
                w.raw_pick(
                    p.computed,
                    b",\"computed\":true,\"key\":",
                    b",\"computed\":false,\"key\":",
                );
            }
            write_expression(w, ctx.view.expr(p.key), ctx);
            if p.optional {
                w.raw(",\"optional\":true");
            }
            write_type_annotation_field(w, p.type_annotation.as_ref(), ctx);
            close_node(w, "TSPropertySignature", p.span, ctx);
        }
        ast::TSTypeElement::MethodSignature(m) => {
            // Field order: `readonly` (only when true), `computed`, `key`, then
            // per kind — a get/set signature's `kind` is assigned between the
            // two `parsePropertyName` calls (right after the key); a plain
            // method's `kind` lands on the finished signature (last). Then
            // `optional` (only when true), `typeParameters?`, `parameters`,
            // `typeAnnotation?` (the return type's wire name).
            //
            // `readonly` has no acorn counterpart — acorn-typescript rejects the
            // modifier on a method signature — so it is placed as on
            // `TSPropertySignature`, and an acorn-valid input's wire never holds it.
            node_header(w, "TSMethodSignature", m.span, ctx);
            if m.readonly {
                w.raw(",\"readonly\":true");
            }
            w.raw(",\"computed\":");
            w.bool(m.computed);
            w.raw(",\"key\":");
            write_expression(w, ctx.view.expr(m.key), ctx);
            let getset = matches!(m.kind, ast::MethodKind::Get | ast::MethodKind::Set);
            if getset {
                w.raw(",\"kind\":");
                w.token(match m.kind {
                    ast::MethodKind::Get => "get",
                    _ => "set",
                });
            }
            if m.optional {
                w.raw(",\"optional\":true");
            }
            write_type_parameters_field(w, m.type_parameters.as_ref(), ctx);
            w.raw(",\"parameters\":");
            write_expressions(w, ctx.view.run(m.params), ctx);
            write_type_annotation_field(w, m.return_type.as_ref(), ctx);
            if !getset {
                w.raw(",\"kind\":\"method\"");
            }
            close_node(w, "TSMethodSignature", m.span, ctx);
        }
        ast::TSTypeElement::CallSignature(c) => {
            write_signature_declaration(
                w,
                "TSCallSignatureDeclaration",
                c.span,
                c.type_parameters.as_ref(),
                ctx.view.run(c.params),
                c.return_type.as_ref(),
                ctx,
            );
        }
        ast::TSTypeElement::ConstructSignature(c) => {
            write_signature_declaration(
                w,
                "TSConstructSignatureDeclaration",
                c.span,
                c.type_parameters.as_ref(),
                ctx.view.run(c.params),
                c.return_type.as_ref(),
                ctx,
            );
        }
        ast::TSTypeElement::IndexSignature(i) => write_index_signature(w, i, ctx),
    }
}

/// Shared call/construct signature shape: `typeParameters?`, `parameters`,
/// `typeAnnotation?` (the return type's wire name).
fn write_signature_declaration(
    w: &mut JsonWriter,
    node_type: &'static str,
    span: tsv_lang::Span,
    type_parameters: Option<&ast::TSTypeParameterDeclaration>,
    params: &[ast::Expression<'_>],
    return_type: Option<&ast::TSTypeAnnotation>,
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
/// true), `parameters`, `typeAnnotation`, `decorators` (a class member's, only when present).
pub(super) fn write_index_signature(
    w: &mut JsonWriter,
    sig: &ast::TSIndexSignature,
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
    write_array(w, ctx.view.index_params(sig.parameters), |w, p| {
        // Index-signature parameters carry the binding's optional flag and
        // type annotation, never decorators.
        write_identifier_parts(
            w,
            p.span,
            p.ident_name(),
            p.optional,
            p.type_annotation(ctx.view),
            None,
            ctx,
        );
    });
    // A typeless index signature (`[key: string]`) omits `typeAnnotation`
    // entirely — acorn parses it that way (the missing value type is a deferred
    // tsc checker error, TS1021, not a parse error).
    if let Some(type_annotation) = &sig.type_annotation {
        w.raw(",\"typeAnnotation\":");
        write_type_annotation(w, type_annotation, ctx);
    }
    // A class index signature's decorators come last, as acorn-typescript attaches
    // them once it has built the member.
    write_decorators_field(w, ctx.view.opt_decorators(sig.decorators), ctx);
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
    iface: &ast::TSInterfaceDeclaration<'_>,
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
        write_array(w, ctx.view.heritages(iface.extends), |w, h| {
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
    write_array(w, ctx.view.type_members(iface.body.body), |w, m| {
        write_type_element(w, m, ctx);
    });
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
    func: &ast::TSDeclareFunction<'_>,
    ctx: &Ctx<'_>,
    exported: bool,
) {
    node_header(w, "TSDeclareFunction", func.span, ctx);
    if func.declare && !exported {
        w.raw(",\"declare\":true");
    }
    w.raw(",\"id\":");
    write_identifier_plain(w, &func.id, ctx);
    write_function_flags_fields(w, func.generator, func.r#async);
    write_type_parameters_field(w, func.type_parameters.as_ref(), ctx);
    w.raw(",\"params\":");
    write_expressions(w, ctx.view.run(func.params), ctx);
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
    enum_decl: &ast::TSEnumDeclaration<'_>,
    span: tsv_lang::Span,
    ctx: &Ctx<'_>,
) {
    node_header(w, "TSEnumDeclaration", span, ctx);
    if enum_decl.r#const {
        w.raw(",\"const\":true");
    }
    if enum_decl.declare {
        w.raw(",\"declare\":true");
    }
    w.raw(",\"id\":");
    write_identifier_plain(w, &enum_decl.id, ctx);
    w.raw(",\"members\":");
    write_array(w, ctx.view.enum_members(enum_decl.members), |w, member| {
        node_header(w, "TSEnumMember", member.span, ctx);
        w.raw(",\"id\":");
        match &member.id {
            ast::TSEnumMemberId::Identifier(id) => write_identifier_plain(w, id, ctx),
            ast::TSEnumMemberId::String(lit) => write_literal(w, lit, ctx),
        }
        if let Some(init) = ctx.view.opt(member.initializer) {
            w.raw(",\"initializer\":");
            write_expression(w, init, ctx);
        }
        close_node(w, "TSEnumMember", member.span, ctx);
    });
    close_node(w, "TSEnumDeclaration", span, ctx);
}
