//! The pure AST predicates the flow walk dispatches on (`binder.go` /
//! `utilities.go` ports) — free functions with no `FlowBuilder` state, factored
//! out of the visitor modules. Purely a locality split — no behavior change.

use tsv_ts::ast::internal::{
    AssignmentOperator, BinaryExpression, BinaryOperator, Expression, LiteralValue, Statement,
    UnaryOperator,
};

/// `is_potentially_executable` (utilities.go:4210) — the statement range (minus
/// `Block`/`Empty`, which are below the range), with `VariableStatement` gated
/// on block-scoping or an initializer, plus class/enum/module declarations.
pub(super) fn is_potentially_executable(stmt: &Statement<'_>) -> bool {
    use Statement as S;
    match stmt {
        S::ExpressionStatement(_)
        | S::IfStatement(_)
        | S::DoWhileStatement(_)
        | S::WhileStatement(_)
        | S::ForStatement(_)
        | S::ForInStatement(_)
        | S::ForOfStatement(_)
        | S::ContinueStatement(_)
        | S::BreakStatement(_)
        | S::ReturnStatement(_)
        | S::SwitchStatement(_)
        | S::LabeledStatement(_)
        | S::ThrowStatement(_)
        | S::TryStatement(_)
        | S::DebuggerStatement(_) => true,
        S::VariableDeclaration(d) => {
            use tsv_ts::ast::internal::VariableDeclarationKind as K;
            d.kind != K::Var || d.declarations.iter().any(|decl| decl.init.is_some())
        }
        S::ClassDeclaration(_) | S::TSEnumDeclaration(_) | S::TSModuleDeclaration(_) => true,
        _ => false,
    }
}

/// Whether a statement kind is in tsc's `[FirstStatement, LastStatement]` range
/// (binder.go:1663) — the entry-flow write set. Excludes `Block`/`Empty` (below
/// the range) and every declaration kind (above it).
pub(super) fn is_statement_range(stmt: &Statement<'_>) -> bool {
    use Statement as S;
    matches!(
        stmt,
        S::ExpressionStatement(_)
            | S::VariableDeclaration(_)
            | S::IfStatement(_)
            | S::DoWhileStatement(_)
            | S::WhileStatement(_)
            | S::ForStatement(_)
            | S::ForInStatement(_)
            | S::ForOfStatement(_)
            | S::ContinueStatement(_)
            | S::BreakStatement(_)
            | S::ReturnStatement(_)
            | S::SwitchStatement(_)
            | S::LabeledStatement(_)
            | S::ThrowStatement(_)
            | S::TryStatement(_)
            | S::DebuggerStatement(_)
    )
}

/// `IsDottedName` (utilities.go:1613).
pub(super) fn is_dotted_name(expr: &Expression<'_>) -> bool {
    use Expression as E;
    match expr {
        E::Identifier(_) | E::ThisExpression(_) | E::Super(_) | E::MetaProperty(_) => true,
        E::MemberExpression(m) if !m.computed => is_dotted_name(m.object),
        E::ParenthesizedExpression(p) => is_dotted_name(p.expression),
        _ => false,
    }
}

/// `isNarrowableReference` (binder.go:2633) — the access flow-write gate.
/// Adapted to tsv's AST (tsc's comma/assignment `BinaryExpression` cases are
/// tsv's `SequenceExpression` / `AssignmentExpression`).
pub(in crate::binder::flow) fn is_narrowable_reference(node: &Expression<'_>) -> bool {
    use Expression as E;
    match node {
        E::Identifier(_) | E::ThisExpression(_) | E::Super(_) | E::MetaProperty(_) => true,
        E::MemberExpression(m) if !m.computed => is_narrowable_reference(m.object),
        E::ParenthesizedExpression(p) => is_narrowable_reference(p.expression),
        E::TSNonNullExpression(t) => is_narrowable_reference(t.expression),
        E::MemberExpression(m) => {
            // computed element access
            is_string_or_numeric_literal_like(m.property)
                || (is_entity_name_expression(m.property) && is_narrowable_reference(m.object))
        }
        E::AssignmentExpression(a) => is_left_hand_side_expression(a.left),
        E::SequenceExpression(s) => s.expressions.last().is_some_and(is_narrowable_reference),
        _ => false,
    }
}

fn is_string_or_numeric_literal_like(node: &Expression<'_>) -> bool {
    matches!(
        node,
        Expression::Literal(l) if matches!(l.value, LiteralValue::String(_) | LiteralValue::Number(_))
    )
}

/// `IsEntityNameExpression` (utilities.go:1595) — an identifier or a dotted
/// property-access chain bottoming in one.
fn is_entity_name_expression(node: &Expression<'_>) -> bool {
    use Expression as E;
    match node {
        E::Identifier(_) => true,
        E::MemberExpression(m) if !m.computed => {
            matches!(m.property, E::Identifier(_)) && is_entity_name_expression(m.object)
        }
        _ => false,
    }
}

/// `isLeftHandSideExpressionKind` (utilities.go:396) — the postfix/primary
/// expression forms. Reached only via the rare `(x = y).z` narrowable case.
fn is_left_hand_side_expression(node: &Expression<'_>) -> bool {
    use Expression as E;
    matches!(
        node,
        E::MemberExpression(_)
            | E::NewExpression(_)
            | E::CallExpression(_)
            | E::TaggedTemplateExpression(_)
            | E::ArrayExpression(_)
            | E::ParenthesizedExpression(_)
            | E::ObjectExpression(_)
            | E::ClassExpression(_)
            | E::FunctionExpression(_)
            | E::Identifier(_)
            | E::PrivateIdentifier(_)
            | E::RegexLiteral(_)
            | E::Literal(_)
            | E::TemplateLiteral(_)
            | E::ThisExpression(_)
            | E::Super(_)
            | E::TSNonNullExpression(_)
            | E::MetaProperty(_)
            | E::ImportExpression(_)
    )
}

pub(super) fn is_true_keyword(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::Literal(l) if matches!(l.value, LiteralValue::Boolean(true)))
}

pub(super) fn is_false_keyword(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::Literal(l) if matches!(l.value, LiteralValue::Boolean(false)))
}

/// Whether a condition node is a logical `&&`/`||`/`??` or a logical
/// compound-assignment `&&=`/`||=`/`??=` — the `bindCondition` non-atomic test
/// (binder.go:1801, combining `IsLogicalExpression` + `isLogicalAssignment`).
/// Such a node's sub-binder already wired the true/false targets, so
/// `bindCondition` must NOT re-add the atomic true/false conditions.
pub(super) fn is_logical_condition(e: &Expression<'_>) -> bool {
    match e {
        Expression::BinaryExpression(b) => b.operator.is_logical(),
        Expression::AssignmentExpression(a) => is_logical_assign_op(a.operator),
        _ => false,
    }
}

/// Whether an expression **threads** the enclosing condition targets into its
/// operands (vs being a value boundary that resets them). Mirrors the four
/// threading arms of `visit_expression`: `!`, `&&`/`||`/`??`, logical-assignment,
/// and parentheses — the same set tsgo's `isTopLevelLogicalExpression`
/// (binder.go:2782) ascends through. Every other expression is a value
/// sub-position (see the reset in `visit_expression`).
pub(super) fn is_condition_threading(e: &Expression<'_>) -> bool {
    match e {
        Expression::UnaryExpression(u) => u.operator == UnaryOperator::Bang,
        Expression::ParenthesizedExpression(_) => true,
        _ => is_logical_condition(e),
    }
}

/// Whether an assignment operator is a logical compound-assignment
/// (`||=`/`&&=`/`??=`) — `IsLogicalOrCoalescingAssignmentOperator`.
pub(super) fn is_logical_assign_op(op: AssignmentOperator) -> bool {
    matches!(
        op,
        AssignmentOperator::LogicalOrAssign
            | AssignmentOperator::LogicalAndAssign
            | AssignmentOperator::NullishAssign
    )
}

/// `isNarrowingExpression` (binder.go:2602) — the `createFlowCondition` gate.
/// Adapted to tsv's AST: comma / assignment are their own `SequenceExpression` /
/// `AssignmentExpression` nodes (tsc folds them into `BinaryExpression`), so their
/// `isNarrowingBinaryExpression` cases move here.
pub(super) fn is_narrowing_expression(expr: &Expression<'_>) -> bool {
    use Expression as E;
    match expr {
        E::Identifier(_) | E::ThisExpression(_) => true,
        E::MemberExpression(_) => contains_narrowable_reference(expr),
        E::CallExpression(c) => {
            c.arguments.iter().any(contains_narrowable_reference)
                || matches!(c.callee, E::MemberExpression(m)
                    if !m.computed && contains_narrowable_reference(m.object))
        }
        E::ParenthesizedExpression(p) => is_narrowing_expression(p.expression),
        E::TSNonNullExpression(t) => is_narrowing_expression(t.expression),
        E::UnaryExpression(u)
            if u.operator == UnaryOperator::Typeof || u.operator == UnaryOperator::Bang =>
        {
            is_narrowing_expression(u.argument)
        }
        E::BinaryExpression(b) => is_narrowing_binary_expression(b),
        // The `isNarrowingBinaryExpression` assignment cases (`=`/`||=`/`&&=`/`??=`
        // → containsNarrowableReference(left)); other compound assignments are not
        // narrowing.
        E::AssignmentExpression(a) => {
            matches!(
                a.operator,
                AssignmentOperator::Assign
                    | AssignmentOperator::LogicalOrAssign
                    | AssignmentOperator::LogicalAndAssign
                    | AssignmentOperator::NullishAssign
            ) && contains_narrowable_reference(a.left)
        }
        // The `isNarrowingBinaryExpression` comma case (`isNarrowingExpression`
        // of the last operand).
        E::SequenceExpression(s) => s.expressions.last().is_some_and(is_narrowing_expression),
        _ => false,
    }
}

/// `containsNarrowableReference` (binder.go:2620) — a narrowable reference, or an
/// optional-chain node whose object/callee contains one.
fn contains_narrowable_reference(expr: &Expression<'_>) -> bool {
    if is_narrowable_reference(expr) {
        return true;
    }
    match expr {
        Expression::MemberExpression(m) if expr.has_optional_in_chain() => {
            contains_narrowable_reference(m.object)
        }
        Expression::CallExpression(c) if expr.has_optional_in_chain() => {
            contains_narrowable_reference(c.callee)
        }
        Expression::TSNonNullExpression(n) if expr.has_optional_in_chain() => {
            contains_narrowable_reference(n.expression)
        }
        _ => false,
    }
}

/// `isNarrowingBinaryExpression` (binder.go:2666) for tsv's `BinaryExpression`
/// (which never carries the comma / assignment operators — those are separate
/// nodes, handled in `is_narrowing_expression`).
fn is_narrowing_binary_expression(b: &BinaryExpression<'_>) -> bool {
    use BinaryOperator as Op;
    match b.operator {
        Op::EqualsEquals | Op::BangEquals | Op::EqualsEqualsEquals | Op::BangEqualsEquals => {
            let left = skip_parens(b.left);
            let right = skip_parens(b.right);
            is_narrowable_operand(left)
                || is_narrowable_operand(right)
                || is_narrowing_typeof_operands(right, left)
                || is_narrowing_typeof_operands(left, right)
                || (is_boolean_literal(right) && is_narrowing_expression(left))
                || (is_boolean_literal(left) && is_narrowing_expression(right))
        }
        Op::Instanceof => is_narrowable_operand(b.left),
        Op::In => is_narrowing_expression(b.right),
        _ => false,
    }
}

/// `isNarrowableOperand` (binder.go:2686).
fn is_narrowable_operand(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::ParenthesizedExpression(p) => is_narrowable_operand(p.expression),
        Expression::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => {
            is_narrowable_operand(a.left)
        }
        Expression::SequenceExpression(s) => {
            s.expressions.last().is_some_and(is_narrowable_operand)
        }
        _ => contains_narrowable_reference(expr),
    }
}

/// `isNarrowingTypeOfOperands` (binder.go:2702) — `typeof <operand> === <string>`.
fn is_narrowing_typeof_operands(expr1: &Expression<'_>, expr2: &Expression<'_>) -> bool {
    matches!(expr1, Expression::UnaryExpression(u)
        if u.operator == UnaryOperator::Typeof && is_narrowable_operand(u.argument))
        && is_string_literal_like(expr2)
}

/// `IsStringLiteralLike` — a string literal or a no-substitution template.
fn is_string_literal_like(e: &Expression<'_>) -> bool {
    match e {
        Expression::Literal(l) => matches!(l.value, LiteralValue::String(_)),
        Expression::TemplateLiteral(t) => t.expressions.is_empty(),
        _ => false,
    }
}

/// `IsBooleanLiteral` — a `true` / `false` keyword literal.
fn is_boolean_literal(e: &Expression<'_>) -> bool {
    matches!(e, Expression::Literal(l) if matches!(l.value, LiteralValue::Boolean(_)))
}

/// `SkipParentheses` — strip grouping `ParenthesizedExpression` wrappers (rare in
/// tsv, which discards grouping parens except under `preserve_parens`).
fn skip_parens<'a, 'arena>(e: &'a Expression<'arena>) -> &'a Expression<'arena> {
    let mut e = e;
    while let Expression::ParenthesizedExpression(p) = e {
        e = p.expression;
    }
    e
}
