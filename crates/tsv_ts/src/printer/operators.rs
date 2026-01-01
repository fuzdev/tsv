// Operator expression printing for TypeScript
//
// Handles printing of unary and binary expressions with:
// - Operator precedence and parenthesization
// - Clarity-based parens (mixing logical operators, etc.)

use super::{ParenContext, Printer, needs_parens};
use crate::ast::internal::{self, BinaryOperator, Expression};
use tsv_lang::Span;
use tsv_lang::doc::{self, Doc};

/// Holds information about an operand in a binary expression chain
/// Used to track position information for comment placement
struct ChainOperand {
    doc: Doc,
    span: Span,
}

/// Style for building binary expression chain docs
#[derive(Clone, Copy)]
enum BinaryChainStyle {
    /// Wrapped in a group, flat structure (for standalone binary expressions)
    Grouped,
    /// No group wrapper, flat structure (for conditions where parent controls breaking)
    Ungrouped,
    /// First operand at base indent, continuation lines indented (for attribute contexts)
    ContinuationIndent,
}

/// Operator position in source, used for comment splitting
struct OperatorPosition {
    /// Start position of operator in source
    start: u32,
    /// End position of operator in source (start + operator length)
    end: u32,
}

impl<'a> Printer<'a> {
    /// Build a Doc for an update expression
    pub(super) fn build_update_doc(&self, update: &internal::UpdateExpression) -> Doc {
        let argument_doc = self.build_expression_doc(&update.argument);
        let operator_doc = doc::text(update.operator.as_str());

        if update.prefix {
            // Prefix: ++x, --x
            doc::concat(vec![operator_doc, argument_doc])
        } else {
            // Postfix: x++, x--
            doc::concat(vec![argument_doc, operator_doc])
        }
    }

    /// Build a Doc for a unary expression
    pub(super) fn build_unary_doc(&self, unary: &internal::UnaryExpression) -> Doc {
        let argument_doc = if needs_parens(&unary.argument, ParenContext::UnaryArgument) {
            // Binary expressions need parens - use grouping for logical ops to allow line breaking
            if let Expression::BinaryExpression(binary) = unary.argument.as_ref() {
                if binary.operator.is_logical() {
                    let inner = self.build_expression_doc(&unary.argument);
                    doc::group(doc::concat(vec![
                        doc::text("("),
                        doc::indent_softline(inner),
                        doc::softline(),
                        doc::text(")"),
                    ]))
                } else {
                    doc::concat(vec![
                        doc::text("("),
                        self.build_expression_doc(&unary.argument),
                        doc::text(")"),
                    ])
                }
            } else {
                // Non-binary that needs parens (shouldn't happen currently)
                doc::concat(vec![
                    doc::text("("),
                    self.build_expression_doc(&unary.argument),
                    doc::text(")"),
                ])
            }
        } else {
            self.build_expression_doc(&unary.argument)
        };

        // Keyword operators need a space before the operand
        if unary.operator.is_keyword_operator() {
            doc::concat(vec![
                doc::text(unary.operator.as_str()),
                doc::text(" "),
                argument_doc,
            ])
        } else {
            doc::concat(vec![doc::text(unary.operator.as_str()), argument_doc])
        }
    }

    /// Build a Doc for a binary expression
    ///
    /// Implements prettier's "add parens for clarity" behavior where mixing certain
    /// operators requires parentheses for readability:
    /// - `a && b || c` → `(a && b) || c` (mixing && and ||)
    /// - `a || b && c` → `a || (b && c)`
    /// - `a == b == c` → `(a == b) == c` (chained equality)
    /// - `x + (y + z)` → preserves right-side parens for same precedence
    ///
    /// Also supports line wrapping for long binary expressions:
    /// ```text
    /// a +
    /// b +
    /// c
    /// ```
    ///
    /// In inline embedded contexts (e.g., Svelte template expressions `{...}`),
    /// continuation lines get extra indentation to align with the outer context.
    ///
    /// See: prettier/src/language-js/print/binaryish.js
    pub(super) fn build_binary_doc(&self, binary: &internal::BinaryExpression) -> Doc {
        // Use continuation indent in inline embedded contexts (first_line_offset > 0)
        // This ensures wrapped lines get proper indentation in Svelte template expressions
        if self.config.first_line_offset > 0 {
            self.build_binary_chain_doc_with_continuation_indent(binary)
        } else {
            self.build_binary_chain_doc(binary)
        }
    }

    /// Build a doc for a chain of binary operators with line wrapping support
    ///
    /// When the chain exceeds print width, breaks after operators:
    /// ```text
    /// a +
    /// b +
    /// c
    /// ```
    ///
    /// Flattens same-precedence operators that can be chained (e.g., a + b + c)
    /// but preserves parentheses where needed for clarity (e.g., a * b / c).
    ///
    /// Note: The binary expression doc itself does NOT add indent for continuations.
    /// Indentation comes from the parent context (e.g., assignment adds indent,
    /// function call args add indent, etc.).
    ///
    /// Handles comments between operands (Prettier 3.7 #17723):
    /// - Line comments force a line break
    /// - Block comments are printed inline
    fn build_binary_chain_doc(&self, binary: &internal::BinaryExpression) -> Doc {
        self.build_binary_chain_doc_core(binary, BinaryChainStyle::Grouped)
    }

    /// Build a binary chain doc WITHOUT the outer group wrapper
    ///
    /// Use in contexts where the parent group should control breaking (e.g., if conditions).
    /// The line() elements will break with the parent group.
    pub(super) fn build_binary_chain_doc_ungrouped(
        &self,
        binary: &internal::BinaryExpression,
    ) -> Doc {
        self.build_binary_chain_doc_core(binary, BinaryChainStyle::Ungrouped)
    }

    /// Build a binary chain doc with continuation indent
    ///
    /// When the chain breaks, continuation lines are indented relative to the first:
    /// ```text
    /// first &&
    ///   second &&
    ///   third
    /// ```
    ///
    /// This is used in attribute contexts (like Svelte's `={...}`) where prettier uses
    /// this specific indentation style.
    pub(super) fn build_binary_chain_doc_with_continuation_indent(
        &self,
        binary: &internal::BinaryExpression,
    ) -> Doc {
        self.build_binary_chain_doc_core(binary, BinaryChainStyle::ContinuationIndent)
    }

    /// Core implementation for binary chain doc building
    ///
    /// Handles three styles:
    /// - `Grouped`: Wrapped in a group, flat structure (standalone binary expressions)
    /// - `Ungrouped`: No group wrapper, flat structure (conditions where parent controls breaking)
    /// - `ContinuationIndent`: First operand at base, rest indented (attribute contexts)
    fn build_binary_chain_doc_core(
        &self,
        binary: &internal::BinaryExpression,
        style: BinaryChainStyle,
    ) -> Doc {
        // Collect all operands (with spans) and operators in the chain
        let mut operands: Vec<ChainOperand> = Vec::new();
        let mut operators = Vec::new();
        self.collect_binary_chain_with_spans(binary, &mut operands, &mut operators);

        if operands.len() <= 1 {
            // Single operand, shouldn't happen but handle gracefully
            return self.build_expression_doc(&binary.left);
        }

        // For ContinuationIndent, we separate first operand from the rest
        // For other styles, we build a flat parts list
        match style {
            BinaryChainStyle::ContinuationIndent => {
                self.build_binary_chain_continuation_indent(&operands, &operators)
            }
            _ => self.build_binary_chain_flat(&operands, &operators, style),
        }
    }

    /// Build a flat binary chain (Grouped or Ungrouped style)
    ///
    /// Matches Prettier's binaryish.js structure (lines 169-178):
    /// - First operand + first operator at base indent
    /// - Rest wrapped in indent() for continuation indent when broken
    ///
    /// When flat: "first + second + third"
    /// When broken:
    /// "first +
    ///     second +
    ///     third"
    fn build_binary_chain_flat(
        &self,
        operands: &[ChainOperand],
        operators: &[BinaryOperator],
        style: BinaryChainStyle,
    ) -> Doc {
        if operands.is_empty() {
            return doc::text("");
        }

        if operands.len() == 1 {
            return operands[0].doc.clone();
        }

        // First operand stays at base indent
        let mut head_parts = vec![operands[0].doc.clone()];

        // Add first operator to head (stays at base indent with first operand)
        let first_op = operators[0];
        let first_op_str = first_op.as_str();
        let first_op_pos =
            self.find_operator_position(operands[0].span.end, operands[1].span.start, first_op_str);

        // Comments before first operator
        let comments_before_first_op =
            self.build_inline_comments_between_doc(operands[0].span.end, first_op_pos.start);
        head_parts.push(comments_before_first_op);
        head_parts.push(doc::text(" "));
        head_parts.push(doc::text(first_op_str));

        // Build continuation parts (will be wrapped in indent)
        let mut continuation_parts = Vec::new();

        for i in 1..operands.len() {
            let operand = &operands[i];
            let prev_operand = &operands[i - 1];
            let operator = operators[i - 1];
            let op_str = operator.as_str();
            let op_pos =
                self.find_operator_position(prev_operand.span.end, operand.span.start, op_str);

            // Add line break and operand
            self.append_post_operator_parts(
                &mut continuation_parts,
                op_pos.end,
                prev_operand.span.end,
                operand,
            );

            // Add next operator (if not last operand)
            if i < operands.len() - 1 {
                let next_op = operators[i];
                let next_op_str = next_op.as_str();
                let next_op_pos = self.find_operator_position(
                    operand.span.end,
                    operands[i + 1].span.start,
                    next_op_str,
                );

                // Comments before next operator
                let comments_before_next_op =
                    self.build_inline_comments_between_doc(operand.span.end, next_op_pos.start);
                continuation_parts.push(comments_before_next_op);
                continuation_parts.push(doc::text(" "));
                continuation_parts.push(doc::text(next_op_str));
            }
        }

        // Combine: head + continuation
        // NO internal continuation indent for binary expressions - the parent context (assignment,
        // enum member, etc.) provides the indent wrapper. This matches Prettier's binaryish.js
        // where `shouldIndentIfInlining` cases return `group(parts)` without internal indent.
        let mut parts = head_parts;
        parts.append(&mut continuation_parts);

        match style {
            BinaryChainStyle::Grouped => doc::group(doc::concat(parts)),
            _ => doc::concat(parts),
        }
    }

    /// Build a binary chain with continuation indent
    ///
    /// When flat: "first && second && third"
    /// When broken:
    /// "first &&
    ///   second &&
    ///   third"
    fn build_binary_chain_continuation_indent(
        &self,
        operands: &[ChainOperand],
        operators: &[BinaryOperator],
    ) -> Doc {
        // First operand + first operator (stays at base indent)
        let mut first_parts = vec![operands[0].doc.clone()];

        let first_op = operators[0];
        let first_op_str = first_op.as_str();
        let first_op_pos =
            self.find_operator_position(operands[0].span.end, operands[1].span.start, first_op_str);

        // Comments before first operator
        let comments_before_op =
            self.build_inline_comments_between_doc(operands[0].span.end, first_op_pos.start);
        first_parts.push(comments_before_op);
        first_parts.push(doc::text(" "));
        first_parts.push(doc::text(first_op_str));

        // Build continuation parts (will be indented)
        let mut continuation_parts = Vec::new();

        for i in 1..operands.len() {
            let operand = &operands[i];
            let prev_operand = &operands[i - 1];
            let operator = operators[i - 1];
            let op_str = operator.as_str();
            let op_pos =
                self.find_operator_position(prev_operand.span.end, operand.span.start, op_str);

            // Handle comments after operator and line breaks
            self.append_post_operator_parts(
                &mut continuation_parts,
                op_pos.end,
                prev_operand.span.end,
                operand,
            );

            // Add operator after this operand (if not the last)
            if i < operands.len() - 1 {
                let next_op = operators[i];
                let next_op_str = next_op.as_str();
                let next_op_pos = self.find_operator_position(
                    operand.span.end,
                    operands[i + 1].span.start,
                    next_op_str,
                );

                let next_comments =
                    self.build_inline_comments_between_doc(operand.span.end, next_op_pos.start);
                continuation_parts.push(next_comments);
                continuation_parts.push(doc::text(" "));
                continuation_parts.push(doc::text(next_op_str));
            }
        }

        // Combine: first_parts + indent(continuation_parts)
        doc::group(doc::concat(vec![
            doc::concat(first_parts),
            doc::indent(doc::concat(continuation_parts)),
        ]))
    }

    /// Append post-operator parts (comments and line breaks) to a parts vector
    ///
    /// Handles line comments vs block comments appropriately.
    fn append_post_operator_parts(
        &self,
        parts: &mut Vec<Doc>,
        op_end: u32,
        prev_operand_end: u32,
        operand: &ChainOperand,
    ) {
        let has_line_comment = self.has_line_comments_between(prev_operand_end, operand.span.start);

        if has_line_comment {
            let comment_on_own_line = self.has_newline_before_comment(op_end, operand.span.start);

            if comment_on_own_line {
                // Comment is on its own line: `a &&\n// comment\nb`
                parts.push(doc::hardline());
                let comments_doc = self
                    .build_inline_comments_between_doc_no_leading_space(op_end, operand.span.start);
                parts.push(comments_doc);
                parts.push(doc::hardline());
            } else {
                // Comment after operator: `a && // comment\nb`
                let comments_doc =
                    self.build_inline_comments_between_doc(op_end, operand.span.start);
                parts.push(comments_doc);
                parts.push(doc::hardline());
            }
        } else {
            // Block comments or no comments
            let comments_doc = self.build_inline_comments_between_doc(op_end, operand.span.start);
            parts.push(comments_doc);
            parts.push(doc::line());
        }
        parts.push(operand.doc.clone());
    }

    /// Find operator position between two operands in source
    ///
    /// Returns the start and end positions of the operator string in the source,
    /// which is used to correctly split comments before/after the operator.
    fn find_operator_position(
        &self,
        prev_span_end: u32,
        next_span_start: u32,
        op_str: &str,
    ) -> OperatorPosition {
        let range_start = prev_span_end as usize;
        let range_end = next_span_start as usize;
        let search_range = &self.source[range_start..range_end];
        let op_offset = search_range.find(op_str).unwrap_or(0);
        let op_start = (range_start + op_offset) as u32;
        OperatorPosition {
            start: op_start,
            end: op_start + op_str.len() as u32,
        }
    }

    /// Collect all operands (with spans) and operators from a chain of binary expressions
    ///
    /// Uses `should_flatten()` to determine which operators can be chained together.
    /// Left-associative: recursively flattens left side, keeps right operand as-is.
    fn collect_binary_chain_with_spans(
        &self,
        expr: &internal::BinaryExpression,
        operands: &mut Vec<ChainOperand>,
        operators: &mut Vec<BinaryOperator>,
    ) {
        // Recursively flatten left side if it can be chained with current operator
        if let Expression::BinaryExpression(left_binary) = &*expr.left {
            if expr.operator.can_flatten_with(left_binary.operator) {
                self.collect_binary_chain_with_spans(left_binary, operands, operators);
            } else {
                operands.push(ChainOperand {
                    doc: self.build_binary_operand_doc(&expr.left, expr.operator, false),
                    span: expr.left.span(),
                });
            }
        } else {
            operands.push(ChainOperand {
                doc: self.build_binary_operand_doc(&expr.left, expr.operator, false),
                span: expr.left.span(),
            });
        }

        // Add current operator
        operators.push(expr.operator);

        // Add right operand (don't flatten right side - left-associative)
        operands.push(ChainOperand {
            doc: self.build_binary_operand_doc(&expr.right, expr.operator, true),
            span: expr.right.span(),
        });
    }

    /// Build operand with parens if needed for clarity
    fn build_binary_operand_doc(
        &self,
        operand: &Expression,
        parent_op: BinaryOperator,
        is_right: bool,
    ) -> Doc {
        let ctx = if is_right {
            ParenContext::BinaryRight { parent_op }
        } else {
            ParenContext::BinaryLeft { parent_op }
        };
        let operand_doc = self.build_expression_doc(operand);
        if needs_parens(operand, ctx) {
            doc::parens(operand_doc)
        } else {
            operand_doc
        }
    }

    /// Build a Doc for an await expression
    pub(super) fn build_await_doc(&self, await_expr: &internal::AwaitExpression) -> Doc {
        let argument_doc = if needs_parens(&await_expr.argument, ParenContext::AwaitArgument) {
            doc::concat(vec![
                doc::text("("),
                self.build_expression_doc(&await_expr.argument),
                doc::text(")"),
            ])
        } else {
            self.build_expression_doc(&await_expr.argument)
        };

        doc::concat(vec![doc::text("await "), argument_doc])
    }

    /// Build a Doc for a yield expression
    pub(super) fn build_yield_doc(&self, yield_expr: &internal::YieldExpression) -> Doc {
        let mut parts = Vec::new();

        if yield_expr.delegate {
            parts.push(doc::text("yield*"));
        } else {
            parts.push(doc::text("yield"));
        }

        if let Some(ref arg) = yield_expr.argument {
            parts.push(doc::text(" "));
            parts.push(self.build_expression_doc(arg));
        }

        doc::concat(parts)
    }

    /// Build a Doc for a sequence expression
    pub(super) fn build_sequence_doc(&self, seq: &internal::SequenceExpression) -> Doc {
        let mut parts = Vec::new();
        parts.push(doc::text("("));
        for (i, expr) in seq.expressions.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(", "));
            }
            parts.push(self.build_expression_doc(expr));
        }
        parts.push(doc::text(")"));
        doc::concat(parts)
    }
}
