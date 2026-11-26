// Control flow statement writers.

use super::expressions::write_expression;
use super::statements::{write_block_statement, write_statement, write_variable_declaration};
use super::{
    Ctx, Sink, close_node, node_header, write_array, write_identifier_plain, write_or_null,
};
use crate::ast;
use tsv_lang::Span;

/// Emits an `IfStatement` node.
pub(super) fn write_if_statement(
    w: &mut Sink<'_>,
    if_stmt: &ast::IfStatement,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "IfStatement", span, ctx);
    raw!(w, "IfStatement", ",\"test\":");
    write_expression(w, ctx.view.expr(if_stmt.test), ctx);
    raw!(w, "IfStatement", ",\"consequent\":");
    write_statement(w, ctx.view.stmt(if_stmt.consequent), ctx);
    raw!(w, "IfStatement", ",\"alternate\":");
    write_or_null(w, ctx.view.opt_stmt(if_stmt.alternate), |w, alt| {
        write_statement(w, alt, ctx);
    });
    close_node(w, "IfStatement", span, ctx);
}

/// Emits a `ForStatement` node. `init`/`test`/`update` are nullable.
pub(super) fn write_for_statement(
    w: &mut Sink<'_>,
    for_stmt: &ast::ForStatement,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "ForStatement", span, ctx);
    raw!(w, "ForStatement", ",\"init\":");
    match ctx.view.opt_for_head(for_stmt.init) {
        Some(ast::ForInit::VariableDeclaration(decl)) => {
            write_variable_declaration(w, decl, ctx);
        }
        Some(ast::ForInit::Expression(expr)) => write_expression(w, ctx.view.expr(*expr), ctx),
        None => w.null(),
    }
    raw!(w, "ForStatement", ",\"test\":");
    write_or_null(w, for_stmt.test.as_ref(), |w, test| {
        write_expression(w, ctx.view.expr(*test), ctx);
    });
    raw!(w, "ForStatement", ",\"update\":");
    write_or_null(w, for_stmt.update.as_ref(), |w, update| {
        write_expression(w, ctx.view.expr(*update), ctx);
    });
    raw!(w, "ForStatement", ",\"body\":");
    write_statement(w, ctx.view.stmt(for_stmt.body), ctx);
    close_node(w, "ForStatement", span, ctx);
}

/// Emit a `for`-`in`/`for`-`of` `left` (an untagged declaration-or-pattern).
fn write_for_in_of_left(w: &mut Sink<'_>, left: &ast::ForInOfLeft, ctx: &Ctx<'_>) {
    match left {
        ast::ForInOfLeft::VariableDeclaration(decl) => {
            write_variable_declaration(w, decl, ctx);
        }
        ast::ForInOfLeft::Pattern(expr) => write_expression(w, ctx.view.expr(*expr), ctx),
    }
}

/// Emits a `ForInStatement` node.
pub(super) fn write_for_in_statement(
    w: &mut Sink<'_>,
    for_in: &ast::ForInStatement,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "ForInStatement", span, ctx);
    raw!(w, "ForInStatement", ",\"left\":");
    write_for_in_of_left(w, ctx.view.for_left(for_in.left), ctx);
    raw!(w, "ForInStatement", ",\"right\":");
    write_expression(w, ctx.view.expr(for_in.right), ctx);
    raw!(w, "ForInStatement", ",\"body\":");
    write_statement(w, ctx.view.stmt(for_in.body), ctx);
    close_node(w, "ForInStatement", span, ctx);
}

/// Emits a `ForOfStatement` node. Field order: `await`, `left`, `right`,
/// `body`.
pub(super) fn write_for_of_statement(
    w: &mut Sink<'_>,
    for_of: &ast::ForOfStatement,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "ForOfStatement", span, ctx);
    raw!(w, "ForOfStatement", ",\"await\":");
    w.bool(for_of.r#await);
    raw!(w, "ForOfStatement", ",\"left\":");
    write_for_in_of_left(w, ctx.view.for_left(for_of.left), ctx);
    raw!(w, "ForOfStatement", ",\"right\":");
    write_expression(w, ctx.view.expr(for_of.right), ctx);
    raw!(w, "ForOfStatement", ",\"body\":");
    write_statement(w, ctx.view.stmt(for_of.body), ctx);
    close_node(w, "ForOfStatement", span, ctx);
}

/// Emits a `WhileStatement` node.
pub(super) fn write_while_statement(
    w: &mut Sink<'_>,
    while_stmt: ast::WhileStatement,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "WhileStatement", span, ctx);
    raw!(w, "WhileStatement", ",\"test\":");
    write_expression(w, ctx.view.expr(while_stmt.test), ctx);
    raw!(w, "WhileStatement", ",\"body\":");
    write_statement(w, ctx.view.stmt(while_stmt.body), ctx);
    close_node(w, "WhileStatement", span, ctx);
}

/// Emits a `WithStatement` node. Field order: `object`, `body`.
pub(super) fn write_with_statement(
    w: &mut Sink<'_>,
    with_stmt: ast::WithStatement,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "WithStatement", span, ctx);
    raw!(w, "WithStatement", ",\"object\":");
    write_expression(w, ctx.view.expr(with_stmt.object), ctx);
    raw!(w, "WithStatement", ",\"body\":");
    write_statement(w, ctx.view.stmt(with_stmt.body), ctx);
    close_node(w, "WithStatement", span, ctx);
}

/// Emits a `DoWhileStatement` node. Field order: `body`, `test`.
pub(super) fn write_do_while_statement(
    w: &mut Sink<'_>,
    do_while: ast::DoWhileStatement,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "DoWhileStatement", span, ctx);
    raw!(w, "DoWhileStatement", ",\"body\":");
    write_statement(w, ctx.view.stmt(do_while.body), ctx);
    raw!(w, "DoWhileStatement", ",\"test\":");
    write_expression(w, ctx.view.expr(do_while.test), ctx);
    close_node(w, "DoWhileStatement", span, ctx);
}

/// Emits a `SwitchStatement` node (each case a `SwitchCase`).
pub(super) fn write_switch_statement(
    w: &mut Sink<'_>,
    switch_stmt: &ast::SwitchStatement,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "SwitchStatement", span, ctx);
    raw!(w, "SwitchStatement", ",\"discriminant\":");
    write_expression(w, ctx.view.expr(switch_stmt.discriminant), ctx);
    raw!(w, "SwitchStatement", ",\"cases\":");
    // acorn initializes `consequent` before parsing `test`, so it serializes first.
    write_array(w, ctx.view.cases(switch_stmt.cases), |w, case| {
        node_header!(w, "SwitchCase", case.span, ctx);
        raw!(w, "SwitchCase", ",\"consequent\":");
        write_array(w, ctx.view.stmt_list(case.consequent), |w, s| {
            write_statement(w, ctx.view.stmt(*s), ctx);
        });
        raw!(w, "SwitchCase", ",\"test\":");
        write_or_null(w, case.test.as_ref(), |w, t| {
            write_expression(w, ctx.view.expr(*t), ctx);
        });
        close_node(w, "SwitchCase", case.span, ctx);
    });
    close_node(w, "SwitchStatement", span, ctx);
}

/// Emits a `TryStatement` node (its `handler` a `CatchClause`). `handler` and
/// `finalizer` are nullable, as is the catch clause's `param`.
pub(super) fn write_try_statement(
    w: &mut Sink<'_>,
    try_stmt: &ast::TryStatement,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "TryStatement", span, ctx);
    raw!(w, "TryStatement", ",\"block\":");
    write_block_statement(w, &try_stmt.block, ctx);
    raw!(w, "TryStatement", ",\"handler\":");
    write_or_null(w, ctx.view.opt_catch(try_stmt.handler), |w, clause| {
        node_header!(w, "CatchClause", clause.span, ctx);
        raw!(w, "CatchClause", ",\"param\":");
        write_or_null(w, clause.param.as_ref(), |w, p| write_expression(w, p, ctx));
        raw!(w, "CatchClause", ",\"body\":");
        write_block_statement(w, &clause.body, ctx);
        close_node(w, "CatchClause", clause.span, ctx);
    });
    raw!(w, ",\"finalizer\":");
    write_or_null(w, try_stmt.finalizer.as_ref(), |w, f| {
        write_block_statement(w, f, ctx);
    });
    close_node(w, "TryStatement", span, ctx);
}

/// Emits a `ThrowStatement` node.
pub(super) fn write_throw_statement(
    w: &mut Sink<'_>,
    throw_stmt: ast::ThrowStatement,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "ThrowStatement", span, ctx);
    raw!(w, "ThrowStatement", ",\"argument\":");
    write_expression(w, ctx.view.expr(throw_stmt.argument), ctx);
    close_node(w, "ThrowStatement", span, ctx);
}

/// Emits a `BreakStatement` node. `label` is nullable.
pub(super) fn write_break_statement(
    w: &mut Sink<'_>,
    break_stmt: &ast::BreakStatement,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "BreakStatement", span, ctx);
    raw!(w, "BreakStatement", ",\"label\":");
    write_or_null(w, break_stmt.label.as_ref(), |w, id| {
        write_identifier_plain(w, id, ctx);
    });
    close_node(w, "BreakStatement", span, ctx);
}

/// Emits a `ContinueStatement` node. `label` is nullable.
pub(super) fn write_continue_statement(
    w: &mut Sink<'_>,
    continue_stmt: &ast::ContinueStatement,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "ContinueStatement", span, ctx);
    raw!(w, "ContinueStatement", ",\"label\":");
    write_or_null(w, continue_stmt.label.as_ref(), |w, id| {
        write_identifier_plain(w, id, ctx);
    });
    close_node(w, "ContinueStatement", span, ctx);
}

/// Emits a `LabeledStatement` node.
pub(super) fn write_labeled_statement(
    w: &mut Sink<'_>,
    labeled: &ast::LabeledStatement,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "LabeledStatement", span, ctx);
    // acorn assigns `body` before `label`, so it serializes first.
    raw!(w, "LabeledStatement", ",\"body\":");
    write_statement(w, ctx.view.stmt(labeled.body), ctx);
    raw!(w, "LabeledStatement", ",\"label\":");
    write_identifier_plain(w, &labeled.label, ctx);
    close_node(w, "LabeledStatement", span, ctx);
}
