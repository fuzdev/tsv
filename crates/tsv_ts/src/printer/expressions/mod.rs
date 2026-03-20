// Expression printing for TypeScript
//
// This module coordinates expression printing and delegates to specialized submodules:
// - literals.rs: Literals, identifiers, regex, spread, normalize helper
// - functions.rs: Arrow functions and function expressions
// - blocks.rs: Block statements (reusable utility)
// - patterns.rs: All destructuring patterns (object, array, assignment, rest)
// - ../objects.rs: Object expressions and property handling
// - ../arrays.rs: Array expressions
// - ../operators.rs: Unary and binary expressions
// - ../calls.rs: Call, member, and conditional expressions
//
// This module handles:
// - Expression dispatch (print_expression, build_expression_doc)
// - Template literals (both regular and tagged)

mod blocks;
mod functions;
mod literals;
mod patterns;

pub(super) use literals::{format_string_literal_from_ast, normalize_number_literal};

// Re-export for submodules to use `super::X` instead of `super::super::X`
use super::chain;
pub(super) use super::{
    ParenContext, PatternContext, Printer, needs_parens, object_pattern_should_expand,
    unwrap_parenthesized,
};
use crate::ast::internal::{BinaryExpression, BinaryOperator, Expression};
use tsv_lang::doc::arena::DocId;
use tsv_lang::printing::visual_width;

impl<'a> Printer<'a> {
    /// Print an expression using doc-based formatting
    pub fn print_expression(&mut self, expression: &Expression) {
        let doc = self.build_expression_doc(expression);
        self.write_arena_doc(doc);
    }

    /// Build a Doc for an expression (for use in object/array contexts and statements)
    pub(super) fn build_expression_doc(&self, expr: &Expression) -> DocId {
        let d = self.d();

        // Take and clear is_expression_statement so it doesn't leak to sub-expressions.
        // Only chain formatting needs this flag (for the isShort merge heuristic).
        // Re-set it only for expression types that enter chain formatting:
        // CallExpression, MemberExpression, TSNonNullExpression.
        let was_expr_stmt = self.is_expression_statement.replace(false);

        match expr {
            Expression::Literal(lit) => self.build_literal_doc(lit),
            Expression::Identifier(id) => self.build_identifier_doc(id),
            Expression::PrivateIdentifier(pid) => self.build_private_identifier_doc(pid),
            Expression::ObjectExpression(obj) => {
                // Consume flag BEFORE building — build_object_doc recurses into
                // property values which may contain nested ObjectExpressions that
                // would incorrectly consume the flag if we checked it after.
                let needs_arrow_parens = self.arrow_body_object_needs_parens.replace(false);
                let doc = self.build_object_doc(obj);
                if needs_arrow_parens {
                    let d = self.d();
                    d.concat(&[d.text("("), doc, d.text(")")])
                } else {
                    doc
                }
            }
            Expression::ArrayExpression(arr) => self.build_array_doc(arr),
            Expression::UnaryExpression(unary) => self.build_unary_doc(unary),
            Expression::UpdateExpression(update) => self.build_update_doc(update),
            Expression::BinaryExpression(binary) => self.build_binary_doc(binary),
            Expression::CallExpression(call) => {
                self.is_expression_statement.set(was_expr_stmt);
                self.build_call_doc(call)
            }
            Expression::NewExpression(new_expr) => self.build_new_doc(new_expr),
            Expression::MemberExpression(member) => {
                self.is_expression_statement.set(was_expr_stmt);
                self.build_member_doc(member)
            }
            Expression::ConditionalExpression(cond) => {
                self.build_conditional_doc_with_wrapping(cond)
            }
            Expression::ArrowFunctionExpression(arrow) => self.build_arrow_doc(arrow),
            Expression::FunctionExpression(func) => self.build_function_doc(func),
            Expression::ClassExpression(class_expr) => self.build_class_expression_doc(class_expr),
            Expression::SpreadElement(spread) => self.build_spread_doc(spread),
            Expression::TemplateLiteral(template) => self.build_template_literal_doc(template),
            Expression::TaggedTemplateExpression(tagged) => self.build_tagged_template_doc(tagged),
            Expression::AwaitExpression(await_expr) => self.build_await_doc(await_expr),
            Expression::YieldExpression(yield_expr) => self.build_yield_doc(yield_expr),
            Expression::SequenceExpression(seq) => self.build_sequence_doc(seq),
            Expression::RegexLiteral(regex) => self.build_regex_doc(regex),
            Expression::Super(_) => d.text("super"),
            Expression::AssignmentExpression(assign) => self.build_assignment_doc(assign),
            Expression::ObjectPattern(obj) => self.build_object_pattern_doc(obj),
            Expression::ArrayPattern(arr) => self.build_array_pattern_doc(arr),
            Expression::AssignmentPattern(pattern) => self.build_assignment_pattern_doc(pattern),
            Expression::RestElement(rest) => self.build_rest_element_doc(rest),
            Expression::TSTypeAssertion(type_assert) => {
                self.build_ts_type_assertion_doc(type_assert)
            }
            Expression::TSAsExpression(as_expr) => self.build_ts_as_doc(as_expr),
            Expression::TSSatisfiesExpression(sat_expr) => self.build_ts_satisfies_doc(sat_expr),
            Expression::TSInstantiationExpression(inst_expr) => {
                self.build_ts_instantiation_doc(inst_expr)
            }
            Expression::TSNonNullExpression(non_null_expr) => {
                self.is_expression_statement.set(was_expr_stmt);
                self.build_ts_non_null_doc(non_null_expr)
            }
            Expression::ImportExpression(import_expr) => {
                self.build_import_expression_doc(import_expr)
            }
            Expression::MetaProperty(meta) => self.build_meta_property_doc(meta),
            Expression::TSParameterProperty(param_prop) => {
                self.build_ts_parameter_property_doc(param_prop)
            }
        }
    }

    /// Build doc for function parameter expression, using FunctionParameter context for patterns
    pub(super) fn build_function_parameter_doc(&self, expr: &Expression) -> DocId {
        match expr {
            Expression::ObjectPattern(obj) => {
                self.build_object_pattern_doc_with_context(obj, PatternContext::FunctionParameter)
            }
            // For other expressions, use normal doc building
            _ => self.build_expression_doc(expr),
        }
    }

    /// Build doc for expression in call argument or array element context
    ///
    /// Binary/logical expressions get continuation indent when they break:
    /// ```text
    /// fn(
    ///     aaa &&
    ///         bbb,  // extra indent on continuation
    /// )
    /// ```
    ///
    /// Assignment expressions are wrapped in parens for clarity:
    /// `fn((a = b))` not `fn(a = b)`
    pub(super) fn build_arg_expression_doc(&self, expr: &Expression) -> DocId {
        let d = self.d();
        // Assignment expressions need parens in argument context for clarity
        if needs_parens(expr, ParenContext::Argument) {
            return d.parens(self.build_expression_doc(expr));
        }

        match expr {
            Expression::BinaryExpression(binary) => {
                // Use indented binary chain - continuation lines get extra indent
                self.build_binary_chain_doc_indented(binary)
            }
            Expression::ConditionalExpression(cond) => {
                // Ternary in call/new args: binary expressions in branches use
                // continuation indent. Matches Prettier's shouldNotIndent = false
                // when grandparent is CallExpression/NewExpression (binaryish.js:112).
                self.build_conditional_doc_with_binary_test_indent(cond)
            }
            // For other expressions, use normal doc building
            _ => self.build_expression_doc(expr),
        }
    }

    /// Build a Doc for an expression with forced expansion (hardlines).
    ///
    /// Used by chain arg formatting when we need the object/array to expand
    /// internally with hardlines so fits() can correctly measure the first line.
    /// For example, `.fn({prop})` should become `.fn({\n  prop,\n})` when expanded.
    pub(super) fn build_arg_expression_doc_expanded(&self, expr: &Expression) -> DocId {
        match expr {
            Expression::ObjectExpression(obj) => self.build_object_doc_expanded(obj),
            Expression::ArrayExpression(arr) => self.build_array_doc_expanded(arr),
            // For other expressions, use normal doc building
            _ => self.build_arg_expression_doc(expr),
        }
    }

    //
    // TypeScript Type Assertions
    //

    /// Build a Doc for a TypeScript angle-bracket type assertion: `<Type>expr`
    fn build_ts_type_assertion_doc(
        &self,
        type_assert: &crate::ast::internal::TSTypeAssertion,
    ) -> DocId {
        let d = self.d();
        let expr_needs_parens =
            needs_parens(&type_assert.expression, ParenContext::AngleBracketAssertion);
        let mut parts = vec![
            d.text("<"),
            self.build_type_doc_with_wrapping_type_args(&type_assert.type_annotation),
            d.text(">"),
        ];
        if expr_needs_parens {
            parts.push(d.text("("));
        }
        parts.push(self.build_expression_doc(&type_assert.expression));
        if expr_needs_parens {
            parts.push(d.text(")"));
        }
        d.concat(&parts)
    }

    /// Build a Doc for a TypeScript `as` expression
    ///
    /// Preserves comments between expression and `as` keyword (Prettier 3.7 #18161)
    /// Comments between `as` and type are moved to after the type (Prettier normalization)
    fn build_ts_as_doc(&self, as_expr: &crate::ast::internal::TSAsExpression) -> DocId {
        let d = self.d();
        let needs_parens = needs_parens(&as_expr.expression, ParenContext::TypeAssertion);
        let mut parts = Vec::new();
        if needs_parens {
            parts.push(d.text("("));
        }
        parts.push(self.build_expression_doc(&as_expr.expression));
        if needs_parens {
            parts.push(d.text(")"));
        }

        // Find the `as` keyword position
        let expr_end = as_expr.expression.span().end;
        let type_start = as_expr.type_annotation.span().start;
        let as_keyword_pos = self.find_keyword_in_range(expr_end, type_start, "as");

        // Comments between expression and `as` keyword → place before ` as`
        if let Some(as_pos) = as_keyword_pos {
            parts.push(self.build_inline_comments_between_doc(expr_end, as_pos));
        }

        parts.push(d.text(" as "));
        parts.push(self.build_type_doc_with_wrapping_type_args(&as_expr.type_annotation));

        // Comments between `as` keyword and type → place after the type
        if let Some(as_pos) = as_keyword_pos {
            let as_end = as_pos + 2; // "as" is 2 chars
            parts.push(self.build_inline_comments_between_doc(as_end, type_start));
        }

        d.concat(&parts)
    }

    /// Build a Doc for a TypeScript `satisfies` expression
    ///
    /// Preserves comments between expression and `satisfies` keyword (Prettier 3.7 #18162)
    fn build_ts_satisfies_doc(
        &self,
        sat_expr: &crate::ast::internal::TSSatisfiesExpression,
    ) -> DocId {
        let d = self.d();
        let needs_parens = needs_parens(&sat_expr.expression, ParenContext::TypeAssertion);
        let mut parts = Vec::new();
        if needs_parens {
            parts.push(d.text("("));
        }
        parts.push(self.build_expression_doc(&sat_expr.expression));
        if needs_parens {
            parts.push(d.text(")"));
        }

        // Include comments between expression and `satisfies` keyword
        let expr_end = sat_expr.expression.span().end;
        let type_start = sat_expr.type_annotation.span().start;
        parts.push(self.build_inline_comments_between_doc(expr_end, type_start));

        parts.push(d.text(" satisfies "));
        parts.push(self.build_type_doc_with_wrapping_type_args(&sat_expr.type_annotation));
        d.concat(&parts)
    }

    /// Build a Doc for a TypeScript instantiation expression
    fn build_ts_instantiation_doc(
        &self,
        inst_expr: &crate::ast::internal::TSInstantiationExpression,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        let needs_parens =
            needs_parens(&inst_expr.expression, ParenContext::InstantiationExpression);
        if needs_parens {
            parts.push(d.text("("));
        }
        parts.push(self.build_expression_doc(&inst_expr.expression));
        if needs_parens {
            parts.push(d.text(")"));
        }
        parts.push(self.build_type_parameter_instantiation_doc(&inst_expr.type_arguments));
        d.concat(&parts)
    }

    /// Build a Doc for a TypeScript non-null assertion expression
    ///
    /// When wrapping certain expressions in parens (binary, ternary, etc.),
    /// prettier indents continuations when the expression breaks:
    /// ```text
    /// (veryLongExpr ||
    ///     continuation)!
    /// ```
    fn build_ts_non_null_doc(
        &self,
        non_null_expr: &crate::ast::internal::TSNonNullExpression,
    ) -> DocId {
        let d = self.d();
        let needs_parens = needs_parens(&non_null_expr.expression, ParenContext::NonNull);

        if needs_parens {
            // For expressions that need parens, use a special doc structure
            // that indents continuations when breaking
            let inner_doc =
                self.build_expression_doc_with_indent_on_break(&non_null_expr.expression);
            d.concat(&[d.text("("), inner_doc, d.text(")!")])
        } else if Self::is_chain_expression(&non_null_expr.expression) {
            // When inner expression is a chain (member or call), use chain architecture
            // to properly handle breaking. This ensures the outer `!` is included
            // in the linearized chain for proper segment grouping.
            let expr = Expression::TSNonNullExpression(non_null_expr.clone());
            let nodes = chain::linearize_chain(&expr);
            let groups = chain::group_chain_nodes(nodes);
            chain::build_chain_doc(&groups, self)
        } else {
            let inner_doc = self.build_expression_doc(&non_null_expr.expression);
            d.concat(&[inner_doc, d.text("!")])
        }
    }

    /// Check if an expression is part of a chain (member, call, or non-null)
    fn is_chain_expression(expr: &Expression) -> bool {
        matches!(
            expr,
            Expression::MemberExpression(_)
                | Expression::CallExpression(_)
                | Expression::TSNonNullExpression(_)
        )
    }

    /// Build expression doc with indentation added to line breaks
    /// Used when expression is inside inline parens like `(expr)!`
    pub(crate) fn build_expression_doc_with_indent_on_break(&self, expr: &Expression) -> DocId {
        match expr {
            Expression::BinaryExpression(binary) => {
                // Build binary chain with indented continuations
                self.build_binary_chain_doc_indented(binary)
            }
            _ => self.build_expression_doc(expr),
        }
    }

    /// Build binary chain doc with indented continuations
    /// Used when the binary expression is inside inline parens
    fn build_binary_chain_doc_indented(&self, binary: &BinaryExpression) -> DocId {
        let d = self.d();
        d.group(self.build_binary_chain_parts_indented(binary))
    }

    /// Build binary chain parts with indented continuations (no group wrapper)
    ///
    /// Returns the concat without a group wrapper, for cases where the caller
    /// wants to control the grouping (e.g., chain printing).
    pub(crate) fn build_binary_chain_parts_indented(&self, binary: &BinaryExpression) -> DocId {
        let d = self.d();
        // If there are comments within the binary expression, use the comment-aware
        // implementation from operators.rs which preserves comments and their line breaks.
        // This handles cases like: fn(a && // comment\n    b)
        if self.has_comments_between(binary.span.start, binary.span.end) {
            // Use the parts version (no group wrapper) since our caller controls grouping
            return self.build_binary_chain_parts_with_continuation_indent(binary);
        }

        // Collect all operands and operators in the chain
        let mut operands = Vec::new();
        let mut operators = Vec::new();
        self.collect_binary_operands_for_indent(binary, &mut operands, &mut operators);

        if operands.len() <= 1 {
            // Fallback to regular expression doc
            return self.build_binary_doc(binary);
        }

        // Build with indented continuations for chains:
        // "first +
        //     second -
        //     third"
        //
        // When shouldGroup is true (operand types differ from current node type,
        // e.g., `(LogicalExpr) + (ConditionalExpr)`), wrap each continuation in
        // its own sub-group so it can independently evaluate whether it fits on
        // the current line when the outer group breaks. This matches Prettier's
        // binaryish.js where shouldGroup controls whether `right` gets a group.
        //
        // When shouldGroup is false (operands are same AST type category, e.g.,
        // `(BinaryExpr) * 100`), all continuations break together with the outer
        // group. This matches Prettier's behavior for same-type chains.
        let should_group = Self::should_group_binary_continuation(binary);
        // shouldInlineLogicalExpression: when the outermost logical has a non-empty
        // object/array on the right, keep operator and RHS on the same line.
        // Prettier ref: binaryish.js:275, 361
        let should_inline_last = super::assignment::should_inline_logical_expression(binary);
        let mut parts = Vec::new();

        for (i, operand) in operands.iter().enumerate() {
            let is_last = i == operands.len() - 1;
            if i == 0 {
                parts.push(*operand);
            } else if is_last && should_inline_last {
                // shouldInlineLogicalExpression: keep operator and object/array on same line
                // Use indent with space (no line break) instead of indent_line.
                // For 2-operand chains: prettier returns group(parts) with no indent
                //   (shouldInline && !samePrecedence → flat). We skip indent.
                // For 3+ operand chains: prettier uses indent(rest) which applies to all
                //   continuation operands. We need indent to match the level.
                // Prettier ref: binaryish.js:275-280, 131, 169-178
                let is_chained = operands.len() > 2;
                let op_and_operand = if is_chained {
                    // In a chain, use indent (matches other continuations' indent level)
                    // but space instead of line (keeps operator and object on same line)
                    d.concat(&[
                        d.text(" "),
                        d.text(operators[i - 1].as_str()),
                        d.indent(d.concat(&[d.text(" "), *operand])),
                    ])
                } else {
                    // 2-operand: flat, no indent (prettier returns group(parts) directly)
                    d.concat(&[
                        d.text(" "),
                        d.text(operators[i - 1].as_str()),
                        d.text(" "),
                        *operand,
                    ])
                };
                if should_group {
                    parts.push(d.group(op_and_operand));
                } else {
                    parts.push(op_and_operand);
                }
            } else if should_group {
                // Sub-group for independent fitting
                parts.push(d.group(d.concat(&[
                    d.text(" "),
                    d.text(operators[i - 1].as_str()),
                    d.indent_line(*operand),
                ])));
            } else {
                parts.push(d.text(" "));
                parts.push(d.text(operators[i - 1].as_str()));
                parts.push(d.indent_line(*operand));
            }
        }

        d.concat(&parts)
    }

    /// Collect operands and operators from a binary chain (helper for indented version)
    ///
    /// Uses `can_flatten_with()` to determine which operators can be chained together.
    /// Flattens both left and right sides when operators are compatible.
    fn collect_binary_operands_for_indent(
        &self,
        expr: &BinaryExpression,
        operands: &mut Vec<DocId>,
        operators: &mut Vec<BinaryOperator>,
    ) {
        // Recursively flatten left side if it can be chained with current operator
        if let Expression::BinaryExpression(left_binary) = &*expr.left {
            if expr.operator.can_flatten_with(left_binary.operator) {
                self.collect_binary_operands_for_indent(left_binary, operands, operators);
            } else {
                // Can't flatten - build operand with parens if needed
                operands.push(self.build_binary_operand_doc(&expr.left, expr.operator, false));
            }
        } else {
            operands.push(self.build_binary_operand_doc(&expr.left, expr.operator, false));
        }

        // Add current operator
        operators.push(expr.operator);

        // Also flatten right side for truly associative operators (removes redundant parens)
        // Only logical operators are truly associative; arithmetic preserves right-side parens
        if let Expression::BinaryExpression(right_binary) = &*expr.right
            && expr.operator.can_flatten_with(right_binary.operator)
            && expr.operator.is_logical()
            && right_binary.operator.is_logical()
        {
            self.collect_binary_operands_for_indent(right_binary, operands, operators);
            return;
        }

        // Right operand can't be flattened - add as-is
        operands.push(self.build_binary_operand_doc(&expr.right, expr.operator, true));
    }

    /// Build binary chain specifically for parenthesized context in chain printing
    ///
    /// Structure: operand1 " /", line, operand2 " /", line, operand3
    /// In flat: `a / b / c`
    /// In break: `a /\nb /\nc` (with outer indent providing indentation)
    pub(crate) fn build_binary_chain_for_parens(&self, binary: &BinaryExpression) -> DocId {
        let d = self.d();
        // Collect all operands and operators in the chain
        let mut operands = Vec::new();
        let mut operators = Vec::new();
        self.collect_binary_operands_for_indent(binary, &mut operands, &mut operators);

        if operands.len() <= 1 {
            // Fallback to regular expression doc
            return self.build_binary_doc(binary);
        }

        // For 2-operand non-logical chains, wrap in a group with line() so the
        // binary can independently decide whether to break at the operator.
        // The group stays flat when the operands fit; when they don't, line()
        // fires and breaks at the operator (e.g., `left +\nright`), preventing
        // the operands' internal break points (like member chain dots) from
        // firing instead.
        if operands.len() == 2 && !operators[0].is_logical() {
            return d.group(d.concat(&[
                operands[0],
                d.text(" "),
                d.text(operators[0].as_str()),
                d.line(),
                operands[1],
            ]));
        }

        // For 3+ operand chains, use line breaks between operands:
        // operand1 " /", line, operand2 " /", line, operand3
        let mut parts = Vec::new();

        for (i, operand) in operands.iter().enumerate() {
            if i == 0 {
                // First operand
                parts.push(*operand);
            } else {
                // Subsequent operands: line break then operand
                parts.push(d.line()); // space in flat, newline in break
                parts.push(*operand);
            }

            // Add operator after operand (except for last)
            if i < operators.len() {
                parts.push(d.text(" "));
                parts.push(d.text(operators[i].as_str()));
            }
        }

        d.concat(&parts)
    }

    //
    // Template Literals
    //

    /// Build a Doc for a template literal.
    ///
    /// Based on Prettier's two-phase approach (template-literal.js), with
    /// divergences for width enforcement and user intent preservation:
    /// 1. Render each `${}` expression at infinite width
    /// 2. If NO newlines → wrap with softline (divergence: Prettier uses atomic text)
    /// 3. If HAS newlines → qualifying types get softline wrap, others keep doc as-is
    /// 4. Wrap in group; use group_break when user wrote `${\n expr \n}`
    fn build_template_literal_doc(
        &self,
        template: &crate::ast::internal::TemplateLiteral,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        parts.push(d.line_suffix_boundary());
        parts.push(d.text("`"));

        let mut previous_quasi_indent_size: usize = 0;
        // Track whether any quasi before the current interpolation contains a newline.
        // When true, literalline has been emitted (resetting output column to 0) but the
        // indent level on the command stack still carries the code context. We use align(0)
        // to reset the indent to absolute 0. When false, the template is inline and the
        // code context indent is correct — no reset needed.
        let mut any_quasi_had_newline = false;

        for (i, quasi) in template.quasis.iter().enumerate() {
            // Template content — split at newlines and join with literalline
            // (matches Prettier's replaceEndOfLine(node.value.raw) for TemplateElement).
            // Using literalline makes will_break() propagate correctly through
            // containing groups, so chains/calls break when they contain multiline templates.
            parts.push(self.replace_end_of_line(&quasi.raw));

            // Interpolation
            if i < template.expressions.len() {
                let expr = &template.expressions[i];
                let next_quasi = &template.quasis[i + 1];

                // Calculate indent size from quasi text (Prettier's getIndentSize)
                let text = &quasi.raw;
                let quasi_has_newline = text.contains('\n');
                any_quasi_had_newline = any_quasi_had_newline || quasi_has_newline;
                let indent_size = if quasi_has_newline {
                    self.get_template_indent_size(text)
                } else {
                    previous_quasi_indent_size
                };
                previous_quasi_indent_size = indent_size;

                // Build expression doc
                let prev_in_template = self.in_template_interpolation.get();
                self.in_template_interpolation.set(true);
                let expr_doc = if needs_parens(expr, ParenContext::TemplateLiteralExpression) {
                    d.parens(self.build_expression_doc(expr))
                } else {
                    self.build_expression_doc(expr)
                };
                self.in_template_interpolation.set(prev_in_template);

                // Collect comments in the interpolation region
                let leading_comments: Vec<_> =
                    tsv_lang::comments_in_range(self.comments, quasi.span.end, expr.span().start)
                        .collect();
                let trailing_comments: Vec<_> = tsv_lang::comments_in_range(
                    self.comments,
                    expr.span().end,
                    next_quasi.span.start,
                )
                .collect();

                // Check if any comments are line comments (non-block)
                let has_line_comment = leading_comments.iter().any(|c| !c.is_block)
                    || trailing_comments.iter().any(|c| !c.is_block);

                // Build comment docs
                let leading_comments_doc =
                    self.build_template_interpolation_comments(&leading_comments, true);
                let trailing_comments_doc =
                    self.build_template_interpolation_comments(&trailing_comments, false);

                // Combine expression with comments
                let full_expr_doc = if has_line_comment {
                    self.build_template_comments_and_expr_doc(
                        &leading_comments,
                        expr_doc,
                        &trailing_comments,
                    )
                } else {
                    d.concat(&[leading_comments_doc, expr_doc, trailing_comments_doc])
                };

                // Detect if user deliberately wrapped ${...} across lines by
                // checking for newlines BETWEEN ${ and the expression start, or
                // between the expression end and }. This distinguishes:
                //   `${  \n  expr  \n  }` → user wrapped (preserve with group_break)
                //   `${obj\n.method()}` → expression-internal newline (don't force break)
                let interp_start = quasi.span.end as usize + 2; // skip "${"
                let interp_end = next_quasi.span.start as usize - 1; // before "}"
                let expr_start = expr.span().start as usize;
                let expr_end = expr.span().end as usize;
                let before_expr = self.source.get(interp_start..expr_start).unwrap_or("");
                let after_expr = self.source.get(expr_end..interp_end).unwrap_or("");
                let user_wrapped_expression =
                    before_expr.contains('\n') || after_expr.contains('\n');

                // Check whether expression doc has structural newlines (hardlines)
                // by rendering at infinite width. Softlines/lines become spaces;
                // only hardlines (from block bodies, literalline in templates, etc.)
                // produce newlines. This matches Prettier's two-phase approach.
                let rendered = self.render_arena_doc_flat(full_expr_doc);
                let interpolation_has_newline = rendered.contains('\n');

                let has_comments = !leading_comments.is_empty() || !trailing_comments.is_empty();

                let expression_doc = if !interpolation_has_newline {
                    // No structural newlines: wrap ALL types with softline.
                    // The group breaks when the line exceeds print_width.
                    // Diverges from Prettier (which uses atomic text here) —
                    // we keep doc structure so ${/} can break to respect print_width.
                    d.concat(&[
                        d.indent(d.concat(&[d.softline(), full_expr_doc])),
                        d.softline(),
                    ])
                } else if Self::is_template_softline_expression(expr, has_comments)
                    || user_wrapped_expression
                {
                    // Structural newlines + qualifying type or user-wrapped:
                    // wrap with softline. Qualifying types match Prettier.
                    // User-wrapped preserves intent.
                    d.concat(&[
                        d.indent(d.concat(&[d.softline(), full_expr_doc])),
                        d.softline(),
                    ])
                } else {
                    // Structural newlines + non-qualifying type (chains, calls,
                    // arrows with block bodies): keep doc as-is.
                    // Chain starts inline after ${. Matches Prettier.
                    full_expr_doc
                };

                // Apply alignment based on quasi indent (Prettier's addAlignmentToDoc).
                // Only when a preceding quasi had a newline (literalline was emitted),
                // because that resets output column to 0 but leaves the command stack's
                // indent at the code context level. align(0) inside add_alignment_to_doc
                // resets to absolute 0, then indent^n positions at the template indent.
                // For inline templates (no newline in any quasi), skip — code context is correct.
                let aligned = if any_quasi_had_newline {
                    self.add_alignment_to_doc(expression_doc, indent_size)
                } else {
                    expression_doc
                };

                // Wrap in group: group(["${", aligned, lineSuffixBoundary, "}"])
                // Use group_break when user wrapped the expression across lines —
                // preserves their formatting choice (unlike Prettier which collapses).
                let group_doc =
                    d.concat(&[d.text("${"), aligned, d.line_suffix_boundary(), d.text("}")]);
                parts.push(if user_wrapped_expression {
                    d.group_break(group_doc)
                } else {
                    d.group(group_doc)
                });
            }
        }

        parts.push(d.text("`"));
        d.concat(&parts)
    }

    /// Get the visual indent size of the last line in template quasi text.
    /// Equivalent to Prettier's `getIndentSize`.
    fn get_template_indent_size(&self, text: &str) -> usize {
        if let Some(last_nl) = text.rfind('\n') {
            let after_nl = &text[last_nl + 1..];
            let ws_end = after_nl
                .chars()
                .take_while(|c| *c == '\t' || *c == ' ')
                .count();
            visual_width(&after_nl[..ws_end], self.config.tab_width)
        } else {
            0
        }
    }

    /// Apply alignment to a doc based on indent size.
    /// Equivalent to Prettier's `addAlignmentToDoc(doc, size, tabWidth)`.
    ///
    /// Always wraps with `align(0)` to reset indent to absolute 0,
    /// then applies indent levels from zero. This is critical because
    /// after `literalline` in template content, the output column resets
    /// to 0 but the indent level on the command stack still carries the
    /// code context. Without the reset, softlines would break at the
    /// inherited code indent instead of the template's visual position.
    fn add_alignment_to_doc(&self, doc: DocId, size: usize) -> DocId {
        let d = self.d();
        let tab_width = self.config.tab_width;
        let n = size / tab_width;
        let r = size - n * tab_width;
        let mut result = doc;
        for _ in 0..n {
            result = d.indent(result);
        }
        if r > 0 {
            result = d.align_spaces(r, result);
        }
        // Reset to absolute indent 0. Uses align(0) not dedent because
        // dedent only decrements by 1 (saturating_sub), while we need
        // to reset to 0 regardless of the current indent depth.
        d.align(0, result)
    }

    /// Convert a string with newlines into a doc with literalline between parts.
    /// Equivalent to Prettier's `replaceEndOfLine(text)` for TemplateElement nodes.
    fn replace_end_of_line(&self, text: &str) -> DocId {
        let d = self.d();
        if !text.contains('\n') {
            return d.text_owned(text.to_string());
        }
        let mut doc_parts = Vec::new();
        for (i, part) in text.split('\n').enumerate() {
            if i > 0 {
                doc_parts.push(d.literalline());
            }
            if !part.is_empty() {
                doc_parts.push(d.text_owned(part.to_string()));
            }
        }
        d.concat(&doc_parts)
    }

    /// Whether this expression type qualifies for softline wrapping when it
    /// contains structural newlines (hardlines). Matches Prettier's handling
    /// of "simple" template expressions.
    ///
    /// Qualifying types: their docs don't inherently produce hardlines from
    /// block structure. Any hardlines come from nested content (e.g., a
    /// ternary whose branch contains a multiline template).
    ///
    /// Non-qualifying types (CallExpression, ArrowFunctionExpression, etc.):
    /// their hardlines come from block bodies / argument lists. Softline
    /// wrapping would force the ${/} group to break unnecessarily.
    fn is_template_softline_expression(expr: &Expression, has_comments: bool) -> bool {
        if has_comments {
            return true;
        }
        matches!(
            expr,
            Expression::Identifier(_)
                | Expression::Literal(_)
                | Expression::MemberExpression(_)
                | Expression::ConditionalExpression(_)
                | Expression::TemplateLiteral(_)
                | Expression::UnaryExpression(_)
                | Expression::UpdateExpression(_)
                | Expression::MetaProperty(_)
                | Expression::Super(_)
        )
    }

    /// Build comments doc for template literal interpolations
    ///
    /// Line comments get a hardline after them (required since `//` extends to EOL).
    /// Block comments get a space after (leading) or before (trailing).
    fn build_template_interpolation_comments(
        &self,
        comments: &[&crate::ast::internal::Comment],
        is_leading: bool,
    ) -> DocId {
        let d = self.d();
        if comments.is_empty() {
            return d.empty();
        }

        let mut parts = Vec::new();
        for comment in comments {
            if is_leading {
                // Leading comments: comment then separator
                parts.push(self.build_comment_doc(comment));
                if comment.is_block {
                    parts.push(d.text(" "));
                } else {
                    // Line comment requires hardline after
                    parts.push(d.hardline());
                }
            } else {
                // Trailing comments: space then comment
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
                // Line comments at the end still need hardline for proper formatting
                if !comment.is_block {
                    parts.push(d.hardline());
                }
            }
        }
        d.concat(&parts)
    }

    /// Build comments and expression doc for template interpolation with line comments.
    ///
    /// Uses `hardline` after line comments so the enclosing `indent()` wrapper handles indentation.
    /// Does NOT add hardline after the last trailing comment since the closing `}` literalline
    /// will provide that newline.
    fn build_template_comments_and_expr_doc(
        &self,
        leading_comments: &[&crate::ast::internal::Comment],
        expr_doc: DocId,
        trailing_comments: &[&crate::ast::internal::Comment],
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // Leading comments
        for comment in leading_comments {
            parts.push(self.build_comment_doc(comment));
            if comment.is_block {
                parts.push(d.text(" "));
            } else {
                parts.push(d.hardline());
            }
        }

        // Expression
        parts.push(expr_doc);

        // Trailing comments - don't add hardline after the last one since the
        // closing `}` has its own literalline that provides the newline
        let last_idx = trailing_comments.len().saturating_sub(1);
        for (i, comment) in trailing_comments.iter().enumerate() {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
            if !comment.is_block && i < last_idx {
                parts.push(d.hardline());
            }
        }

        d.concat(&parts)
    }

    /// Build a Doc for a tagged template expression
    fn build_tagged_template_doc(
        &self,
        tagged: &crate::ast::internal::TaggedTemplateExpression,
    ) -> DocId {
        let d = self.d();
        let tag_doc = self.build_expression_doc(&tagged.tag);

        // Wrap tag in parens if needed (e.g., ternary: `(a ? b : c)`template``)
        // This must happen BEFORE adding removed-paren comments so comments stay outside
        let tag_doc = if needs_parens(&tagged.tag, ParenContext::Callee) {
            d.parens(tag_doc)
        } else {
            tag_doc
        };

        // Check for comments between removed parentheses and tag
        // e.g., (/* comment */ tag)`template` has tagged.span.start at '(' and tag.span.start at 'tag'
        let tag_doc = self.prepend_removed_paren_comments(
            tagged.span.start,
            tagged.tag.span().start,
            tag_doc,
        );

        d.concat(&[tag_doc, self.build_template_literal_doc(&tagged.quasi)])
    }

    /// Build a Doc for a TypeScript parameter property
    fn build_ts_parameter_property_doc(
        &self,
        param_prop: &crate::ast::internal::TSParameterProperty,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // Print accessibility modifier
        if let Some(acc) = &param_prop.accessibility {
            parts.push(d.text(acc.as_str()));
            parts.push(d.text(" "));
        }

        // Print readonly modifier
        if param_prop.readonly {
            parts.push(d.text("readonly "));
        }

        // Print the parameter (identifier or assignment pattern)
        parts.push(self.build_expression_doc(&param_prop.parameter));

        d.concat(&parts)
    }
}
