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
        match expr {
            Expression::Literal(lit) => self.build_literal_doc(lit),
            Expression::Identifier(id) => self.build_identifier_doc(id),
            Expression::PrivateIdentifier(pid) => self.build_private_identifier_doc(pid),
            Expression::ObjectExpression(obj) => self.build_object_doc(obj),
            Expression::ArrayExpression(arr) => self.build_array_doc(arr),
            Expression::UnaryExpression(unary) => self.build_unary_doc(unary),
            Expression::UpdateExpression(update) => self.build_update_doc(update),
            Expression::BinaryExpression(binary) => self.build_binary_doc(binary),
            Expression::CallExpression(call) => self.build_call_doc(call),
            Expression::NewExpression(new_expr) => self.build_new_doc(new_expr),
            Expression::MemberExpression(member) => self.build_member_doc(member),
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
        let mut parts = Vec::new();

        for (i, operand) in operands.iter().enumerate() {
            if i == 0 {
                parts.push(*operand);
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

        // For 2-operand non-logical chains, use flat formatting (no line breaks)
        // to avoid ugly breaks like `(a /\nb)`. Logical operators can still break.
        if operands.len() == 2 && !operators[0].is_logical() {
            return d.concat(&[
                operands[0],
                d.text(" "),
                d.text(operators[0].as_str()),
                d.text(" "),
                operands[1],
            ]);
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

    /// Build a Doc for a template literal
    ///
    /// Formatting strategy: preserve user intent for newlines
    /// 1. If user wrote `${\n...`: preserve it (break after `${`)
    /// 2. Else if compact `${...}` would overflow: expand to `${\n...\n}`
    /// 3. Else: keep inline
    ///
    /// Divergence from Prettier:
    /// - We preserve `${\n...}` where Prettier might collapse (chains, calls, arrays)
    /// - We expand compact overflow where Prettier keeps inline
    ///
    /// Both add structure rather than remove it.
    fn build_template_literal_doc(
        &self,
        template: &crate::ast::internal::TemplateLiteral,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        parts.push(d.text("`"));

        for (i, quasi) in template.quasis.iter().enumerate() {
            // Template content (raw, preserving escape sequences verbatim)
            parts.push(d.text_owned(quasi.raw.clone()));

            // Interpolation
            if i < template.expressions.len() {
                let expr = &template.expressions[i];

                // Get the next quasi to determine trailing comment region
                let next_quasi = &template.quasis[i + 1];

                // Extract line-start whitespace from quasi for template-aware indentation.
                // If the quasi contains '\n', find whitespace after the last newline.
                // This whitespace represents the visual column where the line starts in the template.
                let line_start_whitespace = Self::extract_line_start_whitespace(&quasi.raw);

                // Build expression doc (non-flat, can break at method calls)
                // Set in_template_interpolation to collapse blank lines
                let prev_in_template = self.in_template_interpolation.get();
                self.in_template_interpolation.set(true);
                // Assignment expressions need parens in template literals: `${(a = b)}`
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
                // Line comments require a newline after them, so flat layout is invalid
                let has_line_comment = leading_comments.iter().any(|c| !c.is_block)
                    || trailing_comments.iter().any(|c| !c.is_block);

                // Check if user wrote ${\n (newline after ${) - preserve user intent
                // Source between quasi.span.end and expr.span().start includes `${` + whitespace/comments
                let interpolation_prefix = self
                    .source
                    .get(quasi.span.end as usize..expr.span().start as usize)
                    .unwrap_or("");
                let user_wrote_newline = interpolation_prefix.contains('\n');

                // Build leading comments doc with proper line breaks after line comments
                let leading_comments_doc = self.build_template_interpolation_comments(
                    &leading_comments,
                    true, // is_leading
                );

                // Build trailing comments doc with space before each
                let trailing_comments_doc = self.build_template_interpolation_comments(
                    &trailing_comments,
                    false, // is_leading
                );

                // Get expression source for width calculation and break detection
                let expr_source = self
                    .source
                    .get(expr.span().start as usize..expr.span().end as usize)
                    .unwrap_or("");
                let has_internal_breaks = expr_source.contains('\n');

                // Determine if we should break after ${:
                // 1. User wrote ${\n - preserve user intent (never collapse)
                // 2. Line comments present - force multiline
                // 3. First line would overflow - break to respect print width
                //
                // Even for expressions with internal breaks, we check if the FIRST line
                // (up to the first internal newline) would overflow.
                let should_break = if user_wrote_newline || has_line_comment {
                    true
                } else {
                    // Check if first line would overflow
                    let (prefix_width, has_newline_before) =
                        self.accumulated_template_prefix_width(template, i);

                    // For expressions with internal breaks, measure only up to first newline
                    let first_line_len = if has_internal_breaks {
                        expr_source.find('\n').unwrap_or(expr_source.len())
                    } else {
                        expr_source.len()
                    };

                    let suffix_width = self.estimate_template_suffix_width(&next_quasi.raw);

                    // Core width: prefix + "${" + first_line + "}" (if no internal breaks) + suffix
                    // For internal breaks, don't add "}" since it's not on the first line
                    let is_last_interpolation = i == template.expressions.len() - 1;
                    let is_same_line_ending =
                        is_last_interpolation && !next_quasi.raw.contains('\n');
                    // Backtick width (1) if template ends on same line
                    let closing_backtick = if is_same_line_ending { 1 } else { 0 };
                    // Trailing content after template (";", ");", etc.) - only needed for
                    // template-internal lines where ${} starts at column 0 after a newline.
                    // For inline templates, this is already in statement_buffer.
                    let trailing_after_template = if is_same_line_ending {
                        let after_template = &self.source[template.span.end as usize..];
                        let line_end = after_template.find('\n').unwrap_or(after_template.len());
                        after_template[..line_end].len()
                    } else {
                        0
                    };
                    let interp_width = if has_internal_breaks {
                        prefix_width + 2 + first_line_len
                    } else {
                        prefix_width + 2 + first_line_len + 1 + suffix_width + closing_backtick
                    };

                    // Add context based on template structure:
                    // - Newline before: template-internal line, add trailing content width
                    // - No newlines: inline template, trailing already in statement_buffer
                    let total_width = if has_newline_before {
                        interp_width + trailing_after_template
                    } else {
                        // For inline templates, estimate the output column position.
                        // Use the expected output indentation level, not source position,
                        // because the source might be compacted (very long lines).
                        let base_indent = self
                            .estimate_inline_template_output_indent(template.span.start as usize);
                        // Add buffers for context that appears before the template.
                        // Buffer amounts are tuned to distinguish 100-char (inline)
                        // from 101-char (break) cases.
                        let (_, is_expression_context, _) =
                            self.analyze_template_context(template.span.start as usize);
                        // Check context type from preceding characters
                        let before_pos = &self.source[..template.span.start as usize];
                        let trimmed = before_pos.trim_end();
                        let last_char = trimmed.chars().last();
                        let is_ternary_context = matches!(last_char, Some(':' | '?'));
                        let has_closing_delimiter = matches!(last_char, Some('(' | '[' | '{'));
                        let is_arrow_body = trimmed.ends_with("=>");

                        // Check if template starts on its own line (after newline + whitespace only)
                        let template_on_own_line = before_pos.rfind('\n').is_some_and(|nl_pos| {
                            before_pos[nl_pos + 1..]
                                .chars()
                                .all(|c| c == ' ' || c == '\t')
                        });

                        // base_indent is in indent LEVELS, not visual chars.
                        // When template is on its own line, only visual indent matters.
                        // Context-specific handling applies only for inline templates.
                        let (statement_buffer, closing, expr_prefix, use_visual) =
                            if template_on_own_line {
                                // Template on own line: just visual indent + trailing content (;)
                                (0, trailing_after_template, 0, true)
                            } else if has_closing_delimiter {
                                // Inline after bracket: minimal buffer
                                (0, 0, 3, true)
                            } else if is_ternary_context {
                                // Ternary arms: ": " prefix + indent adjustment
                                (0, 0, 4, true)
                            } else if is_arrow_body {
                                // Arrow function body: deeper indent in output
                                (0, 0, 7, true)
                            } else if is_expression_context {
                                // Logical operators (&&, ||): operator on previous line
                                // Include trailing content (;) since template ends up on own line
                                (0, trailing_after_template, 0, true)
                            } else if base_indent <= 2 {
                                // Statement-level: "const x = " + semicolon
                                (11, 1, 0, false)
                            } else {
                                (0, 0, 0, true)
                            };
                        let visual_indent = if use_visual {
                            base_indent * self.config.tab_width
                        } else {
                            base_indent
                        };
                        let estimated_col = visual_indent + statement_buffer + expr_prefix;
                        estimated_col + interp_width + closing
                    };
                    total_width > self.config.print_width
                };

                if should_break {
                    let content = if has_line_comment {
                        self.build_template_comments_and_expr_doc(
                            &leading_comments,
                            expr_doc,
                            &trailing_comments,
                        )
                    } else {
                        d.concat(&[leading_comments_doc, expr_doc, trailing_comments_doc])
                    };

                    let base_indent = if let Some(ws) = line_start_whitespace {
                        // Template has explicit whitespace after a newline in the quasi
                        Self::whitespace_to_indent_levels(ws, self.config.tab_width)
                    } else {
                        // Inline template - analyze context to determine indent
                        self.determine_inline_template_base_indent(quasi.span.start as usize)
                    };

                    parts.push(self.build_aligned_interpolation(content, base_indent));
                } else {
                    // Inline: ${content}
                    // Use align() to set ABSOLUTE indent level for expression's internal hardlines.
                    // This is needed because the Svelte wrapper uses start_indent_level=1 for ALL
                    // hardlines, so we must use absolute positioning to avoid double indentation.
                    let content =
                        d.concat(&[leading_comments_doc, expr_doc, trailing_comments_doc]);

                    if has_internal_breaks {
                        // Expression has internal breaks - need to account for template's visual position
                        // Calculate absolute indent level from template's line-start whitespace
                        // Note: the expression's own formatter (chain, arrow, etc.) will add +1 for its
                        // internal continuation, so we use the template position directly without +1
                        let indent_levels = if let Some(ws) = line_start_whitespace {
                            // Template has whitespace: use its visual position
                            Self::whitespace_to_indent_levels(ws, self.config.tab_width)
                        } else {
                            // No template whitespace: use 1 (for script level)
                            1
                        };

                        let indented_content = d.align(indent_levels, content);
                        // Wrap with base_indent_override=0 for correct width calculations
                        parts.push(d.with_base_indent_override(
                            d.concat(&[d.text("${"), indented_content, d.text("}")]),
                            0,
                        ));
                    } else {
                        // Simple expression without internal breaks - use align for absolute positioning
                        // (even though no hardlines exist, this ensures consistency with has_internal_breaks case)
                        let indent_levels = if let Some(ws) = line_start_whitespace {
                            Self::whitespace_to_indent_levels(ws, self.config.tab_width)
                        } else {
                            1
                        };
                        // Wrap with base_indent_override=0 for correct width calculations
                        parts.push(d.with_base_indent_override(
                            d.concat(&[d.text("${"), d.align(indent_levels, content), d.text("}")]),
                            0,
                        ));
                    }
                }
            }
        }

        parts.push(d.text("`"));
        d.concat(&parts)
    }

    /// Calculate accumulated line width for multi-interpolation templates.
    ///
    /// For templates like `${a}__${b}__${c}`, when checking if `${c}` overflows,
    /// we need to include the width of all previous content on the same line.
    ///
    /// Returns (visual_width, found_newline) where:
    /// - visual_width: width from the last newline (or backtick) up to the interpolation
    /// - found_newline: true if there's a newline before this interpolation
    fn accumulated_template_prefix_width(
        &self,
        template: &crate::ast::internal::TemplateLiteral,
        interpolation_index: usize,
    ) -> (usize, bool) {
        // Walk backwards to find the last newline
        for i in (0..=interpolation_index).rev() {
            let quasi = &template.quasis[i];

            if let Some(pos) = quasi.raw.rfind('\n') {
                // Found the last newline - count content after it
                let after_newline = &quasi.raw[pos + 1..];
                let mut width = self.visual_width(after_newline);

                // Add any subsequent expressions and quasis on this line
                // (these quasis should not have newlines since we found the last one)
                for j in i..interpolation_index {
                    // Expression width
                    let expr = &template.expressions[j];
                    let expr_source = self
                        .source
                        .get(expr.span().start as usize..expr.span().end as usize)
                        .unwrap_or("");
                    width += 2 + expr_source.len() + 1; // "${" + expr + "}"

                    // Next quasi width (no newlines expected)
                    let next_quasi = &template.quasis[j + 1];
                    width += self.visual_width(&next_quasi.raw);
                }

                return (width, true);
            }
        }

        // No newlines found - accumulate everything from the backtick
        let mut width = 1; // Opening backtick
        for i in 0..=interpolation_index {
            let quasi = &template.quasis[i];
            width += self.visual_width(&quasi.raw);

            if i < interpolation_index {
                let expr = &template.expressions[i];
                let expr_source = self
                    .source
                    .get(expr.span().start as usize..expr.span().end as usize)
                    .unwrap_or("");
                width += 2 + expr_source.len() + 1; // "${" + expr + "}"
            }
        }

        (width, false)
    }

    /// Estimate the visual width of template content on the same line as `}`.
    /// This is the content before the first newline (or entire content if no newline).
    fn estimate_template_suffix_width(&self, raw: &str) -> usize {
        let line_content = if let Some(pos) = raw.find('\n') {
            &raw[..pos]
        } else {
            raw
        };
        self.visual_width(line_content)
    }

    /// Calculate visual width of a string (tabs expand to tab_width).
    fn visual_width(&self, s: &str) -> usize {
        visual_width(s, self.config.tab_width)
    }

    /// Extract the whitespace after the last newline in template content.
    /// Returns Some(whitespace) if there's a newline, None otherwise.
    ///
    /// This is used for template-aware indentation: when breaking at `${`,
    /// we align to the visual column of the line in the template.
    fn extract_line_start_whitespace(raw: &str) -> Option<&str> {
        // Find the last newline
        let last_newline_pos = raw.rfind('\n')?;

        // Get content after the newline
        let after_newline = &raw[last_newline_pos + 1..];

        // Extract leading whitespace (tabs and spaces)
        let whitespace_end = after_newline
            .chars()
            .take_while(|c| *c == '\t' || *c == ' ')
            .count();

        Some(&after_newline[..whitespace_end])
    }

    /// Convert whitespace string to equivalent indent levels.
    ///
    /// Tabs count as tab_width visual columns. Spaces count as 1.
    /// Result is visual width / tab_width, rounded UP to ensure content
    /// is at least as indented as the template.
    fn whitespace_to_indent_levels(ws: &str, tab_width: usize) -> usize {
        let visual = visual_width(ws, tab_width);
        // Round up: (visual + tab_width - 1) / tab_width
        visual.div_ceil(tab_width)
    }

    /// Build a multiline template interpolation with aligned content.
    ///
    /// Creates: `${\n<tabs>content\n<closing_tabs>}`
    /// Uses absolute positioning via doc::align for consistent indentation.
    fn build_aligned_interpolation(&self, content: DocId, base_indent: usize) -> DocId {
        let d = self.d();
        let content_indent = base_indent + 1;
        let content_tabs = "\t".repeat(content_indent);
        let closing_tabs = "\t".repeat(base_indent);

        let content_doc = d.align(
            content_indent,
            d.concat(&[d.literalline(), d.text_owned(content_tabs), content]),
        );

        // Build the interpolation group without isolation.
        // IsolatedGroup is applied at the call/array level where hugging is needed,
        // not here - this allows ternaries and binary expressions to see the breaks.
        d.group(d.concat(&[
            d.text("${"),
            content_doc,
            d.literalline(),
            d.text_owned(format!("{closing_tabs}}}")),
        ]))
    }

    /// Determine base indent level for inline template at given source position.
    ///
    /// This is used for determining the indent level AFTER a template interpolation breaks.
    /// It uses context-specific bonuses to match the expected output indentation.
    fn determine_inline_template_base_indent(&self, pos: usize) -> usize {
        let (ws_indent, is_expression_context, template_nesting) =
            self.analyze_template_context(pos);

        // Check context type from preceding characters
        let before_pos = &self.source[..pos];
        let trimmed = before_pos.trim_end();
        let last_char = trimmed.chars().last();
        let is_ternary_context = matches!(last_char, Some(':' | '?'));
        let has_closing_delimiter = matches!(last_char, Some('(' | '[' | '{'));
        let is_arrow_body = trimmed.ends_with("=>");

        // Check if template starts on its own line (after newline + whitespace + context)
        let template_on_own_line = before_pos.rfind('\n').is_some_and(|nl_pos| {
            before_pos[nl_pos + 1..]
                .chars()
                .all(|c| c == ' ' || c == '\t' || c == '?' || c == ':')
        });

        // Add context-specific bonuses for proper indentation after breaking.
        // These account for both the expression context and template nesting.
        // Note: build_aligned_interpolation adds +1 for content, so these are
        // relative to the closing bracket position.
        //
        // For templates inside another template's interpolation, the outer will break
        // and add indent. The amount varies by context:
        // - Ternary: moderate nesting (outer breaks, ternary arm adds 1)
        // - Arrow: deep nesting (outer breaks, map/arrow each add 1)
        // - Delimiters: moderate nesting (outer breaks, structure adds 1)
        // - Expression (&&, ||): minimal nesting (outer breaks, operator flat)
        // Use .max(1) to ensure minimum indent of 1 (Svelte script level) when source
        // has no whitespace (unformatted code). Formatted code will have ws_indent >= 1.
        if is_ternary_context {
            // Ternary arms: when source already broken, use ws_indent + 1
            // When source is compact (ternary will break in output), need +2
            if template_on_own_line {
                ws_indent.max(1) + 1 + template_nesting * 2
            } else {
                ws_indent.max(1) + 2 + template_nesting * 2
            }
        } else if is_arrow_body {
            // Arrow function body: +3 per nesting level (deeper structure)
            ws_indent.max(1) + template_nesting * 3
        } else if has_closing_delimiter {
            // Array/function/object: +2 per nesting level
            ws_indent.max(1) + template_nesting * 2
        } else if is_expression_context {
            // Other expressions (&&, ||): +1 for context (binary breaks add indent)
            // +1 per nesting level for template depth
            ws_indent.max(1) + 1 + template_nesting
        } else if ws_indent == 0 {
            // Unformatted source (no leading whitespace) → use default of 1
            // This hardcoded value works for Svelte context where scripts are at indent level 1.
            // For standalone TypeScript (level 0), formatted source will have whitespace
            // so this branch won't be taken.
            1
        } else {
            // Statement context → use line-start whitespace
            ws_indent
        }
    }

    /// Estimate the output column for an inline template.
    ///
    /// This is used for the BREAK DECISION to determine if a template would exceed print width.
    /// It uses a more aggressive nesting estimate to account for deeply nested templates in
    /// compact source that would have much higher indent in formatted output.
    fn estimate_inline_template_output_indent(&self, pos: usize) -> usize {
        let (ws_indent, is_expression_context, template_nesting) =
            self.analyze_template_context(pos);

        // The source's ws_indent may underestimate output indent for nested templates.
        // When outer interpolations break, they add indent to inner content.
        // For nesting=1, the indent is usually already correct (ws_indent reflects it).
        // For nesting>=2, add adjustment to compensate for deeper nesting.
        let nesting_adjustment = template_nesting.saturating_sub(1);

        // The expression prefix (": ", "? ", "|| " etc.) is handled separately in the
        // width calculation, not here in the indent estimate.
        if is_expression_context {
            // Expression context: +1 for context, +nesting for depth
            ws_indent.max(1) + 1 + nesting_adjustment
        } else if ws_indent == 0 {
            // Unformatted source (no leading whitespace) → use default of 1
            // (Same reasoning as in determine_inline_template_base_indent)
            1 + nesting_adjustment
        } else {
            // Statement context → use line-start whitespace + nesting
            ws_indent + nesting_adjustment
        }
    }

    /// Analyze the context of a template at the given source position.
    ///
    /// Returns (ws_indent, is_expression_context, template_nesting_count).
    fn analyze_template_context(&self, pos: usize) -> (usize, bool, usize) {
        let before_pos = &self.source[..pos.min(self.source.len())];
        let line_start = before_pos.rfind('\n').map_or(0, |p| p + 1);
        let line_prefix = &self.source[line_start..pos];

        // Calculate line-start whitespace
        let ws_end = line_prefix
            .chars()
            .take_while(|c| *c == '\t' || *c == ' ')
            .count();
        let line_ws = &line_prefix[..ws_end];
        let ws_indent = Self::whitespace_to_indent_levels(line_ws, self.config.tab_width);

        // Check if template is in expression context by looking at preceding char
        let before_backtick = line_prefix.strip_suffix('`').unwrap_or(line_prefix);
        let non_ws_prefix = before_backtick[ws_end..].trim_end();
        let last_char = non_ws_prefix.chars().last();

        // Expression context: punctuators indicating we're inside an expression
        let is_expression_context = matches!(
            last_char,
            Some(':' | '?' | ',' | '(' | '[' | '{' | '&' | '|')
        );

        // Count template interpolation nesting depth
        let template_nesting = non_ws_prefix.matches("${").count();

        (ws_indent, is_expression_context, template_nesting)
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
