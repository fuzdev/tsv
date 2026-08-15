//! Call-argument and arrow shape predicates shared by the call, new, and chain printers.

use crate::ast::internal::{self, Expression};

/// Check if an argument is "hopefully short" enough to stay inline
///
/// Matches Prettier's `isHopefullyShortCallArgument` logic, which is STRICTER
/// than `isSimpleCallArgument`. Key differences:
/// - Call expressions with > 1 argument are NOT short (even if structurally simple)
/// - Binary expressions check both sides with depth=1
/// - A JSDoc cast is transparent: the question is re-asked of the wrapped expression,
///   so the rules above still fire through it (`is_simple_call_argument` has no such
///   case, which is what keeps the transparency one level deep)
///
/// Used to determine if tail args can stay inline after a function callback.
fn is_hopefully_short_arg(expr: &Expression<'_>) -> bool {
    match expr {
        // Prettier: if (node.type === "ParenthesizedExpression") recurse into node.expression.
        // A JSDoc cast is prettier's retained `ParenthesizedExpression` — its annotation
        // attaches to the INNER expression, so the wrapper is transparent here and the
        // shortness question is asked of what it wraps. The recursion is one level deep on
        // purpose: `is_simple_call_argument` has no paren case, so a cast nested inside
        // another argument stays opaque, matching prettier.
        Expression::JsdocCast(cast) => is_hopefully_short_arg(cast.inner),

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
/// The `has_comments_to_emit_between` closure checks for comments inside empty containers
/// (typically `printer.has_comments_to_emit_between`).
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
        // TS cast wrappers (`as` / `satisfies` / `<T>`, never `!`): mirror prettier's
        // couldExpandArg, which looks through the cast to a non-empty (or commented)
        // object/array and expands all args rather than expand-first.
        Expression::TSAsExpression(_)
        | Expression::TSSatisfiesExpression(_)
        | Expression::TSTypeAssertion(_)
            if cast_wraps_expandable_object_or_array(arg, &has_comments) =>
        {
            false
        }
        // Other args: check if "hopefully short"
        _ => is_hopefully_short_arg(arg),
    }
}

/// Whether a TS cast-wrapped arg (`as` / `satisfies` / `<T>`, never non-null) wraps an
/// object or array prettier's `couldExpandArg` would expand — a non-empty, or
/// comment-bearing, object/array. Such a second arg forces expand-all rather than
/// expand-first, mirroring `!couldExpandArg`. An empty, uncommented wrapped collection
/// (`{} as T`) stays "short" and still expands-first, matching prettier.
fn cast_wraps_expandable_object_or_array<F>(arg: &Expression<'_>, has_comments: &F) -> bool
where
    F: Fn(u32, u32) -> bool,
{
    match unwrap_ts_type_wrappers(arg) {
        Expression::ObjectExpression(obj) => {
            !obj.properties.is_empty() || has_comments(obj.span.start, obj.span.end)
        }
        Expression::ArrayExpression(arr) => {
            !arr.elements.is_empty() || has_comments(arr.span.start, arr.span.end)
        }
        _ => false,
    }
}

/// Whether an arrow's expression body is a call, looking through a trailing `!`.
///
/// Matches prettier's `isCallExpression(stripChainElementWrappers(body))` in
/// `couldExpandArg`: `(x) => fn()` and `(x) => fn()!` are both call bodies, so the
/// arrow hugs the call's open paren rather than breaking at it.
pub(in crate::printer) fn arrow_body_is_call_through_non_null(body: &Expression<'_>) -> bool {
    matches!(
        crate::printer::needs_parens::strip_non_null_wrappers(body),
        Expression::CallExpression(_)
    )
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

/// Unwrap the TypeScript cast wrappers (`as`, `satisfies`, `<T>`) to get the inner expression.
/// Returns the innermost non-cast expression.
///
/// Mirrors Prettier's `couldExpandArg`, which looks through `isBinaryCastExpression`
/// (`as`/`satisfies`) and `TSTypeAssertion` (`<T>x`) but NOT `TSNonNullExpression`: a
/// `{...}!` / `[...]!` argument is not treated as an expandable object/array, so it does
/// not hug the call parens.
fn unwrap_ts_type_wrappers<'a>(expr: &'a Expression<'a>) -> &'a Expression<'a> {
    match expr {
        Expression::TSAsExpression(e) => unwrap_ts_type_wrappers(e.expression),
        Expression::TSSatisfiesExpression(e) => unwrap_ts_type_wrappers(e.expression),
        Expression::TSTypeAssertion(e) => unwrap_ts_type_wrappers(e.expression),
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

/// Prettier's **`isReactHookCallWithDepsArray`** (`print/call-arguments.js`): a
/// zero-parameter block-body arrow immediately followed by an array literal —
/// `useEffect(() => {…}, [a, b])`, or the three-argument
/// `useImperativeHandle(ref, () => {…}, [a, b])` whose first argument is a plain
/// identifier.
///
/// The shape is the whole rule: prettier reads **no callee name**, so any call written this
/// way takes the layout, `new` and member-chain calls included (one `printCallArguments`
/// serves them all). It is also the FIRST thing that printer asks, above `anyArgEmptyLine`
/// and every specialized layout — which is why an author blank line between the callback and
/// the deps array is collapsed rather than preserved here.
///
/// The `!args.some(hasComment)` conjunct asks about comments **attached to an argument**,
/// not comments anywhere inside one: a comment in the callback's body or between the deps
/// array's own brackets leaves the layout alone, while one leading or trailing an argument
/// refuses it. The caller supplies that answer through `arg_gap_has_comment`, which it asks
/// of the gaps around the arguments — `(`→first, each inter-argument gap, last→`)` — on the
/// ON-PAGE axis, since an owned annotation glued to an argument is a comment prettier's
/// `hasComment` sees.
pub(in crate::printer) fn is_react_hook_call_with_deps_array<F>(
    args: &[Expression<'_>],
    arg_gap_has_comment: F,
) -> bool
where
    F: Fn() -> bool,
{
    let base = match args.len() {
        2 => 0,
        3 if matches!(&args[0], Expression::Identifier(_)) => 1,
        _ => return false,
    };

    is_hook_callback_with_deps(&args[base], &args[base + 1]) && !arg_gap_has_comment()
}

/// The shape half of [`is_react_hook_call_with_deps_array`] — prettier's
/// `isValidHookCallbackAndDepsFormat` minus its comment conjunct: a zero-parameter
/// block-body arrow followed by an array literal.
///
/// Split out for `import(…)`, whose AST carries `source` + `options` rather than an argument
/// slice; prettier reaches it through the same `printCallArguments` (`ImportExpression` is in
/// that printer's header list), so the shape question must have one answer for both.
pub(in crate::printer) fn is_hook_callback_with_deps(
    callback: &Expression<'_>,
    deps: &Expression<'_>,
) -> bool {
    matches!(
        callback,
        Expression::ArrowFunctionExpression(arrow)
            if arrow.params.is_empty()
                && matches!(arrow.body, internal::ArrowFunctionBody::BlockStatement(_))
    ) && matches!(deps, Expression::ArrayExpression(_))
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

        // Everything else is not simple: arrow functions, function expressions, and
        // spread elements (matches prettier — no SpreadElement case), etc.
        _ => false,
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

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;

    /// Parse a bare expression to its internal AST node (spans index into `src`),
    /// allocated in the caller-supplied `arena`.
    fn parse_expr<'a>(arena: &'a Bump, src: &str) -> Expression<'a> {
        crate::parse_expression_with_comments(src, 0, arena)
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

    #[test]
    fn react_hook_deps_array_shape() {
        let arena = Bump::new();
        // The gap-comment conjunct is the caller's; these cases isolate the shape.
        let shape = |src: &str| is_react_hook_call_with_deps_array(args_of(&arena, src), || false);

        // Two-argument form: zero-parameter block arrow, then an array literal.
        assert!(shape("useEffect(() => {}, [a, b])"));
        // The callee name is never read — any call written in the shape takes the layout.
        assert!(shape("fn(() => {}, [a])"));
        // An empty deps array still counts; so does an `async` callback.
        assert!(shape("fn(() => {}, [])"));
        assert!(shape("fn(async () => {}, [a])"));

        // A parameter on the callback disqualifies it…
        assert!(!shape("fn((x) => {}, [a])"));
        // …as does an expression body, or a non-array second argument.
        assert!(!shape("fn(() => a, [b])"));
        assert!(!shape("fn(() => {}, {})"));
        assert!(!shape("fn(() => {}, a)"));
        // A spread is not the callback.
        assert!(!shape("fn(...[() => {}], [a])"));

        // Three-argument form: a plain IDENTIFIER first, then the same pair.
        assert!(shape("useImperativeHandle(ref, () => {}, [a])"));
        // Any other first argument refuses it — prettier tests `type === "Identifier"`.
        assert!(!shape("useImperativeHandle(o.p, () => {}, [a])"));
        assert!(!shape("useImperativeHandle(1, () => {}, [a])"));

        // One or four arguments are not the shape at all.
        assert!(!shape("fn(() => {})"));
        assert!(!shape("fn(() => {}, [a], b)"));

        // A comment attached to an argument refuses it, whatever the shape says.
        assert!(!is_react_hook_call_with_deps_array(
            args_of(&arena, "fn(() => {}, [a])"),
            || true
        ));
    }
}
