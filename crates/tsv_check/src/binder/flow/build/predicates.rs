//! The pure AST predicates the flow walk dispatches on (`binder.go` /
//! `utilities.go` ports) — free functions with no `FlowBuilder` state, factored
//! out of the visitor modules. Purely a locality split — no behavior change.

use tsv_ts::ast::{
    AssignmentOperator, AstView, BinaryExpression, BinaryOperator, Expression, ExpressionKind,
    LiteralValue, Statement, StatementKind, UnaryOperator,
};

/// `is_potentially_executable` (utilities.go:4210) — the statement range (minus
/// `Block`/`Empty`, which are below the range), with `VariableStatement` gated
/// on block-scoping or an initializer, plus class/enum/module declarations.
pub(super) fn is_potentially_executable(view: AstView<'_>, stmt: &Statement) -> bool {
    use StatementKind as S;
    match &stmt.kind {
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
        | S::WithStatement(_)
        | S::SwitchStatement(_)
        | S::LabeledStatement(_)
        | S::ThrowStatement(_)
        | S::TryStatement(_)
        | S::DebuggerStatement(_) => true,
        S::VariableDeclaration(d) => {
            use tsv_ts::ast::VariableDeclarationKind as K;
            d.kind != K::Var
                || view
                    .declarators(d.declarations)
                    .iter()
                    .any(|decl| decl.init.is_some())
        }
        S::ClassDeclaration(_) | S::TSEnumDeclaration(_) | S::TSModuleDeclaration(_) => true,
        _ => false,
    }
}

/// Whether a statement kind is in tsc's `[FirstStatement, LastStatement]` range
/// (binder.go:1663) — the entry-flow write set. Excludes `Block`/`Empty` (below
/// the range) and every declaration kind (above it).
pub(super) fn is_statement_range(stmt: &Statement) -> bool {
    use StatementKind as S;
    matches!(
        &stmt.kind,
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
            | S::WithStatement(_)
            | S::SwitchStatement(_)
            | S::LabeledStatement(_)
            | S::ThrowStatement(_)
            | S::TryStatement(_)
            | S::DebuggerStatement(_)
    )
}

/// `IsDottedName` (utilities.go:1613).
pub(super) fn is_dotted_name(view: AstView<'_>, expr: &Expression) -> bool {
    use ExpressionKind as E;
    match &expr.kind {
        E::Identifier(_) | E::ThisExpression(_) | E::Super(_) | E::MetaProperty(_) => true,
        E::MemberExpression(m) if !m.computed => is_dotted_name(view, view.expr(m.object)),
        E::ParenthesizedExpression(p) => is_dotted_name(view, view.expr(p.expression)),
        _ => false,
    }
}

/// `isNarrowableReference` (binder.go:2633) — the access flow-write gate.
/// Adapted to tsv's AST (tsc's comma/assignment `BinaryExpression` cases are
/// tsv's `SequenceExpression` / `AssignmentExpression`).
pub(in crate::binder::flow) fn is_narrowable_reference(
    view: AstView<'_>,
    node: &Expression,
) -> bool {
    use ExpressionKind as E;
    match &node.kind {
        E::Identifier(_) | E::ThisExpression(_) | E::Super(_) | E::MetaProperty(_) => true,
        E::MemberExpression(m) if !m.computed => is_narrowable_reference(view, view.expr(m.object)),
        E::ParenthesizedExpression(p) => is_narrowable_reference(view, view.expr(p.expression)),
        E::TSNonNullExpression(t) => is_narrowable_reference(view, view.expr(t.expression)),
        E::MemberExpression(m) => {
            // computed element access
            let property = view.expr(m.property);
            is_string_or_numeric_literal_like(property)
                || (is_entity_name_expression(view, property)
                    && is_narrowable_reference(view, view.expr(m.object)))
        }
        E::AssignmentExpression(a) => is_left_hand_side_expression(view.expr(a.left)),
        E::SequenceExpression(s) => view
            .expr_list(s.expressions)
            .last()
            .is_some_and(|&e| is_narrowable_reference(view, view.expr(e))),
        _ => false,
    }
}

fn is_string_or_numeric_literal_like(node: &Expression) -> bool {
    matches!(
        &node.kind,
        ExpressionKind::Literal(l) if matches!(l.value, LiteralValue::String(_) | LiteralValue::Number(_))
    )
}

/// `IsEntityNameExpression` (utilities.go:1595) — an identifier or a dotted
/// property-access chain bottoming in one.
fn is_entity_name_expression(view: AstView<'_>, node: &Expression) -> bool {
    use ExpressionKind as E;
    match &node.kind {
        E::Identifier(_) => true,
        E::MemberExpression(m) if !m.computed => {
            matches!(view.expr(m.property).kind, E::Identifier(_))
                && is_entity_name_expression(view, view.expr(m.object))
        }
        _ => false,
    }
}

/// `isLeftHandSideExpressionKind` (utilities.go:396) — the postfix/primary
/// expression forms. Reached only via the rare `(x = y).z` narrowable case.
fn is_left_hand_side_expression(node: &Expression) -> bool {
    use ExpressionKind as E;
    matches!(
        node.kind,
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

pub(super) fn is_true_keyword(expr: &Expression) -> bool {
    matches!(&expr.kind, ExpressionKind::Literal(l) if matches!(l.value, LiteralValue::Boolean(true)))
}

pub(super) fn is_false_keyword(expr: &Expression) -> bool {
    matches!(&expr.kind, ExpressionKind::Literal(l) if matches!(l.value, LiteralValue::Boolean(false)))
}

/// Whether a condition node is a logical `&&`/`||`/`??` or a logical
/// compound-assignment `&&=`/`||=`/`??=` — the `bindCondition` non-atomic test
/// (binder.go:1801, combining `IsLogicalExpression` + `isLogicalAssignment`).
/// Such a node's sub-binder already wired the true/false targets, so
/// `bindCondition` must NOT re-add the atomic true/false conditions.
pub(super) fn is_logical_condition(e: &Expression) -> bool {
    match &e.kind {
        ExpressionKind::BinaryExpression(b) => b.operator.is_logical(),
        ExpressionKind::AssignmentExpression(a) => is_logical_assign_op(a.operator),
        _ => false,
    }
}

/// Whether an expression **threads** the enclosing condition targets into its
/// operands (vs being a value boundary that resets them). Mirrors the four
/// threading arms of `visit_expression`: `!`, `&&`/`||`/`??`, logical-assignment,
/// and parentheses — the same set tsgo's `isTopLevelLogicalExpression`
/// (binder.go:2782) ascends through. Every other expression is a value
/// sub-position (see the reset in `visit_expression`).
pub(super) fn is_condition_threading(e: &Expression) -> bool {
    match &e.kind {
        ExpressionKind::UnaryExpression(u) => u.operator == UnaryOperator::Bang,
        ExpressionKind::ParenthesizedExpression(_) => true,
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
pub(super) fn is_narrowing_expression(view: AstView<'_>, expr: &Expression) -> bool {
    use ExpressionKind as E;
    match &expr.kind {
        E::Identifier(_) | E::ThisExpression(_) => true,
        E::MemberExpression(_) => contains_narrowable_reference(view, expr),
        E::CallExpression(c) => {
            view.expr_list(c.arguments)
                .iter()
                .any(|&a| contains_narrowable_reference(view, view.expr(a)))
                || matches!(&view.expr(c.callee).kind, E::MemberExpression(m)
                    if !m.computed && contains_narrowable_reference(view, view.expr(m.object)))
        }
        E::ParenthesizedExpression(p) => is_narrowing_expression(view, view.expr(p.expression)),
        E::TSNonNullExpression(t) => is_narrowing_expression(view, view.expr(t.expression)),
        E::UnaryExpression(u)
            if u.operator == UnaryOperator::Typeof || u.operator == UnaryOperator::Bang =>
        {
            is_narrowing_expression(view, view.expr(u.argument))
        }
        E::BinaryExpression(b) => is_narrowing_binary_expression(view, b),
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
            ) && contains_narrowable_reference(view, view.expr(a.left))
        }
        // The `isNarrowingBinaryExpression` comma case (`isNarrowingExpression`
        // of the last operand).
        E::SequenceExpression(s) => view
            .expr_list(s.expressions)
            .last()
            .is_some_and(|&e| is_narrowing_expression(view, view.expr(e))),
        _ => false,
    }
}

/// `containsNarrowableReference` (binder.go:2620) — a narrowable reference, or an
/// optional-chain node whose object/callee contains one.
fn contains_narrowable_reference(view: AstView<'_>, expr: &Expression) -> bool {
    if is_narrowable_reference(view, expr) {
        return true;
    }
    match &expr.kind {
        ExpressionKind::MemberExpression(m) if expr.has_optional_in_chain(view) => {
            contains_narrowable_reference(view, view.expr(m.object))
        }
        ExpressionKind::CallExpression(c) if expr.has_optional_in_chain(view) => {
            contains_narrowable_reference(view, view.expr(c.callee))
        }
        ExpressionKind::TSNonNullExpression(n) if expr.has_optional_in_chain(view) => {
            contains_narrowable_reference(view, view.expr(n.expression))
        }
        _ => false,
    }
}

/// `isNarrowingBinaryExpression` (binder.go:2666) for tsv's `BinaryExpression`
/// (which never carries the comma / assignment operators — those are separate
/// nodes, handled in `is_narrowing_expression`).
fn is_narrowing_binary_expression(view: AstView<'_>, b: &BinaryExpression) -> bool {
    use BinaryOperator as Op;
    match b.operator {
        Op::EqualsEquals | Op::BangEquals | Op::EqualsEqualsEquals | Op::BangEqualsEquals => {
            let left = skip_parens(view, view.expr(b.left));
            let right = skip_parens(view, view.expr(b.right));
            is_narrowable_operand(view, left)
                || is_narrowable_operand(view, right)
                || is_narrowing_typeof_operands(view, right, left)
                || is_narrowing_typeof_operands(view, left, right)
                || (is_boolean_literal(right) && is_narrowing_expression(view, left))
                || (is_boolean_literal(left) && is_narrowing_expression(view, right))
        }
        Op::Instanceof => is_narrowable_operand(view, view.expr(b.left)),
        Op::In => is_narrowing_expression(view, view.expr(b.right)),
        _ => false,
    }
}

/// `isNarrowableOperand` (binder.go:2686).
fn is_narrowable_operand(view: AstView<'_>, expr: &Expression) -> bool {
    match &expr.kind {
        ExpressionKind::ParenthesizedExpression(p) => {
            is_narrowable_operand(view, view.expr(p.expression))
        }
        ExpressionKind::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => {
            is_narrowable_operand(view, view.expr(a.left))
        }
        ExpressionKind::SequenceExpression(s) => view
            .expr_list(s.expressions)
            .last()
            .is_some_and(|&e| is_narrowable_operand(view, view.expr(e))),
        _ => contains_narrowable_reference(view, expr),
    }
}

/// `isNarrowingTypeOfOperands` (binder.go:2702) — `typeof <operand> === <string>`.
fn is_narrowing_typeof_operands(view: AstView<'_>, expr1: &Expression, expr2: &Expression) -> bool {
    matches!(&expr1.kind, ExpressionKind::UnaryExpression(u)
        if u.operator == UnaryOperator::Typeof && is_narrowable_operand(view, view.expr(u.argument)))
        && is_string_literal_like(expr2)
}

/// `IsStringLiteralLike` — a string literal or a no-substitution template.
fn is_string_literal_like(e: &Expression) -> bool {
    match &e.kind {
        ExpressionKind::Literal(l) => matches!(l.value, LiteralValue::String(_)),
        ExpressionKind::TemplateLiteral(t) => t.expressions.is_empty(),
        _ => false,
    }
}

/// `IsBooleanLiteral` — a `true` / `false` keyword literal.
fn is_boolean_literal(e: &Expression) -> bool {
    matches!(&e.kind, ExpressionKind::Literal(l) if matches!(l.value, LiteralValue::Boolean(_)))
}

/// `SkipParentheses` — strip grouping `ParenthesizedExpression` wrappers (rare in
/// tsv, which discards grouping parens except under `preserve_parens`).
fn skip_parens<'a>(view: AstView<'a>, e: &'a Expression) -> &'a Expression {
    let mut e = e;
    while let ExpressionKind::ParenthesizedExpression(p) = &e.kind {
        e = view.expr(p.expression);
    }
    e
}
