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
use super::is_string_literal;
use crate::ast::internal::{self, Expression};
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
    // With the fits logic, calls can "fit" even when they break internally,
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

    // Curried arrow functions with return type → break after operator
    // Produces: `key:\n  (x: T): H =>\n  (y) =>\n    expr`
    if is_curried_arrow_with_return_type(right_expr) {
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
///
/// Note: Curried arrow functions with return type annotations are NOT self-expanding.
/// They need BreakAfterOperator to produce:
///   const f =
///       (x: T): H =>
///       (y) => ...
pub fn is_self_expanding_value(expr: &Expression) -> bool {
    match expr {
        Expression::ObjectExpression(_)
        | Expression::ArrayExpression(_)
        | Expression::FunctionExpression(_)
        | Expression::ClassExpression(_) => true,

        // Arrow functions are self-expanding UNLESS they're curried with return type
        Expression::ArrowFunctionExpression(_) => !is_curried_arrow_with_return_type(expr),

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
pub fn is_curried_arrow_with_return_type(expr: &Expression) -> bool {
    if let Expression::ArrowFunctionExpression(arrow) = expr {
        // Must be a curried arrow (body is another arrow)
        let is_curried = matches!(
            &arrow.body,
            internal::ArrowFunctionBody::Expression(body) if matches!(&**body, Expression::ArrowFunctionExpression(_))
        );

        if !is_curried {
            return false;
        }

        // Check if ANY arrow in the chain has return type (with params)
        arrow_chain_has_return_type(arrow)
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
pub fn arrow_chain_has_return_type(arrow: &internal::ArrowFunctionExpression) -> bool {
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
pub fn is_simple_self_expanding(expr: &Expression) -> bool {
    match expr {
        Expression::ArrayExpression(arr) => {
            if arr.elements.is_empty() {
                return true;
            }
            if arr.elements.len() > 1 {
                return false;
            }
            // Single element - check if it will expand
            arr.elements[0]
                .as_ref()
                .is_none_or(|e| !is_self_expanding_value(e))
        }
        Expression::ObjectExpression(obj) => {
            if obj.properties.is_empty() {
                return true;
            }
            if obj.properties.len() > 1 {
                return false;
            }
            // Single property - check if value will expand (for non-shorthand)
            match &obj.properties[0] {
                internal::ObjectProperty::Property(prop) => {
                    !prop.shorthand && !is_self_expanding_value(&prop.value)
                }
                internal::ObjectProperty::SpreadElement(_) => false,
            }
        }
        _ => false,
    }
}

/// Check if we should break after the operator for this expression
///
/// Returns true for expressions that don't break well internally:
/// - Poorly breakable chains (member-only chains, trivial call chains)
/// - String literals (can't break internally)
/// - Regex literals (can't break internally)
///
/// Precondition: Only called when is_short_key is false (checked in choose_layout)
fn should_break_after_operator(expr: &Expression, source: &str, print_width: usize) -> bool {
    // Unwrap wrapper expressions to get to the core
    let core_expr = unwrap_expression(expr);

    // String literals should break after operator
    if is_string_literal(core_expr) {
        return true;
    }

    // Regex literals can't break internally, so break after operator
    if matches!(core_expr, Expression::RegexLiteral(_)) {
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
        // TSNonNullExpression is transparent - continue checking
        Expression::TSNonNullExpression(non_null) => {
            is_poorly_breakable_chain_recursive(&non_null.expression, deep, source, print_width)
        }
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
            // Check if this call has trivial args (empty or single short arg)
            // Matches Prettier: args.length === 0 || (args.length === 1 && isLoneShortArgument)
            // Arrow functions, objects, arrays are NOT "lone short arguments" - they should
            // be allowed to break internally via the call's conditional_group states.
            let is_trivial_call = call.arguments.is_empty()
                || (call.arguments.len() == 1
                    && is_short_arg(&call.arguments[0], source, print_width));

            if !is_trivial_call {
                return false;
            }

            // Check if callee is a member chain that might be a factory pattern
            if !matches!(
                &*call.callee,
                Expression::MemberExpression(_) | Expression::TSNonNullExpression(_)
            ) {
                // Non-memberish callee (e.g., `fn()()`), continue down
                return is_poorly_breakable_chain_recursive(
                    &call.callee,
                    true,
                    source,
                    print_width,
                );
            }

            // Count calls in the chain
            let call_count = count_calls_in_chain(&call.callee) + 1; // +1 for this call

            // Single call with member access: obj.fn(arg) → poorly breakable
            // Continue checking to ensure it's a valid chain structure
            if call_count == 1 {
                return is_poorly_breakable_chain_recursive(
                    &call.callee,
                    true,
                    source,
                    print_width,
                );
            }

            // 2 calls: check if factory pattern
            if call_count == 2 {
                if is_factory_chain(&call.callee, source) {
                    // Factory pattern with 2 calls → break after operator
                    return true;
                }
                // Non-factory with 2 calls → let chain formatter handle it
                return false;
            }

            // More than 2 calls → let chain formatter handle it
            false
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
pub fn is_call_on_member_chain(expr: &Expression) -> bool {
    if let Expression::CallExpression(call) = expr {
        // The callee must be a member expression chain (possibly with non-null assertions)
        let is_member_chain = matches!(
            &*call.callee,
            Expression::MemberExpression(_) | Expression::TSNonNullExpression(_)
        ) && count_calls_in_chain(&call.callee) == 0;

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
pub fn is_single_call_on_member_chain(expr: &Expression) -> bool {
    if let Expression::CallExpression(call) = expr {
        matches!(
            &*call.callee,
            Expression::MemberExpression(_) | Expression::TSNonNullExpression(_)
        ) && count_calls_in_chain(&call.callee) == 0
    } else {
        false
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

/// Check if expression is a type assertion (`as` or `satisfies`) wrapping a call with long arguments.
///
/// Returns true when the expression is TSAsExpression/TSSatisfiesExpression wrapping a
/// CallExpression with non-trivial arguments (multiple args or single long arg).
///
/// Used for break-after-operator layout decisions: when a type assertion call has long args,
/// we break after `=` instead of inside the call. If the call has short/trivial args, the
/// type annotation can break instead.
pub fn is_type_assertion_call(expr: &Expression, source: &str, print_width: usize) -> bool {
    let call = match expr {
        Expression::TSAsExpression(as_expr) => match as_expr.expression.as_ref() {
            Expression::CallExpression(call) => call,
            _ => return false,
        },
        Expression::TSSatisfiesExpression(sat_expr) => match sat_expr.expression.as_ref() {
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
fn is_member_only_chain(expr: &Expression) -> bool {
    match expr {
        Expression::MemberExpression(member) => is_member_only_chain(&member.object),
        Expression::TSNonNullExpression(non_null) => is_member_only_chain(&non_null.expression),
        Expression::Identifier(_) | Expression::Super(_) => true,
        _ => false,
    }
}

/// Count calls in a chain expression.
fn count_calls_in_chain(expr: &Expression) -> usize {
    match expr {
        Expression::CallExpression(call) => 1 + count_calls_in_chain(&call.callee),
        Expression::MemberExpression(member) => count_calls_in_chain(&member.object),
        Expression::TSNonNullExpression(non_null) => count_calls_in_chain(&non_null.expression),
        _ => 0,
    }
}

/// Check if a chain starts with a factory pattern (capital letter or special prefixes).
///
/// Factory patterns include:
/// - Capital letter start: Object.keys, React.createElement, etc.
/// - Special prefixes: $_, $__ (lodash-style)
fn is_factory_chain(expr: &Expression, source: &str) -> bool {
    match expr {
        Expression::CallExpression(call) => is_factory_chain(&call.callee, source),
        Expression::MemberExpression(member) => is_factory_chain(&member.object, source),
        Expression::TSNonNullExpression(non_null) => is_factory_chain(&non_null.expression, source),
        Expression::Identifier(id) => {
            let name = id.span.extract(source);
            // Factory patterns: capital letter start OR $_ style (lodash)
            name.chars()
                .next()
                .is_some_and(|c| c.is_uppercase() || (c == '$' || c == '_'))
        }
        Expression::Super(_) => true,
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
        left_doc: DocId,
        operator: &'static str,
        right_expr: &Expression,
        is_short_key: bool,
    ) -> DocId {
        let d = self.d();
        let mut layout = choose_layout(
            right_expr,
            is_short_key,
            self.source,
            self.config.print_width,
        );

        // Override layout based on line comments in the RHS:
        //
        // For member-only chains with line comments, force BreakAfterOperator.
        // Line comments cause Prettier's first pass to break at `=`.
        //
        // For call chains with line comments, do NOT use BreakAfterOperator.
        // The chain formatter handles breaking at the comment location.
        // Use NeverBreakAfterOperator so the chain stays with `=` and breaks internally.
        if layout != AssignmentLayout::BreakAfterOperator
            && self.has_line_comments_in_member_chain(right_expr)
        {
            layout = AssignmentLayout::BreakAfterOperator;
        } else if layout == AssignmentLayout::BreakAfterOperator
            && matches!(right_expr, Expression::CallExpression(_))
            && self.has_line_comments_in_call_chain(right_expr)
        {
            layout = AssignmentLayout::NeverBreakAfterOperator;
        }

        let right_doc = self.build_expression_doc(right_expr);

        match layout {
            AssignmentLayout::BreakAfterOperator => {
                // Break after operator with nested groups - matches prettier exactly
                // Structure: group([group(left), op, group(indent([line, right]))])
                // Each inner group can break independently based on remaining width
                d.group(d.concat(&[
                    d.group(left_doc),
                    d.text(operator),
                    d.group(d.indent_line(right_doc)),
                ]))
            }

            AssignmentLayout::NeverBreakAfterOperator => {
                // Never break after operator - matches prettier: group([group(left), op, " ", right])
                // Wrapping left_doc in a group allows right_doc's conditional_groups to expand independently
                // Structure: group([group(left), op, " ", right])
                d.group(d.concat(&[d.group(left_doc), d.text(operator), d.text(" "), right_doc]))
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
                    d.group_with_id(d.indent(d.line()), GroupId::Assignment),
                    d.line_suffix_boundary(),
                    d.indent_if_break(right_doc, GroupId::Assignment, false),
                ]))
            }
        }
    }

    /// Check if an expression is a member-only chain with line comments.
    ///
    /// Member-only chains with line comments between segments should force
    /// BreakAfterOperator layout to match Prettier's first-pass behavior.
    fn has_line_comments_in_member_chain(&self, expr: &Expression) -> bool {
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
    pub(crate) fn has_line_comments_in_call_chain(&self, expr: &Expression) -> bool {
        self.has_line_comments_in_chain(expr)
    }

    /// Check if an expression contains an import expression with trailing comments.
    ///
    /// Import expressions with trailing comments (e.g., `import('./x' // comment)` or
    /// `import('./x' /* comment */)` or `import('./x', {opts} // comment)`)
    /// expand internally and should not use fluid layout. The import itself handles
    /// its own expansion, so the assignment should use default layout.
    /// Handles both direct imports and `await import(...)`.
    pub(crate) fn has_import_with_trailing_comments(&self, expr: &Expression) -> bool {
        match expr {
            Expression::ImportExpression(import) => {
                let paren_close = import.span.end;
                // Check for comments after the last argument (source or options)
                let last_arg_end = import
                    .options
                    .as_ref()
                    .map_or_else(|| import.source.span().end, |opts| opts.span().end);
                self.has_comments_between(last_arg_end, paren_close)
            }
            Expression::AwaitExpression(await_expr) => {
                self.has_import_with_trailing_comments(&await_expr.argument)
            }
            _ => false,
        }
    }

    /// Recursively check for line comments in a chain (calls, members, non-null).
    fn has_line_comments_in_chain(&self, expr: &Expression) -> bool {
        match expr {
            Expression::CallExpression(call) => self.has_line_comments_in_chain(&call.callee),
            Expression::MemberExpression(member) => {
                // Check for line comments between object and property
                let obj_end = member.object.span().end;
                let prop_start = member.property.span().start;
                if self.has_line_comments_between(obj_end, prop_start) {
                    return true;
                }
                self.has_line_comments_in_chain(&member.object)
            }
            Expression::TSNonNullExpression(non_null) => {
                self.has_line_comments_in_chain(&non_null.expression)
            }
            _ => false,
        }
    }
}
