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

    /// Tagged template tag: `` <expr>`template` ``
    ///
    /// Same precedence rules as `Callee`, plus an optional chain always needs
    /// parens here — an optional chain can't be a template tag per spec
    /// (`` a?.b`x` `` is a syntax error), so the parens seal it.
    TaggedTemplateTag,

    /// Base of member/call chain: `<expr>.method()`
    ChainBase,

    /// Inside TSNonNullExpression: `<expr>!`
    NonNull,

    /// Left side of `as` or `satisfies`: `<expr> as T`
    /// Only angle-bracket `<T>x` needs parens here (as/satisfies are left-associative)
    TypeAssertion,

    /// Expression in angle-bracket assertion: `<T><expr>`
    /// All type assertions need parens here
    AngleBracketAssertion,

    /// Expression in TSInstantiationExpression: `<expr><T>`
    InstantiationExpression,

    /// Argument of unary operator: `!<expr>`, `typeof <expr>`
    UnaryArgument,

    /// Argument of await: `await <expr>`
    AwaitArgument,

    /// Argument of yield: `yield <expr>`
    /// Only AssignmentExpression needs parens (yield has lower precedence than binary/conditional)
    YieldArgument,

    /// Arrow function body (expression form): `() => <expr>`
    ArrowBody,

    /// Object property value: `{key: <expr>}`
    ObjectPropertyValue,

    /// Default value of a parameter/pattern (`(a = <expr>) =>`) or a class
    /// property value (`a = <expr>;`)
    DefaultValue,

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

    /// Statement test condition: `if (<expr>)`, `while (<expr>)`, `for (;<expr>;)`, `do {} while (<expr>)`
    /// Assignment expressions need double-parens for clarity: `while ((x = y))`
    StatementTest,

    /// Superclass of a class heritage clause: `class C extends <expr> {}`
    /// Prettier wraps everything that isn't a bare identifier/member/call/literal
    /// (incl. `new`, tagged templates, and non-null, which are valid `extends`
    /// operands but still parenthesized for clarity).
    SuperClass,

    /// Left side of an assignment: `<expr> = …` / `<expr> += …`.
    /// A type-assertion target (`as` / `satisfies` / `<T>`) must be parenthesized —
    /// `(x as T) = 1` (bare `x as T = 1` is a parse error). Non-null `x!` is valid
    /// bare, so it isn't wrapped (matches prettier).
    AssignmentTarget,
}

/// Whether `expr` is an `in` binary expression — the operator that must be
/// parenthesized inside a `for` header init so it isn't read as the `for (x in
/// y)` separator. Shared by `needs_parens` (the ambient for-init rule) and the
/// surgical `in`-wrap at positions that build an expression without a
/// `needs_parens` check.
pub(crate) fn is_in_binary(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::BinaryExpression(b) if b.operator == BinaryOperator::In)
}

/// Determines if an expression needs parentheses in a given context.
///
/// This is the central entry point for all parenthesization decisions.
///
/// `in_for_init` is the ambient "building a `for` header init clause" flag: when
/// set, an `in` binary expression always needs parens (prettier parenthesizes
/// every `in` lexically under the init, regardless of context). It's threaded as
/// a parameter rather than read from a context because parenthesization is a pure
/// function of the node and its surroundings.
pub fn needs_parens(expr: &Expression<'_>, ctx: ParenContext, in_for_init: bool) -> bool {
    // Ambient for-init rule: an `in` binary always needs parens here. ORed ahead
    // of the context match so it applies uniformly (call args, object values,
    // binary operands, etc.) and never double-wraps a node a context already
    // parenthesizes for precedence (`!(a in b)`, `(a in b).p`).
    if in_for_init && is_in_binary(expr) {
        return true;
    }
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
        // TaggedTemplateTag (`(x as T)`template``) shares these precedence rules; both it
        // and NewCallee add the optional-chain rule below.
        // Note: SequenceExpression already adds its own parens in build_sequence_doc
        // Note: ClassExpression needs parens only in NewCallee: `class {}()` is valid but `new class {}()` is not
        ParenContext::Callee | ParenContext::NewCallee | ParenContext::TaggedTemplateTag => {
            if matches!(ctx, ParenContext::NewCallee) {
                // ClassExpression only needs parens in `new` context
                if matches!(expr, Expression::ClassExpression(_)) {
                    return true;
                }
                // A `new` callee containing a call needs parens so the arguments
                // bind to the `new`, not to the inner call: `new (f())()`,
                // `new (a.b())()`, `new (f().C)()`, `new (a?.b())()`. Without them
                // `new f()()` parses as `(new f())()` — different semantics.
                if new_callee_has_call(expr) {
                    return true;
                }
            }
            // A `new` callee or template tag may NOT be an (unsealed) optional chain
            // per spec — `new a?.b()` / `` a?.b`x` `` are syntax errors. So the parens
            // are *always* required (unlike the boundary-dependent member/call/non-null
            // cases, which depend on what follows the chain). The plain call `Callee`
            // context is excluded: `(a?.b)()` strips to the valid `a?.b()`. A non-null
            // assertion that seals the chain (`(a?.b)!`) is handled by the sealed-base
            // rendering, not here (`has_optional_in_chain` returns false for it).
            if matches!(
                ctx,
                ParenContext::NewCallee | ParenContext::TaggedTemplateTag
            ) && expr.has_optional_in_chain()
            {
                return true;
            }
            is_await_or_yield(expr)
                || is_type_assertion(expr)
                || is_function_like(expr)
                || is_unary_or_update(expr)
                || matches!(
                    expr,
                    Expression::ConditionalExpression(_)
                        | Expression::BinaryExpression(_)
                        | Expression::AssignmentExpression(_)
                )
        }

        // Chain base: `(a + b).method()`, `(await x).method()`, `(yield x).method()`, etc.
        // Numeric literals need parens for `.method()` calls: `0.toString()` is invalid syntax.
        // Prettier normalizes `0..toString()` to `(0).toString()`.
        //
        // Update/unary expressions and arrow functions as a member-access object
        // also need parens: `(++c).p`, `(-a).p`, `(!a).p`, `(typeof a).p`,
        // `(() => 1).p`. Without them the prefix operator binds to the member
        // access (`-a.p` is `-(a.p)`) or the arrow body absorbs it (`() => 1.p` is
        // an arrow returning `1.p`). Function/class/object expressions do NOT need
        // them — their brace-delimited bodies make the parens redundant, and
        // prettier strips them (`(function () {}).p` → `function () {}.p`).
        ParenContext::ChainBase => {
            is_lower_precedence(expr)
                || is_numeric_literal(expr)
                || is_unary_or_update(expr)
                || matches!(expr, Expression::ArrowFunctionExpression(_))
        }

        // Spread argument: `...(a || b)`, `...(a ? b : c)`, `...(await x)`, `...(x as T)`
        ParenContext::SpreadArgument => is_lower_precedence(expr),

        // Non-null: `(a + b)!`, `(!x)!`, `(a ? b : c)!`, `(yield x)!`, `(++x)!`, etc.
        // UpdateExpression needs parens too: `(++x)!` is `NonNull(++x)`, but `++x!`
        // parses as `++(x!)` (`Update(NonNull)`) — a different AST.
        ParenContext::NonNull => is_lower_precedence(expr) || is_unary_or_update(expr),

        // Type assertion (as/satisfies): `(a + b) as T`, `(await x) as T`, `(<U>x) as T`
        // Arrow functions need parens because `(...args) => x as T` parses as `(...args) => (x as T)`
        // Ternary/assignment need parens: `(a ? b : c) as T` vs `a ? b : c as T` (different semantics)
        // Only angle-bracket assertions need parens here (as/satisfies are left-associative)
        ParenContext::TypeAssertion => {
            is_await_or_yield(expr)
                || matches!(
                    expr,
                    Expression::BinaryExpression(_)
                        | Expression::ConditionalExpression(_)
                        | Expression::AssignmentExpression(_)
                        | Expression::ArrowFunctionExpression(_)
                        | Expression::TSTypeAssertion(_)
                )
        }

        // Angle-bracket assertion: `<T>(a + b)`, `<T>(<U>x)`, `<T>(x as U)`, `<T>(a ? b : c)`
        // Unary argument: `!(a + b)`, `!(await x)`, `!(<T>x)`, `typeof (a ? b : c)`
        // Both need parens for: await/yield, all type assertions, binary, conditional, assignment, arrow
        ParenContext::AngleBracketAssertion | ParenContext::UnaryArgument => {
            is_await_or_yield(expr)
                || is_type_assertion(expr)
                || matches!(
                    expr,
                    Expression::BinaryExpression(_)
                        | Expression::ConditionalExpression(_)
                        | Expression::AssignmentExpression(_)
                        | Expression::ArrowFunctionExpression(_)
                )
        }

        // Instantiation: `(<T>() => {})<U>`, `(x as A)<T>`, `(<T>x)<U>`, `(await x)<T>`, `(a = b)<T>`
        // Ternary/binary/assignment need parens to preserve semantics:
        // `(a ? b : c)<T>` vs `a ? b : c<T>` (different - ternary result vs alternate instantiated)
        ParenContext::InstantiationExpression => {
            is_await_or_yield(expr)
                || is_type_assertion(expr)
                || is_function_like(expr)
                || matches!(
                    expr,
                    Expression::ConditionalExpression(_)
                        | Expression::AssignmentExpression(_)
                        | Expression::BinaryExpression(_)
                )
        }

        // Await argument: `await (a + b)`, `await (x as T)`, `await (<T>x)`, `await (a ? b : c)`
        // Parens needed for precedence/semantics - await has higher precedence than ?:
        // Assignment: `await (x ??= y)` — without parens, parses as `(await x) ??= y` (syntax error)
        ParenContext::AwaitArgument => {
            is_type_assertion(expr)
                || matches!(
                    expr,
                    Expression::BinaryExpression(_)
                        | Expression::ConditionalExpression(_)
                        | Expression::AssignmentExpression(_)
                )
        }

        // Yield argument: `yield (x ??= y)` — assignment needs parens for clarity
        // Unlike await, yield has lower precedence than binary/conditional, so those don't need parens
        ParenContext::YieldArgument => matches!(expr, Expression::AssignmentExpression(_)),

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

        // Assignment as a default/class-property value keeps its parens:
        // `(a = (b = c)) =>`, `a = (this.a = b);`
        ParenContext::DefaultValue => matches!(expr, Expression::AssignmentExpression(_)),

        // These contexts all need parens around assignment expressions for clarity:
        // - Call/array/new argument: `fn((a = b))`, `[(a = b)]`, `new Fn((a = b))`
        // - Template literal expression: `${(a = b)}`
        // - Computed property key: `{[(a = b)]: c}`
        ParenContext::Argument
        | ParenContext::TemplateLiteralExpression
        | ParenContext::ComputedPropertyKey => {
            matches!(expr, Expression::AssignmentExpression(_))
        }

        // Statement test: `while ((x = y))`, `if ((x = getValue()))`, `for (;(x = y);)`
        // Double-parens signal intentional assignment (not a typo for ==)
        ParenContext::StatementTest => matches!(expr, Expression::AssignmentExpression(_)),

        // Superclass: `extends (a + b)`, `extends (a ? b : c)`, `extends (await x)`,
        // `extends ((a) => b)`, `extends (x as T)`, `extends (-x)`. The
        // lower-precedence and unary/update forms cover the operator cases; beyond
        // those prettier also parenthesizes `new`, tagged templates, and a bare object
        // (which would otherwise be read as the class body) — all valid `extends`
        // operands it still wraps for clarity. Bare identifiers, member/call chains,
        // literals, untagged templates, and `class`/`function` expressions stay
        // unparenthesized. `SequenceExpression` is absent because `build_sequence_doc`
        // already adds its own parens.
        //
        // Prettier first strips the chain-element wrappers (non-null `!` — and, in
        // ESTree, the `ChainExpression` optional-chain wrapper, which tsv folds into
        // member/call nodes) and tests the *inner* expression (#18652): a lone
        // `extends (Base!)` drops to `extends Base!`, while `extends (new Base()!)`
        // keeps the parens because the stripped `new` still wraps.
        ParenContext::SuperClass => {
            let stripped = strip_non_null_wrappers(expr);
            is_lower_precedence(stripped)
                || is_unary_or_update(stripped)
                || matches!(
                    stripped,
                    Expression::ArrowFunctionExpression(_)
                        | Expression::NewExpression(_)
                        | Expression::TaggedTemplateExpression(_)
                        | Expression::ObjectExpression(_)
                )
        }

        // A type-assertion target needs parens to round-trip (`(x as T) = …`);
        // non-null `x!` is a valid bare assignment target, so it isn't wrapped.
        ParenContext::AssignmentTarget => matches!(
            expr,
            Expression::TSAsExpression(_)
                | Expression::TSSatisfiesExpression(_)
                | Expression::TSTypeAssertion(_)
        ),
    }
}

//
// Simple predicates (expression type groupings)
//

/// Strip trailing non-null assertion (`!`) wrappers, returning the inner expression.
///
/// tsv's mirror of prettier's `stripChainElementWrappers`: tsv has no distinct
/// `ChainExpression` node (optional chains fold into member/call), so only the
/// non-null wrapper needs unwrapping. Shared by the `extends`-clause paren decision
/// (#18652: `extends (Base!)` → `extends Base!`, the `!` binds tightly so the
/// heritage paren is redundant) and the call-arg arrow-body check
/// (`arrow_body_is_call_through_non_null`, `couldExpandArg`'s
/// `isCallExpression(stripChainElementWrappers(body))` — `=> fn()!` is a call body
/// that hugs the open paren).
pub(in crate::printer) fn strip_non_null_wrappers<'a>(
    mut expr: &'a Expression<'a>,
) -> &'a Expression<'a> {
    while let Expression::TSNonNullExpression(non_null) = expr {
        expr = non_null.expression;
    }
    expr
}

/// `await x` or `yield x` - always need parens together in most contexts
fn is_await_or_yield(expr: &Expression<'_>) -> bool {
    matches!(
        expr,
        Expression::AwaitExpression(_) | Expression::YieldExpression(_)
    )
}

/// Lower precedence expressions that need parens in chain/spread/non-null contexts
/// Combines: await/yield + type assertions + binary/conditional/assignment
fn is_lower_precedence(expr: &Expression<'_>) -> bool {
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
fn is_type_assertion(expr: &Expression<'_>) -> bool {
    matches!(
        expr,
        Expression::TSAsExpression(_)
            | Expression::TSSatisfiesExpression(_)
            | Expression::TSTypeAssertion(_)
    )
}

/// Arrow function or function expression
fn is_function_like(expr: &Expression<'_>) -> bool {
    matches!(
        expr,
        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
    )
}

/// Prefix/postfix unary or update expression (`-x`, `!x`, `typeof x`, `void x`,
/// `delete x`, `++x`, `x--`). These bind looser than member access, call, and
/// the postfix `!` non-null operator, so they need parens as a member-access
/// object (`(-x).p`), a chain callee (`(-x)()`), or a non-null operand (`(++x)!`)
/// — without them the operator captures the wrong operand (`-x.p` is `-(x.p)`;
/// `++x!` is `++(x!)`). `UpdateExpression` is easy to omit when adding such a
/// context (it was missed for `ChainBase` and `NonNull`); routing every
/// postfix/access-precedence arm through this predicate keeps them in lockstep.
fn is_unary_or_update(expr: &Expression<'_>) -> bool {
    matches!(
        expr,
        Expression::UnaryExpression(_) | Expression::UpdateExpression(_)
    )
}

/// Whether a `new` callee contains a call expression in its leftmost
/// member/non-null chain. Prettier parenthesizes such a callee so the `new`
/// arguments bind to the `new` rather than the inner call: `new (f())()`,
/// `new (a.b())()`, `new (f().C)()`, `new (a?.b())()`. Mirrors prettier's
/// `NewExpression` callee rule (needs-parens.js). Member access walks the
/// object (the call must be to the left of `new`'s argument list to be
/// captured), so `new a[b]()` — no inner call — stays unparenthesized.
fn new_callee_has_call(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::CallExpression(_) => true,
        Expression::MemberExpression(member) => new_callee_has_call(member.object),
        Expression::TSNonNullExpression(non_null) => new_callee_has_call(non_null.expression),
        _ => false,
    }
}

/// Numeric literal - needs parens in chain base context because `0.toString()` is invalid.
/// Prettier normalizes `0..toString()` to `(0).toString()`.
fn is_numeric_literal(expr: &Expression<'_>) -> bool {
    matches!(
        expr,
        Expression::Literal(lit) if matches!(lit.value, LiteralValue::Number(_))
    )
}

//
// Complex helpers (non-trivial logic)
//

/// Expression statement: `<expr>;`
/// Object/function/class expressions and object pattern assignments need parens
/// when they start the statement, to avoid being reparsed as a block, function
/// declaration, or class declaration. `({...});`, `(function () {});`,
/// `(class {});` — matches prettier's "statement starts with `{`/`function`/`class`"
/// rule (parentheses/needs-parentheses.js).
fn needs_parens_expression_statement(expr: &Expression<'_>) -> bool {
    match expr {
        // Object expression: `({...});` needs parens to avoid being parsed as a block
        Expression::ObjectExpression(_) => true,
        // Function/class expression: `(function () {});` / `(class {});` need parens
        // to avoid being reparsed as a declaration (which also changes meaning —
        // an anonymous declaration is a syntax error).
        Expression::FunctionExpression(_) | Expression::ClassExpression(_) => true,
        // Object pattern assignment: `({a, b} = obj);` needs parens
        Expression::AssignmentExpression(assign) => {
            matches!(assign.left, Expression::ObjectPattern(_))
        }
        // Sequence: check the first expression
        Expression::SequenceExpression(seq) => seq
            .expressions
            .first()
            .is_some_and(needs_parens_expression_statement),
        _ => false,
    }
}

/// Walk to the leftmost (first-printed) leaf of an expression, mirroring
/// prettier's `startsWithNoLookaheadToken` (utilities/starts-with-no-lookahead-token.js).
///
/// Used to decide whether an expression statement must be wrapped in parens
/// because its leftmost token is an object/function/class — e.g. `(class {}).foo`
/// wraps the class, not the whole member expression. Recurses through the
/// positions that print first (`.left`, `.object`, `.callee`, `.test`, …) and
/// stops at IIFE callees/tags (already parenthesized) to match prettier.
pub(crate) fn leftmost_no_lookahead<'a>(expr: &'a Expression<'a>) -> &'a Expression<'a> {
    match expr {
        // Binary and logical share `BinaryExpression` here — recurse into `.left`.
        Expression::BinaryExpression(b) => leftmost_no_lookahead(b.left),
        Expression::AssignmentExpression(a) => leftmost_no_lookahead(a.left),
        Expression::MemberExpression(m) => leftmost_no_lookahead(m.object),
        Expression::ConditionalExpression(c) => leftmost_no_lookahead(c.test),
        Expression::SequenceExpression(s) => {
            s.expressions.first().map_or(expr, leftmost_no_lookahead)
        }
        // IIFEs (`(function () {})()` / `` (function () {})`x` ``) are already
        // parenthesized by their callee/tag, so prettier stops the walk there.
        Expression::CallExpression(call) => {
            if matches!(call.callee, Expression::FunctionExpression(_)) {
                expr
            } else {
                leftmost_no_lookahead(call.callee)
            }
        }
        Expression::TaggedTemplateExpression(t) => {
            if matches!(t.tag, Expression::FunctionExpression(_)) {
                expr
            } else {
                leftmost_no_lookahead(t.tag)
            }
        }
        // Postfix update (`x++`) prints its argument first; prefix (`++x`) does not.
        Expression::UpdateExpression(u) if !u.prefix => leftmost_no_lookahead(u.argument),
        Expression::TSAsExpression(e) => leftmost_no_lookahead(e.expression),
        Expression::TSSatisfiesExpression(e) => leftmost_no_lookahead(e.expression),
        Expression::TSNonNullExpression(e) => leftmost_no_lookahead(e.expression),
        Expression::TSInstantiationExpression(e) => leftmost_no_lookahead(e.expression),
        _ => expr,
    }
}

/// Binary operand: `<expr> op y` or `x op <expr>`
fn needs_parens_binary_operand(
    expr: &Expression<'_>,
    parent_op: BinaryOperator,
    is_right: bool,
) -> bool {
    // These expressions need parens when used as operands of binary expressions.
    // Some have lower precedence, others are for clarity (await/yield).
    // e.g., `a && (b ? c : d)` - without parens it becomes `(a && b) ? c : d`
    // e.g., `(x as string) in obj` - without parens it becomes `x as (string in obj)`
    // e.g., `b || ((fn) => fn)` - without parens it becomes `(b || fn) => fn` (syntax error)
    // e.g., `a && (await b)` - parens for clarity (Prettier style)
    if matches!(
        expr,
        Expression::ConditionalExpression(_)
            | Expression::AssignmentExpression(_)
            | Expression::TSAsExpression(_)
            | Expression::TSSatisfiesExpression(_)
            | Expression::ArrowFunctionExpression(_)
            | Expression::AwaitExpression(_)
            | Expression::YieldExpression(_)
    ) {
        return true;
    }

    // Unary expressions as left operand of ** require parens (ES2016+ syntax rule)
    // `-2 ** 3` is a syntax error; must be `(-2) ** 3` or `-(2 ** 3)`
    if !is_right
        && parent_op == BinaryOperator::StarStar
        && matches!(expr, Expression::UnaryExpression(_))
    {
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
