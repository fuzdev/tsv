// Type alias, function, and class declaration writers — the writer twin of
// `convert::declarations`.

use super::super::super::internal;
use super::super::Schema;
use super::expressions::{ExprFlags, write_expression, write_expression_inner, write_expressions};
use super::statements::{write_block_statement, write_statement};
use super::types::{write_index_signature, write_type, write_type_parameter_instantiation};
use super::{
    Ctx, JsonWriter, node_header, write_array, write_identifier_plain,
    write_identifier_with_optional, write_name, write_or_null, write_return_type_field,
    write_type_annotation_field, write_type_parameters_field,
};
use tsv_lang::Span;

/// Mirrors `convert_decorator`: an unparenthesized decorator's call/member
/// spine omits `optional` (`strip_decorator_spine_optional`); a parenthesized
/// `@(expr)` rides the full expression parser and keeps it. Parens are
/// stripped from the expression, so the only signal is the span gap.
pub(super) fn write_decorator(
    w: &mut JsonWriter,
    decorator: &internal::Decorator<'_>,
    ctx: &Ctx<'_>,
) {
    let parenthesized = decorator.span.end > decorator.expression.span().end;
    node_header(w, "Decorator", decorator.span, ctx);
    w.raw(",\"expression\":");
    write_expression_inner(
        w,
        &decorator.expression,
        ctx,
        ExprFlags {
            in_chain: false,
            force_optional: false,
            strip_optional: !parenthesized,
        },
    );
    w.raw("}");
}

/// Emit a `decorators` field when the internal node carries decorators
/// (`Option<Vec>` with skip-if-none: present ⇒ emitted, even empty).
fn write_decorators_field(
    w: &mut JsonWriter,
    decorators: Option<&[internal::Decorator<'_>]>,
    ctx: &Ctx<'_>,
) {
    if let Some(decs) = decorators {
        w.raw(",\"decorators\":");
        write_array(w, decs, |w, d| write_decorator(w, d, ctx));
    }
}

/// Mirrors `convert_type_alias_declaration`. Field order: `id`,
/// `typeParameters?`, `typeAnnotation`, `declare` (only when true).
pub(super) fn write_type_alias_declaration(
    w: &mut JsonWriter,
    type_alias: &internal::TSTypeAliasDeclaration<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "TSTypeAliasDeclaration", type_alias.span, ctx);
    w.raw(",\"id\":");
    write_identifier_plain(w, &type_alias.id, ctx);
    write_type_parameters_field(w, type_alias.type_parameters.as_ref(), ctx);
    w.raw(",\"typeAnnotation\":");
    write_type(w, &type_alias.type_annotation, ctx);
    if type_alias.declare {
        w.raw(",\"declare\":true");
    }
    w.raw("}");
}

/// Mirrors `convert_function_declaration`. Field order: `id` (nullable),
/// `expression` (always false), `generator`, `async`, `typeParameters?`,
/// `params`, `returnType?`, `body`.
pub(super) fn write_function_declaration(
    w: &mut JsonWriter,
    func_decl: &internal::FunctionDeclaration<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "FunctionDeclaration", func_decl.span, ctx);
    w.raw(",\"id\":");
    write_or_null(w, func_decl.id.as_ref(), |w, id| {
        write_identifier_plain(w, id, ctx);
    });
    w.raw(",\"expression\":false,\"generator\":");
    w.bool(func_decl.generator);
    w.raw(",\"async\":");
    w.bool(func_decl.r#async);
    write_type_parameters_field(w, func_decl.type_parameters.as_ref(), ctx);
    w.raw(",\"params\":");
    write_expressions(w, func_decl.params, ctx);
    write_return_type_field(w, func_decl.return_type.as_ref(), ctx);
    w.raw(",\"body\":");
    write_block_statement(w, &func_decl.body, ctx);
    w.raw("}");
}

/// The super-class wrap decision (mirrors `maybe_wrap_super_class`): when
/// `extends Base<T>` sits on a different line from the closing `>` of the type
/// parameters, acorn-typescript emits `superClass` as a
/// `TSInstantiationExpression` consuming `superTypeParameters`. Returns the
/// combined span to wrap with, or `None` for the plain shape. All offsets are
/// internal byte offsets (the same-line scan byte-indexes `source`).
fn super_class_wrap_span(
    type_params_span: Option<Span>,
    super_class: Option<&internal::Expression<'_>>,
    super_type_parameters_end: Option<u32>,
    source: &str,
) -> Option<Span> {
    let tp_span = type_params_span?;
    // The public super-class node starts at the JsdocCast-unwrapped inner
    // expression (mirrors `converted_start`).
    let sc_start = super_class.map(|e| e.unwrap_jsdoc_casts().span().start)?;
    let stp_end = super_type_parameters_end?;
    if tsv_lang::printing::is_same_line(source, tp_span.end, sc_start) {
        return None;
    }
    Some(Span::new(sc_start, stp_end))
}

/// Emit the `superClass` (nullable) and `superTypeParameters?` fields, applying
/// the wrap decision. Shared by class declarations and expressions.
fn write_super_class_fields(
    w: &mut JsonWriter,
    super_class: Option<&internal::Expression<'_>>,
    super_type_parameters: Option<&internal::TSTypeParameterInstantiation<'_>>,
    type_params_span: Option<Span>,
    ctx: &Ctx<'_>,
) {
    let wrap_span = super_class_wrap_span(
        type_params_span,
        super_class,
        super_type_parameters.map(|tp| tp.span.end),
        ctx.source,
    );
    w.raw(",\"superClass\":");
    if let (Some(e), Some(stp), Some(ws)) = (super_class, super_type_parameters, wrap_span) {
        // Wrapped: `superTypeParameters` is consumed into the wrapper.
        node_header(w, "TSInstantiationExpression", ws, ctx);
        w.raw(",\"expression\":");
        write_expression(w, e, ctx);
        w.raw(",\"typeArguments\":");
        write_type_parameter_instantiation(w, stp, ctx);
        w.raw("}");
    } else {
        write_or_null(w, super_class, |w, e| write_expression(w, e, ctx));
        if let Some(stp) = super_type_parameters {
            w.raw(",\"superTypeParameters\":");
            write_type_parameter_instantiation(w, stp, ctx);
        }
    }
}

/// Mirrors `convert_class_declaration`. Field order: `decorators?`,
/// `declare?`, `abstract?`, `id` (nullable), `typeParameters?`, `superClass`
/// (nullable), `superTypeParameters?`, `implements?`, `body`.
pub(super) fn write_class_declaration(
    w: &mut JsonWriter,
    class_decl: &internal::ClassDeclaration<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "ClassDeclaration", class_decl.span, ctx);
    write_decorators_field(w, class_decl.decorators, ctx);
    if class_decl.declare {
        w.raw(",\"declare\":true");
    }
    if class_decl.r#abstract {
        w.raw(",\"abstract\":true");
    }
    w.raw(",\"id\":");
    write_or_null(w, class_decl.id.as_ref(), |w, id| {
        write_identifier_plain(w, id, ctx);
    });
    write_type_parameters_field(w, class_decl.type_parameters.as_ref(), ctx);
    write_super_class_fields(
        w,
        class_decl.super_class,
        class_decl.super_type_parameters.as_ref(),
        class_decl.type_parameters.as_ref().map(|tp| tp.span),
        ctx,
    );
    write_implements_field(w, class_decl.implements, ctx);
    w.raw(",\"body\":");
    write_class_body(w, &class_decl.body, ctx);
    w.raw("}");
}

/// Mirrors `convert_class_expression` (no `declare` field).
pub(super) fn write_class_expression(
    w: &mut JsonWriter,
    class_expr: &internal::ClassExpression<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "ClassExpression", class_expr.span, ctx);
    write_decorators_field(w, class_expr.decorators, ctx);
    if class_expr.r#abstract {
        w.raw(",\"abstract\":true");
    }
    w.raw(",\"id\":");
    write_or_null(w, class_expr.id.as_ref(), |w, id| {
        write_identifier_plain(w, id, ctx);
    });
    write_type_parameters_field(w, class_expr.type_parameters.as_ref(), ctx);
    write_super_class_fields(
        w,
        class_expr.super_class,
        class_expr.super_type_parameters.as_ref(),
        class_expr.type_parameters.as_ref().map(|tp| tp.span),
        ctx,
    );
    write_implements_field(w, class_expr.implements, ctx);
    w.raw(",\"body\":");
    write_class_body(w, &class_expr.body, ctx);
    w.raw("}");
}

/// `implements` is `Option<Vec>` skipped when convert leaves it `None` (an
/// empty internal list).
fn write_implements_field(
    w: &mut JsonWriter,
    implements: &[internal::TSInterfaceHeritage<'_>],
    ctx: &Ctx<'_>,
) {
    if implements.is_empty() {
        return;
    }
    w.raw(",\"implements\":");
    write_array(w, implements, |w, h| {
        write_expression_with_type_arguments(w, h, ctx);
    });
}

/// Mirrors `convert_class_body` + `convert_class_member`.
fn write_class_body(w: &mut JsonWriter, body: &internal::ClassBody<'_>, ctx: &Ctx<'_>) {
    node_header(w, "ClassBody", body.span, ctx);
    w.raw(",\"body\":");
    write_array(w, body.body, |w, m| match m {
        internal::ClassMember::MethodDefinition(method) => {
            write_method_definition(w, method, ctx);
        }
        internal::ClassMember::PropertyDefinition(prop) => {
            write_property_definition(w, prop, ctx);
        }
        internal::ClassMember::StaticBlock(block) => {
            // Always TypeScript class context.
            node_header(w, "StaticBlock", block.span, ctx);
            w.raw(",\"body\":");
            write_array(w, block.body, |w, s| {
                write_statement(w, s, ctx, Schema::Acorn);
            });
            w.raw("}");
        }
        internal::ClassMember::IndexSignature(sig) => {
            write_index_signature(w, sig, ctx);
        }
    });
    w.raw("}");
}

/// Mirrors `convert_method_definition`. Field order: `decorators?`,
/// `accessibility?`, `abstract?`, `static`, `override` (only when true),
/// `optional?`, `computed`, `key`, `kind`, `typeParameters?` (moved here from
/// the FunctionExpression, acorn convention), `value`.
fn write_method_definition(
    w: &mut JsonWriter,
    method: &internal::MethodDefinition<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "MethodDefinition", method.span, ctx);
    write_decorators_field(w, method.decorators, ctx);
    if let Some(acc) = method.accessibility {
        w.raw(",\"accessibility\":");
        w.token(acc.as_str());
    }
    if method.r#abstract {
        w.raw(",\"abstract\":true");
    }
    w.raw(",\"static\":");
    w.bool(method.is_static);
    if method.r#override {
        w.raw(",\"override\":true");
    }
    if method.optional {
        w.raw(",\"optional\":true");
    }
    w.raw(",\"computed\":");
    w.bool(method.computed);
    w.raw(",\"key\":");
    write_expression(w, &method.key, ctx);
    w.raw(",\"kind\":");
    w.token(method.kind.as_str());
    let func = &method.value;
    write_type_parameters_field(w, func.type_parameters.as_ref(), ctx);
    // Abstract methods and overload signatures emit TSDeclareMethod (no
    // body): abstract flag OR an empty body with a zero-width (synthetic)
    // span. The value span starts at the `(`, not at the method keyword.
    // Both value shapes are otherwise identical — typeParameters stays on
    // the MethodDefinition, never on the value node.
    let is_bodyless = method.r#abstract
        || (func.body.body.is_empty() && func.body.span.start == func.body.span.end);
    let (value_type, body) = if is_bodyless {
        ("TSDeclareMethod", None)
    } else {
        ("FunctionExpression", Some(&func.body))
    };
    w.raw(",\"value\":");
    node_header(
        w,
        value_type,
        Span::new(func.params_start, func.span.end),
        ctx,
    );
    w.raw(",\"id\":");
    write_or_null(w, func.id.as_ref(), |w, id| {
        write_identifier_with_optional(w, id, ctx);
    });
    w.raw(",\"expression\":false,\"generator\":");
    w.bool(func.generator);
    w.raw(",\"async\":");
    w.bool(func.r#async);
    w.raw(",\"params\":");
    write_expressions(w, func.params, ctx);
    write_return_type_field(w, func.return_type.as_ref(), ctx);
    if let Some(body) = body {
        w.raw(",\"body\":");
        write_block_statement(w, body, ctx);
    }
    w.raw("}}");
}

/// Mirrors `convert_property_definition`. Field order: `decorators?`,
/// `abstract?`, `accessor?`, `accessibility?`, `readonly?`, `override?`,
/// `declare?`, `static`, `computed`, `key`, `optional?`, `definite?`,
/// `typeAnnotation?`, `value` (nullable).
fn write_property_definition(
    w: &mut JsonWriter,
    prop: &internal::PropertyDefinition<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "PropertyDefinition", prop.span, ctx);
    write_decorators_field(w, prop.decorators, ctx);
    if prop.r#abstract {
        w.raw(",\"abstract\":true");
    }
    if prop.accessor {
        w.raw(",\"accessor\":true");
    }
    if let Some(acc) = prop.accessibility {
        w.raw(",\"accessibility\":");
        w.token(acc.as_str());
    }
    if prop.readonly {
        w.raw(",\"readonly\":true");
    }
    if prop.r#override {
        w.raw(",\"override\":true");
    }
    if prop.declare {
        w.raw(",\"declare\":true");
    }
    w.raw(",\"static\":");
    w.bool(prop.is_static);
    w.raw(",\"computed\":");
    w.bool(prop.computed);
    w.raw(",\"key\":");
    write_expression(w, &prop.key, ctx);
    if matches!(prop.modifier, internal::PropertyModifier::Optional) {
        w.raw(",\"optional\":true");
    }
    if matches!(prop.modifier, internal::PropertyModifier::Definite) {
        w.raw(",\"definite\":true");
    }
    write_type_annotation_field(w, prop.type_annotation.as_ref(), ctx);
    w.raw(",\"value\":");
    write_or_null(w, prop.value.as_ref(), |w, v| write_expression(w, v, ctx));
    w.raw("}");
}

/// Mirrors `convert_type_parameter_declaration` (and its byte-identical
/// `_simple` sibling — one writer serves both call-site families). Field
/// order: `params`, `extra?` (`{"trailingComma":N}`, emitted like
/// `start`/`end` in the mapper's output space).
pub(super) fn write_type_parameter_declaration(
    w: &mut JsonWriter,
    params: &internal::TSTypeParameterDeclaration<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "TSTypeParameterDeclaration", params.span, ctx);
    w.raw(",\"params\":");
    write_array(w, params.params, |w, p| write_type_parameter(w, p, ctx));
    if let Some(pos) = params.trailing_comma {
        w.raw(",\"extra\":{\"trailingComma\":");
        w.u32(ctx.loc.pos(pos));
        w.raw("}");
    }
    w.raw("}");
}

/// Mirrors `convert_type_parameter`. Field order: `const`/`in`/`out` (each
/// only when true), `name`, `constraint?`, `default?`.
pub(super) fn write_type_parameter(
    w: &mut JsonWriter,
    param: &internal::TSTypeParameter<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "TSTypeParameter", param.span, ctx);
    if param.is_const {
        w.raw(",\"const\":true");
    }
    if param.is_in {
        w.raw(",\"in\":true");
    }
    if param.is_out {
        w.raw(",\"out\":true");
    }
    w.raw(",\"name\":");
    write_name(w, param.name.span, param.name.name, ctx);
    if let Some(c) = &param.constraint {
        w.raw(",\"constraint\":");
        write_type(w, c, ctx);
    }
    if let Some(d) = &param.default {
        w.raw(",\"default\":");
        write_type(w, d, ctx);
    }
    w.raw("}");
}

/// Mirrors `convert_expression_with_type_arguments` (implements clause) +
/// `convert_entity_name_to_expression`.
fn write_expression_with_type_arguments(
    w: &mut JsonWriter,
    heritage: &internal::TSInterfaceHeritage<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "TSExpressionWithTypeArguments", heritage.span, ctx);
    w.raw(",\"expression\":");
    write_entity_name_to_expression(w, &heritage.expression, ctx);
    if let Some(ta) = &heritage.type_arguments {
        w.raw(",\"typeParameters\":");
        write_type_parameter_instantiation(w, ta, ctx);
    }
    w.raw("}");
}

/// Mirrors `convert_entity_name_to_expression`: `Foo` emits an `Identifier`
/// (carrying the binding's `optional` flag), `Foo.Bar` a `MemberExpression`
/// with `computed:false, optional:false`.
fn write_entity_name_to_expression(
    w: &mut JsonWriter,
    entity: &internal::TSEntityName<'_>,
    ctx: &Ctx<'_>,
) {
    match entity {
        internal::TSEntityName::Identifier(id) => write_identifier_with_optional(w, id, ctx),
        internal::TSEntityName::QualifiedName(qn) => {
            node_header(w, "MemberExpression", qn.span, ctx);
            w.raw(",\"object\":");
            write_entity_name_to_expression(w, &qn.left, ctx);
            w.raw(",\"property\":");
            write_identifier_with_optional(w, &qn.right, ctx);
            w.raw(",\"computed\":false,\"optional\":false}");
        }
    }
}
