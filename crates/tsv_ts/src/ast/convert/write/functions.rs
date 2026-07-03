// Function-related expression writers — the writer twin of `convert::functions`.

use super::super::super::internal;
use super::expressions::{ExprFlags, write_expression, write_expression_inner, write_expressions};
use super::statements::write_block_statement;
use super::{
    Ctx, JsonWriter, close_node, node_header, write_identifier_with_optional, write_or_null,
    write_return_type_field, write_type_arguments_field, write_type_parameters_field,
};

/// Mirrors `convert_arrow_function_expression`. Field order:
/// `id` (always null), `expression`, `generator` (always false), `async`,
/// `params`, `body`, `typeParameters?`, `returnType?`.
pub(super) fn write_arrow_function_expression(
    w: &mut JsonWriter,
    arrow: &internal::ArrowFunctionExpression<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "ArrowFunctionExpression", arrow.span, ctx);
    w.raw(",\"id\":null,\"expression\":");
    w.bool(arrow.body.is_expression());
    w.raw(",\"generator\":false,\"async\":");
    w.bool(arrow.r#async);
    w.raw(",\"params\":");
    write_expressions(w, arrow.params, ctx);
    w.raw(",\"body\":");
    match &arrow.body {
        internal::ArrowFunctionBody::Expression(expr) => write_expression(w, expr, ctx),
        internal::ArrowFunctionBody::BlockStatement(block) => write_block_statement(w, block, ctx),
    }
    write_type_parameters_field(w, arrow.type_parameters.as_ref(), ctx);
    write_return_type_field(w, arrow.return_type.as_ref(), ctx);
    close_node(w, "ArrowFunctionExpression", arrow.span, ctx);
}

/// Mirrors `convert_function_expression`. Field order:
/// `id` (nullable), `expression`, `generator`, `async`, `typeParameters?`,
/// `params`, `returnType?`, `body`.
pub(super) fn write_function_expression(
    w: &mut JsonWriter,
    func: &internal::FunctionExpression<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "FunctionExpression", func.span, ctx);
    w.raw(",\"id\":");
    write_or_null(w, func.id.as_ref(), |w, id| {
        write_identifier_with_optional(w, id, ctx);
    });
    w.raw(",\"expression\":false,\"generator\":");
    w.bool(func.generator);
    w.raw(",\"async\":");
    w.bool(func.r#async);
    write_type_parameters_field(w, func.type_parameters.as_ref(), ctx);
    w.raw(",\"params\":");
    write_expressions(w, func.params, ctx);
    write_return_type_field(w, func.return_type.as_ref(), ctx);
    w.raw(",\"body\":");
    write_block_statement(w, &func.body, ctx);
    close_node(w, "FunctionExpression", func.span, ctx);
}

/// Mirrors `convert_new_expression`. Field order: `callee`, `arguments`,
/// `typeArguments?`.
pub(super) fn write_new_expression(
    w: &mut JsonWriter,
    new_expr: &internal::NewExpression<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "NewExpression", new_expr.span, ctx);
    w.raw(",\"callee\":");
    write_expression(w, new_expr.callee, ctx);
    w.raw(",\"arguments\":");
    write_expressions(w, new_expr.arguments, ctx);
    write_type_arguments_field(w, new_expr.type_arguments.as_ref(), ctx);
    close_node(w, "NewExpression", new_expr.span, ctx);
}

/// Mirrors `convert_call_expression` (chain-aware). Field order: `callee`,
/// `arguments`, `typeArguments?`, `optional?`.
///
/// `force_optional` / `strip_optional` are this node's own post-hoc overrides
/// (see `ExprFlags`): strip omits `optional` entirely (decorator spine) and
/// wins over force (`true`), which wins over the computed value — matching
/// convert's mutate-after-convert order.
pub(super) fn write_call_expression(
    w: &mut JsonWriter,
    call: &internal::CallExpression<'_>,
    ctx: &Ctx<'_>,
    callee_in_chain: bool,
    force_optional: bool,
    strip_optional: bool,
) {
    node_header(w, "CallExpression", call.span, ctx);
    w.raw(",\"callee\":");
    write_expression_inner(
        w,
        call.callee,
        ctx,
        ExprFlags {
            in_chain: callee_in_chain,
            // acorn-typescript's `?.<T>(...)` path marks the callee node
            // itself optional.
            force_optional: call.optional && call.type_arguments.is_some(),
            // The decorator spine strip walks call → callee.
            strip_optional,
        },
    );
    w.raw(",\"arguments\":");
    write_expressions(w, call.arguments, ctx);
    write_type_arguments_field(w, call.type_arguments.as_ref(), ctx);
    if strip_optional {
        // Omitted along an unparenthesized decorator's call/member spine.
    } else if force_optional {
        w.raw(",\"optional\":true");
    } else {
        // acorn-typescript omits `optional` on a typeArguments call unless the
        // call is part of an optional chain; the chain test is the call's own
        // left segment (parens seal the segment).
        let in_optional_chain = call.optional
            || (call.span.start >= call.callee.span().start && call.callee.has_optional_in_chain());
        if call.type_arguments.is_none() || in_optional_chain {
            w.raw(",\"optional\":");
            w.bool(call.optional);
        }
    }
    close_node(w, "CallExpression", call.span, ctx);
}

/// Mirrors `convert_member_expression` (chain-aware). Field order: `object`,
/// `property`, `computed`, `optional?`. Same strip/force precedence as
/// `write_call_expression`; the strip walks member → object.
pub(super) fn write_member_expression(
    w: &mut JsonWriter,
    member: &internal::MemberExpression<'_>,
    ctx: &Ctx<'_>,
    object_in_chain: bool,
    force_optional: bool,
    strip_optional: bool,
) {
    node_header(w, "MemberExpression", member.span, ctx);
    w.raw(",\"object\":");
    write_expression_inner(
        w,
        member.object,
        ctx,
        ExprFlags {
            in_chain: object_in_chain,
            force_optional: false,
            strip_optional,
        },
    );
    w.raw(",\"property\":");
    write_expression(w, member.property, ctx);
    w.raw(",\"computed\":");
    w.bool(member.computed);
    if strip_optional {
        // Omitted along an unparenthesized decorator's call/member spine.
    } else {
        w.raw(",\"optional\":");
        w.bool(force_optional || member.optional);
    }
    close_node(w, "MemberExpression", member.span, ctx);
}

/// Mirrors `convert_conditional_expression`.
pub(super) fn write_conditional_expression(
    w: &mut JsonWriter,
    cond: &internal::ConditionalExpression<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "ConditionalExpression", cond.span, ctx);
    w.raw(",\"test\":");
    write_expression(w, cond.test, ctx);
    w.raw(",\"consequent\":");
    write_expression(w, cond.consequent, ctx);
    w.raw(",\"alternate\":");
    write_expression(w, cond.alternate, ctx);
    close_node(w, "ConditionalExpression", cond.span, ctx);
}

/// Mirrors `convert_await_expression`.
pub(super) fn write_await_expression(
    w: &mut JsonWriter,
    await_expr: &internal::AwaitExpression<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "AwaitExpression", await_expr.span, ctx);
    w.raw(",\"argument\":");
    write_expression(w, await_expr.argument, ctx);
    close_node(w, "AwaitExpression", await_expr.span, ctx);
}

/// Mirrors `convert_yield_expression`. Field order: `delegate`, `argument`
/// (nullable).
pub(super) fn write_yield_expression(
    w: &mut JsonWriter,
    yield_expr: &internal::YieldExpression<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "YieldExpression", yield_expr.span, ctx);
    w.raw(",\"delegate\":");
    w.bool(yield_expr.delegate);
    w.raw(",\"argument\":");
    write_or_null(w, yield_expr.argument.as_ref(), |w, e| {
        write_expression(w, e, ctx);
    });
    close_node(w, "YieldExpression", yield_expr.span, ctx);
}
