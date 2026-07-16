// Loop statement printing: for, for-in, for-of
//
// for-loop header layout (init/test/update clauses with comment placement),
// for-in/for-of left/right printing.

use crate::ast::internal::{self, Expression, Statement};
use crate::printer::{CommentVec, LeadingGlue, Printer};
use smallvec::smallvec;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::{TriviaProfile, find_char, skip_comment};

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

/// Mutable cursor state while laying out an empty `for (;;)` header's comments.
struct EmptyForCursor {
    /// A `//` line comment was just emitted: it runs to end-of-line, so the next
    /// item must start on a new line.
    pending_break: bool,
    /// The previously emitted item was a block comment, so a separating space is
    /// owed before a following `;`.
    prev_block: bool,
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
        parts: &mut DocBuf,
        paren_end: u32,
        body: &Statement<'_>,
    ) {
        let d = self.d();
        let body_start = body.span().start;
        let body_doc = self.build_statement_doc(body, false);

        if !self.has_comments_to_emit_between(paren_end, body_start) {
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
            let mut inner: DocBuf = smallvec![d.hardline()];
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
            let mut inner = DocBuf::new();
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
    fn build_for_statement_with_body_doc(&self, stmt: &internal::ForStatement<'_>) -> DocId {
        let d = self.d();
        let header_doc = self.build_for_header_doc(stmt);
        if matches!(stmt.body, Statement::EmptyStatement(_)) {
            // No space before empty statement: `for (...);`
            d.concat(&[header_doc, self.build_statement_doc(stmt.body, false)])
        } else if let Statement::BlockStatement(block) = stmt.body {
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
            let body_doc = self.build_statement_doc(stmt.body, false);
            d.group(d.concat(&[header_doc, d.indent_line(body_doc)]))
        }
    }

    /// Get the end position of a for loop header (position after the closing paren)
    fn get_for_header_end(&self, stmt: &internal::ForStatement<'_>) -> u32 {
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
    fn build_for_header_doc(&self, stmt: &internal::ForStatement<'_>) -> DocId {
        self.build_for_header_doc_impl(stmt, None)
    }

    /// Build doc for an empty `for (;;)` header that has comments inside the parens.
    ///
    /// Preserves comments in their authored positions — a divergence from prettier,
    /// which relocates every comment outside the parens once all three clauses are
    /// empty (prettier itself keeps them inline when any clause is non-empty, so its
    /// relocation is internally inconsistent). See the
    /// `empty_clauses*_comment_prettier_divergence` fixtures.
    ///
    /// The header breaks only where a `//` line comment forces a line end: with
    /// block comments alone the whole header stays on one line (`for (/* a */ ;;)`);
    /// a line comment drops the rest of the header to the next line, but the `;;`
    /// stay together when nothing separates them (`for ( // c⏎\t;;⏎)`). `for_open`
    /// is the already-built `for (` prefix (carrying any `for`→`(` keyword comment).
    fn build_for_empty_with_comments(
        &self,
        stmt: &internal::ForStatement<'_>,
        for_open: DocId,
    ) -> DocId {
        let d = self.d();
        let Some(open) = self.find_open_paren_after(stmt.span.start) else {
            return d.concat(&[for_open, d.text(";;)")]);
        };
        let Some(close) = self.matching_close_paren(open) else {
            return d.concat(&[for_open, d.text(";;)")]);
        };
        let (Some(s1), Some(s2)) = self.find_for_semicolons(open) else {
            return d.concat(&[for_open, d.text(";;)")]);
        };

        // A `//` line comment anywhere in the header runs to end-of-line, so it
        // forces the following tokens onto new lines; with only block comments the
        // header stays inline.
        let breaking = self.has_line_comments_between(open + 1, close);

        let mut inner = DocBuf::new();
        let mut cur = EmptyForCursor {
            pending_break: false,
            prev_block: false,
        };

        // Region before the first `;` is anchored on `(` (a leading block comment
        // hugs it: `for (/* a */`); regions after a `;` space-separate block
        // comments (`; /* b */`).
        self.emit_empty_for_comments(&mut inner, &mut cur, open + 1, s1, open, true);
        self.emit_empty_for_semicolon(&mut inner, &mut cur);
        self.emit_empty_for_comments(&mut inner, &mut cur, s1 + 1, s2, s1, false);
        self.emit_empty_for_semicolon(&mut inner, &mut cur);
        self.emit_empty_for_comments(&mut inner, &mut cur, s2 + 1, close, s2, false);

        // A `//` forces breaks: indent the body and drop `)` to its own line.
        // Block-only headers stay on the single `for (…)` line.
        let body = d.concat(&inner);
        if breaking {
            d.concat(&[for_open, d.indent(body), d.hardline(), d.text(")")])
        } else {
            d.concat(&[for_open, body, d.text(")")])
        }
    }

    /// Emit the comments of one empty-`for` header region (`[start, end)`) into
    /// `inner`, advancing `cur`. `anchor` is the end of the token the region
    /// follows (used for same-line classification); `hug` is set for the leading
    /// region so a block comment hugs the `(` with no separating space.
    fn emit_empty_for_comments(
        &self,
        inner: &mut DocBuf,
        cur: &mut EmptyForCursor,
        start: u32,
        end: u32,
        anchor: u32,
        hug: bool,
    ) {
        let d = self.d();
        let mut prev = anchor;
        let mut first = true;
        for comment in comments_to_emit_in_range(self.comments, start, end) {
            if comment.is_block {
                if cur.pending_break {
                    inner.push(d.hardline());
                    cur.pending_break = false;
                } else if !(first && hug) {
                    inner.push(d.text(" "));
                }
                inner.push(self.build_comment_doc(comment));
                cur.prev_block = true;
            } else {
                // Line comment: breaks the line after itself (`pending_break`).
                if cur.pending_break || !self.is_same_line(prev, comment.span.start) {
                    inner.push(d.hardline());
                } else {
                    inner.push(d.text(" "));
                }
                inner.push(self.build_comment_doc(comment));
                cur.pending_break = true;
                cur.prev_block = false;
            }
            prev = comment.span.end;
            first = false;
        }
    }

    /// Emit one `;` of an empty-`for` header into `inner`, advancing `cur`: a
    /// pending line comment forces it to a new line, a preceding block comment
    /// owes it a separating space, otherwise it joins the run (`;;`).
    fn emit_empty_for_semicolon(&self, inner: &mut DocBuf, cur: &mut EmptyForCursor) {
        let d = self.d();
        if cur.pending_break {
            inner.push(d.hardline());
            cur.pending_break = false;
        } else if cur.prev_block {
            inner.push(d.text(" "));
        }
        inner.push(d.text(";"));
        cur.prev_block = false;
    }

    fn build_for_header_doc_impl(
        &self,
        stmt: &internal::ForStatement<'_>,
        keyword_comments: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        let has_init = stmt.init.is_some();
        let has_test = stmt.test.is_some();
        let has_update = stmt.update.is_some();
        let has_any = has_init || has_test || has_update;

        // Build "for" + optional keyword comments + " (" prefix
        let for_open = if let Some(kc) = keyword_comments {
            // `kc` carries its own trailing space (block) or hardline (line).
            d.concat(&[d.text("for"), kc, d.text("(")])
        } else {
            d.text("for (")
        };

        // Check if there are any comments inside the for parens
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren_approx = open_paren.and_then(|p| self.matching_close_paren(p));
        let has_comments_inside =
            if let (Some(open), Some(close)) = (open_paren, close_paren_approx) {
                self.has_comments_to_emit_between(open, close)
            } else {
                false
            };

        if !has_any && !has_comments_inside {
            // Empty for (;;) with no comments - no wrapping needed
            return d.concat(&[for_open, d.text(";;)")]);
        }

        if !has_any && has_comments_inside {
            // Empty for (;;) with comments — preserve them inline where authored
            // (divergence from prettier; see empty_clauses*_comment_prettier_divergence).
            return self.build_for_empty_with_comments(stmt, for_open);
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
            // Reuse the close-paren already found above (`matching_close_paren` is a
            // depth-tracked scan over the whole header) instead of recomputing it.
            close_paren: close_paren_approx,
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
        let has_own_line_comments =
            has_line_comment_in_header || self.for_header_has_own_line_comments(&spans);

        // Extract span positions for use throughout this function
        let init_start = spans.init_start;
        let test_start = spans.test_start;
        let update_start = spans.update_start;
        let close_paren = spans.close_paren;

        let mut inner_parts = DocBuf::new();

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
            for comment in comments_to_emit_in_range(self.comments, open + 1, first_start) {
                if self.comment_hugs_next(comment, first_start) {
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
        // The init clause→`;` gap comments bind to the `;` like a list separator.
        self.push_for_clause_semicolon(&mut inner_parts, init_end, first_semi);

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
        } else {
            // No test clause: still emit the post-`;` separator (a space when flat) so
            // the header isn't collapsed to `;;`. Prettier keeps it whenever the header
            // isn't fully empty — `for (x = 0; ;)`, not `for (x = 0;;)` (the fully-empty
            // `for (;;)` is handled by the early return above). Covers init-only,
            // update-only, and init+update alike.
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
        // The test clause→`;` gap comments bind to the `;` like a list separator.
        self.push_for_clause_semicolon(&mut inner_parts, test_end, second_semi);

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
        }
        // When the update clause is absent, nothing trails the last `;`: prettier
        // 3.9 (#19188) dropped the space it used to add before `)` →
        // `for (…; cond;)`, not `for (…; cond; )`.

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
    ) -> DocBuf {
        let d = self.d();
        let mut parts = DocBuf::new();
        for comment in comments_to_emit_in_range(self.comments, search_start, clause_start) {
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
    fn build_for_clause_leading_comments(&self, start: u32, clause_start: u32) -> DocBuf {
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

    /// Emit a for-header clause terminator `;` with its content→`;` gap comments
    /// bound to the `;` **like a list separator** (`split_separator_gap_comments`,
    /// `block_after_separator: false`): a same-line block stays before the `;`
    /// (`a /* c */;`), a same-line line trails it via `line_suffix` (`a; // c`),
    /// and an own-line comment defers to its own line **after** the `;`
    /// (matching prettier, which keeps a for-header comment inline only when all
    /// three clauses are empty — see `build_for_empty_with_comments`). A blank
    /// line before an own-line comment is not preserved, as prettier collapses it
    /// in a for-header gap. `clause_end`/`semi` are the clause's end and the
    /// source `;` position; either being absent emits a bare `;`.
    fn push_for_clause_semicolon(
        &self,
        parts: &mut DocBuf,
        clause_end: Option<u32>,
        semi: Option<u32>,
    ) {
        let d = self.d();
        let after = match (clause_end, semi) {
            (Some(start), Some(sep)) => self.split_separator_gap_comments(parts, start, sep, false),
            _ => DocBuf::new(),
        };
        parts.push(d.text(";"));
        parts.extend(after);
    }

    /// Find the two `;` separators in a for-header, scanning forward from
    /// `scan_start`. Returns `(first_semi, second_semi)`; the second is only sought
    /// once the first is found.
    fn find_for_semicolons(&self, scan_start: u32) -> (Option<u32>, Option<u32>) {
        // Skip any `;` inside a comment in a clause (`for (let i = 0 /* ; */; …)`).
        let bytes = self.source.as_bytes();
        let first_semi = find_char(
            bytes,
            scan_start as usize,
            bytes.len(),
            b';',
            TriviaProfile::JS,
        )
        .map(|p| p as u32);
        let second_semi = first_semi
            .and_then(|p| {
                find_char(
                    bytes,
                    (p + 1) as usize,
                    bytes.len(),
                    b';',
                    TriviaProfile::JS,
                )
            })
            .map(|p| p as u32);
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
    /// update expression. Unlike `push_for_clause_semicolon` (the content→`;` gap,
    /// bound to the `;`), this emits every comment kind that shares a line with the
    /// clause end, from the region *after* the `;`.
    fn push_for_clause_same_line_comments(
        &self,
        parts: &mut DocBuf,
        range_start: u32,
        boundary: u32,
        end: u32,
    ) {
        let d = self.d();
        for comment in comments_to_emit_in_range(self.comments, range_start, boundary) {
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
        parts: &mut DocBuf,
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
        for comment in comments_to_emit_in_range(self.comments, search_start, clause_start) {
            if comment.is_block
                && self.is_same_line(comment.span.end, clause_start)
                && prev_end.is_none_or(|pe| !self.is_same_line(pe, comment.span.start))
            {
                parts.push(self.build_comment_doc(comment));
                parts.push(d.text(" "));
            }
        }
    }

    /// Build a for-header init/update clause that is a `SequenceExpression`
    /// (`for (a = 1, b = 2; …)` / `for (…; a++, b++)`), preserving comments in the
    /// inter-operand comma gaps — a same-line block leads the next operand inline
    /// (`, /* c */ b`), a line comment trails the comma and forces the per-operand
    /// break (`a, // c⏎\tb`) — the same gap handling as a multi-declarator init
    /// clause (`push_for_clause_comma_gap`). The comment-free case stays a flat
    /// `", "` join. `build_elem` renders one operand: the init clause wraps it in
    /// `wrap_for_init_in` for the `[~In]` restriction, the update clause does not.
    ///
    /// Before this, both sequence branches emitted a comment-blind `", "` join,
    /// silently dropping every inter-operand comment (`for (a = 1, /* c */ b = 2;
    /// …)` lost `/* c */`).
    fn build_for_sequence_clause_doc(
        &self,
        seq: &internal::SequenceExpression<'_>,
        build_elem: impl Fn(&Expression<'_>) -> DocId,
    ) -> DocId {
        let d = self.d();
        let first = seq.expressions[0].span().start;
        let last = seq.expressions[seq.expressions.len() - 1].span().end;
        if !self.has_comments_to_emit_between(first, last) {
            return d.join(seq.expressions.iter().map(&build_elem), ", ");
        }
        let mut docs = DocBuf::new();
        for (i, e) in seq.expressions.iter().enumerate() {
            if i > 0 {
                self.push_for_clause_comma_gap(
                    &mut docs,
                    seq.expressions[i - 1].span().end,
                    e.span().start,
                );
            }
            docs.push(build_elem(e));
        }
        // Group + indent so a line-comment break continuation-indents one level,
        // matching the multi-declarator init clause.
        d.group(d.indent(d.concat(&docs)))
    }

    /// Render a for-header init/update clause expression. A `SequenceExpression`
    /// (`a = 1, b = 2`) routes through `build_for_sequence_clause_doc` for
    /// inter-operand comment handling; any other expression is rendered directly
    /// by `build_elem`. Sharing this dispatch keeps the init and update clauses
    /// from diverging on how a comma sequence is detected and routed. `build_elem`
    /// renders one operand — the init clause wraps it in `wrap_for_init_in` for the
    /// `[~In]` restriction, the update clause does not.
    fn build_for_expr_clause(
        &self,
        expr: &Expression<'_>,
        build_elem: impl Fn(&Expression<'_>) -> DocId,
    ) -> DocId {
        if let Expression::SequenceExpression(seq) = expr {
            self.build_for_sequence_clause_doc(seq, build_elem)
        } else {
            build_elem(expr)
        }
    }

    /// Build a Doc for a for loop update expression
    fn build_for_update_doc(&self, expr: &Expression<'_>) -> DocId {
        self.build_for_expr_clause(expr, |e| self.build_expression_doc(e))
    }

    /// Build a complete for-in statement doc including the body
    fn build_for_in_statement_with_body_doc(&self, stmt: &internal::ForInStatement<'_>) -> DocId {
        self.build_for_in_of_statement_with_body_doc(
            &stmt.left,
            &stmt.right,
            stmt.body,
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
            if let Some(new_i) = skip_comment(bytes, i, len) {
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
    fn build_for_of_statement_with_body_doc(&self, stmt: &internal::ForOfStatement<'_>) -> DocId {
        self.build_for_in_of_statement_with_body_doc(
            &stmt.left,
            &stmt.right,
            stmt.body,
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
        left: &internal::ForInOfLeft<'_>,
        right: &Expression<'_>,
        body: &Statement<'_>,
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
            let await_pos = self.find_keyword_in_range(for_keyword_end, left_start, "await");
            // Same line-safe builder as the `await`→`(` gap below: a line comment in
            // the `for`→`await` gap breaks `await` onto the next line so the `//`
            // can't swallow it; a block comment keeps its glue space.
            let for_await_c = await_pos
                .and_then(|ap| self.build_keyword_paren_comments(for_keyword_end, Some(ap)));
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
        let mut parts = d.pooled_docbuf();
        self.push_for_open_paren(
            &mut parts,
            keyword_comments,
            for_await_comments,
            await_paren_comments,
            is_await,
        );

        let async_lhs_paren = self.for_lhs_needs_async_paren(left, keyword, is_await);

        if has_line_comments {
            return self.build_for_in_of_with_line_comments(
                left,
                right,
                body,
                keyword,
                keyword_pos,
                open_paren,
                close_paren,
                async_lhs_paren,
                &mut parts,
            );
        }

        // Comments between ( and left
        if let Some(open) = open_paren {
            for comment in comments_to_emit_in_range(self.comments, open + 1, left_start) {
                if comment.is_block {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.text(" "));
                }
            }
        }

        parts.push(self.build_for_in_of_left_doc(left, async_lhs_paren));

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
        left: &internal::ForInOfLeft<'_>,
        right: &Expression<'_>,
        body: &Statement<'_>,
        keyword: &str, // "in" or "of"
        keyword_pos: u32,
        open_paren: Option<u32>,
        close_paren: Option<u32>,
        // Whether the LHS is a bare `async` identifier needing parens (see
        // `for_lhs_needs_async_paren`); precomputed by the caller, which has the
        // `is_await` flag this method doesn't carry.
        wrap_async_paren: bool,
        // The `for ... (` opening, prebuilt by the caller (comments preserved,
        // `await` from the AST) — shared with the inline layout. Filled in place
        // (a pooled buffer owned by the caller) rather than taken by value.
        parts: &mut DocBuf,
    ) -> DocId {
        let d = self.d();
        let left_start = self.get_for_in_of_left_start(left);
        let left_end = self.get_for_in_of_left_end(left);
        let right_start = right.span().start;
        let right_end = right.span().end;
        let keyword_end = keyword_pos + keyword.len() as u32;

        // Inner content with hardline breaks
        let mut inner = DocBuf::new();

        // Comments before left (after open paren)
        if let Some(open) = open_paren {
            for comment in comments_to_emit_in_range(self.comments, open + 1, left_start) {
                inner.push(d.hardline());
                inner.push(self.build_comment_doc(comment));
            }
        }

        // Left side (const y)
        inner.push(d.hardline());
        inner.push(self.build_for_in_of_left_doc(left, wrap_async_paren));

        // Comments after left, before keyword — emit all (own-line comments normalize to inline)
        for comment in comments_to_emit_in_range(self.comments, left_end, keyword_pos) {
            inner.push(d.text(" "));
            inner.push(self.build_comment_doc(comment));
        }

        // Keyword with extra indent (hardline is INSIDE the indent so keyword gets extra indent)
        let keyword_doc = match keyword {
            "in" => d.text("in"),
            "of" => d.text("of"),
            _ => d.text("of"), // fallback
        };
        let mut keyword_parts: DocBuf = smallvec![d.hardline(), keyword_doc];

        // Comments after keyword, before right — emit all (own-line comments normalize to inline)
        for comment in comments_to_emit_in_range(self.comments, keyword_end, right_start) {
            keyword_parts.push(d.text(" "));
            keyword_parts.push(self.build_comment_doc(comment));
        }

        // Right side (items)
        keyword_parts.push(d.hardline());
        keyword_parts.push(self.build_expression_doc(right));

        // Comments after right, before close paren
        if let Some(close) = close_paren {
            for comment in comments_to_emit_in_range(self.comments, right_end, close) {
                keyword_parts.push(d.text(" "));
                keyword_parts.push(self.build_comment_doc(comment));
            }
        }

        inner.push(d.indent(d.concat(&keyword_parts)));

        parts.push(d.indent(d.concat(&inner)));
        parts.push(d.hardline());

        // `)` + comments + body (shared with the inline layout)
        self.push_for_close_paren_and_body(parts, body, right_end, close_paren);

        // Group so the non-block body's `adjustClause` line breaks (the
        // hardline-broken header forces this group open via `will_break`).
        d.group(d.concat(parts))
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
        parts: &mut DocBuf,
        keyword_comments: Option<DocId>,
        for_await_comments: Option<DocId>,
        await_paren_comments: Option<DocId>,
        is_await: bool,
    ) {
        let d = self.d();
        parts.push(d.text("for"));
        // `for` → (`await` | `(`) transition. Both `keyword_comments` (the non-await
        // `for`→`(` gap) and `for_await_comments` (the `for`→`await` gap) are built by
        // `build_keyword_paren_comments`, so each already carries its own trailing
        // space (block) or hardline (line) — a line comment breaks the next token
        // (`(` or `await`) onto its own line so the `//` can't swallow it. The two are
        // mutually exclusive (keyword_comments only non-await, for_await_comments only
        // await).
        if let Some(kc) = keyword_comments {
            parts.push(kc);
        } else if let Some(fac) = for_await_comments {
            parts.push(fac);
        } else {
            parts.push(d.text(" "));
        }
        if is_await {
            parts.push(d.text("await"));
            // `await` → `(` transition: `await_paren_comments` carries its own
            // trailing space/break; otherwise a plain space.
            if let Some(apc) = await_paren_comments {
                parts.push(apc);
            } else {
                parts.push(d.text(" "));
            }
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
        parts: &mut DocBuf,
        body: &Statement<'_>,
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
    fn get_for_in_of_left_end(&self, left: &internal::ForInOfLeft<'_>) -> u32 {
        match left {
            internal::ForInOfLeft::VariableDeclaration(decl) => decl.span.end,
            internal::ForInOfLeft::Pattern(expr) => expr.span().end,
        }
    }

    /// Get the start position of the left side of a for-in/for-of statement
    fn get_for_in_of_left_start(&self, left: &internal::ForInOfLeft<'_>) -> u32 {
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
    fn append_for_in_of_block_comments(&self, parts: &mut DocBuf, start: u32, end: u32) -> bool {
        let d = self.d();
        let mut added = false;
        for comment in comments_to_emit_in_range(self.comments, start, end) {
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
    fn append_for_in_of_trailing_comments(&self, parts: &mut DocBuf, start: u32, end: u32) {
        let d = self.d();
        for comment in comments_to_emit_in_range(self.comments, start, end) {
            if comment.is_block {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }
        }
    }

    pub(in crate::printer::statements) fn build_for_statement_doc(
        &self,
        stmt: &internal::ForStatement<'_>,
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

        if has_pre_paren_comments || self.has_comments_to_emit_between(header_end, body_start) {
            // Check if we have line comments (need special handling)
            let has_line_comment = self.has_line_comments_between(header_end, body_start);

            // Build parts with proper comment handling. A comment between `)` and the
            // body does NOT force the header to break — the header decides its own
            // flat/break on its own width (prettier 3.9 collapses `for (i; c; u)` and
            // trails the comment after `)`). Only comments *inside* the parens (handled
            // in `build_for_header_doc_impl`) or overflow expand the header.
            let mut parts: DocBuf =
                smallvec![self.build_for_header_doc_impl(stmt, keyword_comments)];

            // Post-header comments. Non-block bodies use Prettier's `adjustClause`
            // (`indent([line, body])`) wrapped with the header in an outer group, so
            // the body drops to its own indented line when the header breaks (a
            // comment hardline propagates) or the whole thing overflows — while the
            // header group still decides its own flat/break.
            let is_block_body = matches!(stmt.body, Statement::BlockStatement(_));
            // A C-style `for` collapses its empty block body (`for (…) {}`).
            let body_doc = self.build_collapsing_body_doc(stmt.body);

            let (tail, group_it) = if self.has_comments_to_emit_between(header_end, body_start) {
                if has_line_comment && !is_block_body {
                    // Line comment(s), non-block body: each comment on its own
                    // indented line, then the body — break-safe so a `//` can't
                    // swallow the next comment or the body (matches Prettier's
                    // adjustClause; multiple comments previously collapsed inline).
                    let mut inner = DocBuf::new();
                    let mut prev = header_end;
                    for comment in comments_to_emit_in_range(self.comments, header_end, body_start)
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
                    let mut own_line_lines: CommentVec<'_> = smallvec![];
                    for comment in own_line.into_iter().chain(inline_next) {
                        if comment.is_block {
                            inline_prev.push(comment);
                        } else {
                            own_line_lines.push(comment);
                        }
                    }
                    let mut tail = DocBuf::new();
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
            } else if matches!(stmt.body, Statement::EmptyStatement(_)) {
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

    /// Emit the separator between two comma-separated for-header operands (the
    /// `prev_end → curr_start` gap): the comma plus any inter-operand comments,
    /// kept on the author's side of the comma — a before-comma block trails the
    /// previous operand (`… = 0 /* c */,`), an after-comma comment leads the next
    /// one (`, /* c */ x`). A line comment trails the comma via `line_suffix` and,
    /// like an own-line block, forces the per-operand break; inline blocks stay
    /// width-based (the `line` collapses when the group fits). Mirrors the
    /// variable-declarator inter-declarator comment placement. Shared by the
    /// multi-declarator init clause and the init/update `SequenceExpression`
    /// clauses (`build_for_sequence_clause_doc`).
    fn push_for_clause_comma_gap(&self, decl_docs: &mut DocBuf, prev_end: u32, curr_start: u32) {
        let d = self.d();
        if !self.has_comments_to_emit_between(prev_end, curr_start) {
            decl_docs.push(d.text(","));
            decl_docs.push(d.line());
            return;
        }
        let comma_pos = self.comma_between(prev_end, curr_start);

        if self.has_line_comments_between(prev_end, curr_start) {
            // A line comment forces the break, which the gap owns. The whole declarator
            // run is wrapped in a `d.indent()` by the caller, so continuation lines need
            // no explicit indent text (empty).
            self.push_inter_item_line_comment_gap(
                decl_docs,
                prev_end,
                comma_pos,
                curr_start,
                d.empty(),
            );
        } else {
            // Blocks only: a before-comma block trails the previous initializer; the
            // width-based `line` separates; after-comma blocks lead the next
            // declarator (an own-line block drops to its own line and forces the group
            // to break, an inline block hugs `, /* c */ x`). A stranded after-comma
            // block (on the comma's line, newline before the next declarator) trails
            // the comma instead — preserving the author's placement; prettier relocates
            // it before the comma.
            self.push_before_comma_blocks(decl_docs, prev_end, comma_pos);
            decl_docs.push(d.text(","));
            self.push_stranded_after_comma_blocks(decl_docs, comma_pos, curr_start);
            decl_docs.push(d.line());
            let after: CommentVec<'_> =
                comments_to_emit_in_range(self.comments, comma_pos, curr_start)
                    .filter(|c| !self.is_stranded_after_comma_block(c, comma_pos, curr_start))
                    .collect();
            self.push_leading_comment_run(
                decl_docs,
                after.iter().copied(),
                curr_start,
                LeadingGlue::Adjacent,
                d.empty(),
            );
        }
    }

    /// Prefix a for-header declaration's `continuation` (the declarator run, or the
    /// for-of/for-in binding) with its `kind` keyword plus any comment in the
    /// keyword→binding gap (`for (const /* c */ x of y)`, `for (let /* c */ i = 0;
    /// …)`). Routes through `build_keyword_to_name_continuation` — the same helper the
    /// standalone declaration uses — so the gap comment isn't dropped; byte-identical
    /// to `kind + " " + continuation` when the gap is comment-free, so a caller's
    /// enclosing `group`/`indent` is preserved. A for-header declaration is never
    /// `declare`, but its kind may still be two words (`await using`), whose own
    /// interior gap is emitted by `build_keyword_words_doc`.
    fn build_for_decl_keyword_gap(
        &self,
        decl: &internal::VariableDeclaration<'_>,
        binding_start: u32,
        continuation: DocId,
    ) -> DocId {
        self.build_keyword_header_doc(
            decl.kind.words(),
            decl.span.start,
            binding_start,
            continuation,
        )
    }

    fn build_for_init_doc(&self, init: &internal::ForInit<'_>) -> DocId {
        let d = self.d();
        // The init clause is `[~In]`: an `in` binary must be parenthesized so it
        // isn't read as the `for (x in y)` separator. Set for the whole init
        // subtree (prettier parenthesizes every `in` lexically under the init,
        // including inside nested function/class bodies); a nested for-header
        // re-enables it for its own init. The `wrap_for_init_in` calls below cover
        // the positions that build an expression without a `needs_parens` check;
        // everything else routes through `needs_parens`, now flag-aware.
        let saved_in_for_init = self.in_for_init.replace(true);
        let result = match init {
            internal::ForInit::VariableDeclaration(decl) => {
                // Build each declarator's `id = value` doc.
                let mut decl_docs: DocBuf = DocBuf::new();
                for (i, declarator) in decl.declarations.iter().enumerate() {
                    if i > 0 {
                        let prev_end = decl.declarations[i - 1].span.end;
                        self.push_for_clause_comma_gap(
                            &mut decl_docs,
                            prev_end,
                            declarator.span.start,
                        );
                    }
                    let mut one: DocBuf = smallvec![self.build_expression_doc(&declarator.id)];
                    if let Some(init) = &declarator.init {
                        let id_end = declarator.id.span().end;
                        let init_start = init.span().start;
                        let eq_pos = self.find_equals_position(id_end, init_start);
                        // A comment after `=` that forces a break (line comment, or an
                        // own-line / multiline block) breaks after the `=` and keeps the
                        // comment on its own line — the same handling as a variable
                        // declarator (gluing it up onto the `=` line would be
                        // non-idempotent). A single-line block glued inline to `=` still
                        // hugs the value across a source newline (`i = /* c */⏎0` →
                        // `i = /* c */ 0`) and keeps the header flat.
                        if let Some(rhs) =
                            self.build_eq_comment_break_rhs(eq_pos, init_start, || {
                                self.wrap_for_init_in(init, self.build_expression_doc(init))
                            })
                        {
                            one.push(rhs);
                        } else {
                            one.push(d.text(" = "));
                            if let Some(comments) =
                                self.build_rhs_comments_glued_opt(eq_pos + 1, init_start)
                            {
                                one.push(comments);
                            }
                            one.push(self.wrap_for_init_in(init, self.build_expression_doc(init)));
                        }
                    }
                    decl_docs.push(d.concat(&one));
                }
                // The keyword→first-declarator gap carries a comment (`for (let /* c */
                // i = 0; …)`) that must not be dropped — see `build_for_decl_keyword_gap`.
                let first_decl_start = decl.declarations[0].span.start;
                if decl.declarations.len() > 1 {
                    // Multiple declarators break on width: they stay on one line when the
                    // init clause fits and drop onto their own lines (continuation
                    // indented one level) when it doesn't — matching prettier's
                    // `printVariableDeclaration`. A declarator whose `=` comment forces a
                    // break also breaks the group (its hardline propagates).
                    d.group(self.build_for_decl_keyword_gap(
                        decl,
                        first_decl_start,
                        d.indent(d.concat(&decl_docs)),
                    ))
                } else {
                    self.build_for_decl_keyword_gap(decl, first_decl_start, d.concat(&decl_docs))
                }
            }
            internal::ForInit::Expression(expr) => {
                // Sequence expressions in for loop init don't need outer parens
                // e.g., `for (i = 0, j = 0; ...)` not `for ((i = 0, j = 0); ...)`.
                // Same dispatch as build_for_update_doc, but each operand is `[~In]`
                // wrapped (`wrap_for_init_in`).
                self.build_for_expr_clause(expr, |e| {
                    self.wrap_for_init_in(e, self.build_expression_doc(e))
                })
            }
        };
        self.in_for_init.set(saved_in_for_init);
        result
    }

    pub(in crate::printer::statements) fn build_for_in_statement_doc(
        &self,
        stmt: &internal::ForInStatement<'_>,
    ) -> DocId {
        // Delegate to the sophisticated version that handles empty block expansion
        self.build_for_in_statement_with_body_doc(stmt)
    }

    pub(in crate::printer::statements) fn build_for_of_statement_doc(
        &self,
        stmt: &internal::ForOfStatement<'_>,
    ) -> DocId {
        // Delegate to the sophisticated version that handles empty block expansion
        self.build_for_of_statement_with_body_doc(stmt)
    }

    fn build_for_in_of_left_doc(
        &self,
        left: &internal::ForInOfLeft<'_>,
        wrap_async_paren: bool,
    ) -> DocId {
        let d = self.d();
        match left {
            internal::ForInOfLeft::VariableDeclaration(decl) => {
                let Some(declarator) = decl.declarations.first() else {
                    // A for-in/of head always binds something (`for (const of x)` is a
                    // parse error), so there is no declarator to bound a gap search at
                    // and nothing to print but the kind. `as_str()` is the joined text —
                    // the very thing `words()` exists to avoid, since it emits `await
                    // using`'s interior gap as a fixed space and would drop a comment
                    // authored there. Safe only because this arm is unreachable; assert
                    // that rather than let a future caller reach it silently.
                    debug_assert!(
                        false,
                        "a for-in/of variable declaration always has a declarator"
                    );
                    return d.concat(&[d.text(decl.kind.as_str()), d.text(" ")]);
                };
                // The keyword→binding gap carries a comment (`for (const /* c */ x of y)`)
                // that must not be dropped — see `build_for_decl_keyword_gap`. Covers
                // `const`/`let`/`var`/`using`/`await using` uniformly.
                let id_doc = self.build_expression_doc(&declarator.id);
                self.build_for_decl_keyword_gap(decl, declarator.span.start, id_doc)
            }
            // `for ((async) of x)` keeps parens around the bare `async` identifier
            // (the caller decides via `wrap_async_paren` — a non-await for-of, where
            // bare `for (async of x)` is a syntax error).
            internal::ForInOfLeft::Pattern(expr) => {
                let doc = self.build_expression_doc(expr);
                if wrap_async_paren { d.parens(doc) } else { doc }
            }
        }
    }

    /// Whether the for-in/for-of LHS is a bare `async` identifier that must be
    /// parenthesized: only in a **non-await for-of** (bare `for (async of x)` is a
    /// syntax error — the parser can't tell it from `for (async ... )`). Mirrors
    /// prettier's identifier rule (parentheses/identifier.js:
    /// `name === "async" && !parent.await && parent.type === "ForOfStatement"`).
    fn for_lhs_needs_async_paren(
        &self,
        left: &internal::ForInOfLeft<'_>,
        keyword: &str,
        is_await: bool,
    ) -> bool {
        keyword == "of"
            && !is_await
            && matches!(
                left,
                internal::ForInOfLeft::Pattern(Expression::Identifier(id))
                    if self.with_ident_name(id, |s| s == "async")
            )
    }

    /// Get the end position of a ForInit
    fn get_for_init_span_end(&self, init: &internal::ForInit<'_>) -> u32 {
        match init {
            internal::ForInit::VariableDeclaration(decl) => decl.span.end,
            internal::ForInit::Expression(expr) => expr.span().end,
        }
    }
}
