// Conditional (ternary) expression printing for TypeScript
//
// Handles: a ? b : c, nested ternaries, comments in ternaries

use crate::ast::internal;
use crate::printer::{CommentVec, Printer, template_literal_has_newlines};
use smallvec::smallvec;
use tsv_lang::INDENT;
use tsv_lang::comments_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// Check if an expression is a nullish coalescing expression (`??`)
///
/// Prettier wraps `??` in parens when inside a ternary for clarity.
pub(in crate::printer) fn is_nullish_coalescing(expr: &internal::Expression<'_>) -> bool {
    matches!(
        expr,
        internal::Expression::BinaryExpression(bin)
            if bin.operator == internal::BinaryOperator::QuestionQuestion
    )
}

/// A ternary consequent/alternate that gets clarity parens (prettier:
/// needs-parentheses.js, the `ConditionalExpression` parent case). `as`/`satisfies`
/// and an assignment bind tighter than `?:` so the parens are pure clarity (same
/// AST); `??` is always parenthesized under a conditional. Shared by the inline and
/// line-comment layouts so both branch paths agree.
fn ternary_branch_needs_parens(expr: &internal::Expression<'_>) -> bool {
    matches!(
        expr,
        internal::Expression::TSAsExpression(_)
            | internal::Expression::TSSatisfiesExpression(_)
            | internal::Expression::AssignmentExpression(_)
    ) || is_nullish_coalescing(expr)
}

/// A ternary TEST that gets parens (prettier: needs-parentheses.js). For arrow/yield
/// it is **semantic** — without parens the body absorbs the ternary (`() => 1 ? x : y`
/// parses as `() => (1 ? x : y)`; `yield 1 ? x : y` as `yield (1 ? x : y)`); for
/// `as`/`satisfies`/assignment/`??` it is clarity (same AST, they bind tighter than
/// `?:`). Shared by the inline and line-comment layouts so both agree — the
/// line-comment path must not drop the semantic arrow/yield parens.
fn ternary_test_needs_parens(expr: &internal::Expression<'_>) -> bool {
    is_nullish_coalescing(expr)
        || matches!(
            expr,
            internal::Expression::AssignmentExpression(_)
                | internal::Expression::AwaitExpression(_)
                | internal::Expression::ArrowFunctionExpression(_)
                | internal::Expression::YieldExpression(_)
                | internal::Expression::TSAsExpression(_)
                | internal::Expression::TSSatisfiesExpression(_)
        )
}

/// Check if an expression is a template literal containing newlines
///
/// When a template literal contains embedded newlines in its quasi strings,
/// it should be treated as "multiline" for formatting purposes. This is used
/// to force ternaries to break when their consequent or alternate is multiline.
fn is_multiline_template_literal(expr: &internal::Expression<'_>) -> bool {
    matches!(expr, internal::Expression::TemplateLiteral(t) if template_literal_has_newlines(t))
}

impl<'a> Printer<'a> {
    /// Build a Doc for a conditional expression with wrapping support
    pub(in crate::printer) fn build_conditional_doc_with_wrapping(
        &self,
        cond: &internal::ConditionalExpression<'_>,
    ) -> DocId {
        self.build_conditional_doc_impl(cond, false, false)
    }

    /// Build a Doc for a conditional expression in return/throw/call/new context.
    ///
    /// When the ternary's parent is ReturnStatement, ThrowStatement, CallExpression,
    /// or NewExpression, binary expressions in the test position use continuation
    /// indent. This matches Prettier's shouldNotIndent (binaryish.js:109-113) which
    /// exempts binaries from indent only when the grandparent is NOT one of these types.
    pub(in crate::printer) fn build_conditional_doc_with_binary_test_indent(
        &self,
        cond: &internal::ConditionalExpression<'_>,
    ) -> DocId {
        self.build_conditional_doc_impl(cond, false, true)
    }

    /// Implementation of conditional doc building
    ///
    /// `is_chained` indicates this conditional is nested within a parent conditional
    /// (either in consequent when broken, or in alternate). When chained, we don't
    /// wrap in a new group, so the parent's break decision cascades to this one.
    ///
    /// `indent_binary_test` indicates the ternary is inside a return/throw/call/new
    /// statement, so binary expressions in the test position should use continuation
    /// indent (matching Prettier's shouldNotIndent = false for these grandparents).
    fn build_conditional_doc_impl(
        &self,
        cond: &internal::ConditionalExpression<'_>,
        is_chained: bool,
        indent_binary_test: bool,
    ) -> DocId {
        let d = self.d();
        let test_end = cond.test.span().end;
        let consequent_start = cond.consequent.span().start;
        let consequent_end = cond.consequent.span().end;
        let alternate_start = cond.alternate.span().start;

        // Check for line comments that force breaking
        let has_line_comments = self.has_line_comments_between(test_end, consequent_start)
            || self.has_line_comments_between(consequent_end, alternate_start);

        // A branch-gap comment separated from its value by a blank line forces the
        // break too — prettier breaks on `a ? /* c */⏎⏎b` even though an own-line
        // block comment with no blank stays inline (`a ? /* c */⏎b`). Scan the whole
        // test→consequent / consequent→alternate ranges (the `?`/`:` sit before the
        // gap comments, so the blank-after-comment check is unaffected by them).
        let has_blank_separated_comment = self
            .comment_followed_by_blank(test_end, consequent_start)
            || self.comment_followed_by_blank(consequent_end, alternate_start);

        // Check for multiline template literals in test, consequent, or alternate
        // Template literals with embedded newlines should force the ternary to break,
        // even though those newlines don't appear in the doc structure.
        let has_multiline_template = is_multiline_template_literal(cond.test)
            || is_multiline_template_literal(cond.consequent)
            || is_multiline_template_literal(cond.alternate);

        // If there are line comments, a blank-separated branch comment, or multiline
        // template literals, use a breaking layout. Other block comments after ? or :
        // are handled inline in the non-breaking path.
        if has_line_comments || has_blank_separated_comment || has_multiline_template {
            return self.build_conditional_doc_with_line_comments(cond, is_chained);
        }

        // Prettier's shouldNotIndent (binaryish.js:109-113) exempts binaries whose
        // parent is ConditionalExpression from continuation indent, UNLESS the
        // grandparent is ReturnStatement, ThrowStatement, CallExpression, or
        // NewExpression. In those cases, shouldNotIndent = false and the binary
        // gets indent(rest) for its continuation lines.
        //
        // In embedded contexts (Svelte attributes), the grandparent is a template
        // node (none of the above), so shouldNotIndent = true → no indent.
        let test = if let internal::Expression::BinaryExpression(binary) = cond.test {
            if self.embed.is_embedded() {
                // Embedded: shouldNotIndent = true (grandparent is Svelte template node)
                self.build_binary_chain_doc(binary)
            } else if indent_binary_test {
                // Grandparent is return/throw/call/new: shouldNotIndent = false
                self.build_binary_chain_doc_with_continuation_indent(binary)
            } else {
                // Default: shouldNotIndent = true (grandparent is assignment, variable, etc.)
                self.build_binary_chain_doc(binary)
            }
        } else {
            self.build_expression_doc(cond.test)
        };
        // Several test-position expressions get parens (Prettier: needs-parentheses.js).
        // See `ternary_test_needs_parens` for the arrow/yield semantics vs the
        // `as`/`satisfies`/assignment/`??` clarity cases.
        let test = if ternary_test_needs_parens(cond.test) {
            d.parens(test)
        } else {
            test
        };
        // Parenthesize an `in` test inside a for-header init (`for (a = (b in c) ? …;…)`);
        // a no-op elsewhere. The test is `[~In]`, so the parens are load-bearing.
        let test = self.wrap_for_init_in(cond.test, test);
        // Prettier's shouldNotIndent (binaryish.js:109-113) also applies to binaries
        // in consequent/alternate positions: when parent is ConditionalExpression and
        // grandparent is ReturnStatement/ThrowStatement/CallExpression/NewExpression,
        // shouldNotIndent = false → binary gets indent(rest) for continuation lines.
        // In assignment/variable contexts, shouldNotIndent = true → flat (no indent).
        // Bound the consequent's own paren-comment scan at its end — the
        // consequent-to-`:` comment is emitted by `comments_before_colon` below, so a
        // wider boundary would double-emit it.
        let consequent =
            self.build_ternary_branch_expr_doc(cond.consequent, indent_binary_test, consequent_end);

        // Split comments around ? and : operators.
        // Comments before ? go after test, comments after ? go before consequent,
        // comments after : go before alternate.
        let question_pos = self.find_char_outside_comments(test_end, consequent_start, b'?');
        let colon_pos = self.find_char_outside_comments(consequent_end, alternate_start, b':');

        // Comments between test and ?
        let comments_before_question = if let Some(q) = question_pos {
            self.build_inline_comments_between_doc(test_end, q)
        } else {
            self.build_inline_comments_between_doc(test_end, consequent_start)
        };

        // Comments between ? and consequent (e.g., `b ? /* comment */ c`)
        // Trailing space so the comment doesn't touch the consequent
        let comments_after_question = if let Some(q) = question_pos {
            self.build_inline_comments_between_doc_trailing_space(q + 1, consequent_start)
        } else {
            d.empty()
        };

        // Comments between consequent and : (e.g., `b ? c /* comment */ : d`)
        let comments_before_colon = if let Some(c) = colon_pos {
            self.build_inline_comments_between_doc(consequent_end, c)
        } else {
            d.empty()
        };

        // Comments between : and alternate (e.g., `c : /* comment */ d`)
        let comments_after_colon = if let Some(c) = colon_pos {
            self.build_inline_comments_between_doc_trailing_space(c + 1, alternate_start)
        } else {
            d.empty()
        };

        // Handle nested conditional in consequent specially:
        // - When flat: parens for parsing `a ? (b ? c : d) : e`
        // - When broken: continue chain without parens (same as alternate)
        //
        // Prettier wraps each branch in indent() so that multiline content
        // (like arrow block bodies) gets proper nesting. Exception: nested
        // conditionals handle their own indentation, so no extra wrapper.
        let consequent_doc =
            if let internal::Expression::ConditionalExpression(nested) = cond.consequent {
                // Broken version: continue chain without parens
                let broken_consequent =
                    self.build_conditional_doc_impl(nested, true, indent_binary_test);
                if d.will_break(consequent) {
                    // Consequent forces breaking (e.g., line comments produce hardlines).
                    // Skip if_break and use broken layout directly — the outer group
                    // will break because broken_consequent contains hardlines.
                    // Matches Prettier's willBreak(consequentDoc) → shouldBreak check
                    // in printTernaryOld (ternary-old.js).
                    broken_consequent
                } else {
                    // Normal if_break: parens when flat, chain when broken
                    let flat_consequent = d.parens(consequent);
                    d.if_break(broken_consequent, flat_consequent)
                }
            } else if ternary_branch_needs_parens(cond.consequent) {
                d.indent(d.parens(consequent))
            } else {
                d.indent(consequent)
            };

        // Handle nested conditional in alternate: continue the chain
        // - Nested conditional does NOT need parens: `a ? b : c ? d : e`
        //   (right-associative, so naturally parsed as `a ? b : (c ? d : e)`)
        // - `as`/`satisfies` need parens to avoid `:` ambiguity: `a ? b : (c as T)`
        // - `??` needs parens for clarity: `a ? b : (c ?? d)`
        let alternate_doc =
            if let internal::Expression::ConditionalExpression(nested) = cond.alternate {
                // Recursively build as chained (no group wrapper, no parens)
                // No indent wrapper - nested conditional has its own structure
                self.build_conditional_doc_impl(nested, true, indent_binary_test)
            } else {
                let alternate = self.build_ternary_branch_expr_doc(
                    cond.alternate,
                    indent_binary_test,
                    cond.span.end,
                );
                let alternate = if ternary_branch_needs_parens(cond.alternate) {
                    d.parens(alternate)
                } else {
                    alternate
                };
                d.indent(alternate)
            };

        let inner = d.concat(&[
            test,
            comments_before_question,
            d.indent(d.concat(&[
                d.line(),
                d.text("? "),
                comments_after_question,
                consequent_doc,
                comments_before_colon,
                d.line(),
                d.text(": "),
                comments_after_colon,
                alternate_doc,
            ])),
        ]);

        // If chained (nested in another conditional), don't wrap in group
        // This allows the parent's break decision to cascade
        if is_chained { inner } else { d.group(inner) }
    }

    /// Build a conditional expression doc when there are line comments
    ///
    /// Line comments force the ternary to break because they end at newline.
    /// This produces:
    /// ```js
    /// test // comment
    ///   ? // comment
    ///     consequent // comment
    ///   : // comment
    ///     alternate
    /// ```
    fn build_conditional_doc_with_line_comments(
        &self,
        cond: &internal::ConditionalExpression<'_>,
        _is_chained: bool,
    ) -> DocId {
        let d = self.d();
        let test_end = cond.test.span().end;
        let consequent_start = cond.consequent.span().start;
        let consequent_end = cond.consequent.span().end;
        let alternate_start = cond.alternate.span().start;

        // Build test expression with parens if needed — the same predicate as the
        // non-breaking path, so the load-bearing arrow/yield parens (and the
        // `as`/`satisfies` clarity parens) are never dropped just because a branch
        // carries a line comment.
        let test = self.build_expression_doc(cond.test);
        let test = if ternary_test_needs_parens(cond.test) {
            d.parens(test)
        } else {
            test
        };
        // Parenthesize an `in` test inside a for-header init (`for (a = (b in c) ? …;…)`);
        // a no-op elsewhere. The test is `[~In]`, so the parens are load-bearing.
        let test = self.wrap_for_init_in(cond.test, test);

        // Find the ? and : positions for proper comment categorization
        let question_pos = self.find_char_outside_comments(test_end, consequent_start, b'?');
        let colon_pos = self.find_char_outside_comments(consequent_end, alternate_start, b':');

        let mut parts = smallvec![test];

        // Comments between test and ? (see split_pre_operator_comments): same-line
        // comments trail the test, later-line comments precede the `?` on their own
        // lines.
        let comments_before_q_end = question_pos.unwrap_or(consequent_start);
        let mut pre_question_own_line = DocBuf::new();
        self.split_pre_operator_comments(
            test_end,
            comments_before_q_end,
            &mut parts,
            &mut pre_question_own_line,
        );

        // Start the indented part: own-line pre-? comments, then ? on a new line
        let mut q_parts = pre_question_own_line;
        q_parts.push(d.hardline());
        q_parts.push(d.text("?"));

        // Comments between ? and consequent: first trails `?` inline, later ones take
        // their own indented line (author blanks preserved). `consequent_on_own_line`
        // is set when a comment can't share the consequent's line (the blank, if any,
        // is preserved below).
        let (consequent_on_own_line, blank_before_consequent) =
            self.emit_ternary_branch_comments(&mut q_parts, question_pos, consequent_start);

        // Consequent expression — when the outer ternary enters breaking layout
        // (line comments or multiline templates), nested conditionals in the
        // consequent must also break. Without group_break, the inner ternary's
        // group stays flat (content fits on one line), but Prettier cascades
        // the break from the parent to the entire ternary chain.
        let (consequent, is_nested_cond) =
            if let internal::Expression::ConditionalExpression(nested) = cond.consequent {
                let chained = self.build_conditional_doc_impl(nested, true, false);
                (d.group_break(chained), true)
            } else {
                let expr_doc = self
                    .wrap_for_init_in(cond.consequent, self.build_expression_doc(cond.consequent));
                // Clarity parens (`(a ?? b)`, `(x as T)`) exactly as the inline layout
                // applies them — the line-comment path must not drop them.
                let expr_doc = if ternary_branch_needs_parens(cond.consequent) {
                    d.parens(expr_doc)
                } else {
                    expr_doc
                };
                (expr_doc, false)
            };
        // A nested conditional handles its own indent via its chained structure;
        // any other consequent hangs one level deeper (its own multiline content
        // then aligns with the main layout, whether it sits on its own line after a
        // comment or trails a single block comment / bare `?`).
        let placed_consequent = if is_nested_cond {
            consequent
        } else {
            d.indent(consequent)
        };
        if consequent_on_own_line {
            // A comment can't share the consequent's line — consequent on a new line
            // (preserving an author blank line before it).
            if blank_before_consequent {
                q_parts.push(d.literalline());
            }
            q_parts.push(d.hardline());
            q_parts.push(d.text(INDENT));
        } else {
            // Single block comment or no comment - space then consequent
            q_parts.push(d.text(" "));
        }
        q_parts.push(placed_consequent);

        // Comments between consequent and :. Mirrors the test→? handling above
        // (same shared helper): same-line comments trail the consequent, later-line
        // comments precede the `:` on their own lines — both flow into q_parts in
        // source order (trailing run first, then own-line run).
        let comments_before_colon_end = colon_pos.unwrap_or(alternate_start);
        let mut colon_own_line = DocBuf::new();
        self.split_pre_operator_comments(
            consequent_end,
            comments_before_colon_end,
            &mut q_parts,
            &mut colon_own_line,
        );
        q_parts.append(&mut colon_own_line);

        // : on new line
        q_parts.push(d.hardline());
        q_parts.push(d.text(":"));

        // Comments between : and alternate — same shape as the ?→consequent gap.
        let (alternate_on_own_line, blank_before_alternate) =
            self.emit_ternary_branch_comments(&mut q_parts, colon_pos, alternate_start);

        // Alternate expression - nested conditionals cascade the break without extra indent
        let alternate_doc =
            if let internal::Expression::ConditionalExpression(nested) = cond.alternate {
                // Recursively use breaking layout - no indent wrapper (has its own structure)
                self.build_conditional_doc_with_line_comments(nested, true)
            } else {
                // Regular expressions get indent wrapper, plus the same clarity parens
                // the inline layout applies (`(a ?? b)`, `(x as T)`).
                let expr_doc = self
                    .wrap_for_init_in(cond.alternate, self.build_expression_doc(cond.alternate));
                let expr_doc = if ternary_branch_needs_parens(cond.alternate) {
                    d.parens(expr_doc)
                } else {
                    expr_doc
                };
                d.indent(expr_doc)
            };

        if alternate_on_own_line {
            if blank_before_alternate {
                q_parts.push(d.literalline());
            }
            q_parts.push(d.hardline());
            q_parts.push(d.text(INDENT));
        } else {
            q_parts.push(d.text(" "));
        }
        q_parts.push(alternate_doc);

        parts.push(d.indent(d.concat(&q_parts)));

        d.concat(&parts)
    }

    /// Emit the comments between a ternary operator (`?` or `:`) and its branch value
    /// into `parts`: the first trails the operator inline (`? /* c */`), each later one
    /// takes its own indented line (author blanks preserved). Shared by the
    /// ?→consequent and :→alternate gaps.
    ///
    /// Returns `(value_on_own_line, blank_before_value)`: the value drops onto its own
    /// line when a comment can't share it — a line comment, a later own-line comment,
    /// or a blank line before the value — and the caller preserves that trailing blank.
    fn emit_ternary_branch_comments(
        &self,
        parts: &mut DocBuf,
        op_pos: Option<u32>,
        value_start: u32,
    ) -> (bool, bool) {
        let d = self.d();
        let comments: CommentVec<'_> = op_pos
            .map(|p| comments_in_range(self.comments, p + 1, value_start).collect())
            .unwrap_or_default();
        let mut has_line_comment = false;
        let mut last_own_line = false;
        for (i, comment) in comments.iter().enumerate() {
            if i == 0 {
                // First comment trails the operator inline (`? /* c */`).
                parts.push(d.text(" "));
            } else {
                // Subsequent comments take their own line (author blank preserved).
                self.push_blank_preserving_hardline(
                    parts,
                    comments[i - 1].span.end,
                    comment.span.start,
                );
                parts.push(d.text(INDENT));
                last_own_line = true;
            }
            parts.push(self.build_comment_doc(comment));
            if !comment.is_block {
                has_line_comment = true;
            }
        }
        let blank_before_value = comments
            .last()
            .is_some_and(|c| self.has_blank_line_between(c.span.end, value_start));
        (
            has_line_comment || last_own_line || blank_before_value,
            blank_before_value,
        )
    }

    /// Split the comments in a ternary operand→operator gap into trailing vs
    /// own-line docs, shared by the test→`?` and consequent→`:` sites.
    ///
    /// A comment on the operand's own source line trails it (a block stays inline
    /// with its width counted; a line comment uses `line_suffix`, zero width, so a
    /// long trailing comment never forces a binary operand to break — see
    /// `test_trailing_long_comment`) and is pushed to `trailing`. A comment the
    /// author placed on a *later* line drops to its own line, aligned with the
    /// operator it precedes, and is pushed to `own_line` (a `d.hardline()` then the
    /// comment). A `//` ends its line, so a same-line run trails at most one line
    /// comment; everything after it already starts on a later line.
    ///
    /// This preserves the author's "before the operator" placement — prettier
    /// instead relocates later-line comments across the operator — and never merges
    /// consecutive line comments onto the operand line, which would reverse their
    /// order and fuse them into one node (the property-signature `// c2 // c1`
    /// quirk, here in a ternary). The two before-operator sites share this helper
    /// so they cannot drift apart (the original merge bug was exactly such a drift
    /// from the correct after-operator handling).
    // The same-line/later-line classification is shared via
    // `tsv_lang::ClassifiedComments` (also used by `calls/arg_comments.rs`
    // PartitionedComments and the member-chain `push_gap_comments_and_break`), so the
    // "same-line trails, later-line breaks, never merge" rule lives in one place. Only
    // the emission differs per shape — operator (here) / comma / dot — which is
    // intentional (separator placement genuinely differs), not drift.
    fn split_pre_operator_comments(
        &self,
        operand_end: u32,
        gap_end: u32,
        trailing: &mut DocBuf,
        own_line: &mut DocBuf,
    ) {
        let d = self.d();
        // Same shared same-line/later-line classification as the call-argument
        // (`PartitionedComments`) and member-chain (`push_gap_comments_and_break`)
        // gap printers.
        let classified = tsv_lang::ClassifiedComments::from_range(
            self.comments,
            operand_end,
            gap_end,
            self.line_breaks,
        );
        // Same-line comments (blocks, then the at-most-one line comment) trail the
        // operand in source order; `build_trailing_comment_doc` keeps a block inline
        // and routes a line comment through `line_suffix`.
        for &comment in classified
            .trailing_block
            .iter()
            .chain(&classified.trailing_line)
        {
            trailing.push(self.build_trailing_comment_doc(comment));
        }
        // Later-line comments drop to their own line before the operator, in source
        // order.
        for comment in classified.leading_in_source_order() {
            own_line.push(d.hardline());
            own_line.push(self.build_comment_doc(comment));
        }
    }

    /// Build expression doc for a ternary branch (consequent/alternate).
    ///
    /// When `indent_binary` is true (grandparent is return/throw/call/new),
    /// binary expressions use continuation indent matching Prettier's
    /// shouldNotIndent=false for these contexts (binaryish.js:109-113).
    fn build_ternary_branch_expr_doc(
        &self,
        expr: &internal::Expression<'_>,
        indent_binary: bool,
        boundary_end: u32,
    ) -> DocId {
        let doc = if indent_binary && let internal::Expression::BinaryExpression(binary) = expr {
            self.build_binary_chain_doc_with_continuation_indent(binary)
        } else {
            self.build_expression_doc_with_paren_comments(expr, boundary_end)
        };
        // Parenthesize an `in` consequent/alternate inside a for-header init
        // (`for (a = c ? (b in c) : 0;…)`); a no-op elsewhere. Prettier wraps every
        // `in` under the init; the alternate is `[~In]` so there it is load-bearing.
        self.wrap_for_init_in(expr, doc)
    }
}
