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

use super::Printer;
use super::expressions::format_string_literal_from_ast;
use crate::ast::internal::{self, Expression};
use tsv_lang::doc::{self, Doc, GroupId};

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

/// Choose the layout strategy for an assignment
///
/// Follows prettier's `chooseLayout` logic in assignment.js
///
/// `is_short_key`: True for property keys shorter than `tabWidth + MIN_OVERLAP_FOR_BREAK`.
/// Short keys don't benefit from breaking after the colon. For non-property assignments
/// (e.g., `x = value`), pass `false`.
pub fn choose_layout(
    right_expr: &Expression,
    is_short_key: bool,
    source: &str,
    print_width: usize,
) -> AssignmentLayout {
    // Objects, arrays, functions, classes, and calls handle their own expansion
    // The value expands internally: `key: { ... }` not `key:\n{ ... }`
    //
    // Call expressions use conditional_group to try multiple states during fits().
    // With the updated fits.rs logic, calls can "fit" even when they break internally,
    // allowing the call to handle breaking before the assignment does.
    if is_self_expanding_value(right_expr) {
        return AssignmentLayout::NeverBreakAfterOperator;
    }

    // Binary expressions → break after operator
    if matches!(right_expr, Expression::BinaryExpression(_)) {
        return AssignmentLayout::BreakAfterOperator;
    }

    // Sequence expressions → break after operator
    if matches!(right_expr, Expression::SequenceExpression(_)) {
        return AssignmentLayout::BreakAfterOperator;
    }

    // Conditional expressions with binary test → break after operator
    // When a ternary has a BinaryExpression as its test, prettier breaks after the
    // operator to put the entire ternary on the next line with increased indentation.
    // This allows the ternary to stay intact if it fits, or break internally if needed.
    if let Expression::ConditionalExpression(cond) = right_expr
        && matches!(cond.test.as_ref(), Expression::BinaryExpression(_))
    {
        return AssignmentLayout::BreakAfterOperator;
    }

    // Short property keys → never break after operator
    // (wrapping object properties with very short keys usually doesn't add much value)
    if is_short_key {
        return AssignmentLayout::NeverBreakAfterOperator;
    }

    // Check if RHS is a poorly breakable chain (should break after operator)
    // Note: is_short_key is false here due to early return above
    if should_break_after_operator(right_expr, source, print_width) {
        return AssignmentLayout::BreakAfterOperator;
    }

    // Simple values that shouldn't break → never break after operator
    if is_simple_value(right_expr) {
        return AssignmentLayout::NeverBreakAfterOperator;
    }

    // Default → fluid layout
    AssignmentLayout::Fluid
}

/// Check if an expression handles its own expansion (objects, arrays, functions, classes)
///
/// These values should never have a break between key: and value because they
/// expand internally. `key: { ... }` NOT `key:\n{ ... }`
///
/// Note: Call expressions and NewExpressions are NOT included here.
/// They use Fluid layout (from choose_layout default) which allows breaking
/// after the operator when the total line exceeds printWidth.
pub fn is_self_expanding_value(expr: &Expression) -> bool {
    matches!(
        expr,
        Expression::ObjectExpression(_)
            | Expression::ArrayExpression(_)
            | Expression::FunctionExpression(_)
            | Expression::ArrowFunctionExpression(_)
            | Expression::ClassExpression(_)
    )
}

/// Check if an expression is self-expanding but won't actually expand because
/// it's empty or trivially short. Used when LHS has a breakable type annotation -
/// we need a break point after `=` so the type doesn't expand prematurely.
pub fn is_simple_self_expanding(expr: &Expression) -> bool {
    match expr {
        Expression::ArrayExpression(arr) => arr.elements.len() <= 1,
        Expression::ObjectExpression(obj) => obj.properties.len() <= 1,
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
fn should_break_after_operator(expr: &Expression, source: &str, print_width: usize) -> bool {
    // Unwrap wrapper expressions to get to the core
    let core_expr = unwrap_expression(expr);

    // String literals should break after operator
    if matches!(
        core_expr,
        Expression::Literal(lit) if matches!(lit.value, internal::LiteralValue::String { .. })
    ) {
        return true;
    }

    // Check if it's a poorly breakable chain
    is_poorly_breakable_chain(core_expr, source, print_width)
}

/// Unwrap wrapper expressions (TSNonNullExpression, await, unary, yield)
fn unwrap_expression(expr: &Expression) -> &Expression {
    match expr {
        Expression::TSNonNullExpression(non_null) => unwrap_expression(&non_null.expression),
        Expression::AwaitExpression(await_expr) => unwrap_expression(&await_expr.argument),
        Expression::UnaryExpression(unary) => unwrap_expression(&unary.argument),
        Expression::YieldExpression(yield_expr) => {
            if let Some(arg) = &yield_expr.argument {
                unwrap_expression(arg)
            } else {
                expr
            }
        }
        _ => expr,
    }
}

/// Check if an expression is a member chain that will use member chain formatting.
///
/// Prettier's `printMemberChain` returns `group(oneLine)` without `memberChain` label
/// when the chain is too short (groups.length <= cutoff), causing
/// `isPoorlyBreakableMemberOrCallChain` to treat it as "poorly breakable".
///
/// Cutoff logic:
/// - Factory pattern (starts with capital or `$_`): cutoff = 3
/// - Non-factory: cutoff = 2
///
/// Groups are counted following Prettier's grouping algorithm:
/// - First group: base identifier
/// - Subsequent groups: sequences of MemberExpression followed by CallExpression
///
/// See: prettier/src/language-js/print/member-chain.js (lines 310-360)
fn is_member_chain_with_multiple_calls(expr: &Expression, source: &str) -> bool {
    // Count groups in the chain
    let group_count = count_member_chain_groups(expr);

    // If we don't have at least 2 groups (base + 1 call), not a chain
    if group_count < 2 {
        return false;
    }

    // Extract the base identifier to check factory pattern
    let base_name = get_base_identifier(expr, source);

    // Determine cutoff based on factory pattern
    let cutoff = if let Some(name) = base_name {
        if is_factory_pattern(name) { 3 } else { 2 }
    } else {
        2
    };

    // If groups <= cutoff, Prettier doesn't use memberChain label
    // so it's treated as "poorly breakable"
    group_count > cutoff
}

/// Check if a name matches Prettier's factory pattern: /^[A-Z]|^[$_]+$/
fn is_factory_pattern(name: &str) -> bool {
    // Starts with capital letter, or all characters are $ or _
    name.chars()
        .next()
        .is_some_and(|c| c.is_uppercase() || name.chars().all(|c| c == '$' || c == '_'))
}

/// Count groups in a member chain following Prettier's grouping algorithm.
///
/// Example: `Factory.create(x).build(y)` creates 3 groups:
/// - Group 0: [Identifier("Factory")]
/// - Group 1: [MemberExpression(.create), CallExpression(.create(x))]
/// - Group 2: [MemberExpression(.build), CallExpression(.build(y))]
///
/// See: prettier/src/language-js/print/member-chain.js (lines 190-259)
fn count_member_chain_groups(expr: &Expression) -> usize {
    let mut groups = 0;
    let mut current = expr;
    let mut has_seen_call_in_group = false;

    loop {
        match current {
            Expression::TSNonNullExpression(non_null) => {
                current = &non_null.expression;
            }
            Expression::CallExpression(call) => {
                if !has_seen_call_in_group {
                    // First call in this group
                    has_seen_call_in_group = true;
                }

                // Continue traversing through callee
                current = &call.callee;
            }
            Expression::MemberExpression(member) => {
                if has_seen_call_in_group {
                    // We've seen a call and now hit a member - new group
                    groups += 1;
                    has_seen_call_in_group = false;
                }

                // Continue traversing through object
                current = &member.object;
            }
            _ => {
                // Base of the chain (identifier, literal, etc.) - counts as a group
                groups += 1;

                // If we had a pending call group, count it
                if has_seen_call_in_group {
                    groups += 1;
                }

                break;
            }
        }
    }

    groups
}

/// Extract the base identifier name from a member chain.
fn get_base_identifier<'a>(expr: &Expression, source: &'a str) -> Option<&'a str> {
    let mut current = expr;

    loop {
        match current {
            Expression::TSNonNullExpression(non_null) => {
                current = &non_null.expression;
            }
            Expression::CallExpression(call) => {
                current = &call.callee;
            }
            Expression::MemberExpression(member) => {
                current = &member.object;
            }
            Expression::Identifier(ident) => {
                return Some(ident.span.extract(source));
            }
            _ => {
                return None;
            }
        }
    }
}

/// A chain is poorly breakable if it doesn't have good internal break points:
/// - Member-only chains: `a.b.c.d` (no calls to break on)
/// - Trivial call chains: `a.b().c()` (calls with no/simple args)
///
/// Corresponds to prettier's `isPoorlyBreakableMemberOrCallChain`
pub fn is_poorly_breakable_chain(expr: &Expression, source: &str, print_width: usize) -> bool {
    is_poorly_breakable_chain_recursive(expr, false, source, print_width)
}

fn is_poorly_breakable_chain_recursive(
    expr: &Expression,
    deep: bool,
    source: &str,
    print_width: usize,
) -> bool {
    match expr {
        // TSNonNullExpression: continue checking
        Expression::TSNonNullExpression(non_null) => {
            is_poorly_breakable_chain_recursive(&non_null.expression, deep, source, print_width)
        }

        // CallExpression: check if it's a trivial call
        Expression::CallExpression(call) => {
            // If this is a member chain with 2+ calls, it's NOT poorly breakable
            // Member chains handle their own breaking internally
            // Check this FIRST before checking if call is trivial
            if is_member_chain_with_multiple_calls(expr, source) {
                return false;
            }

            // Empty args or single short arg = trivial call
            // Note: identifiers are excluded from "short" - they break inside call
            let is_trivial_call = call.arguments.is_empty()
                || (call.arguments.len() == 1
                    && is_short_arg(&call.arguments[0], source, print_width));

            if !is_trivial_call {
                return false;
            }

            // Continue down the chain
            is_poorly_breakable_chain_recursive(&call.callee, true, source, print_width)
        }

        // MemberExpression: continue down the chain
        Expression::MemberExpression(member) => {
            is_poorly_breakable_chain_recursive(&member.object, true, source, print_width)
        }

        // Base cases: identifiers and `this` are valid chain roots
        Expression::Identifier(_) | Expression::Super(_) => deep,

        // Everything else breaks the chain
        _ => false,
    }
}

/// Check if an argument is "short" (won't expand when formatted)
///
/// Matches Prettier's `isLoneShortArgument` logic:
/// - Identifiers are short if name length <= threshold (printWidth * 0.25)
/// - String literals are short if formatted length <= threshold
/// - Template literals without expressions are short if raw length <= threshold and no newlines
/// - Other literals (numbers, booleans, null) are short
fn is_short_arg(expr: &Expression, source: &str, print_width: usize) -> bool {
    // Prettier's LONE_SHORT_ARGUMENT_THRESHOLD_RATE = 0.25
    // Threshold = printWidth * 0.25 (using integer division: printWidth / 4)
    let threshold = print_width / 4;

    match expr {
        // String literals: check formatted length against threshold
        Expression::Literal(lit) if matches!(lit.value, internal::LiteralValue::String { .. }) => {
            format_string_literal_from_ast(lit, source).len() <= threshold
        }
        // Template literals: short if no expressions, raw length <= threshold, and no newlines
        Expression::TemplateLiteral(template) => {
            template.expressions.is_empty()
                && !template.quasis.is_empty()
                && template.quasis[0].raw.len() <= threshold
                && !super::template_literal_has_newlines(template)
        }
        // Other literals (numbers, booleans, null, bigint) are short
        Expression::Literal(_) => true,
        // Identifiers: short if name length <= threshold
        Expression::Identifier(id) => id.span.extract(source).len() <= threshold,
        // `this` is short (can't break)
        Expression::Super(_) => true,
        // Everything else might be complex
        _ => false,
    }
}

/// Check if an expression is a simple value that shouldn't break
fn is_simple_value(expr: &Expression) -> bool {
    matches!(
        expr,
        Expression::Literal(lit) if matches!(
            lit.value,
            internal::LiteralValue::Boolean(_)
            | internal::LiteralValue::Number(_)
        )
    ) || matches!(expr, Expression::TemplateLiteral(_))
}

impl<'a> Printer<'a> {
    /// Build a Doc for an assignment (variable declaration or object property)
    ///
    /// This is the unified entry point that matches prettier's `printAssignment`.
    ///
    /// `is_short_key`: True for property keys shorter than `tabWidth + MIN_OVERLAP_FOR_BREAK`.
    /// For non-property assignments (e.g., `x = value`), pass `false`.
    pub fn build_assignment_layout(
        &self,
        left_doc: Doc,
        operator: &'static str,
        right_expr: &Expression,
        is_short_key: bool,
    ) -> Doc {
        let layout = choose_layout(
            right_expr,
            is_short_key,
            self.source,
            self.config.print_width,
        );
        let right_doc = self.build_expression_doc(right_expr);

        match layout {
            AssignmentLayout::BreakAfterOperator => {
                // Break after operator with nested groups - matches prettier exactly
                // Structure: group([group(left), op, group(indent([line, right]))])
                // Each inner group can break independently based on remaining width
                doc::group(doc::concat(vec![
                    doc::group(left_doc),
                    doc::text(operator),
                    doc::group(doc::indent(doc::concat(vec![doc::line(), right_doc]))),
                ]))
            }

            AssignmentLayout::NeverBreakAfterOperator => {
                // Never break after operator - matches prettier: group([group(left), op, " ", right])
                // Wrapping left_doc in a group allows right_doc's conditional_groups to expand independently
                // Structure: group([group(left), op, " ", right])
                doc::group(doc::concat(vec![
                    doc::group(left_doc),
                    doc::text(operator),
                    doc::text(" "),
                    right_doc,
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
                doc::group(doc::concat(vec![
                    doc::group(left_doc),
                    doc::text(operator),
                    doc::group_with_id(doc::indent(doc::line()), GroupId::Assignment),
                    doc::line_suffix_boundary(),
                    doc::indent_if_break(right_doc, GroupId::Assignment, false),
                ]))
            }
        }
    }
}
