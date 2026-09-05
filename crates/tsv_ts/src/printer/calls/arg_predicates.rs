//! Call-argument and arrow shape predicates shared by the call, new, and chain printers.

use crate::ast::internal::{self, Expression, TSType};
use crate::printer::needs_parens::strip_non_null_wrappers;
use crate::printer::types::helpers::{is_simple_type, unwrap_parenthesized};

/// Check if an argument is "hopefully short" enough to stay inline
///
/// Matches Prettier's `isHopefullyShortCallArgument` logic, which is STRICTER
/// than `isSimpleCallArgument`. Key differences:
/// - Call expressions with > 1 argument are NOT short (even if structurally simple)
/// - Binary expressions check both sides with depth=1
/// - An `as` / `satisfies` cast is short only under [`cast_is_hopefully_short`]; an
///   angle-bracket assertion (`<T>x`) is never short
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

        // Prettier: `isBinaryCastExpression(node) || node.type === "TypeCastExpression"` —
        // the `as` / `satisfies` pair (the Flow spellings have no tsv equivalent).
        Expression::TSAsExpression(cast) => {
            cast_is_hopefully_short(cast.type_annotation, cast.expression)
        }
        Expression::TSSatisfiesExpression(cast) => {
            cast_is_hopefully_short(cast.type_annotation, cast.expression)
        }

        // An angle-bracket assertion (`<T>x`) is NEVER short: prettier's
        // `isBinaryCastExpression` does not list `TSTypeAssertion`, so it falls past the cast
        // branch to `isSimpleCallArgument`, which has no case for it either. Stated here
        // rather than left to the fallthrough (which now answers the same) because the
        // asymmetry with the two arms above IS the rule, and it is asked about exactly here:
        // `<T>{}` and `{} as T` are one seed written two ways, and only one of them hugs.
        Expression::TSTypeAssertion(_) => false,

        // Prettier: if (isCallLikeExpression(node) && getCallArguments(node).length > 1) return false
        _ if CallLikeArguments::of(expr).is_some_and(|arguments| arguments.len() > 1) => false,

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

/// Prettier's **cast branch of `isHopefullyShortCallArgument`**: an `as` / `satisfies`
/// cast is short when its target type is simple and the expression it wraps is a simple
/// call argument at depth 1.
///
/// The type is read through two rewrites before [`is_simple_type`] sees it, both
/// prettier's:
///
/// 1. **Array element**, at most twice — `T[]` and `T[][]` are as short as `T`, `T[][][]`
///    is not (prettier stops after two, and so does this).
/// 2. **A lone type argument** of a type reference — `A<B>` is as short as `B`, while
///    `A<B, C>` keeps the reference itself, which is never simple once it carries type
///    arguments. That one-argument boundary is the whole reason `{} as Record<K, V>`
///    breaks all args where `{} as T` hugs.
///
/// Parens are unwrapped at each step because prettier's TS AST has no
/// `TSParenthesizedType` node — `(T)[]` reaches its check already as `T[]`.
fn cast_is_hopefully_short(type_annotation: &TSType<'_>, expression: &Expression<'_>) -> bool {
    let mut ty = unwrap_parenthesized(type_annotation);
    for _ in 0..2 {
        let TSType::Array(array) = ty else { break };
        ty = unwrap_parenthesized(array.element_type);
    }
    if let TSType::TypeReference(reference) = ty
        && let Some(type_arguments) = &reference.type_arguments
        && let [lone] = type_arguments.params
    {
        ty = lone;
    }

    is_simple_type(ty) && is_simple_call_argument(expression, 1)
}

/// The **call-like family** prettier's `isCallLikeExpression` names — a call, a `new`, or a
/// dynamic `import(…)` — read through its `getCallArguments`. An import's "arguments" are its
/// specifier plus the optional options object, which is why this is not simply a slice.
///
/// One spelling of a closed list that both [`is_hopefully_short_arg`] and
/// [`is_simple_call_argument`] ask about. A member present in one of them and missing from
/// the other is exactly the drift this exists to prevent — `import(…)` was missing from both.
#[derive(Clone, Copy)]
enum CallLikeArguments<'a> {
    /// A call's or `new`'s callee and argument list.
    Called {
        callee: &'a Expression<'a>,
        arguments: &'a [Expression<'a>],
    },
    /// A dynamic import's specifier and optional options object. It has **no callee**, and
    /// prettier's simplicity test skips the callee arm for it rather than failing on it.
    Imported {
        source: &'a Expression<'a>,
        options: Option<&'a Expression<'a>>,
    },
}

impl<'a> CallLikeArguments<'a> {
    /// The call-like reading of `expr`, or `None` when it is not call-like.
    fn of(expr: &'a Expression<'a>) -> Option<Self> {
        match expr {
            Expression::CallExpression(call) => Some(Self::Called {
                callee: call.callee,
                arguments: call.arguments,
            }),
            Expression::NewExpression(new_expr) => Some(Self::Called {
                callee: new_expr.callee,
                arguments: new_expr.arguments,
            }),
            Expression::ImportExpression(import_expr) => Some(Self::Imported {
                source: import_expr.source,
                options: import_expr.options,
            }),
            _ => None,
        }
    }

    /// Prettier's `getCallArguments(node).length`.
    fn len(self) -> usize {
        match self {
            Self::Called { arguments, .. } => arguments.len(),
            Self::Imported { options, .. } => 1 + usize::from(options.is_some()),
        }
    }

    /// The callee whose own simplicity prettier tests, or `None` where it has none to test.
    fn callee(self) -> Option<&'a Expression<'a>> {
        match self {
            Self::Called { callee, .. } => Some(callee),
            Self::Imported { .. } => None,
        }
    }

    /// Whether every argument satisfies `predicate`.
    fn all(self, mut predicate: impl FnMut(&'a Expression<'a>) -> bool) -> bool {
        match self {
            Self::Called { arguments, .. } => arguments.iter().all(predicate),
            Self::Imported { source, options } => {
                predicate(source) && options.is_none_or(predicate)
            }
        }
    }
}

/// Whether a second argument keeps the "expand first arg" hug — prettier's
/// `shouldExpandFirstArg` minus its first-argument half, i.e. its three named refusals plus
/// `isHopefullyShortCallArgument(secondArg) && !couldExpandArg(secondArg)`.
///
/// `leading_gap_start` is where this argument's leading gap opens (the previous argument's
/// printed end) — [`could_expand_collection_arg`] needs it to ask prettier's
/// `hasComment(node)` of a bare collection.
///
/// The `has_comments` closure answers whether a comment OCCUPIES THE PAGE in a range — the
/// on-page axis, since `hasComment` is a pure layout question that an owned annotation
/// satisfies. Callers pass `printer.has_comments_on_page_between`.
pub(super) fn is_short_second_arg_for_expand_first<F>(
    arg: &Expression<'_>,
    leading_gap_start: u32,
    has_comments: F,
) -> bool
where
    F: Fn(u32, u32) -> bool,
{
    match arg {
        // The three kinds prettier's `shouldExpandFirstArg` names outright. A spread needs
        // no arm of its own: `isSimpleCallArgument` has no case for one either, so it is
        // refused below exactly as prettier refuses it.
        Expression::ArrowFunctionExpression(_)
        | Expression::FunctionExpression(_)
        | Expression::ConditionalExpression(_) => false,
        // `!couldExpandArg(secondArg)`
        _ if could_expand_collection_arg(arg, leading_gap_start, &has_comments) => false,
        // `isHopefullyShortCallArgument(secondArg)`. A truly empty, uncommented `{}` / `[]`
        // reaches this arm and is simple by the vacuous `all` over its members — as it is in
        // prettier, which likewise gives an empty collection no arm of its own.
        _ => is_hopefully_short_arg(arg),
    }
}

/// Prettier's **`couldExpandArg` collection arms**: an object or array that would expand —
/// non-empty, or carrying a comment — asked through the `as` / `satisfies` / `<T>` casts
/// `couldExpandArg` recurses into (never through `!`, which it does not look past).
///
/// **Which comments count depends on the spelling, and that asymmetry is prettier's.**
/// `hasComment` is asked of whatever the recursion landed on, so for a BARE collection a
/// comment written before it attaches to the collection and blocks the hug, while for a
/// cast-wrapped one it attaches to the **cast** — which is why `/* c */ {} as T` still hugs
/// and `/* c */ {}` does not. Hence the range: `leading_gap_start` for a bare collection,
/// the collection's own start for a wrapped one.
///
/// A JSDoc cast is deliberately absent: prettier keeps its parens, so `couldExpandArg` sees
/// an opaque paren node rather than the collection inside and expands-first even for a
/// non-empty one. The transparency a cast does get is in [`is_hopefully_short_arg`] —
/// pinned by `calls/expand_first_jsdoc_cast_second_arg`.
fn could_expand_collection_arg<F>(
    arg: &Expression<'_>,
    leading_gap_start: u32,
    has_comments: &F,
) -> bool
where
    F: Fn(u32, u32) -> bool,
{
    let (collection, comments_from) = match arg {
        Expression::ObjectExpression(_) | Expression::ArrayExpression(_) => {
            (arg, leading_gap_start)
        }
        Expression::TSAsExpression(_)
        | Expression::TSSatisfiesExpression(_)
        | Expression::TSTypeAssertion(_) => {
            let inner = unwrap_ts_type_wrappers(arg);
            (inner, inner.span().start)
        }
        _ => return false,
    };

    match collection {
        Expression::ObjectExpression(obj) => {
            !obj.properties.is_empty() || has_comments(comments_from, obj.span.end)
        }
        Expression::ArrayExpression(arr) => {
            !arr.elements.is_empty() || has_comments(comments_from, arr.span.end)
        }
        _ => false,
    }
}

/// Whether an arrow's expression body is a call, looking through a trailing `!`.
///
/// Matches prettier's `isCallExpression(stripChainElementWrappers(body))` in
/// `couldExpandArg`: `(x) => fn()` and `(x) => fn()!` are both call bodies, so the
/// arrow hugs the call's open paren rather than breaking at it.
pub(super) fn arrow_body_is_call_through_non_null(body: &Expression<'_>) -> bool {
    matches!(strip_non_null_wrappers(body), Expression::CallExpression(_))
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
pub(super) fn is_ternary_arrow_body(body: &Expression<'_>) -> bool {
    matches!(body, Expression::ConditionalExpression(_))
}

/// Whether an expression is an object literal with properties — the narrow
/// `couldExpandArg` an `import(…)`'s **options** argument asks for its expand-last states.
/// Deliberately not [`could_expand_collection_arg`]: that one answers the expand-FIRST
/// question, which looks through casts and counts comments; this one is a plain shape test
/// on the last argument.
pub(super) fn is_expandable_object(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::ObjectExpression(obj) if !obj.properties.is_empty())
}

/// Check if the last argument is an array or object expression (unwrapping type assertions)
#[inline]
pub(super) fn last_arg_is_array_or_object(arguments: &[Expression<'_>]) -> bool {
    arguments.last().is_some_and(is_array_or_object_unwrapped)
}

/// Check if an expression is an array or object, unwrapping TS type wrappers
pub(super) fn is_array_or_object_unwrapped(expr: &Expression<'_>) -> bool {
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
fn unwrap_ts_type_wrappers<'a>(mut expr: &'a Expression<'a>) -> &'a Expression<'a> {
    loop {
        expr = match expr {
            Expression::TSAsExpression(cast) => cast.expression,
            Expression::TSSatisfiesExpression(cast) => cast.expression,
            Expression::TSTypeAssertion(cast) => cast.expression,
            _ => return expr,
        };
    }
}

/// Check if an expression is a function with a block body.
///
/// Matches arrow functions with block bodies (`() => { ... }`) and
/// function expressions (`function() { ... }`). These contain hardlines.
#[inline]
pub(super) fn is_block_function(expr: &Expression<'_>) -> bool {
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
pub(super) fn is_react_hook_call_with_deps_array<F>(
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
pub(super) fn is_hook_callback_with_deps(callback: &Expression<'_>, deps: &Expression<'_>) -> bool {
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
/// - Literals, identifiers, `this`, `super`
/// - Template literals without newlines (with simple expressions)
/// - Objects with simple property values
/// - Arrays with simple elements
/// - Call-like nodes ([`CallLikeArguments`]) with a simple callee and few simple arguments
/// - Member expressions with simple object and property
/// - Update expressions, and unary ones over `!` / `-` / `+` / `~`
///
/// Everything else is NOT simple — notably a TS cast, a meta property, and `typeof` / `void` /
/// `delete`, each of which prettier leaves to its own closing `return false`.
///
/// Reference: prettier/src/language-js/utils/index.js `isSimpleCallArgument`
pub(crate) fn is_simple_call_argument(expr: &Expression<'_>, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }

    // Prettier opens with `node = stripChainElementWrappers(node)` — the chain-element
    // wrappers ONLY, at the same depth. A cast (`as` / `satisfies` / `<T>`) is deliberately
    // not looked through: prettier has no case for one, so it falls to the closing
    // `_ => false`. That refusal is load-bearing at both callers — a cast argument is what
    // force-expands a 3+ call member chain
    // (`call_has_complex_args`) and what refuses the expand-first hug through
    // `is_hopefully_short_arg`.
    let expr = strip_non_null_wrappers(expr);

    // Call-like nodes: the callee must be simple at THIS depth, the arguments must fit within
    // the remaining depth, and each must be simple one level shallower. Prettier skips the
    // callee arm for a dynamic `import(…)`
    // (`node.type === "ImportExpression" || isSimpleCallArgument(node.callee, depth)`) rather
    // than failing on it — it has no callee. Answered ahead of the match because a match guard
    // cannot bind; the family is disjoint from every arm below.
    if let Some(arguments) = CallLikeArguments::of(expr) {
        return arguments
            .callee()
            .is_none_or(|callee| is_simple_call_argument(callee, depth))
            && arguments.len() <= depth
            && arguments.all(|argument| is_simple_call_argument(argument, depth - 1));
    }

    match expr {
        // Simple literals are always simple (Prettier: isLiteral)
        Expression::Literal(_) => true,

        // Regex: simple only if pattern is short (Prettier: getStringWidth(pattern) <= 5).
        // Uses the precomputed pattern width so this stays source-free.
        Expression::RegexLiteral(regex) => usize::from(regex.pattern_width) <= 5,

        // Single-word types are simple (Prettier: `isSingleWordType`). A meta property
        // (`import.meta`, `new.target`) is deliberately NOT one — prettier lists neither it
        // nor any `Meta*` node in `isSingleWordType` or `isLiteral`, so it falls to the
        // closing `_ => false`. `PrivateIdentifier` is on prettier's list but unreachable
        // here: a bare `#x` is only ever the left side of `#x in o`, and `a.#b` is answered
        // by the member arm's non-computed short-circuit before it asks.
        Expression::Identifier(_) | Expression::ThisExpression(_) | Expression::Super(_) => true,

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
                !p.computed && (p.shorthand || is_simple_call_argument(p.value, depth - 1))
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

        // Unary expressions with simple operands. Prettier's
        // `simpleCallArgumentUnaryOperators` is exactly these four — `typeof`, `void` and
        // `delete` are all absent, so a `typeof x` argument is NOT simple.
        Expression::UnaryExpression(unary) => {
            matches!(
                unary.operator,
                internal::UnaryOperator::Minus
                    | internal::UnaryOperator::Plus
                    | internal::UnaryOperator::Bang
                    | internal::UnaryOperator::Tilde
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
pub(super) fn is_function_composition_args(arguments: &[Expression<'_>]) -> bool {
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
    fn simple_call_argument_looks_through_only_the_chain_wrappers() {
        let arena = Bump::new();
        let simple = |src: &str| is_simple_call_argument(&parse_expr(&arena, src), 2);
        // `!` is prettier's `stripChainElementWrappers`, so `x!` is as simple as `x`.
        assert!(simple("x!"));
        // A TS cast is not looked through — prettier has no case for one.
        assert!(!simple("x as T"));
        assert!(!simple("x satisfies T"));
        assert!(!simple("<T>x"));
        // Only four unary operators are simple; `typeof` / `void` / `delete` are not.
        assert!(simple("-x"));
        assert!(simple("!x"));
        assert!(!simple("typeof x"));
        assert!(!simple("void x"));
        assert!(!simple("delete x.y"));
        // A dynamic import is call-like: its specifier plus optional options object are its
        // arguments, and it has no callee to test.
        assert!(simple("import('a')"));
        assert!(simple("import(a, b)"));
        // …and the argument count is still held to the depth.
        assert!(!is_simple_call_argument(
            &parse_expr(&arena, "import(a, b)"),
            1
        ));
    }

    #[test]
    fn hopefully_short_reads_a_cast_through_its_type() {
        let arena = Bump::new();
        let short = |src: &str| is_hopefully_short_arg(&parse_expr(&arena, src));
        // A bare type reference is simple, so the cast-wrapped seed stays short.
        assert!(short("{} as T"));
        assert!(short("{} satisfies T"));
        // A lone type argument is descended into; two of them are not.
        assert!(short("{} as A<B>"));
        assert!(!short("{} as A<B, C>"));
        // An array element type is unwrapped, at most twice.
        assert!(short("{} as T[]"));
        assert!(short("{} as T[][]"));
        assert!(!short("{} as T[][][]"));
        // The wrapped expression must itself be simple at depth 1.
        assert!(!short("fn(a) as T"));
        assert!(!short("({} as A) as B"));
        // An angle-bracket assertion is never short, whatever its type says.
        assert!(!short("<T>{}"));
        // A call-like argument with more than one argument is never short.
        assert!(short("fn(a)"));
        assert!(!short("fn(a, b)"));
        assert!(short("import('a')"));
        assert!(!short("import('a', b)"));
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
