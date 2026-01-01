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

pub(super) use literals::normalize_number_literal;

// Re-export for submodules to use `super::X` instead of `super::super::X`
pub(super) use super::{
    CommentSpacing, ParenContext, PatternContext, Printer, needs_parens,
    object_pattern_should_expand,
};
use crate::ast::internal::Expression;
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    /// Print an expression using doc-based formatting
    pub fn print_expression(&mut self, expression: &Expression) {
        let doc = self.build_expression_doc(expression);
        self.write_doc(&doc);
    }

    /// Build a Doc for an expression (for use in object/array contexts and statements)
    pub(super) fn build_expression_doc(&self, expr: &Expression) -> Doc {
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
            Expression::Super(_) => doc::text("super"),
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
    pub(super) fn build_function_parameter_doc(&self, expr: &Expression) -> Doc {
        match expr {
            Expression::ObjectPattern(obj) => {
                self.build_object_pattern_doc_with_context(obj, PatternContext::FunctionParameter)
            }
            // For other expressions, use normal doc building
            _ => self.build_expression_doc(expr),
        }
    }

    // =========================================================================
    // TypeScript Type Assertions
    // =========================================================================

    /// Build a Doc for a TypeScript angle-bracket type assertion: `<Type>expr`
    fn build_ts_type_assertion_doc(
        &self,
        type_assert: &crate::ast::internal::TSTypeAssertion,
    ) -> Doc {
        doc::concat(vec![
            doc::text("<"),
            self.build_type_doc(&type_assert.type_annotation),
            doc::text(">"),
            self.build_expression_doc(&type_assert.expression),
        ])
    }

    /// Build a Doc for a TypeScript `as` expression
    ///
    /// Preserves comments between expression and `as` keyword (Prettier 3.7 #18161)
    /// Comments between `as` and type are moved to after the type (Prettier normalization)
    fn build_ts_as_doc(&self, as_expr: &crate::ast::internal::TSAsExpression) -> Doc {
        let needs_parens = needs_parens(&as_expr.expression, ParenContext::TypeAssertion);
        let mut parts = Vec::new();
        if needs_parens {
            parts.push(doc::text("("));
        }
        parts.push(self.build_expression_doc(&as_expr.expression));
        if needs_parens {
            parts.push(doc::text(")"));
        }

        // Find the `as` keyword position
        let expr_end = as_expr.expression.span().end;
        let type_start = as_expr.type_annotation.span().start;
        let as_keyword_pos = self.find_keyword_in_range(expr_end, type_start, "as");

        // Comments between expression and `as` keyword → place before ` as`
        if let Some(as_pos) = as_keyword_pos {
            parts.push(self.build_inline_comments_between_doc(expr_end, as_pos));
        }

        parts.push(doc::text(" as "));
        parts.push(self.build_type_doc(&as_expr.type_annotation));

        // Comments between `as` keyword and type → place after the type
        if let Some(as_pos) = as_keyword_pos {
            let as_end = as_pos + 2; // "as" is 2 chars
            parts.push(self.build_inline_comments_between_doc(as_end, type_start));
        }

        doc::concat(parts)
    }

    /// Build a Doc for a TypeScript `satisfies` expression
    ///
    /// Preserves comments between expression and `satisfies` keyword (Prettier 3.7 #18162)
    fn build_ts_satisfies_doc(
        &self,
        sat_expr: &crate::ast::internal::TSSatisfiesExpression,
    ) -> Doc {
        let needs_parens = needs_parens(&sat_expr.expression, ParenContext::TypeAssertion);
        let mut parts = Vec::new();
        if needs_parens {
            parts.push(doc::text("("));
        }
        parts.push(self.build_expression_doc(&sat_expr.expression));
        if needs_parens {
            parts.push(doc::text(")"));
        }

        // Include comments between expression and `satisfies` keyword
        let expr_end = sat_expr.expression.span().end;
        let type_start = sat_expr.type_annotation.span().start;
        parts.push(self.build_inline_comments_between_doc(expr_end, type_start));

        parts.push(doc::text(" satisfies "));
        parts.push(self.build_type_doc(&sat_expr.type_annotation));
        doc::concat(parts)
    }

    /// Build a Doc for a TypeScript instantiation expression
    fn build_ts_instantiation_doc(
        &self,
        inst_expr: &crate::ast::internal::TSInstantiationExpression,
    ) -> Doc {
        let mut parts = Vec::new();
        let needs_parens =
            needs_parens(&inst_expr.expression, ParenContext::InstantiationExpression);
        if needs_parens {
            parts.push(doc::text("("));
        }
        parts.push(self.build_expression_doc(&inst_expr.expression));
        if needs_parens {
            parts.push(doc::text(")"));
        }
        parts.push(self.build_type_parameter_instantiation_doc(&inst_expr.type_arguments));
        doc::concat(parts)
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
    ) -> Doc {
        let needs_parens = needs_parens(&non_null_expr.expression, ParenContext::NonNull);

        if needs_parens {
            // For expressions that need parens, use a special doc structure
            // that indents continuations when breaking
            let inner_doc =
                self.build_expression_doc_with_indent_on_break(&non_null_expr.expression);
            doc::concat(vec![doc::text("("), inner_doc, doc::text(")!")])
        } else if Self::is_chain_expression(&non_null_expr.expression) {
            // When inner expression is a chain (member or call), use chain architecture
            // to properly handle breaking. This ensures the outer `!` is included
            // in the linearized chain for proper segment grouping.
            use super::chain;
            let expr = Expression::TSNonNullExpression(non_null_expr.clone());
            let nodes = chain::linearize_chain(&expr);
            let groups = chain::group_chain_nodes(nodes);
            chain::build_chain_doc(&groups, self)
        } else {
            let inner_doc = self.build_expression_doc(&non_null_expr.expression);
            doc::concat(vec![inner_doc, doc::text("!")])
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
    pub(crate) fn build_expression_doc_with_indent_on_break(&self, expr: &Expression) -> Doc {
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
    fn build_binary_chain_doc_indented(
        &self,
        binary: &crate::ast::internal::BinaryExpression,
    ) -> Doc {
        doc::group(self.build_binary_chain_parts_indented(binary))
    }

    /// Build binary chain parts with indented continuations (no group wrapper)
    ///
    /// Returns the concat without a group wrapper, for cases where the caller
    /// wants to control the grouping (e.g., chain printing).
    pub(crate) fn build_binary_chain_parts_indented(
        &self,
        binary: &crate::ast::internal::BinaryExpression,
    ) -> Doc {
        // Collect all operands and operators in the chain
        let mut operands = Vec::new();
        let mut operators = Vec::new();
        self.collect_binary_operands_for_indent(binary, &mut operands, &mut operators);

        if operands.len() <= 1 {
            // Fallback to regular expression doc
            return self.build_binary_doc(binary);
        }

        // Build with indented continuations:
        // "first +
        //     second -
        //     third"
        let mut parts = Vec::new();

        for (i, operand) in operands.iter().enumerate() {
            if i == 0 {
                parts.push(operand.clone());
            } else {
                parts.push(doc::text(" "));
                parts.push(doc::text(operators[i - 1].as_str()));
                parts.push(doc::indent_line(operand.clone()));
            }
        }

        doc::concat(parts)
    }

    /// Collect operands and operators from a binary chain (helper for indented version)
    ///
    /// Uses `can_flatten_with()` to determine which operators can be chained together.
    fn collect_binary_operands_for_indent(
        &self,
        expr: &crate::ast::internal::BinaryExpression,
        operands: &mut Vec<Doc>,
        operators: &mut Vec<crate::ast::internal::BinaryOperator>,
    ) {
        // Recursively flatten left side if it can be chained with current operator
        if let Expression::BinaryExpression(left_binary) = &*expr.left {
            if expr.operator.can_flatten_with(left_binary.operator) {
                self.collect_binary_operands_for_indent(left_binary, operands, operators);
            } else {
                // Can't flatten - build operand with parens if needed
                let doc = self.build_expression_doc(&expr.left);
                let ctx = ParenContext::BinaryLeft {
                    parent_op: expr.operator,
                };
                if needs_parens(&expr.left, ctx) {
                    operands.push(doc::parens(doc));
                } else {
                    operands.push(doc);
                }
            }
        } else {
            operands.push(self.build_expression_doc(&expr.left));
        }

        // Add current operator
        operators.push(expr.operator);

        // Add right operand with parens if needed
        if let Expression::BinaryExpression(_) = &*expr.right {
            let doc = self.build_expression_doc(&expr.right);
            let ctx = ParenContext::BinaryRight {
                parent_op: expr.operator,
            };
            if needs_parens(&expr.right, ctx) {
                operands.push(doc::parens(doc));
            } else {
                operands.push(doc);
            }
        } else {
            operands.push(self.build_expression_doc(&expr.right));
        }
    }

    // =========================================================================
    // Template Literals
    // =========================================================================

    /// Build a Doc for a template literal
    fn build_template_literal_doc(&self, template: &crate::ast::internal::TemplateLiteral) -> Doc {
        let mut parts = Vec::new();
        parts.push(doc::text("`"));

        for (i, quasi) in template.quasis.iter().enumerate() {
            // Template content (raw, preserving escapes)
            parts.push(doc::text_owned(quasi.raw.clone()));

            // Interpolation
            if i < template.expressions.len() {
                let expr = &template.expressions[i];
                parts.push(doc::text("${"));

                // Check for comments between ${ and the expression
                // The quasi.span.end is the position right after ${, so comments start there
                parts.push(self.build_comments_between(
                    quasi.span.end,
                    expr.span().start,
                    CommentSpacing::Trailing,
                ));

                parts.push(self.build_expression_doc(expr));
                parts.push(doc::text("}"));
            }
        }

        parts.push(doc::text("`"));
        doc::concat(parts)
    }

    /// Build a Doc for a tagged template expression
    fn build_tagged_template_doc(
        &self,
        tagged: &crate::ast::internal::TaggedTemplateExpression,
    ) -> Doc {
        doc::concat(vec![
            self.build_expression_doc(&tagged.tag),
            self.build_template_literal_doc(&tagged.quasi),
        ])
    }

    /// Build a Doc for a TypeScript parameter property
    fn build_ts_parameter_property_doc(
        &self,
        param_prop: &crate::ast::internal::TSParameterProperty,
    ) -> Doc {
        let mut parts = Vec::new();

        // Print accessibility modifier
        if let Some(acc) = &param_prop.accessibility {
            parts.push(doc::text(acc.as_str()));
            parts.push(doc::text(" "));
        }

        // Print readonly modifier
        if param_prop.readonly {
            parts.push(doc::text("readonly "));
        }

        // Print the parameter (identifier or assignment pattern)
        parts.push(self.build_expression_doc(&param_prop.parameter));

        doc::concat(parts)
    }
}
