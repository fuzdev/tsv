// Control flow statement conversions

use super::super::{internal, public};
use super::{
    Schema, convert_block_statement, convert_expression, convert_statement,
    convert_variable_declaration, create_location,
};
use string_interner::DefaultStringInterner;
use tsv_lang::LocationTracker;

/// Schema for control flow statement bodies.
///
/// Control flow bodies (if/for/while/etc.) never contain import/export
/// declarations, so the schema doesn't matter. We use `Acorn` (the default)
/// for simplicity.
const SCHEMA: Schema = Schema::Acorn;

pub(in crate::ast) fn convert_if_statement(
    if_stmt: &internal::IfStatement,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::IfStatement {
    public::IfStatement {
        node_type: "IfStatement".to_string(),
        start: if_stmt.span.start,
        end: if_stmt.span.end,
        loc: create_location(if_stmt.span, loc, offset),
        test: Box::new(convert_expression(
            &if_stmt.test,
            source,
            loc,
            interner,
            offset,
        )),
        consequent: Box::new(convert_statement(
            &if_stmt.consequent,
            source,
            loc,
            interner,
            offset,
            SCHEMA,
        )),
        alternate: if_stmt.alternate.as_ref().map(|alt| {
            Box::new(convert_statement(
                alt, source, loc, interner, offset, SCHEMA,
            ))
        }),
    }
}

pub(in crate::ast) fn convert_for_statement(
    for_stmt: &internal::ForStatement,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ForStatement {
    public::ForStatement {
        node_type: "ForStatement".to_string(),
        start: for_stmt.span.start,
        end: for_stmt.span.end,
        loc: create_location(for_stmt.span, loc, offset),
        init: for_stmt
            .init
            .as_ref()
            .map(|init| convert_for_init(init, source, loc, interner, offset)),
        test: for_stmt
            .test
            .as_ref()
            .map(|test| Box::new(convert_expression(test, source, loc, interner, offset))),
        update: for_stmt
            .update
            .as_ref()
            .map(|update| Box::new(convert_expression(update, source, loc, interner, offset))),
        body: Box::new(convert_statement(
            &for_stmt.body,
            source,
            loc,
            interner,
            offset,
            SCHEMA,
        )),
    }
}

pub(in crate::ast) fn convert_for_in_statement(
    for_in: &internal::ForInStatement,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ForInStatement {
    public::ForInStatement {
        node_type: "ForInStatement".to_string(),
        start: for_in.span.start,
        end: for_in.span.end,
        loc: create_location(for_in.span, loc, offset),
        left: convert_for_in_of_left(&for_in.left, source, loc, interner, offset),
        right: Box::new(convert_expression(
            &for_in.right,
            source,
            loc,
            interner,
            offset,
        )),
        body: Box::new(convert_statement(
            &for_in.body,
            source,
            loc,
            interner,
            offset,
            SCHEMA,
        )),
    }
}

pub(in crate::ast) fn convert_for_of_statement(
    for_of: &internal::ForOfStatement,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ForOfStatement {
    public::ForOfStatement {
        node_type: "ForOfStatement".to_string(),
        start: for_of.span.start,
        end: for_of.span.end,
        loc: create_location(for_of.span, loc, offset),
        left: convert_for_in_of_left(&for_of.left, source, loc, interner, offset),
        right: Box::new(convert_expression(
            &for_of.right,
            source,
            loc,
            interner,
            offset,
        )),
        r#await: for_of.r#await,
        body: Box::new(convert_statement(
            &for_of.body,
            source,
            loc,
            interner,
            offset,
            SCHEMA,
        )),
    }
}

pub(in crate::ast) fn convert_while_statement(
    while_stmt: &internal::WhileStatement,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::WhileStatement {
    public::WhileStatement {
        node_type: "WhileStatement".to_string(),
        start: while_stmt.span.start,
        end: while_stmt.span.end,
        loc: create_location(while_stmt.span, loc, offset),
        test: Box::new(convert_expression(
            &while_stmt.test,
            source,
            loc,
            interner,
            offset,
        )),
        body: Box::new(convert_statement(
            &while_stmt.body,
            source,
            loc,
            interner,
            offset,
            SCHEMA,
        )),
    }
}

pub(in crate::ast) fn convert_do_while_statement(
    do_while: &internal::DoWhileStatement,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::DoWhileStatement {
    public::DoWhileStatement {
        node_type: "DoWhileStatement".to_string(),
        start: do_while.span.start,
        end: do_while.span.end,
        loc: create_location(do_while.span, loc, offset),
        body: Box::new(convert_statement(
            &do_while.body,
            source,
            loc,
            interner,
            offset,
            SCHEMA,
        )),
        test: Box::new(convert_expression(
            &do_while.test,
            source,
            loc,
            interner,
            offset,
        )),
    }
}

pub(in crate::ast) fn convert_switch_statement(
    switch_stmt: &internal::SwitchStatement,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::SwitchStatement {
    public::SwitchStatement {
        node_type: "SwitchStatement".to_string(),
        start: switch_stmt.span.start,
        end: switch_stmt.span.end,
        loc: create_location(switch_stmt.span, loc, offset),
        discriminant: Box::new(convert_expression(
            &switch_stmt.discriminant,
            source,
            loc,
            interner,
            offset,
        )),
        cases: switch_stmt
            .cases
            .iter()
            .map(|case| convert_switch_case(case, source, loc, interner, offset))
            .collect(),
    }
}

pub(in crate::ast) fn convert_try_statement(
    try_stmt: &internal::TryStatement,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::TryStatement {
    public::TryStatement {
        node_type: "TryStatement".to_string(),
        start: try_stmt.span.start,
        end: try_stmt.span.end,
        loc: create_location(try_stmt.span, loc, offset),
        block: convert_block_statement(&try_stmt.block, source, loc, interner, offset),
        handler: try_stmt
            .handler
            .as_ref()
            .map(|h| convert_catch_clause(h, source, loc, interner, offset)),
        finalizer: try_stmt
            .finalizer
            .as_ref()
            .map(|f| convert_block_statement(f, source, loc, interner, offset)),
    }
}

pub(in crate::ast) fn convert_throw_statement(
    throw_stmt: &internal::ThrowStatement,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ThrowStatement {
    public::ThrowStatement {
        node_type: "ThrowStatement".to_string(),
        start: throw_stmt.span.start,
        end: throw_stmt.span.end,
        loc: create_location(throw_stmt.span, loc, offset),
        argument: Box::new(convert_expression(
            &throw_stmt.argument,
            source,
            loc,
            interner,
            offset,
        )),
    }
}

pub(in crate::ast) fn convert_break_statement(
    break_stmt: &internal::BreakStatement,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::BreakStatement {
    public::BreakStatement {
        node_type: "BreakStatement".to_string(),
        start: break_stmt.span.start,
        end: break_stmt.span.end,
        loc: create_location(break_stmt.span, loc, offset),
        label: break_stmt
            .label
            .as_ref()
            .map(|id| super::convert_identifier(id, loc, interner, offset)),
    }
}

pub(in crate::ast) fn convert_continue_statement(
    continue_stmt: &internal::ContinueStatement,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ContinueStatement {
    public::ContinueStatement {
        node_type: "ContinueStatement".to_string(),
        start: continue_stmt.span.start,
        end: continue_stmt.span.end,
        loc: create_location(continue_stmt.span, loc, offset),
        label: continue_stmt
            .label
            .as_ref()
            .map(|id| super::convert_identifier(id, loc, interner, offset)),
    }
}

pub(in crate::ast) fn convert_labeled_statement(
    labeled: &internal::LabeledStatement,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::LabeledStatement {
    public::LabeledStatement {
        node_type: "LabeledStatement".to_string(),
        start: labeled.span.start,
        end: labeled.span.end,
        loc: create_location(labeled.span, loc, offset),
        label: super::convert_identifier(&labeled.label, loc, interner, offset),
        body: Box::new(convert_statement(
            &labeled.body,
            source,
            loc,
            interner,
            offset,
            SCHEMA,
        )),
    }
}

// Helper functions

fn convert_for_init(
    init: &internal::ForInit,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ForInit {
    match init {
        internal::ForInit::VariableDeclaration(decl) => public::ForInit::VariableDeclaration(
            convert_variable_declaration(decl, source, loc, interner, offset),
        ),
        internal::ForInit::Expression(expr) => public::ForInit::Expression(Box::new(
            convert_expression(expr, source, loc, interner, offset),
        )),
    }
}

fn convert_for_in_of_left(
    left: &internal::ForInOfLeft,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::ForInOfLeft {
    match left {
        internal::ForInOfLeft::VariableDeclaration(decl) => {
            public::ForInOfLeft::VariableDeclaration(convert_variable_declaration(
                decl, source, loc, interner, offset,
            ))
        }
        internal::ForInOfLeft::Pattern(expr) => {
            let converted = convert_expression(expr, source, loc, interner, offset);
            public::ForInOfLeft::Pattern(Box::new(expression_to_pattern(converted)))
        }
    }
}

/// Convert expression types to pattern types for for-in/of LHS.
/// Acorn converts `ObjectExpression` → `ObjectPattern` and `ArrayExpression` → `ArrayPattern`
/// when used in the left-hand side of for-in/of statements.
fn expression_to_pattern(expr: public::Expression) -> public::Expression {
    match expr {
        public::Expression::ObjectExpression(obj) => {
            public::Expression::ObjectPattern(public::ObjectPattern {
                node_type: "ObjectPattern".to_string(),
                start: obj.start,
                end: obj.end,
                loc: obj.loc,
                properties: obj
                    .properties
                    .into_iter()
                    .map(|p| match p {
                        public::ObjectProperty::Property(p) => {
                            public::ObjectPatternProperty::Property(p)
                        }
                        public::ObjectProperty::SpreadElement(s) => {
                            public::ObjectPatternProperty::RestElement(public::RestElement {
                                node_type: "RestElement".to_string(),
                                start: s.start,
                                end: s.end,
                                loc: s.loc,
                                argument: s.argument,
                                type_annotation: None,
                            })
                        }
                    })
                    .collect(),
                type_annotation: None,
            })
        }
        public::Expression::ArrayExpression(arr) => {
            public::Expression::ArrayPattern(public::ArrayPattern {
                node_type: "ArrayPattern".to_string(),
                start: arr.start,
                end: arr.end,
                loc: arr.loc,
                elements: arr.elements,
                type_annotation: None,
            })
        }
        other => other,
    }
}

fn convert_switch_case(
    case: &internal::SwitchCase,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::SwitchCase {
    public::SwitchCase {
        node_type: "SwitchCase".to_string(),
        start: case.span.start,
        end: case.span.end,
        loc: create_location(case.span, loc, offset),
        test: case
            .test
            .as_ref()
            .map(|t| Box::new(convert_expression(t, source, loc, interner, offset))),
        consequent: case
            .consequent
            .iter()
            .map(|s| convert_statement(s, source, loc, interner, offset, SCHEMA))
            .collect(),
    }
}

fn convert_catch_clause(
    clause: &internal::CatchClause,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::CatchClause {
    public::CatchClause {
        node_type: "CatchClause".to_string(),
        start: clause.span.start,
        end: clause.span.end,
        loc: create_location(clause.span, loc, offset),
        param: clause
            .param
            .as_ref()
            .map(|p| Box::new(convert_expression(p, source, loc, interner, offset))),
        body: convert_block_statement(&clause.body, source, loc, interner, offset),
    }
}
