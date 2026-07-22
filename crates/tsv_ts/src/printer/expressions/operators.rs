// Operator expression printing for TypeScript
//
// Handles printing of unary and binary expressions with:
// - Operator precedence and parenthesization
// - Clarity-based parens (mixing logical operators, etc.)

use crate::ast::internal::{self, BinaryOperator, Expression};
use crate::printer::comments::CommentSpacing;
use crate::printer::{CommentVec, ParenContext, Printer};
use smallvec::{SmallVec, smallvec};
use tsv_lang::Span;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::{DocBuf, arena::DocId};

/// Holds information about an operand in a binary expression chain
/// Used to track position information for comment placement
struct ChainOperand {
    doc: DocId,
    span: Span,
}

/// Stack buffers for a flattened binary chain's operands / operators, collected
/// once per binary expression. The common 2–3 operand chain stays inline
/// (`ChainOperand` is 12 bytes, `BinaryOperator` 1 byte); longer chains spill.
type OperandBuf = SmallVec<[ChainOperand; 8]>;
pub(super) type OperatorBuf = SmallVec<[BinaryOperator; 8]>;

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
#[derive(Clone, Copy)]
struct OperatorPosition {
    /// Start position of operator in source
    start: u32,
    /// End position of operator in source (start + operator length)
    end: u32,
}

impl<'a> Printer<'a> {
    /// Build a Doc for an update expression
    pub(in crate::printer) fn build_update_doc(
        &self,
        update: &internal::UpdateExpression<'_>,
    ) -> DocId {
        let d = self.d();
        let argument_doc = self.build_expression_doc(update.argument);
        // A type-assertion operand keeps its parens: `(a as T)++` (bare
        // `a as T++` binds `++` to `T`).
        let argument_doc = if self.needs_parens(update.argument, ParenContext::UpdateArgument) {
            d.parens(argument_doc)
        } else {
            argument_doc
        };
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
    pub(in crate::printer) fn build_unary_doc(
        &self,
        unary: &internal::UnaryExpression<'_>,
    ) -> DocId {
        let d = self.d();

        // Check for comments between operator and argument.
        // When grouping parens containing a JSDoc comment are stripped by the parser,
        // the comment ends up in the gap between operator and argument span.
        // Re-add parens to preserve the comment: `!(/** @type {T} */ expr.prop)`
        let operator_end = unary.span.start + unary.operator.as_str().len() as u32;
        let argument_start = unary.argument.span().start;
        let argument_end = unary.argument.span().end;
        // A single-line block glued to the operator hugs the operand even across a
        // source newline (`!/* c */⏎x` → `!(/* c */ x)`), matching prettier.
        let leading_comments_opt = self.build_rhs_comments_glued_opt(operator_end, argument_start);
        // Whether a leading comment is *present* — the gate for re-adding the parens — as
        // opposed to whether this emitter has to print it. A **forward-binding** comment (a
        // bundler annotation, a JSDoc cast) is `owned_by_node`, so the operand's own doc
        // prints it and `leading_comments_opt` is `None` — but the parens are still wanted,
        // for the same reason an ordinary gap comment wants them: bare,
        // `!/* @__PURE__ */ f()` reads as annotating the *operator* rather than the operand.
        // Counting the owned comment here is what keeps the wrap, and it is the ONLY thing
        // that does — `needs_parens` deliberately doesn't (it would double-wrap).
        //
        // This is the `has_comments_on_page_in_range` / `has_comments_to_emit_in_range` split: an
        // emit-decision skips owned comments, a layout/semantic gate counts them.
        //
        // An operand that already prints its own value-position pair is the exception:
        // that pair encloses the owned comment on the plain needs-parens path below
        // (`!(/** @type {A} */ (x), y)`, `!(/* c */ x = y)`, `!(/* c */ cond ? b : c)`),
        // so counting the owned comment here would drive the comment-holder wrap and
        // double the parens (`!((/* c */ x = y))`). Skipping it routes the operand to that
        // plain path, where its own single pair encloses the owned comment (prepended by
        // `build_expression_doc`). That's any `needs_parens` operand — assignment, a
        // conditional, an arrow, await/yield, a type assertion — plus a sequence, whose
        // own printer supplies the pair (`needs_parens` reports false for it). Binary is
        // deliberately NOT here: its plain path renders through
        // `build_binary_chain_doc_ungrouped`, which does not prepend the owned comment, so
        // binary must take the comment-holder path (where `needs_paren_wrap` excludes it —
        // one pair either way). A *trailing* comment on such an operand still keeps both
        // pairs (the comment-holder path via `has_trailing_comments`) — that is the
        // deliberate `!((x = y) /* c */)` form, pinned by `operand_paren_comment`.
        // Asked three times below (the exception here, the comment-holder inner wrap,
        // and the plain path), always with the same operand + context — compute once.
        let arg_needs_parens = self.needs_parens(
            unary.argument,
            ParenContext::UnaryArgument {
                parent_op: unary.operator,
            },
        );
        let operand_encloses_owned_comment =
            matches!(unary.argument, Expression::SequenceExpression(_))
                || (arg_needs_parens && !matches!(unary.argument, Expression::BinaryExpression(_)));
        let owned_leading_comment = leading_comments_opt.is_none()
            && !operand_encloses_owned_comment
            && tsv_lang::has_comments_on_page_in_range(self.comments, operator_end, argument_start);
        let has_leading_comments = leading_comments_opt.is_some() || owned_leading_comment;

        // Check for trailing comments after the argument but inside the original parens.
        // When the parser strips grouping parens from `!(x /* c */)`, the comment
        // between argument end and unary span end is lost if we don't re-add parens.
        let has_trailing_comments = self.has_comments_to_emit_between(argument_end, unary.span.end);

        // Determine if multiline layout is needed: line comments force newlines,
        // and block comments on their own line (newline in source) preserve structure.
        // For trailing comments, check if the comment itself is on a different line
        // from the argument (not just whether there's a newline in the whole range,
        // which could be between the comment and the closing paren).
        let has_own_line_trailing_comment = self
            .comments_on_page_between(argument_end, unary.span.end)
            .any(|c| !c.is_block || self.has_newline_between(argument_end, c.span.start));
        // A line comment is already caught by `has_line_comments_between` above; here
        // a leading block forces the multiline layout only when it can't glue inline
        // (multiline, or own-line with a newline before it).
        let needs_multiline = self.has_line_comments_between(operator_end, argument_start)
            || has_own_line_trailing_comment
            || self
                .comments_on_page_between(operator_end, argument_start)
                .any(|c| self.comment_cannot_glue_to_operator(c));

        let argument_doc = if has_leading_comments || has_trailing_comments {
            // Comments inside grouping parens — must wrap in parens to preserve them.
            let inner = self.build_expression_doc(unary.argument);
            // The outer comment-holder parens already group the operand, so the inner
            // needs_parens layer is redundant for a binary/logical operand — prettier
            // strips it (`!(x + y /* c */)`). Assignment/ternary operands keep their
            // parens for clarity in both formatters, so leave those untouched.
            let needs_paren_wrap =
                arg_needs_parens && !matches!(unary.argument, Expression::BinaryExpression(_));
            let inner = if needs_paren_wrap {
                d.parens(inner)
            } else {
                inner
            };
            if needs_multiline {
                // Multiline layout: !(\n  /* c */\n  expr\n) or !(\n  expr // c\n)
                let mut indent_parts: DocBuf = smallvec![d.hardline()];
                // Leading comments between operator and operand: same per-line
                // block/line layout (blank-preserving hardlines, inline blocks hugged)
                // as the inline path. Non-gluing here — the layout is already vertical,
                // so a single-line block keeps the author's break to the next line.
                if let Some(leading) = self.build_rhs_comments_opt(operator_end, argument_start) {
                    indent_parts.push(leading);
                }
                indent_parts.push(inner);
                // Add trailing comments with appropriate spacing
                for comment in
                    comments_to_emit_in_range(self.comments, argument_end, unary.span.end)
                {
                    if !comment.is_block
                        || !self.has_newline_between(argument_end, comment.span.start)
                    {
                        // Line comment or block comment on same line as argument
                        indent_parts.push(d.text(" "));
                        indent_parts.push(self.build_comment_doc(comment));
                    } else {
                        // Block comment on its own line
                        indent_parts.push(d.hardline());
                        indent_parts.push(self.build_comment_doc(comment));
                    }
                }
                d.concat(&[
                    d.text("("),
                    d.indent(d.concat(&indent_parts)),
                    d.hardline(),
                    d.text(")"),
                ])
            } else {
                // Inline layout: !(/* c */ expr) or !(expr /* c */)
                let mut parts = DocBuf::new();
                parts.push(d.text("("));
                if let Some(leading) = leading_comments_opt {
                    parts.push(leading);
                }
                parts.push(inner);
                // Trailing block comments inline: `expr /* c */`
                for comment in
                    comments_to_emit_in_range(self.comments, argument_end, unary.span.end)
                {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
                parts.push(d.text(")"));
                d.concat(&parts)
            }
        } else if arg_needs_parens {
            // Binary expressions need parens - grouping lets the parens expand when the arg is long
            if let Expression::BinaryExpression(binary) = unary.argument {
                // Wrap any binaryish arg (logical or not) in a single paren group.
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
                // Non-binary that needs parens (e.g., ternary or assignment in unary/assertion)
                d.concat(&[
                    d.text("("),
                    self.build_expression_doc(unary.argument),
                    d.text(")"),
                ])
            }
        } else {
            self.build_expression_doc(unary.argument)
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
    pub(in crate::printer) fn build_binary_doc(
        &self,
        binary: &internal::BinaryExpression<'_>,
    ) -> DocId {
        // Use continuation indent in embedded expression contexts (Svelte template expressions).
        // This matches Prettier where JsExpressionRoot parent triggers the normal indent path
        // (group([head, indent(rest)])) vs the shouldNotIndent path (group(parts)).
        if self.embed.is_embedded() {
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
    pub(in crate::printer) fn build_binary_chain_doc(
        &self,
        binary: &internal::BinaryExpression<'_>,
    ) -> DocId {
        self.build_binary_chain_doc_core(binary, BinaryChainStyle::Grouped)
    }

    /// Build a binary chain doc WITHOUT the outer group wrapper
    ///
    /// Use in contexts where the parent group should control breaking
    /// (e.g., !!(), new expression callee, return/throw).
    /// The line() elements will break with the parent group, but shouldGroup
    /// is computed normally — 2-operand chains get a sub-group.
    pub(in crate::printer) fn build_binary_chain_doc_ungrouped(
        &self,
        binary: &internal::BinaryExpression<'_>,
    ) -> DocId {
        self.build_binary_chain_doc_core(binary, BinaryChainStyle::Ungrouped)
    }

    /// Build a binary chain doc for condition parentheses (if/while/for/do-while/switch)
    ///
    /// Like ungrouped, but also suppresses shouldGroup for logical operators so that
    /// logical chain breaks are controlled by the parent condition group.
    /// Matches Prettier's `isInsideParenthesis` behavior (binaryish.js:331).
    pub(in crate::printer) fn build_binary_chain_doc_ungrouped_condition(
        &self,
        binary: &internal::BinaryExpression<'_>,
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
    pub(in crate::printer) fn build_binary_chain_doc_with_continuation_indent(
        &self,
        binary: &internal::BinaryExpression<'_>,
    ) -> DocId {
        self.build_binary_chain_doc_core(binary, BinaryChainStyle::ContinuationIndent)
    }

    /// Build binary chain with continuation indent WITHOUT group wrapper
    ///
    /// Use this when the caller controls grouping (e.g., chain printing context).
    /// Handles comments between operands correctly.
    pub(in crate::printer) fn build_binary_chain_parts_with_continuation_indent(
        &self,
        binary: &internal::BinaryExpression<'_>,
    ) -> DocId {
        // Collect all operands (with spans) and operators in the chain
        let mut operands: OperandBuf = OperandBuf::new();
        let mut operators: OperatorBuf = OperatorBuf::new();
        self.collect_binary_chain_with_spans(binary, &mut operands, &mut operators);

        if operands.len() <= 1 {
            // Single operand, shouldn't happen but handle gracefully
            return self.build_expression_doc(binary.left);
        }

        let should_inline_last = super::assignment::should_inline_logical_expression(binary);
        let should_group = Self::should_group_binary_continuation(binary);
        let chain = self.build_binary_chain_continuation_indent_parts(
            &operands,
            &operators,
            should_inline_last,
            should_group,
        );
        self.wrap_chain_with_paren_comments(binary, &operands, chain)
    }

    /// Core implementation for binary chain doc building
    ///
    /// Handles three styles:
    /// - `Grouped`: Wrapped in a group, flat structure (standalone binary expressions)
    /// - `Ungrouped`: No group wrapper, flat structure (conditions where parent controls breaking)
    /// - `ContinuationIndent`: First operand at base, rest indented (attribute contexts)
    fn build_binary_chain_doc_core(
        &self,
        binary: &internal::BinaryExpression<'_>,
        style: BinaryChainStyle,
    ) -> DocId {
        // Collect all operands (with spans) and operators in the chain
        let mut operands: OperandBuf = OperandBuf::new();
        let mut operators: OperatorBuf = OperatorBuf::new();
        self.collect_binary_chain_with_spans(binary, &mut operands, &mut operators);

        if operands.len() <= 1 {
            // Single operand, shouldn't happen but handle gracefully
            return self.build_expression_doc(binary.left);
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
        let chain = match style {
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
        };
        self.wrap_chain_with_paren_comments(binary, &operands, chain)
    }

    /// Wrap a binary chain doc with comments from stripped grouping parens.
    ///
    /// When the parser strips parens like `(/* l */ a + b /* t */)`, the
    /// comments are orphaned in the gaps between `binary.span` and the outer
    /// operand spans. Without this, those comments are silently dropped — a
    /// SAFETY violation.
    ///
    /// Leading comments (between `binary.span.start` and the leftmost operand)
    /// are prepended via `prepend_removed_paren_comments`. Trailing comments
    /// (between the rightmost operand and `binary.span.end`) emit inline for
    /// same-line blocks (` /* t */`) and via `line_suffix` for line/own-line
    /// comments (so they defer past any enclosing semicolon).
    fn wrap_chain_with_paren_comments(
        &self,
        binary: &internal::BinaryExpression<'_>,
        operands: &[ChainOperand],
        chain: DocId,
    ) -> DocId {
        let Some(leftmost_start) = operands.first().map(|o| o.span.start) else {
            return chain;
        };
        let Some(rightmost_end) = operands.last().map(|o| o.span.end) else {
            return chain;
        };

        let with_leading =
            self.prepend_removed_paren_comments(binary.span.start, leftmost_start, chain);

        if rightmost_end >= binary.span.end {
            return with_leading;
        }
        let mut parts = smallvec![with_leading];
        self.append_trailing_paren_comments(&mut parts, rightmost_end, binary.span.end);
        // `concat` short-circuits the no-trailing-comment case (`[with_leading]`).
        self.d().concat(&parts)
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
    pub(in crate::printer) fn should_group_binary_continuation(
        binary: &internal::BinaryExpression<'_>,
    ) -> bool {
        let current_is_logical = binary.operator.is_logical();

        // Check if left operand is same AST type category
        let left_is_same_category = matches!(
            binary.left,
            Expression::BinaryExpression(inner) if inner.operator.is_logical() == current_is_logical
        );

        // Check if right operand is same AST type category
        let right_is_same_category = matches!(
            binary.right,
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
    ) -> (DocBuf, DocBuf) {
        if operands.is_empty() || operands.len() == 1 {
            // Edge cases handled by callers
            return (DocBuf::new(), DocBuf::new());
        }

        // Whole-chain comment presence gate (idiom 8): one on-page lookup over the chain's
        // whole span lets the per-gap emitters below skip their per-gap comment scans for the
        // ~all chains that hold no comment anywhere. A *presence* flag (on-page counts owned),
        // so it fails open — it can only add work on a commented chain, never suppress a
        // comment (the perf80 hazard). Every operand→operator / operator→operand gap the
        // emitters scan lies within `[first operand start, last operand end]`.
        let chain_has_comments = self.has_comments_on_page_between(
            operands[0].span.start,
            operands[operands.len() - 1].span.end,
        );

        // First operand + first operator (stays at base indent)
        let mut head_parts: DocBuf = smallvec![operands[0].doc];

        let first_op_str = operators[0].as_str();

        let first_op_pos =
            self.find_operator_position(operands[0].span.end, operands[1].span.start, first_op_str);

        // operand[0] → first operator. A line comment in this gap would swallow the
        // operator if emitted inline; the helper keeps it trailing the operand and
        // reports whether it forced the operator onto the next line.
        let mut prev_forced_break = self.push_operand_operator_gap(
            &mut head_parts,
            operands[0].span.end,
            first_op_pos.start,
            first_op_str,
            chain_has_comments,
        );

        // Build continuation parts
        let mut continuation_parts: DocBuf = DocBuf::new();

        // The operand[i-1]→operand[i] operator gap is located once and carried across
        // iterations: iteration i's leading gap is either the first gap (i == 1) or the
        // trailing gap the previous iteration already scanned, so it is never re-scanned.
        let mut op_pos = first_op_pos;

        for i in 1..operands.len() {
            let operand = &operands[i];

            // shouldInlineLogicalExpression: the last operand (non-empty object/array)
            // uses a space instead of line(), keeping operator and RHS on the same line.
            let allow_breaks = !(i == operands.len() - 1 && should_inline_last);

            // When the previous operand→operator gap forced a break, the operator now
            // leads this operand on the same line, so hug it with a space (not a line).
            self.append_post_operator_parts(
                &mut continuation_parts,
                op_pos.end,
                operand,
                allow_breaks,
                prev_forced_break,
                chain_has_comments,
            );

            // operand[i] → next operator (if not last operand)
            if i < operands.len() - 1 {
                let next_op_str = operators[i].as_str();
                let next_op_pos = self.find_operator_position(
                    operand.span.end,
                    operands[i + 1].span.start,
                    next_op_str,
                );

                prev_forced_break = self.push_operand_operator_gap(
                    &mut continuation_parts,
                    operand.span.end,
                    next_op_pos.start,
                    next_op_str,
                    chain_has_comments,
                );

                // Carry this trailing gap forward as the next iteration's leading gap.
                op_pos = next_op_pos;
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

    /// Emit a binary chain's operand→operator gap, returning whether a line comment
    /// in the gap forced the operator onto the next line.
    ///
    /// Without a line comment the gap renders inline as it always has
    /// (`operand <inline block comments> operator`). With a line comment, emitting it
    /// inline would run to end-of-line and **swallow the operator**
    /// (`1 // c⏎+ 2` → `1 // c + 2`, the `+ 2` absorbed into the comment — content
    /// loss). Instead the comment is kept trailing the operand where the author wrote
    /// it — the first, on the operand's own line, via `line_suffix` (zero width); any
    /// later ones on their own line — and a hardline then forces the operator down to
    /// hug its right operand (`1 // c⏎+ 2`). Returns `true` in that case so the caller
    /// hugs the following operand with a space rather than a breakable line (avoiding
    /// the `1 // c⏎+⏎2` over-break). Prettier instead relocates the comment past the
    /// operator; see conformance_prettier.md §Comment relocation.
    fn push_operand_operator_gap(
        &self,
        parts: &mut DocBuf,
        operand_end: u32,
        op_start: u32,
        op_str: &'static str,
        chain_has_comments: bool,
    ) -> bool {
        let d = self.d();

        // Zero-comment fast path: the operand→operator gap holds no comment (the
        // ubiquitous case), so emit just the operator — no empty comment node in the
        // parts concat, and no per-gap comment scan at all. Byte-identical: the gap is
        // comment-free, so the general path below would build `empty()` here (renders to
        // nothing). The gap ⊆ the binary span, so this can only skip work, never a comment.
        // The whole-chain gate short-circuits the per-gap scan when the chain is
        // comment-free (`chain_has_comments` false ⇒ this gap holds none to emit either).
        if !chain_has_comments || !self.has_comments_to_emit_between(operand_end, op_start) {
            parts.push(d.text(" "));
            parts.push(d.text(op_str));
            return false;
        }

        if !self.has_line_comments_between(operand_end, op_start) {
            // No line comment — inline gap (block comments stay inline, as before).
            parts.push(self.build_inline_comments_between_doc(operand_end, op_start));
            parts.push(d.text(" "));
            parts.push(d.text(op_str));
            return false;
        }

        // Keep each comment where the author wrote it, then break before the operator.
        let mut pos = operand_end;
        for (i, comment) in
            comments_to_emit_in_range(self.comments, operand_end, op_start).enumerate()
        {
            if i == 0 && !self.comment_has_newline_between(pos, comment.span.start) {
                // On the operand's line (`1 // c`): trail via `line_suffix` (zero width)
                // so a long comment never forces the preceding operand group to break.
                parts.push(self.build_trailing_comment_doc(comment));
            } else {
                // On its own line — preserve an author blank line before it.
                self.push_blank_preserving_hardline(parts, pos, comment.span.start);
                parts.push(self.build_comment_doc(comment));
            }
            pos = comment.span.end;
        }

        parts.push(d.hardline());
        parts.push(d.text(op_str));
        true
    }

    /// Append post-operator parts (comments and line breaks) to a parts vector
    ///
    /// Handles line comments vs block comments appropriately.
    /// When `allow_breaks` is true, uses `line()` (space when flat, newline when broken).
    /// When `lead_with_space` is true, the leading separator is a hard space instead of a
    /// breakable line — used when the previous operand→operator gap forced a break, so the
    /// operator now leads this operand on the same line (`1 // c⏎+ 2`, not `1 // c⏎+⏎2`).
    ///
    /// Handles multiple consecutive comments by preserving their line structure:
    /// - `a && // comment1\n// comment2\nb` keeps each comment on its own line
    fn append_post_operator_parts(
        &self,
        parts: &mut DocBuf,
        op_end: u32,
        operand: &ChainOperand,
        allow_breaks: bool,
        lead_with_space: bool,
        chain_has_comments: bool,
    ) {
        let d = self.d();
        // Collect all comments in the range between operator and next operand. The
        // whole-chain gate skips this per-gap scan + collect for the ~all chains with no
        // comment: `chain_has_comments` false ⇒ this gap (⊆ the chain span) holds none to
        // emit, so the collect would be empty and the `is_empty()` path below runs — the
        // gate reaches it without the scan. Byte-identical (a *presence* flag: on-page ⊇
        // to-emit, so a false gate proves this gap emits nothing).
        let comments: CommentVec<'_> = if chain_has_comments {
            comments_to_emit_in_range(self.comments, op_end, operand.span.start).collect()
        } else {
            CommentVec::new()
        };

        if comments.is_empty() {
            // No comments - simple case
            if allow_breaks && !lead_with_space {
                parts.push(d.line());
            } else {
                parts.push(d.text(" "));
            }
            parts.push(operand.doc);
            return;
        }

        // A comment forces the operand onto its own (broken) line when it's a line
        // comment OR a block comment with a newline AFTER it (toward the next comment
        // / the operand) — prettier's `hasLeadingOwnLineComment`. Keying on the newline
        // *after* (not before) is what makes this idempotent: prettier's own
        // width-broken output puts an inline-leading block at line start (newline
        // before, none after — `&&⏎/* c */ b`), which must stay inline, not re-break.
        let forces_own_line = self.comment_hangs_binary_operand(op_end, operand.span.start);

        if !forces_own_line {
            // Only inline-leading block comments - place as leading on RHS operand.
            // In flat mode: `a || /* comment */ b` (space from line(), comment+trailing space, operand)
            // In break mode: `a ||\n<indent>/* comment */ b` (comment leads continuation line)
            let comments_doc =
                self.build_comments_between(op_end, operand.span.start, CommentSpacing::Trailing);
            if allow_breaks && !lead_with_space {
                parts.push(d.line());
            } else {
                parts.push(d.text(" "));
            }
            parts.push(comments_doc);
            parts.push(operand.doc);
            return;
        }

        // An own-line (or line) comment forces the chain to break. Each comment keeps
        // its line; authored blank lines are preserved. A trailing comment glued to the
        // operand (no newline after it) stays inline-leading it.
        let mut pos = op_end;
        for (i, comment) in comments.iter().enumerate() {
            let is_first = i == 0;
            // Comment-adjacency read (real even in canonical mode): decides
            // line_suffix-vs-own-line emission, so it must see source newlines.
            let has_newline_before = self.comment_has_newline_between(pos, comment.span.start);

            if is_first && !has_newline_before {
                // First comment on same line as operator: `a && // comment`. A
                // line comment goes through `line_suffix` (zero width), so a long
                // trailing comment never forces the preceding operand group to
                // break — matching prettier's `lineSuffix`. Block comments stay
                // inline, width counted.
                parts.push(self.build_trailing_comment_doc(comment));
            } else {
                // Comment on its own line (preserve an author blank line before it).
                self.push_blank_preserving_hardline(parts, pos, comment.span.start);
                parts.push(self.build_comment_doc(comment));
            }
            pos = comment.span.end;
        }

        // Operand: on its own line when the last comment has a newline after it
        // (preserving an author blank line), else glued inline (`/* c */ operand`).
        // Comment-adjacency read (real even in canonical mode): a line comment always
        // has a source newline before the operand, and gluing the operand after its
        // `line_suffix` would swallow it at flush (inside `${…}` this even makes the
        // output unparseable).
        if self.comment_has_newline_between(pos, operand.span.start) {
            self.push_blank_preserving_hardline(parts, pos, operand.span.start);
        } else {
            parts.push(d.text(" "));
        }
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
            if let Some(new_i) = tsv_lang::source_scan::skip_comment(bytes, i, range_end) {
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
        expr: &internal::BinaryExpression<'_>,
        operands: &mut OperandBuf,
        operators: &mut OperatorBuf,
    ) {
        // Recursively flatten left side if it can be chained with current operator
        if let Expression::BinaryExpression(left_binary) = expr.left {
            if expr.operator.can_flatten_with(left_binary.operator) {
                self.collect_binary_chain_with_spans(left_binary, operands, operators);
            } else {
                operands.push(ChainOperand {
                    doc: self.build_binary_operand_doc(expr.left, expr.operator, false),
                    span: expr.left.span(),
                });
            }
        } else {
            operands.push(ChainOperand {
                doc: self.build_binary_operand_doc(expr.left, expr.operator, false),
                span: expr.left.span(),
            });
        }

        // Add current operator
        operators.push(expr.operator);

        // Also flatten right side for truly associative operators (removes redundant parens)
        // e.g., `a && (b && c)` becomes `a && b && c`
        // Only logical operators are truly associative; arithmetic preserves right-side parens
        if let Expression::BinaryExpression(right_binary) = expr.right
            && expr.operator.can_flatten_with(right_binary.operator)
            && expr.operator.is_logical()
            && right_binary.operator.is_logical()
        {
            self.collect_binary_chain_with_spans(right_binary, operands, operators);
            return;
        }

        // Right operand can't be flattened - add as-is
        operands.push(ChainOperand {
            doc: self.build_binary_operand_doc(expr.right, expr.operator, true),
            span: expr.right.span(),
        });
    }

    /// Build operand with parens if needed for clarity
    pub(in crate::printer) fn build_binary_operand_doc(
        &self,
        operand: &Expression<'_>,
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
        // - Script contexts (LayoutMode::Standalone): Use group with parens INSIDE
        //   so the fit calculation includes `)`. This ensures `(A + B) *` at 101 chars
        //   breaks inside the parens, not just at `*`.
        // - Embedded expression contexts (LayoutMode::Embedded): Use the grouped
        //   approach from build_binary_chain_doc_with_continuation_indent, which keeps
        //   short 2-operand binaries flat (Prettier's behavior for template expressions).
        if self.needs_parens(operand, ctx) {
            if let Expression::BinaryExpression(inner_binary) = operand {
                if self.embed.is_embedded() {
                    // Embedded expression context: use grouped approach that keeps short binaries flat
                    let inner_doc =
                        self.build_binary_chain_doc_with_continuation_indent(inner_binary);
                    return d.parens(inner_doc);
                }
                // Script context: include parens in group for proper line width calculation
                let inner_parts =
                    self.build_binary_chain_parts_with_continuation_indent(inner_binary);
                return d.group(d.parens(inner_parts));
            }
            let operand_doc = self.build_chain_aware_operand_doc(operand);
            d.parens(operand_doc)
        } else if let Expression::BinaryExpression(inner_binary) = operand {
            // Nested binary sub-expressions use continuation indent.
            // Prettier's shouldNotIndent (binaryish.js:96-115) evaluates to false when
            // parent is BinaryExpression (none of the conditions match), so the inner
            // chain gets indent(rest). E.g., `0.5 * a(...) * b(...)` inside `... + 1.0`
            // indents the `*` continuation lines relative to `0.5`.
            self.build_binary_chain_doc_with_continuation_indent(inner_binary)
        } else {
            self.build_chain_aware_operand_doc(operand)
        }
    }

    /// Build a binary operand's doc, routing a curried arrow-chain operand
    /// (`cond ?? ((a) => (b) => …)`) through the progressive call-arg/binaryish
    /// chain layout. Mirrors prettier's `isBinaryish(parent)` reaching
    /// `printArrowFunctionSignatures`; `should_use_arrow_chain_layout` still gates
    /// on untyped / comment-free chains, so a typed or comment-bearing operand
    /// falls through to the default path.
    fn build_chain_aware_operand_doc(&self, operand: &Expression<'_>) -> DocId {
        if crate::printer::is_curried_arrow_chain(operand) {
            self.build_with_arrow_chain_context(
                crate::printer::ArrowChainContext::CallArgOrBinaryish,
                || self.build_expression_doc(operand),
            )
        } else {
            self.build_expression_doc(operand)
        }
    }

    /// Build a Doc for an await expression
    pub(in crate::printer) fn build_await_doc(
        &self,
        await_expr: &internal::AwaitExpression<'_>,
    ) -> DocId {
        let d = self.d();

        // Preserve comments from stripped grouping parens: `await (/** @type {T} */ expr)`
        let keyword_end = await_expr.span.start + "await".len() as u32;
        let argument_start = await_expr.argument.span().start;
        let argument_end = await_expr.argument.span().end;
        let comments_opt = self.build_keyword_operand_comments_opt(keyword_end, argument_start);

        // Trailing comments from stripped grouping parens: `await (x /* c */)` → `await x /* c */`
        let has_trailing_comments =
            self.has_comments_to_emit_between(argument_end, await_expr.span.end);

        let argument_doc = if comments_opt.is_some() || has_trailing_comments {
            // The grouping parens are required when the operand needs them (`await`
            // binds tighter than a binary/ternary operand, so `await x + y` is
            // `(await x) + y`). Keep them, and keep the comment INSIDE them where the
            // author wrote it — prettier relocates it past `)` (and floats it past `;`
            // on the next pass); tsv preserves the position. Mirrors `build_spread_doc`.
            let needs_parens = self.needs_parens(await_expr.argument, ParenContext::AwaitArgument);
            let inner = self.build_expression_doc(await_expr.argument);
            let mut parts = DocBuf::new();
            if needs_parens {
                parts.push(d.text("("));
            }
            if let Some(comments) = comments_opt {
                parts.push(comments);
            }
            parts.push(inner);
            self.append_trailing_paren_comments(&mut parts, argument_end, await_expr.span.end);
            if needs_parens {
                parts.push(d.text(")"));
            }
            d.concat(&parts)
        } else if self.needs_parens(await_expr.argument, ParenContext::AwaitArgument) {
            d.concat(&[
                d.text("("),
                self.build_expression_doc(await_expr.argument),
                d.text(")"),
            ])
        } else {
            self.build_expression_doc(await_expr.argument)
        };

        d.concat(&[d.text("await "), argument_doc])
    }

    /// Build a Doc for a yield expression
    pub(in crate::printer) fn build_yield_doc(
        &self,
        yield_expr: &internal::YieldExpression<'_>,
    ) -> DocId {
        let d = self.d();
        let keyword = if yield_expr.delegate {
            "yield*"
        } else {
            "yield"
        };
        let Some(arg) = yield_expr.argument else {
            return d.text(keyword);
        };

        let keyword_end = yield_expr.span.start + keyword.len() as u32;
        let argument_start = arg.span().start;
        let argument_end = arg.span().end;

        // Trailing comments from stripped grouping parens: `yield (x /* c */)` → `yield x /* c */`
        let has_trailing_comments =
            self.has_comments_to_emit_between(argument_end, yield_expr.span.end);

        // A comment that forces the break takes the parenthesized form. `yield` is a
        // restricted production (`yield [no LineTerminator here] AssignmentExpression`,
        // ECMA-262 §15.5), so without the parens ASI ends the `yield` at the newline and
        // the operand becomes a separate expression statement — the `yield` silently
        // loses its argument. Same gate and same layout as its `return`/`throw` siblings;
        // see `build_hanging_paren_doc` for the shared rule, and
        // docs/conformance_prettier.md §Comment relocation for why prettier (whose own
        // retention is scoped to those two) diverges here.
        if self.argument_has_own_line_comment(yield_expr.span.start, arg) {
            let mut body = DocBuf::new();
            if let Some(comments) = self.build_rhs_comments_opt(keyword_end, argument_start) {
                body.push(comments);
            }
            body.push(self.build_expression_doc(arg));
            // The trailing comment stays INSIDE the parens, where it was written.
            if has_trailing_comments {
                self.append_trailing_paren_comments(&mut body, argument_end, yield_expr.span.end);
            }
            return self.build_hanging_paren_doc(keyword, d.concat(&body));
        }

        let mut parts: DocBuf = smallvec![d.text(keyword), d.text(" ")];
        // Every remaining comment is glued to the keyword with the operand after it on
        // some line, so the operand is pulled up onto the comment's line rather than
        // keeping the author's break — the break would be ASI, not layout.
        let leading_comments_opt = self.build_rhs_comments_glued_opt(keyword_end, argument_start);

        if leading_comments_opt.is_some() || has_trailing_comments {
            if let Some(comments) = leading_comments_opt {
                parts.push(comments);
            }
            parts.push(self.build_expression_doc(arg));
            self.append_trailing_paren_comments(&mut parts, argument_end, yield_expr.span.end);
        } else if self.needs_parens(arg, ParenContext::YieldArgument) {
            // Assignment needs parens: `yield (x ??= y)`
            parts.push(d.text("("));
            parts.push(self.build_expression_doc(arg));
            parts.push(d.text(")"));
        } else {
            parts.push(self.build_expression_doc(arg));
        }

        d.concat(&parts)
    }

    /// Build a Doc for a sequence expression
    ///
    /// Redundantly-parenthesized operand comments anchored to the sequence's
    /// outer *edges* float OUT of the sequence parens, matching prettier's fixed
    /// point: a leading comment on the first operand (`((/* c */ x), y)`) is
    /// emitted before the opening `(` (`/* c */ (x, y)`) and a trailing comment
    /// on the last operand (`(x, (y /* c */))`) after the closing `)`
    /// (`(x, y) /* c */`). Each floated comment keeps its source line-treatment —
    /// own-line (hardline) when a newline separates it from the operand, inline
    /// (space) otherwise. Preserving the line-treatment is what makes the float
    /// idempotent even when the sequence is nested inside surrounding comments (a
    /// naive always-inline float re-collapses on the second pass).
    /// See operand_edge_comment_prettier_divergence.
    ///
    /// Interior operand comments (between two operands) stay stripped + inline on
    /// the comma-gap path below and match prettier — see operand_comments.
    ///
    /// This is the statement/throw/call-argument default (the comment floats out).
    /// Value positions (return / variable init / assignment RHS) instead keep the
    /// last operand's trailing comment INSIDE the parens — see
    /// [`Self::build_sequence_doc_value`].
    pub(in crate::printer) fn build_sequence_doc(
        &self,
        seq: &internal::SequenceExpression<'_>,
    ) -> DocId {
        // Float-out path: the last operand's trailing comment is the caller's job
        // (it lives in the stripped grouping-paren gap, outside `seq.span`), so the
        // in-sequence trailing scan stops at `seq.span.end`.
        self.build_sequence_doc_inner(seq, seq.span.end, false)
    }

    /// Value-position variant: a trailing comment on the last operand stays
    /// **inside** the parens (`return (a, b /* c */)` / `const x = (a, b // c)`)
    /// rather than floating out after `)`. Prettier keeps sequence/assignment
    /// trailing comments inside the added parens in value positions (return arg,
    /// variable init, assignment RHS) — its #19263 — while floating them out in
    /// statement / throw / call-argument positions. Callers in value positions use
    /// this; everything else uses [`Self::build_sequence_doc`].
    ///
    /// `trailing_end` is where the stripped grouping `)` sits (the comment between
    /// the last operand and it must be kept inside) — the caller finds it because
    /// it falls *outside* `seq.span` (the grouping parens aren't part of the node).
    pub(in crate::printer) fn build_sequence_doc_value(
        &self,
        seq: &internal::SequenceExpression<'_>,
        trailing_end: u32,
    ) -> DocId {
        self.build_sequence_doc_inner(seq, trailing_end, true)
    }

    fn build_sequence_doc_inner(
        &self,
        seq: &internal::SequenceExpression<'_>,
        trailing_end: u32,
        keep_trailing_inside: bool,
    ) -> DocId {
        // Line comments anywhere up to `trailing_end` (incl. the last operand's
        // trailing comment, which lives outside `seq.span` in value positions) need
        // break handling so the comment isn't swallowed by the following comma/operand
        // or the closing `)`.
        // Axis-free: the rule looks only at LINE comments, and ownership binds only a block
        // comment (`owned ⇒ is_block`), so skipping and counting give the same answer.
        if comments_to_emit_in_range(self.comments, seq.span.start, trailing_end)
            .any(|c| !c.is_block)
        {
            return self.build_sequence_doc_with_line_comments(
                seq,
                trailing_end,
                keep_trailing_inside,
            );
        }

        let d = self.d();
        let n = seq.expressions.len();
        let mut parts = DocBuf::with_capacity(n * 3 + 4);

        // Whole-sequence comment gate: the inter-operand gaps (after/before each comma)
        // all lie within `seq.span`, so with no comment there, every per-operand gap is
        // empty. Skip the per-operand comma scans + the `empty()` comment children on the
        // comment-free common path. Byte-identical (the line-comment path already branched
        // off above, so a present comment here is a block, handled by the full path).
        let seq_has_comments = self.has_comments_to_emit_between(seq.span.start, seq.span.end);

        // First operand's leading-edge comments float OUT, before the opening `(`.
        let first_start = seq.expressions[0].span().start;
        self.append_floated_leading_comments(&mut parts, seq.span.start, first_start);

        parts.push(d.text("("));
        for (i, expr) in seq.expressions.iter().enumerate() {
            let is_last = i + 1 == n;
            let expr_start = expr.span().start;
            let expr_end = expr.span().end;

            if i > 0 {
                parts.push(d.text(", "));
                // Leading comments of this operand: the gap after the previous comma.
                // Redundant operand parens are stripped, so a comment the user wrote
                // inside them (`(/* c */ b)`) is preserved inline before the operand.
                if seq_has_comments {
                    let prev_end = seq.expressions[i - 1].span().end;
                    if let Some(comma) = self.find_comma_after(prev_end) {
                        parts.push(self.build_comments_between(
                            comma + 1,
                            expr_start,
                            CommentSpacing::Trailing,
                        ));
                    }
                }
            }

            // Assignment expressions in sequences need individual parens.
            let core = self.build_expression_doc(expr);
            let inner = if matches!(expr, Expression::AssignmentExpression(_)) {
                d.parens(core)
            } else {
                core
            };
            parts.push(inner);

            // Trailing comments of this operand: the gap before the next comma.
            if seq_has_comments
                && !is_last
                && let Some(comma) = self.find_comma_after(expr_end)
            {
                parts.push(self.build_comments_between(expr_end, comma, CommentSpacing::Leading));
            }
        }
        let last_end = seq.expressions[n - 1].span().end;
        if keep_trailing_inside {
            // Value position: a same-line block comment stays INSIDE before `)`
            // (`(a, b /* c */)`). Block-only path, so the comments are blocks. The
            // comment lives between the last operand and the grouping `)`
            // (`trailing_end`), outside `seq.span`.
            for comment in comments_to_emit_in_range(self.comments, last_end, trailing_end) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }
            parts.push(d.text(")"));
        } else {
            parts.push(d.text(")"));
            // Last operand's trailing-edge comments float OUT, after the closing `)`.
            // Same-line block comments stay inline (`(x, y) /* c */`); own-line block
            // comments defer via `line_suffix` (`append_trailing_paren_comments`) so
            // they land past the enclosing comma/semicolon — where they re-parse to,
            // keeping the float idempotent. Line comments never reach this path (they
            // route to the legacy layout above).
            self.append_trailing_paren_comments(&mut parts, last_end, seq.span.end);
        }

        d.concat(&parts)
    }

    /// Emit the first operand's leading-edge comments, floated out before the
    /// sequence's opening `(`, preserving each comment's source line-treatment:
    /// own-line (a newline before the operand) → hardline, inline → space. The
    /// spacing follows each comment and is sized by the gap to the next token (the
    /// following comment, else the operand at `operand_start`). On re-parse these
    /// land in the enclosing context's leading-comment domain, which emits the
    /// same own-line/inline treatment — so the float is idempotent.
    fn append_floated_leading_comments(&self, parts: &mut DocBuf, start: u32, operand_start: u32) {
        let d = self.d();
        let comments: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, start, operand_start).collect();
        for (i, comment) in comments.iter().enumerate() {
            parts.push(self.build_comment_doc(comment));
            let next = comments.get(i + 1).map_or(operand_start, |c| c.span.start);
            // Comment-adjacency read (real even in canonical mode): a line comment
            // always has a newline before the next token, and gluing content after
            // its inline emission would swallow it.
            if self.comment_has_newline_between(comment.span.end, next) {
                parts.push(d.hardline());
            } else {
                parts.push(d.text(" "));
            }
        }
    }

    /// Sequence layout used when the sequence contains a line comment, which forces
    /// a multiline break so the comment isn't swallowed by the following comma or
    /// operand. Mirrors prettier's `group(join([",", line], parts))`: each comma gap
    /// is partitioned by line — a comment with no newline before it *trails* the
    /// preceding operand (a same-line block stays inline before the comma; a line
    /// comment defers past the comma via `line_suffix`, rendering at end-of-line);
    /// an own-line comment *leads* the next operand on its own line. A `break_parent`
    /// forces the group (and any enclosing call/arg group) to break.
    ///
    /// The outer-edge comments — leading on the first operand, trailing on the last —
    /// still float OUT of the parens via the same helpers as the block-comment path
    /// (`append_floated_leading_comments` / `append_trailing_paren_comments`).
    fn build_sequence_doc_with_line_comments(
        &self,
        seq: &internal::SequenceExpression<'_>,
        trailing_end: u32,
        keep_trailing_inside: bool,
    ) -> DocId {
        let d = self.d();
        let n = seq.expressions.len();

        // First operand's leading-edge comments float OUT, before the opening `(`.
        let mut outer = DocBuf::new();
        let first_start = seq.expressions[0].span().start;
        self.append_floated_leading_comments(&mut outer, seq.span.start, first_start);

        // Build per-operand docs (own-line leading + core + same-line trailing),
        // joined by `,` + line inside a group forced to break.
        let mut inner: DocBuf = smallvec![d.break_parent()];
        for (i, expr) in seq.expressions.iter().enumerate() {
            let is_last = i + 1 == n;
            let expr_start = expr.span().start;
            let expr_end = expr.span().end;
            let mut od = DocBuf::new();

            // Own-line comments from the previous comma gap lead this operand.
            // The same-line prefix of that gap trails the previous operand (emitted
            // there), so skip it here; once a comment is own-line the rest follow.
            if i > 0 {
                let prev_end = seq.expressions[i - 1].span().end;
                let mut pos = prev_end;
                let mut in_trailing_run = true;
                for comment in comments_to_emit_in_range(self.comments, prev_end, expr_start) {
                    // Comment-adjacency read (real even in canonical mode).
                    let own_line = self.comment_has_newline_between(pos, comment.span.start);
                    // Once a comment is own-line (or the trailing run already ended),
                    // it and the rest lead the next operand.
                    if !in_trailing_run || own_line {
                        in_trailing_run = false;
                        od.push(self.build_comment_doc(comment));
                        od.push(d.hardline());
                    }
                    pos = comment.span.end;
                }
            }

            // Assignment expressions in sequences need individual parens.
            let core = self.build_expression_doc(expr);
            od.push(if matches!(expr, Expression::AssignmentExpression(_)) {
                d.parens(core)
            } else {
                core
            });

            // Same-line comments in the next comma gap trail this operand: a block
            // stays inline before the comma; a line comment defers via `line_suffix`
            // so it renders after the comma at end-of-line. Own-line comments belong
            // to the next operand (handled above), so stop at the first one.
            if !is_last {
                let next_start = seq.expressions[i + 1].span().start;
                let mut pos = expr_end;
                for comment in comments_to_emit_in_range(self.comments, expr_end, next_start) {
                    // Comment-adjacency read (real even in canonical mode): an
                    // own-line comment must lead the next operand, not merge into
                    // the previous operand's `line_suffix` trailing run.
                    if self.comment_has_newline_between(pos, comment.span.start) {
                        break;
                    }
                    // Same-line trailing comment: block inline before the comma, line
                    // comment deferred via `line_suffix` to render after the comma.
                    od.push(self.build_trailing_comment_doc(comment));
                    pos = comment.span.end;
                }
            } else if keep_trailing_inside {
                // Value position: the last operand's trailing comment stays INSIDE the
                // parens, trailing the operand (`b // c` then `)` on its own line) — a
                // block inline, a line comment via `line_suffix`. The `softline` before
                // `)` in the keep-inside assembly below flushes the `line_suffix`. The
                // comment lives up to the grouping `)` (`trailing_end`), outside `seq.span`.
                for comment in comments_to_emit_in_range(self.comments, expr_end, trailing_end) {
                    od.push(self.build_trailing_comment_doc(comment));
                }
            }

            if i > 0 {
                inner.push(d.text(","));
                inner.push(d.line());
            }
            inner.push(d.concat(&od));
        }

        if keep_trailing_inside {
            // Value position: `(\n\ta,\n\tb // c\n)` — operands indented, the trailing
            // comment kept inside (above). `break_parent` (in `inner`) forces it open.
            outer.push(d.group(d.concat(&[
                d.text("("),
                d.indent(d.concat(&[d.softline(), d.concat(&inner)])),
                d.softline(),
                d.text(")"),
            ])));
            return d.concat(&outer);
        }

        outer.push(d.text("("));
        outer.push(d.group(d.concat(&inner)));
        outer.push(d.text(")"));

        // Last operand's trailing-edge comments float OUT, after the closing `)`.
        let last_end = seq.expressions[n - 1].span().end;
        self.append_trailing_paren_comments(&mut outer, last_end, seq.span.end);

        d.concat(&outer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run `should_group_binary_continuation` on a parsed binary expression.
    fn group(src: &str) -> bool {
        let arena = bumpalo::Bump::new();
        let expr = crate::parse_expression_with_comments(src, 0, &arena)
            .expect("expression should parse")
            .0;
        match expr {
            Expression::BinaryExpression(b) => Printer::should_group_binary_continuation(&b),
            other => panic!("expected a binary expression, got: {other:?}"),
        }
    }

    #[test]
    fn should_group_binary_continuation_by_category() {
        // A logical operand under an arithmetic parent — categories differ, so the
        // continuation gets its own group.
        assert!(group("(a && b) + c"));
        assert!(group("(a && b) * c"));
        // Flattened same-category chains do NOT group (the left is the same category).
        assert!(!group("a && b && c"));
        assert!(!group("a * b * c"));
    }
}
