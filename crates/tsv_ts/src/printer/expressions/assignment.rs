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

use crate::ast::internal::{self, AssignmentOperator, Expression, JsdocCast};
use crate::printer::ArrowChainContext;
use crate::printer::Printer;
use crate::printer::calls::chain_has_calls;
use crate::printer::chain::chain_paren_leading_gap;
use crate::printer::class_expr_has_decorators;
use crate::printer::conditional_should_break_after_op;
use crate::printer::expressions::literals::format_string_literal_from_ast;
use crate::printer::is_string_literal;
use crate::printer::layout::{fluid_after_operator, hang_after_operator};
use crate::printer::types::helpers::unwrap_parenthesized;
use tsv_lang::PRINT_WIDTH;
use tsv_lang::Span;
use tsv_lang::doc::GroupId;
use tsv_lang::doc::arena::{DocArena, DocId};

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

    /// Break the LEFT, keep the value on the operator's line.
    /// Structure: group([left, op, " ", group(right)])
    ///
    /// The mirror of `NeverBreakAfterOperator`, and the difference is which side is
    /// grouped: there the LEFT gets its own group (so it breaks on its own width) and the
    /// value rides bare; here the left is bare — it breaks with the outer group, which is
    /// the whole point — and the VALUE gets the group. Prettier spells both, one line
    /// apart (`assignment.js` `break-lhs` / `never-break-after-operator`).
    BreakLhs,
}

/// What the assignment's LEFT is, for the two `choose_layout` arms that ask.
///
/// The two facts are mutually exclusive — a property key is never a destructuring
/// pattern — so they are one parameter rather than two bools that could both be set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentLeft {
    /// Nothing the layout keys on.
    Plain,

    /// An object-literal key shorter than `tabWidth + MIN_OVERLAP_FOR_BREAK`
    /// (prettier's `isObjectPropertyWithShortKey`): wrapping the value under such a key
    /// buys nothing, so an unbreakable value welds to the `:`.
    ShortKey,

    /// A COMPLEX destructuring target (prettier's `isComplexDestructuring`): an object
    /// pattern with more than two properties, one of which is renamed or carries a
    /// default. Prettier breaks the target and keeps the value on the operator's line,
    /// and it decides this **before** it looks at the value at all — so the answer is
    /// the same for every RHS kind ([`Printer::is_complex_destructuring_target`]).
    ComplexPattern,
}

impl AssignmentLeft {
    /// The object-literal property's spelling: `ShortKey` when its key is short enough
    /// for prettier's `isObjectPropertyWithShortKey`, `Plain` otherwise. A property key
    /// is never a destructuring pattern, so this is the whole of that caller's answer.
    pub fn short_key(is_short_key: bool) -> Self {
        if is_short_key {
            Self::ShortKey
        } else {
            Self::Plain
        }
    }
}

impl<'a> Printer<'a> {
    /// Record that `value` is about to be built as the direct value of a **value gap**, so
    /// a JSDoc cast there reflows its comment→`(` break to a space
    /// ([`Printer::jsdoc_cast_value_gap_target`]).
    ///
    /// Called by every value gap that can hold a cast: the declarator `=`, the shared
    /// assignment layout (`build_assignment_layout` — assignment expressions,
    /// class-property initializers, object-literal values), the object property's own
    /// own-line-comment arm (which hangs the value itself rather than routing through that
    /// builder), the binding-default `=` (object, array and parameter alike), the enum
    /// member `=`, and the arrow body.
    ///
    /// A non-cast value clears the target rather than leaving a stale one, which is why
    /// every call is unconditional: the flag names one node, and a gap that no longer has a
    /// cast in it must not keep vouching for the last one that did. That clearing is also
    /// why the answer is read by `build_jsdoc_cast_lead_doc`, which its caller invokes
    /// BEFORE building the inner — a cast's own value gaps mark themselves while that inner
    /// is built, overwriting the mark that was meant for it.
    pub(in crate::printer) fn mark_jsdoc_cast_value_gap(&self, value: &Expression<'_>) {
        self.jsdoc_cast_value_gap_target.set(match value {
            Expression::JsdocCast(cast) => Some(cast.span),
            _ => None,
        });
    }

    /// Whether this cast is the one a value gap recorded — asked where the separator is
    /// chosen ([`Printer::build_jsdoc_cast_lead_doc`]).
    ///
    /// Span-keyed, so a cast nested deeper inside the value (a call argument, an operand)
    /// answers `false` and keeps the width-decided soft `line`. That is the intent: the
    /// reflow rule is about the break between the gap's head and its value, and a cast in
    /// an argument list sits in a list that keeps lines.
    pub(in crate::printer) fn jsdoc_cast_in_value_gap(&self, cast: &JsdocCast<'_>) -> bool {
        self.jsdoc_cast_value_gap_target.get() == Some(cast.span)
    }
}

/// Choose the layout strategy for an assignment
///
/// Follows prettier's `chooseLayout` logic in assignment.js
///
/// ⚠️ **The declarator has a hand-rolled TWIN of this dispatch** — `variable.rs`'s
/// `should_break_after_op_rhs` / `needs_break_after_op_layout` / `needs_fluid_for_breakable_lhs`
/// chain, which answers the same prettier function for `const x = …` and reaches arms this one
/// does not (a module-path call, a regex literal, a multiline string). The two are
/// not interchangeable, and they DRIFT: the sequence arm below was here from the start and
/// missing there, so `const a = (a, b)` hung its operands off the `=` column while `x = (a, b)`
/// broke correctly. Add a `chooseLayout` fact to one and check the other — same standing hazard
/// as the two call-argument printers.
///
/// `left` is what the LHS is ([`AssignmentLeft`]) — the two facts any arm here keys on.
/// `AssignmentLeft::ComplexPattern` answers on its own, ahead of everything; `ShortKey`
/// is read by one `never-break-after-operator` arm. A plain `x = value` is `Plain`.
///
/// `can_break_left`: Whether the printed left-hand side contains a break point
/// (prettier's `canBreakLeftDoc`). It gates the `never-break-after-operator` cases: an
/// unbreakable RHS may only stay welded to the operator when the LHS has nowhere to
/// break either — otherwise the assignment falls through to `fluid` and breaks after the
/// operator, rather than letting the LHS break inside the assignment target.
///
/// Only the two `never-break-after-operator` arms read it, and every arm above them
/// returns without looking, so the caller asks the doc only when one of those arms can be
/// reached (a short key, or a simple value) — see [`Printer::build_assignment_layout`].
/// Prettier computes `canBreakLeftDoc` eagerly; on a real corpus two thirds of those
/// answers went unread, a doc walk apiece.
pub fn choose_layout(
    right_expr: &Expression<'_>,
    left: AssignmentLeft,
    can_break_left: bool,
    printer: &Printer<'_>,
) -> AssignmentLayout {
    // A COMPLEX destructuring target decides the layout on its own, ahead of every arm
    // below — prettier asks it before `shouldBreakAfterOperator` (`assignment.js`
    // chooseLayout), so the target breaks and the value stays on the operator's line
    // whatever the value is: a logical chain, a call, an object literal, an arrow, a
    // string. Placing it after any RHS-keyed arm would answer only the kinds that fall
    // through that arm, and this path would then disagree with the declarator's — which
    // asks the same question at the same point (`build_variable_declaration_doc`) — on
    // every other RHS kind.
    if left == AssignmentLeft::ComplexPattern {
        return AssignmentLayout::BreakLhs;
    }

    // A curried arrow chain (`(a) => (b) => …`) with no head triggering prettier's
    // `shouldBreakChain` uses fluid layout: break after `=` only when the signature heads
    // don't fit on the operator line, letting a hugging body (object/array/block) expand in
    // place otherwise. A chain that DOES trigger it forces the break via break-after-operator
    // (handled by `is_curried_arrow_chain_that_breaks` below) — the assignment RHS renders
    // that break itself, which is why `should_use_arrow_chain_layout` declines the chain
    // layout here and only here.
    if is_curried_arrow_chain(right_expr) && !is_curried_arrow_chain_that_breaks(right_expr) {
        return AssignmentLayout::Fluid;
    }

    // Binary expressions → break after operator, UNLESS it's a logical expression
    // with an inlinable RHS (non-empty object/array). In that case, the RHS
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

    // A curried chain whose heads trigger `shouldBreakChain` → break after operator.
    // Produces: `key:\n  (x: T): H =>\n  (y) =>\n    expr`
    if is_curried_arrow_chain_that_breaks(right_expr) {
        return AssignmentLayout::BreakAfterOperator;
    }

    // Short property keys → never break after operator
    // (wrapping object properties with very short keys usually doesn't add much value)
    if left == AssignmentLeft::ShortKey && !can_break_left {
        return AssignmentLayout::NeverBreakAfterOperator;
    }

    // Check if RHS is a poorly breakable chain (should break after operator)
    if should_break_after_operator(right_expr, printer) {
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

/// Check if a binary expression is a logical expression with an inlinable RHS.
///
/// Returns true when a LogicalExpression (`&&`, `||`, `??`) has a non-empty object,
/// non-empty array, or JSX element on the right side. These cases should NOT use
/// BreakAfterOperator — the RHS handles its own expansion.
///
/// The right operand asked about is [`internal::BinaryExpression::rebalanced_right`], not
/// `binary.right`: prettier normalizes a same-operator right-nested logical tree away at
/// PARSE time, so `a ?? (b ?? [1])` is `(a ?? b) ?? [1]` at every question the printer asks.
/// Reading `binary.right` here is what made the paren-nested authoring diverge AND rewrite
/// its own output on a second pass (`logical/inline_chain_paren_nested_long`).
///
/// Prettier ref: `shouldInlineLogicalExpression` (binaryish.js:361)
pub fn should_inline_logical_expression(binary: &internal::BinaryExpression<'_>) -> bool {
    if !binary.operator.is_logical() {
        return false;
    }

    match binary.rebalanced_right() {
        Expression::ObjectExpression(obj) => !obj.properties.is_empty(),
        Expression::ArrayExpression(arr) => !arr.elements.is_empty(),
        // Note: Prettier also checks isJsxElement, but JSX is not supported in tsv
        _ => false,
    }
}

/// [`arrow_chain_should_break`] asked of an expression: is this a curried chain, and does
/// any head in it trigger prettier's `shouldBreakChain`? False for a non-curried arrow.
///
/// Examples that break:
///   const f = (x: T): H => (y) => expr    // outer has a return type AND parameters
///   const f = (x: T) => (y): H => expr    // inner does
///   const f = ({ a }) => (y) => expr      // a non-identifier parameter
///
/// Examples that stay inline:
///   const f = (x: T) => (y) => expr       // an annotated identifier is still simple
///   const f = (): H => (y) => expr        // a return type with no parameters
pub fn is_curried_arrow_chain_that_breaks(expr: &Expression<'_>) -> bool {
    is_curried_arrow_chain(expr)
        && matches!(expr, Expression::ArrowFunctionExpression(arrow) if arrow_chain_should_break(arrow))
}

/// Check if an expression is a curried arrow function (its body is another
/// arrow). The terminal body may be an expression or a block. Used to route the
/// assignment RHS through the arrow-chain layout regardless of whether any head
/// triggers the chain break.
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

/// Prettier's `shouldBreakChain` (`print/arrow-function.js`), accumulated over every head of
/// a curried chain: the chain's heads each take their own line, however short the chain is,
/// when ANY head has
/// - a return type annotation **and** parameters,
/// - type parameters (generics like `<T>`), or
/// - a parameter that is not a plain identifier (destructuring, default, rest).
///
/// ⚠️ **It is a BREAK, not a refusal of the chain layout**, and the name says so because
/// reading it the other way is a bug: spelled as
/// "has a return type", it routes such a chain *out* of
/// `build_arrow_chain_doc` entirely, which silently drops the break in every position
/// where nothing else owned one (call argument, binaryish operand). The one site that still
/// declines on it is the assignment RHS, where `choose_layout` answers the same question
/// with `AssignmentLayout::BreakAfterOperator` and so renders the break itself — see
/// `should_use_arrow_chain_layout`.
pub fn arrow_chain_should_break(arrow: &internal::ArrowFunctionExpression<'_>) -> bool {
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
        return arrow_chain_should_break(inner);
    }

    false
}

/// The **tail** of prettier's `shouldBreakAfterOperator` — not the whole function: the
/// value UNDER its wrappers (a unary operator, `await`, `yield`, a `!`) is a string
/// literal or a poorly breakable chain, both of which don't break well internally.
///
/// The arms prettier answers AHEAD of that tail — its `switch` (a sequence, a decorated
/// class expression, a binaryish that won't inline) — stay with each caller, since the two
/// callers interleave them with their own arms differently. Both `chooseLayout` twins share
/// this one, so a wrapped chain answers the same in each: `choose_layout` here and the
/// declarator cascade in `statements/variable.rs`
/// (`declarations/variable/poorly_breakable_chain_unwrap_long`).
///
/// Precondition: only called when the left is not a short key (prettier's `hasShortKey`
/// returns before the tail; checked in `choose_layout`).
///
/// Note: prettier does NOT include RegexLiteral here. Regex falls through to Fluid layout,
/// which produces the same output since regex can't break internally — the declarator twin
/// names it outright to reach the same place.
pub fn should_break_after_operator(expr: &Expression<'_>, printer: &Printer<'_>) -> bool {
    // Unwrap wrapper expressions to get to the core
    let core_expr = unwrap_expression(expr);

    // String literals should break after operator
    if is_string_literal(core_expr) {
        return true;
    }

    // Check if it's a poorly breakable chain
    is_poorly_breakable_chain(core_expr, printer)
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
/// - A comment inside the `<…>` forces it open (see the comment leg below) — the one
///   clause that fires whatever the argument's shape, or
/// - The single type argument is an object/type literal, union, or intersection
///   (always), or a mapped type that force-breaks (see below). Asked of the argument
///   with its redundant paren shell unwrapped.
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
fn is_call_with_complex_type_arguments(
    call: &internal::CallExpression<'_>,
    printer: &Printer<'_>,
) -> bool {
    use internal::TSType;
    let Some(type_args) = &call.type_arguments else {
        return false;
    };
    if type_args.params.len() > 1 {
        return true;
    }
    // The comment half of the same `willBreak` fallback, and the only one that fires for
    // a type argument outside the unconditional list: a comment that forces the `<…>`
    // open (`f<A // c>()`) gives the call an internal break point exactly as a union
    // does, so the `=` hugs it. The sound static stand-in is a comment ON PAGE inside the
    // `<…>` *and* a newline in that span — every force-break here is a line comment or an
    // own-line block, and both leave a newline between the brackets, while an inline
    // block (`f</* c */ A>()`), which forces nothing, leaves none. **On-page** is the
    // axis because this is a layout gate; an emit-keyed answer would go blind to an owned
    // comment. Without this leg the call was classified poorly-breakable while its doc
    // force-broke, so the `=` broke *and* the `<…>` opened — two breaks where prettier
    // takes one, and a second fixed point (prettier's form was not tsv-stable).
    if printer.has_comments_on_page_between(type_args.span.start, type_args.span.end)
        && type_args.span.extract(printer.source).contains('\n')
    {
        return true;
    }
    // Asked of the **unwrapped** argument: a redundant paren shell is stripped in
    // type-argument position, and prettier's AST has no paren node here at all, so its
    // unconditional list sees the union directly. Reading the shell instead classified
    // `f<(A | B)>()` as simple where the bare `f<A | B>()` was complex — one authoring of
    // one type reaching two layouts.
    match type_args.params.first().map(unwrap_parenthesized) {
        Some(TSType::TypeLiteral(_) | TSType::Union(_) | TSType::Intersection(_)) => true,
        Some(TSType::Mapped(m)) => m.span.extract(printer.source).contains('\n'),
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
/// - the `memberChain` label is the chain's own grouping against the short-chain cutoff
///   (`call_prints_as_member_chain` — linearize + group, no doc)
/// - `is_trivial_call` + `is_short_arg` → checks arg complexity directly
///
/// This is faster (O(chain_length) walks, no doc allocation) and keeps layout
/// selection cleanly separated from doc building. Every path where prettier's
/// `willBreak()` returns true maps to a condition we check statically (non-trivial args,
/// comments via `call_arg_has_comments`, complex type args).
///
/// We have `DocArena::will_break()` infrastructure if a real gap ever surfaces.
pub fn is_poorly_breakable_chain(expr: &Expression<'_>, printer: &Printer<'_>) -> bool {
    let walk = PoorlyBreakableWalk {
        printer,
        // A member-gap comment only withholds the operator break on a chain prettier
        // prints through `printMemberChain`, and that needs a CALL — see the
        // `MemberExpression` arm.
        chain_has_calls: chain_has_calls(expr),
    };
    is_poorly_breakable_chain_recursive(expr, false, &walk)
}

/// Everything one [`is_poorly_breakable_chain`] walk holds fixed while it descends, so the
/// six recursive call sites carry one reference instead of a widening argument list.
struct PoorlyBreakableWalk<'a, 'src> {
    printer: &'a Printer<'src>,
    /// Whether the chain holds a call anywhere. Scopes the member-gap comment gate, whose
    /// prettier counterpart (`printCallExpression`'s `doc.label?.memberChain`) is reachable
    /// only through a call.
    chain_has_calls: bool,
}

fn is_poorly_breakable_chain_recursive(
    expr: &Expression<'_>,
    deep: bool,
    walk: &PoorlyBreakableWalk<'_, '_>,
) -> bool {
    let printer = walk.printer;
    match expr {
        // TSNonNullExpression is transparent - continue checking
        Expression::TSNonNullExpression(non_null) => {
            is_poorly_breakable_chain_recursive(non_null.expression, deep, walk)
        }
        // Note: TSAsExpression and TSSatisfiesExpression are NOT included here.
        // They have breakable type annotations, so they're not "poorly breakable".

        // CallExpression: prettier's arm is three refusals and a descent — a chain the
        // member-chain printer LABELS (it prints past the cutoff and breaks itself), a
        // non-trivial argument list, complex type arguments — then `goDeeper` on the
        // callee. The label is asked last here because it is the one that walks the chain.
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
                    && is_short_arg(&call.arguments[0], printer.source)
                    && !call_arg_has_comments(call, printer));

            if !is_trivial_call {
                return false;
            }

            // Calls with complex type arguments (object/mapped/union/intersection types,
            // or multiple type args) are NOT poorly breakable - they have internal break
            // points via the type arguments.
            // Matches Prettier's `isCallExpressionWithComplexTypeArguments` (assignment.js:422)
            if is_call_with_complex_type_arguments(call, printer) {
                return false;
            }

            // `doc.label?.memberChain`: a memberish callee prints through
            // `printMemberChain`, and the label is on every result but the short chain's.
            // The count is the chain's own grouping (`call_prints_as_member_chain`), not a
            // call count — `this.x.y()?.a.b('s')` is three groups with no merge and so
            // labelled, while `fn(a).b.then('s')` is two (the base's call joins the first
            // group) and so poorly breakable. A `TSNonNullExpression` callee is grouped
            // the way tsv's chain printer groups it; prettier's `isMemberish` does not
            // name it and prints such a call as an opaque base instead.
            // TODO: mirror that opaque-base grouping for a `!`-wrapped callee (`a.b!().c()`).
            if matches!(
                call.callee,
                Expression::MemberExpression(_) | Expression::TSNonNullExpression(_)
            ) && crate::printer::chain::call_prints_as_member_chain(call, printer)
            {
                return false;
            }

            // `goDeeper` on the callee: every call above the root must be trivial too.
            is_poorly_breakable_chain_recursive(call.callee, true, walk)
        }

        // MemberExpression: a TRAILING comment in the member's own gap — glued to the
        // line the object (or a previous comment) ends on, in the object→`.` gap or a
        // computed member's object→`[` gap (bracket-INTERIOR comments render inline
        // and don't count) — makes the chain printer break the chain there while the
        // chain's HEAD still fits the operator line, so the chain is not poorly
        // breakable and the operator break is withheld (`const c = foo /* c */⏎.b();`).
        // Prettier lands the same way: the comment-carrying chain skips
        // printMemberChain's unlabeled short-chain return, the labeled doc opts out of
        // isPoorlyBreakableMemberOrCallChain (member-chain.js `nodeHasComment`), and
        // fluid keeps the fittable head on the operator line. An OWN-LINE gap comment
        // is deliberately not counted: it makes the head multi-line from its first
        // token, and there prettier's operator break stands
        // (`member/prettier_ignore_base_comment` pins that side). **On page**, because
        // this is a layout gate. Otherwise continue down.
        //
        // ⚠️ Scoped to a chain that HOLDS A CALL, because that is the only chain the
        // modelled prettier path can reach: the `memberChain` label lives on a CALL's
        // doc, so a call-free `a /* c */.b` never opts out and stays poorly breakable
        // (`member/computed_pre_bracket_block_comment_prettier_divergence`). The scope
        // belongs HERE, not in a caller: a second predicate that answers `true` for the
        // call-free chain and is OR-ed in from outside cancels this gate only where it is
        // spelled — and carries a second copy of the chain-root test, which then drifts.
        Expression::MemberExpression(member) => {
            if walk.chain_has_calls {
                let object_end = member.object.span().end;
                let gap_end = if member.computed {
                    crate::printer::chain::find_bracket_position(
                        printer.source,
                        object_end,
                        member.property.span().start,
                    )
                } else {
                    member.property.span().start
                };
                if printer
                    .comments_on_page_between(object_end, gap_end)
                    .any(|c| {
                        !tsv_lang::source_scan::has_newline_before_position(
                            printer.source,
                            c.span.start,
                        )
                    })
                {
                    return false;
                }
            }
            is_poorly_breakable_chain_recursive(member.object, true, walk)
        }

        // Base case: prettier's own last line, `deep && (node.type === "Identifier" ||
        // node.type === "ThisExpression")`.
        //
        // ⚠️ **`super` is not on that list**, and the omission is load-bearing rather than
        // an oversight of prettier's: a `super.p` initializer past width takes the FLUID
        // layout, so the `=` holds its line and the lookup takes the break
        // (`assignment/poorly_breakable_chain_root`). Do not widen it to match the
        // neighbouring root tests in this file — their `Super` arms are NOT precedent.
        // `is_short_arg` names a prettier function that stops at `Identifier` /
        // `ThisExpression` exactly as this one does (`isLoneShortArgument`), so its `Super`
        // is tsv's own widening — inert, since a bare `super` argument is only reachable
        // on tsv's over-accept surface. `is_member_only_chain` mirrors no prettier
        // function at all: it is the structural "this chain holds no call" test, the same
        // axis as [`chain_has_calls`] above, and `super` genuinely is a chain root there.
        // (The chain grouping's `should_not_wrap` merges on a lone `super` head as it does
        // on `this` — that is prettier's `printMemberChain`, a different function, and a
        // `Super`-rooted chain bottoms out false here regardless.)
        Expression::Identifier(_) | Expression::ThisExpression(_) => deep,

        // Everything else breaks the chain
        _ => false,
    }
}

/// Check if an argument is "short" (won't expand when formatted)
///
/// Prettier ref: `isLoneShortArgument` in utils/index.js:434
/// Threshold: `printWidth * LONE_SHORT_ARGUMENT_THRESHOLD_RATE` (0.25)
///
/// Note: Prettier uses JS `.length` (UTF-16 code units) for all measurements,
/// we use `.len()` (UTF-8 bytes). These match for ASCII (the common case).
fn is_short_arg(expr: &Expression<'_>, source: &str) -> bool {
    // Prettier: LONE_SHORT_ARGUMENT_THRESHOLD_RATE = 0.25 (utils/index.js:433)
    let threshold = PRINT_WIDTH / 4;

    match expr {
        // Prettier: node.type === "Identifier" && node.name.length <= threshold
        Expression::Identifier(id) => id.span.extract(source).len() <= threshold,

        // Prettier: isSignedNumericLiteral(node) && !hasComment(node.argument)
        // + general UnaryExpression recursion (line 471-472)
        // We combine both: recurse into all unary arguments.
        Expression::UnaryExpression(unary) => is_short_arg(unary.argument, source),

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

/// Whether a poorly-breakable chain carries a string argument whose **literal text spans
/// lines** — a line continuation (`'a\`⏎`b'`) or a raw `LineTerminator` (invalid JS that
/// tsv currently over-accepts; `docs/checklist_typescript.md` §Known over-acceptance).
///
/// Either way the string force-breaks the RHS doc (`will_break`) at a **leaf** — its own
/// mandatory newline — not at a chain break point, so it does not invalidate the
/// `is_poorly_breakable_chain` claim. For the continuation spelling prettier agrees
/// (its `isLoneShortArgument` string arm does not exclude newlines, unlike the template
/// arm), keeps the chain poorly-breakable, and breaks after the operator; on the raw
/// spelling prettier has no verdict — its parser rejects the input — so the exemption
/// merely keeps the assert sound over tsv's actual accept surface. This is an exemption
/// for the `is_poorly_breakable_chain` debug_assert below, and deliberately NOT the shared
/// `is_multiline_string_literal` (a *layout* predicate, continuation-only — widening that
/// one would change fluid-layout behavior for the over-accepted raw form).
fn chain_has_multiline_string_arg(expr: &Expression<'_>, source: &str) -> bool {
    match expr {
        Expression::CallExpression(call) => {
            call.arguments
                .iter()
                .any(|arg| arg_is_multiline_string(arg, source))
                || chain_has_multiline_string_arg(call.callee, source)
        }
        Expression::MemberExpression(member) => {
            chain_has_multiline_string_arg(member.object, source)
        }
        Expression::TSNonNullExpression(non_null) => {
            chain_has_multiline_string_arg(non_null.expression, source)
        }
        _ => false,
    }
}

/// Whether an argument is a string literal whose raw span contains a newline —
/// unwrapping unary operators like `is_short_arg` does.
fn arg_is_multiline_string(expr: &Expression<'_>, source: &str) -> bool {
    match expr {
        Expression::UnaryExpression(unary) => arg_is_multiline_string(unary.argument, source),
        Expression::Literal(lit) if matches!(lit.value, internal::LiteralValue::String { .. }) => {
            let raw = lit.span.extract(source);
            raw.contains('\n') || raw.contains('\r')
        }
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
fn call_arg_has_comments(call: &internal::CallExpression<'_>, printer: &Printer<'_>) -> bool {
    if call.arguments.is_empty() {
        return false;
    }
    // Check for any comments in the argument region (between callee end and closing paren)
    let args_region_start = call.callee.span().end;
    let args_region_end = call.span.end;
    printer.has_comments_on_page_between(args_region_start, args_region_end)
}

/// Check if expression is a type assertion (`as` or `satisfies`) wrapping a call with long arguments.
///
/// Returns true when the expression is TSAsExpression/TSSatisfiesExpression wrapping a
/// CallExpression with non-trivial arguments (multiple args or single long arg).
///
/// Used for break-after-operator layout decisions: when a type assertion call has long args,
/// we break after `=` instead of inside the call. If the call has short/trivial args, the
/// type annotation can break instead.
pub fn is_type_assertion_call(expr: &Expression<'_>, source: &str) -> bool {
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
        || call.arguments.len() == 1 && is_short_arg(&call.arguments[0], source))
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

/// Check if an expression is a simple value that shouldn't break.
///
/// Prettier's `never-break-after-operator` value list (`chooseLayout`, assignment.js
/// 181-191), read when the LEFT cannot break: a boolean, a number, a template literal, a
/// tagged template, and a class expression — `const x = class {}` stays welded to its `=`
/// past the print width where an empty `{}` drops (`empty_value_long`). Only an
/// UNDECORATED class is on the list because a decorated one never reaches it: prettier's
/// `shouldBreakAfterOperator` answers it first, as both of tsv's layout twins do
/// (`class_expr_has_decorators`).
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
    ) || matches!(expr, Expression::ClassExpression(c) if !class_expr_has_decorators(c))
}

/// The RHS comment / freeze / stripped-paren-boundary controls for
/// [`Printer::build_assignment_layout`].
///
/// Bundles the inputs describing comments, the value-head freeze and grouping-paren
/// boundaries around the RHS, so the layout entry point takes one value instead of
/// four loose args.
pub struct RhsCommentInfo {
    /// Inline comments between the operator and the RHS (e.g. `x = /** @type {T} */ (e)`);
    /// `None` when the caller handles comments separately.
    pub comments: Option<DocId>,
    /// The operator→RHS gap holds a line comment (or an own-line / multiline block),
    /// forcing `BreakAfterOperator` so the comment and value indent together.
    pub has_line_comment: bool,
    /// The gap's run is glued through to the value
    /// ([`Printer::comment_run_glued_through`]), so a `will_break` on `comments` is a
    /// preserved multiline block's interior and must not hang the value: the layout
    /// stays width-decided, with the run's first line charged to the operator's line by
    /// the fits walk. `false` when the run has an own-line separator (a real hardline
    /// the value must sit under), or when `comments` is `None`.
    pub glued_through: bool,
    /// An INDENTABLE block comment leads the RHS
    /// ([`Printer::indentable_block_leads_value`]) — prettier's `chooseLayout` fourth
    /// disjunct, which takes `BreakAfterOperator` ahead of every layout the value or the
    /// left would otherwise choose. Resolved by the caller, which is the one that holds the
    /// gap's spans; **on page**, so a comment the RHS owns counts even though `comments` is
    /// `None` for it.
    pub indentable_leads_value: bool,
    /// When `Some`, scan for trailing comments from stripped grouping parens between
    /// the RHS end and this boundary, wrapping in parens if found.
    pub boundary: Option<u32>,
    /// The value-head freeze the operator→RHS gap resolved (an own-line directive there
    /// freezes the whole RHS, per [`Printer::value_head_frozen_span`]): the span to emit
    /// verbatim in place of the RHS's doc.
    pub frozen: Option<Span>,
}

impl RhsCommentInfo {
    /// The gap carries nothing but a freeze verdict — the shape most callers want, and
    /// the one the zero-comment fast paths reach with `None`.
    pub fn frozen_only(frozen: Option<Span>) -> Self {
        Self {
            comments: None,
            has_line_comment: false,
            glued_through: false,
            indentable_leads_value: false,
            boundary: None,
            frozen,
        }
    }
}

impl<'a> Printer<'a> {
    /// Record `value` as the value an ASSIGNMENT position is building — the parents
    /// prettier's binaryish `shouldIndentIfInlining` names (a declarator's initializer, an
    /// assignment's RHS, an object property's value, a class property's initializer). The
    /// binary chain builder reads it for the one layout arm keyed on that position: an
    /// inlining logical chain with earlier operators indents them
    /// (`build_binary_chain_doc_core`). Called by [`Self::build_assignment_layout`] and by
    /// the two declarator value builders, which build their value without it.
    ///
    /// Keyed by span and not consumed, like `mark_ternary_extra_indent`.
    pub(crate) fn mark_assignment_value(&self, value: &Expression<'_>) {
        self.assignment_value_target.set(Some(value.span()));
    }

    /// Build a Doc for an assignment (variable declaration or object property)
    ///
    /// This is the unified entry point that matches prettier's `printAssignment`.
    ///
    /// `left` is what the LHS is ([`AssignmentLeft`]): a short object-literal key, a
    /// complex destructuring target, or `Plain` for everything else (`x = value`).
    ///
    /// `rhs_info` carries the operator→RHS gap's facts — inline comments, whether the gap
    /// forces `BreakAfterOperator`, a stripped-paren boundary to scan, and the value-head
    /// freeze. [`RhsCommentInfo::frozen_only`] is the shape for a caller that has nothing
    /// but the freeze verdict.
    ///
    /// `operator` is the operator's doc (`:`, ` =`, ` +=`, …), built by the caller where
    /// its spelling is a literal — a constant there, where a `&'static str` threaded down
    /// to this one `text()` site would be a runtime match at every layout.
    pub fn build_assignment_layout(
        &self,
        left_doc: DocId,
        operator: DocId,
        right_expr: &Expression<'_>,
        left: AssignmentLeft,
        rhs_info: RhsCommentInfo,
    ) -> DocId {
        let d = self.d();
        // A frozen RHS bypasses layout selection entirely: the slice prints verbatim, so
        // every choice `choose_layout` makes about how the value breaks is moot, and the
        // heuristic assertions below reason about a doc the value no longer has. The
        // directive's own line already hangs the value under the operator.
        if let Some(frozen) = rhs_info.frozen {
            return self.build_frozen_assignment_doc(
                left_doc,
                operator,
                right_expr,
                frozen,
                rhs_info.comments,
                rhs_info.boundary,
            );
        }
        // The LHS walk is asked only when an arm that reads it can be reached: the two
        // `never-break-after-operator` arms, gated on a short key or a simple value. Every
        // other arm of `choose_layout` returns first, and on a real corpus that is two
        // thirds of the asks. A gated bool rather than a lazy cell: this frame sits on the
        // object-literal recursion (property → value → property), where a memo cell's
        // extra words cost ~1% of the nesting depth the formatter survives.
        let can_break_left = (left == AssignmentLeft::ShortKey || is_simple_value(right_expr))
            && d.can_break(left_doc);
        let mut layout = choose_layout(right_expr, left, can_break_left, self);

        // Override layout based on comments:
        //
        // Line comments between operator and RHS (e.g., `a = // comment\n  b`)
        // contain a hardline that forces a break. BreakAfterOperator provides
        // the indent context so the comment and expression are indented together.
        //
        // Multiline block comments (e.g., `a = /**\n * comment\n */\n  b`) also
        // force break-after-operator. Detected via will_break on the rhs_comments doc.
        // Prettier ref: hasLeadingOwnLineComment → break-after-operator in chooseLayout
        if rhs_info.has_line_comment && layout != AssignmentLayout::BreakAfterOperator {
            layout = AssignmentLayout::BreakAfterOperator;
        }
        // The same comment one shell deeper: an own-line run inside the grouping parens of
        // the value's LEFTMOST node (`= (⏎// c⏎a as any) ? b : c`, `= (⏎// c⏎a).b`). The
        // value's printer strips that shell and hoists the run ahead of its whole doc, so
        // the doc opens with the run and a hardline — exactly what the operator→RHS gap's
        // own line comment produces, and it wants the same layout: `BreakAfterOperator`
        // is what puts the run and the value under the operator at the value's indent.
        // Left to the value's own arm (`Fluid` for a conditional, the chain's for a
        // member), the run rendered FLUSH on the operator's line and the reparse — reading
        // the comment as the operator→RHS gap's — indented it, a second fixed point one
        // pass away. Prettier reaches the operator→RHS reading on its own second pass; a
        // binary value already lands here through its own arm, so this is the shape the
        // seam gives every left-side kind. Asked ahead of the chain overrides below, which
        // keep the value on the operator's line only for a run its own pair RETAINS.
        if layout != AssignmentLayout::BreakAfterOperator
            && self.left_spine_shell_has_own_line_comment(right_expr)
        {
            layout = AssignmentLayout::BreakAfterOperator;
        }
        if layout != AssignmentLayout::BreakAfterOperator
            && let Some(comments_doc) = rhs_info.comments
            && d.will_break(comments_doc)
            // A break that is a preserved multiline block's interior in a run glued
            // through to the value (`= /* x⏎y */ /* c */ v`) is not an own-line
            // separator: prettier does not hang the run, and the layout stays
            // width-decided — the fits walk charges the run's first line to the
            // operator's line, so `Fluid` breaks there exactly when that line does
            // not fit.
            && !rhs_info.glued_through
        {
            layout = AssignmentLayout::BreakAfterOperator;
        }
        // Member-only AND call chains with line comments break internally at the
        // comment location (the chain formatter does this — see
        // build_member_only_chain_with_comments_doc and the call-chain breaking path).
        // Keep the chain with `=` (NeverBreakAfterOperator) so it doesn't also break
        // after the operator, which would double-indent the broken chain.
        //
        // Asked BEFORE the hang rule below: an indentable comment leading the value
        // hangs it whatever its chain holds — prettier hangs it too, and the chain
        // then breaks at its own comment one level further in — and the declarator's
        // twin answers the same way (`chain_value_glued_multiline_block_comment`).
        if self.has_line_comments_in_member_chain(right_expr)
            || (layout == AssignmentLayout::BreakAfterOperator
                && self.has_line_comments_in_call_chain(right_expr))
        {
            layout = AssignmentLayout::NeverBreakAfterOperator;
        }
        // The operator→RHS hang — prettier's `chooseLayout` fourth disjunct plus the
        // own-line JSDoc cast, both in [`Printer::value_hangs_under_operator`], stated once
        // so this and the declarator's twin cannot answer it differently. Its indentable
        // half is resolved by the caller, which holds the gap's spans
        // (`RhsCommentInfo::indentable_leads_value`).
        //
        // A separate arm from the `will_break` one above and not foldable into it: that one
        // is width-adjacent and declines a run glued through to the value, where this rule
        // fires ON the glue (`= /**⏎ */ (x)`, the comment glued to a paren the printer
        // discards). A *preserved* multi-line block takes neither and is placed by width
        // like any other text, its first line charged to the operator's line.
        //
        // A comment the RHS *owns* (a JSDoc cast, a bundler annotation) is glued to its
        // first token and travels inside its doc, so it is never in `rhs_comments` — the
        // gap emits nothing for it. The gap reading is **on page** and counts it anyway,
        // which is why no owned companion is needed here.
        if layout != AssignmentLayout::BreakAfterOperator
            && self.value_hangs_under_operator(rhs_info.indentable_leads_value, right_expr)
        {
            layout = AssignmentLayout::BreakAfterOperator;
        }

        // Signal the arrow printer that a curried arrow-chain RHS should use the
        // assignment-RHS chain layout.
        let chain_context = if is_curried_arrow_chain(right_expr) {
            ArrowChainContext::AssignmentRhs
        } else {
            ArrowChainContext::None
        };
        // Every gap routed through this builder is a value gap
        // (`mark_jsdoc_cast_value_gap`), and every value built here is an assignment's.
        self.mark_jsdoc_cast_value_gap(right_expr);
        self.mark_assignment_value(right_expr);
        let right_doc = self.build_with_arrow_chain_context(chain_context, || {
            if let Some(boundary) = rhs_info.boundary {
                self.build_expression_doc_with_paren_comments(right_expr, boundary, false)
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
        // Two exemptions, both cases where the doc force-breaks for a reason that is NOT a
        // chain break point the static analysis missed:
        //
        // - A COMMENT in the RHS. The classifier deliberately models comment-free geometry
        //   only (as prettier's `isLoneShortArgument` bails on `hasComment`), and every
        //   comment-driven break is the comment's own: an interior line comment
        //   (`a = foo // c⏎.bar!` — a `//` must end its line; the layout override above
        //   takes NeverBreakAfterOperator for it), a multiline block whose interior newline
        //   must print (`a = b/* a⏎b */.map(fn)`), a glued block the chain formatter breaks
        //   per-member for, or an owned leading comment printing inside the value's doc (a
        //   JSDoc cast, a bundler annotation — `owned_leading_comment_at`, whose span sits
        //   BEFORE the RHS span and so needs its own arm). The exemption is deliberately
        //   ONE span predicate (on-page, the layout axis) plus the owned arm — not a
        //   per-gap enumeration: the RHS is classified through `unwrap_expression` (strips
        //   `await`/`!`/unary/`yield`), so a gap enumeration must cover the wrapper gaps
        //   too (`a: await // c⏎x.a`), and two rounds of enumerating (line comments, then
        //   multiline blocks) each missed one. (The VariableDeclarator path prints through
        //   its own layout and never reaches this assert; the assignment-expression /
        //   object-property / pattern / class-field callers do.)
        //   TODO: for a multiline block comment in a chain gap there is no layout override
        //   yet — tsv breaks after the operator AND at the comment
        //   (`a =⏎ b /* a⏎b */⏎ .map(fn);`) where prettier keeps the chain on the operator
        //   line (one break). A real layout gap, deliberately NOT sanctioned by this
        //   exemption; fixing it means extending the NeverBreakAfterOperator override
        //   above to that comment kind, fixtures-first.
        // - A *line-continuation string argument* (`a = fn('x\`⏎`y')`) force-breaks the doc at
        //   a leaf — the string's own mandatory newline — not at a chain break point. Prettier
        //   agrees the chain is poorly breakable here (a line-continuation string is a lone
        //   short argument) and breaks after the operator, so the classification is sound.
        //   (`chain_has_multiline_string_arg`).
        debug_assert!(
            {
                let core_expr = unwrap_expression(right_expr);
                let rhs_span = right_expr.span();
                self.owned_leading_comment_at(rhs_span.start).is_some()
                    || self.has_comments_on_page_between(rhs_span.start, rhs_span.end)
                    || chain_has_multiline_string_arg(core_expr, self.source)
                    || !is_poorly_breakable_chain(core_expr, self)
                    || !d.will_break(right_doc)
            },
            "is_poorly_breakable_chain classified expression as poorly breakable but the \
             printed doc contains forced breaks — static analysis missed a break-emitting node"
        );

        // Build the RHS doc with optional inline comments prepended
        // Comments use Trailing spacing (`/* comment */ `) so no extra space needed
        let right_doc_with_comments = if let Some(comments_doc) = rhs_info.comments {
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
                    operator,
                    hang_after_operator(d, right_doc_with_comments),
                ]))
            }

            AssignmentLayout::NeverBreakAfterOperator => {
                // Never break after operator - matches prettier: group([group(left), op, " ", right])
                // Wrapping left_doc in a group allows right_doc's conditional_groups to expand independently
                // Structure: group([group(left), op, " ", right])
                d.group(d.concat(&[
                    d.group(left_doc),
                    operator,
                    d.text(" "),
                    right_doc_with_comments,
                ]))
            }

            AssignmentLayout::BreakLhs => {
                // Break the left, keep the value on the operator's line - matches
                // prettier: group([leftDoc, operator, " ", group(rightDoc)])
                //
                // `left_doc` is not merely left ungrouped here — the group its own
                // builder put on it is STRIPPED. Both halves matter: an inner group
                // answers the fit check on its own content, so the pattern stays flat
                // while the line overflows and the break lands on the value instead,
                // which is the exact break this layout exists to force. Prettier reaches
                // the same shape from the other side: `printObject` returns the pattern's
                // content *ungrouped* when its parent is an `AssignmentExpression` or a
                // `VariableDeclarator` ("printAssignment is responsible for adding a group
                // if needed", `print/object.js`). The declarator's hand-rolled twin
                // strips it the same way (`build_variable_declaration_doc`).
                d.group(d.concat(&[
                    d.unwrap_group(left_doc),
                    operator,
                    d.text(" "),
                    d.group(right_doc_with_comments),
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
                    operator,
                    fluid_after_operator(d, right_doc_with_comments, GroupId::Assignment),
                ]))
            }
        }
    }

    /// The `BreakAfterOperator` shape for a FROZEN right-hand side: the gap's comment run
    /// (which holds the directive that froze it) and the verbatim slice, hung under the
    /// operator.
    ///
    /// Always this one layout — the directive is alone on its line, so the run ends in a
    /// hardline and no width-decided form could keep the value beside the operator anyway.
    /// The slice replaces the RHS's doc, so no POSITION paren is added here: the ordinary
    /// path leaves that to the caller too (an object property and a class field each apply
    /// their own `needs_parens` before choosing this layout).
    ///
    /// `boundary` is the gap the RHS's own grouping shell lives in
    /// (`RhsCommentInfo::boundary`), threaded for exactly the reason the ordinary path
    /// threads it: the shell's `)` is past the slice's end, so a comment written inside it
    /// reaches no other emitter ([`Printer::build_frozen_value_shell_doc`]). `None` at a
    /// host that scans no shell, where the plain slice is the whole emission.
    fn build_frozen_assignment_doc(
        &self,
        left_doc: DocId,
        operator: DocId,
        right_expr: &Expression<'_>,
        frozen: Span,
        comments: Option<DocId>,
        boundary: Option<u32>,
    ) -> DocId {
        let d = self.d();
        let frozen_doc = match boundary {
            Some(boundary) => {
                self.build_frozen_value_shell_doc(right_expr, frozen, boundary, false)
            }
            None => self.build_frozen_expression_doc(right_expr, frozen),
        };
        let rhs = match comments {
            Some(comments_doc) => d.concat(&[comments_doc, frozen_doc]),
            None => frozen_doc,
        };
        d.group(d.concat(&[d.group(left_doc), operator, hang_after_operator(d, rhs)]))
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

    /// Check if an expression is a call-bearing chain with line comments.
    ///
    /// For call chains with line comments (e.g., `items // comment\n.foo()`),
    /// we should NOT use BreakAfterOperator because the chain formatter
    /// handles breaking at the comment location. The chain question is
    /// [`chain_has_calls`] — a call anywhere in the chain, not just at the top:
    /// a trailing plain member over a call base (`fn()⏎// c⏎.bar`) breaks at the
    /// comment exactly like its called twin (`.bar()`), and gating on the top
    /// node is what made only the twin hug the `=`.
    pub(crate) fn has_line_comments_in_call_chain(&self, expr: &Expression<'_>) -> bool {
        chain_has_calls(expr) && self.has_line_comments_in_chain(expr)
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
        // The base's own LEADING gap, where the pair the base prints keeps the run
        // inside it (`const m = ( // c⏎\ta?.b⏎)!.ccc`, `const z = ( // c⏎\t() => {}⏎)().p`).
        // That run breaks the chain internally exactly as one in a member or
        // operand→`!` gap does, so the `=` hugs the shell rather than also breaking.
        // The gap is asked through the one predicate the linearizer and the chain
        // head read (`chain_paren_leading_gap`); a run the pair does NOT keep — a
        // cast, a sequence, a ternary base — leads the whole value instead and is
        // deliberately not a chain break.
        if let Some((start, base_start)) = chain_paren_leading_gap(expr, self.comments)
            && self.has_line_comments_between(start, base_start)
        {
            return true;
        }
        match expr {
            Expression::CallExpression(call) => {
                // A line comment between the callee (or its type arguments) and the
                // arguments forces the break too: the `//` must end its line before the
                // `(`, so the argument list drops to an indented continuation
                // (`build_empty_args_parens_doc`). That break is the comment's, not a chain break
                // point the static analysis missed.
                let args_gap_start = call
                    .type_arguments
                    .as_ref()
                    .map_or_else(|| call.callee.span().end, |ta| ta.span.end);
                let args_gap_end = call
                    .arguments
                    .first()
                    .map_or(call.span.end, |arg| arg.span().start);
                self.has_line_comments_between(args_gap_start, args_gap_end)
                    || self.has_line_comments_in_chain(call.callee)
            }
            Expression::MemberExpression(member) => {
                // Check for line comments between object and property. For a computed
                // member this range deliberately includes the bracket INTERIOR: a
                // comment inside `[...]` makes the brackets break around it, so the
                // chain still breaks internally and the layout answer is the same —
                // keep the RHS on the operator line rather than also breaking after
                // `=` (`const a = obj[ // force⏎…`).
                //
                // ⚠️ A **frozen** gap is the one exception, and for the reason the rule
                // is stated on: there is no chain to break. The `//` there is the
                // format-ignore directive itself, and what follows is a verbatim slice
                // the chain formatter never lays out — so the `=` owes it nothing, and
                // the RHS takes the same layout its unfrozen twin takes. Reading the
                // directive as a chain break instead pinned the RHS to the `=` line
                // where the identical unfrozen source hangs it
                // (`const a = /* line1⏎line2 */ obj⏎// prettier-ignore⏎.prop`), a
                // divergence only an owned multi-line comment ahead of the base makes
                // visible — see `build_frozen_opaque_node_doc`.
                let obj_end = member.object.span().end;
                let prop_start = member.property.span().start;
                if !self.member_gap_frozen(obj_end, prop_start)
                    && self.has_line_comments_between(obj_end, prop_start)
                {
                    return true;
                }
                self.has_line_comments_in_chain(member.object)
            }
            Expression::TSNonNullExpression(non_null) => {
                // The operand→`!` gap: a `//` in a retained paren shell
                // (`(a?.b // c⏎)!`) opens the shell — the chain breaks at the comment
                // like any other chain gap, so the `=` hugs it.
                let operand_end = non_null.expression.span().end;
                if self.has_line_comments_between(operand_end, non_null.span.end) {
                    return true;
                }
                self.has_line_comments_in_chain(non_null.expression)
            }
            _ => false,
        }
    }
}

impl AssignmentOperator {
    /// The operator with its leading space (`" ="`, `" +="`, …) as a doc.
    ///
    /// Each arm spells its own literal, so the text is a constant at the arm — the
    /// prelude fold, or a direct intern of a fixed address — where `d.text(
    /// self.as_str_with_leading_space())` would hand `text()` a runtime string and pay the
    /// prelude's length switch and byte jump at every assignment. A test binds the two
    /// tables.
    #[inline]
    pub(in crate::printer) fn doc_with_leading_space(self, d: &DocArena) -> DocId {
        match self {
            AssignmentOperator::Assign => d.text(" ="),
            AssignmentOperator::AddAssign => d.text(" +="),
            AssignmentOperator::SubtractAssign => d.text(" -="),
            AssignmentOperator::MultiplyAssign => d.text(" *="),
            AssignmentOperator::DivideAssign => d.text(" /="),
            AssignmentOperator::RemainderAssign => d.text(" %="),
            AssignmentOperator::ExponentiateAssign => d.text(" **="),
            AssignmentOperator::LeftShiftAssign => d.text(" <<="),
            AssignmentOperator::RightShiftAssign => d.text(" >>="),
            AssignmentOperator::UnsignedRightShiftAssign => d.text(" >>>="),
            AssignmentOperator::BitwiseOrAssign => d.text(" |="),
            AssignmentOperator::BitwiseXorAssign => d.text(" ^="),
            AssignmentOperator::BitwiseAndAssign => d.text(" &="),
            AssignmentOperator::LogicalOrAssign => d.text(" ||="),
            AssignmentOperator::LogicalAndAssign => d.text(" &&="),
            AssignmentOperator::NullishAssign => d.text(" ??="),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `doc_with_leading_space`'s per-arm literals are `as_str_with_leading_space`'s
    /// table: within one document `text()` interns a static by identity, so the two
    /// spellings agree exactly when they return the same node.
    #[test]
    fn assignment_operator_doc_matches_its_string_table() {
        let d = DocArena::new();
        for op in [
            AssignmentOperator::Assign,
            AssignmentOperator::AddAssign,
            AssignmentOperator::SubtractAssign,
            AssignmentOperator::MultiplyAssign,
            AssignmentOperator::DivideAssign,
            AssignmentOperator::RemainderAssign,
            AssignmentOperator::ExponentiateAssign,
            AssignmentOperator::LeftShiftAssign,
            AssignmentOperator::RightShiftAssign,
            AssignmentOperator::UnsignedRightShiftAssign,
            AssignmentOperator::BitwiseOrAssign,
            AssignmentOperator::BitwiseXorAssign,
            AssignmentOperator::BitwiseAndAssign,
            AssignmentOperator::LogicalOrAssign,
            AssignmentOperator::LogicalAndAssign,
            AssignmentOperator::NullishAssign,
        ] {
            assert_eq!(
                op.doc_with_leading_space(&d),
                d.text(op.as_str_with_leading_space()),
                "{op:?}"
            );
        }
    }
}
