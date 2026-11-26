// Type alias, function, and class declaration writers.

use super::expressions::{
    ChainState, ExprFlags, write_expression, write_expression_inner, write_expressions,
};
use super::statements::{write_block_statement, write_statement};
use super::types::{
    write_entity_name, write_index_signature, write_type, write_type_parameter_instantiation,
};
use super::{
    Ctx, Sink, close_node, node_header, write_array, write_function_flags_fields,
    write_identifier_plain, write_identifier_with_optional, write_name_field, write_or_null,
    write_return_type_field, write_type_annotation_field, write_type_parameters_field,
};
use crate::ast;
use crate::ast::AstView;
use tsv_lang::Span;

/// Emits a `Decorator` node: an unparenthesized decorator's call/member
/// spine omits `optional`; a parenthesized `@(expr)` rides the full expression
/// parser and keeps it. Parens are stripped from the expression, so the only
/// signal is the span gap.
pub(super) fn write_decorator(w: &mut Sink<'_>, decorator: &ast::Decorator, ctx: &Ctx<'_>) {
    let parenthesized = decorator.span.end > ctx.view.expr(decorator.expression).span().end;
    node_header!(w, "Decorator", decorator.span, ctx);
    raw!(w, "Decorator", ",\"expression\":");
    write_expression_inner(
        w,
        ctx.view.expr(decorator.expression),
        ctx,
        ExprFlags {
            // A decorator expression is a fresh chain root — nothing above it
            // has resolved its spine either way.
            chain: ChainState::Unresolved,
            force_optional: false,
            strip_optional: !parenthesized,
        },
    );
    close_node(w, "Decorator", decorator.span, ctx);
}

/// Emit a `decorators` field when the internal node carries decorators
/// (`Option<Vec>` with skip-if-none: present ⇒ emitted, even empty).
pub(super) fn write_decorators_field<const E: u16>(
    w: &mut Sink<'_>,
    decorators: Option<&[ast::Decorator]>,
    ctx: &Ctx<'_>,
) {
    if let Some(decs) = decorators {
        raw!(w, @E, ",\"decorators\":");
        write_array(w, decs, |w, d| write_decorator(w, d, ctx));
    }
}

/// Emits a `TSTypeAliasDeclaration` node. Field order: `id`,
/// `typeParameters?`, `typeAnnotation`, `declare` (only when true).
pub(super) fn write_type_alias_declaration(
    w: &mut Sink<'_>,
    type_alias: &ast::TSTypeAliasDeclaration,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "TSTypeAliasDeclaration", span, ctx);
    raw!(w, "TSTypeAliasDeclaration", ",\"id\":");
    write_identifier_plain(w, &type_alias.id, ctx);
    write_type_parameters_field::<{ at!("TSTypeAliasDeclaration") }>(
        w,
        type_alias.type_parameters.as_ref(),
        ctx,
    );
    raw!(w, "TSTypeAliasDeclaration", ",\"typeAnnotation\":");
    write_type(w, ctx.view.ty(type_alias.type_annotation), ctx);
    if type_alias.declare {
        raw!(w, "TSTypeAliasDeclaration", ",\"declare\":true");
    }
    close_node(w, "TSTypeAliasDeclaration", span, ctx);
}

/// Emits a `FunctionDeclaration` node. Field order: `id` (nullable),
/// `expression` (always false), `generator`, `async`, `typeParameters?`,
/// `params`, `returnType?`, `body`.
pub(super) fn write_function_declaration(
    w: &mut Sink<'_>,
    func_decl: &ast::FunctionDeclaration,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "FunctionDeclaration", func_decl.span, ctx);
    raw!(w, "FunctionDeclaration", ",\"id\":");
    write_or_null(w, func_decl.id.as_ref(), |w, id| {
        write_identifier_plain(w, id, ctx);
    });
    write_function_flags_fields::<{ at!("FunctionDeclaration") }>(
        w,
        func_decl.generator,
        func_decl.r#async,
    );
    write_type_parameters_field::<{ at!("FunctionDeclaration") }>(
        w,
        func_decl.type_parameters.as_ref(),
        ctx,
    );
    raw!(w, "FunctionDeclaration", ",\"params\":");
    write_expressions(w, ctx.view.exprs(func_decl.params), ctx);
    write_return_type_field::<{ at!("FunctionDeclaration") }>(
        w,
        func_decl.return_type.as_ref(),
        ctx,
    );
    raw!(w, "FunctionDeclaration", ",\"body\":");
    write_block_statement(w, &func_decl.body, ctx);
    close_node(w, "FunctionDeclaration", func_decl.span, ctx);
}

/// The super-class wrap decision: when
/// `extends Base<T>` sits on a different line from the closing `>` of the type
/// parameters, acorn-typescript emits `superClass` as a
/// `TSInstantiationExpression` consuming `superTypeParameters`. Returns the
/// combined span to wrap with, or `None` for the plain shape. All offsets are
/// internal byte offsets (the same-line scan byte-indexes `source`).
fn super_class_wrap_span(
    view: AstView<'_>,
    type_params_span: Option<Span>,
    super_class: Option<&ast::Expression>,
    super_type_parameters_end: Option<u32>,
    source: &str,
) -> Option<Span> {
    let tp_span = type_params_span?;
    // The public super-class node starts at the CommentBoundPair-unwrapped inner
    // expression.
    let sc_start = super_class.map(|e| e.unwrap_comment_bound_pairs(view).span().start)?;
    let stp_end = super_type_parameters_end?;
    if tsv_lang::printing::is_same_line(source, tp_span.end, sc_start) {
        return None;
    }
    Some(Span::new(sc_start, stp_end))
}

/// Emit the `superClass` (nullable) and `superTypeParameters?` fields, applying
/// the wrap decision. Shared by class declarations and expressions.
fn write_super_class_fields<const E: u16>(
    w: &mut Sink<'_>,
    super_class: Option<&ast::Expression>,
    super_type_parameters: Option<&ast::TSTypeParameterInstantiation>,
    type_params_span: Option<Span>,
    ctx: &Ctx<'_>,
) {
    let wrap_span = super_class_wrap_span(
        ctx.view,
        type_params_span,
        super_class,
        super_type_parameters.map(|tp| tp.span.end),
        ctx.source,
    );
    raw!(w, @E, ",\"superClass\":");
    if let (Some(e), Some(stp), Some(ws)) = (super_class, super_type_parameters, wrap_span) {
        // Wrapped: `superTypeParameters` is consumed into the wrapper.
        node_header!(w, "TSInstantiationExpression", ws, ctx);
        raw!(w, "TSInstantiationExpression", ",\"expression\":");
        write_expression(w, e, ctx);
        raw!(w, "TSInstantiationExpression", ",\"typeArguments\":");
        write_type_parameter_instantiation(w, stp, ctx);
        close_node(w, "TSInstantiationExpression", ws, ctx);
    } else {
        write_or_null(w, super_class, |w, e| write_expression(w, e, ctx));
        if let Some(stp) = super_type_parameters {
            raw!(w, @E, ",\"superTypeParameters\":");
            write_type_parameter_instantiation(w, stp, ctx);
        }
    }
}

/// Emits a `ClassDeclaration` node. Field order: `declare?`, `abstract?`, `decorators?`,
/// `id` (nullable), `typeParameters?`, `superClass` (nullable), `superTypeParameters?`,
/// `implements?`, `body` — acorn-typescript's statement route (`declare class`,
/// `tsTryParseDeclare` stamping `declare` before the class parses). Its other routes
/// (`declare abstract class`, `export declare class`) stamp `declare` on the finished node;
/// tsv's wire keeps the one order.
pub(super) fn write_class_declaration(
    w: &mut Sink<'_>,
    class_decl: &ast::ClassDeclaration,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "ClassDeclaration", class_decl.span, ctx);
    if class_decl.declare {
        raw!(w, "ClassDeclaration", ",\"declare\":true");
    }
    if class_decl.r#abstract {
        raw!(w, "ClassDeclaration", ",\"abstract\":true");
    }
    write_decorators_field::<{ at!("ClassDeclaration") }>(
        w,
        ctx.view.opt_decorators(class_decl.decorators),
        ctx,
    );
    raw!(w, "ClassDeclaration", ",\"id\":");
    write_or_null(w, class_decl.id.as_ref(), |w, id| {
        write_identifier_plain(w, id, ctx);
    });
    write_type_parameters_field::<{ at!("ClassDeclaration") }>(
        w,
        class_decl.type_parameters.as_ref(),
        ctx,
    );
    write_super_class_fields::<{ at!("ClassDeclaration") }>(
        w,
        ctx.view.opt_expr(class_decl.super_class),
        class_decl.super_type_parameters.as_ref(),
        class_decl.type_parameters.as_ref().map(|tp| tp.span),
        ctx,
    );
    write_implements_field::<{ at!("ClassDeclaration") }>(
        w,
        ctx.view.heritages(class_decl.implements),
        ctx,
    );
    raw!(w, "ClassDeclaration", ",\"body\":");
    write_class_body(w, &class_decl.body, ctx);
    close_node(w, "ClassDeclaration", class_decl.span, ctx);
}

/// Emits a `ClassExpression` node (no `declare` field).
pub(super) fn write_class_expression(
    w: &mut Sink<'_>,
    class_expr: &ast::ClassExpression,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "ClassExpression", span, ctx);
    write_decorators_field::<{ at!("ClassExpression") }>(
        w,
        ctx.view.opt_decorators(class_expr.decorators),
        ctx,
    );
    if class_expr.r#abstract {
        raw!(w, "ClassExpression", ",\"abstract\":true");
    }
    raw!(w, "ClassExpression", ",\"id\":");
    write_or_null(w, class_expr.id.as_ref(), |w, id| {
        write_identifier_plain(w, id, ctx);
    });
    write_type_parameters_field::<{ at!("ClassExpression") }>(
        w,
        class_expr.type_parameters.as_ref(),
        ctx,
    );
    write_super_class_fields::<{ at!("ClassExpression") }>(
        w,
        ctx.view.opt_expr(class_expr.super_class),
        class_expr.super_type_parameters.as_ref(),
        class_expr.type_parameters.as_ref().map(|tp| tp.span),
        ctx,
    );
    write_implements_field::<{ at!("ClassExpression") }>(
        w,
        ctx.view.heritages(class_expr.implements),
        ctx,
    );
    raw!(w, "ClassExpression", ",\"body\":");
    write_class_body(w, &class_expr.body, ctx);
    close_node(w, "ClassExpression", span, ctx);
}

/// The `implements` field is skip-if-empty — an empty internal list emits no
/// field.
fn write_implements_field<const E: u16>(
    w: &mut Sink<'_>,
    implements: &[ast::TSInterfaceHeritage],
    ctx: &Ctx<'_>,
) {
    if implements.is_empty() {
        return;
    }
    raw!(w, @E, ",\"implements\":");
    write_array(w, implements, |w, h| {
        write_expression_with_type_arguments(w, h, ctx);
    });
}

/// Emits a `ClassBody` node (dispatching each `ClassMember`).
fn write_class_body(w: &mut Sink<'_>, body: &ast::ClassBody, ctx: &Ctx<'_>) {
    node_header!(w, "ClassBody", body.span, ctx);
    raw!(w, "ClassBody", ",\"body\":");
    write_array(w, ctx.view.class_members(body.body), |w, m| match m {
        ast::ClassMember::MethodDefinition(method) => {
            write_method_definition(w, method, ctx);
        }
        ast::ClassMember::PropertyDefinition(prop) => {
            write_property_definition(w, prop, ctx);
        }
        ast::ClassMember::StaticBlock(block) => {
            node_header!(w, "StaticBlock", block.span, ctx);
            raw!(w, "StaticBlock", ",\"body\":");
            write_array(w, ctx.view.stmt_list(block.body), |w, s| {
                write_statement(w, ctx.view.stmt(*s), ctx);
            });
            // acorn-typescript attaches a member's decorators once it has built it,
            // so a static block's list follows its `body`.
            write_decorators_field::<{ at!("StaticBlock") }>(
                w,
                ctx.view.opt_decorators(block.decorators),
                ctx,
            );
            close_node(w, "StaticBlock", block.span, ctx);
        }
        ast::ClassMember::IndexSignature(sig) => {
            write_index_signature(w, sig, ctx);
        }
    });
    close_node(w, "ClassBody", body.span, ctx);
}

/// A class member's `accessibility` field, when present.
///
/// A class member's modifier fields take one order (the member writers' docs), where
/// acorn-typescript's `tsParseModifiers` stamps each as it reads its keyword, so its wire
/// follows the member's source order; `static` is (re-)assigned after that loop, last.
fn write_accessibility_field<const E: u16>(
    w: &mut Sink<'_>,
    accessibility: Option<ast::Accessibility>,
) {
    match accessibility {
        None => {}
        Some(ast::Accessibility::Public) => raw!(w, @E, ",\"accessibility\":\"public\""),
        Some(ast::Accessibility::Private) => raw!(w, @E, ",\"accessibility\":\"private\""),
        Some(ast::Accessibility::Protected) => {
            raw!(w, @E, ",\"accessibility\":\"protected\"");
        }
    }
}

/// Emits a `MethodDefinition` node. Field order: `abstract?`, `accessibility?`,
/// `override?`, `static`, `computed`, `key`, `optional?`, `kind`,
/// `typeParameters?` (moved here from the FunctionExpression, acorn
/// convention), `value`, `decorators?` (attached to the finished member, so
/// they serialize last).
fn write_method_definition(w: &mut Sink<'_>, method: &ast::MethodDefinition, ctx: &Ctx<'_>) {
    node_header!(w, "MethodDefinition", method.span, ctx);
    if method.r#abstract {
        raw!(w, "MethodDefinition", ",\"abstract\":true");
    }
    write_accessibility_field::<{ at!("MethodDefinition") }>(w, method.accessibility);
    if method.r#override {
        raw!(w, "MethodDefinition", ",\"override\":true");
    }
    raw!(w, "MethodDefinition", ",\"static\":");
    w.bool(method.is_static);
    raw!(w, "MethodDefinition", ",\"computed\":");
    w.bool(method.computed);
    raw!(w, "MethodDefinition", ",\"key\":");
    write_expression(w, ctx.view.expr(method.key), ctx);
    if method.optional {
        raw!(w, "MethodDefinition", ",\"optional\":true");
    }
    raw!(w, "MethodDefinition", ",\"kind\":");
    keyword!(
        w,
        token,
        method.kind => as_str,
        ast::MethodKind => [Constructor, Method, Get, Set],
        "MethodDefinition"."kind"
    );
    let func = &method.value;
    write_type_parameters_field::<{ at!("MethodDefinition") }>(
        w,
        func.type_parameters.as_ref(),
        ctx,
    );
    // Abstract methods and overload signatures emit TSDeclareMethod (no
    // body): abstract flag OR an empty body with a zero-width (synthetic)
    // span. The value span starts at the `(`, not at the method keyword.
    // Both value shapes are otherwise identical — typeParameters stays on
    // the MethodDefinition, never on the value node.
    let is_bodyless = method.r#abstract
        || (ctx.view.stmt_list(func.body.body).is_empty()
            && func.body.span.start == func.body.span.end);
    let (value_type, body) = if is_bodyless {
        ("TSDeclareMethod", None)
    } else {
        ("FunctionExpression", Some(&func.body))
    };
    raw!(w, "MethodDefinition", ",\"value\":");
    let value_span = Span::new(func.params_start, func.span.end);
    tape! {
        let entry = if is_bodyless {
            entry!("TSDeclareMethod")
        } else {
            entry!("FunctionExpression")
        };
    }
    node_header!(w, @dyn value_type => entry, value_span, ctx);
    raw!(w, @in entry => "TSDeclareMethod" | "FunctionExpression", ",\"id\":");
    write_or_null(w, func.id.as_ref(), |w, id| {
        write_identifier_with_optional(w, id, ctx);
    });
    json_only! { write_function_flags_fields::<0>(w, func.generator, func.r#async); }
    tape! {
        let (generator, r#async) = (func.generator, func.r#async);
        if is_bodyless {
            write_function_flags_fields::<{ entry!("TSDeclareMethod") }>(w, generator, r#async);
        } else {
            write_function_flags_fields::<{ entry!("FunctionExpression") }>(w, generator, r#async);
        }
    }
    raw!(w, @in entry => "TSDeclareMethod" | "FunctionExpression", ",\"params\":");
    write_expressions(w, ctx.view.exprs(func.params), ctx);
    json_only! { write_return_type_field::<0>(w, func.return_type.as_ref(), ctx); }
    tape! {
        let return_type = func.return_type.as_ref();
        if is_bodyless {
            write_return_type_field::<{ entry!("TSDeclareMethod") }>(w, return_type, ctx);
        } else {
            write_return_type_field::<{ entry!("FunctionExpression") }>(w, return_type, ctx);
        }
    }
    if let Some(body) = body {
        raw!(w, "FunctionExpression", ",\"body\":");
        write_block_statement(w, body, ctx);
    }
    // Close the `value` function node, then the `MethodDefinition`.
    close_node(w, value_type, value_span, ctx);
    write_decorators_field::<{ at!("MethodDefinition") }>(
        w,
        ctx.view.opt_decorators(method.decorators),
        ctx,
    );
    close_node(w, "MethodDefinition", method.span, ctx);
}

/// Emits a `PropertyDefinition` node. Field order: `declare?`, `abstract?`, `accessor?`,
/// `accessibility?`, `readonly?`, `override?`, `static`, `computed`,
/// `key`, `optional?`, `definite?`, `typeAnnotation?`, `value` (nullable),
/// `decorators?` (attached to the finished member, so they serialize last).
fn write_property_definition(w: &mut Sink<'_>, prop: &ast::PropertyDefinition, ctx: &Ctx<'_>) {
    node_header!(w, "PropertyDefinition", prop.span, ctx);
    if prop.declare {
        raw!(w, "PropertyDefinition", ",\"declare\":true");
    }
    if prop.r#abstract {
        raw!(w, "PropertyDefinition", ",\"abstract\":true");
    }
    if prop.accessor {
        raw!(w, "PropertyDefinition", ",\"accessor\":true");
    }
    write_accessibility_field::<{ at!("PropertyDefinition") }>(w, prop.accessibility);
    if prop.readonly {
        raw!(w, "PropertyDefinition", ",\"readonly\":true");
    }
    if prop.r#override {
        raw!(w, "PropertyDefinition", ",\"override\":true");
    }
    raw!(w, "PropertyDefinition", ",\"static\":");
    w.bool(prop.is_static);
    raw!(w, "PropertyDefinition", ",\"computed\":");
    w.bool(prop.computed);
    raw!(w, "PropertyDefinition", ",\"key\":");
    write_expression(w, ctx.view.expr(prop.key), ctx);
    if matches!(prop.modifier, ast::PropertyModifier::Optional) {
        raw!(w, "PropertyDefinition", ",\"optional\":true");
    }
    if matches!(prop.modifier, ast::PropertyModifier::Definite) {
        raw!(w, "PropertyDefinition", ",\"definite\":true");
    }
    write_type_annotation_field::<{ at!("PropertyDefinition") }>(
        w,
        prop.type_annotation.as_ref(),
        ctx,
    );
    raw!(w, "PropertyDefinition", ",\"value\":");
    write_or_null(w, prop.value.as_ref(), |w, v| {
        write_expression(w, ctx.view.expr(*v), ctx);
    });
    write_decorators_field::<{ at!("PropertyDefinition") }>(
        w,
        ctx.view.opt_decorators(prop.decorators),
        ctx,
    );
    close_node(w, "PropertyDefinition", prop.span, ctx);
}

/// Emits a `TSTypeParameterDeclaration` node. Field order: `params`, `extra?`
/// (`{"trailingComma":N}`, emitted like `start`/`end` in the mapper's output
/// space).
pub(super) fn write_type_parameter_declaration(
    w: &mut Sink<'_>,
    params: &ast::TSTypeParameterDeclaration,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "TSTypeParameterDeclaration", params.span, ctx);
    raw!(w, "TSTypeParameterDeclaration", ",\"params\":");
    write_array(w, ctx.view.type_params(params.params), |w, p| {
        write_type_parameter(w, p, ctx);
    });
    if let Some(pos) = params.trailing_comma {
        raw!(
            w,
            "TSTypeParameterDeclaration",
            ",\"extra\":{\"trailingComma\":"
        );
        w.u32(ctx.pos(pos));
        raw!(w, "}");
    }
    close_node(w, "TSTypeParameterDeclaration", params.span, ctx);
}

/// Emits a `TSTypeParameter` node. Field order: `const?`, `in?`, `out?`, `name`,
/// `constraint?`, `default?` — the order both formatters print the modifiers in.
/// (acorn-typescript's `tsParseModifiers` stamps each flag as it consumes the keyword, so
/// its wire follows the source's spelling; tsv's keeps the one order.)
pub(super) fn write_type_parameter(w: &mut Sink<'_>, param: &ast::TSTypeParameter, ctx: &Ctx<'_>) {
    node_header!(w, "TSTypeParameter", param.span, ctx);
    if param
        .modifiers
        .contains(ast::TSTypeParameterModifier::Const)
    {
        raw!(w, "TSTypeParameter", ",\"const\":true");
    }
    if param.modifiers.contains(ast::TSTypeParameterModifier::In) {
        raw!(w, "TSTypeParameter", ",\"in\":true");
    }
    if param.modifiers.contains(ast::TSTypeParameterModifier::Out) {
        raw!(w, "TSTypeParameter", ",\"out\":true");
    }
    write_name_field(w, param.name.ident_name(), param.name.span.start, ctx);
    if let Some(c) = ctx.view.opt_ty(param.constraint) {
        raw!(w, "TSTypeParameter", ",\"constraint\":");
        write_type(w, c, ctx);
    }
    if let Some(d) = ctx.view.opt_ty(param.default) {
        raw!(w, "TSTypeParameter", ",\"default\":");
        write_type(w, d, ctx);
    }
    close_node(w, "TSTypeParameter", param.span, ctx);
}

/// Emits a `TSExpressionWithTypeArguments` node (an implements clause): the
/// `expression` is the heritage entity name (an `Identifier` or a
/// `TSQualifiedName`, matching `extends`), plus `typeParameters?`.
fn write_expression_with_type_arguments(
    w: &mut Sink<'_>,
    heritage: &ast::TSInterfaceHeritage,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "TSExpressionWithTypeArguments", heritage.span, ctx);
    raw!(w, "TSExpressionWithTypeArguments", ",\"expression\":");
    write_entity_name(w, &heritage.expression, ctx);
    if let Some(ta) = &heritage.type_arguments {
        raw!(w, "TSExpressionWithTypeArguments", ",\"typeParameters\":");
        write_type_parameter_instantiation(w, ta, ctx);
    }
    close_node(w, "TSExpressionWithTypeArguments", heritage.span, ctx);
}
