// Control flow statement printing for TypeScript
//
// Statement families live in submodules; this mod.rs keeps the helpers they
// share (comment partitioning, keyword/paren comment placement, and the
// condition-group builders used across the statement families).
//
// - if_else.rs: if/else statements and else-clause layout
// - loops/: for / for-in / for-of headers and bodies (for_loop.rs), while, do-while (while_loop.rs)
// - switch.rs: switch statements and case bodies
// - try_jump.rs: try/catch/finally, throw, break/continue, labeled statements

mod if_else;
mod loops;
mod switch;
mod try_jump;

use smallvec::SmallVec;

use crate::ast::internal::{Expression, Statement, UnaryOperator};
use crate::printer::{CommentVec, Printer};
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;
use tsv_lang::{Comment, comments_in_range};

impl<'a> Printer<'a> {
    /// Build a control-flow *body* whose empty block form collapses (`do {} while (cond)`,
    /// C-style `for (…) {}`). The generic `build_statement_doc` dispatch EXPANDS a
    /// statement-position empty block to `{\n}`, so a collapse-context body must build a
    /// `BlockStatement` directly via the collapse path; a non-block body keeps the generic
    /// dispatch (a non-empty block is identical either way — `expand_empty` only affects the
    /// empty case). The `while` handler and `catch` inline their own block builds (extra
    /// close-paren handling / an always-block body), so they don't route through here.
    fn build_collapsing_body_doc(&self, body: &Statement<'_>) -> DocId {
        if let Statement::BlockStatement(block) = body {
            self.build_block_statement_doc(block)
        } else {
            self.build_statement_doc(body)
        }
    }

    /// Partition comments between two positions into inline vs own-line.
    ///
    /// Returns `(inline_with_prev, own_line, inline_with_next)` where:
    /// - `inline_with_prev`: Comments on the same line as `prev_end`
    /// - `own_line`: Comments on their own line (not same line as prev or next)
    /// - `inline_with_next`: Comments on the same line as `next_start`
    ///
    /// This helper reduces repetitive comment classification code throughout
    /// control flow statement printing.
    fn partition_comments_by_line(
        &self,
        prev_end: u32,
        next_start: u32,
    ) -> (CommentVec<'a>, CommentVec<'a>, CommentVec<'a>) {
        let mut inline_prev = SmallVec::new();
        let mut own_line = SmallVec::new();
        let mut inline_next = SmallVec::new();

        for comment in comments_in_range(self.comments, prev_end, next_start) {
            let same_line_as_prev = self.is_same_line(prev_end, comment.span.start);
            let same_line_as_next = self.is_same_line(comment.span.end, next_start);

            if same_line_as_prev {
                inline_prev.push(comment);
            } else if same_line_as_next {
                inline_next.push(comment);
            } else {
                own_line.push(comment);
            }
        }

        (inline_prev, own_line, inline_next)
    }

    /// Build comments between a keyword and its `(`, preserving position.
    ///
    /// Returns a doc for comments between `keyword_end` and `open_paren` if any exist.
    /// Example: `if/* c */(a)` → `if /* c */ (a)` (comment stays between keyword and paren)
    fn build_keyword_paren_comments(
        &self,
        keyword_end: u32,
        open_paren: Option<u32>,
    ) -> Option<DocId> {
        open_paren.and_then(|op| self.build_inline_comments_between_doc_opt(keyword_end, op))
    }

    /// Build docs for comments between statement parts (e.g., between `}` and `else`).
    ///
    /// Handles:
    /// - Inline comments: added with leading space on same line
    /// - Own-line comments: added with hardline; the first hugs the previous
    ///   token (a leading blank in the gap is dropped), while blank lines between
    ///   subsequent comments are preserved
    ///
    /// Returns the end position after the last comment (for tracking).
    fn build_comments_between_parts(
        &self,
        parts: &mut DocBuf,
        inline_prev: &[&Comment],
        own_line: &[&Comment],
        prev_end: u32,
    ) -> u32 {
        let d = self.d();
        // Trailing comments stay on same line
        for comment in inline_prev {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }

        // Own-line comments: the *first* one hugs the previous token — an authored
        // blank line in a control-flow gap before a body-leading comment is always
        // dropped, so a body block's `{` never sits below a blank. Uniform across
        // `if`/`while`/`for`/`do`/`else`/`try`/`catch`/`switch`/… and consistent
        // with tsv's own handling when `{` is on the header line (`if (a) {\n\n// c`
        // also collapses). Blanks *between* subsequent comments are preserved.
        let mut end = prev_end;
        for comment in own_line {
            let keep_blank =
                end != prev_end && self.has_blank_line_between(end, comment.span.start);
            if keep_blank {
                // Blank line then comment: literalline (empty) + hardline (indented)
                parts.push(d.literalline());
                parts.push(d.hardline());
            } else {
                parts.push(d.hardline());
            }
            parts.push(self.build_comment_doc(comment));
            end = comment.span.end;
        }
        end
    }

    /// Append `)` + comments + `;` for empty statement bodies.
    ///
    /// Handles comments between `)` and `;`:
    /// - Block comments: `if (a) /* comment */ ;`
    /// - Line comments: `if (a) // comment\n;`
    /// - No comments: `if (a);`
    fn append_close_paren_empty_stmt_with_comments(
        &self,
        parts: &mut DocBuf,
        paren_end: u32,
        empty_start: u32,
    ) {
        let d = self.d();
        parts.push(d.text(")"));
        if self.has_comments_between(paren_end, empty_start) {
            let has_line = self.has_line_comments_between(paren_end, empty_start);
            let comment_doc =
                self.build_inline_comments_between_doc_no_leading_space(paren_end, empty_start);
            if has_line {
                parts.push(d.text(" "));
                parts.push(comment_doc);
                parts.push(d.hardline());
                parts.push(d.text(";"));
            } else {
                parts.push(d.text(" "));
                parts.push(comment_doc);
                parts.push(d.text(" ;"));
            }
        } else {
            parts.push(d.text(";"));
        }
    }

    /// Append `) ` to parts, extracting any comments between the close paren and body.
    ///
    /// Used for block bodies: if, while, for-in/for-of `{ }`. For non-block bodies in
    /// for-in/for-of, use `append_close_paren_with_non_block_body` which also indents.
    ///
    /// Block comments are always inlined (trailing after `)`). Line comments preserve
    /// their position: trailing stays trailing, own-line stays on its own line (with
    /// blank line preservation). Line comments force a hardline before the body.
    fn append_close_paren_with_comments(
        &self,
        parts: &mut DocBuf,
        paren_end: u32,
        body_start: u32,
    ) {
        let d = self.d();
        if self.has_comments_between(paren_end, body_start) {
            let (mut inline_prev, own_line, inline_next) =
                self.partition_comments_by_line(paren_end, body_start);

            // Own-line block comments become inline — block comments are flexible
            // and should normalize to trailing position (matches prettier).
            // Only line comments preserve own-line position.
            // inline_next (comments on same line as body `{`) are treated same as own_line.
            let mut own_line_lines: CommentVec<'_> = SmallVec::new();
            for comment in own_line.into_iter().chain(inline_next) {
                if comment.is_block {
                    inline_prev.push(comment);
                } else {
                    own_line_lines.push(comment);
                }
            }

            parts.push(d.text(")"));
            // Use the end of the last inline comment for blank-line detection in the
            // own-line loop — reclassified block comments shift the reference point.
            let effective_prev_end = inline_prev.last().map_or(paren_end, |c| c.span.end);
            self.build_comments_between_parts(
                parts,
                &inline_prev,
                &own_line_lines,
                effective_prev_end,
            );

            // Line comments force a hardline before body; block-only gets a space.
            if !own_line_lines.is_empty() || inline_prev.iter().any(|c| !c.is_block) {
                parts.push(d.hardline());
            } else {
                parts.push(d.text(" "));
            }
        } else {
            parts.push(d.text(") "));
        }
    }

    /// Build an adjust-clause doc with head-body comment handling for non-block bodies.
    ///
    /// Used by if/while for `stmt (cond) /* c */ fn();` and `stmt (cond) // c\n fn();`.
    /// Returns the full `keyword (condition) body` doc including comments when present.
    ///
    /// `head_parts` are the docs before the `)` (e.g., `["if (", condition_group]`).
    fn build_adjust_clause_with_comments(
        &self,
        head_parts: &[DocId],
        paren_end: u32,
        body_start: u32,
        body_doc: DocId,
    ) -> DocId {
        let d = self.d();
        if self.has_comments_between(paren_end, body_start) {
            let has_line = self.has_line_comments_between(paren_end, body_start);
            let comment_doc =
                self.build_inline_comments_between_doc_no_leading_space(paren_end, body_start);
            let mut parts: DocBuf = SmallVec::from_slice(head_parts);
            parts.push(d.text(")"));
            if has_line {
                // Line comment forces break: stmt (cond)\n\t// comment\n\tfn();
                parts.push(d.indent(d.concat(&[
                    d.hardline(),
                    comment_doc,
                    d.hardline(),
                    body_doc,
                ])));
                d.concat(&parts)
            } else {
                // Block comment stays with statement: stmt (cond) /* c */ fn();
                // When broken: stmt (cond)\n\t/* c */ fn();
                parts.push(d.indent(d.concat(&[d.line(), comment_doc, d.text(" "), body_doc])));
                d.group(d.concat(&parts))
            }
        } else {
            let mut parts: DocBuf = SmallVec::from_slice(head_parts);
            parts.push(d.text(")"));
            parts.push(d.indent_line(body_doc));
            d.group(d.concat(&parts))
        }
    }

    /// Prettier's `shouldInlineCondition` (miscellaneous.js): a `!` / `!!`-negated
    /// parenthesized logical condition (`if (!(a || b))`, `while (!!(a && b))`) hugs
    /// the `(` instead of breaking onto its own line, so the whole statement reads
    /// `if (!(` … `)) {` rather than `if (⏎ !(…) ⏎) {`.
    ///
    /// True iff the test is `!X` or `!!X` (but not `!!!X`) where `X` is a *logical*
    /// binary expression. This matches only the `printIfOrWhileConditionOrWithStatementObject`
    /// callers (`if` / `while` / `do-while`), never `switch`. Comments on the condition
    /// disable inlining upstream — the caller only reaches the bare-doc path when the
    /// condition parens hold no comments.
    fn condition_should_inline_negation(&self, test: &Expression<'_>) -> bool {
        let Expression::UnaryExpression(outer) = test else {
            return false;
        };
        if outer.operator != UnaryOperator::Bang {
            return false;
        }
        // Peel one optional inner `!` (so `!` and `!!` qualify; a third `!` leaves a
        // UnaryExpression here and fails the logical-binary check below).
        let inner = match outer.argument {
            Expression::UnaryExpression(u) if u.operator == UnaryOperator::Bang => u.argument,
            other => other,
        };
        matches!(inner, Expression::BinaryExpression(b) if b.operator.is_logical())
    }

    /// Build the condition doc for `if` / `while`, honoring the negation-inline rule.
    ///
    /// Mirrors Prettier's `printIfOrWhileConditionOrWithStatementObject`: when
    /// `condition_should_inline_negation` holds (and the parens carry no comments) the
    /// test doc is emitted bare so `!(…)` hugs `(`; otherwise the standard condition
    /// group wraps it. `switch` and the do-while comment-preservation path build their
    /// condition group directly and are deliberately excluded.
    fn build_statement_condition_doc(
        &self,
        test: &Expression<'_>,
        open_paren: Option<u32>,
        close_paren: Option<u32>,
    ) -> DocId {
        if self.condition_should_inline_negation(test) {
            let no_comments = match (open_paren, close_paren) {
                (Some(open), Some(close)) => !self.has_comments_between(open + 1, close),
                _ => true,
            };
            if no_comments {
                return self.build_condition_doc(test);
            }
        }
        match (open_paren, close_paren) {
            (Some(open), Some(close)) => {
                self.build_condition_group_with_comments(test, open, close)
            }
            _ => self.build_condition_group(test),
        }
    }

    /// Build a condition group for if/while/for/switch statements
    ///
    /// Creates the standard Prettier condition structure:
    /// ```text
    /// group([indent([softline, condition]), softline])
    /// ```
    ///
    /// This group decides whether the condition breaks (operators go to new lines).
    /// Binary expressions use ungrouped version so this parent group controls their breaking.
    fn build_condition_group(&self, test_expr: &Expression<'_>) -> DocId {
        let d = self.d();
        let test_doc = self.build_condition_doc(test_expr);
        d.group(d.concat(&[d.indent_softline(test_doc), d.softline()]))
    }

    /// Build a condition group with comment support for if/while/do-while/switch statements
    ///
    /// Handles comments inside condition/discriminant parens:
    /// ```js
    /// if (
    ///     // before condition
    ///     x // inline with condition
    ///     // trailing after condition
    /// ) {
    /// ```
    fn build_condition_group_with_comments(
        &self,
        test_expr: &Expression<'_>,
        open_paren_pos: u32,
        close_paren_pos: u32,
    ) -> DocId {
        self.build_condition_group_with_comments_impl(
            test_expr,
            open_paren_pos,
            close_paren_pos,
            false, // normalize inline comments to own line
        )
    }

    /// Build condition group preserving inline comments after open paren
    ///
    /// Used for do-while where we intentionally differ from Prettier's behavior
    /// of moving comments outside the parens.
    fn build_condition_group_preserve_inline(
        &self,
        test_expr: &Expression<'_>,
        open_paren_pos: u32,
        close_paren_pos: u32,
    ) -> DocId {
        self.build_condition_group_with_comments_impl(
            test_expr,
            open_paren_pos,
            close_paren_pos,
            true, // preserve inline comments
        )
    }

    fn build_condition_group_with_comments_impl(
        &self,
        test_expr: &Expression<'_>,
        open_paren_pos: u32,
        close_paren_pos: u32,
        preserve_inline: bool,
    ) -> DocId {
        let d = self.d();
        let test_start = test_expr.span().start;
        let test_end = test_expr.span().end;

        // Check for comments before and after the condition
        let has_leading = self.has_comments_between(open_paren_pos + 1, test_start);
        let has_trailing = self.has_comments_between(test_end, close_paren_pos);

        if !has_leading && !has_trailing {
            // No comments - use the standard condition group
            return self.build_condition_group(test_expr);
        }

        // Build with comments
        let test_doc = self.build_condition_doc(test_expr);
        let mut inner_parts = DocBuf::new();

        // Collect leading comments
        // Classification based on position relative to open paren AND condition:
        // - "inline with open paren" = comment STARTS on same line as open paren
        // - "own line" = comment does NOT start on same line as open paren
        let leading_comments: CommentVec<'_> = if has_leading {
            comments_in_range(self.comments, open_paren_pos + 1, test_start).collect()
        } else {
            SmallVec::new()
        };

        // Check if there are own-line leading comments (not on same line as open paren)
        let has_own_line_leading = leading_comments
            .iter()
            .any(|c| !self.is_same_line(open_paren_pos, c.span.start));

        if preserve_inline {
            // Preserve inline comments after open paren (used for do-while divergence)
            let mut has_inline_comment_followed_by_newline = false;

            // Leading inline comments (on same line as open paren)
            for comment in &leading_comments {
                if self.is_same_line(open_paren_pos, comment.span.start) {
                    // Only add space if source has whitespace between ( and comment
                    let space_between =
                        &self.source[(open_paren_pos + 1) as usize..comment.span.start as usize];
                    if !space_between.is_empty() {
                        inner_parts.push(d.text(" "));
                    }
                    inner_parts.push(self.build_comment_doc(comment));
                    if !self.is_same_line(comment.span.end, test_start) {
                        has_inline_comment_followed_by_newline = true;
                    } else {
                        inner_parts.push(d.text(" "));
                    }
                }
            }

            if has_inline_comment_followed_by_newline {
                inner_parts.push(d.hardline());
            }

            // Own-line comments
            for comment in &leading_comments {
                if !self.is_same_line(open_paren_pos, comment.span.start) {
                    if !has_inline_comment_followed_by_newline {
                        inner_parts.push(d.hardline());
                    }
                    inner_parts.push(self.build_comment_doc(comment));
                    if !self.is_same_line(comment.span.end, test_start) {
                        inner_parts.push(d.hardline());
                    } else {
                        inner_parts.push(d.text(" "));
                    }
                }
            }

            if !has_inline_comment_followed_by_newline && !has_own_line_leading {
                inner_parts.push(d.softline());
            }
        } else {
            // Normalize comments based on their position:
            // - Comments on own line (not same line as open paren): force break with hardline
            // - Comments inline with open paren: allow collapsing with softline
            let mut added_comment = false;
            let mut last_comment_same_line_as_test = false;
            for comment in &leading_comments {
                let on_same_line_as_open = self.is_same_line(open_paren_pos, comment.span.start);

                if on_same_line_as_open {
                    // Comment is inline with open paren - use softline to allow collapse
                    inner_parts.push(d.softline());
                } else {
                    // Comment is on its own line - force break
                    inner_parts.push(d.hardline());
                }
                inner_parts.push(self.build_comment_doc(comment));
                added_comment = true;

                // Check if condition is on same line as comment end
                last_comment_same_line_as_test = self.is_same_line(comment.span.end, test_start);
                // Space if on same line, hardline if on different line
                if last_comment_same_line_as_test {
                    inner_parts.push(d.text(" "));
                } else if !comment.is_block {
                    // Line comment - need hardline before condition (next comment iteration will add it, or we add it below)
                }
            }

            // Add softline before condition if no comments were added
            // If we added comments and the last one wasn't on same line as test, we need hardline
            if !added_comment {
                inner_parts.push(d.softline());
            } else if !last_comment_same_line_as_test {
                inner_parts.push(d.hardline());
            }
        }

        // The condition itself
        inner_parts.push(test_doc);

        // Trailing comments use partition_comments_by_line since the classification matches:
        // inline = starts on same line as test_end (goes to inline_prev)
        // own line = doesn't start on same line as test_end
        let (trailing_inline, trailing_own_line, _) =
            self.partition_comments_by_line(test_end, close_paren_pos);

        // Trailing inline comments (same line as condition)
        for comment in &trailing_inline {
            inner_parts.push(d.text(" "));
            inner_parts.push(self.build_comment_doc(comment));
        }

        // Trailing comments on their own line (after condition)
        for comment in &trailing_own_line {
            inner_parts.push(d.hardline());
            inner_parts.push(self.build_comment_doc(comment));
        }

        // Structure: group([indent([softline/hardline, comments, condition, comments]), softline/hardline])
        // The closing softline/hardline is OUTSIDE the indent so `)` aligns with `(`
        // Force break when trailing inline line comments exist — flattening would cause
        // the // comment to swallow the closing `) {` producing unparseable output
        let has_trailing_line_comment = trailing_inline.iter().any(|c| !c.is_block);
        let closing =
            if has_own_line_leading || !trailing_own_line.is_empty() || has_trailing_line_comment {
                d.hardline()
            } else {
                d.softline()
            };

        d.group(d.concat(&[d.indent(d.concat(&inner_parts)), closing]))
    }

    /// Find the position of the opening paren for a keyword statement
    /// Returns the position of '(' after the keyword.
    ///
    /// Skips `(` characters inside comments and strings (`if /* (note) */ (cond)`),
    /// so a parenthesis in a leading comment can't be mistaken for the condition's
    /// open paren.
    fn find_open_paren_after(&self, start: u32) -> Option<u32> {
        find_char_skipping_comments(
            self.source.as_bytes(),
            start as usize,
            self.source.len(),
            b'(',
        )
        .map(|p| p as u32)
    }

    /// Build a doc for a condition expression (if/while/for test)
    ///
    /// For binary expressions, uses ungrouped version so parent group controls breaking.
    /// Logical operators (`&&`, `||`, `??`) break with the parent condition group.
    /// Non-logical operators (`<`, `===`, etc.) keep a sub-group for independent evaluation
    /// (e.g., `for (i = 0; i < len; i++)` — the `i < len` stays flat).
    /// Assignment expressions get double-parens for clarity: `while ((x = y))`
    fn build_condition_doc(&self, expr: &Expression<'_>) -> DocId {
        let inner = match expr {
            Expression::BinaryExpression(binary) => {
                self.build_binary_chain_doc_ungrouped_condition(binary)
            }
            _ => self.build_expression_doc(expr),
        };
        if self.needs_parens(expr, super::ParenContext::StatementTest) {
            let d = self.d();
            d.parens(inner)
        } else {
            inner
        }
    }
}
