// Unified Assignment Layout Engine
//
// Routes both variable declarations (`id = value`) and object property values (`key: value`)
// through the same layout selection logic, matching prettier's `printAssignment` function.
//
// ## Architecture
//
// 1. `build_assignment_layout()` - Main entry point, builds doc for assignments
// 2. `choose_layout()` - Selects layout strategy based on expression type
// 3. `is_poorly_breakable_chain()` - Detects chains that don't break well internally
//
// ## Reference
//
// - prettier/src/language-js/print/assignment.js

use crate::ast::internal::{self, Expression, JsdocCast};
use crate::printer::ArrowChainContext;
use crate::printer::Printer;
use crate::printer::conditional_should_break_after_op;
use crate::printer::expressions::literals::format_string_literal_from_ast;
use crate::printer::is_string_literal;
use crate::printer::layout::{fluid_after_operator, hang_after_operator};
use tsv_lang::Comment;
use tsv_lang::PRINT_WIDTH;
use tsv_lang::doc::GroupId;
use tsv_lang::doc::arena::DocId;

/// Prettier's heuristic for "short" property keys.
///
/// Keys shorter than `tabWidth + MIN_OVERLAP_FOR_BREAK` don't benefit from
/// breaking after the colon. This is an aesthetic choice, not principled -
/// Prettier tuned it empirically until output "looked right".
///
/// Reference: prettier/src/language-js/print/assignment.js
pub const MIN_OVERLAP_FOR_BREAK: usize = 3;

/// Assignment layout strategies (matches prettier's chooseLayout return values)
///
/// Note: Assignment chains (a = b = c) are handled separately in expressions/patterns.rs
/// via context passing, not through this unified layout system. Chain formatting requires
/// parent context tracking which doesn't fit the "key: value" model used here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentLayout {
    /// Break after operator, then RHS is indented
    /// Structure: group([left, op, indent([line, right])])
    BreakAfterOperator,

    /// Never break after operator - keep on same line
    /// Structure: group([left, op, " ", right])
    NeverBreakAfterOperator,

    /// Fluid layout - breaks after operator only if needed
    /// Structure: group([left, op, group(indent(line)), indentIfBreak(right)])
    Fluid,
}

/// Whether the author gave a JSDoc cast's comment a line of its own — a newline on
/// **both** sides of it, as in `const a =⏎\t/** @type {A} */⏎\t(expr)`.
///
/// Both sides is the rule prettier applies, and only that shape hangs. A newline on
/// one side alone collapses to a space:
///
/// ```js
/// const a = /** @type {A} */⏎  (expr);  // →  const a = /** @type {A} */ (expr);
/// const a =⏎  /** @type {A} */ (expr);  // →  const a = /** @type {A} */ (expr);
/// ```
///
/// The single source of truth for both consequences of that shape: the hang itself
/// (`choose_layout` below, and the declarator's own predicates in
/// `statements/variable.rs`) and the hardline the cast prints between the comment and
/// its `(` (`build_jsdoc_cast_doc`). They must agree — a hang without the hardline
/// leaves the `(` stranded, and a hardline without the hang un-indents it.
pub fn jsdoc_cast_comment_is_own_line(cast: &JsdocCast<'_>, source: &str) -> bool {
    let bytes = source.as_bytes();
    // Only whitespace between the start of the line and the comment.
    let mut i = cast.comment.span.start as usize;
    let newline_before = loop {
        if i == 0 {
            break true;
        }
        i -= 1;
        match bytes[i] {
            b'\n' => break true,
            b' ' | b'\t' | b'\r' => {}
            _ => break false,
        }
    };
    newline_before
        && !tsv_lang::printing::is_same_line(source, cast.comment.span.end, cast.span.start)
}

/// Choose the layout strategy for an assignment
///
/// Follows prettier's `chooseLayout` logic in assignment.js
///
/// `is_short_key`: True for property keys shorter than `tabWidth + MIN_OVERLAP_FOR_BREAK`.
/// Short keys don't benefit from breaking after the colon. For non-property assignments
/// (e.g., `x = value`), pass `false`.
///
/// `can_break_left`: Whether the printed left-hand side contains a break point
/// (prettier's `canBreakLeftDoc`). It gates the `never-break-after-operator` cases: an
/// unbreakable RHS may only stay welded to the operator when the LHS has nowhere to
/// break either — otherwise the assignment falls through to `fluid` and breaks after the
/// operator, rather than letting the LHS break inside the assignment target.
pub fn choose_layout(
    right_expr: &Expression<'_>,
    is_short_key: bool,
    can_break_left: bool,
    source: &str,
    print_width: usize,
    comments: &[Comment],
) -> AssignmentLayout {
    // Untyped curried arrow chains (`(a) => (b) => …`) use fluid layout: break
    // after `=` only when the signature heads don't fit on the operator line,
    // letting a hugging body (object/array/block) expand in place otherwise.
    // Typed chains (any arrow has a return type with params, type parameters, or
    // a non-identifier param) instead force the break via break-after-operator
    // (handled by `is_curried_arrow_with_return_type` below), so the heads always
    // drop onto their own lines.
    if is_curried_arrow_chain(right_expr) && !is_curried_arrow_with_return_type(right_expr) {
        return AssignmentLayout::Fluid;
    }

    // Objects, arrays, functions, classes, and calls handle their own expansion
    // The value expands internally: `key: { ... }` not `key:\n{ ... }`
    //
    // Call expressions use conditional_group to try multiple states during fits().
    // With the fits logic, calls can "fit" even when they break internally,
    // allowing the call to handle breaking before the assignment does.
    if is_self_expanding_value(right_expr) {
        return AssignmentLayout::NeverBreakAfterOperator;
    }

    // Binary expressions → break after operator, UNLESS it's a logical expression
    // with a self-expanding RHS (non-empty object/array). In that case, the RHS
    // handles its own expansion: `x = foo || { a: 1 }` not `x =\n  foo || {a: 1}`
    //
    // Prettier ref: shouldBreakAfterOperator (assignment.js:199)
    //   `isBinaryish(rightNode) && !shouldInlineLogicalExpression(rightNode)`
    if let Expression::BinaryExpression(binary) = right_expr
        && !should_inline_logical_expression(binary)
    {
        return AssignmentLayout::BreakAfterOperator;
    }

    // Sequence expressions → break after operator
    if matches!(right_expr, Expression::SequenceExpression(_)) {
        return AssignmentLayout::BreakAfterOperator;
    }

    // Decorated class expression → break after operator (`const C =\n\t@dec\n\tclass {}`).
    if let Expression::ClassExpression(c) = right_expr
        && class_expr_has_decorators(c)
    {
        return AssignmentLayout::BreakAfterOperator;
    }

    // Conditional expressions with binary test → break after operator
    // Prettier ref: shouldBreakAfterOperator (assignment.js:216-219)
    if conditional_should_break_after_op(right_expr) {
        return AssignmentLayout::BreakAfterOperator;
    }

    // Curried arrow functions with return type → break after operator
    // Produces: `key:\n  (x: T): H =>\n  (y) =>\n    expr`
    if is_curried_arrow_with_return_type(right_expr) {
        return AssignmentLayout::BreakAfterOperator;
    }

    // Short property keys → never break after operator
    // (wrapping object properties with very short keys usually doesn't add much value)
    if is_short_key && !can_break_left {
        return AssignmentLayout::NeverBreakAfterOperator;
    }

    // Check if RHS is a poorly breakable chain (should break after operator)
    if should_break_after_operator(right_expr, source, print_width, comments) {
        return AssignmentLayout::BreakAfterOperator;
    }

    // Simple values that shouldn't break → never break after operator.
    //
    // Only when the LHS can't break either (prettier's `!canBreakLeftDoc`, assignment.js
    // chooseLayout:181-191). When it CAN — `params['key'] = \`template\`;`, whose computed
    // lookup is a breakable group — welding the unbreakable RHS to the operator would
    // force the overflow into the assignment *target*, splitting `params[⏎ 'key'⏎] =`.
    // Prettier instead falls through to `fluid` and breaks after the `=`.
    if is_simple_value(right_expr) && !can_break_left {
        return AssignmentLayout::NeverBreakAfterOperator;
    }

    // Default → fluid layout
    AssignmentLayout::Fluid
}

/// Whether a class expression carries decorators (`@dec class {}`).
///
/// A decorated class expression breaks after the assignment operator (each
/// decorator on its own line) rather than self-expanding; an undecorated one
/// expands its body in place. Prettier ref: shouldBreakAfterOperator
/// (assignment.js:228) `case "ClassExpression": isNonEmptyArray(decorators)`;
/// the never-break ClassExpression case (assignment.js:189) only applies once
/// that has ruled out a decorated class.
pub fn class_expr_has_decorators(c: &internal::ClassExpression<'_>) -> bool {
    c.decorators.is_some_and(|d| !d.is_empty())
}

/// Check if an expression handles its own expansion (objects, arrays, functions, classes)
///
/// These values should never have a break between key: and value because they
/// expand internally. `key: { ... }` NOT `key:\n{ ... }`
///
/// Note: Call expressions and NewExpressions are NOT included here.
/// They use Fluid layout (from choose_layout default) which allows breaking
/// after the operator when the total line exceeds printWidth.
///
/// Note: Curried arrow functions with return type annotations are NOT self-expanding.
/// They need BreakAfterOperator to produce:
///   const f =
///       (x: T): H =>
///       (y) => ...
pub fn is_self_expanding_value(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::ObjectExpression(_)
        | Expression::ArrayExpression(_)
        | Expression::FunctionExpression(_) => true,

        // An undecorated class expression expands its body in place (`= class {…}`);
        // a *decorated* one breaks after the operator instead (`choose_layout`).
        Expression::ClassExpression(c) => !class_expr_has_decorators(c),

        // Arrow functions are self-expanding UNLESS they're curried with return type
        Expression::ArrowFunctionExpression(_) => !is_curried_arrow_with_return_type(expr),

        _ => false,
    }
}

/// Check if a binary expression is a logical expression with a self-expanding RHS.
///
/// Returns true when a LogicalExpression (`&&`, `||`, `??`) has a non-empty object,
/// non-empty array, or JSX element on the right side. These cases should NOT use
/// BreakAfterOperator — the RHS handles its own expansion.
///
/// Prettier ref: `shouldInlineLogicalExpression` (binaryish.js:361)
pub fn should_inline_logical_expression(binary: &internal::BinaryExpression<'_>) -> bool {
    if !binary.operator.is_logical() {
        return false;
    }

    match binary.right {
        Expression::ObjectExpression(obj) => !obj.properties.is_empty(),
        Expression::ArrayExpression(arr) => !arr.elements.is_empty(),
        // Note: Prettier also checks isJsxElement, but JSX is not supported in tsv
        _ => false,
    }
}

/// Check if an expression is a curried arrow function where ANY arrow in the chain
/// has a return type annotation (with params). Returns false for non-curried arrows.
///
/// Prettier breaks the entire chain if ANY arrow has:
/// - return type annotation AND parameters
/// - type parameters (generics)
/// - non-identifier params (destructuring, defaults)
///
/// Examples that break:
///   const f = (x: T): H => (y) => expr    // outer has return type
///   const f = (x: T) => (y): H => expr    // inner has return type
///   const f = (x: T): A => (y): B => expr // both have return types
///
/// Examples that stay inline:
///   const f = (x: T) => (y) => expr       // neither has return type
pub fn is_curried_arrow_with_return_type(expr: &Expression<'_>) -> bool {
    // A curried chain (body is another arrow) where ANY arrow carries a
    // return type / type params / non-identifier param.
    is_curried_arrow_chain(expr)
        && matches!(expr, Expression::ArrowFunctionExpression(arrow) if arrow_chain_has_return_type(arrow))
}

/// Check if an expression is a curried arrow function (its body is another
/// arrow). The terminal body may be an expression or a block. Used to route the
/// assignment RHS through the arrow-chain layout regardless of whether the chain
/// carries a return type.
pub fn is_curried_arrow_chain(expr: &Expression<'_>) -> bool {
    if let Expression::ArrowFunctionExpression(arrow) = expr {
        matches!(
            &arrow.body,
            internal::ArrowFunctionBody::Expression(body)
                if matches!(&**body, Expression::ArrowFunctionExpression(_))
        )
    } else {
        false
    }
}

/// Recursively check if any arrow in a curried chain should trigger chain breaking.
/// Used by both assignment context (for break-after-equals) and arrow body formatting.
///
/// Prettier breaks the chain if ANY arrow has:
/// - return type annotation AND parameters
/// - type parameters (generics like `<T>`)
/// - non-identifier params (destructuring, defaults, rest)
pub fn arrow_chain_has_return_type(arrow: &internal::ArrowFunctionExpression<'_>) -> bool {
    // Check this arrow for breaking conditions:
    // 1. return_type AND has params
    // 2. type_params (generics)
    // 3. any param that's not a simple identifier
    let has_non_identifier_param = arrow
        .params
        .iter()
        .any(|p| !matches!(p, Expression::Identifier(_)));

    let should_break = (arrow.return_type.is_some() && !arrow.params.is_empty())
        || arrow.type_parameters.is_some()
        || has_non_identifier_param;

    if should_break {
        return true;
    }

    // Check inner arrow if body is an arrow
    if let internal::ArrowFunctionBody::Expression(body) = &arrow.body
        && let Expression::ArrowFunctionExpression(inner) = &**body
    {
        return arrow_chain_has_return_type(inner);
    }

    false
}

/// Check if an expression is self-expanding but won't actually expand because
/// it's empty or trivially short. Used when LHS has a breakable type annotation -
/// we need a break point after `=` so the type doesn't expand prematurely.
///
/// Returns true only if the value truly won't expand:
/// - Empty arrays/objects
/// - Single-element arrays/objects where the element itself won't expand
pub fn is_simple_self_expanding(expr: &Expression<'_>) -> bool {
    match expr {
        // Only empty arrays/objects are "simple" — they truly won't expand.
        // Non-empty arrays/objects have group softlines that can break internally
        // when the line exceeds print_width, so they handle their own expansion.
        // Treating non-empty ones as "simple" would force break-after-operator,
        // preventing the array/object from expanding naturally (e.g., `= [\n  elem,\n]`).
        Expression::ArrayExpression(arr) => arr.elements.is_empty(),
        Expression::ObjectExpression(obj) => obj.properties.is_empty(),
        _ => false,
    }
}

/// Check if we should break after the operator for this expression
///
/// Returns true for expressions that don't break well internally:
/// - Poorly breakable chains (member-only chains, trivial call chains)
/// - String literals (can't break internally)
///
/// Precondition: Only called when is_short_key is false (checked in choose_layout)
///
/// Note: Prettier does NOT include RegexLiteral here. Regex falls through to
/// Fluid layout, which produces the same output since regex can't break internally.
fn should_break_after_operator(
    expr: &Expression<'_>,
    source: &str,
    print_width: usize,
    comments: &[Comment],
) -> bool {
    // Unwrap wrapper expressions to get to the core
    let core_expr = unwrap_expression(expr);

    // String literals should break after operator
    if is_string_literal(core_expr) {
        return true;
    }

    // Check if it's a poorly breakable chain
    is_poorly_breakable_chain(core_expr, source, print_width, comments)
}

/// Unwrap wrapper expressions (TSNonNullExpression, await, unary, yield, parenthesized)
fn unwrap_expression<'a>(expr: &'a Expression<'a>) -> &'a Expression<'a> {
    match expr {
        Expression::TSNonNullExpression(non_null) => unwrap_expression(non_null.expression),
        Expression::AwaitExpression(await_expr) => unwrap_expression(await_expr.argument),
        Expression::UnaryExpression(unary) => unwrap_expression(unary.argument),
        Expression::YieldExpression(yield_expr) => {
            if let Some(arg) = yield_expr.argument {
                unwrap_expression(arg)
            } else {
                expr
            }
        }
        _ => expr,
    }
}

/// Check if a call expression has complex type arguments that provide internal break points.
///
/// Returns `true` (has complex type args) when:
/// - More than 1 type argument, or
/// - The single type argument is an object/type literal, union, or intersection
///   (always), or a mapped type that force-breaks (see below).
///
/// These cases are NOT poorly breakable — the type arguments themselves can break,
/// so we should not break at the assignment operator.
///
/// Matches Prettier's `isCallExpressionWithComplexTypeArguments` (assignment.js:422),
/// which lists object/union/intersection unconditionally, then falls back to
/// `willBreak(print("typeArguments"))`. tsv covers the unconditional list directly;
/// for a single mapped type-arg — which prettier treats as complex only via that
/// `willBreak` fallback (mapped is absent from its explicit list) — it approximates
/// the fallback with a sound static check: the mapped's source span contains a
/// newline. A newline-free single-line mapped type-arg cannot force-break (its only
/// breaks are width-driven `line`s), so it is poorly-breakable and the assignment
/// breaks after `=` like prettier; every force-break — object-style `shouldBreak`
/// from an authored newline, a forcing comment, or a nested forced break — leaves a
/// newline in the span, so a newline is a sound stand-in for the printed doc's
/// `willBreak` (and keeps the `is_poorly_breakable_chain` debug_assert sound: a
/// force-breaking mapped type-arg is never classified poorly-breakable).
///
/// The mapped-type source-newline read below is one half of a pair with
/// `build_mapped_type_doc` (printer/types/composite.rs), which reads the same source
/// newline to decide the force-break. Both are deliberately left un-erased by the
/// canonical reprint (`crate::format_canonical`) — gating either one alone, or both,
/// is unsound. See `build_mapped_type_doc` for the full reasoning before touching this.
fn is_call_with_complex_type_arguments(call: &internal::CallExpression<'_>, source: &str) -> bool {
    use internal::TSType;
    let Some(type_args) = &call.type_arguments else {
        return false;
    };
    if type_args.params.len() > 1 {
        return true;
    }
    match type_args.params.first() {
        Some(TSType::TypeLiteral(_) | TSType::Union(_) | TSType::Intersection(_)) => true,
        Some(TSType::Mapped(m)) => m.span.extract(source).contains('\n'),
        _ => false,
    }
}

/// A chain is poorly breakable if it doesn't have good internal break points:
/// - Member-only chains: `a.b.c.d` (no calls to break on)
/// - Trivial call chains: `a.b().c()` (calls with no/simple args)
///
/// Corresponds to prettier's `isPoorlyBreakableMemberOrCallChain` (assignment.js:359-400).
///
/// ## Architectural difference from prettier
///
/// Prettier prints the call expression doc, then inspects it:
/// - `doc.label?.memberChain` — checks if `printMemberChain` handled the chain
/// - This requires printing the entire call subtree (no caching), then discarding it.
///   The same subtree is printed again for real output — effectively 2x print cost.
///
/// We use static AST analysis instead:
/// - `call_count > 2` → chain formatter handles it (matches prettier's memberChain label)
/// - `call_count == 2` + factory check → factory patterns with trivial args
/// - `is_trivial_call` + `is_short_arg` → checks arg complexity directly
///
/// This is faster (single O(chain_length) walk, no doc allocation) and keeps layout
/// selection cleanly separated from doc building. Validated against 35+ targeted edge
/// cases and 3442 corpus files with zero divergences. Every path where prettier's
/// `willBreak()` returns true maps to a condition we check statically (non-trivial args,
/// comments via `call_arg_has_comments`, complex type args).
///
/// We have `DocArena::will_break()` infrastructure if a real gap ever surfaces.
pub fn is_poorly_breakable_chain(
    expr: &Expression<'_>,
    source: &str,
    print_width: usize,
    comments: &[Comment],
) -> bool {
    is_poorly_breakable_chain_recursive(expr, false, source, print_width, comments)
}

fn is_poorly_breakable_chain_recursive(
    expr: &Expression<'_>,
    deep: bool,
    source: &str,
    print_width: usize,
    comments: &[Comment],
) -> bool {
    match expr {
        // TSNonNullExpression is transparent - continue checking
        Expression::TSNonNullExpression(non_null) => is_poorly_breakable_chain_recursive(
            non_null.expression,
            deep,
            source,
            print_width,
            comments,
        ),
        // Note: TSAsExpression and TSSatisfiesExpression are NOT included here.
        // They have breakable type annotations, so they're not "poorly breakable".

        // CallExpression: check if it's a factory pattern with trivial args
        //
        // Factory patterns (Object.keys, React.createElement, etc.) with 2 calls
        // and trivial args should use break-after-operator layout. This keeps the
        // chain flat on the indented line instead of expanding call args.
        //
        // For non-factory chains or chains with more calls, the chain formatter
        // handles breaking internally.
        Expression::CallExpression(call) => {
            // Check if this call has trivial args (empty or single short arg without comments)
            // Matches Prettier: args.length === 0 || (args.length === 1 && isLoneShortArgument)
            // Arrow functions, objects, arrays are NOT "lone short arguments" - they should
            // be allowed to break internally via the call's conditional_group states.
            //
            // Prettier's isLoneShortArgument returns false when the argument has any comment
            // (hasComment check at utils/index.js:437). Arguments with comments are not
            // "short" because the comment changes formatting behavior — the call should be
            // allowed to expand args instead of breaking at the assignment operator.
            let is_trivial_call = call.arguments.is_empty()
                || (call.arguments.len() == 1
                    && is_short_arg(&call.arguments[0], source, print_width)
                    && !call_arg_has_comments(call, comments));

            if !is_trivial_call {
                return false;
            }

            // Calls with complex type arguments (object/mapped/union/intersection types,
            // or multiple type args) are NOT poorly breakable - they have internal break
            // points via the type arguments.
            // Matches Prettier's `isCallExpressionWithComplexTypeArguments` (assignment.js:422)
            if is_call_with_complex_type_arguments(call, source) {
                return false;
            }

            // Check if callee is a member chain that might be a factory pattern
            if !matches!(
                call.callee,
                Expression::MemberExpression(_) | Expression::TSNonNullExpression(_)
            ) {
                // Non-memberish callee (e.g., `fn()()`), continue down
                return is_poorly_breakable_chain_recursive(
                    call.callee,
                    true,
                    source,
                    print_width,
                    comments,
                );
            }

            // Count calls in the chain
            let call_count = count_calls_in_chain(call.callee) + 1; // +1 for this call

            // Single call with member access: obj.fn(arg) → poorly breakable
            // Continue checking to ensure it's a valid chain structure
            if call_count == 1 {
                return is_poorly_breakable_chain_recursive(
                    call.callee,
                    true,
                    source,
                    print_width,
                    comments,
                );
            }

            // 2 calls: check if factory pattern AND all calls have trivial args
            // Prettier's isPoorlyBreakableMemberOrCallChain recurses through the
            // entire chain checking each call's args. A factory chain like
            // `A.fn("long string").optional()` is NOT poorly breakable because
            // the inner call has a non-trivial arg that provides a good break point.
            if call_count == 2 {
                if is_factory_chain(call.callee, source) {
                    return is_poorly_breakable_chain_recursive(
                        call.callee,
                        true,
                        source,
                        print_width,
                        comments,
                    );
                }
                // Non-factory with 2 calls → let chain formatter handle it
                return false;
            }

            // More than 2 calls → let chain formatter handle it
            false
        }

        // MemberExpression: continue down the chain
        Expression::MemberExpression(member) => {
            is_poorly_breakable_chain_recursive(member.object, true, source, print_width, comments)
        }

        // Base cases: identifiers, `this`, and `super` are valid chain roots
        Expression::Identifier(_) | Expression::ThisExpression(_) | Expression::Super(_) => deep,

        // Everything else breaks the chain
        _ => false,
    }
}

/// Check if an expression is a call on a member chain with complex args.
///
/// Returns true for patterns like `a.b.c.filter((x) => x.s)` where a single call
/// is at the end of a member expression chain AND the call has complex args
/// (arrow functions, objects, arrays). These expressions benefit from fluid layout
/// because breaking at `=` is preferable to expanding call args.
///
/// The key insight is that for single-call chains with non-trivial args, there are
/// no good internal break points. Breaking at `=` keeps the chain flat on an indented
/// line, while expanding args would create deeper nesting.
///
/// Does NOT match:
/// - Bare calls: `foo()` (no member chain)
/// - Multiple calls: `a.b().c()` (handled by chain formatter)
/// - Trivial args: `obj.fn(arg)` (chain formatter handles these well)
pub fn is_call_on_member_chain(expr: &Expression<'_>) -> bool {
    if let Expression::CallExpression(call) = expr {
        // The callee must be a member expression chain (possibly with non-null assertions)
        let is_member_chain = matches!(
            call.callee,
            Expression::MemberExpression(_) | Expression::TSNonNullExpression(_)
        ) && count_calls_in_chain(call.callee) == 0;

        if !is_member_chain {
            return false;
        }

        // Only match when args are "complex" (arrow, object, array) - these are the cases
        // where Prettier breaks at `=` instead of expanding args
        call.arguments.iter().any(|arg| {
            matches!(
                arg,
                Expression::ArrowFunctionExpression(_)
                    | Expression::ObjectExpression(_)
                    | Expression::ArrayExpression(_)
                    | Expression::FunctionExpression(_)
            )
        })
    } else {
        false
    }
}

/// Check if an expression is a single call on a member chain (without complex-arg requirement).
///
/// Like `is_call_on_member_chain` but without the complex-args check. Used to detect
/// `a.fn(anyArg)` patterns for width-based layout decisions in variable declarations.
pub fn is_single_call_on_member_chain(expr: &Expression<'_>) -> bool {
    if let Expression::CallExpression(call) = expr {
        matches!(
            call.callee,
            Expression::MemberExpression(_) | Expression::TSNonNullExpression(_)
        ) && count_calls_in_chain(call.callee) == 0
    } else {
        false
    }
}

/// Check if a call expression on a member chain has a regex literal as its root.
///
/// Matches patterns like `/regex/.exec(b)` where the chain root is a `RegexLiteral`.
/// These chains are NOT poorly-breakable (only Identifier/Super are valid roots in
/// `is_poorly_breakable_chain`) but should use fluid layout matching Prettier's default.
pub fn is_regex_root_chain(expr: &Expression<'_>) -> bool {
    if let Expression::CallExpression(call) = expr {
        let mut node = call.callee;
        loop {
            match node {
                Expression::MemberExpression(member) => node = member.object,
                Expression::TSNonNullExpression(non_null) => node = non_null.expression,
                _ => break,
            }
        }
        matches!(node, Expression::RegexLiteral(_))
    } else {
        false
    }
}

/// Check if an argument is "short" (won't expand when formatted)
///
/// Prettier ref: `isLoneShortArgument` in utils/index.js:434
/// Threshold: `printWidth * LONE_SHORT_ARGUMENT_THRESHOLD_RATE` (0.25)
///
/// Note: Prettier uses JS `.length` (UTF-16 code units) for all measurements,
/// we use `.len()` (UTF-8 bytes). These match for ASCII (the common case).
fn is_short_arg(expr: &Expression<'_>, source: &str, print_width: usize) -> bool {
    // Prettier: LONE_SHORT_ARGUMENT_THRESHOLD_RATE = 0.25 (utils/index.js:433)
    let threshold = print_width / 4;

    match expr {
        // Prettier: node.type === "Identifier" && node.name.length <= threshold
        Expression::Identifier(id) => id.span.extract(source).len() <= threshold,

        // Prettier: isSignedNumericLiteral(node) && !hasComment(node.argument)
        // + general UnaryExpression recursion (line 471-472)
        // We combine both: recurse into all unary arguments.
        Expression::UnaryExpression(unary) => is_short_arg(unary.argument, source, print_width),

        // Prettier: regexpPattern.length <= threshold (line 456)
        Expression::RegexLiteral(regex) => regex.pattern(source).len() <= threshold,

        // Prettier: printString(getRaw(node), options).length <= threshold (line 460)
        Expression::Literal(lit) if matches!(lit.value, internal::LiteralValue::String { .. }) => {
            format_string_literal_from_ast(lit, source).len() <= threshold
        }

        // Prettier: node.quasis[0].value.raw.length <= threshold && !includes("\n") (line 464-468)
        Expression::TemplateLiteral(template) => {
            template.expressions.is_empty()
                && !template.quasis.is_empty()
                && template.quasis[0].raw(source).len() <= threshold
                && !crate::printer::template_literal_has_newlines(template)
        }

        // Prettier: CallExpression with 0 args + Identifier callee (line 475-481)
        // callee.name.length <= threshold - 2 (accounts for "()")
        Expression::CallExpression(call) => {
            call.arguments.is_empty()
                && matches!(call.callee, Expression::Identifier(id)
                    if id.span.extract(source).len() <= threshold.saturating_sub(2))
        }

        // Prettier: isLiteral(node) — numbers, booleans, null, bigint (line 483)
        Expression::Literal(_) => true,

        // this / super — trivially short
        Expression::ThisExpression(_) | Expression::Super(_) => true,

        _ => false,
    }
}

/// Check if a call expression's arguments have any associated comments.
///
/// Matches Prettier's `hasComment(node)` check inside `isLoneShortArgument` (utils/index.js:437).
/// When an argument has comments, it should not be considered "short" because the comment
/// changes the formatting behavior — the call should expand args instead of being treated
/// as a poorly breakable chain.
///
/// Uses the comment region between the callee end and call span end to find any comments
/// in the argument area (covers leading, trailing, and inter-argument comments).
fn call_arg_has_comments(call: &internal::CallExpression<'_>, comments: &[Comment]) -> bool {
    if call.arguments.is_empty() {
        return false;
    }
    // Check for any comments in the argument region (between callee end and closing paren)
    let args_region_start = call.callee.span().end;
    let args_region_end = call.span.end;
    tsv_lang::has_comments_on_page_in_range(comments, args_region_start, args_region_end)
}

/// Check if expression is a type assertion (`as` or `satisfies`) wrapping a call with long arguments.
///
/// Returns true when the expression is TSAsExpression/TSSatisfiesExpression wrapping a
/// CallExpression with non-trivial arguments (multiple args or single long arg).
///
/// Used for break-after-operator layout decisions: when a type assertion call has long args,
/// we break after `=` instead of inside the call. If the call has short/trivial args, the
/// type annotation can break instead.
pub fn is_type_assertion_call(expr: &Expression<'_>, source: &str, print_width: usize) -> bool {
    let call = match expr {
        Expression::TSAsExpression(as_expr) => match as_expr.expression {
            Expression::CallExpression(call) => call,
            _ => return false,
        },
        Expression::TSSatisfiesExpression(sat_expr) => match sat_expr.expression {
            Expression::CallExpression(call) => call,
            _ => return false,
        },
        _ => return false,
    };

    // Non-trivial = multiple args OR single long arg
    // (Trivial = empty args OR single short arg)
    !(call.arguments.is_empty()
        || call.arguments.len() == 1 && is_short_arg(&call.arguments[0], source, print_width))
}

/// Check if an expression is a member-only chain (no calls).
fn is_member_only_chain(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::MemberExpression(member) => is_member_only_chain(member.object),
        Expression::TSNonNullExpression(non_null) => is_member_only_chain(non_null.expression),
        Expression::Identifier(_) | Expression::ThisExpression(_) | Expression::Super(_) => true,
        _ => false,
    }
}

/// Check if an expression is a member-only chain with a literal base
/// (e.g., `'string'.length`, `` `template`.length ``).
///
/// These chains need Fluid assignment layout because the literal base can't break
/// internally but may exceed print_width on the assignment line. Without Fluid,
/// the member access breaks to the next line but the assignment stays flat,
/// potentially exceeding print_width.
///
/// Prettier handles this via `printMemberExpression` which produces
/// `[objectDoc, group(indent([softline, ".prop"]))]` — the assignment's
/// `chooseLayout` returns Fluid (default) for these expressions.
pub fn is_literal_member_chain(expr: &Expression<'_>) -> bool {
    if !matches!(expr, Expression::MemberExpression(_)) {
        return false;
    }
    let root = member_chain_root(expr);
    matches!(
        root,
        Expression::Literal(_) | Expression::TemplateLiteral(_)
    )
}

/// Walk a member chain to find the root expression.
fn member_chain_root<'a>(expr: &'a Expression<'a>) -> &'a Expression<'a> {
    match expr {
        Expression::MemberExpression(member) => member_chain_root(member.object),
        Expression::TSNonNullExpression(non_null) => member_chain_root(non_null.expression),
        _ => expr,
    }
}

/// Count calls in a chain expression.
fn count_calls_in_chain(expr: &Expression<'_>) -> usize {
    match expr {
        Expression::CallExpression(call) => 1 + count_calls_in_chain(call.callee),
        Expression::MemberExpression(member) => count_calls_in_chain(member.object),
        Expression::TSNonNullExpression(non_null) => count_calls_in_chain(non_null.expression),
        _ => 0,
    }
}

/// Check if a chain starts with a factory pattern (capital letter or special prefixes).
///
/// Factory patterns include:
/// - Capital letter start: Object.keys, React.createElement, etc.
/// - Pure `$`/`_` identifiers: `$`, `_`, `$_`, `$__` (lodash-style)
///
/// Matches Prettier's `isFactory`: `/^[A-Z]|^[$_]+$/u` (member-chain.js:273)
/// Note: `$util`, `_helper` etc. are NOT factories — only pure `$`/`_` names.
fn is_factory_chain(expr: &Expression<'_>, source: &str) -> bool {
    match expr {
        Expression::CallExpression(call) => is_factory_chain(call.callee, source),
        Expression::MemberExpression(member) => is_factory_chain(member.object, source),
        Expression::TSNonNullExpression(non_null) => is_factory_chain(non_null.expression, source),
        Expression::Identifier(id) => {
            super::literals::is_factory_identifier_name(id.span.extract(source))
        }
        Expression::ThisExpression(_) | Expression::Super(_) => true,
        _ => false,
    }
}

/// Check if an expression is a simple value that shouldn't break
/// Values that should never break after operator: booleans, numbers, template literals.
///
/// Prettier ref: chooseLayout (assignment.js:181-191) — when !canBreakLeftDoc.
/// Note: ClassExpression is handled separately by is_self_expanding_value.
pub fn is_simple_value(expr: &Expression<'_>) -> bool {
    matches!(
        expr,
        Expression::Literal(lit) if matches!(
            lit.value,
            internal::LiteralValue::Boolean(_)
            | internal::LiteralValue::Number(_)
        )
    ) || matches!(
        expr,
        Expression::TemplateLiteral(_) | Expression::TaggedTemplateExpression(_)
    )
}

impl<'a> Printer<'a> {
    /// Build a Doc for an assignment (variable declaration or object property)
    ///
    /// This is the unified entry point that matches prettier's `printAssignment`.
    ///
    /// `is_short_key`: True for property keys shorter than `tabWidth + MIN_OVERLAP_FOR_BREAK`.
    /// For non-property assignments (e.g., `x = value`), pass `false`.
    ///
    /// `rhs_comments`: Optional inline comments between the operator and the RHS expression
    /// (e.g., `x = /** @type {T} */ (expr)`). Pass `None` for callers that handle
    /// comments separately (object properties, variable declarations).
    pub fn build_assignment_layout(
        &self,
        left_doc: DocId,
        operator: &'static str,
        right_expr: &Expression<'_>,
        is_short_key: bool,
        rhs_comments: Option<DocId>,
    ) -> DocId {
        self.build_assignment_layout_with_line_comment(
            left_doc,
            operator,
            right_expr,
            is_short_key,
            rhs_comments,
            false,
            None,
        )
    }

    /// Like `build_assignment_layout`, but with explicit control over line comment handling.
    ///
    /// When `rhs_has_line_comment` is true, forces `BreakAfterOperator` layout so the
    /// line comment and expression get proper indentation instead of being placed inline.
    ///
    /// When `right_boundary` is `Some`, checks for trailing comments from stripped grouping
    /// parens between `right_expr.span().end` and the boundary. If found, wraps in parens.
    #[allow(clippy::too_many_arguments)]
    pub fn build_assignment_layout_with_line_comment(
        &self,
        left_doc: DocId,
        operator: &'static str,
        right_expr: &Expression<'_>,
        is_short_key: bool,
        rhs_comments: Option<DocId>,
        rhs_has_line_comment: bool,
        right_boundary: Option<u32>,
    ) -> DocId {
        let d = self.d();
        let mut layout = choose_layout(
            right_expr,
            is_short_key,
            d.can_break(left_doc),
            self.source,
            PRINT_WIDTH,
            self.comments,
        );

        // Override layout based on comments:
        //
        // Line comments between operator and RHS (e.g., `a = // comment\n  b`)
        // contain a hardline that forces a break. BreakAfterOperator provides
        // the indent context so the comment and expression are indented together.
        //
        // Multiline block comments (e.g., `a = /**\n * comment\n */\n  b`) also
        // force break-after-operator. Detected via will_break on the rhs_comments doc.
        // Prettier ref: hasLeadingOwnLineComment → break-after-operator in chooseLayout
        if rhs_has_line_comment && layout != AssignmentLayout::BreakAfterOperator {
            layout = AssignmentLayout::BreakAfterOperator;
        }
        if layout != AssignmentLayout::BreakAfterOperator
            && let Some(comments_doc) = rhs_comments
            && d.will_break(comments_doc)
        {
            layout = AssignmentLayout::BreakAfterOperator;
        }
        // A comment the RHS *owns* (a JSDoc cast, a bundler annotation) is glued to its
        // first token and travels inside its doc, so it is never in `rhs_comments` — the
        // gap emits nothing for it. It is still on the page and still hangs the value, so
        // ask the node. See `owned_leading_comment_hangs_value`.
        if layout != AssignmentLayout::BreakAfterOperator
            && self.owned_leading_comment_hangs_value(right_expr)
        {
            layout = AssignmentLayout::BreakAfterOperator;
        }
        // Member-only AND call chains with line comments break internally at the
        // comment location (the chain formatter does this — see
        // build_member_only_chain_with_comments_doc and the call-chain breaking path).
        // Keep the chain with `=` (NeverBreakAfterOperator) so it doesn't also break
        // after the operator, which would double-indent the broken chain.
        if self.has_line_comments_in_member_chain(right_expr)
            || (layout == AssignmentLayout::BreakAfterOperator
                && matches!(right_expr, Expression::CallExpression(_))
                && self.has_line_comments_in_call_chain(right_expr))
        {
            layout = AssignmentLayout::NeverBreakAfterOperator;
        }

        // Signal the arrow printer that a curried arrow-chain RHS should use the
        // assignment-RHS chain layout.
        let chain_context = if is_curried_arrow_chain(right_expr) {
            ArrowChainContext::AssignmentRhs
        } else {
            ArrowChainContext::None
        };
        let right_doc = self.build_with_arrow_chain_context(chain_context, || {
            if let Some(boundary) = right_boundary {
                self.build_expression_doc_with_paren_comments(right_expr, boundary)
            } else {
                self.build_expression_doc(right_expr)
            }
        });
        // Parenthesize an `in` RHS inside a for-header init (`for (a = (b in c);…)`);
        // a no-op elsewhere. The assignment builder is the RHS's only build site and
        // never routes it through `needs_parens`, so the for-init rule is applied here.
        let right_doc = self.wrap_for_init_in(right_expr, right_doc);

        // Validate static heuristic: if is_poorly_breakable_chain classified this
        // expression as poorly breakable (no good internal break points), the printed
        // doc should not contain forced breaks (hardlines/breakParent). If it does,
        // our static AST analysis missed a break-emitting node — the chain actually
        // has internal break points and may need a different layout.
        //
        // A comment the RHS *owns* is exempt: it prints inside the RHS's doc, so the doc
        // force-breaks for a reason that is not a chain break point at all. "Poorly
        // breakable" is a claim about the *chain*, and the layout already hangs the value
        // for such a comment (`owned_leading_comment_hangs_value`). Without the exemption an
        // owned multi-line annotation on a trivial call (`a = /**⏎ * @__PURE__⏎ */ fn();`)
        // trips the assert on every debug build.
        debug_assert!(
            {
                let core_expr = unwrap_expression(right_expr);
                self.owned_leading_comment_hangs_value(right_expr)
                    || !is_poorly_breakable_chain(
                        core_expr,
                        self.source,
                        PRINT_WIDTH,
                        self.comments,
                    )
                    || !d.will_break(right_doc)
            },
            "is_poorly_breakable_chain classified expression as poorly breakable but the \
             printed doc contains forced breaks — static analysis missed a break-emitting node"
        );

        // Build the RHS doc with optional inline comments prepended
        // Comments use Trailing spacing (`/* comment */ `) so no extra space needed
        let right_doc_with_comments = if let Some(comments_doc) = rhs_comments {
            d.concat(&[comments_doc, right_doc])
        } else {
            right_doc
        };

        match layout {
            AssignmentLayout::BreakAfterOperator => {
                // Break after operator with nested groups - matches prettier exactly
                // Structure: group([group(left), op, group(indent([line, right]))])
                // Each inner group can break independently based on remaining width
                d.group(d.concat(&[
                    d.group(left_doc),
                    d.text(operator),
                    hang_after_operator(d, right_doc_with_comments),
                ]))
            }

            AssignmentLayout::NeverBreakAfterOperator => {
                // Never break after operator - matches prettier: group([group(left), op, " ", right])
                // Wrapping left_doc in a group allows right_doc's conditional_groups to expand independently
                // Structure: group([group(left), op, " ", right])
                d.group(d.concat(&[
                    d.group(left_doc),
                    d.text(operator),
                    d.text(" "),
                    right_doc_with_comments,
                ]))
            }

            AssignmentLayout::Fluid => {
                // Fluid layout - break after operator only if needed
                // Matches Prettier's assignment.js lines 59-67 exactly:
                // group([
                //   group(leftDoc),
                //   operator,
                //   group(indent(line), { id: groupId }),      // Marker group
                //   lineSuffixBoundary,
                //   indentIfBreak(rightDoc, { groupId }),      // Conditional indent
                // ])
                d.group(d.concat(&[
                    d.group(left_doc),
                    d.text(operator),
                    fluid_after_operator(d, right_doc_with_comments, GroupId::Assignment),
                ]))
            }
        }
    }

    /// Check if an expression is a member-only chain with line comments.
    ///
    /// Member-only chains with line comments between segments should force
    /// BreakAfterOperator layout to match Prettier's first-pass behavior.
    pub(crate) fn has_line_comments_in_member_chain(&self, expr: &Expression<'_>) -> bool {
        // Only check member-only chains (no calls)
        if !is_member_only_chain(expr) {
            return false;
        }
        self.has_line_comments_in_chain(expr)
    }

    /// Check if an expression is a call chain with line comments.
    ///
    /// For call chains with line comments (e.g., `items // comment\n.foo()`),
    /// we should NOT use BreakAfterOperator because the chain formatter
    /// handles breaking at the comment location.
    pub(crate) fn has_line_comments_in_call_chain(&self, expr: &Expression<'_>) -> bool {
        self.has_line_comments_in_chain(expr)
    }

    /// Check if an expression contains an import expression with trailing comments.
    ///
    /// Import expressions with trailing comments (e.g., `import('./x' // comment)` or
    /// `import('./x' /* comment */)` or `import('./x', {opts} // comment)`)
    /// expand internally and should not use fluid layout. The import itself handles
    /// its own expansion, so the assignment should use default layout.
    /// Handles both direct imports and `await import(...)`.
    pub(crate) fn has_import_with_trailing_comments(&self, expr: &Expression<'_>) -> bool {
        match expr {
            Expression::ImportExpression(import) => {
                let paren_close = import.span.end;
                // Check for comments after the last argument (source or options)
                let last_arg_end = import
                    .options
                    .as_ref()
                    .map_or_else(|| import.source.span().end, |opts| opts.span().end);
                self.has_comments_to_emit_between(last_arg_end, paren_close)
            }
            Expression::AwaitExpression(await_expr) => {
                self.has_import_with_trailing_comments(await_expr.argument)
            }
            _ => false,
        }
    }

    /// Recursively check for line comments in a chain (calls, members, non-null).
    fn has_line_comments_in_chain(&self, expr: &Expression<'_>) -> bool {
        match expr {
            Expression::CallExpression(call) => self.has_line_comments_in_chain(call.callee),
            Expression::MemberExpression(member) => {
                // Check for line comments between object and property
                let obj_end = member.object.span().end;
                let prop_start = member.property.span().start;
                if self.has_line_comments_between(obj_end, prop_start) {
                    return true;
                }
                self.has_line_comments_in_chain(member.object)
            }
            Expression::TSNonNullExpression(non_null) => {
                self.has_line_comments_in_chain(non_null.expression)
            }
            _ => false,
        }
    }
}
