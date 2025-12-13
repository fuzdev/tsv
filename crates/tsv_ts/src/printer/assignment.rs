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
use crate::ast::internal::{self, Expression};
use tsv_lang::doc::{self, Doc};

/// Assignment layout strategies (matches prettier's chooseLayout return values)
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

/// Context for layout selection
#[derive(Default)]
pub struct LayoutContext {
    /// True if the key is short (< tabWidth + 3)
    pub is_short_key: bool,
}

impl LayoutContext {
    /// Create context for an object property
    pub fn for_property(key_width: usize, tab_width: usize) -> Self {
        // Short key threshold: tabWidth + MIN_OVERLAP_FOR_BREAK (3)
        // Keys shorter than this don't benefit from line breaks
        let is_short_key = key_width < tab_width + 3;
        Self { is_short_key }
    }
}

/// Choose the layout strategy for an assignment
///
/// Follows prettier's `chooseLayout` logic in assignment.js
pub fn choose_layout(right_expr: &Expression, context: &LayoutContext) -> AssignmentLayout {
    // Objects and arrays handle their own expansion - never break between key and value
    // The value expands internally: `key: { ... }` not `key:\n{ ... }`
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

    // Short property keys → never break after operator
    // (wrapping object properties with very short keys usually doesn't add much value)
    if context.is_short_key {
        return AssignmentLayout::NeverBreakAfterOperator;
    }

    // Check if RHS is a poorly breakable chain (should break after operator)
    if should_break_after_operator(right_expr, context.is_short_key) {
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
fn is_self_expanding_value(expr: &Expression) -> bool {
    matches!(
        expr,
        Expression::ObjectExpression(_)
            | Expression::ArrayExpression(_)
            | Expression::FunctionExpression(_)
            | Expression::ArrowFunctionExpression(_)
            | Expression::ClassExpression(_)
    )
}

/// Check if we should break after the operator for this expression
///
/// Returns true for expressions that don't break well internally:
/// - Poorly breakable chains (member-only chains, trivial call chains)
/// - String literals (can't break internally)
fn should_break_after_operator(expr: &Expression, has_short_key: bool) -> bool {
    if has_short_key {
        return false;
    }

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
    is_poorly_breakable_chain(core_expr)
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

/// Check if an expression is a "poorly breakable" member or call chain
///
/// A chain is poorly breakable if it doesn't have good internal break points:
/// - Member-only chains: `a.b.c.d` (no calls to break on)
/// - Trivial call chains: `a.b().c()` (calls with no/simple args)
///
/// Corresponds to prettier's `isPoorlyBreakableMemberOrCallChain`
pub fn is_poorly_breakable_chain(expr: &Expression) -> bool {
    is_poorly_breakable_chain_recursive(expr, false)
}

fn is_poorly_breakable_chain_recursive(expr: &Expression, deep: bool) -> bool {
    match expr {
        // TSNonNullExpression: continue checking
        Expression::TSNonNullExpression(non_null) => {
            is_poorly_breakable_chain_recursive(&non_null.expression, deep)
        }

        // CallExpression: check if it's a trivial call
        Expression::CallExpression(call) => {
            // Empty args or single short arg = trivial call
            let is_trivial_call = call.arguments.is_empty()
                || (call.arguments.len() == 1 && is_short_arg(&call.arguments[0]));

            if !is_trivial_call {
                return false;
            }

            // Continue down the chain
            is_poorly_breakable_chain_recursive(&call.callee, true)
        }

        // MemberExpression: continue down the chain
        Expression::MemberExpression(member) => {
            is_poorly_breakable_chain_recursive(&member.object, true)
        }

        // Base cases: identifiers and `this` are valid chain roots
        Expression::Identifier(_) | Expression::Super(_) => deep,

        // Everything else breaks the chain
        _ => false,
    }
}

/// Check if an argument is "short" (won't expand when formatted)
fn is_short_arg(expr: &Expression) -> bool {
    match expr {
        // Literals are short
        Expression::Literal(_) => true,
        // Identifiers are short
        Expression::Identifier(_) => true,
        // `this` is short
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
    pub fn build_assignment_layout(
        &self,
        left_doc: Doc,
        operator: &'static str,
        right_expr: &Expression,
        context: LayoutContext,
    ) -> Doc {
        let layout = choose_layout(right_expr, &context);
        let right_doc = self.build_expression_doc(right_expr);

        match layout {
            AssignmentLayout::BreakAfterOperator => {
                // Break after operator, RHS indented
                // Structure: group([left, op, indent([line, right])])
                doc::group(doc::concat(vec![
                    left_doc,
                    doc::text(operator),
                    doc::indent_line(right_doc),
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
                // Structure: group([left, op, indent([line, right])])
                // Note: This is a simplified version; prettier uses indentIfBreak
                doc::group(doc::concat(vec![
                    left_doc,
                    doc::text(operator),
                    doc::indent_line(right_doc),
                ]))
            }
        }
    }
}
