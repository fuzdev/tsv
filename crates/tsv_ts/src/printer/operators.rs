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

    /// Build binary chain with continuation indent WITHOUT group wrapper
    ///
    /// Use this when the caller controls grouping (e.g., chain printing context).
    /// Handles comments between operands correctly.
    pub(super) fn build_binary_chain_parts_with_continuation_indent(
        &self,
        binary: &internal::BinaryExpression,
    ) -> Doc {
        // Collect all operands (with spans) and operators in the chain
        let mut operands: Vec<ChainOperand> = Vec::new();
        let mut operators = Vec::new();
        self.collect_binary_chain_with_spans(binary, &mut operands, &mut operators);

        if operands.len() <= 1 {
            // Single operand, shouldn't happen but handle gracefully
            return self.build_expression_doc(&binary.left);
        }

        // In Svelte template expressions, use restrictive behavior for short binaries
        // In script contexts, allow breaks for all binaries
        let restrict_short_binaries = self.config.first_line_offset > 0;
        self.build_binary_chain_continuation_indent_parts(
            &operands,
            &operators,
            restrict_short_binaries,
        )
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

    /// Common logic for building binary chain (shared by flat and continuation indent styles)
    ///
    /// Returns (head_parts, continuation_parts) where head includes first operand + operator.
    ///
    /// The `restrict_short_binaries` parameter controls behavior for 2-operand non-logical binaries:
    /// - `true`: Use spaces (no breaks) for these expressions. Used in Svelte template expressions
    ///   where Prettier keeps short binaries like `typeof x === 'string'` on one line.
    /// - `false`: Allow breaks for all binaries. Used in script contexts where Prettier breaks
    ///   long string concat like `'aaa...' + 'bbb...'` at the operator.
    fn build_binary_chain_parts(
        &self,
        operands: &[ChainOperand],
        operators: &[BinaryOperator],
        restrict_short_binaries: bool,
    ) -> (Vec<Doc>, Vec<Doc>) {
        if operands.is_empty() || operands.len() == 1 {
            // Edge cases handled by callers
            return (Vec::new(), Vec::new());
        }

        // First operand + first operator (stays at base indent)
        let mut head_parts = vec![operands[0].doc.clone()];

        let first_op = operators[0];
        let first_op_str = first_op.as_str();

        // Always allow line breaks - the group fitting algorithm decides when to actually
        // break based on print width. Prettier uses `line()` for all binary continuations
        // except for `shouldInlineLogicalExpression` cases (LogicalExpression with
        // object/array/JSX on right), which we don't need special handling for here.
        let allow_breaks = true;
        let _ = restrict_short_binaries; // Parameter kept for API compatibility
        let first_op_pos =
            self.find_operator_position(operands[0].span.end, operands[1].span.start, first_op_str);

        // Comments before first operator
        let comments_before_first_op =
            self.build_inline_comments_between_doc(operands[0].span.end, first_op_pos.start);
        head_parts.push(comments_before_first_op);
        head_parts.push(doc::text(" "));
        head_parts.push(doc::text(first_op_str));

        // Build continuation parts
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
                allow_breaks,
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

        (head_parts, continuation_parts)
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
            return doc::empty();
        }

        if operands.len() == 1 {
            return operands[0].doc.clone();
        }

        // In script contexts (flat style), allow all breaks - print width decides
        let (mut head_parts, continuation_parts) =
            self.build_binary_chain_parts(operands, operators, false);

        // Combine: head + continuation (NO internal indent)
        // The parent context (assignment, etc.) provides the indent wrapper.
        // Nested binaries inside parens get their own group via build_binary_operand_doc.
        head_parts.extend(continuation_parts);

        match style {
            BinaryChainStyle::Grouped => doc::group(doc::concat(head_parts)),
            _ => doc::concat(head_parts),
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
        // In Svelte template expressions (first_line_offset > 0), use restrictive behavior
        doc::group(self.build_binary_chain_continuation_indent_parts(operands, operators, true))
    }

    /// Build binary chain continuation indent parts WITHOUT group wrapper
    ///
    /// Returns the concat of first_parts + indent(continuation_parts) without
    /// wrapping in a group. Use this when the caller controls grouping.
    ///
    /// `restrict_short_binaries`: When true, uses spaces (no breaks) for 2-operand
    /// non-logical binaries. Used in Svelte template expressions. When false, allows
    /// breaks for all binaries based on print width. Used in script contexts.
    fn build_binary_chain_continuation_indent_parts(
        &self,
        operands: &[ChainOperand],
        operators: &[BinaryOperator],
        restrict_short_binaries: bool,
    ) -> Doc {
        let (first_parts, continuation_parts) =
            self.build_binary_chain_parts(operands, operators, restrict_short_binaries);

        // Combine: first_parts + indent(continuation_parts)
        doc::concat(vec![
            doc::concat(first_parts),
            doc::indent(doc::concat(continuation_parts)),
        ])
    }

    /// Append post-operator parts (comments and line breaks) to a parts vector
    ///
    /// Handles line comments vs block comments appropriately.
    /// When `allow_breaks` is true, uses `line()` (space when flat, newline when broken).
    ///
    /// Handles multiple consecutive comments by preserving their line structure:
    /// - `a && // comment1\n// comment2\nb` keeps each comment on its own line
    fn append_post_operator_parts(
        &self,
        parts: &mut Vec<Doc>,
        op_end: u32,
        _prev_operand_end: u32,
        operand: &ChainOperand,
        allow_breaks: bool,
    ) {
        // Collect all comments in the range between operator and next operand
        let comments: Vec<_> =
            tsv_lang::comments_in_range(self.comments, op_end, operand.span.start).collect();

        if comments.is_empty() {
            // No comments - simple case
            if allow_breaks {
                parts.push(doc::line());
            } else {
                parts.push(doc::text(" "));
            }
            parts.push(operand.doc.clone());
            return;
        }

        // Check if any comment is a line comment
        let has_line_comment = comments.iter().any(|c| !c.is_block);

        if !has_line_comment {
            // Only block comments - join them inline
            let comments_doc = self.build_inline_comments_between_doc(op_end, operand.span.start);
            parts.push(comments_doc);
            if allow_breaks {
                parts.push(doc::line());
            } else {
                parts.push(doc::text(" "));
            }
            parts.push(operand.doc.clone());
            return;
        }

        // Has line comments - need to preserve line structure
        // Process each comment individually to maintain proper line breaks
        let mut pos = op_end;
        for (i, comment) in comments.iter().enumerate() {
            let is_first = i == 0;
            let has_newline_before = self.has_newline_between(pos, comment.span.start);

            if is_first && !has_newline_before {
                // First comment on same line as operator: `a && // comment`
                parts.push(doc::text(" "));
                parts.push(self.build_comment_doc(comment));
            } else {
                // Comment on its own line
                parts.push(doc::hardline());
                parts.push(self.build_comment_doc(comment));
            }
            pos = comment.span.end;
        }

        // Add final hardline before operand (since we have line comments)
        parts.push(doc::hardline());
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
    /// Flattens both left and right sides when operators are compatible (e.g., `&&`, `||`).
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

        // Also flatten right side for truly associative operators (removes redundant parens)
        // e.g., `a && (b && c)` becomes `a && b && c`
        // Only logical operators are truly associative; arithmetic preserves right-side parens
        if let Expression::BinaryExpression(right_binary) = &*expr.right
            && expr.operator.can_flatten_with(right_binary.operator)
            && expr.operator.is_logical()
            && right_binary.operator.is_logical()
        {
            self.collect_binary_chain_with_spans(right_binary, operands, operators);
            return;
        }

        // Right operand can't be flattened - add as-is
        operands.push(ChainOperand {
            doc: self.build_binary_operand_doc(&expr.right, expr.operator, true),
            span: expr.right.span(),
        });
    }

    /// Build operand with parens if needed for clarity
    pub(super) fn build_binary_operand_doc(
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

        // For binary expressions that need parens, use continuation indent so that
        // when the inner binary breaks, its continuation lines are indented.
        // This gives: `(first &&\n\t\tsecond)` not `(first &&\n\tsecond)`
        //
        // Context-dependent behavior:
        // - Script contexts (first_line_offset = 0): Use group with parens INSIDE so the
        //   fit calculation includes `)`. This ensures `(A + B) *` at 101 chars breaks
        //   inside the parens, not just at `*`.
        // - Svelte template contexts (first_line_offset > 0): Use the grouped approach
        //   from build_binary_chain_doc_with_continuation_indent, which keeps short
        //   2-operand binaries flat (Prettier's behavior for template expressions).
        if needs_parens(operand, ctx) {
            if let Expression::BinaryExpression(inner_binary) = operand {
                if self.config.first_line_offset > 0 {
                    // Svelte template context: use grouped approach that keeps short binaries flat
                    let inner_doc =
                        self.build_binary_chain_doc_with_continuation_indent(inner_binary);
                    return doc::parens(inner_doc);
                }
                // Script context: include parens in group for proper line width calculation
                let inner_parts =
                    self.build_binary_chain_parts_with_continuation_indent(inner_binary);
                return doc::group(doc::concat(vec![
                    doc::text("("),
                    inner_parts,
                    doc::text(")"),
                ]));
            }
            let operand_doc = self.build_expression_doc(operand);
            doc::parens(operand_doc)
        } else {
            self.build_expression_doc(operand)
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
            // Assignment expressions in sequences need individual parens
            let expr_doc = self.build_expression_doc(expr);
            let expr_doc = if matches!(expr, Expression::AssignmentExpression(_)) {
                doc::parens(expr_doc)
            } else {
                expr_doc
            };
            parts.push(expr_doc);
        }
        parts.push(doc::text(")"));
        doc::concat(parts)
    }
}
