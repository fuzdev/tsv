// Operator expression printing for TypeScript
//
// Handles printing of unary and binary expressions with:
// - Operator precedence and parenthesization
// - Clarity-based parens (mixing logical operators, etc.)

use super::comments::CommentSpacing;
use super::{ParenContext, Printer, needs_parens};
use crate::ast::internal::{self, BinaryOperator, Expression};
use tsv_lang::Span;
use tsv_lang::doc::arena::DocId;

/// Holds information about an operand in a binary expression chain
/// Used to track position information for comment placement
struct ChainOperand {
    doc: DocId,
    span: Span,
}

/// Style for building binary expression chain docs
#[derive(Clone, Copy)]
enum BinaryChainStyle {
    /// Wrapped in a group, flat structure (for standalone binary expressions)
    Grouped,
    /// No group wrapper, flat structure (for contexts where parent controls breaking)
    Ungrouped,
    /// Like Ungrouped, but also suppresses shouldGroup for logical operators.
    /// Used only for condition parentheses (if/while/for/do-while/switch) where
    /// Prettier's `isInsideParenthesis` is true. In these contexts, logical chain
    /// breaks must be controlled by the parent condition group, not a sub-group.
    UngroupedCondition,
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
    pub(super) fn build_update_doc(&self, update: &internal::UpdateExpression) -> DocId {
        let d = self.d();
        let argument_doc = self.build_expression_doc(&update.argument);
        let operator_doc = d.text(update.operator.as_str());

        if update.prefix {
            // Prefix: ++x, --x
            d.concat(&[operator_doc, argument_doc])
        } else {
            // Postfix: x++, x--
            d.concat(&[argument_doc, operator_doc])
        }
    }

    /// Build a Doc for a unary expression
    pub(super) fn build_unary_doc(&self, unary: &internal::UnaryExpression) -> DocId {
        let d = self.d();

        // Check for comments between operator and argument.
        // When grouping parens containing a JSDoc comment are stripped by the parser,
        // the comment ends up in the gap between operator and argument span.
        // Re-add parens to preserve the comment: `!(/** @type {T} */ expr.prop)`
        let operator_end = unary.span.start + unary.operator.as_str().len() as u32;
        let argument_start = unary.argument.span().start;
        let comments_opt = self.build_rhs_comments_opt(operator_end, argument_start);

        let has_line_comment = self.has_line_comments_between(operator_end, argument_start);
        let argument_doc = if let Some(comments) = comments_opt {
            let inner = self.build_expression_doc(&unary.argument);
            if has_line_comment {
                // Line comments need indent + hardline structure:
                // !(\n  // comment\n  expr\n)
                d.concat(&[
                    d.text("("),
                    d.indent(d.concat(&[d.hardline(), comments, inner])),
                    d.hardline(),
                    d.text(")"),
                ])
            } else {
                d.concat(&[d.text("("), comments, inner, d.text(")")])
            }
        } else if needs_parens(&unary.argument, ParenContext::UnaryArgument) {
            // Binary expressions need parens - use grouping for logical ops to allow line breaking
            if let Expression::BinaryExpression(binary) = unary.argument.as_ref() {
                if binary.operator.is_logical() {
                    // Use ungrouped binary chain in a single paren group.
                    // Matches Prettier's `parent.type === "UnaryExpression"` path
                    // (binaryish.js:88-91): `group([indent([softline, ...parts]), softline])`.
                    // The chain's shouldGroup is computed normally: 2-operand chains
                    // get a sub-group (can stay flat at inner indent when paren group
                    // breaks), 3+ chained operands break together with the paren group.
                    let inner = self.build_binary_chain_doc_ungrouped(binary);
                    d.group(d.concat(&[
                        d.text("("),
                        d.indent_softline(inner),
                        d.softline(),
                        d.text(")"),
                    ]))
                } else {
                    d.concat(&[
                        d.text("("),
                        self.build_expression_doc(&unary.argument),
                        d.text(")"),
                    ])
                }
            } else {
                // Non-binary that needs parens (e.g., ternary or assignment in unary/assertion)
                d.concat(&[
                    d.text("("),
                    self.build_expression_doc(&unary.argument),
                    d.text(")"),
                ])
            }
        } else {
            self.build_expression_doc(&unary.argument)
        };

        // Keyword operators need a space before the operand
        if unary.operator.is_keyword_operator() {
            d.concat(&[d.text(unary.operator.as_str()), d.text(" "), argument_doc])
        } else {
            d.concat(&[d.text(unary.operator.as_str()), argument_doc])
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
    pub(super) fn build_binary_doc(&self, binary: &internal::BinaryExpression) -> DocId {
        // Use continuation indent in embedded expression contexts (Svelte template expressions).
        // This matches Prettier where JsExpressionRoot parent triggers the normal indent path
        // (group([head, indent(rest)])) vs the shouldNotIndent path (group(parts)).
        if self.config.is_embedded_expression {
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
    pub(super) fn build_binary_chain_doc(&self, binary: &internal::BinaryExpression) -> DocId {
        self.build_binary_chain_doc_core(binary, BinaryChainStyle::Grouped)
    }

    /// Build a binary chain doc WITHOUT the outer group wrapper
    ///
    /// Use in contexts where the parent group should control breaking
    /// (e.g., !!(), new expression callee, return/throw).
    /// The line() elements will break with the parent group, but shouldGroup
    /// is computed normally — 2-operand chains get a sub-group.
    pub(super) fn build_binary_chain_doc_ungrouped(
        &self,
        binary: &internal::BinaryExpression,
    ) -> DocId {
        self.build_binary_chain_doc_core(binary, BinaryChainStyle::Ungrouped)
    }

    /// Build a binary chain doc for condition parentheses (if/while/for/do-while/switch)
    ///
    /// Like ungrouped, but also suppresses shouldGroup for logical operators so that
    /// logical chain breaks are controlled by the parent condition group.
    /// Matches Prettier's `isInsideParenthesis` behavior (binaryish.js:331).
    pub(super) fn build_binary_chain_doc_ungrouped_condition(
        &self,
        binary: &internal::BinaryExpression,
    ) -> DocId {
        self.build_binary_chain_doc_core(binary, BinaryChainStyle::UngroupedCondition)
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
    ) -> DocId {
        self.build_binary_chain_doc_core(binary, BinaryChainStyle::ContinuationIndent)
    }

    /// Build binary chain with continuation indent WITHOUT group wrapper
    ///
    /// Use this when the caller controls grouping (e.g., chain printing context).
    /// Handles comments between operands correctly.
    pub(super) fn build_binary_chain_parts_with_continuation_indent(
        &self,
        binary: &internal::BinaryExpression,
    ) -> DocId {
        // Collect all operands (with spans) and operators in the chain
        let mut operands: Vec<ChainOperand> = Vec::new();
        let mut operators = Vec::new();
        self.collect_binary_chain_with_spans(binary, &mut operands, &mut operators);

        if operands.len() <= 1 {
            // Single operand, shouldn't happen but handle gracefully
            return self.build_expression_doc(&binary.left);
        }

        let should_inline_last = super::assignment::should_inline_logical_expression(binary);
        let should_group = Self::should_group_binary_continuation(binary);
        self.build_binary_chain_continuation_indent_parts(
            &operands,
            &operators,
            should_inline_last,
            should_group,
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
    ) -> DocId {
        // Collect all operands (with spans) and operators in the chain
        let mut operands: Vec<ChainOperand> = Vec::new();
        let mut operators = Vec::new();
        self.collect_binary_chain_with_spans(binary, &mut operands, &mut operators);

        if operands.len() <= 1 {
            // Single operand, shouldn't happen but handle gracefully
            return self.build_expression_doc(&binary.left);
        }

        // Compute shouldGroup from the original binary expression.
        // This matches Prettier's shouldGroup in printBinaryishExpressions:
        // the continuation gets its own group only when both operand types
        // differ from the current node type (BinaryExpression vs LogicalExpression).
        //
        // In UngroupedCondition mode (if/while/for/do-while/switch conditions),
        // logical operators (&&, ||, ??) must NOT get a sub-group — the parent
        // condition group controls their breaking. This matches Prettier's
        // `isInsideParenthesis` suppression (binaryish.js:331).
        // Without this, `while (a < b && c === d)` keeps the chain flat when
        // the condition group breaks, because the sub-group evaluates fit
        // independently.
        //
        // In plain Ungrouped mode (!!(), new, return/throw), shouldGroup is
        // computed normally — 2-operand chains get a sub-group so they can
        // stay flat when the parent's paren group breaks.
        let should_group = if matches!(style, BinaryChainStyle::UngroupedCondition)
            && binary.operator.is_logical()
        {
            false
        } else {
            Self::should_group_binary_continuation(binary)
        };

        // shouldInlineLogicalExpression: when the outermost logical has a non-empty
        // object/array on the right, keep operator and RHS on the same line.
        // Prettier ref: binaryish.js:275, 361
        let should_inline_last = super::assignment::should_inline_logical_expression(binary);

        // For ContinuationIndent, we separate first operand from the rest
        // For other styles, we build a flat parts list
        match style {
            BinaryChainStyle::ContinuationIndent => self.build_binary_chain_continuation_indent(
                &operands,
                &operators,
                should_inline_last,
                should_group,
            ),
            _ => self.build_binary_chain_flat(
                &operands,
                &operators,
                style,
                should_group,
                should_inline_last,
            ),
        }
    }

    /// Check if the binary continuation should be wrapped in its own group.
    ///
    /// Matches Prettier's `shouldGroup` in `printBinaryishExpressions`:
    /// - Returns true when both left and right operands are a different AST type
    ///   category than the current node (BinaryExpression vs LogicalExpression).
    /// - In ESTree, `+`, `*`, etc. are BinaryExpression while `&&`, `||`, `??`
    ///   are LogicalExpression. We use `is_logical()` to distinguish these categories.
    ///
    /// When shouldGroup is true, the continuation gets its own group, allowing it
    /// to independently evaluate whether it fits on the current line when the outer
    /// group breaks (e.g., due to a multi-line parenthesized left operand).
    pub(super) fn should_group_binary_continuation(binary: &internal::BinaryExpression) -> bool {
        let current_is_logical = binary.operator.is_logical();

        // Check if left operand is same AST type category
        let left_is_same_category = matches!(
            &*binary.left,
            Expression::BinaryExpression(inner) if inner.operator.is_logical() == current_is_logical
        );

        // Check if right operand is same AST type category
        let right_is_same_category = matches!(
            &*binary.right,
            Expression::BinaryExpression(inner) if inner.operator.is_logical() == current_is_logical
        );

        // shouldGroup when NEITHER operand is the same category
        !left_is_same_category && !right_is_same_category
    }

    /// Common logic for building binary chain (shared by flat and continuation indent styles)
    ///
    /// Returns (head_parts, continuation_parts) where head includes first operand + operator.
    ///
    /// When `should_inline_last` is true (shouldInlineLogicalExpression), the last operand
    /// uses a space instead of `line()`, keeping operator and RHS on the same line so the
    /// object/array can self-expand. Prettier ref: binaryish.js:275, 361
    fn build_binary_chain_parts(
        &self,
        operands: &[ChainOperand],
        operators: &[BinaryOperator],
        should_inline_last: bool,
    ) -> (Vec<DocId>, Vec<DocId>) {
        let d = self.d();
        if operands.is_empty() || operands.len() == 1 {
            // Edge cases handled by callers
            return (Vec::new(), Vec::new());
        }

        // First operand + first operator (stays at base indent)
        let mut head_parts = vec![operands[0].doc];

        let first_op = operators[0];
        let first_op_str = first_op.as_str();

        let first_op_pos =
            self.find_operator_position(operands[0].span.end, operands[1].span.start, first_op_str);

        // Comments before first operator
        let comments_before_first_op =
            self.build_inline_comments_between_doc(operands[0].span.end, first_op_pos.start);
        head_parts.push(comments_before_first_op);
        head_parts.push(d.text(" "));
        head_parts.push(d.text(first_op_str));

        // Build continuation parts
        let mut continuation_parts = Vec::new();

        for i in 1..operands.len() {
            let operand = &operands[i];
            let prev_operand = &operands[i - 1];
            let operator = operators[i - 1];
            let op_str = operator.as_str();
            let op_pos =
                self.find_operator_position(prev_operand.span.end, operand.span.start, op_str);

            // shouldInlineLogicalExpression: the last operand (non-empty object/array)
            // uses a space instead of line(), keeping operator and RHS on the same line.
            let allow_breaks = !(i == operands.len() - 1 && should_inline_last);

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
                continuation_parts.push(d.text(" "));
                continuation_parts.push(d.text(next_op_str));
            }
        }

        (head_parts, continuation_parts)
    }

    /// Build a flat binary chain (Grouped or Ungrouped style)
    ///
    /// Matches Prettier's binaryish.js structure:
    /// - First operand + first operator at base indent (head)
    /// - Continuation (line + remaining operands) optionally in a sub-group
    ///
    /// When `should_group` is true (operand types differ from current node,
    /// e.g., `(LogicalExpr) + d`), the continuation gets its own group so it
    /// can independently evaluate fit when the outer group breaks due to a
    /// multi-line left operand. When false (same category, e.g., `(a+b)*c`),
    /// continuation breaks with the outer group.
    fn build_binary_chain_flat(
        &self,
        operands: &[ChainOperand],
        operators: &[BinaryOperator],
        style: BinaryChainStyle,
        should_group: bool,
        should_inline_last: bool,
    ) -> DocId {
        let d = self.d();
        if operands.is_empty() {
            return d.empty();
        }

        if operands.len() == 1 {
            return operands[0].doc;
        }

        let (mut head_parts, continuation_parts) =
            self.build_binary_chain_parts(operands, operators, should_inline_last);

        if !continuation_parts.is_empty() {
            if should_group {
                // Sub-group: continuation evaluates fit independently
                head_parts.push(d.group(d.concat(&continuation_parts)));
            } else {
                // No sub-group: continuation breaks with outer group
                head_parts.extend(continuation_parts);
            }
        }

        match style {
            BinaryChainStyle::Grouped => d.group(d.concat(&head_parts)),
            _ => d.concat(&head_parts),
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
        should_inline_last: bool,
        should_group: bool,
    ) -> DocId {
        let d = self.d();
        d.group(self.build_binary_chain_continuation_indent_parts(
            operands,
            operators,
            should_inline_last,
            should_group,
        ))
    }

    /// Build binary chain continuation indent parts WITHOUT group wrapper.
    ///
    /// Returns the concat of first_parts + indent(continuation_parts) without
    /// wrapping in a group. Used in Svelte template expressions and when the
    /// caller controls grouping.
    ///
    /// When `should_group` is true, wraps the continuation in a sub-group so it
    /// can independently evaluate fit (bypassing the renderer's `will_break` check
    /// on the outer group).
    fn build_binary_chain_continuation_indent_parts(
        &self,
        operands: &[ChainOperand],
        operators: &[BinaryOperator],
        should_inline_last: bool,
        should_group: bool,
    ) -> DocId {
        let d = self.d();
        let (first_parts, continuation_parts) =
            self.build_binary_chain_parts(operands, operators, should_inline_last);

        // When should_group is true, wrap the continuation in its own group so it
        // can independently evaluate fit. Without this, the renderer's will_break()
        // check on the outer group sees hardlines in the left operand (e.g., a
        // multi-line call expression) and forces the entire group to Break mode,
        // even when the continuation (e.g., `?? 'text'`) fits on the closing line.
        //
        // When should_inline_last is true, skip indent entirely — matching prettier's
        // early return of group(parts) with no indent wrapper (binaryish.js:131-134).
        // The inlined last operand (object/array) handles its own indentation.
        let continuation_doc = if should_inline_last {
            d.concat(&continuation_parts)
        } else {
            d.indent(d.concat(&continuation_parts))
        };
        let continuation_doc = if should_group {
            d.group(continuation_doc)
        } else {
            continuation_doc
        };

        d.concat(&[d.concat(&first_parts), continuation_doc])
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
        parts: &mut Vec<DocId>,
        op_end: u32,
        _prev_operand_end: u32,
        operand: &ChainOperand,
        allow_breaks: bool,
    ) {
        let d = self.d();
        // Collect all comments in the range between operator and next operand
        let comments: Vec<_> =
            tsv_lang::comments_in_range(self.comments, op_end, operand.span.start).collect();

        if comments.is_empty() {
            // No comments - simple case
            if allow_breaks {
                parts.push(d.line());
            } else {
                parts.push(d.text(" "));
            }
            parts.push(operand.doc);
            return;
        }

        // Check if any comment is a line comment
        let has_line_comment = comments.iter().any(|c| !c.is_block);

        if !has_line_comment {
            // Only block comments - place as leading on RHS operand.
            // In flat mode: `a || /* comment */ b` (space from line(), comment+trailing space, operand)
            // In break mode: `a ||\n<indent>/* comment */ b` (comment leads continuation line)
            let comments_doc =
                self.build_comments_between(op_end, operand.span.start, CommentSpacing::Trailing);
            if allow_breaks {
                parts.push(d.line());
            } else {
                parts.push(d.text(" "));
            }
            parts.push(comments_doc);
            parts.push(operand.doc);
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
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else {
                // Comment on its own line
                parts.push(d.hardline());
                parts.push(self.build_comment_doc(comment));
            }
            pos = comment.span.end;
        }

        // Add final hardline before operand (since we have line comments)
        parts.push(d.hardline());
        parts.push(operand.doc);
    }

    /// Find operator position between two operands in source
    ///
    /// Returns the start and end positions of the operator string in the source,
    /// which is used to correctly split comments before/after the operator.
    /// Skips over comments to avoid matching operators inside them.
    fn find_operator_position(
        &self,
        prev_span_end: u32,
        next_span_start: u32,
        op_str: &str,
    ) -> OperatorPosition {
        let range_start = prev_span_end as usize;
        let range_end = next_span_start as usize;
        let bytes = self.source.as_bytes();
        let op_bytes = op_str.as_bytes();
        let op_len = op_bytes.len();
        let mut i = range_start;

        while i + op_len <= range_end {
            // Skip comments
            if let Some(new_i) = super::analysis::skip_comment(bytes, i, range_end) {
                i = new_i;
                continue;
            }
            // Check for operator match
            if &bytes[i..i + op_len] == op_bytes {
                return OperatorPosition {
                    start: i as u32,
                    end: (i + op_len) as u32,
                };
            }
            i += 1;
        }
        // Fallback (shouldn't happen in valid code)
        OperatorPosition {
            start: prev_span_end,
            end: prev_span_end + op_str.len() as u32,
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
    ) -> DocId {
        let d = self.d();
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
        // - Script contexts (is_embedded_expression = false): Use group with parens INSIDE
        //   so the fit calculation includes `)`. This ensures `(A + B) *` at 101 chars
        //   breaks inside the parens, not just at `*`.
        // - Embedded expression contexts (is_embedded_expression = true): Use the grouped
        //   approach from build_binary_chain_doc_with_continuation_indent, which keeps
        //   short 2-operand binaries flat (Prettier's behavior for template expressions).
        if needs_parens(operand, ctx) {
            if let Expression::BinaryExpression(inner_binary) = operand {
                if self.config.is_embedded_expression {
                    // Embedded expression context: use grouped approach that keeps short binaries flat
                    let inner_doc =
                        self.build_binary_chain_doc_with_continuation_indent(inner_binary);
                    return d.parens(inner_doc);
                }
                // Script context: include parens in group for proper line width calculation
                let inner_parts =
                    self.build_binary_chain_parts_with_continuation_indent(inner_binary);
                return d.group(d.concat(&[d.text("("), inner_parts, d.text(")")]));
            }
            let operand_doc = self.build_expression_doc(operand);
            d.parens(operand_doc)
        } else if let Expression::BinaryExpression(inner_binary) = operand {
            // Nested binary sub-expressions use continuation indent.
            // Prettier's shouldNotIndent (binaryish.js:96-115) evaluates to false when
            // parent is BinaryExpression (none of the conditions match), so the inner
            // chain gets indent(rest). E.g., `0.5 * a(...) * b(...)` inside `... + 1.0`
            // indents the `*` continuation lines relative to `0.5`.
            self.build_binary_chain_doc_with_continuation_indent(inner_binary)
        } else {
            self.build_expression_doc(operand)
        }
    }

    /// Build a Doc for an await expression
    pub(super) fn build_await_doc(&self, await_expr: &internal::AwaitExpression) -> DocId {
        let d = self.d();

        // Preserve comments from stripped grouping parens: `await (/** @type {T} */ expr)`
        let keyword_end = await_expr.span.start + "await".len() as u32;
        let argument_start = await_expr.argument.span().start;
        let comments_opt = self.build_rhs_comments_opt(keyword_end, argument_start);

        let argument_doc = if let Some(comments) = comments_opt {
            let inner = self.build_expression_doc(&await_expr.argument);
            d.concat(&[comments, inner])
        } else if needs_parens(&await_expr.argument, ParenContext::AwaitArgument) {
            d.concat(&[
                d.text("("),
                self.build_expression_doc(&await_expr.argument),
                d.text(")"),
            ])
        } else {
            self.build_expression_doc(&await_expr.argument)
        };

        d.concat(&[d.text("await "), argument_doc])
    }

    /// Build a Doc for a yield expression
    pub(super) fn build_yield_doc(&self, yield_expr: &internal::YieldExpression) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        if yield_expr.delegate {
            parts.push(d.text("yield*"));
        } else {
            parts.push(d.text("yield"));
        }

        if let Some(ref arg) = yield_expr.argument {
            parts.push(d.text(" "));
            // Preserve comments from stripped grouping parens: `yield (/** @type {T} */ expr)`
            let keyword_end = yield_expr.span.start
                + if yield_expr.delegate {
                    "yield*"
                } else {
                    "yield"
                }
                .len() as u32;
            let argument_start = arg.span().start;
            if let Some(comments) = self.build_rhs_comments_opt(keyword_end, argument_start) {
                parts.push(comments);
            }
            parts.push(self.build_expression_doc(arg));
        }

        d.concat(&parts)
    }

    /// Build a Doc for a sequence expression
    pub(super) fn build_sequence_doc(&self, seq: &internal::SequenceExpression) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        parts.push(d.text("("));
        for (i, expr) in seq.expressions.iter().enumerate() {
            if i > 0 {
                parts.push(d.text(", "));
            }
            // Assignment expressions in sequences need individual parens
            let expr_doc = self.build_expression_doc(expr);
            let expr_doc = if matches!(expr, Expression::AssignmentExpression(_)) {
                d.parens(expr_doc)
            } else {
                expr_doc
            };
            parts.push(expr_doc);
        }
        parts.push(d.text(")"));
        d.concat(&parts)
    }
}
