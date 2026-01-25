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

use crate::ast::internal::{BinaryOperator, Expression, LiteralValue};

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

    /// Callee position: `<expr>()` or tagged template tag: `<expr>`template``
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

    /// Object property value: `{key: <expr>}`
    ObjectPropertyValue,

    /// Spread element argument: `...<expr>`
    SpreadArgument,

    /// Call/array/new argument: `fn(<expr>)`, `[<expr>]`, `new Fn(<expr>)`
    /// Assignment expressions need parens for clarity
    Argument,

    /// Template literal expression: `${<expr>}`
    /// Assignment expressions need parens for clarity
    TemplateLiteralExpression,

    /// Computed property key: `{[<expr>]: value}`
    /// Assignment expressions need parens for clarity
    ComputedPropertyKey,
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

        // Callee: `(a ? b : c)()`, `(a + b)()`, `(() => {})()`, `(x as T)()`, `(<T>x)()`, etc.
        // Also used for tagged template tags: `(x as T)`template``
        // Note: SequenceExpression already adds its own parens in build_sequence_doc
        // Note: ClassExpression needs parens only in NewCallee: `class {}()` is valid but `new class {}()` is not
        ParenContext::Callee | ParenContext::NewCallee => {
            // ClassExpression only needs parens in `new` context
            if matches!(ctx, ParenContext::NewCallee)
                && matches!(expr, Expression::ClassExpression(_))
            {
                return true;
            }
            is_await_or_yield(expr)
                || is_type_assertion(expr)
                || is_function_like(expr)
                || matches!(
                    expr,
                    Expression::ConditionalExpression(_)
                        | Expression::BinaryExpression(_)
                        | Expression::AssignmentExpression(_)
                        | Expression::UnaryExpression(_)
                        | Expression::UpdateExpression(_)
                )
        }

        // Chain base: `(a + b).method()`, `(await x).method()`, `(yield x).method()`, etc.
        // Numeric literals need parens for `.method()` calls: `0.toString()` is invalid syntax.
        // Prettier normalizes `0..toString()` to `(0).toString()`.
        ParenContext::ChainBase => is_lower_precedence(expr) || is_numeric_literal(expr),

        // Spread argument: `...(a || b)`, `...(a ? b : c)`, `...(await x)`, `...(x as T)`
        ParenContext::SpreadArgument => is_lower_precedence(expr),

        // Non-null: `(a + b)!`, `(!x)!`, `(a ? b : c)!`, `(yield x)!`, etc.
        ParenContext::NonNull => {
            is_lower_precedence(expr) || matches!(expr, Expression::UnaryExpression(_))
        }

        // Type assertion: `(a + b) as T`, `(await x) as T`, `(yield x) as T`
        // Unary argument: `!(a + b)`, `!(await x)`, `!(yield x)` - parens for clarity/precedence
        ParenContext::TypeAssertion | ParenContext::UnaryArgument => {
            is_await_or_yield(expr) || matches!(expr, Expression::BinaryExpression(_))
        }

        // Instantiation: `(<T>() => {})<U>`, `(x as A)<T>`, `(<T>x)<U>`, `(await x)<T>`, `(yield x)<T>`
        ParenContext::InstantiationExpression => {
            is_await_or_yield(expr) || is_type_assertion(expr) || is_function_like(expr)
        }

        // Await argument: `await (a + b)`, `await (x as T)` - parens needed for precedence/semantics
        ParenContext::AwaitArgument => {
            matches!(
                expr,
                Expression::BinaryExpression(_)
                    | Expression::TSAsExpression(_)
                    | Expression::TSSatisfiesExpression(_)
            )
        }

        // Arrow body: `() => ({})`, `() => (x = y)`
        // Note: ConditionalExpression is handled specially in build_arrow_body_doc
        // using if_break - parens only when inline, not when on new line
        ParenContext::ArrowBody => matches!(
            expr,
            Expression::ObjectExpression(_) | Expression::AssignmentExpression(_)
        ),

        // Object property value: `{key: (a = b)}`
        // Assignment expressions need parens in object literals (not in ObjectPattern)
        ParenContext::ObjectPropertyValue => matches!(expr, Expression::AssignmentExpression(_)),

        // These contexts all need parens around assignment expressions for clarity:
        // - Call/array/new argument: `fn((a = b))`, `[(a = b)]`, `new Fn((a = b))`
        // - Template literal expression: `${(a = b)}`
        // - Computed property key: `{[(a = b)]: c}`
        ParenContext::Argument
        | ParenContext::TemplateLiteralExpression
        | ParenContext::ComputedPropertyKey => {
            matches!(expr, Expression::AssignmentExpression(_))
        }
    }
}

//
// Simple predicates (expression type groupings)
//

/// `await x` or `yield x` - always need parens together in most contexts
fn is_await_or_yield(expr: &Expression) -> bool {
    matches!(
        expr,
        Expression::AwaitExpression(_) | Expression::YieldExpression(_)
    )
}

/// Lower precedence expressions that need parens in chain/spread/non-null contexts
/// Combines: await/yield + type assertions + binary/conditional/assignment
fn is_lower_precedence(expr: &Expression) -> bool {
    is_await_or_yield(expr)
        || is_type_assertion(expr)
        || matches!(
            expr,
            Expression::BinaryExpression(_)
                | Expression::ConditionalExpression(_)
                | Expression::AssignmentExpression(_)
        )
}

/// `x as T`, `x satisfies T`, or `<T>x` - TypeScript type assertions
fn is_type_assertion(expr: &Expression) -> bool {
    matches!(
        expr,
        Expression::TSAsExpression(_)
            | Expression::TSSatisfiesExpression(_)
            | Expression::TSTypeAssertion(_)
    )
}

/// Arrow function or function expression
fn is_function_like(expr: &Expression) -> bool {
    matches!(
        expr,
        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
    )
}

/// Numeric literal - needs parens in chain base context because `0.toString()` is invalid.
/// Prettier normalizes `0..toString()` to `(0).toString()`.
fn is_numeric_literal(expr: &Expression) -> bool {
    matches!(
        expr,
        Expression::Literal(lit) if matches!(lit.value, LiteralValue::Number(_))
    )
}

//
// Complex helpers (non-trivial logic)
//

/// Expression statement: `<expr>;`
/// Object expressions and object pattern assignments need parens to avoid ambiguity
/// with block statements. `({...});` not `{...};`
fn needs_parens_expression_statement(expr: &Expression) -> bool {
    match expr {
        // Object expression: `({...});` needs parens to avoid being parsed as a block
        Expression::ObjectExpression(_) => true,
        // Object pattern assignment: `({a, b} = obj);` needs parens
        Expression::AssignmentExpression(assign) => {
            matches!(assign.left.as_ref(), Expression::ObjectPattern(_))
        }
        // Sequence: check the first expression
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
    // These expressions have lower precedence than any binary operator, so they ALWAYS
    // need parens when used as operands.
    // e.g., `a && (b ? c : d)` - without parens it becomes `(a && b) ? c : d`
    // e.g., `(x as string) in obj` - without parens it becomes `x as (string in obj)`
    // e.g., `b || ((fn) => fn)` - without parens it becomes `(b || fn) => fn` (syntax error)
    if matches!(
        expr,
        Expression::ConditionalExpression(_)
            | Expression::AssignmentExpression(_)
            | Expression::TSAsExpression(_)
            | Expression::TSSatisfiesExpression(_)
            | Expression::ArrowFunctionExpression(_)
    ) {
        return true;
    }

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
