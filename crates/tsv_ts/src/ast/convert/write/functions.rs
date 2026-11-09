// Function-related expression writers.

use super::expressions::{ChainState, ExprFlags, write_expression, write_expressions};
use super::statements::write_block_statement;
use super::{
    Ctx, JsonWriter, close_node, node_header, write_function_flags_fields,
    write_identifier_with_optional, write_or_null, write_return_type_field,
    write_type_arguments_field, write_type_parameters_field,
};
use crate::ast;
use tsv_lang::Span;

/// Emits an `ArrowFunctionExpression` node.
///
/// acorn-typescript's arrow paths assign fields in different orders, so the
/// wire order is shape-dependent:
///
/// - `async <T>(…)` (`tsTryParseGenericAsyncArrowFunction`): `typeParameters`,
///   `params`, `returnType?`, then the `parseArrowExpression` tail (`id`,
///   `expression`, `generator`, `async`, `body`). Detected as an async arrow
///   whose type parameters start after the arrow itself (`async` keyword
///   first); a `<T>async (…)` graft starts at the `<`.
/// - everything else (`parseParenArrowList` / `parseSubscriptAsyncArrow`):
///   `returnType?` first (stamped on the fresh node), then `id`, `expression`,
///   `generator`, `async`, `params`, `body`, with `typeParameters?` last (the
///   `<T>(…) => …` reading grafts them onto the finished arrow).
pub(super) fn write_arrow_function_expression(
    w: &mut JsonWriter,
    arrow: &ast::ArrowFunctionExpression<'_>,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header(w, "ArrowFunctionExpression", span, ctx);
    let generic_async = arrow.r#async
        && arrow
            .type_parameters
            .as_ref()
            .is_some_and(|tp| tp.span.start > span.start);
    if generic_async {
        write_type_parameters_field(w, arrow.type_parameters.as_ref(), ctx);
        w.raw(",\"params\":");
        write_expressions(w, arrow.params, ctx);
        write_return_type_field(w, arrow.return_type.as_ref(), ctx);
        w.raw_pick(
            arrow.body.is_expression(),
            b",\"id\":null,\"expression\":true,\"generator\":false,\"async\":true,\"body\":",
            b",\"id\":null,\"expression\":false,\"generator\":false,\"async\":true,\"body\":",
        );
        write_arrow_body(w, &arrow.body, ctx);
    } else {
        write_return_type_field(w, arrow.return_type.as_ref(), ctx);
        w.raw_pick(
            arrow.body.is_expression(),
            b",\"id\":null,\"expression\":true,\"generator\":false,\"async\":",
            b",\"id\":null,\"expression\":false,\"generator\":false,\"async\":",
        );
        w.raw_pick(arrow.r#async, b"true,\"params\":", b"false,\"params\":");
        write_expressions(w, arrow.params, ctx);
        w.raw(",\"body\":");
        write_arrow_body(w, &arrow.body, ctx);
        write_type_parameters_field(w, arrow.type_parameters.as_ref(), ctx);
    }
    close_node(w, "ArrowFunctionExpression", span, ctx);
}

fn write_arrow_body(w: &mut JsonWriter, body: &ast::ArrowFunctionBody<'_>, ctx: &Ctx<'_>) {
    match body {
        ast::ArrowFunctionBody::Expression(expr) => write_expression(w, expr, ctx),
        ast::ArrowFunctionBody::BlockStatement(block) => write_block_statement(w, block, ctx),
    }
}

/// Emits a `FunctionExpression` node. Field order:
/// `id` (nullable), `expression`, `generator`, `async`, `typeParameters?`,
/// `params`, `returnType?`, `body` — except an object-literal method value
/// (`type_params_last`), where acorn-typescript grafts `typeParameters` onto
/// the finished `parseMethod` node, so it serializes after `body`.
pub(super) fn write_function_expression(
    w: &mut JsonWriter,
    func: &ast::FunctionExpression<'_>,
    ctx: &Ctx<'_>,
    type_params_last: bool,
) {
    node_header(w, "FunctionExpression", func.span, ctx);
    w.raw(",\"id\":");
    write_or_null(w, func.id.as_ref(), |w, id| {
        write_identifier_with_optional(w, id, ctx);
    });
    write_function_flags_fields(w, func.generator, func.r#async);
    if !type_params_last {
        write_type_parameters_field(w, func.type_parameters.as_ref(), ctx);
    }
    w.raw(",\"params\":");
    write_expressions(w, func.params, ctx);
    write_return_type_field(w, func.return_type.as_ref(), ctx);
    w.raw(",\"body\":");
    write_block_statement(w, &func.body, ctx);
    if type_params_last {
        write_type_parameters_field(w, func.type_parameters.as_ref(), ctx);
    }
    close_node(w, "FunctionExpression", func.span, ctx);
}

/// Emits a `NewExpression` node. Field order: `callee`, `typeArguments?`,
/// `arguments` — acorn-typescript hoists `typeArguments` off the callee in
/// `parseNewCallee` before the argument list is parsed.
pub(super) fn write_new_expression(
    w: &mut JsonWriter,
    new_expr: &ast::NewExpression<'_>,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header(w, "NewExpression", span, ctx);
    w.raw(",\"callee\":");
    write_expression(w, new_expr.callee, ctx);
    write_type_arguments_field(w, new_expr.type_arguments.as_ref(), ctx);
    w.raw(",\"arguments\":");
    write_expressions(w, new_expr.arguments.iter().copied(), ctx);
    close_node(w, "NewExpression", span, ctx);
}

/// The flags a `CallExpression`'s callee is written with: the chain verdict it inherits,
/// and `force_optional` for acorn-typescript's `?.<T>(...)` path, which marks the callee
/// node itself optional; the decorator spine strip walks call → callee.
pub(super) fn call_callee_flags(
    call: &ast::CallExpression<'_>,
    callee_chain: ChainState,
    strip_optional: bool,
) -> ExprFlags {
    ExprFlags {
        chain: callee_chain,
        force_optional: call.optional && call.type_arguments.is_some(),
        strip_optional,
    }
}

/// Emits the rest of a `CallExpression` node (chain-aware) once its header and callee are
/// written — the expression writer's chain walk writes those down the spine
/// (`write_chain`). Field order: `callee`, `arguments`, `typeArguments?`, `optional?`.
///
/// `force_optional` / `strip_optional` are this node's own `optional` overrides
/// (see `ExprFlags`): strip omits `optional` entirely (decorator spine) and
/// wins over force (`true`), which wins over the computed value.
pub(super) fn write_call_expression_tail(
    w: &mut JsonWriter,
    call: &ast::CallExpression<'_>,
    span: Span,
    ctx: &Ctx<'_>,
    callee_chain: ChainState,
    force_optional: bool,
    strip_optional: bool,
) {
    w.raw(",\"arguments\":");
    write_expressions(w, call.arguments.iter().copied(), ctx);
    write_type_arguments_field(w, call.type_arguments.as_ref(), ctx);
    if strip_optional {
        // Omitted along an unparenthesized decorator's call/member spine.
    } else if force_optional {
        w.raw(",\"optional\":true");
    } else {
        // acorn-typescript omits `optional` on a typeArguments call unless the
        // call is part of an optional chain; the chain test is the call's own
        // left segment (parens seal the segment).
        //
        // Ordered so the chain test runs only when a `typeArguments` call makes
        // it load-bearing: it walks the callee spine, and the overwhelming
        // majority of calls carry no type arguments and would pay that walk to
        // discard the answer. `KnownFree` is the caller having already resolved
        // that spine to optional-free.
        if call.type_arguments.is_none()
            || call.optional
            || (callee_chain != ChainState::KnownFree
                && span.start >= call.callee.span().start
                && call.callee.has_optional_in_chain())
        {
            w.raw_pick(call.optional, b",\"optional\":true", b",\"optional\":false");
        }
    }
    close_node(w, "CallExpression", span, ctx);
}

/// Emits the rest of a `MemberExpression` node (chain-aware) once its header and object
/// are written, as [`write_call_expression_tail`] does a call's. Field order: `object`,
/// `property`, `computed`, `optional?`. Same strip/force precedence as a call's; the strip
/// walks member → object (its object's flags are the chain walk's).
pub(super) fn write_member_expression_tail(
    w: &mut JsonWriter,
    member: &ast::MemberExpression<'_>,
    span: Span,
    ctx: &Ctx<'_>,
    force_optional: bool,
    strip_optional: bool,
) {
    w.raw(",\"property\":");
    write_expression(w, member.property, ctx);
    // One literal per value, `optional` folded in where it is written: both
    // flags are nearly always `false`, so each branch predicts at its site.
    if strip_optional {
        // `optional` is omitted along an unparenthesized decorator's
        // call/member spine.
        w.raw_pick(
            member.computed,
            b",\"computed\":true",
            b",\"computed\":false",
        );
    } else if force_optional || member.optional {
        w.raw_pick(
            member.computed,
            b",\"computed\":true,\"optional\":true",
            b",\"computed\":false,\"optional\":true",
        );
    } else {
        w.raw_pick(
            member.computed,
            b",\"computed\":true,\"optional\":false",
            b",\"computed\":false,\"optional\":false",
        );
    }
    close_node(w, "MemberExpression", span, ctx);
}

/// Emits a `ConditionalExpression` node.
pub(super) fn write_conditional_expression(
    w: &mut JsonWriter,
    cond: &ast::ConditionalExpression<'_>,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header(w, "ConditionalExpression", span, ctx);
    w.raw(",\"test\":");
    write_expression(w, cond.test, ctx);
    w.raw(",\"consequent\":");
    write_expression(w, cond.consequent, ctx);
    w.raw(",\"alternate\":");
    write_expression(w, cond.alternate, ctx);
    close_node(w, "ConditionalExpression", span, ctx);
}

/// Emits an `AwaitExpression` node.
pub(super) fn write_await_expression(
    w: &mut JsonWriter,
    await_expr: &ast::AwaitExpression<'_>,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header(w, "AwaitExpression", span, ctx);
    w.raw(",\"argument\":");
    write_expression(w, await_expr.argument, ctx);
    close_node(w, "AwaitExpression", span, ctx);
}

/// Emits a `YieldExpression` node. Field order: `delegate`, `argument`
/// (nullable).
pub(super) fn write_yield_expression(
    w: &mut JsonWriter,
    yield_expr: &ast::YieldExpression<'_>,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header(w, "YieldExpression", span, ctx);
    w.raw(",\"delegate\":");
    w.bool(yield_expr.delegate);
    w.raw(",\"argument\":");
    write_or_null(w, yield_expr.argument.as_ref(), |w, e| {
        write_expression(w, e, ctx);
    });
    close_node(w, "YieldExpression", span, ctx);
}
