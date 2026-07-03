// Control flow statement writers — the writer twin of `convert::control_flow`.

use super::super::super::internal;
use super::super::Schema;
use super::expressions::write_expression;
use super::statements::{write_block_statement, write_statement, write_variable_declaration};
use super::{
    Ctx, JsonWriter, close_node, node_header, write_array, write_identifier_plain, write_or_null,
};

/// Control flow bodies never contain import/export declarations, so the schema
/// doesn't matter; `Acorn` for simplicity (mirrors convert's `SCHEMA`).
const SCHEMA: Schema = Schema::Acorn;

/// Mirrors `convert_if_statement`.
pub(super) fn write_if_statement(
    w: &mut JsonWriter,
    if_stmt: &internal::IfStatement<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "IfStatement", if_stmt.span, ctx);
    w.raw(",\"test\":");
    write_expression(w, &if_stmt.test, ctx);
    w.raw(",\"consequent\":");
    write_statement(w, if_stmt.consequent, ctx, SCHEMA);
    w.raw(",\"alternate\":");
    write_or_null(w, if_stmt.alternate.as_ref(), |w, alt| {
        write_statement(w, alt, ctx, SCHEMA);
    });
    close_node(w, "IfStatement", if_stmt.span, ctx);
}

/// Mirrors `convert_for_statement`. `init`/`test`/`update` are nullable.
pub(super) fn write_for_statement(
    w: &mut JsonWriter,
    for_stmt: &internal::ForStatement<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "ForStatement", for_stmt.span, ctx);
    w.raw(",\"init\":");
    match &for_stmt.init {
        Some(internal::ForInit::VariableDeclaration(decl)) => {
            write_variable_declaration(w, decl, ctx);
        }
        Some(internal::ForInit::Expression(expr)) => write_expression(w, expr, ctx),
        None => w.null(),
    }
    w.raw(",\"test\":");
    write_or_null(w, for_stmt.test.as_ref(), |w, test| {
        write_expression(w, test, ctx);
    });
    w.raw(",\"update\":");
    write_or_null(w, for_stmt.update.as_ref(), |w, update| {
        write_expression(w, update, ctx);
    });
    w.raw(",\"body\":");
    write_statement(w, for_stmt.body, ctx, SCHEMA);
    close_node(w, "ForStatement", for_stmt.span, ctx);
}

/// Mirrors `convert_for_in_of_left` (an untagged declaration-or-pattern).
fn write_for_in_of_left(w: &mut JsonWriter, left: &internal::ForInOfLeft<'_>, ctx: &Ctx<'_>) {
    match left {
        internal::ForInOfLeft::VariableDeclaration(decl) => {
            write_variable_declaration(w, decl, ctx);
        }
        internal::ForInOfLeft::Pattern(expr) => write_expression(w, expr, ctx),
    }
}

/// Mirrors `convert_for_in_statement`.
pub(super) fn write_for_in_statement(
    w: &mut JsonWriter,
    for_in: &internal::ForInStatement<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "ForInStatement", for_in.span, ctx);
    w.raw(",\"left\":");
    write_for_in_of_left(w, &for_in.left, ctx);
    w.raw(",\"right\":");
    write_expression(w, &for_in.right, ctx);
    w.raw(",\"body\":");
    write_statement(w, for_in.body, ctx, SCHEMA);
    close_node(w, "ForInStatement", for_in.span, ctx);
}

/// Mirrors `convert_for_of_statement`. Field order: `await`, `left`, `right`,
/// `body` (the public struct declares `await` first, unlike convert's build
/// order).
pub(super) fn write_for_of_statement(
    w: &mut JsonWriter,
    for_of: &internal::ForOfStatement<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "ForOfStatement", for_of.span, ctx);
    w.raw(",\"await\":");
    w.bool(for_of.r#await);
    w.raw(",\"left\":");
    write_for_in_of_left(w, &for_of.left, ctx);
    w.raw(",\"right\":");
    write_expression(w, &for_of.right, ctx);
    w.raw(",\"body\":");
    write_statement(w, for_of.body, ctx, SCHEMA);
    close_node(w, "ForOfStatement", for_of.span, ctx);
}

/// Mirrors `convert_while_statement`.
pub(super) fn write_while_statement(
    w: &mut JsonWriter,
    while_stmt: &internal::WhileStatement<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "WhileStatement", while_stmt.span, ctx);
    w.raw(",\"test\":");
    write_expression(w, &while_stmt.test, ctx);
    w.raw(",\"body\":");
    write_statement(w, while_stmt.body, ctx, SCHEMA);
    close_node(w, "WhileStatement", while_stmt.span, ctx);
}

/// Mirrors `convert_do_while_statement`. Field order: `body`, `test`.
pub(super) fn write_do_while_statement(
    w: &mut JsonWriter,
    do_while: &internal::DoWhileStatement<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "DoWhileStatement", do_while.span, ctx);
    w.raw(",\"body\":");
    write_statement(w, do_while.body, ctx, SCHEMA);
    w.raw(",\"test\":");
    write_expression(w, &do_while.test, ctx);
    close_node(w, "DoWhileStatement", do_while.span, ctx);
}

/// Mirrors `convert_switch_statement` + `convert_switch_case`.
pub(super) fn write_switch_statement(
    w: &mut JsonWriter,
    switch_stmt: &internal::SwitchStatement<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "SwitchStatement", switch_stmt.span, ctx);
    w.raw(",\"discriminant\":");
    write_expression(w, &switch_stmt.discriminant, ctx);
    w.raw(",\"cases\":");
    write_array(w, switch_stmt.cases, |w, case| {
        node_header(w, "SwitchCase", case.span, ctx);
        w.raw(",\"test\":");
        write_or_null(w, case.test.as_ref(), |w, t| write_expression(w, t, ctx));
        w.raw(",\"consequent\":");
        write_array(w, case.consequent, |w, s| {
            write_statement(w, s, ctx, SCHEMA);
        });
        close_node(w, "SwitchCase", case.span, ctx);
    });
    close_node(w, "SwitchStatement", switch_stmt.span, ctx);
}

/// Mirrors `convert_try_statement` + `convert_catch_clause`. `handler` and
/// `finalizer` are nullable, as is the catch clause's `param`.
pub(super) fn write_try_statement(
    w: &mut JsonWriter,
    try_stmt: &internal::TryStatement<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "TryStatement", try_stmt.span, ctx);
    w.raw(",\"block\":");
    write_block_statement(w, &try_stmt.block, ctx);
    w.raw(",\"handler\":");
    write_or_null(w, try_stmt.handler.as_ref(), |w, clause| {
        node_header(w, "CatchClause", clause.span, ctx);
        w.raw(",\"param\":");
        write_or_null(w, clause.param.as_ref(), |w, p| write_expression(w, p, ctx));
        w.raw(",\"body\":");
        write_block_statement(w, &clause.body, ctx);
        close_node(w, "CatchClause", clause.span, ctx);
    });
    w.raw(",\"finalizer\":");
    write_or_null(w, try_stmt.finalizer.as_ref(), |w, f| {
        write_block_statement(w, f, ctx);
    });
    close_node(w, "TryStatement", try_stmt.span, ctx);
}

/// Mirrors `convert_throw_statement`.
pub(super) fn write_throw_statement(
    w: &mut JsonWriter,
    throw_stmt: &internal::ThrowStatement<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "ThrowStatement", throw_stmt.span, ctx);
    w.raw(",\"argument\":");
    write_expression(w, &throw_stmt.argument, ctx);
    close_node(w, "ThrowStatement", throw_stmt.span, ctx);
}

/// Mirrors `convert_break_statement`. `label` is nullable.
pub(super) fn write_break_statement(
    w: &mut JsonWriter,
    break_stmt: &internal::BreakStatement<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "BreakStatement", break_stmt.span, ctx);
    w.raw(",\"label\":");
    write_or_null(w, break_stmt.label.as_ref(), |w, id| {
        write_identifier_plain(w, id, ctx);
    });
    close_node(w, "BreakStatement", break_stmt.span, ctx);
}

/// Mirrors `convert_continue_statement`. `label` is nullable.
pub(super) fn write_continue_statement(
    w: &mut JsonWriter,
    continue_stmt: &internal::ContinueStatement<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "ContinueStatement", continue_stmt.span, ctx);
    w.raw(",\"label\":");
    write_or_null(w, continue_stmt.label.as_ref(), |w, id| {
        write_identifier_plain(w, id, ctx);
    });
    close_node(w, "ContinueStatement", continue_stmt.span, ctx);
}

/// Mirrors `convert_labeled_statement`.
pub(super) fn write_labeled_statement(
    w: &mut JsonWriter,
    labeled: &internal::LabeledStatement<'_>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "LabeledStatement", labeled.span, ctx);
    w.raw(",\"label\":");
    write_identifier_plain(w, &labeled.label, ctx);
    w.raw(",\"body\":");
    write_statement(w, labeled.body, ctx, SCHEMA);
    close_node(w, "LabeledStatement", labeled.span, ctx);
}
