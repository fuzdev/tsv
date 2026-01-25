// Control flow statement printing for TypeScript

use smallvec::SmallVec;

use super::Printer;
use crate::ast::internal::{self, Statement};
use tsv_lang::{SymbolToU32, doc};

/// Small vector of comment references, stack-allocated for typical cases.
type CommentVec<'a> = SmallVec<[&'a tsv_lang::Comment; 2]>;

/// Span positions for a for loop header
///
/// Groups the optional span positions for init, test, and update clauses
/// to avoid passing many Option parameters.
struct ForHeaderSpans {
    open_paren: Option<u32>,
    init_start: Option<u32>,
    init_end: Option<u32>,
    test_start: Option<u32>,
    test_end: Option<u32>,
    update_start: Option<u32>,
    close_paren: Option<u32>,
}

/// Check if a statement can be printed inline after `if (cond)` without a newline.
///
/// Block, expression, break, continue, return, throw, and empty statements stay inline.
/// Other statements (if, for, while, etc.) go on a new line with indent.
fn is_inline_consequent(stmt: &Statement) -> bool {
    matches!(
        stmt,
        Statement::BlockStatement(_)
            | Statement::ExpressionStatement(_)
            | Statement::BreakStatement(_)
            | Statement::ContinueStatement(_)
            | Statement::ReturnStatement(_)
            | Statement::ThrowStatement(_)
            | Statement::EmptyStatement(_)
    )
}

/// Check if a statement can be printed inline after `else` without a newline.
///
/// Same as `is_inline_consequent` but also allows IfStatement for else-if chains.
fn is_inline_alternate(stmt: &Statement) -> bool {
    is_inline_consequent(stmt) || matches!(stmt, Statement::IfStatement(_))
}

impl<'a> Printer<'a> {
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

        for comment in tsv_lang::comments_in_range(self.comments, prev_end, next_start) {
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

    /// Build docs for comments between statement parts (e.g., between `}` and `else`).
    ///
    /// Handles:
    /// - Inline comments: added with leading space on same line
    /// - Own-line comments: added with hardline, preserving blank lines before them
    ///
    /// Returns the end position after the last comment (for tracking).
    fn build_comments_between_parts(
        &self,
        parts: &mut Vec<doc::Doc>,
        inline_prev: &[&tsv_lang::Comment],
        own_line: &[&tsv_lang::Comment],
        prev_end: u32,
    ) -> u32 {
        // Trailing comments stay on same line
        for comment in inline_prev {
            parts.push(doc::text(" "));
            parts.push(self.build_comment_doc(comment));
        }

        // Own-line comments: preserve blank lines before them
        let mut end = prev_end;
        for comment in own_line {
            if self.has_blank_line_between(end, comment.span.start) {
                // Blank line then comment: literalline (empty) + hardline (indented)
                parts.push(doc::literalline());
                parts.push(doc::hardline());
            } else {
                parts.push(doc::hardline());
            }
            parts.push(self.build_comment_doc(comment));
            end = comment.span.end;
        }
        end
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
    fn build_condition_group(&self, test_expr: &internal::Expression) -> doc::Doc {
        let test_doc = self.build_condition_doc(test_expr);
        doc::group(doc::concat(vec![
            doc::indent(doc::concat(vec![doc::softline(), test_doc])),
            doc::softline(),
        ]))
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
        test_expr: &internal::Expression,
        open_paren_pos: u32,
        close_paren_pos: u32,
    ) -> doc::Doc {
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
        test_expr: &internal::Expression,
        open_paren_pos: u32,
        close_paren_pos: u32,
    ) -> doc::Doc {
        self.build_condition_group_with_comments_impl(
            test_expr,
            open_paren_pos,
            close_paren_pos,
            true, // preserve inline comments
        )
    }

    fn build_condition_group_with_comments_impl(
        &self,
        test_expr: &internal::Expression,
        open_paren_pos: u32,
        close_paren_pos: u32,
        preserve_inline: bool,
    ) -> doc::Doc {
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
        let mut inner_parts = Vec::new();

        // Collect leading comments
        // Classification based on position relative to open paren AND condition:
        // - "inline with open paren" = comment STARTS on same line as open paren
        // - "own line" = comment does NOT start on same line as open paren
        let leading_comments: Vec<_> = if has_leading {
            tsv_lang::comments_in_range(self.comments, open_paren_pos + 1, test_start).collect()
        } else {
            Vec::new()
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
                        inner_parts.push(doc::text(" "));
                    }
                    inner_parts.push(self.build_comment_doc(comment));
                    if !self.is_same_line(comment.span.end, test_start) {
                        has_inline_comment_followed_by_newline = true;
                    } else {
                        inner_parts.push(doc::text(" "));
                    }
                }
            }

            if has_inline_comment_followed_by_newline {
                inner_parts.push(doc::hardline());
            }

            // Own-line comments
            for comment in &leading_comments {
                if !self.is_same_line(open_paren_pos, comment.span.start) {
                    if !has_inline_comment_followed_by_newline {
                        inner_parts.push(doc::hardline());
                    }
                    inner_parts.push(self.build_comment_doc(comment));
                    if !self.is_same_line(comment.span.end, test_start) {
                        inner_parts.push(doc::hardline());
                    } else {
                        inner_parts.push(doc::text(" "));
                    }
                }
            }

            if !has_inline_comment_followed_by_newline && !has_own_line_leading {
                inner_parts.push(doc::softline());
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
                    inner_parts.push(doc::softline());
                } else {
                    // Comment is on its own line - force break
                    inner_parts.push(doc::hardline());
                }
                inner_parts.push(self.build_comment_doc(comment));
                added_comment = true;

                // Check if condition is on same line as comment end
                last_comment_same_line_as_test = self.is_same_line(comment.span.end, test_start);
                // Space if on same line, hardline if on different line
                if last_comment_same_line_as_test {
                    inner_parts.push(doc::text(" "));
                } else if !comment.is_block {
                    // Line comment - need hardline before condition (next comment iteration will add it, or we add it below)
                }
            }

            // Add softline before condition if no comments were added
            // If we added comments and the last one wasn't on same line as test, we need hardline
            if !added_comment {
                inner_parts.push(doc::softline());
            } else if !last_comment_same_line_as_test {
                inner_parts.push(doc::hardline());
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
            inner_parts.push(doc::text(" "));
            inner_parts.push(self.build_comment_doc(comment));
        }

        // Trailing comments on their own line (after condition)
        for comment in &trailing_own_line {
            inner_parts.push(doc::hardline());
            inner_parts.push(self.build_comment_doc(comment));
        }

        // Structure: group([indent([softline/hardline, comments, condition, comments]), softline/hardline])
        // The closing softline/hardline is OUTSIDE the indent so `)` aligns with `(`
        let closing = if has_own_line_leading || !trailing_own_line.is_empty() {
            doc::hardline()
        } else {
            doc::softline()
        };

        doc::group(doc::concat(vec![
            doc::indent(doc::concat(inner_parts)),
            closing,
        ]))
    }

    /// Find the position of the opening paren for a keyword statement
    /// Returns the position of '(' after the keyword
    fn find_open_paren_after(&self, start: u32) -> Option<u32> {
        self.source[start as usize..]
            .find('(')
            .map(|p| start + p as u32)
    }

    /// Find the position of the closing paren after a given position
    fn find_close_paren_after(&self, start: u32) -> Option<u32> {
        self.source[start as usize..]
            .find(')')
            .map(|p| start + p as u32)
    }

    /// Build a doc for an if statement with proper line-width wrapping
    ///
    /// Matches Prettier's architecture from estree.js:
    /// ```js
    /// group([
    ///   "if (",
    ///   group([indent([softline, test]), softline]),  // inner group for condition
    ///   ")",
    ///   adjustClause(consequent),  // body handling
    /// ])
    /// ```
    fn build_if_statement_with_wrapping_doc(&self, stmt: &internal::IfStatement) -> doc::Doc {
        let is_block = matches!(stmt.consequent.as_ref(), Statement::BlockStatement(_));
        let is_empty = matches!(stmt.consequent.as_ref(), Statement::EmptyStatement(_));

        // Find paren positions for comment handling
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren = self.find_close_paren_after(stmt.test.span().end);

        // Build condition group (handles breaking within condition and comments)
        let condition_group = if let (Some(open), Some(close)) = (open_paren, close_paren) {
            self.build_condition_group_with_comments(&stmt.test, open, close)
        } else {
            self.build_condition_group(&stmt.test)
        };

        if is_block {
            // Block consequent: group(["if (" + condition + ") " + block])
            // Outer group controls whether the whole if statement breaks
            let mut parts = vec![doc::text("if ("), condition_group, doc::text(") ")];

            if let Statement::BlockStatement(block) = stmt.consequent.as_ref() {
                parts.push(self.build_block_statement_expand_empty_doc(block));
            }

            // Handle else clause
            if let Some(alternate) = &stmt.alternate {
                parts.push(doc::text(" else "));
                if let Statement::BlockStatement(block) = alternate.as_ref() {
                    parts.push(self.build_block_statement_expand_empty_doc(block));
                } else if is_inline_alternate(alternate) {
                    parts.push(self.build_statement_doc(alternate));
                } else {
                    parts.push(doc::hardline());
                    parts.push(doc::indent(self.build_statement_doc(alternate)));
                }
            }

            // Outer group for the whole if statement
            doc::group(doc::concat(parts))
        } else if is_empty {
            // Empty statement: `if (cond);`
            doc::group(doc::concat(vec![
                doc::text("if ("),
                condition_group,
                doc::text(");"),
            ]))
        } else {
            // Non-block consequent: use adjustClause equivalent
            // Prettier's adjustClause returns: indent([line, clause])
            // - When flat: line becomes space -> `if (cond) a;`
            // - When broken: line becomes newline + indent -> `if (cond)\n\ta;`
            let consequent_doc = self.build_statement_doc(&stmt.consequent);
            let adjust_clause = doc::indent(doc::concat(vec![doc::line(), consequent_doc]));

            let mut parts = vec![doc::group(doc::concat(vec![
                doc::text("if ("),
                condition_group,
                doc::text(")"),
                adjust_clause,
            ]))];

            // Handle else clause for non-block consequent
            if let Some(alternate) = &stmt.alternate {
                parts.push(doc::hardline());
                parts.push(doc::text("else "));
                if is_inline_alternate(alternate) {
                    if let Statement::BlockStatement(block) = alternate.as_ref() {
                        parts.push(self.build_block_statement_expand_empty_doc(block));
                    } else {
                        parts.push(self.build_statement_doc(alternate));
                    }
                } else {
                    parts.push(doc::hardline());
                    parts.push(doc::indent(self.build_statement_doc(alternate)));
                }
            }

            doc::concat(parts)
        }
    }

    /// Build a doc for a condition expression (if/while/for test)
    ///
    /// For binary expressions, uses ungrouped version so parent group controls breaking.
    /// This ensures all operands break together when the condition exceeds print width.
    fn build_condition_doc(&self, expr: &internal::Expression) -> doc::Doc {
        match expr {
            internal::Expression::BinaryExpression(binary) => {
                // Use ungrouped version so parent group controls breaking
                self.build_binary_chain_doc_ungrouped(binary)
            }
            _ => self.build_expression_doc(expr),
        }
    }

    /// Build a complete for statement doc including the body
    ///
    /// This includes the body in the doc so the width calculation accounts for ` {`.
    fn build_for_statement_with_body_doc(&self, stmt: &internal::ForStatement) -> doc::Doc {
        let header_doc = self.build_for_header_doc(stmt);
        let is_empty = matches!(stmt.body.as_ref(), Statement::EmptyStatement(_));
        let is_block = matches!(stmt.body.as_ref(), Statement::BlockStatement(_));

        if is_empty {
            // No space before empty statement: `for (...);`
            doc::concat(vec![header_doc, self.build_statement_doc(&stmt.body)])
        } else if is_block {
            // Block body: `for (...) { ... }`
            // Note: Unlike for-in/for-of, standard for loops keep empty blocks inline `{}`
            if let Statement::BlockStatement(block) = stmt.body.as_ref() {
                doc::concat(vec![
                    header_doc,
                    doc::text(" "),
                    self.build_block_statement_doc(block),
                ])
            } else {
                unreachable!()
            }
        } else {
            // Non-block body: `for (...) stmt;`
            doc::concat(vec![
                header_doc,
                doc::text(" "),
                self.build_statement_doc(&stmt.body),
            ])
        }
    }

    /// Get the end position of a for loop header (position after the closing paren)
    fn get_for_header_end(&self, stmt: &internal::ForStatement) -> u32 {
        // Find the last expression end
        let last_expr_end = stmt
            .update
            .as_ref()
            .map(|u| u.span().end)
            .or_else(|| stmt.test.as_ref().map(|t| t.span().end))
            .or_else(|| stmt.init.as_ref().map(|i| self.get_for_init_span_end(i)));

        // Find closing paren after last expression (or after "for (" if empty)
        let search_start = last_expr_end.unwrap_or(stmt.span.start + 4);
        self.find_close_paren_after(search_start)
            .map_or(search_start, |p| p + 1)
    }

    /// Build a Doc for the for loop header with wrapping support
    ///
    /// Handles comments in each clause position:
    /// ```js
    /// for (
    ///     // before init
    ///     let i = 0; // inline with init
    ///     // before test
    ///     i < 10; // inline with test
    ///     // before update
    ///     i++ // inline with update
    /// ) {
    /// ```
    fn build_for_header_doc(&self, stmt: &internal::ForStatement) -> doc::Doc {
        self.build_for_header_doc_impl(stmt, false)
    }

    /// Build doc for empty for (;;) with comments inside
    ///
    /// Preserves comments in their original positions (divergence from prettier).
    /// Format: for (\n\t; // comment\n\t; // comment\n\t// comment\n)
    fn build_for_empty_with_comments(&self, stmt: &internal::ForStatement) -> doc::Doc {
        let Some(open_paren) = self.find_open_paren_after(stmt.span.start) else {
            return doc::text("for (;;)");
        };
        let Some(close_paren) = self.find_close_paren_after(open_paren) else {
            return doc::text("for (;;)");
        };

        // Find the two semicolons
        let first_semi = self.source[open_paren as usize..]
            .find(';')
            .map(|p| open_paren + p as u32);
        let second_semi = first_semi.and_then(|p| {
            self.source[(p + 1) as usize..]
                .find(';')
                .map(|off| p + 1 + off as u32)
        });

        let mut parts = vec![doc::text("for (")];
        let mut inner_parts = Vec::new();

        // First semicolon line: ; // inline comment
        inner_parts.push(doc::hardline());
        inner_parts.push(doc::text(";"));
        if let (Some(semi1), Some(semi2)) = (first_semi, second_semi) {
            for comment in tsv_lang::comments_in_range(self.comments, semi1 + 1, semi2) {
                if self.is_same_line(semi1, comment.span.start) {
                    inner_parts.push(doc::text(" "));
                    inner_parts.push(self.build_comment_doc(comment));
                }
            }
        }

        // Second semicolon line: ; // inline comment
        inner_parts.push(doc::hardline());
        inner_parts.push(doc::text(";"));

        // Comments after second semicolon: inline first, then own-line
        if let Some(semi2) = second_semi {
            let mut own_line_comments = Vec::new();
            for comment in tsv_lang::comments_in_range(self.comments, semi2 + 1, close_paren) {
                if self.is_same_line(semi2, comment.span.start) {
                    inner_parts.push(doc::text(" "));
                    inner_parts.push(self.build_comment_doc(comment));
                } else {
                    own_line_comments.push(comment);
                }
            }
            for comment in own_line_comments {
                inner_parts.push(doc::hardline());
                inner_parts.push(self.build_comment_doc(comment));
            }
        }

        parts.push(doc::indent(doc::concat(inner_parts)));
        parts.push(doc::hardline());
        parts.push(doc::text(")"));

        doc::concat(parts)
    }

    fn build_for_header_doc_breaking(&self, stmt: &internal::ForStatement) -> doc::Doc {
        self.build_for_header_doc_impl(stmt, true)
    }

    fn build_for_header_doc_impl(
        &self,
        stmt: &internal::ForStatement,
        force_break: bool,
    ) -> doc::Doc {
        let has_init = stmt.init.is_some();
        let has_test = stmt.test.is_some();
        let has_update = stmt.update.is_some();
        let has_any = has_init || has_test || has_update;

        // Check if there are any comments inside the for parens
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren_approx = open_paren.and_then(|p| self.find_close_paren_after(p));
        let has_comments_inside =
            if let (Some(open), Some(close)) = (open_paren, close_paren_approx) {
                self.has_comments_between(open, close)
            } else {
                false
            };

        if !has_any && !has_comments_inside {
            // Empty for (;;) with no comments - no wrapping needed
            return doc::text("for (;;)");
        }

        if !has_any && has_comments_inside {
            // Empty for (;;) with comments - need to preserve them
            // This is a divergence from prettier (see for_empty_clauses_prettier_divergence)
            return self.build_for_empty_with_comments(stmt);
        }

        // Find paren position for detecting leading comments before init
        let open_paren = self.find_open_paren_after(stmt.span.start);

        // Determine spans for each part
        let init_end = stmt.init.as_ref().map(|i| self.get_for_init_span_end(i));
        let test_end = stmt.test.as_ref().map(|t| t.span().end);
        let update_end = stmt.update.as_ref().map(|u| u.span().end);

        let spans = ForHeaderSpans {
            open_paren,
            init_start: stmt.init.as_ref().map(|i| match i {
                internal::ForInit::VariableDeclaration(d) => d.span.start,
                internal::ForInit::Expression(e) => e.span().start,
            }),
            init_end,
            test_start: stmt.test.as_ref().map(|t| t.span().start),
            test_end,
            update_start: stmt.update.as_ref().map(|u| u.span().start),
            close_paren: update_end
                .or(test_end)
                .or(init_end)
                .and_then(|e| self.find_close_paren_after(e)),
        };

        // Check if we have any own-line comments that force expansion
        let has_own_line_comments = force_break || self.for_header_has_own_line_comments(&spans);

        // Extract span positions for use throughout this function
        let init_start = spans.init_start;
        let test_start = spans.test_start;
        let update_start = spans.update_start;
        let close_paren = spans.close_paren;

        let mut inner_parts = Vec::new();

        // Leading comments before init (after open paren)
        // Handles both own-line comments (with hardlines) and inline block comments
        if let (Some(open), Some(first_start)) =
            (open_paren, init_start.or(test_start).or(update_start))
        {
            let leading = self.build_for_clause_leading_comments(open + 1, first_start);
            if !leading.is_empty() {
                inner_parts.extend(leading);
            }

            // Inline block comments before the first clause (on the same line)
            // e.g., `for (/* before init */ let j = 0; ...)`
            for comment in tsv_lang::comments_in_range(self.comments, open + 1, first_start) {
                if comment.is_block && self.is_same_line(comment.span.end, first_start) {
                    inner_parts.push(self.build_comment_doc(comment));
                    inner_parts.push(doc::text(" "));
                }
            }
        }

        // Find semicolon positions for proper comment boundary detection
        // The semicolons in `for (init; test; update)` are at specific positions in source
        let first_semi = self.source[stmt.span.start as usize..]
            .find(';')
            .map(|p| stmt.span.start + p as u32);
        let second_semi = first_semi.and_then(|p| {
            self.source[(p + 1) as usize..]
                .find(';')
                .map(|off| p + 1 + off as u32)
        });

        // Init part
        if let Some(init) = &stmt.init {
            if inner_parts.is_empty() {
                inner_parts.push(doc::softline());
            }
            inner_parts.push(self.build_for_init_doc(init));
        }
        inner_parts.push(doc::text(";"));

        // Inline comments after init (between semicolon and test, on same line as init)
        if let (Some(semi), Some(end)) = (first_semi, init_end) {
            let boundary = test_start
                .or(update_start)
                .or(close_paren)
                .unwrap_or(stmt.span.end);
            for comment in tsv_lang::comments_in_range(self.comments, semi + 1, boundary) {
                if self.is_same_line(end, comment.span.start) {
                    inner_parts.push(doc::text(" "));
                    inner_parts.push(self.build_comment_doc(comment));
                }
            }
        }

        // Leading comments before test (own line, between first semi and test)
        if let Some(start) = test_start {
            let search_start = first_semi.map_or_else(
                || {
                    init_end.unwrap_or_else(|| {
                        open_paren.map_or_else(|| stmt.span.start + 5, |p| p + 1)
                    })
                },
                |s| s + 1,
            );
            let leading =
                self.build_for_clause_leading_comments_with_prev(search_start, start, init_end);
            if !leading.is_empty() {
                inner_parts.extend(leading);
            } else if has_init {
                inner_parts.push(doc::line());
            }

            // Inline block comments before test (on same line)
            // e.g., `for (let i = 0; /* before test */ i < 10; ...)`
            for comment in tsv_lang::comments_in_range(self.comments, search_start, start) {
                if comment.is_block
                    && self.is_same_line(comment.span.end, start)
                    && init_end.is_none_or(|ie| !self.is_same_line(ie, comment.span.start))
                {
                    inner_parts.push(self.build_comment_doc(comment));
                    inner_parts.push(doc::text(" "));
                }
            }
        } else if has_update {
            inner_parts.push(doc::line());
        }

        // Test part
        if let Some(test) = &stmt.test {
            if !has_init && inner_parts.len() == 1 {
                // Only ";" so far, add line (becomes space in flat mode, newline when breaking)
                inner_parts.push(doc::line());
            }
            inner_parts.push(self.build_expression_doc(test));
        }
        inner_parts.push(doc::text(";"));

        // Inline comments after test (between second semicolon and update, on same line as test)
        if let (Some(semi), Some(end)) = (second_semi, test_end) {
            let boundary = update_start.or(close_paren).unwrap_or(stmt.span.end);
            for comment in tsv_lang::comments_in_range(self.comments, semi + 1, boundary) {
                if self.is_same_line(end, comment.span.start) {
                    inner_parts.push(doc::text(" "));
                    inner_parts.push(self.build_comment_doc(comment));
                }
            }
        }

        // Leading comments before update (own line, between second semi and update)
        if let Some(start) = update_start {
            let search_start = second_semi.map_or_else(
                || {
                    test_end.or(init_end).unwrap_or_else(|| {
                        open_paren.map_or_else(|| stmt.span.start + 5, |p| p + 1)
                    })
                },
                |s| s + 1,
            );
            let leading =
                self.build_for_clause_leading_comments_with_prev(search_start, start, test_end);
            if !leading.is_empty() {
                inner_parts.extend(leading);
            } else {
                inner_parts.push(doc::line());
            }

            // Inline block comments before update (on same line)
            // e.g., `for (let i = 0; i < 10; /* before update */ i++)`
            for comment in tsv_lang::comments_in_range(self.comments, search_start, start) {
                if comment.is_block
                    && self.is_same_line(comment.span.end, start)
                    && test_end.is_none_or(|te| !self.is_same_line(te, comment.span.start))
                {
                    inner_parts.push(self.build_comment_doc(comment));
                    inner_parts.push(doc::text(" "));
                }
            }
        }

        // Update part
        if let Some(update) = &stmt.update {
            if !has_init && !has_test && inner_parts.len() == 2 {
                // Only ";;" so far, add line (becomes space in flat mode)
                inner_parts.push(doc::line());
            }
            inner_parts.push(self.build_for_update_doc(update));
            // Inline comments after update (on same line as update expression)
            if let Some(end) = update_end {
                let boundary = close_paren.unwrap_or(stmt.span.end);
                for comment in tsv_lang::comments_in_range(self.comments, end, boundary) {
                    if self.is_same_line(end, comment.span.start) {
                        inner_parts.push(doc::text(" "));
                        inner_parts.push(self.build_comment_doc(comment));
                    }
                }
            }
        } else if has_test && !has_own_line_comments {
            // Prettier adds trailing space when update is None but test exists (no comments)
            inner_parts.push(doc::if_break(doc::text(""), doc::text(" ")));
        }

        let closing = if has_own_line_comments {
            doc::hardline()
        } else {
            doc::softline()
        };

        doc::group(doc::concat(vec![
            doc::text("for ("),
            doc::indent(doc::concat(inner_parts)),
            closing,
            doc::text(")"),
        ]))
    }

    /// Build leading comments for a for clause (comments on their own line before the clause)
    ///
    /// `search_start` - where to start looking for comments
    /// `clause_start` - start of the next clause
    /// `prev_expr_end` - end of the previous expression (to filter out inline comments)
    fn build_for_clause_leading_comments_with_prev(
        &self,
        search_start: u32,
        clause_start: u32,
        prev_expr_end: Option<u32>,
    ) -> Vec<doc::Doc> {
        let mut parts = Vec::new();
        for comment in tsv_lang::comments_in_range(self.comments, search_start, clause_start) {
            // Only include comments that are:
            // 1. NOT on the same line as the next clause
            // 2. NOT on the same line as the previous expression (inline comments)
            let is_own_line_before_clause = !self.is_same_line(comment.span.end, clause_start);
            let is_own_line_after_prev =
                prev_expr_end.is_none_or(|end| !self.is_same_line(end, comment.span.start));
            if is_own_line_before_clause && is_own_line_after_prev {
                parts.push(doc::hardline());
                parts.push(self.build_comment_doc(comment));
            }
        }
        if !parts.is_empty() {
            parts.push(doc::hardline());
        }
        parts
    }

    /// Build leading comments for a for clause (comments on their own line before the clause)
    fn build_for_clause_leading_comments(&self, start: u32, clause_start: u32) -> Vec<doc::Doc> {
        self.build_for_clause_leading_comments_with_prev(start, clause_start, None)
    }

    /// Check if for header has any own-line comments that force expansion
    fn for_header_has_own_line_comments(&self, spans: &ForHeaderSpans) -> bool {
        // Check for leading comments before first clause
        if let (Some(open), Some(first)) = (
            spans.open_paren,
            spans.init_start.or(spans.test_start).or(spans.update_start),
        ) {
            let (_, own_line, _) = self.partition_comments_by_line(open + 1, first);
            if !own_line.is_empty() {
                return true;
            }
        }

        // Check between init and test (or init and update if no test)
        let after_init = spans.test_start.or(spans.update_start);
        if let (Some(end), Some(start)) = (spans.init_end, after_init) {
            let (_, own_line, _) = self.partition_comments_by_line(end, start);
            if !own_line.is_empty() {
                return true;
            }
        }

        // Check between test and update
        if let (Some(end), Some(start)) = (spans.test_end, spans.update_start) {
            let (_, own_line, _) = self.partition_comments_by_line(end, start);
            if !own_line.is_empty() {
                return true;
            }
        }

        false
    }

    /// Build a Doc for a for loop update expression
    fn build_for_update_doc(&self, expr: &internal::Expression) -> doc::Doc {
        if let internal::Expression::SequenceExpression(seq) = expr {
            doc::join(
                seq.expressions.iter().map(|e| self.build_expression_doc(e)),
                ", ",
            )
        } else {
            self.build_expression_doc(expr)
        }
    }

    /// Build a complete for-in statement doc including the body
    fn build_for_in_statement_with_body_doc(&self, stmt: &internal::ForInStatement) -> doc::Doc {
        let left_start = self.get_for_in_of_left_start(&stmt.left);
        let left_end = self.get_for_in_of_left_end(&stmt.left);
        let right_start = stmt.right.span().start;
        let right_end = stmt.right.span().end;
        let close_paren = self.find_close_paren_after(right_end);

        // Find 'in' keyword position (search with or without spaces)
        let in_pos = self
            .find_keyword_position(left_end, right_start, "in")
            .unwrap_or(left_end);

        // Check for line comments in the header - if present, use breaking layout
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close = close_paren.unwrap_or(right_end + 1);
        let has_line_comments = if let Some(open) = open_paren {
            self.has_line_comments_in_range(open + 1, close)
        } else {
            self.has_line_comments_in_range(left_start, close)
        };

        if has_line_comments {
            return self.build_for_in_of_with_line_comments(
                &stmt.left,
                &stmt.right,
                &stmt.body,
                stmt.span.start,
                "in",
                in_pos,
                close_paren,
            );
        }

        let mut parts = vec![doc::text("for (")];
        parts.push(self.build_for_in_of_left_doc(&stmt.left));

        // Comments after left, before 'in'
        let has_left_comment =
            self.append_for_in_of_before_keyword_comments(&mut parts, left_end, in_pos);

        if has_left_comment {
            parts.push(doc::text("in"));
        } else {
            parts.push(doc::text(" in"));
        }

        // Comments after 'in', before right
        let in_keyword_end = in_pos + 2; // "in" is 2 chars
        let has_comment =
            self.append_for_in_of_inline_comments(&mut parts, in_keyword_end, right_start);
        if !has_comment {
            parts.push(doc::text(" "));
        }

        parts.push(self.build_expression_doc(&stmt.right));

        // Comments after right, before close paren (no trailing space needed)
        if let Some(close) = close_paren {
            self.append_for_in_of_trailing_comments(&mut parts, right_end, close);
        }

        parts.push(doc::text(") "));

        // Prettier expands empty blocks for for-in
        if let Statement::BlockStatement(block) = stmt.body.as_ref() {
            parts.push(self.build_block_statement_expand_empty_doc(block));
        } else {
            parts.push(self.build_statement_doc(&stmt.body));
        }

        doc::concat(parts)
    }

    /// Find a keyword position between two spans, skipping over comments
    ///
    /// Searches for the keyword with possible surrounding whitespace or comments.
    /// Returns the position where the keyword starts.
    fn find_keyword_position(&self, start: u32, end: u32, keyword: &str) -> Option<u32> {
        let search_range = &self.source[start as usize..end as usize];

        // First try to find " keyword " (with spaces) - outside of comments
        // We need to search manually to avoid matching inside comment content
        let keyword_bytes = keyword.as_bytes();
        let bytes = search_range.as_bytes();
        let mut i = 0;

        while i + keyword.len() <= bytes.len() {
            // Skip over block comments
            if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                // Find end of comment
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2; // Skip past */
                continue;
            }

            // Check if we found the keyword
            if &bytes[i..i + keyword.len()] == keyword_bytes {
                // Check it's not part of an identifier
                let before_ok =
                    i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
                let after_ok = i + keyword.len() >= bytes.len()
                    || !bytes[i + keyword.len()].is_ascii_alphanumeric()
                        && bytes[i + keyword.len()] != b'_';

                if before_ok && after_ok {
                    return Some(start + i as u32);
                }
            }
            i += 1;
        }

        None
    }

    /// Build a complete for-of statement doc including the body
    fn build_for_of_statement_with_body_doc(&self, stmt: &internal::ForOfStatement) -> doc::Doc {
        let left_start = self.get_for_in_of_left_start(&stmt.left);
        let left_end = self.get_for_in_of_left_end(&stmt.left);
        let right_start = stmt.right.span().start;
        let right_end = stmt.right.span().end;
        let close_paren = self.find_close_paren_after(right_end);

        // Find 'of' keyword position (search with or without spaces)
        let of_pos = self
            .find_keyword_position(left_end, right_start, "of")
            .unwrap_or(left_end);

        // Check for line comments in the header - if present, use breaking layout
        // We check from open paren to close paren
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close = close_paren.unwrap_or(right_end + 1);
        let has_line_comments = if let Some(open) = open_paren {
            self.has_line_comments_in_range(open + 1, close)
        } else {
            self.has_line_comments_in_range(left_start, close)
        };

        if has_line_comments {
            return self.build_for_in_of_with_line_comments(
                &stmt.left,
                &stmt.right,
                &stmt.body,
                stmt.span.start,
                "of",
                of_pos,
                close_paren,
            );
        }

        let mut parts = vec![doc::text("for ")];
        if stmt.r#await {
            parts.push(doc::text("await "));
        }
        parts.push(doc::text("("));
        parts.push(self.build_for_in_of_left_doc(&stmt.left));

        // Comments after left, before 'of'
        let has_left_comment =
            self.append_for_in_of_before_keyword_comments(&mut parts, left_end, of_pos);

        if has_left_comment {
            parts.push(doc::text("of"));
        } else {
            parts.push(doc::text(" of"));
        }

        // Comments after 'of', before right
        let of_keyword_end = of_pos + 2; // "of" is 2 chars
        let has_comment =
            self.append_for_in_of_inline_comments(&mut parts, of_keyword_end, right_start);
        if !has_comment {
            parts.push(doc::text(" "));
        }

        parts.push(self.build_expression_doc(&stmt.right));

        // Comments after right, before close paren (no trailing space needed)
        if let Some(close) = close_paren {
            self.append_for_in_of_trailing_comments(&mut parts, right_end, close);
        }

        parts.push(doc::text(") "));

        // Prettier expands empty blocks for for-of
        if let Statement::BlockStatement(block) = stmt.body.as_ref() {
            parts.push(self.build_block_statement_expand_empty_doc(block));
        } else {
            parts.push(self.build_statement_doc(&stmt.body));
        }

        doc::concat(parts)
    }

    /// Build for-in/for-of statement with line comments preserved in their positions
    ///
    /// This is our divergence from Prettier - we preserve line comments where
    /// the user wrote them rather than relocating them.
    #[allow(clippy::too_many_arguments)]
    fn build_for_in_of_with_line_comments(
        &self,
        left: &internal::ForInOfLeft,
        right: &internal::Expression,
        body: &Statement,
        stmt_start: u32,
        keyword: &str, // "in" or "of"
        keyword_pos: u32,
        close_paren: Option<u32>,
    ) -> doc::Doc {
        let left_start = self.get_for_in_of_left_start(left);
        let left_end = self.get_for_in_of_left_end(left);
        let right_start = right.span().start;
        let right_end = right.span().end;
        let open_paren = self.find_open_paren_after(stmt_start);
        let keyword_end = keyword_pos + keyword.len() as u32;

        // Check for "for await" by looking at source before open paren
        let has_await = if let Some(open) = open_paren {
            let before_paren = &self.source[stmt_start as usize..open as usize];
            before_paren.contains("await")
        } else {
            false
        };

        let mut parts = if has_await {
            vec![doc::text("for await (")]
        } else {
            vec![doc::text("for (")]
        };

        // Inner content with hardline breaks
        let mut inner = Vec::new();

        // Comments before left (after open paren)
        if let Some(open) = open_paren {
            for comment in tsv_lang::comments_in_range(self.comments, open + 1, left_start) {
                inner.push(doc::hardline());
                inner.push(self.build_comment_doc(comment));
            }
        }

        // Left side (const y)
        inner.push(doc::hardline());
        inner.push(self.build_for_in_of_left_doc(left));

        // Inline comment after left
        for comment in tsv_lang::comments_in_range(self.comments, left_end, keyword_pos) {
            if self.is_same_line(left_end, comment.span.start) {
                inner.push(doc::text(" "));
                inner.push(self.build_comment_doc(comment));
            }
        }

        // Keyword with extra indent (hardline is INSIDE the indent so keyword gets extra indent)
        let keyword_doc = match keyword {
            "in" => doc::text("in"),
            "of" => doc::text("of"),
            _ => doc::text("of"), // fallback
        };
        let mut keyword_parts = vec![doc::hardline(), keyword_doc];

        // Inline comment after keyword
        for comment in tsv_lang::comments_in_range(self.comments, keyword_end, right_start) {
            if self.is_same_line(keyword_end, comment.span.start) {
                keyword_parts.push(doc::text(" "));
                keyword_parts.push(self.build_comment_doc(comment));
            }
        }

        // Right side (items)
        keyword_parts.push(doc::hardline());
        keyword_parts.push(self.build_expression_doc(right));

        // Inline comment after right
        if let Some(close) = close_paren {
            for comment in tsv_lang::comments_in_range(self.comments, right_end, close) {
                if self.is_same_line(right_end, comment.span.start) {
                    keyword_parts.push(doc::text(" "));
                    keyword_parts.push(self.build_comment_doc(comment));
                }
            }
        }

        inner.push(doc::indent(doc::concat(keyword_parts)));

        parts.push(doc::indent(doc::concat(inner)));
        parts.push(doc::hardline());
        parts.push(doc::text(") "));

        // Body
        if let Statement::BlockStatement(block) = body {
            parts.push(self.build_block_statement_expand_empty_doc(block));
        } else {
            parts.push(self.build_statement_doc(body));
        }

        doc::concat(parts)
    }

    /// Get the end position of the left side of a for-in/for-of statement
    fn get_for_in_of_left_end(&self, left: &internal::ForInOfLeft) -> u32 {
        match left {
            internal::ForInOfLeft::VariableDeclaration(decl) => decl.span.end,
            internal::ForInOfLeft::Pattern(expr) => expr.span().end,
        }
    }

    /// Get the start position of the left side of a for-in/for-of statement
    fn get_for_in_of_left_start(&self, left: &internal::ForInOfLeft) -> u32 {
        match left {
            internal::ForInOfLeft::VariableDeclaration(decl) => decl.span.start,
            internal::ForInOfLeft::Pattern(expr) => expr.span().start,
        }
    }

    /// Check if there are any line comments in the given range
    fn has_line_comments_in_range(&self, start: u32, end: u32) -> bool {
        tsv_lang::comments_in_range(self.comments, start, end).any(|c| !c.is_block)
    }

    /// Append block comments before a keyword (like 'in' or 'of')
    /// Returns true if any comments were added
    /// Adds space-comment-space pattern so keyword doesn't need leading space
    fn append_for_in_of_before_keyword_comments(
        &self,
        parts: &mut Vec<doc::Doc>,
        start: u32,
        end: u32,
    ) -> bool {
        let mut added = false;
        for comment in tsv_lang::comments_in_range(self.comments, start, end) {
            // Only include block comments that are on the same line
            if comment.is_block && self.is_same_line(start, comment.span.start) {
                parts.push(doc::text(" "));
                parts.push(self.build_comment_doc(comment));
                parts.push(doc::text(" "));
                added = true;
            }
        }
        added
    }

    /// Append inline block comments for for-in/for-of statements
    ///
    /// Returns true if any comments were added.
    /// Adds leading space before each comment and trailing space after all comments.
    fn append_for_in_of_inline_comments(
        &self,
        parts: &mut Vec<doc::Doc>,
        start: u32,
        end: u32,
    ) -> bool {
        let mut added = false;
        for comment in tsv_lang::comments_in_range(self.comments, start, end) {
            // Only include block comments (line comments would break the statement)
            if comment.is_block {
                parts.push(doc::text(" "));
                parts.push(self.build_comment_doc(comment));
                added = true;
            }
        }
        // Add trailing space after comments if we added any
        if added {
            parts.push(doc::text(" "));
        }
        added
    }

    /// Append trailing block comments (same line) for for-in/for-of statements
    /// No trailing space (use for comments before close paren)
    fn append_for_in_of_trailing_comments(&self, parts: &mut Vec<doc::Doc>, start: u32, end: u32) {
        for comment in tsv_lang::comments_in_range(self.comments, start, end) {
            // Only include block comments that are on the same line
            if comment.is_block && self.is_same_line(start, comment.span.start) {
                parts.push(doc::text(" "));
                parts.push(self.build_comment_doc(comment));
            }
        }
    }

    /// Build a doc for a while statement with proper line-width wrapping
    ///
    /// Matches Prettier's architecture: the condition wraps to multiple lines
    /// when the `while (condition)` line exceeds print width.
    fn build_while_statement_with_wrapping_doc(&self, stmt: &internal::WhileStatement) -> doc::Doc {
        // Find paren positions for comment handling
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren = self.find_close_paren_after(stmt.test.span().end);

        // Build condition group (handles breaking within condition and comments)
        let condition_group = if let (Some(open), Some(close)) = (open_paren, close_paren) {
            self.build_condition_group_with_comments(&stmt.test, open, close)
        } else {
            self.build_condition_group(&stmt.test)
        };

        let is_block = matches!(stmt.body.as_ref(), Statement::BlockStatement(_));
        let is_empty = matches!(stmt.body.as_ref(), Statement::EmptyStatement(_));

        if is_block {
            // Block body: while (cond) { ... }
            let mut parts = vec![doc::text("while ("), condition_group, doc::text(")")];

            if let Statement::BlockStatement(block) = stmt.body.as_ref() {
                // Check for comments between ) and {
                let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end);
                let block_start = block.span.start;

                if self.has_comments_between(paren_end, block_start) {
                    let (inline_prev, own_line, _) =
                        self.partition_comments_by_line(paren_end, block_start);

                    // Add comments preserving their position
                    self.build_comments_between_parts(
                        &mut parts,
                        &inline_prev,
                        &own_line,
                        paren_end,
                    );

                    parts.push(doc::hardline());
                    parts.push(self.build_block_statement_doc(block));
                } else {
                    parts.push(doc::text(" "));
                    parts.push(self.build_block_statement_doc(block));
                }
            }
            doc::group(doc::concat(parts))
        } else if is_empty {
            // Empty statement: while (cond);
            doc::group(doc::concat(vec![
                doc::text("while ("),
                condition_group,
                doc::text(");"),
            ]))
        } else {
            // Non-block body: use adjustClause equivalent
            // - When flat: line becomes space -> `while (cond) a;`
            // - When broken: line becomes newline + indent -> `while (cond)\n\ta;`
            let body_doc = self.build_statement_doc(&stmt.body);
            let adjust_clause = doc::indent(doc::concat(vec![doc::line(), body_doc]));

            doc::group(doc::concat(vec![
                doc::text("while ("),
                condition_group,
                doc::text(")"),
                adjust_clause,
            ]))
        }
    }

    /// Build a doc for a switch statement with proper line-width wrapping
    ///
    /// Matches Prettier's architecture: the discriminant wraps to multiple lines
    /// when the `switch (discriminant) {` line exceeds print width.
    fn build_switch_statement_with_wrapping_doc(
        &self,
        stmt: &internal::SwitchStatement,
    ) -> doc::Doc {
        // Find paren positions for comment handling
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren = self.find_close_paren_after(stmt.discriminant.span().end);

        // Build condition group (handles breaking within discriminant and comments)
        let condition_group = if let (Some(open), Some(close)) = (open_paren, close_paren) {
            self.build_condition_group_with_comments(&stmt.discriminant, open, close)
        } else {
            self.build_condition_group(&stmt.discriminant)
        };

        // Build cases - they handle their own internal indentation
        // Join cases with hardlines, handling comments between cases
        let mut case_parts = Vec::new();
        // Start after closing paren to avoid picking up comments inside parens
        // (those are handled by build_condition_group_with_comments)
        let mut prev_end = close_paren.map_or_else(|| stmt.discriminant.span().end, |p| p + 1);
        let mut prev_case_label_end: Option<u32> = None;
        let mut is_first_item = true;
        for (i, case) in stmt.cases.iter().enumerate() {
            // Handle comments between previous position and this case
            // (includes comments before first case, and between subsequent cases)
            // Skip inline comments that belong to the previous case label (fallthrough cases)
            let comments: Vec<_> =
                tsv_lang::comments_in_range(self.comments, prev_end, case.span.start).collect();
            let mut last_content_end = prev_end;
            for comment in &comments {
                // Skip comments that are on the same line as the previous case label
                // Those are inline comments for the case (e.g., `case 3: // fallthrough`)
                if prev_case_label_end
                    .is_some_and(|label_end| self.is_same_line(label_end, comment.span.start))
                {
                    continue;
                }
                // Add hardline before comment (except for very first item - body_doc handles that)
                if !is_first_item {
                    case_parts.push(doc::hardline());
                }
                is_first_item = false;
                case_parts.push(self.build_comment_doc(comment));
                last_content_end = comment.span.end;
            }
            // Add hardline before case (except for very first item)
            // Preserve blank lines between cases (check from last content, not prev_end)
            if !is_first_item {
                // Check for blank line between last content (case or comment) and current case
                if self.has_blank_line_between(last_content_end, case.span.start) {
                    case_parts.push(doc::literalline());
                }
                case_parts.push(doc::hardline());
            }
            is_first_item = false;

            // Determine the end boundary for inline comments on this case
            // For empty cases (fallthrough), we need to look ahead to the next case
            let next_case_start = stmt.cases.get(i + 1).map(|c| c.span.start);
            let inline_comment_boundary = next_case_start.unwrap_or(stmt.span.end - 1);

            case_parts.push(self.build_switch_case_doc_inner(case, inline_comment_boundary));

            // Track case label end for filtering inline comments in next iteration
            prev_case_label_end = Some(self.get_case_label_end(case));
            prev_end = case.span.end;
        }

        // Handle trailing comments after the last case (before closing `}`)
        // Also handles comments in empty switch bodies
        let switch_end = stmt.span.end - 1; // Before '}'
        for comment in tsv_lang::comments_in_range(self.comments, prev_end, switch_end) {
            if !is_first_item {
                case_parts.push(doc::hardline());
            }
            is_first_item = false;
            case_parts.push(self.build_comment_doc(comment));
        }

        // Structure: switch (...) { indent([hardline, cases...]) hardline }
        // The indent wraps the hardline so cases start at +1 indent level
        // For empty switch, just output {\n}
        let body_doc = if case_parts.is_empty() {
            doc::hardline()
        } else {
            doc::concat(vec![
                doc::indent(doc::concat(vec![doc::hardline(), doc::concat(case_parts)])),
                doc::hardline(),
            ])
        };

        doc::group(doc::concat(vec![
            doc::text("switch ("),
            condition_group,
            doc::text(") {"),
            body_doc,
            doc::text("}"),
        ]))
    }

    /// Get the end position of a case label (position after the colon)
    fn get_case_label_end(&self, case: &internal::SwitchCase) -> u32 {
        if let Some(test) = &case.test {
            // Find ':' after the test expression
            let test_end = test.span().end;
            self.source[test_end as usize..]
                .find(':')
                .map_or_else(|| test_end + 1, |p| test_end + p as u32 + 1)
        } else {
            // "default:" - find the actual ':' position
            self.source[case.span.start as usize..]
                .find(':')
                .map_or(case.span.start + 8, |p| case.span.start + p as u32 + 1)
        }
    }

    /// Build a doc for a switch case (without outer indent - that's handled by switch)
    ///
    /// `inline_comment_boundary` is the position up to which we should look for inline comments
    /// on this case label (typically the next case start or switch body end).
    fn build_switch_case_doc_inner(
        &self,
        case: &internal::SwitchCase,
        inline_comment_boundary: u32,
    ) -> doc::Doc {
        let mut parts = Vec::new();

        // case X: or default:
        let case_label_end = self.get_case_label_end(case);

        if let Some(test) = &case.test {
            parts.push(doc::text("case "));
            parts.push(self.build_expression_doc(test));
            parts.push(doc::text(":"));
        } else {
            parts.push(doc::text("default:"));
        }

        // Handle inline comments after case label (e.g., `case 1: // comment`)
        // For fallthrough cases (no consequent), use the boundary passed by the switch printer
        let first_stmt_start = case.consequent.first().map(|s| s.span().start);
        let inline_comment_end = first_stmt_start.unwrap_or(inline_comment_boundary);
        let mut has_inline_line_comment = false;
        for comment in
            tsv_lang::comments_in_range(self.comments, case_label_end, inline_comment_end)
        {
            if self.is_same_line(case_label_end, comment.span.start) {
                parts.push(doc::text(" "));
                parts.push(self.build_comment_doc(comment));
                if !comment.is_block {
                    has_inline_line_comment = true;
                }
            }
        }

        // Consequent statements (indented from case line)
        // Handle comments between statements like block statements do
        let mut prev_end = case_label_end;
        let mut prev_stmt_end: Option<u32> = None;

        // Check if first statement is a block - it hugs the case label: `case 'a': { ... }`
        let first_is_block = case
            .consequent
            .first()
            .is_some_and(|s| matches!(s, Statement::BlockStatement(_)));

        for (i, stmt) in case.consequent.iter().enumerate() {
            let stmt_start = stmt.span().start;

            // Check for comments between previous position and this statement
            let comments: Vec<_> =
                tsv_lang::comments_in_range(self.comments, prev_end, stmt_start).collect();

            // Filter out:
            // 1. Trailing same-line comments from the previous statement
            // 2. Inline comments after the case label (already handled above)
            let leading_comments: Vec<_> = if let Some(prev_stmt) = prev_stmt_end {
                comments
                    .iter()
                    .filter(|c| !self.is_same_line(prev_stmt, c.span.start))
                    .copied()
                    .collect()
            } else {
                // For first statement, filter out inline comments after case label
                comments
                    .iter()
                    .filter(|c| !self.is_same_line(case_label_end, c.span.start))
                    .copied()
                    .collect()
            };

            // First block statement hugs the case label: `case 'a': { ... }`
            // Unless there are line comments (inline after label or between label and block)
            if i == 0 && first_is_block {
                let has_leading_line_comment = leading_comments.iter().any(|c| !c.is_block);
                if !has_inline_line_comment && !has_leading_line_comment {
                    // Hug: `case 'a': { ... }`
                    for comment in &leading_comments {
                        parts.push(doc::text(" "));
                        parts.push(self.build_comment_doc(comment));
                    }
                    parts.push(doc::text(" "));
                    parts.push(self.build_statement_doc(stmt));
                } else if has_inline_line_comment && leading_comments.is_empty() {
                    // Inline line comment, no leading: `case 'a': // comment\n{`
                    // Block at case level (no indent)
                    parts.push(doc::hardline());
                    parts.push(self.build_statement_doc(stmt));
                } else {
                    // Leading comments exist - indent both comments and block
                    // e.g., `case 'b':\n  // comment\n  {`
                    let mut stmt_parts = vec![doc::hardline()];
                    for comment in &leading_comments {
                        stmt_parts.push(self.build_comment_doc(comment));
                        stmt_parts.push(doc::hardline());
                    }
                    stmt_parts.push(self.build_statement_doc(stmt));
                    parts.push(doc::indent(doc::concat(stmt_parts)));
                }
            } else {
                // Build the indented content for this statement
                let mut stmt_parts = vec![doc::hardline()];

                // Print leading comments before this statement
                for comment in &leading_comments {
                    stmt_parts.push(self.build_comment_doc(comment));
                    if !comment.is_block {
                        // Line comment: add hardline after
                        stmt_parts.push(doc::hardline());
                    } else if !self.is_same_line(comment.span.end, stmt_start) {
                        // Block comment not on same line as statement - add hardline
                        stmt_parts.push(doc::hardline());
                    } else {
                        // Block comment on same line as statement - add space
                        stmt_parts.push(doc::text(" "));
                    }
                }

                stmt_parts.push(self.build_statement_doc(stmt));

                parts.push(doc::indent(doc::concat(stmt_parts)));
            }

            prev_end = stmt.span().end;
            prev_stmt_end = Some(stmt.span().end);
        }

        // Note: Trailing comments after the last statement are handled by the switch statement
        // printer since they fall outside the SwitchCase span.

        doc::concat(parts)
    }

    //
    // Control Flow Statement Doc Builders
    //

    pub(super) fn build_if_statement_doc(&self, stmt: &internal::IfStatement) -> doc::Doc {
        // Check for comments between consequent and alternate that need special handling
        let has_if_else_comments = stmt.alternate.as_ref().is_some_and(|alt| {
            let consequent_end = stmt.consequent.span().end;
            let alternate_start = alt.span().start;
            self.has_comments_between(consequent_end, alternate_start)
        });

        if has_if_else_comments {
            // Build doc with inline comments between } and else
            self.build_if_statement_with_comments_doc(stmt)
        } else {
            // Delegate to the sophisticated version that handles width-based wrapping
            self.build_if_statement_with_wrapping_doc(stmt)
        }
    }

    /// Build if statement doc with comments between consequent and alternate
    fn build_if_statement_with_comments_doc(&self, stmt: &internal::IfStatement) -> doc::Doc {
        let is_block = matches!(stmt.consequent.as_ref(), Statement::BlockStatement(_));

        let mut parts = vec![doc::text("if ("), self.build_expression_doc(&stmt.test)];

        // Build consequent
        if is_block {
            parts.push(doc::text(") "));
            if let Statement::BlockStatement(block) = stmt.consequent.as_ref() {
                parts.push(self.build_block_statement_expand_empty_doc(block));
            }
        } else if matches!(stmt.consequent.as_ref(), Statement::EmptyStatement(_)) {
            parts.push(doc::text(");"));
        } else if is_inline_consequent(&stmt.consequent) {
            parts.push(doc::text(") "));
            parts.push(self.build_statement_doc(&stmt.consequent));
        } else {
            parts.push(doc::text(")"));
            parts.push(doc::indent(doc::concat(vec![
                doc::hardline(),
                self.build_statement_doc(&stmt.consequent),
            ])));
        }

        // Handle else with comments
        if let Some(alternate) = &stmt.alternate {
            let consequent_end = stmt.consequent.span().end;
            let alternate_start = alternate.span().start;

            let (inline_prev, own_line, _) =
                self.partition_comments_by_line(consequent_end, alternate_start);

            // Add comments between consequent and else
            self.build_comments_between_parts(&mut parts, &inline_prev, &own_line, consequent_end);

            // Determine if else can stay on same line (block consequent only):
            // - No own-line comments AND all inline comments are block comments
            let has_inline_line_comment = inline_prev.iter().any(|c| !c.is_block);
            if is_block && own_line.is_empty() && !has_inline_line_comment {
                parts.push(doc::text(" else "));
            } else {
                parts.push(doc::hardline());
                parts.push(doc::text("else "));
            }

            if let Statement::BlockStatement(block) = alternate.as_ref() {
                parts.push(self.build_block_statement_expand_empty_doc(block));
            } else if is_inline_alternate(alternate) {
                parts.push(self.build_statement_doc(alternate));
            } else {
                parts.push(doc::indent(doc::concat(vec![
                    doc::hardline(),
                    self.build_statement_doc(alternate),
                ])));
            }
        }

        doc::concat(parts)
    }

    pub(super) fn build_for_statement_doc(&self, stmt: &internal::ForStatement) -> doc::Doc {
        // Check for comments between ) and body (Prettier 3.7 #18108)
        let header_end = self.get_for_header_end(stmt);
        let body_start = stmt.body.span().start;

        if self.has_comments_between(header_end, body_start) {
            // Check if we have line comments (need special handling)
            let has_line_comment = self.has_line_comments_between(header_end, body_start);

            // Build parts with proper comment handling
            let mut parts = Vec::new();

            // Force header to break only for line comments
            // Block comments can stay inline with for loop
            if has_line_comment {
                parts.push(self.build_for_header_doc_breaking(stmt));
            } else {
                parts.push(self.build_for_header_doc(stmt));
            }
            parts.push(self.build_inline_comments_between_doc(header_end, body_start));

            if has_line_comment {
                // Line comment: need hardline before body
                parts.push(doc::hardline());
            } else {
                // Block comment: space before body
                parts.push(doc::text(" "));
            }
            parts.push(self.build_statement_doc(&stmt.body));

            doc::concat(parts)
        } else {
            // Delegate to the sophisticated version that handles all edge cases
            self.build_for_statement_with_body_doc(stmt)
        }
    }

    fn build_for_init_doc(&self, init: &internal::ForInit) -> doc::Doc {
        match init {
            internal::ForInit::VariableDeclaration(decl) => {
                let mut parts = vec![doc::text(decl.kind.as_str()), doc::text(" ")];
                for (i, declarator) in decl.declarations.iter().enumerate() {
                    if i > 0 {
                        parts.push(doc::text(", "));
                    }
                    parts.push(self.build_expression_doc(&declarator.id));
                    if let Some(init) = &declarator.init {
                        parts.push(doc::text(" = "));
                        parts.push(self.build_expression_doc(init));
                    }
                }
                doc::concat(parts)
            }
            internal::ForInit::Expression(expr) => {
                // Sequence expressions in for loop init don't need outer parens
                // e.g., `for (i = 0, j = 0; ...)` not `for ((i = 0, j = 0); ...)`
                // Same handling as build_for_update_doc
                if let internal::Expression::SequenceExpression(seq) = expr {
                    doc::join(
                        seq.expressions.iter().map(|e| self.build_expression_doc(e)),
                        ", ",
                    )
                } else {
                    self.build_expression_doc(expr)
                }
            }
        }
    }

    pub(super) fn build_for_in_statement_doc(&self, stmt: &internal::ForInStatement) -> doc::Doc {
        // Delegate to the sophisticated version that handles empty block expansion
        self.build_for_in_statement_with_body_doc(stmt)
    }

    pub(super) fn build_for_of_statement_doc(&self, stmt: &internal::ForOfStatement) -> doc::Doc {
        // Delegate to the sophisticated version that handles empty block expansion
        self.build_for_of_statement_with_body_doc(stmt)
    }

    fn build_for_in_of_left_doc(&self, left: &internal::ForInOfLeft) -> doc::Doc {
        match left {
            internal::ForInOfLeft::VariableDeclaration(decl) => {
                let mut parts = vec![doc::text(decl.kind.as_str()), doc::text(" ")];
                if let Some(declarator) = decl.declarations.first() {
                    parts.push(self.build_expression_doc(&declarator.id));
                }
                doc::concat(parts)
            }
            internal::ForInOfLeft::Pattern(expr) => self.build_expression_doc(expr),
        }
    }

    pub(super) fn build_while_statement_doc(&self, stmt: &internal::WhileStatement) -> doc::Doc {
        // Delegate to the wrapping version for proper condition grouping
        self.build_while_statement_with_wrapping_doc(stmt)
    }

    pub(super) fn build_do_while_statement_doc(
        &self,
        stmt: &internal::DoWhileStatement,
    ) -> doc::Doc {
        let is_block = matches!(stmt.body.as_ref(), Statement::BlockStatement(_));
        let mut parts = vec![doc::text("do "), self.build_statement_doc(&stmt.body)];

        // Find the while keyword position for comment handling
        // Use test expression start and search backwards for "while" to avoid matching "while" in comments
        let body_end = stmt.body.span().end;
        let test_start = stmt.test.span().start;
        let while_pos = self.source[body_end as usize..test_start as usize]
            .rfind("while")
            .map(|p| body_end + p as u32);

        // Check for comments between } and while, determine if while stays on same line
        let while_on_same_line = if let Some(while_start) = while_pos
            && self.has_comments_between(body_end, while_start)
        {
            let (inline_prev, own_line, _) = self.partition_comments_by_line(body_end, while_start);

            // Add comments preserving their position
            self.build_comments_between_parts(&mut parts, &inline_prev, &own_line, body_end);

            // While stays on same line only if: block body, no own-line comments, all inline are block comments
            let has_inline_line_comment = inline_prev.iter().any(|c| !c.is_block);
            is_block && own_line.is_empty() && !has_inline_line_comment
        } else {
            is_block
        };

        if while_on_same_line {
            parts.push(doc::text(" while ("));
        } else {
            parts.push(doc::hardline());
            parts.push(doc::text("while ("));
        }

        // Find paren positions for comment handling
        let open_paren = while_pos.and_then(|p| self.find_open_paren_after(p));
        let close_paren = self.find_close_paren_after(stmt.test.span().end);

        // Check for comments in the condition and use preserve_inline if present
        // Use preserve_inline for do-while to intentionally differ from Prettier
        // Prettier moves comments after `while (` to outside the parens - we keep them in place
        if let (Some(open), Some(close)) = (open_paren, close_paren)
            && (self.has_comments_between(open + 1, stmt.test.span().start)
                || self.has_comments_between(stmt.test.span().end, close))
        {
            parts.push(self.build_condition_group_preserve_inline(&stmt.test, open, close));
        } else {
            parts.push(self.build_expression_doc(&stmt.test));
        }
        parts.push(doc::text(");"));
        doc::concat(parts)
    }

    #[inline]
    pub(super) fn build_switch_statement_doc(&self, stmt: &internal::SwitchStatement) -> doc::Doc {
        // Delegate to the wrapping version which handles proper indentation structure
        self.build_switch_statement_with_wrapping_doc(stmt)
    }

    pub(super) fn build_try_statement_doc(&self, stmt: &internal::TryStatement) -> doc::Doc {
        let mut parts = vec![
            doc::text("try "),
            // Try block expands empty: `try {\n}` not `try {}`
            self.build_block_statement_expand_empty_doc(&stmt.block),
        ];
        if let Some(handler) = &stmt.handler {
            // Check for comments between try block and catch keyword
            let try_end = stmt.block.span.end;
            // Use handler span start which is the position of "catch" keyword
            let catch_keyword_pos = handler.span.start;
            if self.has_comments_between(try_end, catch_keyword_pos) {
                let has_line_comment = self.has_line_comments_between(try_end, catch_keyword_pos);
                parts.push(self.build_inline_comments_between_doc(try_end, catch_keyword_pos));
                if has_line_comment {
                    parts.push(doc::hardline());
                } else {
                    parts.push(doc::text(" "));
                }
                parts.push(doc::text("catch"));
            } else {
                parts.push(doc::text(" catch"));
            }
            if let Some(param) = &handler.param {
                // Find paren positions for comment handling
                let catch_start = handler.body.span.start;
                let open_paren = self.source[stmt.block.span.end as usize..catch_start as usize]
                    .find('(')
                    .map(|p| stmt.block.span.end + p as u32);
                let close_paren = self.find_close_paren_after(param.span().end);

                // Check for comments in catch parameter
                parts.push(doc::text(" ("));
                if let (Some(open), Some(close)) = (open_paren, close_paren)
                    && (self.has_comments_between(open + 1, param.span().start)
                        || self.has_comments_between(param.span().end, close))
                {
                    parts.push(self.build_condition_group_with_comments(param, open, close));
                } else {
                    parts.push(self.build_expression_doc(param));
                }
                parts.push(doc::text(")"));
            }
            parts.push(doc::text(" "));
            // Catch block stays inline: `catch (e) {}`
            parts.push(self.build_block_statement_doc(&handler.body));
        }
        if let Some(finalizer) = &stmt.finalizer {
            // Check for comments before finally (after catch block or try block)
            let prev_end = stmt
                .handler
                .as_ref()
                .map_or(stmt.block.span.end, |h| h.body.span.end);
            // The finalizer span starts at the "finally" keyword
            // Note: finalizer is a BlockStatement, we need to find the keyword position
            // Search for "finally" in source (but avoid matching inside comments)
            // For safety, search backwards from finalizer start for "finally"
            let search_range = &self.source[prev_end as usize..finalizer.span.start as usize];
            let finally_keyword_pos = search_range
                .rfind("finally")
                .map_or(finalizer.span.start, |p| prev_end + p as u32);
            if self.has_comments_between(prev_end, finally_keyword_pos) {
                let has_line_comment =
                    self.has_line_comments_between(prev_end, finally_keyword_pos);
                parts.push(self.build_inline_comments_between_doc(prev_end, finally_keyword_pos));
                if has_line_comment {
                    parts.push(doc::hardline());
                } else {
                    parts.push(doc::text(" "));
                }
                parts.push(doc::text("finally "));
            } else {
                parts.push(doc::text(" finally "));
            }
            // Finally block expands empty: `finally {\n}` not `finally {}`
            parts.push(self.build_block_statement_expand_empty_doc(finalizer));
        }
        doc::concat(parts)
    }

    pub(super) fn build_throw_statement_doc(&self, stmt: &internal::ThrowStatement) -> doc::Doc {
        doc::concat(vec![
            doc::text("throw "),
            self.build_expression_doc(&stmt.argument),
            doc::text(";"),
        ])
    }

    pub(super) fn build_break_statement_doc(&self, stmt: &internal::BreakStatement) -> doc::Doc {
        if let Some(label) = &stmt.label {
            doc::concat(vec![
                doc::text("break "),
                doc::symbol(label.name.to_u32()),
                doc::text(";"),
            ])
        } else {
            doc::text("break;")
        }
    }

    pub(super) fn build_continue_statement_doc(
        &self,
        stmt: &internal::ContinueStatement,
    ) -> doc::Doc {
        if let Some(label) = &stmt.label {
            doc::concat(vec![
                doc::text("continue "),
                doc::symbol(label.name.to_u32()),
                doc::text(";"),
            ])
        } else {
            doc::text("continue;")
        }
    }

    pub(super) fn build_labeled_statement_doc(
        &self,
        stmt: &internal::LabeledStatement,
    ) -> doc::Doc {
        // Check for comments between label and body
        // The colon is after the label identifier
        let colon_end = stmt.label.span.end + 1; // Position after ":"
        let body_start = stmt.body.span().start;

        if self.has_comments_between(colon_end, body_start) {
            let has_line_comment = self.has_line_comments_between(colon_end, body_start);
            let mut parts = vec![
                doc::symbol(stmt.label.name.to_u32()),
                doc::text(":"),
                self.build_inline_comments_between_doc(colon_end, body_start),
            ];
            if has_line_comment {
                parts.push(doc::hardline());
            } else {
                parts.push(doc::text(" "));
            }
            parts.push(self.build_statement_doc(&stmt.body));
            doc::concat(parts)
        } else {
            // No space before empty statement: `label:;` not `label: ;`
            let separator = if matches!(stmt.body.as_ref(), Statement::EmptyStatement(_)) {
                ":"
            } else {
                ": "
            };
            doc::concat(vec![
                doc::symbol(stmt.label.name.to_u32()),
                doc::text(separator),
                self.build_statement_doc(&stmt.body),
            ])
        }
    }

    /// Get the end position of a ForInit
    fn get_for_init_span_end(&self, init: &internal::ForInit) -> u32 {
        match init {
            internal::ForInit::VariableDeclaration(decl) => decl.span.end,
            internal::ForInit::Expression(expr) => expr.span().end,
        }
    }
}
