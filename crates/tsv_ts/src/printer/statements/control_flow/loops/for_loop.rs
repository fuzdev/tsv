// Loop statement printing: for, for-in, for-of
//
// for-loop header layout (init/test/update clauses with comment placement),
// for-in/for-of left/right printing.

use crate::ast::internal::{self, Expression, Statement};
use crate::printer::Printer;
use tsv_lang::doc::arena::DocId;

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

impl<'a> Printer<'a> {
    /// Append `)` + comments + non-block body for for-in/for-of statements.
    ///
    /// Unlike `append_close_paren_with_comments` (which handles block bodies where
    /// indentation isn't needed), this properly indents non-block bodies when line
    /// comments force a break. Also avoids placing block comments after line comments
    /// on the same line (which would absorb them into the line comment text).
    fn append_close_paren_with_non_block_body(
        &self,
        parts: &mut Vec<DocId>,
        paren_end: u32,
        body: &Statement,
    ) {
        let d = self.d();
        let body_start = body.span().start;
        let body_doc = self.build_statement_doc(body);

        if !self.has_comments_between(paren_end, body_start) {
            parts.push(d.text(")"));
            if matches!(body, Statement::EmptyStatement(_)) {
                // Prettier's `adjustClause` returns `";"` directly for an empty
                // body (no leading `line`) → `for (x of y);`, not `for (x of y) ;`.
                parts.push(body_doc);
            } else {
                // Mirror Prettier's `adjustClause`: `indent([line, body])`. The
                // enclosing for-in/for-of group (see `build_for_in/of_statement_with_body_doc`)
                // breaks on overflow, dropping the body to its own indented line;
                // when it fits, `line` is a space → `for (x of y) stmt;`.
                parts.push(d.indent_line(body_doc));
            }
            return;
        }

        let (inline_prev, own_line, inline_next) =
            self.partition_comments_by_line(paren_end, body_start);

        // Check if any line comment forces a break
        let has_line =
            inline_prev.iter().any(|c| !c.is_block) || own_line.iter().any(|c| !c.is_block);

        parts.push(d.text(")"));

        if has_line {
            // Emit trailing comments on the `)` line
            for comment in &inline_prev {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }

            // Remaining comments (own_line + inline_next) go indented before body
            let mut inner = vec![d.hardline()];
            for comment in own_line.into_iter().chain(inline_next) {
                inner.push(self.build_comment_doc(comment));
                if comment.is_block {
                    inner.push(d.text(" "));
                } else {
                    inner.push(d.hardline());
                }
            }
            inner.push(body_doc);
            parts.push(d.indent(d.concat(&inner)));
        } else {
            // Block comments only: adjustClause — `) /* a */ body` stays flat but the
            // comment(s) + body drop to their own indented line when the enclosing
            // for-in/for-of group breaks (overflow). Matches Prettier.
            let mut inner = Vec::new();
            for comment in inline_prev
                .iter()
                .chain(own_line.iter())
                .chain(inline_next.iter())
            {
                inner.push(self.build_comment_doc(comment));
                inner.push(d.text(" "));
            }
            inner.push(body_doc);
            parts.push(d.indent_line(d.concat(&inner)));
        }
    }

    /// Build a complete for statement doc including the body
    ///
    /// This includes the body in the doc so the width calculation accounts for ` {`.
    fn build_for_statement_with_body_doc(&self, stmt: &internal::ForStatement) -> DocId {
        let d = self.d();
        let header_doc = self.build_for_header_doc(stmt);
        if matches!(stmt.body.as_ref(), Statement::EmptyStatement(_)) {
            // No space before empty statement: `for (...);`
            d.concat(&[header_doc, self.build_statement_doc(&stmt.body)])
        } else if let Statement::BlockStatement(block) = stmt.body.as_ref() {
            // Block body: `for (...) { ... }`
            // Note: Unlike for-in/for-of, standard for loops keep empty blocks inline `{}`
            d.concat(&[
                header_doc,
                d.text(" "),
                self.build_block_statement_doc(block),
            ])
        } else {
            // Non-block body. Mirror Prettier's `adjustClause`: the body is
            // `indent([line, body])` wrapped with the header in an outer group.
            // Flat → `for (...) stmt;`. When the header force-breaks (a comment
            // hardline propagates via `will_break`) or the whole thing overflows,
            // the outer group breaks and the body drops to its own indented line;
            // the inner header group still decides its own flat/break, so a
            // width-only overflow keeps the header flat (matching Prettier).
            let body_doc = self.build_statement_doc(&stmt.body);
            d.group(d.concat(&[header_doc, d.indent_line(body_doc)]))
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

        // Find the for header's closing paren via its open paren (depth-tracked, so
        // redundant parens or parens inside a clause don't yield a premature match).
        let search_start = last_expr_end.unwrap_or(stmt.span.start + "for ".len() as u32);
        self.find_open_paren_after(stmt.span.start)
            .and_then(|open| self.matching_close_paren(open))
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
    fn build_for_header_doc(&self, stmt: &internal::ForStatement) -> DocId {
        self.build_for_header_doc_impl(stmt, false, None)
    }

    /// Build doc for empty for (;;) with comments inside
    ///
    /// Preserves comments in their original positions (divergence from prettier).
    /// Format: for (\n\t; // comment\n\t; // comment\n\t// comment\n)
    fn build_for_empty_with_comments(&self, stmt: &internal::ForStatement) -> DocId {
        let d = self.d();
        let Some(open_paren) = self.find_open_paren_after(stmt.span.start) else {
            return d.text("for (;;)");
        };
        let Some(close_paren) = self.matching_close_paren(open_paren) else {
            return d.text("for (;;)");
        };

        // Find the two semicolons
        let (first_semi, second_semi) = self.find_for_semicolons(open_paren);

        let mut parts = vec![d.text("for (")];
        let mut inner_parts = Vec::new();

        // First semicolon line: ; // inline comment
        inner_parts.push(d.hardline());
        inner_parts.push(d.text(";"));
        if let (Some(semi1), Some(semi2)) = (first_semi, second_semi) {
            for comment in tsv_lang::comments_in_range(self.comments, semi1 + 1, semi2) {
                if self.is_same_line(semi1, comment.span.start) {
                    inner_parts.push(d.text(" "));
                    inner_parts.push(self.build_comment_doc(comment));
                }
            }
        }

        // Second semicolon line: ; // inline comment
        inner_parts.push(d.hardline());
        inner_parts.push(d.text(";"));

        // Comments after second semicolon: inline first, then own-line
        if let Some(semi2) = second_semi {
            let mut own_line_comments = Vec::new();
            for comment in tsv_lang::comments_in_range(self.comments, semi2 + 1, close_paren) {
                if self.is_same_line(semi2, comment.span.start) {
                    inner_parts.push(d.text(" "));
                    inner_parts.push(self.build_comment_doc(comment));
                } else {
                    own_line_comments.push(comment);
                }
            }
            for comment in own_line_comments {
                inner_parts.push(d.hardline());
                inner_parts.push(self.build_comment_doc(comment));
            }
        }

        parts.push(d.indent(d.concat(&inner_parts)));
        parts.push(d.hardline());
        parts.push(d.text(")"));

        d.concat(&parts)
    }

    fn build_for_header_doc_impl(
        &self,
        stmt: &internal::ForStatement,
        force_break: bool,
        keyword_comments: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        let has_init = stmt.init.is_some();
        let has_test = stmt.test.is_some();
        let has_update = stmt.update.is_some();
        let has_any = has_init || has_test || has_update;

        // Build "for" + optional keyword comments + " (" prefix
        let for_open = if let Some(kc) = keyword_comments {
            d.concat(&[d.text("for"), kc, d.text(" (")])
        } else {
            d.text("for (")
        };

        // Check if there are any comments inside the for parens
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren_approx = open_paren.and_then(|p| self.matching_close_paren(p));
        let has_comments_inside =
            if let (Some(open), Some(close)) = (open_paren, close_paren_approx) {
                self.has_comments_between(open, close)
            } else {
                false
            };

        if !has_any && !has_comments_inside {
            // Empty for (;;) with no comments - no wrapping needed
            return d.concat(&[for_open, d.text(";;)")]);
        }

        if !has_any && has_comments_inside {
            // Empty for (;;) with comments - need to preserve them
            // This is a divergence from prettier (see for_empty_clauses_prettier_divergence)
            return self.build_for_empty_with_comments(stmt);
        }

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
            close_paren: open_paren.and_then(|o| self.matching_close_paren(o)),
        };

        // Check if we have any own-line comments that force expansion. A line
        // comment anywhere in the header also forces it: the `//` runs to end of
        // line, so the clauses after it must move to their own lines (matching
        // prettier) — otherwise the comment swallows the rest of the header.
        let has_line_comment_in_header =
            if let (Some(open), Some(close)) = (open_paren, spans.close_paren) {
                self.has_line_comments_between(open + 1, close)
            } else {
                false
            };
        let has_own_line_comments = force_break
            || has_line_comment_in_header
            || self.for_header_has_own_line_comments(&spans);

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
                    inner_parts.push(d.text(" "));
                }
            }
        }

        // Find semicolon positions for proper comment boundary detection
        // The semicolons in `for (init; test; update)` are at specific positions in source
        let (first_semi, second_semi) = self.find_for_semicolons(stmt.span.start);

        // Init part
        if let Some(init) = &stmt.init {
            if inner_parts.is_empty() {
                inner_parts.push(d.softline());
            }
            inner_parts.push(self.build_for_init_doc(init));
        }
        // Block comment trailing the init clause stays before its `;` (`a /* c */;`);
        // a line comment is relocated to after the `;` (`a; // c`).
        self.push_for_clause_trailing_comments(&mut inner_parts, init_end, first_semi, true);
        inner_parts.push(d.text(";"));
        self.push_for_clause_trailing_comments(&mut inner_parts, init_end, first_semi, false);

        // Inline comments after init (between semicolon and test, on same line as init)
        if let (Some(semi), Some(end)) = (first_semi, init_end) {
            let boundary = test_start
                .or(update_start)
                .or(close_paren)
                .unwrap_or(stmt.span.end);
            self.push_for_clause_same_line_comments(&mut inner_parts, semi + 1, boundary, end);
        }

        // Leading comments before test (own line, between first semi and test)
        if let Some(start) = test_start {
            let search_start =
                self.for_clause_search_start(stmt.span.start, open_paren, first_semi, init_end);
            self.push_for_clause_leading_section(
                &mut inner_parts,
                search_start,
                start,
                init_end,
                has_init,
            );
        } else if has_update {
            inner_parts.push(d.line());
        }

        // Test part
        if let Some(test) = &stmt.test {
            if !has_init && inner_parts.len() == 1 {
                // Only ";" so far, add line (becomes space in flat mode, newline when breaking)
                inner_parts.push(d.line());
            }
            // Wrap in group so binary chains (Ungrouped mode) have a tight parent
            // to evaluate fit against — matching how if/while use build_condition_group.
            // Without this, logical operators break with the for-header group (too wide)
            // instead of their own condition width.
            let condition_doc = self.build_condition_doc(test);
            inner_parts.push(d.group(condition_doc));
        }
        // Block comment trailing the test clause stays before its `;`; a line comment
        // is relocated to after it.
        self.push_for_clause_trailing_comments(&mut inner_parts, test_end, second_semi, true);
        inner_parts.push(d.text(";"));
        self.push_for_clause_trailing_comments(&mut inner_parts, test_end, second_semi, false);

        // Inline comments after test (between second semicolon and update, on same line as test)
        if let (Some(semi), Some(end)) = (second_semi, test_end) {
            let boundary = update_start.or(close_paren).unwrap_or(stmt.span.end);
            self.push_for_clause_same_line_comments(&mut inner_parts, semi + 1, boundary, end);
        }

        // Leading comments before update (own line, between second semi and update)
        if let Some(start) = update_start {
            let search_start = self.for_clause_search_start(
                stmt.span.start,
                open_paren,
                second_semi,
                test_end.or(init_end),
            );
            self.push_for_clause_leading_section(
                &mut inner_parts,
                search_start,
                start,
                test_end,
                true,
            );
        }

        // Update part
        if let Some(update) = &stmt.update {
            if !has_init && !has_test && inner_parts.len() == 2 {
                // Only ";;" so far, add line (becomes space in flat mode)
                inner_parts.push(d.line());
            }
            inner_parts.push(self.build_for_update_doc(update));
            // Inline comments after update (on same line as update expression)
            if let Some(end) = update_end {
                let boundary = close_paren.unwrap_or(stmt.span.end);
                self.push_for_clause_same_line_comments(&mut inner_parts, end, boundary, end);
            }
        } else if has_test && !has_own_line_comments {
            // Prettier adds trailing space when update is None but test exists (no comments)
            inner_parts.push(d.if_break(d.empty(), d.text(" ")));
        }

        let closing = if has_own_line_comments {
            d.hardline()
        } else {
            d.softline()
        };

        d.group(d.concat(&[
            for_open,
            d.indent(d.concat(&inner_parts)),
            closing,
            d.text(")"),
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
    ) -> Vec<DocId> {
        let d = self.d();
        let mut parts = Vec::new();
        for comment in tsv_lang::comments_in_range(self.comments, search_start, clause_start) {
            // Only include comments that are:
            // 1. NOT on the same line as the next clause
            // 2. NOT on the same line as the previous expression (inline comments)
            let is_own_line_before_clause = !self.is_same_line(comment.span.end, clause_start);
            let is_own_line_after_prev =
                prev_expr_end.is_none_or(|end| !self.is_same_line(end, comment.span.start));
            if is_own_line_before_clause && is_own_line_after_prev {
                parts.push(d.hardline());
                parts.push(self.build_comment_doc(comment));
            }
        }
        if !parts.is_empty() {
            parts.push(d.hardline());
        }
        parts
    }

    /// Build leading comments for a for clause (comments on their own line before the clause)
    fn build_for_clause_leading_comments(&self, start: u32, clause_start: u32) -> Vec<DocId> {
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

    /// Emit comments in `start..end` matching `want_block`, each inline with a
    /// leading space. No-op unless both bounds are known.
    ///
    /// Used for the gap between a for-clause expression and its `;`: block comments
    /// stay before the `;` (`for (a /* c */; ...)`), line comments are relocated to
    /// after it (`a; // c`) — so the caller picks the kind and the insertion point.
    fn push_for_clause_trailing_comments(
        &self,
        parts: &mut Vec<DocId>,
        start: Option<u32>,
        end: Option<u32>,
        want_block: bool,
    ) {
        let (Some(start), Some(end)) = (start, end) else {
            return;
        };
        let d = self.d();
        for comment in tsv_lang::comments_in_range(self.comments, start, end) {
            if comment.is_block == want_block {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }
        }
    }

    /// Find the two `;` separators in a for-header, scanning forward from
    /// `scan_start`. Returns `(first_semi, second_semi)`; the second is only sought
    /// once the first is found.
    fn find_for_semicolons(&self, scan_start: u32) -> (Option<u32>, Option<u32>) {
        let first_semi = self.source[scan_start as usize..]
            .find(';')
            .map(|p| scan_start + p as u32);
        let second_semi = first_semi.and_then(|p| {
            self.source[(p + 1) as usize..]
                .find(';')
                .map(|off| p + 1 + off as u32)
        });
        (first_semi, second_semi)
    }

    /// Resolve where to start searching for a for-clause's leading comments: just
    /// past the preceding `;` if present, else the previous clause's end, else just
    /// inside the open paren (or past `for (` when the paren is unknown).
    fn for_clause_search_start(
        &self,
        stmt_start: u32,
        open_paren: Option<u32>,
        semi: Option<u32>,
        prev_end: Option<u32>,
    ) -> u32 {
        semi.map_or_else(
            || {
                prev_end.unwrap_or_else(|| {
                    open_paren.map_or_else(|| stmt_start + "for (".len() as u32, |p| p + 1)
                })
            },
            |s| s + 1,
        )
    }

    /// Push comments in `range_start..boundary` that sit on the same source line as
    /// `end`, each inline with a leading space. Used for the inline comments
    /// trailing a for-clause: after init's `;`, after test's `;`, and after the
    /// update expression. Unlike `push_for_clause_trailing_comments` (the
    /// `;`-adjacent block/line split), this emits every comment kind that shares a
    /// line with the clause end.
    fn push_for_clause_same_line_comments(
        &self,
        parts: &mut Vec<DocId>,
        range_start: u32,
        boundary: u32,
        end: u32,
    ) {
        let d = self.d();
        for comment in tsv_lang::comments_in_range(self.comments, range_start, boundary) {
            if self.is_same_line(end, comment.span.start) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }
        }
    }

    /// Emit the lead-in before a for-clause: own-line leading comments (or a `line`
    /// when there are none and `push_line_when_empty`), then any inline block
    /// comment on the same line just before the clause.
    fn push_for_clause_leading_section(
        &self,
        parts: &mut Vec<DocId>,
        search_start: u32,
        clause_start: u32,
        prev_end: Option<u32>,
        push_line_when_empty: bool,
    ) {
        let d = self.d();
        let leading =
            self.build_for_clause_leading_comments_with_prev(search_start, clause_start, prev_end);
        if !leading.is_empty() {
            parts.extend(leading);
        } else if push_line_when_empty {
            parts.push(d.line());
        }

        // Inline block comments on the same line just before the clause
        // e.g., `for (let i = 0; /* before test */ i < 10; ...)`
        for comment in tsv_lang::comments_in_range(self.comments, search_start, clause_start) {
            if comment.is_block
                && self.is_same_line(comment.span.end, clause_start)
                && prev_end.is_none_or(|pe| !self.is_same_line(pe, comment.span.start))
            {
                parts.push(self.build_comment_doc(comment));
                parts.push(d.text(" "));
            }
        }
    }

    /// Build a Doc for a for loop update expression
    fn build_for_update_doc(&self, expr: &Expression) -> DocId {
        let d = self.d();
        if let Expression::SequenceExpression(seq) = expr {
            d.join(
                seq.expressions.iter().map(|e| self.build_expression_doc(e)),
                ", ",
            )
        } else {
            self.build_expression_doc(expr)
        }
    }

    /// Build a complete for-in statement doc including the body
    fn build_for_in_statement_with_body_doc(&self, stmt: &internal::ForInStatement) -> DocId {
        self.build_for_in_of_statement_with_body_doc(
            &stmt.left,
            &stmt.right,
            &stmt.body,
            stmt.span.start,
            "in",
            false,
        )
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
        let len = bytes.len();
        let kw_len = keyword.len();
        let mut i = 0;

        while i + kw_len <= len {
            // Skip over comments
            if let Some(new_i) = tsv_lang::source_scan::skip_comment(bytes, i, len) {
                i = new_i;
                continue;
            }

            // Check if we found the keyword
            if &bytes[i..i + kw_len] == keyword_bytes {
                // Check it's not part of an identifier
                let before_ok =
                    i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
                let after_ok = i + kw_len >= len
                    || !bytes[i + kw_len].is_ascii_alphanumeric() && bytes[i + kw_len] != b'_';

                if before_ok && after_ok {
                    return Some(start + i as u32);
                }
            }
            i += 1;
        }

        None
    }

    /// Build a complete for-of statement doc including the body
    fn build_for_of_statement_with_body_doc(&self, stmt: &internal::ForOfStatement) -> DocId {
        self.build_for_in_of_statement_with_body_doc(
            &stmt.left,
            &stmt.right,
            &stmt.body,
            stmt.span.start,
            "of",
            stmt.r#await,
        )
    }

    /// Build a complete for-in/for-of statement doc including the body.
    ///
    /// Shared by `build_for_in_statement_with_body_doc` and
    /// `build_for_of_statement_with_body_doc`: the two differ only in the
    /// `"in"`/`"of"` keyword and for-of's `for await` handling, which collapses
    /// to a no-op when `is_await` is false (for-in has no `await` form). The
    /// `for (` opening is built in split form (`" "` + `"("`) so the optional
    /// `await` keyword slots in between — render-identical to for-in's fused
    /// `" ("`.
    fn build_for_in_of_statement_with_body_doc(
        &self,
        left: &internal::ForInOfLeft,
        right: &Expression,
        body: &Statement,
        stmt_start: u32,
        keyword: &str,  // "in" or "of"
        is_await: bool, // for-of `for await`; always false for for-in
    ) -> DocId {
        let d = self.d();
        let left_start = self.get_for_in_of_left_start(left);
        let left_end = self.get_for_in_of_left_end(left);
        let right_start = right.span().start;
        let right_end = right.span().end;

        // The keyword as a static literal (`d.text` needs `&'static str`), with
        // and without the leading space.
        let (kw, kw_spaced) = if keyword == "of" {
            ("of", " of")
        } else {
            ("in", " in")
        };

        // Find the keyword position (search with or without spaces)
        let keyword_pos = self
            .find_keyword_position(left_end, right_start, keyword)
            .unwrap_or(left_end);

        // Preserve comments between keywords and `(`
        // for await: two gaps — for-to-await and await-to-paren
        // for (non-await): one gap — for-to-paren
        let for_keyword_end = stmt_start + "for".len() as u32;
        let open_paren = self.find_open_paren_after(stmt_start);
        let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));
        let (for_await_comments, await_paren_comments) = if is_await {
            let await_pos = self.find_keyword_in_source(for_keyword_end, left_start, "await");
            let for_await_c = await_pos
                .and_then(|ap| self.build_inline_comments_between_doc_opt(for_keyword_end, ap));
            let await_paren_c = await_pos
                .map(|ap| ap + "await".len() as u32)
                .and_then(|ae| self.build_keyword_paren_comments(ae, open_paren));
            (for_await_c, await_paren_c)
        } else {
            (None, None)
        };
        let keyword_comments = if is_await {
            None
        } else {
            self.build_keyword_paren_comments(for_keyword_end, open_paren)
        };

        // Check for line comments in the header - if present, use breaking layout
        // We check from open paren to close paren
        let close = close_paren.unwrap_or(right_end + 1);
        let has_line_comments = if let Some(open) = open_paren {
            self.has_line_comments_between(open + 1, close)
        } else {
            self.has_line_comments_between(left_start, close)
        };

        // Build the `for ... (` opening once — shared by both the inline and the
        // breaking (line-comment) layouts, so each preserves any `for`-to-`(`
        // comment and emits `await` from the AST.
        let mut parts = Vec::new();
        self.push_for_open_paren(
            &mut parts,
            keyword_comments,
            for_await_comments,
            await_paren_comments,
            is_await,
        );

        if has_line_comments {
            return self.build_for_in_of_with_line_comments(
                left,
                right,
                body,
                keyword,
                keyword_pos,
                open_paren,
                close_paren,
                parts,
            );
        }

        // Comments between ( and left
        if let Some(open) = open_paren {
            for comment in tsv_lang::comments_in_range(self.comments, open + 1, left_start) {
                if comment.is_block {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.text(" "));
                }
            }
        }

        parts.push(self.build_for_in_of_left_doc(left));

        // Comments after left, before the keyword
        let has_left_comment =
            self.append_for_in_of_block_comments(&mut parts, left_end, keyword_pos);

        if has_left_comment {
            parts.push(d.text(kw));
        } else {
            parts.push(d.text(kw_spaced));
        }

        // Comments after the keyword, before right
        let keyword_end = keyword_pos + keyword.len() as u32;
        let has_comment =
            self.append_for_in_of_block_comments(&mut parts, keyword_end, right_start);
        if !has_comment {
            parts.push(d.text(" "));
        }

        parts.push(self.build_expression_doc(right));

        // Comments after right, before close paren (no trailing space needed)
        if let Some(close) = close_paren {
            self.append_for_in_of_trailing_comments(&mut parts, right_end, close);
        }

        // `)` + comments + body (shared with the breaking layout)
        self.push_for_close_paren_and_body(&mut parts, body, right_end, close_paren);

        // Group so a non-block body's `adjustClause` line breaks on overflow
        // (matches Prettier's `printForXStatement`).
        d.group(d.concat(&parts))
    }

    /// Build for-in/for-of statement with line comments preserved in their positions
    ///
    /// This is our divergence from Prettier - we preserve line comments where
    /// the user wrote them rather than relocating them.
    #[allow(clippy::too_many_arguments)]
    fn build_for_in_of_with_line_comments(
        &self,
        left: &internal::ForInOfLeft,
        right: &Expression,
        body: &Statement,
        keyword: &str, // "in" or "of"
        keyword_pos: u32,
        open_paren: Option<u32>,
        close_paren: Option<u32>,
        // The `for ... (` opening, prebuilt by the caller (comments preserved,
        // `await` from the AST) — shared with the inline layout.
        mut parts: Vec<DocId>,
    ) -> DocId {
        let d = self.d();
        let left_start = self.get_for_in_of_left_start(left);
        let left_end = self.get_for_in_of_left_end(left);
        let right_start = right.span().start;
        let right_end = right.span().end;
        let keyword_end = keyword_pos + keyword.len() as u32;

        // Inner content with hardline breaks
        let mut inner = Vec::new();

        // Comments before left (after open paren)
        if let Some(open) = open_paren {
            for comment in tsv_lang::comments_in_range(self.comments, open + 1, left_start) {
                inner.push(d.hardline());
                inner.push(self.build_comment_doc(comment));
            }
        }

        // Left side (const y)
        inner.push(d.hardline());
        inner.push(self.build_for_in_of_left_doc(left));

        // Comments after left, before keyword — emit all (own-line comments normalize to inline)
        for comment in tsv_lang::comments_in_range(self.comments, left_end, keyword_pos) {
            inner.push(d.text(" "));
            inner.push(self.build_comment_doc(comment));
        }

        // Keyword with extra indent (hardline is INSIDE the indent so keyword gets extra indent)
        let keyword_doc = match keyword {
            "in" => d.text("in"),
            "of" => d.text("of"),
            _ => d.text("of"), // fallback
        };
        let mut keyword_parts = vec![d.hardline(), keyword_doc];

        // Comments after keyword, before right — emit all (own-line comments normalize to inline)
        for comment in tsv_lang::comments_in_range(self.comments, keyword_end, right_start) {
            keyword_parts.push(d.text(" "));
            keyword_parts.push(self.build_comment_doc(comment));
        }

        // Right side (items)
        keyword_parts.push(d.hardline());
        keyword_parts.push(self.build_expression_doc(right));

        // Comments after right, before close paren
        if let Some(close) = close_paren {
            for comment in tsv_lang::comments_in_range(self.comments, right_end, close) {
                keyword_parts.push(d.text(" "));
                keyword_parts.push(self.build_comment_doc(comment));
            }
        }

        inner.push(d.indent(d.concat(&keyword_parts)));

        parts.push(d.indent(d.concat(&inner)));
        parts.push(d.hardline());

        // `)` + comments + body (shared with the inline layout)
        self.push_for_close_paren_and_body(&mut parts, body, right_end, close_paren);

        // Group so the non-block body's `adjustClause` line breaks (the
        // hardline-broken header forces this group open via `will_break`).
        d.group(d.concat(&parts))
    }

    /// Push the `for [comments] [await] (` opening into `parts`.
    ///
    /// Shared by the inline and breaking for-in/for-of header layouts so both
    /// preserve any comment in the `for`-to-`(` region (`keyword_comments` /
    /// `for_await_comments` / `await_paren_comments`) and emit `await` from
    /// `is_await` (the AST) — a comment that merely contains the word `await`
    /// stays a comment, never promoted to a `for await` keyword.
    fn push_for_open_paren(
        &self,
        parts: &mut Vec<DocId>,
        keyword_comments: Option<DocId>,
        for_await_comments: Option<DocId>,
        await_paren_comments: Option<DocId>,
        is_await: bool,
    ) {
        let d = self.d();
        parts.push(d.text("for"));
        if let Some(kc) = keyword_comments {
            parts.push(kc);
        }
        if let Some(fac) = for_await_comments {
            parts.push(fac);
        }
        parts.push(d.text(" "));
        if is_await {
            parts.push(d.text("await"));
            if let Some(apc) = await_paren_comments {
                parts.push(apc);
            }
            parts.push(d.text(" "));
        }
        parts.push(d.text("("));
    }

    /// Push `)` + comments + body for a for-in/for-of statement.
    ///
    /// Shared by the inline and breaking layouts: a block body expands an empty
    /// `{}` (`build_block_statement_expand_empty_doc`); a non-block body uses
    /// Prettier's `adjustClause` indentation.
    fn push_for_close_paren_and_body(
        &self,
        parts: &mut Vec<DocId>,
        body: &Statement,
        right_end: u32,
        close_paren: Option<u32>,
    ) {
        let paren_end = close_paren.map_or(right_end + 1, |p| p + 1);
        if let Statement::BlockStatement(block) = body {
            self.append_close_paren_with_comments(parts, paren_end, block.span.start);
            parts.push(self.build_block_statement_expand_empty_doc(block));
        } else {
            self.append_close_paren_with_non_block_body(parts, paren_end, body);
        }
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

    /// Append inline block comments for for-in/for-of statements.
    /// Emits ` comment` for each block comment, plus trailing ` ` if any were added.
    /// Own-line comments normalize to inline. Line comments are skipped (handled by
    /// the breaking layout path).
    /// Returns true if any comments were added.
    fn append_for_in_of_block_comments(
        &self,
        parts: &mut Vec<DocId>,
        start: u32,
        end: u32,
    ) -> bool {
        let d = self.d();
        let mut added = false;
        for comment in tsv_lang::comments_in_range(self.comments, start, end) {
            if comment.is_block {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
                added = true;
            }
        }
        if added {
            parts.push(d.text(" "));
        }
        added
    }

    /// Append trailing block comments for for-in/for-of statements.
    /// Own-line comments normalize to inline. No trailing space.
    fn append_for_in_of_trailing_comments(&self, parts: &mut Vec<DocId>, start: u32, end: u32) {
        let d = self.d();
        for comment in tsv_lang::comments_in_range(self.comments, start, end) {
            if comment.is_block {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }
        }
    }

    pub(in crate::printer::statements) fn build_for_statement_doc(
        &self,
        stmt: &internal::ForStatement,
    ) -> DocId {
        let d = self.d();

        // Preserve comments between `for` keyword and `(` in place:
        //   for/* c */(;;){} → for /* c */ (;;) {}
        let for_keyword_end = stmt.span.start + "for".len() as u32;
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let keyword_comments = self.build_keyword_paren_comments(for_keyword_end, open_paren);
        let has_pre_paren_comments = keyword_comments.is_some();

        // Check for comments between ) and body (Prettier 3.7 #18108)
        let header_end = self.get_for_header_end(stmt);
        let body_start = stmt.body.span().start;

        if has_pre_paren_comments || self.has_comments_between(header_end, body_start) {
            // Check if we have line comments (need special handling)
            let has_line_comment = self.has_line_comments_between(header_end, body_start);

            // Build parts with proper comment handling. A line comment between `)` and
            // the body forces the header to break (block comments can stay inline).
            let mut parts =
                vec![self.build_for_header_doc_impl(stmt, has_line_comment, keyword_comments)];

            // Post-header comments. Non-block bodies use Prettier's `adjustClause`
            // (`indent([line, body])`) wrapped with the header in an outer group, so
            // the body drops to its own indented line when the header breaks (a
            // comment hardline propagates) or the whole thing overflows — while the
            // header group still decides its own flat/break.
            let is_block_body = matches!(stmt.body.as_ref(), Statement::BlockStatement(_));
            let body_doc = self.build_statement_doc(&stmt.body);

            let (tail, group_it) = if self.has_comments_between(header_end, body_start) {
                if has_line_comment && !is_block_body {
                    // Line comment(s), non-block body: each comment on its own
                    // indented line, then the body — break-safe so a `//` can't
                    // swallow the next comment or the body (matches Prettier's
                    // adjustClause; multiple comments previously collapsed inline).
                    let mut inner = Vec::new();
                    let mut prev = header_end;
                    for comment in
                        tsv_lang::comments_in_range(self.comments, header_end, body_start)
                    {
                        if prev != header_end
                            && self.has_blank_line_between(prev, comment.span.start)
                        {
                            inner.push(d.literalline());
                        }
                        inner.push(d.hardline());
                        inner.push(self.build_comment_doc(comment));
                        prev = comment.span.end;
                    }
                    inner.push(d.hardline());
                    inner.push(body_doc);
                    (d.indent(d.concat(&inner)), false)
                } else if has_line_comment {
                    // Line comment(s), block body. Preserve each comment's position
                    // (no inline collapse → no swallow): a comment trailing `)`
                    // stays on the `)` line, own-line comments each keep their own
                    // line; then the block drops to the next line. Mirrors the
                    // shared `append_close_paren_with_comments` (which the C-style
                    // for can't call directly — its `)` is already in the header).
                    let (mut inline_prev, own_line, inline_next) =
                        self.partition_comments_by_line(header_end, body_start);
                    let mut own_line_lines: Vec<&tsv_lang::Comment> = Vec::new();
                    for comment in own_line.into_iter().chain(inline_next) {
                        if comment.is_block {
                            inline_prev.push(comment);
                        } else {
                            own_line_lines.push(comment);
                        }
                    }
                    let mut tail = Vec::new();
                    let effective_prev_end = inline_prev.last().map_or(header_end, |c| c.span.end);
                    self.build_comments_between_parts(
                        &mut tail,
                        &inline_prev,
                        &own_line_lines,
                        effective_prev_end,
                    );
                    tail.push(d.hardline());
                    tail.push(body_doc);
                    (d.concat(&tail), false)
                } else {
                    // Block comment(s) only — built here so the line-comment paths
                    // above don't compute an unused doc.
                    let comment_doc = self
                        .build_inline_comments_between_doc_no_leading_space(header_end, body_start);
                    if is_block_body {
                        // Block comment, block body: `) /* c */ {`
                        (
                            d.concat(&[d.text(" "), comment_doc, d.text(" "), body_doc]),
                            false,
                        )
                    } else {
                        // Block comment, non-block body: adjustClause keeps
                        // `) /* c */ body` flat but drops `\n\t/* c */ body` when the
                        // header breaks.
                        (
                            d.indent_line(d.concat(&[comment_doc, d.text(" "), body_doc])),
                            true,
                        )
                    }
                }
            } else if matches!(stmt.body.as_ref(), Statement::EmptyStatement(_)) {
                // Empty body attaches directly: `);` (no space, no adjustClause).
                // Matches the main path (`build_for_statement_with_body_doc`) and Prettier.
                (body_doc, false)
            } else if is_block_body {
                (d.concat(&[d.text(" "), body_doc]), false)
            } else {
                (d.indent_line(body_doc), true)
            };

            parts.push(tail);
            if group_it {
                d.group(d.concat(&parts))
            } else {
                d.concat(&parts)
            }
        } else {
            // Delegate to the sophisticated version that handles all edge cases
            self.build_for_statement_with_body_doc(stmt)
        }
    }

    fn build_for_init_doc(&self, init: &internal::ForInit) -> DocId {
        let d = self.d();
        match init {
            internal::ForInit::VariableDeclaration(decl) => {
                let mut parts = vec![d.text(decl.kind.as_str()), d.text(" ")];
                for (i, declarator) in decl.declarations.iter().enumerate() {
                    if i > 0 {
                        parts.push(d.text(", "));
                    }
                    parts.push(self.build_expression_doc(&declarator.id));
                    if let Some(init) = &declarator.init {
                        let id_end = declarator.id.span().end;
                        let init_start = init.span().start;
                        let eq_pos = self.find_equals_position(id_end, init_start);
                        parts.push(d.text(" = "));
                        if let Some(comments) = self.build_rhs_comments_opt(eq_pos + 1, init_start)
                        {
                            parts.push(comments);
                        }
                        parts.push(self.build_expression_doc(init));
                    }
                }
                d.concat(&parts)
            }
            internal::ForInit::Expression(expr) => {
                // Sequence expressions in for loop init don't need outer parens
                // e.g., `for (i = 0, j = 0; ...)` not `for ((i = 0, j = 0); ...)`
                // Same handling as build_for_update_doc
                if let Expression::SequenceExpression(seq) = expr {
                    d.join(
                        seq.expressions.iter().map(|e| self.build_expression_doc(e)),
                        ", ",
                    )
                } else {
                    self.build_expression_doc(expr)
                }
            }
        }
    }

    pub(in crate::printer::statements) fn build_for_in_statement_doc(
        &self,
        stmt: &internal::ForInStatement,
    ) -> DocId {
        // Delegate to the sophisticated version that handles empty block expansion
        self.build_for_in_statement_with_body_doc(stmt)
    }

    pub(in crate::printer::statements) fn build_for_of_statement_doc(
        &self,
        stmt: &internal::ForOfStatement,
    ) -> DocId {
        // Delegate to the sophisticated version that handles empty block expansion
        self.build_for_of_statement_with_body_doc(stmt)
    }

    fn build_for_in_of_left_doc(&self, left: &internal::ForInOfLeft) -> DocId {
        let d = self.d();
        match left {
            internal::ForInOfLeft::VariableDeclaration(decl) => {
                let mut parts = vec![d.text(decl.kind.as_str()), d.text(" ")];
                if let Some(declarator) = decl.declarations.first() {
                    parts.push(self.build_expression_doc(&declarator.id));
                }
                d.concat(&parts)
            }
            internal::ForInOfLeft::Pattern(expr) => self.build_expression_doc(expr),
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
