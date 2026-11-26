// Function-related expression writers.

use super::expressions::{ChainState, ExprFlags, write_expression, write_expressions};
use super::statements::write_block_statement;
use super::{
    Ctx, Sink, close_node, node_header, write_function_flags_fields,
    write_identifier_with_optional, write_or_null, write_return_type_field,
    write_type_arguments_field, write_type_parameters_field,
};
use crate::ast;
use tsv_lang::Span;

/// Emits an `ArrowFunctionExpression` node. Field order, on every route: `returnType?`,
/// `id`, `expression`, `generator`, `async`, `params`, `body`, `typeParameters?` —
/// acorn-typescript's common routes' (`parseParenArrowList` / `parseSubscriptAsyncArrow`
/// stamp `returnType` on the fresh node; the `<T>(…) => …` reading grafts `typeParameters`
/// onto the finished arrow). Its generic async route (`async <T>(…)`,
/// `tsTryParseGenericAsyncArrowFunction`) writes `typeParameters`, `params`, `returnType`
/// first; tsv's wire keeps the one order, so the comment attach (which walks in field
/// order) places that route's head comments as the common route would — a cataloged
/// divergence (`docs/conformance_svelte.md` §Comment Attachment Differences).
pub(super) fn write_arrow_function_expression(
    w: &mut Sink<'_>,
    arrow: &ast::ArrowFunctionExpression,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "ArrowFunctionExpression", span, ctx);
    write_return_type_field(w, arrow.return_type.as_ref(), ctx);
    pick!(
        w,
        "ArrowFunctionExpression",
        arrow.body.is_expression(),
        ",\"id\":null,\"expression\":true,\"generator\":false,\"async\":",
        ",\"id\":null,\"expression\":false,\"generator\":false,\"async\":"
    );
    pick!(
        w,
        "ArrowFunctionExpression",
        arrow.r#async,
        "true,\"params\":",
        "false,\"params\":"
    );
    write_expressions(w, ctx.view.exprs(arrow.params), ctx);
    raw!(w, "ArrowFunctionExpression", ",\"body\":");
    write_arrow_body(w, &arrow.body, ctx);
    write_type_parameters_field(w, arrow.type_parameters.as_ref(), ctx);
    close_node(w, "ArrowFunctionExpression", span, ctx);
}

fn write_arrow_body(w: &mut Sink<'_>, body: &ast::ArrowFunctionBody, ctx: &Ctx<'_>) {
    match body {
        ast::ArrowFunctionBody::Expression(expr) => write_expression(w, ctx.view.expr(*expr), ctx),
        ast::ArrowFunctionBody::BlockStatement(block) => write_block_statement(w, block, ctx),
    }
}

/// Emits a `FunctionExpression` node. Field order, on every route: `id` (nullable),
/// `expression`, `generator`, `async`, `typeParameters?`, `params`, `returnType?`,
/// `body`. (acorn-typescript grafts a method value's `typeParameters` onto the finished
/// `parseMethod` node, after `body`; tsv's wire keeps the one order, and no comment
/// attachment moves with it — a method value starts at its `(`, so the walk has already
/// given every comment in its type parameters to an enclosing node.)
pub(super) fn write_function_expression(
    w: &mut Sink<'_>,
    func: &ast::FunctionExpression,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "FunctionExpression", func.span, ctx);
    raw!(w, "FunctionExpression", ",\"id\":");
    write_or_null(w, func.id.as_ref(), |w, id| {
        write_identifier_with_optional(w, id, ctx);
    });
    write_function_flags_fields(w, func.generator, func.r#async);
    write_type_parameters_field(w, func.type_parameters.as_ref(), ctx);
    raw!(w, "FunctionExpression", ",\"params\":");
    write_expressions(w, ctx.view.exprs(func.params), ctx);
    write_return_type_field(w, func.return_type.as_ref(), ctx);
    raw!(w, "FunctionExpression", ",\"body\":");
    write_block_statement(w, &func.body, ctx);
    close_node(w, "FunctionExpression", func.span, ctx);
}

/// Emits a `NewExpression` node. Field order: `callee`, `typeArguments?`,
/// `arguments` — acorn-typescript hoists `typeArguments` off the callee in
/// `parseNewCallee` before the argument list is parsed.
pub(super) fn write_new_expression(
    w: &mut Sink<'_>,
    new_expr: &ast::NewExpression,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "NewExpression", span, ctx);
    raw!(w, "NewExpression", ",\"callee\":");
    write_expression(w, ctx.view.expr(new_expr.callee), ctx);
    write_type_arguments_field(w, new_expr.type_arguments(ctx.view), ctx);
    raw!(w, "NewExpression", ",\"arguments\":");
    write_expressions(
        w,
        ctx.view
            .expr_list(new_expr.arguments)
            .iter()
            .map(|e| ctx.view.expr(*e)),
        ctx,
    );
    close_node(w, "NewExpression", span, ctx);
}

/// The flags a `CallExpression`'s callee is written with: the chain verdict it inherits,
/// and `force_optional` for acorn-typescript's `?.<T>(...)` path, which marks the callee
/// node itself optional; the decorator spine strip walks call → callee.
pub(super) fn call_callee_flags(
    call: &ast::CallExpression,
    callee_chain: ChainState,
    strip_optional: bool,
) -> ExprFlags {
    ExprFlags {
        chain: callee_chain,
        force_optional: call.optional && call.type_args.is_some(),
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
    w: &mut Sink<'_>,
    call: &ast::CallExpression,
    span: Span,
    ctx: &Ctx<'_>,
    callee_chain: ChainState,
    force_optional: bool,
    strip_optional: bool,
) {
    raw!(w, ",\"arguments\":");
    write_expressions(
        w,
        ctx.view
            .expr_list(call.arguments)
            .iter()
            .map(|e| ctx.view.expr(*e)),
        ctx,
    );
    write_type_arguments_field(w, call.type_arguments(ctx.view), ctx);
    if strip_optional {
        // Omitted along an unparenthesized decorator's call/member spine.
    } else if force_optional {
        raw!(w, ",\"optional\":true");
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
        if call.type_args.is_none()
            || call.optional
            || (callee_chain != ChainState::KnownFree
                && span.start >= ctx.view.expr(call.callee).span().start
                && ctx.view.expr(call.callee).has_optional_in_chain(ctx.view))
        {
            pick!(
                w,
                call.optional,
                ",\"optional\":true",
                ",\"optional\":false"
            );
        }
    }
    close_node(w, "CallExpression", span, ctx);
}

/// Emits the rest of a `MemberExpression` node (chain-aware) once its header and object
/// are written, as [`write_call_expression_tail`] does a call's. Field order: `object`,
/// `property`, `computed`, `optional?`. Same strip/force precedence as a call's; the strip
/// walks member → object (its object's flags are the chain walk's).
pub(super) fn write_member_expression_tail(
    w: &mut Sink<'_>,
    member: &ast::MemberExpression,
    span: Span,
    ctx: &Ctx<'_>,
    force_optional: bool,
    strip_optional: bool,
) {
    raw!(w, ",\"property\":");
    write_expression(w, ctx.view.expr(member.property), ctx);
    // One literal per value, `optional` folded in where it is written: both
    // flags are nearly always `false`, so each branch predicts at its site.
    if strip_optional {
        // `optional` is omitted along an unparenthesized decorator's
        // call/member spine.
        pick!(
            w,
            member.computed,
            ",\"computed\":true",
            ",\"computed\":false"
        );
    } else if force_optional || member.optional {
        pick!(
            w,
            member.computed,
            ",\"computed\":true,\"optional\":true",
            ",\"computed\":false,\"optional\":true"
        );
    } else {
        pick!(
            w,
            member.computed,
            ",\"computed\":true,\"optional\":false",
            ",\"computed\":false,\"optional\":false"
        );
    }
    close_node(w, "MemberExpression", span, ctx);
}

/// Emits a `ConditionalExpression` node.
pub(super) fn write_conditional_expression(
    w: &mut Sink<'_>,
    cond: &ast::ConditionalExpression,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "ConditionalExpression", span, ctx);
    raw!(w, "ConditionalExpression", ",\"test\":");
    write_expression(w, ctx.view.expr(cond.test), ctx);
    raw!(w, "ConditionalExpression", ",\"consequent\":");
    write_expression(w, ctx.view.expr(cond.consequent), ctx);
    raw!(w, "ConditionalExpression", ",\"alternate\":");
    write_expression(w, ctx.view.expr(cond.alternate), ctx);
    close_node(w, "ConditionalExpression", span, ctx);
}

/// Emits an `AwaitExpression` node.
pub(super) fn write_await_expression(
    w: &mut Sink<'_>,
    await_expr: ast::AwaitExpression,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "AwaitExpression", span, ctx);
    raw!(w, "AwaitExpression", ",\"argument\":");
    write_expression(w, ctx.view.expr(await_expr.argument), ctx);
    close_node(w, "AwaitExpression", span, ctx);
}

/// Emits a `YieldExpression` node. Field order: `delegate`, `argument`
/// (nullable).
pub(super) fn write_yield_expression(
    w: &mut Sink<'_>,
    yield_expr: ast::YieldExpression,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "YieldExpression", span, ctx);
    raw!(w, "YieldExpression", ",\"delegate\":");
    w.bool(yield_expr.delegate);
    raw!(w, "YieldExpression", ",\"argument\":");
    write_or_null(w, yield_expr.argument.as_ref(), |w, e| {
        write_expression(w, ctx.view.expr(*e), ctx);
    });
    close_node(w, "YieldExpression", span, ctx);
}
