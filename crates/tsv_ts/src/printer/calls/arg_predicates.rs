//! Call-argument and arrow shape predicates shared by the call, new, and chain printers.

use crate::ast::internal::{self, Expression};
use tsv_lang::printing::has_newline_between_fast;

/// Check if an argument is "hopefully short" enough to stay inline
///
/// Matches Prettier's `isHopefullyShortCallArgument` logic, which is STRICTER
/// than `isSimpleCallArgument`. Key differences:
/// - Call expressions with > 1 argument are NOT short (even if structurally simple)
/// - Binary expressions check both sides with depth=1
///
/// Used to determine if tail args can stay inline after a function callback.
fn is_hopefully_short_arg(expr: &Expression<'_>) -> bool {
    match expr {
        // Prettier: if (isCallLikeExpression(node) && getCallArguments(node).length > 1) return false
        Expression::CallExpression(call) if call.arguments.len() > 1 => false,
        Expression::NewExpression(new_expr) if new_expr.arguments.len() > 1 => false,

        // Prettier: if (isBinaryish(node)) check both sides with depth=1
        // Note: Our AST uses BinaryExpression for logical ops (&&, ||, ??) too
        Expression::BinaryExpression(bin) => {
            is_simple_call_argument(bin.left, 1) && is_simple_call_argument(bin.right, 1)
        }

        // Prettier: return isRegExpLiteral(node) || isSimpleCallArgument(node)
        // All regex is "hopefully short" regardless of pattern length — the pattern
        // length check in is_simple_call_argument only matters for chain 3+ calls.
        Expression::RegexLiteral(_) => true,
        _ => is_simple_call_argument(expr, 2),
    }
}

/// Check if an expression is an object that could expand (has properties)
/// Used for "expand last arg" pattern in import expressions
pub(in crate::printer) fn is_expandable_object(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::ObjectExpression(obj) if !obj.properties.is_empty())
}

/// Check if an array is "concisely printed" — all elements are numeric literals.
///
/// Prettier formats these arrays with fill layout, which prevents the
/// expand-last-arg pattern from working (the expanded doc has different
/// break characteristics). When true, the array should NOT use expand-last-arg
/// and instead falls through to the normal inline-or-expand-all path.
pub(in crate::printer) fn is_concise_numeric_array(expr: &Expression<'_>) -> bool {
    if let Expression::ArrayExpression(arr) = expr {
        !arr.elements.is_empty()
            && arr
                .elements
                .iter()
                .all(|elem| elem.as_ref().is_some_and(is_numeric_expression))
    } else {
        false
    }
}

/// Check if an expression is a numeric literal (including unary +/- prefix).
fn is_numeric_expression(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::Literal(lit) => matches!(lit.value, internal::LiteralValue::Number(_)),
        Expression::UnaryExpression(unary) => {
            matches!(
                unary.operator,
                internal::UnaryOperator::Minus | internal::UnaryOperator::Plus
            ) && is_numeric_expression(unary.argument)
        }
        _ => false,
    }
}

/// Check if a second argument is "short" enough for the "expand first arg" pattern.
///
/// Used when the first arg is a block function and we want to keep the second arg
/// inline after the closing `}`. Returns false for expressions that would expand.
///
/// The `has_comments_between` closure checks for comments inside empty containers
/// (typically `printer.has_comments_between`).
pub(in crate::printer) fn is_short_second_arg_for_expand_first<F>(
    arg: &Expression<'_>,
    has_comments: F,
) -> bool
where
    F: Fn(u32, u32) -> bool,
{
    match arg {
        // Functions, ternaries, spreads - these should expand all args
        Expression::ArrowFunctionExpression(_)
        | Expression::FunctionExpression(_)
        | Expression::ConditionalExpression(_)
        | Expression::SpreadElement(_) => false,
        // Non-empty objects expand - use "expand all args" instead
        Expression::ObjectExpression(obj) if !obj.properties.is_empty() => false,
        // Non-empty arrays expand - use "expand all args" instead
        Expression::ArrayExpression(arr) if !arr.elements.is_empty() => false,
        // Empty {} or [] with comments inside should expand
        Expression::ObjectExpression(obj) if has_comments(obj.span.start, obj.span.end) => false,
        Expression::ArrayExpression(arr) if has_comments(arr.span.start, arr.span.end) => false,
        // Truly empty {} and [] are short
        Expression::ObjectExpression(_) | Expression::ArrayExpression(_) => true,
        // Other args: check if "hopefully short"
        _ => is_hopefully_short_arg(arg),
    }
}

/// Check if an arrow function body is a ternary expression
///
/// Check if an arrow body is a ternary that needs conditional paren treatment.
///
/// Matches Prettier's `couldExpandArg` logic for conditional expressions:
/// - Flat: `(x) => (x ? y : z)` - parens prevent ambiguity with `<=`
/// - Break: `(x) =>\n  x ? y : z,` - no parens needed, clearly arrow body
///
/// Call expressions, objects, and arrays are handled by other code paths.
pub(in crate::printer) fn is_ternary_arrow_body(body: &Expression<'_>) -> bool {
    matches!(body, Expression::ConditionalExpression(_))
}

/// Check if an arrow function has trailing comments after its last parameter.
///
/// Returns true if there are comments between the last param and the `=>` token, e.g.:
/// ```text
/// (a: string, // comment
/// ) => {}
/// ```
///
/// Does NOT include comments between `=>` and the body — those are body comments,
/// not trailing param comments.
///
/// `arrow_token_pos` is the byte offset of `=>` in the source. Callers should obtain
/// this via `printer.find_arrow_token_for(arrow)`.
pub(crate) fn arrow_has_trailing_param_comments<F>(
    arrow: &internal::ArrowFunctionExpression<'_>,
    arrow_token_pos: u32,
    has_comments_between: F,
) -> bool
where
    F: Fn(u32, u32) -> bool,
{
    let Some(last_param) = arrow.params.last() else {
        return false;
    };
    let param_end = last_param.span().end;

    has_comments_between(param_end, arrow_token_pos)
}

/// Check if the last argument is an array or object expression (unwrapping type assertions)
#[inline]
pub(in crate::printer) fn last_arg_is_array_or_object(arguments: &[Expression<'_>]) -> bool {
    arguments.last().is_some_and(is_array_or_object_unwrapped)
}

/// Check if an expression is an array or object, unwrapping TS type wrappers
pub(in crate::printer) fn is_array_or_object_unwrapped(expr: &Expression<'_>) -> bool {
    matches!(
        unwrap_ts_type_wrappers(expr),
        Expression::ArrayExpression(_) | Expression::ObjectExpression(_)
    )
}

/// Unwrap TypeScript type wrappers (as, satisfies, <T>, !) to get the inner expression.
/// Returns the innermost non-wrapper expression.
fn unwrap_ts_type_wrappers<'a>(expr: &'a Expression<'a>) -> &'a Expression<'a> {
    match expr {
        Expression::TSAsExpression(e) => unwrap_ts_type_wrappers(e.expression),
        Expression::TSSatisfiesExpression(e) => unwrap_ts_type_wrappers(e.expression),
        Expression::TSTypeAssertion(e) => unwrap_ts_type_wrappers(e.expression),
        Expression::TSNonNullExpression(e) => unwrap_ts_type_wrappers(e.expression),
        _ => expr,
    }
}

/// Get the inner expression if this is a TS type wrapper, otherwise None.
fn get_ts_type_wrapper_inner<'a>(expr: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    match expr {
        Expression::TSAsExpression(e) => Some(e.expression),
        Expression::TSSatisfiesExpression(e) => Some(e.expression),
        Expression::TSTypeAssertion(e) => Some(e.expression),
        Expression::TSNonNullExpression(e) => Some(e.expression),
        _ => None,
    }
}

/// Check if preceding args allow the "expand last arg" conditional group pattern.
///
/// Only checks for multiline objects — the conditional group's fits() mechanism
/// handles width naturally. If preceding args don't fit on one line, the inline
/// state fails and we fall through to expand-all.
///
/// Matches Prettier's `shouldExpandLastArg` which doesn't check preceding arg complexity.
#[inline]
pub(in crate::printer) fn preceding_args_allow_expand_last(
    arguments: &[Expression<'_>],
    line_breaks: &[u32],
) -> bool {
    !has_multiline_object_before_last(arguments, line_breaks)
}

/// Check if an expression is a function with a block body.
///
/// Matches arrow functions with block bodies (`() => { ... }`) and
/// function expressions (`function() { ... }`). These contain hardlines.
#[inline]
pub(in crate::printer) fn is_block_function(expr: &Expression<'_>) -> bool {
    matches!(
        expr,
        Expression::ArrowFunctionExpression(arrow)
            if matches!(arrow.body, internal::ArrowFunctionBody::BlockStatement(_))
    ) || matches!(expr, Expression::FunctionExpression(_))
}

/// Check if an expression is a curried arrow (arrow whose body is another arrow).
///
/// Used to set `skip_arrow_chain` in call arg contexts, matching prettier's
/// `!args.expandLastArg` in `shouldPrintAsChain` — curried arrows in call args
/// should hug their body rather than chain-breaking.
#[inline]
pub(in crate::printer) fn is_curried_arrow(expr: &Expression<'_>) -> bool {
    matches!(
        expr,
        Expression::ArrowFunctionExpression(a)
            if matches!(a.body, internal::ArrowFunctionBody::Expression(e)
                if matches!(e, Expression::ArrowFunctionExpression(_)))
    )
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
pub fn is_simple_call_argument(expr: &Expression<'_>, depth: usize) -> bool {
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

        // Regex: simple only if pattern is short (Prettier: getStringWidth(pattern) <= 5).
        // Uses the precomputed pattern width so this stays source-free.
        Expression::RegexLiteral(regex) => usize::from(regex.pattern_width) <= 5,

        // Single-word types are simple (Prettier: isSingleWordType)
        // Includes: Identifier, ThisExpression, Super, MetaProperty
        Expression::Identifier(_)
        | Expression::ThisExpression(_)
        | Expression::Super(_)
        | Expression::MetaProperty(_) => true,

        // Template literals: simple if no newlines and expressions are simple
        Expression::TemplateLiteral(template) => {
            // Check both raw and cooked for newlines (Prettier checks both).
            // `has_newline` covers the raw side (and the no-escape `Verbatim`
            // cooked, which equals raw); only a `Decoded` cooked can introduce a
            // newline raw lacks (a `\n` escape) — and it owns its string, so this
            // stays source-free.
            let has_newline = template.quasis.iter().any(|q| {
                q.has_newline
                    || matches!(&q.cooked, internal::TemplateCooked::Decoded(c) if c.contains('\n'))
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
            is_simple_call_argument(member.object, depth)
                && (
                    // Non-computed properties (identifiers) are always simple
                    !member.computed
                    // Computed properties must have a simple expression
                    || is_simple_call_argument(member.property, depth)
                )
        }

        // Call expressions: callee must be simple, args count <= depth, all args simple
        Expression::CallExpression(call) => {
            is_simple_call_argument(call.callee, depth)
                && call.arguments.len() <= depth
                && call
                    .arguments
                    .iter()
                    .all(|arg| is_simple_call_argument(arg, depth - 1))
        }

        // New expressions: same logic as calls
        Expression::NewExpression(new_expr) => {
            is_simple_call_argument(new_expr.callee, depth)
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
            ) && is_simple_call_argument(unary.argument, depth)
        }

        // Update expressions (++x, x++)
        Expression::UpdateExpression(update) => is_simple_call_argument(update.argument, depth),

        // Spread elements are NOT simple (matches prettier — no SpreadElement case)
        Expression::SpreadElement(_) => false,

        // Everything else is not simple (arrow functions, function expressions, etc.)
        _ => false,
    }
}

/// Check if an expression contains a call expression with arguments (recursively).
///
/// Used to determine if a chain's first call has an argument that may need to
/// break independently. When the first call's arg contains a call WITH arguments,
/// that inner call might break, so we let each group format independently rather
/// than forcing expansion on the last call.
///
/// Empty calls like `a.b()` don't count because they won't break.
pub fn contains_call_expression(expr: &Expression<'_>) -> bool {
    match expr {
        // A call with arguments might break - return true
        // Empty calls (no args) won't break - continue checking inside
        Expression::CallExpression(call) => {
            !call.arguments.is_empty() || contains_call_expression(call.callee)
        }
        Expression::NewExpression(new_expr) => {
            !new_expr.arguments.is_empty() || contains_call_expression(new_expr.callee)
        }

        // Recurse into common wrapper types
        Expression::MemberExpression(member) => {
            contains_call_expression(member.object)
                || (member.computed && contains_call_expression(member.property))
        }
        Expression::TSAsExpression(e) => contains_call_expression(e.expression),
        Expression::TSSatisfiesExpression(e) => contains_call_expression(e.expression),
        Expression::TSTypeAssertion(e) => contains_call_expression(e.expression),
        Expression::TSNonNullExpression(e) => contains_call_expression(e.expression),
        Expression::TSInstantiationExpression(e) => contains_call_expression(e.expression),
        Expression::AwaitExpression(e) => contains_call_expression(e.argument),
        Expression::UnaryExpression(e) => contains_call_expression(e.argument),
        Expression::UpdateExpression(e) => contains_call_expression(e.argument),
        Expression::SpreadElement(e) => contains_call_expression(e.argument),
        Expression::JsdocCast(cast) => contains_call_expression(cast.inner),

        // Binary expressions (includes logical operators in internal AST)
        Expression::BinaryExpression(e) => {
            contains_call_expression(e.left) || contains_call_expression(e.right)
        }
        Expression::AssignmentExpression(e) => {
            contains_call_expression(e.left) || contains_call_expression(e.right)
        }

        // Conditional expression
        Expression::ConditionalExpression(e) => {
            contains_call_expression(e.test)
                || contains_call_expression(e.consequent)
                || contains_call_expression(e.alternate)
        }

        // Sequence expression
        Expression::SequenceExpression(e) => e.expressions.iter().any(contains_call_expression),

        // Template literal expressions
        Expression::TemplateLiteral(t) => t.expressions.iter().any(contains_call_expression),
        Expression::TaggedTemplateExpression(t) => {
            contains_call_expression(t.tag)
                || t.quasi.expressions.iter().any(contains_call_expression)
        }

        // Array/object literals
        Expression::ArrayExpression(arr) => arr
            .elements
            .iter()
            .any(|el| el.as_ref().is_some_and(contains_call_expression)),
        Expression::ObjectExpression(obj) => obj.properties.iter().any(|prop| match prop {
            internal::ObjectProperty::Property(p) => {
                (p.computed && contains_call_expression(&p.key))
                    || contains_call_expression(&p.value)
            }
            internal::ObjectProperty::SpreadElement(s) => contains_call_expression(s.argument),
        }),

        // Arrow/function expressions - check body for expression arrows
        Expression::ArrowFunctionExpression(arr) => {
            if let internal::ArrowFunctionBody::Expression(body) = &arr.body {
                contains_call_expression(body)
            } else {
                false
            }
        }

        // Simple expressions that don't contain calls
        Expression::Identifier(_)
        | Expression::Literal(_)
        | Expression::RegexLiteral(_)
        | Expression::ThisExpression(_)
        | Expression::Super(_)
        | Expression::MetaProperty(_)
        | Expression::FunctionExpression(_)
        | Expression::ClassExpression(_)
        | Expression::YieldExpression(_)
        | Expression::ImportExpression(_)
        | Expression::ArrayPattern(_)
        | Expression::ObjectPattern(_)
        | Expression::AssignmentPattern(_)
        | Expression::RestElement(_)
        | Expression::PrivateIdentifier(_)
        | Expression::TSParameterProperty(_) => false,
    }
}

/// Check if arguments form a "function composition" pattern that forces expansion.
///
/// Matches Prettier's `isFunctionCompositionArgs` logic:
/// - 2+ arguments
/// - Either: 2+ function/arrow arguments, OR
///   any argument is a call expression containing a function/arrow argument
///
/// This triggers `allArgsBrokenOut()` in Prettier to expand all arguments.
pub(in crate::printer) fn is_function_composition_args(arguments: &[Expression<'_>]) -> bool {
    if arguments.len() <= 1 {
        return false;
    }

    let mut function_count = 0;

    for arg in arguments {
        if matches!(
            arg,
            Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
        ) {
            function_count += 1;
            if function_count > 1 {
                return true;
            }
        } else if let Expression::CallExpression(call) = arg {
            // Check if this call has any function/arrow arguments
            if call.arguments.iter().any(|child_arg| {
                matches!(
                    child_arg,
                    Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
                )
            }) {
                return true;
            }
        }
    }

    false
}

/// Check if an expression is an object with newlines inside it.
///
/// Prettier preserves multiline object formatting and expands all call args
/// when any preceding arg is a multiline object in source.
pub(in crate::printer) fn is_multiline_object(expr: &Expression<'_>, line_breaks: &[u32]) -> bool {
    if let Expression::ObjectExpression(obj) = expr {
        if obj.properties.is_empty() {
            return false;
        }
        // Check if there's a newline after the opening brace
        let first_prop_start = obj.properties[0].span().start;
        has_newline_between_fast(line_breaks, obj.span.start + 1, first_prop_start)
    } else {
        false
    }
}

/// Check if any argument (except the last) is a multiline object.
///
/// When true, the call should use hard expansion instead of the hug pattern.
pub(in crate::printer) fn has_multiline_object_before_last(
    args: &[Expression<'_>],
    line_breaks: &[u32],
) -> bool {
    if args.len() < 2 {
        return false;
    }
    args[..args.len() - 1]
        .iter()
        .any(|arg| is_multiline_object(arg, line_breaks))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;
    use std::cell::RefCell;
    use std::rc::Rc;
    use string_interner::DefaultStringInterner;

    /// Parse a bare expression to its internal AST node (spans index into `src`),
    /// allocated in the caller-supplied `arena`.
    fn parse_expr<'a>(arena: &'a Bump, src: &str) -> Expression<'a> {
        let interner = Rc::new(RefCell::new(DefaultStringInterner::new()));
        crate::parse_expression_with_comments(src, 0, interner, arena)
            .expect("expression should parse")
            .0
    }

    /// Parse a call expression and return its argument list.
    fn args_of<'a>(arena: &'a Bump, src: &str) -> &'a [Expression<'a>] {
        match parse_expr(arena, src) {
            Expression::CallExpression(call) => call.arguments,
            other => panic!("expected a call expression, got: {other:?}"),
        }
    }

    #[test]
    fn concise_numeric_array_detection() {
        let arena = Bump::new();
        assert!(is_concise_numeric_array(&parse_expr(&arena, "[1, 2, 3]")));
        // Unary +/- prefixes still count as numeric.
        assert!(is_concise_numeric_array(&parse_expr(&arena, "[-1, +2]")));
        // Empty array is not concise-numeric.
        assert!(!is_concise_numeric_array(&parse_expr(&arena, "[]")));
        // A non-numeric element disqualifies it.
        assert!(!is_concise_numeric_array(&parse_expr(&arena, "[1, 'x']")));
        // A hole is not a numeric element (unlike is_simple_call_argument).
        assert!(!is_concise_numeric_array(&parse_expr(&arena, "[1, , 2]")));
        // Non-array expressions are never concise-numeric.
        assert!(!is_concise_numeric_array(&parse_expr(&arena, "foo")));
    }

    #[test]
    fn simple_call_argument_depth_and_shape() {
        let arena = Bump::new();
        // Depth 0 is always "not simple".
        assert!(!is_simple_call_argument(&parse_expr(&arena, "x"), 0));
        // Literals / identifiers are simple at any positive depth.
        assert!(is_simple_call_argument(&parse_expr(&arena, "42"), 1));
        assert!(is_simple_call_argument(&parse_expr(&arena, "foo"), 1));
        // Regex is simple only if the pattern width is <= 5.
        assert!(is_simple_call_argument(&parse_expr(&arena, "/abcde/"), 2));
        assert!(!is_simple_call_argument(&parse_expr(&arena, "/abcdef/"), 2));
        // A call's args must fit within the remaining depth: `f(a)` needs depth >= 2.
        assert!(!is_simple_call_argument(&parse_expr(&arena, "f(a)"), 1));
        assert!(is_simple_call_argument(&parse_expr(&arena, "f(a)"), 2));
        // Spread elements are never simple.
        assert!(!is_simple_call_argument(&parse_expr(&arena, "[...x]"), 2));
    }

    #[test]
    fn contains_call_expression_recursion() {
        let arena = Bump::new();
        // An empty call does not count, but we recurse into the callee.
        assert!(!contains_call_expression(&parse_expr(&arena, "a.b()")));
        // A call WITH arguments counts.
        assert!(contains_call_expression(&parse_expr(&arena, "a.b(x)")));
        // Recurse through a binary expression.
        assert!(contains_call_expression(&parse_expr(&arena, "a + f(x)")));
        // A computed member recurses into the property.
        assert!(contains_call_expression(&parse_expr(&arena, "a[f(x)]")));
        // No call anywhere.
        assert!(!contains_call_expression(&parse_expr(&arena, "a + b")));
    }

    #[test]
    fn function_composition_args_detection() {
        let arena = Bump::new();
        // Two arrow args ⇒ composition.
        assert!(is_function_composition_args(args_of(
            &arena,
            "compose(a => a, b => b)"
        )));
        // A single arg is never composition.
        assert!(!is_function_composition_args(args_of(&arena, "f(a => a)")));
        // A call argument that itself wraps a callback ⇒ composition.
        assert!(is_function_composition_args(args_of(
            &arena,
            "compose(x, g(() => {}))"
        )));
        // Two non-function args ⇒ not composition.
        assert!(!is_function_composition_args(args_of(&arena, "f(a, b)")));
    }
}
