//! Shared utility functions for the TypeScript printer

use crate::ast::internal::{self, Expression};
use tsv_lang::printing::has_newline_between;

/// Check if an argument is "hopefully short" enough to stay inline
///
/// Matches Prettier's `isHopefullyShortCallArgument` logic, which is STRICTER
/// than `isSimpleCallArgument`. Key differences:
/// - Call expressions with > 1 argument are NOT short (even if structurally simple)
/// - Binary expressions check both sides with depth=1
///
/// Used to determine if tail args can stay inline after a function callback.
pub(super) fn is_hopefully_short_arg(expr: &Expression) -> bool {
    match expr {
        // Prettier: if (isCallLikeExpression(node) && getCallArguments(node).length > 1) return false
        Expression::CallExpression(call) if call.arguments.len() > 1 => false,
        Expression::NewExpression(new_expr) if new_expr.arguments.len() > 1 => false,

        // Prettier: if (isBinaryish(node)) check both sides with depth=1
        // Note: Our AST uses BinaryExpression for logical ops (&&, ||, ??) too
        Expression::BinaryExpression(bin) => {
            is_simple_call_argument(&bin.left, 1) && is_simple_call_argument(&bin.right, 1)
        }

        // Prettier: return isRegExpLiteral(node) || isSimpleCallArgument(node)
        // RegExp literals are handled by is_simple_call_argument (via Literal)
        _ => is_simple_call_argument(expr, 2),
    }
}

/// Check if an expression is an object that could expand (has properties)
/// Used for "expand last arg" pattern in import expressions
pub(super) fn is_expandable_object(expr: &Expression) -> bool {
    matches!(expr, Expression::ObjectExpression(obj) if !obj.properties.is_empty())
}

/// Check if an arrow function body is a ternary expression
///
/// Matches Prettier's `couldExpandArg` logic for conditional expressions in arrow bodies.
/// When true, the arrow should be printed with conditional parens around the body:
/// - Flat: `(x) => (x ? y : z)` - parens prevent ambiguity with `<=`
/// - Break: `(x) =>\n  x ? y : z,` - no parens needed, clearly arrow body
pub(super) fn could_expand_arrow_body(body: &Expression) -> bool {
    // Only ternary expressions need the special conditional paren treatment
    // Call expressions, objects, arrays are handled by other code paths
    matches!(body, Expression::ConditionalExpression(_))
}

/// Check if the last argument is an array or object expression (unwrapping type assertions)
#[inline]
pub(super) fn last_arg_is_array_or_object(arguments: &[Expression]) -> bool {
    arguments
        .last()
        .is_some_and(is_array_or_object_unwrapped)
}

/// Check if an expression is an array or object, unwrapping TS type wrappers
pub(super) fn is_array_or_object_unwrapped(expr: &Expression) -> bool {
    matches!(
        unwrap_ts_type_wrappers(expr),
        Expression::ArrayExpression(_) | Expression::ObjectExpression(_)
    )
}

/// Unwrap TypeScript type wrappers (as, satisfies, <T>, !) to get the inner expression.
/// Returns the innermost non-wrapper expression.
fn unwrap_ts_type_wrappers(expr: &Expression) -> &Expression {
    match expr {
        Expression::TSAsExpression(e) => unwrap_ts_type_wrappers(&e.expression),
        Expression::TSSatisfiesExpression(e) => unwrap_ts_type_wrappers(&e.expression),
        Expression::TSTypeAssertion(e) => unwrap_ts_type_wrappers(&e.expression),
        Expression::TSNonNullExpression(e) => unwrap_ts_type_wrappers(&e.expression),
        _ => expr,
    }
}

/// Get the inner expression if this is a TS type wrapper, otherwise None.
fn get_ts_type_wrapper_inner(expr: &Expression) -> Option<&Expression> {
    match expr {
        Expression::TSAsExpression(e) => Some(&e.expression),
        Expression::TSSatisfiesExpression(e) => Some(&e.expression),
        Expression::TSTypeAssertion(e) => Some(&e.expression),
        Expression::TSNonNullExpression(e) => Some(&e.expression),
        _ => None,
    }
}

/// Check if all arguments before the last are short/simple
///
/// Used to determine if we can keep short args inline with a complex last arg.
#[inline]
fn preceding_args_are_short(arguments: &[Expression]) -> bool {
    arguments
        .iter()
        .take(arguments.len().saturating_sub(1))
        .all(is_hopefully_short_arg)
}

/// Check if preceding args allow the "hug" pattern (inline with last arg).
///
/// Returns true when:
/// - All preceding args are short/simple
/// - No preceding arg is a multiline object in source
///
/// Note: Block functions are handled by explicit early checks in calls.rs,
/// and arrow functions already fail the "short" check.
#[inline]
pub(super) fn preceding_args_allow_hug(arguments: &[Expression], source: &str) -> bool {
    preceding_args_are_short(arguments) && !has_multiline_object_before_last(arguments, source)
}

/// Check if an expression is a function with a block body.
///
/// Matches arrow functions with block bodies (`() => { ... }`) and
/// function expressions (`function() { ... }`). These contain hardlines.
#[inline]
pub(super) fn is_block_function(expr: &Expression) -> bool {
    matches!(
        expr,
        Expression::ArrowFunctionExpression(arrow)
            if matches!(arrow.body, internal::ArrowFunctionBody::BlockStatement(_))
    ) || matches!(expr, Expression::FunctionExpression(_))
}

/// Check if any argument (except the last) is a function with a block body.
///
/// When true, the call should use full expansion instead of hugging.
pub(super) fn has_block_function_before_last(args: &[Expression]) -> bool {
    if args.len() < 2 {
        return false;
    }
    args[..args.len() - 1].iter().any(is_block_function)
}

/// Check if there are multiple arrow/function arguments
///
/// Returns true if 2+ arguments are arrow or function expressions.
/// Prettier always breaks these to multi-line format.
#[inline]
pub(super) fn has_multiple_function_args(arguments: &[Expression]) -> bool {
    arguments
        .iter()
        .filter(|arg| {
            matches!(
                arg,
                Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
            )
        })
        .nth(1)
        .is_some()
}

/// Check if an expression is a "simple" call argument (Prettier's `isSimpleCallArgument`)
///
/// Uses depth-limited recursion (typically depth=2) to prevent checking arbitrarily
/// deep structures. Returns false at depth 0.
///
/// Simple cases:
/// - Literals, identifiers, `this`, `super`, meta properties
/// - Template literals without newlines (with simple expressions)
/// - Objects with simple property values
/// - Arrays with simple elements
/// - Call/new expressions with simple callee and few simple args
/// - Member expressions with simple object and property
/// - Unary/update expressions with simple arguments
///
/// Reference: prettier/src/language-js/utils/index.js `isSimpleCallArgument`
pub fn is_simple_call_argument(expr: &Expression, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }

    // Unwrap TS type wrappers (as, satisfies, <T>, !) - same depth, just unwrapping
    if let Some(inner) = get_ts_type_wrapper_inner(expr) {
        return is_simple_call_argument(inner, depth);
    }

    match expr {
        // Simple literals are always simple (Prettier: isLiteral)
        Expression::Literal(_) => true,

        // Single-word types are simple (Prettier: isSingleWordType)
        // Includes: Identifier, ThisExpression, Super, MetaProperty
        Expression::Identifier(_) | Expression::Super(_) | Expression::MetaProperty(_) => true,

        // Template literals: simple if no newlines and expressions are simple
        Expression::TemplateLiteral(template) => {
            // Check both raw and cooked for newlines (Prettier checks both)
            let has_newline = template.quasis.iter().any(|q| {
                q.raw.contains('\n') || q.cooked.as_ref().is_some_and(|c| c.contains('\n'))
            });
            if has_newline {
                return false;
            }
            // Check all expressions are simple at reduced depth
            template
                .expressions
                .iter()
                .all(|e| is_simple_call_argument(e, depth - 1))
        }

        // Objects: simple if all properties are non-computed and values are simple
        Expression::ObjectExpression(obj) => obj.properties.iter().all(|prop| match prop {
            internal::ObjectProperty::Property(p) => {
                !p.computed && (p.shorthand || is_simple_call_argument(&p.value, depth - 1))
            }
            // Spread properties are not simple
            internal::ObjectProperty::SpreadElement(_) => false,
        }),

        // Arrays: simple if all elements are simple (None = hole, which is simple)
        Expression::ArrayExpression(arr) => arr.elements.iter().all(|elem| {
            elem.as_ref()
                .is_none_or(|e| is_simple_call_argument(e, depth - 1))
        }),

        // Member expressions: object must be simple, property is simple if not computed
        // (or if computed with a simple expression)
        Expression::MemberExpression(member) => {
            is_simple_call_argument(&member.object, depth)
                && (
                    // Non-computed properties (identifiers) are always simple
                    !member.computed
                    // Computed properties must have a simple expression
                    || is_simple_call_argument(&member.property, depth)
                )
        }

        // Call expressions: callee must be simple, args count <= depth, all args simple
        Expression::CallExpression(call) => {
            is_simple_call_argument(&call.callee, depth)
                && call.arguments.len() <= depth
                && call
                    .arguments
                    .iter()
                    .all(|arg| is_simple_call_argument(arg, depth - 1))
        }

        // New expressions: same logic as calls
        Expression::NewExpression(new_expr) => {
            is_simple_call_argument(&new_expr.callee, depth)
                && new_expr.arguments.len() <= depth
                && new_expr
                    .arguments
                    .iter()
                    .all(|arg| is_simple_call_argument(arg, depth - 1))
        }

        // Unary expressions with simple operands (Prettier checks specific operators)
        Expression::UnaryExpression(unary) => {
            matches!(
                unary.operator,
                internal::UnaryOperator::Minus
                    | internal::UnaryOperator::Plus
                    | internal::UnaryOperator::Bang
                    | internal::UnaryOperator::Tilde
                    | internal::UnaryOperator::Typeof
                    | internal::UnaryOperator::Void
            ) && is_simple_call_argument(&unary.argument, depth)
        }

        // Update expressions (++x, x++)
        Expression::UpdateExpression(update) => is_simple_call_argument(&update.argument, depth),

        // Spread elements: simple if the argument is simple
        Expression::SpreadElement(spread) => is_simple_call_argument(&spread.argument, depth),

        // Everything else is not simple (arrow functions, function expressions, etc.)
        _ => false,
    }
}

/// Check if an expression is an object with source newlines inside it.
///
/// Prettier preserves multiline object formatting and expands all call args
/// when any preceding arg is a multiline object in source.
pub(super) fn is_multiline_object_in_source(expr: &Expression, source: &str) -> bool {
    if let Expression::ObjectExpression(obj) = expr {
        if obj.properties.is_empty() {
            return false;
        }
        // Check if there's a newline after the opening brace
        let first_prop_start = obj.properties[0].span().start;
        has_newline_between(source, obj.span.start + 1, first_prop_start)
    } else {
        false
    }
}

/// Check if any argument (except the last) is a multiline object in source.
///
/// When true, the call should use hard expansion instead of the hug pattern.
pub(super) fn has_multiline_object_before_last(args: &[Expression], source: &str) -> bool {
    if args.len() < 2 {
        return false;
    }
    args[..args.len() - 1]
        .iter()
        .any(|arg| is_multiline_object_in_source(arg, source))
}
