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
pub(crate) mod literals;
mod patterns;

pub(super) use literals::format_string_literal_from_ast;

// Re-export for submodules to use `super::X` instead of `super::super::X`
use super::chain;
use super::comments::CommentSpacing;
pub(super) use super::{
    ParenContext, PatternContext, Printer, needs_parens, object_pattern_should_expand,
    unwrap_parenthesized,
};
use crate::ast::internal::{BinaryExpression, BinaryOperator, Expression, TSType};
use tsv_lang::doc::arena::DocId;
use tsv_lang::printing::visual_width;

impl<'a> Printer<'a> {
    /// Print an expression using doc-based formatting
    pub(crate) fn print_expression(&mut self, expression: &Expression) {
        let doc = self.build_expression_doc(expression);
        self.write_arena_doc(doc);
    }

    /// Wrap `doc` in parens when `span` is the object/function/class node that
    /// starts the enclosing expression statement (set by `build_expression_statement_doc`
    /// via `leftmost_no_lookahead`). Consumes the target so it fires exactly once:
    /// `(class {}).foo` wraps the class, not the whole member expression.
    fn maybe_wrap_expr_stmt_paren(&self, span: tsv_lang::Span, doc: DocId) -> DocId {
        if self.expr_stmt_paren_target.get() == Some(span) {
            self.expr_stmt_paren_target.set(None);
            let d = self.d();
            d.concat(&[d.text("("), doc, d.text(")")])
        } else {
            doc
        }
    }

    /// Build a Doc for an expression (for use in object/array contexts and statements)
    pub(crate) fn build_expression_doc(&self, expr: &Expression) -> DocId {
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
                let doc = if needs_arrow_parens {
                    let d = self.d();
                    d.concat(&[d.text("("), doc, d.text(")")])
                } else {
                    doc
                };
                self.maybe_wrap_expr_stmt_paren(obj.span, doc)
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
            Expression::FunctionExpression(func) => {
                self.maybe_wrap_expr_stmt_paren(func.span, self.build_function_doc(func))
            }
            Expression::ClassExpression(class_expr) => self.maybe_wrap_expr_stmt_paren(
                class_expr.span,
                self.build_class_expression_doc(class_expr),
            ),
            Expression::SpreadElement(spread) => self.build_spread_doc(spread),
            Expression::TemplateLiteral(template) => self.build_template_literal_doc(template),
            Expression::TaggedTemplateExpression(tagged) => self.build_tagged_template_doc(tagged),
            Expression::AwaitExpression(await_expr) => self.build_await_doc(await_expr),
            Expression::YieldExpression(yield_expr) => self.build_yield_doc(yield_expr),
            Expression::SequenceExpression(seq) => self.build_sequence_doc(seq),
            Expression::RegexLiteral(regex) => self.build_regex_doc(regex),
            Expression::ThisExpression(_) => d.text("this"),
            Expression::Super(_) => d.text("super"),
            Expression::AssignmentExpression(assign) => self.build_assignment_doc(assign),
            Expression::ObjectPattern(obj) => self.build_object_pattern_doc(obj),
            Expression::ArrayPattern(arr) => self.build_array_pattern_doc(arr),
            Expression::AssignmentPattern(pattern) => self.build_assignment_pattern_doc(pattern),
            Expression::RestElement(rest) => self.build_rest_element_doc(rest),
            Expression::TSTypeAssertion(type_assert) => {
                self.build_ts_type_assertion_doc(type_assert)
            }
            Expression::TSAsExpression(as_expr) => self.build_binary_cast_doc(
                &as_expr.expression,
                &as_expr.type_annotation,
                "as",
                true,
            ),
            Expression::TSSatisfiesExpression(sat_expr) => self.build_binary_cast_doc(
                &sat_expr.expression,
                &sat_expr.type_annotation,
                "satisfies",
                false,
            ),
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
        // Comments between `<` and the type
        let angle_end = type_assert.span.start + 1; // after `<`
        let type_start = type_assert.type_annotation.span().start;
        let comments_doc =
            self.build_comments_between(angle_end, type_start, CommentSpacing::Trailing);
        let type_doc = self.build_type_doc_with_wrapping_type_args(&type_assert.type_annotation);

        // Mirror Prettier's `printTypeAssertion`: the cast `<Type>` is its own
        // group, breaking after `<` with the type on an indented line and `>`
        // back at the outer indent. Crucially, a union cast type prints *flat* on
        // that line — Prettier's `shouldIndentUnionType` returns false for
        // `TSTypeAssertion`, so it never gets the leading-`|` hanging indent that
        // `as`/`satisfies` casts use (see `build_union_hanging_indent_doc`).
        let cast_group = d.group(d.concat(&[
            d.text("<"),
            d.indent(d.concat(&[d.softline(), comments_doc, type_doc])),
            d.softline(),
            d.text(">"),
        ]));

        let inner_expr = self.build_expression_doc(&type_assert.expression);
        let expr_doc = if expr_needs_parens {
            d.concat(&[d.text("("), inner_expr, d.text(")")])
        } else {
            inner_expr
        };

        // `shouldBreakAfterCast`: object/array-literal expressions hug the cast
        // (they expand themselves), everything else may break the expression into
        // its own parenthesized block before the cast group itself breaks.
        let should_break_after_cast = !matches!(
            &*type_assert.expression,
            Expression::ArrayExpression(_) | Expression::ObjectExpression(_)
        );

        if should_break_after_cast {
            let expr_contents = d.group_break(d.concat(&[
                d.if_break(d.text("("), d.empty()),
                d.indent(d.concat(&[d.softline(), expr_doc])),
                d.softline(),
                d.if_break(d.text(")"), d.empty()),
            ]));
            d.conditional_group(&[
                d.concat(&[cast_group, expr_doc]),
                d.concat(&[cast_group, expr_contents]),
                d.concat(&[cast_group, expr_doc]),
            ])
        } else {
            d.group(d.concat(&[cast_group, expr_doc]))
        }
    }

    /// Build a Doc for a TypeScript binary cast expression — `expr as Type` or
    /// `expr satisfies Type`. Mirrors Prettier's `printBinaryCastExpression`,
    /// which prints both with one function (`isSatisfiesExpression ? "satisfies" : "as"`).
    ///
    /// `keyword` is the bare keyword (`"as"` / `"satisfies"`); `is_as` gates the
    /// `as const` special-case (comments move after `const`), which cannot apply
    /// to `satisfies` (`satisfies const` is invalid TypeScript).
    ///
    /// Preserves comments between the expression and the keyword (Prettier 3.7
    /// #18161 / #18162); comments between the keyword and the type are kept in
    /// place except for `as const`, where Prettier relocates them after `const`.
    fn build_binary_cast_doc(
        &self,
        expression: &Expression,
        type_annotation: &TSType,
        keyword: &'static str,
        is_as: bool,
    ) -> DocId {
        let d = self.d();
        let needs_parens = needs_parens(expression, ParenContext::TypeAssertion);
        let mut parts = Vec::new();
        if needs_parens {
            parts.push(d.text("("));
        }
        parts.push(self.build_expression_doc(expression));
        if needs_parens {
            parts.push(d.text(")"));
        }

        // Find the keyword position
        let expr_end = expression.span().end;
        let type_start = type_annotation.span().start;
        let keyword_pos = self.find_keyword_in_range(expr_end, type_start, keyword);

        // Comments between expression and keyword → place before the keyword
        if let Some(kw_pos) = keyword_pos {
            parts.push(self.build_inline_comments_between_doc(expr_end, kw_pos));
        }

        // Union cast types break after the keyword with a hanging indent.
        if let Some(tail) =
            self.cast_union_hanging_tail(keyword, keyword_pos, type_annotation, type_start)
        {
            parts.push(tail);
            return d.concat(&parts);
        }

        parts.push(d.text(" "));
        parts.push(d.text(keyword));
        parts.push(d.text(" "));

        // Comments between keyword and type
        if let Some(kw_pos) = keyword_pos {
            let kw_end = kw_pos + keyword.len() as u32;
            // `as const`: prettier moves comments after `const`; otherwise keep in place.
            let is_as_const = is_as
                && &self.source[type_start as usize..type_annotation.span().end as usize]
                    == "const";
            if is_as_const {
                parts.push(self.build_type_doc_with_wrapping_type_args(type_annotation));
                parts.push(self.build_inline_comments_between_doc(kw_end, type_start));
            } else {
                parts.push(self.build_comments_between(
                    kw_end,
                    type_start,
                    CommentSpacing::Trailing,
                ));
                parts.push(self.build_type_doc_with_wrapping_type_args(type_annotation));
            }
        } else {
            parts.push(self.build_type_doc_with_wrapping_type_args(type_annotation));
        }

        d.concat(&parts)
    }

    /// The keyword-plus-type tail for an `as`/`satisfies` cast when the cast type
    /// is a non-hugging union: it breaks after the keyword with a hanging indent
    /// (Prettier's `shouldIndentUnionType`). `keyword` is the bare keyword
    /// (`"as"` / `"satisfies"`).
    ///
    /// Returns `None` to fall through to the caller's inline layout — for
    /// non-union or hugging types, or when a comment sits between the keyword and
    /// the type.
    ///
    /// TODO: a comment before a *breaking* union (`x as /* c */ A | B` past print
    /// width) still misses the hanging indent. Prettier is non-idempotent here (it
    /// relocates the comment across the keyword), so the target is a
    /// comment-position-philosophy case, not a clean match — deferred.
    fn cast_union_hanging_tail(
        &self,
        keyword: &'static str,
        keyword_pos: Option<u32>,
        type_annotation: &TSType,
        type_start: u32,
    ) -> Option<DocId> {
        let keyword_len = keyword.len() as u32;
        if keyword_pos.is_some_and(|pos| self.has_comments_between(pos + keyword_len, type_start)) {
            return None;
        }
        let hanging = self.build_union_hanging_indent_doc(type_annotation)?;
        let d = self.d();
        Some(d.concat(&[d.text(" "), d.text(keyword), hanging]))
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
        // Preserve comments between expression and type args: `fn/* c */ <string>`
        let expr_end = inst_expr.expression.span().end;
        let ta_start = inst_expr.type_arguments.span.start;
        if let Some(doc) = self.build_name_to_type_params_comments_opt(
            expr_end,
            ta_start,
            CommentSpacing::Trailing,
        ) {
            parts.push(doc);
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
            let nodes = chain::linearize_chain_from_non_null(non_null_expr);
            let groups = chain::group_chain_nodes(nodes);
            chain::build_chain_doc(&groups, self)
        } else {
            let inner_doc = self.build_expression_doc(&non_null_expr.expression);
            // Check for trailing comments from stripped grouping parens: `(x /* c */)!`
            let argument_end = non_null_expr.expression.span().end;
            let has_trailing_comments =
                self.has_comments_between(argument_end, non_null_expr.span.end);
            if has_trailing_comments {
                let mut parts = vec![inner_doc];
                self.append_trailing_paren_comments(
                    &mut parts,
                    argument_end,
                    non_null_expr.span.end,
                );
                parts.push(d.text("!"));
                d.concat(&parts)
            } else {
                d.concat(&[inner_doc, d.text("!")])
            }
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
    /// Two formatting strategies based on expression type:
    /// - **Qualifying types** (Identifier, MemberExpression, Conditional, etc.):
    ///   softline wrapping in a regular group — `${/}` breaks when line exceeds width.
    /// - **Non-qualifying types** (CallExpression, chains, arrows, etc.):
    ///   no softlines at `${/}` — expression breaks internally, boundaries hug.
    ///   Matches Prettier's approach where these types keep their doc structure.
    fn build_template_literal_doc(
        &self,
        template: &crate::ast::internal::TemplateLiteral,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        parts.push(d.line_suffix_boundary());
        parts.push(d.text("`"));

        let mut previous_quasi_indent_size: usize = 0;

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
                let indent_size = if text.contains('\n') {
                    self.get_template_indent_size(text)
                } else {
                    previous_quasi_indent_size
                };
                previous_quasi_indent_size = indent_size;

                // Build expression doc
                let expr_doc = if needs_parens(expr, ParenContext::TemplateLiteralExpression) {
                    d.parens(self.build_expression_doc(expr))
                } else {
                    self.build_expression_doc(expr)
                };

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

                let has_comments = !leading_comments.is_empty() || !trailing_comments.is_empty();
                let has_trailing_line_comment = trailing_comments.iter().any(|c| !c.is_block);

                // Qualifying types and trailing line comments use softline wrapping
                // at ${/} boundaries so the group can break there.
                // Non-qualifying types keep expression doc as-is (no ${/} softlines)
                // so ${ hugs while the expression breaks internally.
                let use_softline_wrap = has_trailing_line_comment
                    || Self::is_template_softline_expression(expr, has_comments);
                let inner = if use_softline_wrap {
                    d.concat(&[
                        d.indent(d.concat(&[d.softline(), full_expr_doc])),
                        d.softline(),
                    ])
                } else {
                    full_expr_doc
                };

                // Apply alignment based on quasi indent (Prettier's addAlignmentToDoc).
                // Three paths matching Prettier's template-literal.js:262-265:
                // 1. indent_size==0 && quasi ends with \n: expression starts at column 0
                //    on a new line — reset indent to absolute 0 (align(-∞)).
                // 2. indent_size>0: expression follows indented template content —
                //    apply full alignment (indent levels + reset).
                // 3. indent_size==0 && quasi doesn't end with \n: inline quasi (e.g. ", ")
                //    — preserve code context indent, no wrapping.
                let aligned = if indent_size == 0 && text.ends_with('\n') {
                    d.align(0, inner)
                } else if indent_size > 0 {
                    self.add_alignment_to_doc(inner, indent_size)
                } else {
                    inner
                };
                let group_doc =
                    d.concat(&[d.text("${"), aligned, d.line_suffix_boundary(), d.text("}")]);
                // Force break when trailing line comments are present — a line
                // comment on a flat line would swallow the closing `}`.
                parts.push(if has_trailing_line_comment {
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
            visual_width(&after_nl[..ws_end], tsv_lang::TAB_WIDTH)
        } else {
            0
        }
    }

    /// Apply alignment to a doc based on indent size.
    /// Equivalent to Prettier's `addAlignmentToDoc(doc, size, tabWidth)`.
    ///
    /// When size > 0: wraps with `align(0)` to reset indent to absolute 0,
    /// then applies indent levels from zero. This is critical because
    /// after `literalline` in template content, the output column resets
    /// to 0 but the indent level on the command stack still carries the
    /// code context. Without the reset, softlines would break at the
    /// inherited code indent instead of the template's visual position.
    ///
    /// When size == 0: returns doc unchanged (matching Prettier's behavior).
    fn add_alignment_to_doc(&self, doc: DocId, size: usize) -> DocId {
        if size == 0 {
            return doc;
        }
        let d = self.d();
        let tab_width = tsv_lang::TAB_WIDTH;
        // In prettier's useTabs renderer, align(n%tw) creates a WIDTH command
        // that adds lastTabs=1 + lastSpaces=n%tw. When followed by INDENT,
        // flushTabs() emits the pending tab and resetLast() drops the fractional
        // spaces. So align(r) + indent effectively rounds up to a full tab.
        // Match this by using ceiling division for the indent count.
        let n = size.div_ceil(tab_width);
        let mut result = doc;
        for _ in 0..n {
            result = d.indent(result);
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

    /// Whether this expression type qualifies for softline wrapping at
    /// `${`/`}` boundaries. Matches Prettier's qualifying type list
    /// (template-literal.js:230-238).
    ///
    /// Qualifying types: simple expressions with no inherent block structure.
    /// Softline wrapping lets the `${}` group break when the line exceeds width.
    ///
    /// Non-qualifying types (CallExpression, ArrowFunctionExpression,
    /// TemplateLiteral, etc.): have internal break points or their own
    /// visual formatting. `${}` hugs while the expression breaks internally.
    fn is_template_softline_expression(expr: &Expression, has_comments: bool) -> bool {
        if has_comments {
            return true;
        }
        // Matches Prettier's qualifying types (template-literal.js:230-238):
        // Identifier, MemberExpression, ConditionalExpression, SequenceExpression,
        // isBinaryCastExpression (TSAsExpression, TSSatisfiesExpression),
        // isBinaryish (BinaryExpression — includes &&, ||, ??).
        // Plus additional simple types that have no internal break points.
        matches!(
            expr,
            Expression::Identifier(_)
                | Expression::Literal(_)
                | Expression::MemberExpression(_)
                | Expression::ConditionalExpression(_)
                | Expression::UnaryExpression(_)
                | Expression::UpdateExpression(_)
                | Expression::MetaProperty(_)
                | Expression::ThisExpression(_)
                | Expression::Super(_)
                | Expression::BinaryExpression(_)
                | Expression::SequenceExpression(_)
                | Expression::TSAsExpression(_)
                | Expression::TSSatisfiesExpression(_)
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

        let mut parts = vec![tag_doc];
        if let Some(type_args) = &tagged.type_arguments {
            // Preserve comments between tag and type args: `fn/* c */ <string>`template``
            let tag_end = tagged.tag.span().end;
            let ta_start = type_args.span.start;
            if let Some(doc) = self.build_name_to_type_params_comments_opt(
                tag_end,
                ta_start,
                CommentSpacing::Trailing,
            ) {
                parts.push(doc);
            }
            parts.push(self.build_type_parameter_instantiation_doc(type_args));
        }

        // Emit comments between tag (or type_args) and template literal
        // e.g., `foo /* c */ \`x\`` or `foo\n// c\n\`x\``
        let comment_start = tagged
            .type_arguments
            .as_ref()
            .map_or_else(|| tagged.tag.span().end, |ta| ta.span.end);
        let comment_end = tagged.quasi.span.start;
        let gap_comments: Vec<_> =
            tsv_lang::comments_in_range(self.comments, comment_start, comment_end).collect();
        if !gap_comments.is_empty() {
            let mut prev_end = comment_start;
            let mut ends_with_hardline = false;
            for comment in &gap_comments {
                if comment.is_block {
                    let on_own_line = self.has_newline_between(prev_end, comment.span.start);
                    if on_own_line || ends_with_hardline {
                        parts.push(d.hardline());
                        parts.push(self.build_comment_doc(comment));
                        // Check if there's a newline after this block comment
                        ends_with_hardline =
                            self.has_newline_between(comment.span.end, comment_end);
                    } else {
                        parts.push(d.text(" "));
                        parts.push(self.build_comment_doc(comment));
                        ends_with_hardline = false;
                    }
                } else {
                    // Line comment: hardline or space before, always hardline after
                    let on_own_line = self.has_newline_between(prev_end, comment.span.start);
                    if on_own_line || ends_with_hardline {
                        parts.push(d.hardline());
                    } else {
                        parts.push(d.text(" "));
                    }
                    parts.push(self.build_comment_doc(comment));
                    ends_with_hardline = true;
                }
                prev_end = comment.span.end;
            }
            if ends_with_hardline {
                parts.push(d.hardline());
            } else {
                parts.push(d.text(" "));
            }
        }

        parts.push(self.build_template_literal_doc(&tagged.quasi));
        d.concat(&parts)
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
