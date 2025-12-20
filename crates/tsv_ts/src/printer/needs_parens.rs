// Centralized parenthesization logic for TypeScript printer
//
// This module implements prettier's parenthesization system with a single entry point:
// `needs_parens(expr, ctx)` - determines if an expression needs parens in a given context.
//
// ## Architecture
//
// prettier's parenthesization (src/language-js/needs-parens.js) works by:
// - Switching on the node type (expression being printed)
// - Each case examines the parent context and key (which child position)
// - Returns true if parens needed, false otherwise
//
// We model the "parent context + key" as a `ParenContext` enum.
//
// ## References
// - prettier/src/language-js/needs-parens.js
// - prettier/src/language-js/print/index.js (application layer)

use crate::ast::internal::{BinaryOperator, Expression};

/// Context for parenthesization decisions
///
/// This enum captures WHERE an expression appears in the AST, which determines
/// whether it needs parentheses.
#[derive(Debug, Clone, Copy)]
pub enum ParenContext {
    /// Variable declarator init: `const x = <expr>`
    VariableInit,

    /// Expression statement: `<expr>;`
    ExpressionStatement,

    /// Binary left operand: `<expr> + y`
    BinaryLeft { parent_op: BinaryOperator },

    /// Binary right operand: `x + <expr>`
    BinaryRight { parent_op: BinaryOperator },

    /// Callee position: `<expr>()`
    Callee,

    /// New expression callee: `new <expr>()`
    NewCallee,

    /// Base of member/call chain: `<expr>.method()`
    ChainBase,

    /// Inside TSNonNullExpression: `<expr>!`
    NonNull,

    /// Left side of `as` or `satisfies`: `<expr> as T`
    TypeAssertion,

    /// Expression in TSInstantiationExpression: `<expr><T>`
    InstantiationExpression,

    /// Argument of unary operator: `!<expr>`, `typeof <expr>`
    UnaryArgument,

    /// Argument of await: `await <expr>`
    AwaitArgument,

    /// Arrow function body (expression form): `() => <expr>`
    ArrowBody,
}

/// Determines if an expression needs parentheses in a given context.
///
/// This is the central entry point for all parenthesization decisions.
pub fn needs_parens(expr: &Expression, ctx: ParenContext) -> bool {
    match ctx {
        // Assignment as value needs parens: `const x = (y = z);`
        ParenContext::VariableInit => matches!(expr, Expression::AssignmentExpression(_)),

        // Object pattern assignment needs parens: `({a} = x);`
        ParenContext::ExpressionStatement => needs_parens_expression_statement(expr),

        // Binary operand precedence
        ParenContext::BinaryLeft { parent_op } => {
            needs_parens_binary_operand(expr, parent_op, false)
        }
        ParenContext::BinaryRight { parent_op } => {
            needs_parens_binary_operand(expr, parent_op, true)
        }

        // Callee: `(a ? b : c)()`, `(a + b)()`, `(() => {})()`, etc.
        ParenContext::Callee | ParenContext::NewCallee => matches!(
            expr,
            Expression::ConditionalExpression(_)
                | Expression::BinaryExpression(_)
                | Expression::AssignmentExpression(_)
                | Expression::SequenceExpression(_)
                | Expression::ArrowFunctionExpression(_)
                | Expression::FunctionExpression(_)
        ),

        // Chain base: `(a + b).method()`, `(await x).method()`, etc.
        ParenContext::ChainBase => matches!(
            expr,
            Expression::BinaryExpression(_)
                | Expression::ConditionalExpression(_)
                | Expression::AssignmentExpression(_)
                | Expression::AwaitExpression(_)
                | Expression::TSAsExpression(_)
                | Expression::TSSatisfiesExpression(_)
                | Expression::TSTypeAssertion(_)
        ),

        // Non-null: `(a + b)!`, `(!x)!`, `(a ? b : c)!`, etc.
        ParenContext::NonNull => matches!(
            expr,
            Expression::BinaryExpression(_)
                | Expression::UnaryExpression(_)
                | Expression::ConditionalExpression(_)
                | Expression::AssignmentExpression(_)
                | Expression::AwaitExpression(_)
                | Expression::TSTypeAssertion(_)
                | Expression::TSAsExpression(_)
                | Expression::TSSatisfiesExpression(_)
        ),

        // Type assertion: `(a + b) as T`
        ParenContext::TypeAssertion => matches!(expr, Expression::BinaryExpression(_)),

        // Instantiation: `(<T>() => {})<U>`
        ParenContext::InstantiationExpression => matches!(
            expr,
            Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
        ),

        // Unary argument: `!(a + b)` - binary needs parens (higher precedence)
        ParenContext::UnaryArgument => matches!(expr, Expression::BinaryExpression(_)),

        // Await argument: `await (a + b)` - binary needs parens (higher precedence)
        ParenContext::AwaitArgument => matches!(expr, Expression::BinaryExpression(_)),

        // Arrow body: `() => ({})`, `() => (x = y)`, `() => (a ? b : c)`
        ParenContext::ArrowBody => matches!(
            expr,
            Expression::ObjectExpression(_)
                | Expression::AssignmentExpression(_)
                | Expression::ConditionalExpression(_)
        ),
    }
}

// =============================================================================
// Complex helpers (non-trivial logic)
// =============================================================================

/// Expression statement: `<expr>;`
/// Object pattern assignments need parens to avoid ambiguity with block statements
fn needs_parens_expression_statement(expr: &Expression) -> bool {
    match expr {
        Expression::AssignmentExpression(assign) => {
            matches!(assign.left.as_ref(), Expression::ObjectPattern(_))
        }
        Expression::SequenceExpression(seq) => seq
            .expressions
            .first()
            .is_some_and(needs_parens_expression_statement),
        _ => false,
    }
}

/// Binary operand: `<expr> op y` or `x op <expr>`
fn needs_parens_binary_operand(
    expr: &Expression,
    parent_op: BinaryOperator,
    is_right: bool,
) -> bool {
    let Expression::BinaryExpression(child) = expr else {
        return false;
    };
    let child_op = child.operator;

    // Special case: Logical operators (&&, ||, ??) mixing requires parens
    if parent_op.is_logical() && child_op.is_logical() && parent_op != child_op {
        return true;
    }

    let parent_prec = parent_op.precedence();
    let child_prec = child_op.precedence();

    // 1. Child has weaker precedence
    if child_prec < parent_prec {
        return true;
    }

    // 2. Right operand with same precedence - preserve programmer's grouping
    if is_right && child_prec == parent_prec {
        return true;
    }

    // 3. Same precedence but can't flatten
    if child_prec == parent_prec && !parent_op.can_flatten_with(child_op) {
        return true;
    }

    // 4. Special handling for modulo
    if parent_prec < child_prec && child_op == BinaryOperator::Percent {
        return matches!(parent_op, BinaryOperator::Plus | BinaryOperator::Minus)
            || parent_op.is_bitwise();
    }

    // 5. Bitwise operators with different precedence
    if parent_op.is_bitwise() && child_prec != parent_prec {
        return true;
    }

    false
}
