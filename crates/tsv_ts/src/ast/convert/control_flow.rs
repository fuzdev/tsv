// Control flow statement conversions

use super::super::{internal, public};
use super::{
    Schema, convert_block_statement, convert_expression, convert_identifier, convert_statement,
    convert_variable_declaration, create_location,
};
use string_interner::DefaultStringInterner;
use tsv_lang::LocationMapper;

/// Schema for control flow statement bodies.
///
/// Control flow bodies (if/for/while/etc.) never contain import/export
/// declarations, so the schema doesn't matter. We use `Acorn` (the default)
/// for simplicity.
const SCHEMA: Schema = Schema::Acorn;

pub(in crate::ast) fn convert_if_statement<'src>(
    if_stmt: &internal::IfStatement<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::IfStatement<'src> {
    public::IfStatement {
        node_type: "IfStatement",
        start: loc.pos(if_stmt.span.start),
        end: loc.pos(if_stmt.span.end),
        loc: create_location(if_stmt.span, loc),
        test: Box::new(convert_expression(&if_stmt.test, source, loc, interner)),
        consequent: Box::new(convert_statement(
            if_stmt.consequent,
            source,
            loc,
            interner,
            SCHEMA,
        )),
        alternate: if_stmt
            .alternate
            .as_ref()
            .map(|alt| Box::new(convert_statement(alt, source, loc, interner, SCHEMA))),
    }
}

pub(in crate::ast) fn convert_for_statement<'src>(
    for_stmt: &internal::ForStatement<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::ForStatement<'src> {
    public::ForStatement {
        node_type: "ForStatement",
        start: loc.pos(for_stmt.span.start),
        end: loc.pos(for_stmt.span.end),
        loc: create_location(for_stmt.span, loc),
        init: for_stmt
            .init
            .as_ref()
            .map(|init| convert_for_init(init, source, loc, interner)),
        test: for_stmt
            .test
            .as_ref()
            .map(|test| Box::new(convert_expression(test, source, loc, interner))),
        update: for_stmt
            .update
            .as_ref()
            .map(|update| Box::new(convert_expression(update, source, loc, interner))),
        body: Box::new(convert_statement(
            for_stmt.body,
            source,
            loc,
            interner,
            SCHEMA,
        )),
    }
}

pub(in crate::ast) fn convert_for_in_statement<'src>(
    for_in: &internal::ForInStatement<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::ForInStatement<'src> {
    public::ForInStatement {
        node_type: "ForInStatement",
        start: loc.pos(for_in.span.start),
        end: loc.pos(for_in.span.end),
        loc: create_location(for_in.span, loc),
        left: convert_for_in_of_left(&for_in.left, source, loc, interner),
        right: Box::new(convert_expression(&for_in.right, source, loc, interner)),
        body: Box::new(convert_statement(
            for_in.body,
            source,
            loc,
            interner,
            SCHEMA,
        )),
    }
}

pub(in crate::ast) fn convert_for_of_statement<'src>(
    for_of: &internal::ForOfStatement<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::ForOfStatement<'src> {
    public::ForOfStatement {
        node_type: "ForOfStatement",
        start: loc.pos(for_of.span.start),
        end: loc.pos(for_of.span.end),
        loc: create_location(for_of.span, loc),
        left: convert_for_in_of_left(&for_of.left, source, loc, interner),
        right: Box::new(convert_expression(&for_of.right, source, loc, interner)),
        r#await: for_of.r#await,
        body: Box::new(convert_statement(
            for_of.body,
            source,
            loc,
            interner,
            SCHEMA,
        )),
    }
}

pub(in crate::ast) fn convert_while_statement<'src>(
    while_stmt: &internal::WhileStatement<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::WhileStatement<'src> {
    public::WhileStatement {
        node_type: "WhileStatement",
        start: loc.pos(while_stmt.span.start),
        end: loc.pos(while_stmt.span.end),
        loc: create_location(while_stmt.span, loc),
        test: Box::new(convert_expression(&while_stmt.test, source, loc, interner)),
        body: Box::new(convert_statement(
            while_stmt.body,
            source,
            loc,
            interner,
            SCHEMA,
        )),
    }
}

pub(in crate::ast) fn convert_do_while_statement<'src>(
    do_while: &internal::DoWhileStatement<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::DoWhileStatement<'src> {
    public::DoWhileStatement {
        node_type: "DoWhileStatement",
        start: loc.pos(do_while.span.start),
        end: loc.pos(do_while.span.end),
        loc: create_location(do_while.span, loc),
        body: Box::new(convert_statement(
            do_while.body,
            source,
            loc,
            interner,
            SCHEMA,
        )),
        test: Box::new(convert_expression(&do_while.test, source, loc, interner)),
    }
}

pub(in crate::ast) fn convert_switch_statement<'src>(
    switch_stmt: &internal::SwitchStatement<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::SwitchStatement<'src> {
    public::SwitchStatement {
        node_type: "SwitchStatement",
        start: loc.pos(switch_stmt.span.start),
        end: loc.pos(switch_stmt.span.end),
        loc: create_location(switch_stmt.span, loc),
        discriminant: Box::new(convert_expression(
            &switch_stmt.discriminant,
            source,
            loc,
            interner,
        )),
        cases: switch_stmt
            .cases
            .iter()
            .map(|case| convert_switch_case(case, source, loc, interner))
            .collect(),
    }
}

pub(in crate::ast) fn convert_try_statement<'src>(
    try_stmt: &internal::TryStatement<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::TryStatement<'src> {
    public::TryStatement {
        node_type: "TryStatement",
        start: loc.pos(try_stmt.span.start),
        end: loc.pos(try_stmt.span.end),
        loc: create_location(try_stmt.span, loc),
        block: convert_block_statement(&try_stmt.block, source, loc, interner),
        handler: try_stmt
            .handler
            .as_ref()
            .map(|h| convert_catch_clause(h, source, loc, interner)),
        finalizer: try_stmt
            .finalizer
            .as_ref()
            .map(|f| convert_block_statement(f, source, loc, interner)),
    }
}

pub(in crate::ast) fn convert_throw_statement<'src>(
    throw_stmt: &internal::ThrowStatement<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::ThrowStatement<'src> {
    public::ThrowStatement {
        node_type: "ThrowStatement",
        start: loc.pos(throw_stmt.span.start),
        end: loc.pos(throw_stmt.span.end),
        loc: create_location(throw_stmt.span, loc),
        argument: Box::new(convert_expression(
            &throw_stmt.argument,
            source,
            loc,
            interner,
        )),
    }
}

pub(in crate::ast) fn convert_break_statement<'src>(
    break_stmt: &internal::BreakStatement<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::BreakStatement<'src> {
    public::BreakStatement {
        node_type: "BreakStatement",
        start: loc.pos(break_stmt.span.start),
        end: loc.pos(break_stmt.span.end),
        loc: create_location(break_stmt.span, loc),
        label: break_stmt
            .label
            .as_ref()
            .map(|id| convert_identifier(id, source, loc, interner)),
    }
}

pub(in crate::ast) fn convert_continue_statement<'src>(
    continue_stmt: &internal::ContinueStatement<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::ContinueStatement<'src> {
    public::ContinueStatement {
        node_type: "ContinueStatement",
        start: loc.pos(continue_stmt.span.start),
        end: loc.pos(continue_stmt.span.end),
        loc: create_location(continue_stmt.span, loc),
        label: continue_stmt
            .label
            .as_ref()
            .map(|id| convert_identifier(id, source, loc, interner)),
    }
}

pub(in crate::ast) fn convert_labeled_statement<'src>(
    labeled: &internal::LabeledStatement<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::LabeledStatement<'src> {
    public::LabeledStatement {
        node_type: "LabeledStatement",
        start: loc.pos(labeled.span.start),
        end: loc.pos(labeled.span.end),
        loc: create_location(labeled.span, loc),
        label: convert_identifier(&labeled.label, source, loc, interner),
        body: Box::new(convert_statement(
            labeled.body,
            source,
            loc,
            interner,
            SCHEMA,
        )),
    }
}

// Helper functions

fn convert_for_init<'src>(
    init: &internal::ForInit<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::ForInit<'src> {
    match init {
        internal::ForInit::VariableDeclaration(decl) => public::ForInit::VariableDeclaration(
            convert_variable_declaration(decl, source, loc, interner),
        ),
        internal::ForInit::Expression(expr) => {
            public::ForInit::Expression(Box::new(convert_expression(expr, source, loc, interner)))
        }
    }
}

fn convert_for_in_of_left<'src>(
    left: &internal::ForInOfLeft<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::ForInOfLeft<'src> {
    match left {
        internal::ForInOfLeft::VariableDeclaration(decl) => {
            public::ForInOfLeft::VariableDeclaration(convert_variable_declaration(
                decl, source, loc, interner,
            ))
        }
        internal::ForInOfLeft::Pattern(expr) => {
            // The parser already refined the LHS through `to_assignable`, so a
            // destructuring LHS is an `ArrayPattern`/`ObjectPattern` in the
            // internal AST (deeply, with `RestElement`s) — `convert_expression`
            // mirrors it directly; no public-side relabel is needed.
            public::ForInOfLeft::Pattern(Box::new(convert_expression(expr, source, loc, interner)))
        }
    }
}

fn convert_switch_case<'src>(
    case: &internal::SwitchCase<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::SwitchCase<'src> {
    public::SwitchCase {
        node_type: "SwitchCase",
        start: loc.pos(case.span.start),
        end: loc.pos(case.span.end),
        loc: create_location(case.span, loc),
        test: case
            .test
            .as_ref()
            .map(|t| Box::new(convert_expression(t, source, loc, interner))),
        consequent: case
            .consequent
            .iter()
            .map(|s| convert_statement(s, source, loc, interner, SCHEMA))
            .collect(),
    }
}

fn convert_catch_clause<'src>(
    clause: &internal::CatchClause<'_>,
    source: &'src str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) -> public::CatchClause<'src> {
    public::CatchClause {
        node_type: "CatchClause",
        start: loc.pos(clause.span.start),
        end: loc.pos(clause.span.end),
        loc: create_location(clause.span, loc),
        param: clause
            .param
            .as_ref()
            .map(|p| Box::new(convert_expression(p, source, loc, interner))),
        body: convert_block_statement(&clause.body, source, loc, interner),
    }
}
