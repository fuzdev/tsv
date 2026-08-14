// Loop statement printing: for, for-in, for-of
//
// for-loop header layout (init/test/update clauses with comment placement),
// for-in/for-of left/right printing.

use crate::ast::internal::{self, Expression, Statement};
use crate::printer::layout::hang_after_operator;
use crate::printer::{CommentVec, LeadingGlue, OwnedCommentEffect, ParenContext, Printer};
use smallvec::smallvec;
use tsv_lang::Comment;
use tsv_lang::Span;
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
    update_end: Option<u32>,
    first_semi: Option<u32>,
    second_semi: Option<u32>,
    close_paren: Option<u32>,
}

impl ForHeaderSpans {
    /// The end of the init clause's comment region: the clause itself when present,
    /// else its `;`.
    ///
    /// The three region accessors are what keep an ABSENT clause's comments from
    /// leaking into the next clause's region. A boundary chained over the absent
    /// clauses instead (`init_start.or(test_start).or(update_start)`) reaches past the
    /// separators that delimit the slots, so one comment landed in two regions and was
    /// printed twice — or, where only the skipped-over region had an emitter, in none
    /// and was dropped. The `;` positions are the slot boundaries; every region is
    /// bounded by them.
    fn init_region_end(&self) -> Option<u32> {
        self.init_start.or(self.first_semi)
    }

    /// The start of the init clause's comment region: just inside the `(`.
    fn init_region_start(&self) -> Option<u32> {
        self.open_paren.map(|p| p + 1)
    }

    /// The start of the test clause's comment region: the preceding clause's end when
    /// there is one — its `content`→`;` gap belongs to it — else just past that `;`.
    fn test_region_start(&self) -> Option<u32> {
        self.init_end.or_else(|| self.first_semi.map(|s| s + 1))
    }

    /// The start of the update clause's comment region. See
    /// [`Self::test_region_start`].
    fn update_region_start(&self) -> Option<u32> {
        self.test_end.or_else(|| self.second_semi.map(|s| s + 1))
    }

    /// The start of the region that runs to the header's `)` — just past the update
    /// clause when there is one, else its whole (empty) slot. Unlike the init and test
    /// slots, nothing but `)` terminates this one, so the clause and the gap after it
    /// share a single trailing region.
    fn update_trailing_start(&self) -> Option<u32> {
        self.update_end.or_else(|| self.second_semi.map(|s| s + 1))
    }

    /// The end of the test clause's comment region: the clause itself when present,
    /// else its `;`. See [`Self::init_region_end`].
    fn test_region_end(&self) -> Option<u32> {
        self.test_start.or(self.second_semi)
    }

    /// The end of the update clause's comment region: the clause itself when present,
    /// else the header's `)`. See [`Self::init_region_end`].
    fn update_region_end(&self) -> Option<u32> {
        self.update_start.or(self.close_paren)
    }
}

/// The C-style `for` header's own parens, located once per statement.
///
/// [`Printer::matching_close_paren`] is a depth-tracked scan across the whole header,
/// so locating the pair once and threading it is the difference between one such scan
/// per `for` and three — the header-end probe, the header builder, and the empty-header
/// builder each used to redo it. Every consumer has to agree on the same pair anyway:
/// the `)` is what bounds the header's comment regions, so two callers disagreeing
/// about it would bound them differently.
///
/// Both fields are `Option` because a degenerate header may have no locatable paren;
/// each consumer already carries its own fallback for that.
#[derive(Clone, Copy)]
struct ForParens {
    open: Option<u32>,
    close: Option<u32>,
}

/// Source positions for a for-in/for-of header
///
/// Groups the resolved header positions to avoid passing many `u32` parameters
/// to the structural-comment check and the two layout builders. Mirrors
/// `ForHeaderSpans` (the C-style `for` header). Built once by
/// `Printer::for_in_of_spans`.
struct ForInOfSpans {
    open_paren: Option<u32>,
    close_paren: Option<u32>,
    left_start: u32,
    /// Start of the binding *target*, skipping the declaration keyword (the
    /// first declarator's pattern for a `VariableDeclaration` left, the pattern
    /// itself for a bare `Pattern` left) — so the keyword→binding gap
    /// (`const // c⏎x`) reads as header-structural while the binding pattern's
    /// own interior does not. See `get_for_in_of_binding_start`.
    binding_start: u32,
    left_end: u32,
    keyword_pos: u32,
    keyword_end: u32,
    right_start: u32,
    right_end: u32,
}

impl ForInOfSpans {
    /// The closing-paren anchor for the iterable→`)` gap scan, falling back to
    /// just past the iterable when no `)` is locatable. Kept in one place so
    /// every caller derives it identically.
    fn close(&self) -> u32 {
        self.close_paren.unwrap_or(self.right_end + 1)
    }
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
        let body_doc = self.build_statement_head_doc(paren_end, body.span(), || {
            self.build_statement_doc(body, false)
        });

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

        let (inline_prev, own_line) =
            self.partition_comments_trailing_vs_own_line(paren_end, body_start);

        parts.push(d.text(")"));

        if self.header_to_body_gap_breaks(paren_end, body_start) {
            // Emit trailing comments on the `)` line
            for comment in &inline_prev {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }

            // Everything not trailing the `)` goes indented before the body.
            //
            // Separator-BEFORE form: the separator ahead of each comment is keyed on the
            // PREVIOUS comment, and the one before the body on the last — identical to
            // keying each separator on the comment it follows, but it leaves a seam for
            // the blank line an author put between two own-line comments
            // (`push_gap_blank_before`, the one place that rule lives).
            //
            // The separator preserves the authored line: what the author wrote on one
            // line stays on one line. A line comment is the one override — it must end
            // its line or the `//` swallows the next comment / the body. That rule
            // applies both between comments and before the body, so it is written once.
            let sep_after = |p: &Comment| {
                if self.comment_hugs_next(p) {
                    d.text(" ")
                } else {
                    d.hardline()
                }
            };
            let mut inner = DocBuf::new();
            let mut prev: Option<&Comment> = None;
            for comment in own_line {
                match prev {
                    None => inner.push(d.hardline()),
                    Some(p) => {
                        self.push_gap_blank_before(
                            &mut inner,
                            Some(p.span.end),
                            comment.span.start,
                        );
                        inner.push(sep_after(p));
                    }
                }
                inner.push(self.build_comment_doc(comment));
                prev = Some(comment);
            }
            inner.push(prev.map_or_else(|| d.hardline(), sep_after));
            inner.push(body_doc);
            parts.push(d.indent(d.concat(&inner)));
        } else {
            // Nothing forces a break, so by `header_to_body_gap_breaks` every comment is
            // a block comment already trailing `)` — `own_line` and `inline_next` are
            // empty here and iterating them would be dead. adjustClause: `) /* a */ body`
            // stays flat, but the comment(s) + body drop to their own indented line when
            // the enclosing for-in/for-of group breaks (overflow). Matches Prettier.
            let mut inner = DocBuf::new();
            self.push_glued_comment_run(&mut inner, &inline_prev);
            inner.push(body_doc);
            parts.push(d.indent_line(d.concat(&inner)));
        }
    }

    /// Locate a C-style `for` header's parens. See [`ForParens`] — call this once per
    /// statement and thread the result.
    fn for_parens(&self, stmt_start: u32) -> ForParens {
        let open = self.find_open_paren_after(stmt_start);
        ForParens {
            open,
            close: open.and_then(|p| self.matching_close_paren(p)),
        }
    }

    /// Build a complete for statement doc including the body
    ///
    /// This includes the body in the doc so the width calculation accounts for ` {`.
    fn build_for_statement_with_body_doc(
        &self,
        stmt: &internal::ForStatement<'_>,
        parens: ForParens,
    ) -> DocId {
        let d = self.d();
        let header_doc = self.build_for_header_doc(stmt, parens, None);
        if matches!(stmt.body, Statement::EmptyStatement(_)) {
            // No space before empty statement: `for (...);`
            d.concat(&[header_doc, self.build_statement_doc(stmt.body, false)])
        } else if let Statement::BlockStatement(block) = stmt.body {
            // Block body: `for (...) { ... }`
            // Note: Unlike for-in/for-of, standard for loops keep empty blocks inline `{}`
            d.concat(&[
                header_doc,
                d.text(" "),
                self.build_statement_head_doc(
                    self.get_for_header_end(stmt, parens),
                    block.span,
                    || self.build_block_statement_doc(block),
                ),
            ])
        } else {
            // Non-block body. Mirror Prettier's `adjustClause`: the body is
            // `indent([line, body])` wrapped with the header in an outer group.
            // Flat → `for (...) stmt;`. When the header force-breaks (a comment
            // hardline propagates via `will_break`) or the whole thing overflows,
            // the outer group breaks and the body drops to its own indented line;
            // the inner header group still decides its own flat/break, so a
            // width-only overflow keeps the header flat (matching Prettier).
            let body_doc = self.build_statement_head_doc(
                self.get_for_header_end(stmt, parens),
                stmt.body.span(),
                || self.build_statement_doc(stmt.body, false),
            );
            d.group(d.concat(&[header_doc, d.indent_line(body_doc)]))
        }
    }

    /// Get the end position of a for loop header (position after the closing paren)
    ///
    /// `parens.close` is depth-tracked, so redundant parens or parens inside a clause
    /// don't yield a premature match; the last clause's end is the fallback when the
    /// header has no locatable `)`.
    fn get_for_header_end(&self, stmt: &internal::ForStatement<'_>, parens: ForParens) -> u32 {
        // `map_or_else` so the located-paren path — every well-formed header — costs one map
        // and never walks the clause spans for a fallback it discards.
        parens.close.map_or_else(
            || {
                stmt.update
                    .as_ref()
                    .map(|u| u.span().end)
                    .or_else(|| stmt.test.as_ref().map(|t| t.span().end))
                    .or_else(|| stmt.init.as_ref().map(|i| self.get_for_init_span_end(i)))
                    .unwrap_or(stmt.span.start + "for ".len() as u32)
            },
            |p| p + 1,
        )
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
    ///
    /// A *partially*-empty header preserves its empty slots' comments too, but through
    /// the ordinary path's [`Self::push_for_empty_slot_comments`] and with the ordinary
    /// separators — a slot there sits between real clauses, so it takes the same
    /// `softline`/`line` breaks they do. This dedicated emitter exists for the
    /// separator rules above, which only make sense when the parens hold nothing else.
    fn build_for_empty_with_comments(
        &self,
        stmt: &internal::ForStatement<'_>,
        parens: ForParens,
        for_open: DocId,
    ) -> DocId {
        let d = self.d();
        let (Some(open), Some(close)) = (parens.open, parens.close) else {
            return d.concat(&[for_open, d.text(";;)")]);
        };
        let (Some(s1), Some(s2)) = self.find_for_semicolons(stmt, open, Some(close)) else {
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
    ///
    /// `keyword_comments` is any `for`→`(` gap comment, already built by the caller;
    /// `parens` is the header's paren pair, located once per statement ([`ForParens`]).
    fn build_for_header_doc(
        &self,
        stmt: &internal::ForStatement<'_>,
        parens: ForParens,
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

        let ForParens {
            open: open_paren,
            close: close_paren_approx,
        } = parens;
        // The whole paren interior, for the two header-wide gates below. `None` for a
        // degenerate header with no locatable parens, where neither gate has a range to
        // ask about.
        let paren_interior = open_paren.zip(close_paren_approx);

        // Check if there are any comments inside the for parens
        let has_comments_inside = paren_interior
            .is_some_and(|(open, close)| self.has_comments_to_emit_between(open, close));

        if !has_any && !has_comments_inside {
            // Empty for (;;) with no comments - no wrapping needed
            return d.concat(&[for_open, d.text(";;)")]);
        }

        if !has_any && has_comments_inside {
            // Empty for (;;) with comments — preserve them inline where authored
            // (divergence from prettier; see empty_clauses*_comment_prettier_divergence).
            return self.build_for_empty_with_comments(stmt, parens, for_open);
        }

        // Determine spans for each part
        let (init_end, test_end) = self.for_clause_ends(stmt);
        let update_end = stmt.update.as_ref().map(|u| u.span().end);

        // Find semicolon positions for proper comment boundary detection.
        // The semicolons in `for (init; test; update)` are at specific positions in
        // source. Anchored at the clause ends, not `stmt.span.start` — see
        // `find_for_semicolons`. `open_paren` is only the fallback for an absent init,
        // so a header with no open paren to find (already degenerate) keeps the
        // previous keyword-relative behavior.
        let (first_semi, second_semi) = self.find_for_semicolons(
            stmt,
            open_paren.unwrap_or(stmt.span.start),
            close_paren_approx,
        );

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
            update_end,
            first_semi,
            second_semi,
            // The statement's own paren pair, threaded in rather than rescanned — see
            // [`ForParens`].
            close_paren: close_paren_approx,
        };

        // Where each clause's leading run emits from. Resolved once, so the freeze gate
        // below and the emitter further down cannot read different gaps — the directive
        // that freezes a clause has to be exactly the one printed above it.
        let test_search_start =
            self.for_clause_search_start(stmt.span.start, open_paren, first_semi, init_end);
        let update_search_start = self.for_clause_search_start(
            stmt.span.start,
            open_paren,
            second_semi,
            test_end.or(init_end),
        );

        // Rule: an own-line directive in a clause's leading gap freezes that WHOLE clause
        // ([`Printer::value_head_frozen_span`]). The slice is the clause's node span, so
        // the header's `;` stays parent-owned — prettier's slice swallows it and emits a
        // header that no longer parses
        // (`init_declaration_prettier_ignore_head_prettier_divergence`).
        // Each clause is absent-able independently of its gap, so the clause span is the
        // `Option` and the gap is a plain offset.
        let clause_frozen = |gap: u32, start: Option<u32>, end: Option<u32>| {
            start
                .zip(end)
                .and_then(|(start, end)| self.value_head_frozen_span(gap, Span::new(start, end)))
        };
        // Init's gap is itself absent when there is no `(` to open one.
        let init_frozen = spans
            .init_region_start()
            .and_then(|gap| clause_frozen(gap, spans.init_start, init_end));
        let test_frozen = clause_frozen(test_search_start, spans.test_start, test_end);
        let update_frozen = clause_frozen(update_search_start, spans.update_start, update_end);

        // Check if we have any own-line comments that force expansion. A line
        // comment anywhere in the header also forces it: the `//` runs to end of
        // line, so the clauses after it must move to their own lines (matching
        // prettier) — otherwise the comment swallows the rest of the header.
        let has_line_comment_in_header = paren_interior
            .is_some_and(|(open, close)| self.has_line_comments_between(open + 1, close));
        let has_own_line_comments =
            has_line_comment_in_header || self.for_header_has_own_line_comments(&spans);

        let mut inner_parts = DocBuf::new();

        // Every clause is laid out the same way: its region's comments, then the
        // clause (or nothing, when the slot is empty), then its `;`. Each region is
        // bounded by the header's own separators (`ForHeaderSpans::*_region_end`), so
        // exactly one emitter owns each comment.

        // Init clause. Its separator is the header's own `softline` — `for (` and the
        // first clause together when the header fits, the clause on its own line when
        // it breaks — where the later clauses take the `line` that follows their `;`.
        if let (Some(start), Some(init_start)) = (spans.init_region_start(), spans.init_start) {
            self.push_for_clause_leading_section(
                &mut inner_parts,
                start,
                init_start,
                None,
                d.softline(),
            );
        } else {
            inner_parts.push(d.softline());
            if let (Some(start), Some(region_end)) =
                (spans.init_region_start(), spans.init_region_end())
                && self.push_for_empty_slot_comments(&mut inner_parts, start, region_end)
            {
                // The `;` that terminates the slot starts a fresh line (or is
                // space-separated when the header fits).
                inner_parts.push(d.line());
            }
        }
        if let Some(init) = &stmt.init {
            inner_parts.push(init_frozen.map_or_else(
                || self.build_for_init_doc(init),
                |frozen| self.build_frozen_node_doc(frozen),
            ));
        }
        // The init clause→`;` gap comments bind to the `;` like a list separator.
        self.push_for_clause_semicolon(&mut inner_parts, init_end, first_semi);

        // Test clause, same shape as init.
        if let Some(start) = spans.test_start {
            // Inline comments after init (after the `;`, on the same line as init)
            if let (Some(semi), Some(end)) = (first_semi, init_end) {
                self.push_for_clause_same_line_comments(&mut inner_parts, semi + 1, start, end);
            }
            self.push_for_clause_leading_section(
                &mut inner_parts,
                test_search_start,
                start,
                init_end,
                d.line(),
            );
        } else {
            // The post-`;` separator is emitted even with nothing to separate, so the
            // header isn't collapsed to `;;` — prettier keeps it whenever the header
            // isn't fully empty (`for (x = 0; ;)`, not `for (x = 0;;)`; the fully-empty
            // `for (;;)` returned above).
            inner_parts.push(d.line());
            if let (Some(semi), Some(region_end)) = (first_semi, spans.test_region_end())
                && self.push_for_empty_slot_comments(&mut inner_parts, semi + 1, region_end)
            {
                inner_parts.push(d.line());
            }
        }

        if let Some(test) = &stmt.test {
            // Wrap in group so binary chains (Ungrouped mode) have a tight parent
            // to evaluate fit against — matching how if/while use build_condition_group.
            // Without this, logical operators break with the for-header group (too wide)
            // instead of their own condition width.
            inner_parts.push(test_frozen.map_or_else(
                || d.group(self.build_condition_doc(test)),
                // The clarity parens an assignment test prints (`for (; (a = b); )`) are
                // the printer's, not the author's, so they wrap the frozen slice instead
                // of riding inside it — the same `StatementTest` shell, and the same
                // context, the unfrozen clause above applies.
                |frozen| self.build_frozen_value_doc(test, frozen, ParenContext::StatementTest),
            ));
        }
        // The test clause→`;` gap comments bind to the `;` like a list separator.
        self.push_for_clause_semicolon(&mut inner_parts, test_end, second_semi);

        // Update clause. Unlike init and test, nothing terminates its region but the
        // header's own closing `softline`/`hardline` — so the clause and the gap after
        // it share one trailing region (`push_for_update_trailing_comments`, below),
        // which is also the whole slot when the clause is absent.
        if let Some(start) = spans.update_start {
            // Inline comments after test (after the `;`, on the same line as test)
            if let (Some(semi), Some(end)) = (second_semi, test_end) {
                self.push_for_clause_same_line_comments(&mut inner_parts, semi + 1, start, end);
            }
            self.push_for_clause_leading_section(
                &mut inner_parts,
                update_search_start,
                start,
                test_end,
                d.line(),
            );
        }

        if let Some(update) = &stmt.update {
            inner_parts.push(update_frozen.map_or_else(
                || self.build_for_update_doc(update),
                |frozen| self.build_frozen_expression_doc(update, frozen),
            ));
        }
        if let Some(start) = spans.update_trailing_start() {
            self.push_for_update_trailing_comments(
                &mut inner_parts,
                start,
                spans.close_paren.unwrap_or(stmt.span.end),
                update_end,
            );
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

    /// Check if for header has any own-line comments that force expansion
    ///
    /// One check per clause region, each bounded by the header's separators exactly as
    /// the emitters are (`ForHeaderSpans::*_region_end`) — so a comment in an EMPTY
    /// slot is seen here too, and the header it forces open is the one that prints it.
    /// A region starts at the preceding clause's end when there is one (its `content`→`;`
    /// gap belongs to it) and just past the preceding `;` otherwise.
    fn for_header_has_own_line_comments(&self, spans: &ForHeaderSpans) -> bool {
        let region = |start: Option<u32>, end: Option<u32>| {
            start
                .zip(end)
                .is_some_and(|(start, end)| self.has_isolated_comment_between(start, end))
        };

        region(spans.init_region_start(), spans.init_region_end())
            || region(spans.test_region_start(), spans.test_region_end())
            || region(spans.update_region_start(), spans.update_region_end())
            // The update→`)` gap, when an update clause splits it off from the region
            // above. Scanned separately rather than by widening that region's end to
            // `)`, so the update expression's own interior comments stay the
            // expression printer's business.
            || region(spans.update_end, spans.close_paren)
    }

    /// Emit a for-header clause terminator `;` with its content→`;` gap comments
    /// bound to the `;` **like a list separator** (`split_separator_gap_comments`,
    /// `block_after_separator: false`): a same-line block stays before the `;`
    /// (`a /* c */;`), a same-line line trails it via `line_suffix` (`a; // c`),
    /// and an own-line comment defers **after** the `;`, opening on the header's own
    /// `line` rather than on a break of its own
    /// ([`Printer::split_for_header_gap_comments`] — the header decides its own width,
    /// so `x = 0⏎/* c1 */ /* c2 */;⏎b` collapses back onto one line the way prettier
    /// prints it). A blank
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
            (Some(start), Some(sep)) => self.split_for_header_gap_comments(parts, start, sep),
            _ => DocBuf::new(),
        };
        parts.push(d.text(";"));
        parts.extend(after);
    }

    /// Find the two `;` separators in a for-header. Returns `(first_semi,
    /// second_semi)`; the second is only sought once the first is found.
    ///
    /// Each scan is anchored at the **end of the clause its separator follows**, taken
    /// from the AST — never at the `for` keyword or the open paren. That is what makes
    /// first-match correct: from a clause's own end, only trivia and closing parens can
    /// precede the `;`, so the first hit is the separator. A scan anchored ahead of the
    /// clause instead walks *through* its source, and `TriviaProfile::JS` skips comments
    /// and strings but tracks **no brace depth** — so a `;` inside a nested block
    /// (`for (let f = () => { a(); b(); } /* c */; …)`) reads as the header separator,
    /// mis-binding that clause's trailing comments. It double-printed one. The anchor
    /// does this work, not the profile; widening the profile would not help.
    ///
    /// A clause that is absent falls back to just past the preceding delimiter, where
    /// the same adjacency holds (nothing but trivia sits between them).
    ///
    /// The search is bounded by `close_paren`, so a separator can never be picked up from
    /// a *later statement*: both separators are inside the header by definition, and a
    /// header missing one is degenerate, which every caller already handles. That also
    /// keeps the scan O(header) rather than O(rest of file).
    fn find_for_semicolons(
        &self,
        stmt: &internal::ForStatement<'_>,
        open_paren: u32,
        close_paren: Option<u32>,
    ) -> (Option<u32>, Option<u32>) {
        let bytes = self.source.as_bytes();
        let end = close_paren.map_or(bytes.len(), |c| c as usize);
        let (init_end, test_end) = self.for_clause_ends(stmt);
        let scan = |from: u32| {
            find_char(bytes, from as usize, end, b';', TriviaProfile::JS).map(|p| p as u32)
        };
        let first_semi = scan(init_end.unwrap_or(open_paren + 1));
        let second_semi = first_semi.and_then(|first| scan(test_end.unwrap_or(first + 1)));
        (first_semi, second_semi)
    }

    /// The end positions of the `init` and `test` clauses — the AST anchors the header's
    /// `;` separators are located from (see [`Self::find_for_semicolons`]). Derived in one
    /// place so the two scan call sites and the `ForHeaderSpans` build cannot disagree.
    fn for_clause_ends(&self, stmt: &internal::ForStatement<'_>) -> (Option<u32>, Option<u32>) {
        (
            stmt.init.as_ref().map(|i| self.get_for_init_span_end(i)),
            stmt.test.as_ref().map(|t| t.span().end),
        )
    }

    /// Resolve where to start searching for a for-clause's leading comments: just
    /// past the preceding `;` if present, else the previous clause's end, else just
    /// inside the open paren (or past `for (` when the paren is unknown).
    ///
    /// The `;`-first preference is the deliberate mirror of
    /// [`ForHeaderSpans::test_region_start`], which prefers the previous clause's
    /// *end*. Two starts because two questions: a comment before the `;` belongs to the
    /// separator gap and is emitted by [`Self::push_for_clause_semicolon`], so the
    /// leading run must start *after* it or print it twice — while the break scan must
    /// see it, since it is what forces the header open.
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

    /// Emit the lead-in before a for-clause: the leading-comment run, then any comment
    /// hugging the clause on its own line (`for (let i = 0; /* before test */ i < 10;
    /// …)`) — or just `separator` when the gap is empty.
    ///
    /// `separator` is what follows the preceding token when no comment takes that
    /// place — the `line` after the `;` for the test and update clauses, the header's
    /// opening `softline` for init.
    ///
    /// One pass **partitions** the gap, so every comment is printed exactly once and no
    /// comment can fall between two filters that must agree: what shares the *previous*
    /// clause's line was already emitted by
    /// [`Self::push_for_clause_same_line_comments`], a **block** sharing the clause's
    /// line hugs it, and everything else is the leading run. The run is the default arm
    /// on purpose — a `//` normally can't share a line with what follows it, but the
    /// line-break table records only `\n` while the lexer also ends a `//` at a lone
    /// `\r`, so `// c\r i < 10` does read as same-line. Landing that in the hug arm
    /// would emit `// c i < 10` and swallow the clause; the run gives it its own line.
    ///
    /// **No separator here is the run's own**: the header's `separator` leads it and the
    /// shared [`Printer::push_leading_comment_run`] supplies every break inside it and
    /// the one before the clause, so this gap follows the same rules as every other
    /// leading run — an author blank line between two comments survives, and a run glued
    /// onto one line stays glued. Hand-rolling a `hardline` per comment lost both.
    ///
    /// ⚠️ **And the site must not pre-empt the header's width decision either.** Asking
    /// [`Printer::is_own_line_comment`] of the run's FIRST comment and pushing a
    /// `hardline` for it reads as preserving the authored line and is the one-sided
    /// reading: in `x = 0;⏎/* c1 */ /* c2 */⏎b` only `c1` has a newline before it and
    /// only `c2` has one after, so neither owns a line, and prettier keeps the whole
    /// header flat (`docs/comments.md` §Own-line-ness is a SOURCE question). A run whose
    /// first comment IS isolated needs no help from here: it takes the emitter's
    /// `hardline`, which opens the header through `DocArena::will_break` and breaks this
    /// `separator` with it — so the two arms rendered identically wherever the pre-empt
    /// was right, and differed only where it was wrong.
    ///
    /// `search_start` - where to start looking for comments
    /// `clause_start` - start of the next clause
    /// `prev_end` - end of the previous expression (whose own trailing comments are
    /// already emitted); `None` for the init clause, which no expression precedes
    fn push_for_clause_leading_section(
        &self,
        parts: &mut DocBuf,
        search_start: u32,
        clause_start: u32,
        prev_end: Option<u32>,
        separator: DocId,
    ) {
        let d = self.d();
        // A comment trailing the previous clause on its line belongs to that clause, not
        // to this run. What remains splits at the clause the way every gap does: the
        // glued suffix leads it inline, the rest take their own lines.
        let (run, hug) = self.split_glued_comments(
            comments_to_emit_in_range(self.comments, search_start, clause_start)
                .filter(|c| !prev_end.is_some_and(|pe| self.is_same_line(pe, c.span.start))),
        );

        parts.push(separator);
        self.push_leading_comment_run(
            parts,
            run.iter().copied(),
            clause_start,
            LeadingGlue::Adjacent,
            d.empty(),
        );
        self.push_glued_comment_run(parts, &hug);
    }

    /// Emit the comments an **absent** clause's slot holds, joined by `line`, and
    /// report whether any were emitted.
    ///
    /// An empty slot has no other emitter — the clause whose leading run would print
    /// them isn't there — so without this the comments are dropped. They stay in the
    /// slot the author wrote them in, which is what the fully-empty header already
    /// does (`build_for_empty_with_comments`) and a divergence from prettier, which
    /// relocates them into the next clause or out of the header entirely (see the
    /// `empty_slot_comment_prettier_divergence` fixture).
    ///
    /// Only the init and test slots come through here — the ones a `;` terminates. The
    /// caller supplies the separators around the run: the header's leading `softline`
    /// or the `line` after the preceding `;` before it, and a `line` after it so a `//`
    /// can't swallow that `;`. The update slot has no terminator of its own and shares
    /// the update clause's trailing region instead
    /// ([`Self::push_for_update_trailing_comments`]).
    fn push_for_empty_slot_comments(&self, parts: &mut DocBuf, start: u32, end: u32) -> bool {
        let d = self.d();
        let mut emitted = false;
        for comment in comments_to_emit_in_range(self.comments, start, end) {
            if emitted {
                parts.push(d.line());
            }
            parts.push(self.build_comment_doc(comment));
            emitted = true;
        }
        emitted
    }

    /// Emit the update slot's trailing region — the update→`)` gap, and the whole
    /// (empty) slot when there is no update clause. One region either way, since `)` is
    /// all that closes it.
    ///
    /// The separator goes *before* each comment and none after: a comment sharing the
    /// update's line trails it, every other one takes its own line, and the header's
    /// own closing `softline`/`hardline` terminates the last. That is also why an
    /// absent update clause with nothing to say adds no separator at all — prettier 3.9
    /// (#19188) dropped the space it used to put before `)` → `for (…; cond;)`, not
    /// `for (…; cond; )`.
    ///
    /// Only the trailing form used to be emitted, so an own-line comment here was
    /// dropped.
    fn push_for_update_trailing_comments(
        &self,
        parts: &mut DocBuf,
        start: u32,
        close: u32,
        update_end: Option<u32>,
    ) {
        let d = self.d();
        for comment in comments_to_emit_in_range(self.comments, start, close) {
            if update_end.is_some_and(|end| self.is_same_line(end, comment.span.start)) {
                parts.push(d.text(" "));
            } else {
                parts.push(d.line());
            }
            parts.push(self.build_comment_doc(comment));
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
            let frozen = if i > 0 {
                let prev_end = seq.expressions[i - 1].span().end;
                self.push_for_clause_comma_gap(&mut docs, prev_end, e.span().start);
                // Rule A, the same as the general sequence printer's: an own-line
                // directive in the comma gap freezes the FOLLOWING operand. The `[~In]`
                // wrap `build_elem` may apply is moot on a verbatim slice.
                self.gap_frozen_span(prev_end, e.span())
            } else {
                None
            };
            docs.push(frozen.map_or_else(
                || build_elem(e),
                |frozen| self.build_frozen_expression_doc(e, frozen),
            ));
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
        let spans = self.for_in_of_spans(left, right, keyword, stmt_start);

        // The keyword as a static literal (`d.text` needs `&'static str`), with
        // and without the leading space.
        let (kw, kw_spaced) = if keyword == "of" {
            ("of", " of")
        } else {
            ("in", " in")
        };

        // Preserve comments between keywords and `(`
        // for await: two gaps — for-to-await and await-to-paren
        // for (non-await): one gap — for-to-paren
        let for_keyword_end = stmt_start + "for".len() as u32;
        let (for_await_comments, await_paren_comments) = if is_await {
            let await_pos = self.find_keyword_in_range(for_keyword_end, spans.left_start, "await");
            // Same line-safe builder as the `await`→`(` gap below: a line comment in
            // the `for`→`await` gap breaks `await` onto the next line so the `//`
            // can't swallow it; a block comment keeps its glue space.
            let for_await_c = await_pos
                .and_then(|ap| self.build_keyword_paren_comments(for_keyword_end, Some(ap)));
            let await_paren_c = await_pos
                .map(|ap| ap + "await".len() as u32)
                .and_then(|ae| self.build_keyword_paren_comments(ae, spans.open_paren));
            (for_await_c, await_paren_c)
        } else {
            (None, None)
        };
        let keyword_comments = if is_await {
            None
        } else {
            self.build_keyword_paren_comments(for_keyword_end, spans.open_paren)
        };

        // Check for a comment in the header's *structural gaps* that needs its own line
        // — if present, use the breaking layout that preserves it where the author
        // placed it. Only gap comments count: a `//` *inside* the binding or the
        // iterable expression is that expression's own content (printed by its doc) and
        // must NOT force the header open — prettier keeps the head inline and only the
        // iterable breaks. See `in_of_iterable_line_comment`.
        let breaks_for_gap_comment = self.for_in_of_header_gap_comment_forces_break(&spans);

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

        if breaks_for_gap_comment {
            let left_doc = self.build_for_in_of_left_doc(left, async_lhs_paren, &spans);
            return self.build_for_in_of_with_line_comments(
                right, body, keyword, &spans, left_doc, &mut parts,
            );
        }

        // Comments between ( and left
        if let Some(open) = spans.open_paren {
            for comment in comments_to_emit_in_range(self.comments, open + 1, spans.left_start) {
                if comment.is_block {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.text(" "));
                }
            }
        }

        parts.push(self.build_for_in_of_left_doc(left, async_lhs_paren, &spans));

        // Comments after left, before the keyword
        let has_left_comment =
            self.append_for_in_of_block_comments(&mut parts, spans.left_end, spans.keyword_pos);

        if has_left_comment {
            parts.push(d.text(kw));
        } else {
            parts.push(d.text(kw_spaced));
        }

        // Comments after the keyword, before right
        let has_comment =
            self.append_for_in_of_block_comments(&mut parts, spans.keyword_end, spans.right_start);
        if !has_comment {
            parts.push(d.text(" "));
        }

        parts.push(self.build_expression_doc(right));

        // Comments after right, before close paren (no trailing space needed)
        if let Some(close) = spans.close_paren {
            self.append_for_in_of_trailing_comments(&mut parts, spans.right_end, close);
        }

        // `)` + comments + body (shared with the breaking layout)
        self.push_for_close_paren_and_body(&mut parts, body, spans.right_end, spans.close_paren);

        // Group so a non-block body's `adjustClause` line breaks on overflow
        // (matches Prettier's `printForXStatement`).
        d.group(d.concat(&parts))
    }

    /// Resolve the for-in/for-of header's source positions into a `ForInOfSpans`,
    /// computed once and shared by the structural-line-comment check and both the
    /// inline and breaking layouts. Preserves each getter's semantics — notably
    /// `keyword_pos` falls back to `left_end` when the `in`/`of` keyword can't be
    /// located, and `close_paren` follows the depth-tracked open paren.
    fn for_in_of_spans(
        &self,
        left: &internal::ForInOfLeft<'_>,
        right: &Expression<'_>,
        keyword: &str,
        stmt_start: u32,
    ) -> ForInOfSpans {
        let left_end = self.get_for_in_of_left_end(left);
        let right_start = right.span().start;
        // Find the keyword position (search with or without spaces).
        let keyword_pos = self
            .find_keyword_position(left_end, right_start, keyword)
            .unwrap_or(left_end);
        let open_paren = self.find_open_paren_after(stmt_start);
        ForInOfSpans {
            open_paren,
            close_paren: open_paren.and_then(|o| self.matching_close_paren(o)),
            left_start: self.get_for_in_of_left_start(left),
            binding_start: self.get_for_in_of_binding_start(left),
            left_end,
            keyword_pos,
            keyword_end: keyword_pos + keyword.len() as u32,
            right_start,
            right_end: right.span().end,
        }
    }

    /// Whether a comment in one of the for-in/for-of header's *structural gaps* needs a
    /// line of its own, forcing the breaking layout — after `(` up to the binding target
    /// (covering the declaration keyword→binding gap, `const // c⏎x`), between the binding
    /// and the `in`/`of` keyword, between the keyword and the iterable, or between the
    /// iterable and `)`.
    ///
    /// Two kinds do. A `//` line comment, because inline it would swallow the header
    /// tokens after it (the reason below). And, in the `(`→binding region only, an
    /// **honored format-ignore directive** of either spelling, because its own-line
    /// placement is the thing that makes it honored: collapsed into the inline layout it
    /// becomes glued to the following token, hence inert, and the freeze it won on the
    /// first pass is lost on the second — an F1 non-idempotency. An ordinary block comment
    /// still rides inline.
    ///
    /// The directive half is **deliberately not** asked at the three later gaps. Their
    /// emitters TRAIL a block comment onto the preceding token (`const x /* c */⏎of`), so
    /// breaking the header there would not give the directive its own line anyway — it
    /// would only lose that line on the next pass, when the gate reads the trailing
    /// placement and collapses the header again. A break is worth forcing for a comment
    /// only where the broken layout actually gives that comment a line of its own.
    ///
    /// Deliberately excludes the interiors of the binding *pattern*
    /// (`[binding_start, left_end)`) and the iterable (`[right_start, right_end)`): a
    /// line comment *inside* the iterable (`for (const x of [\n\t'a' // c\n])`) — or
    /// inside a destructuring binding — is that expression's own content, printed by
    /// its doc, and must not force the header-breaking layout
    /// (`build_for_in_of_with_line_comments`). Prettier keeps the head inline and
    /// only that expression breaks. A gap comment, by contrast, has no other line
    /// break to ride, so inline the `//` would swallow the following header tokens —
    /// tsv breaks the header and preserves it in place. See the
    /// `in_of_iterable_line_comment` (excluded) and
    /// `of_in_keyword_binding_line_comment_prettier_divergence` (keyword→binding gap,
    /// included) fixtures.
    fn for_in_of_header_gap_comment_forces_break(&self, spans: &ForInOfSpans) -> bool {
        // `(` → binding target (spans the declaration keyword + its keyword→binding
        // gap). Absent open paren (degenerate header) has no locatable gap. The one
        // region whose broken layout puts a comment on its own line, so the one that
        // also asks about an honored directive — which makes it exactly the
        // declaration header's own gap question (`keyword_gap_breaks`), asked here
        // rather than re-spelled, so the two can never drift on what forces a break.
        spans
            .open_paren
            .is_some_and(|open| self.keyword_gap_breaks(open + 1, spans.binding_start))
            // binding → keyword
            || self.has_line_comments_between(spans.left_end, spans.keyword_pos)
            // keyword → iterable
            || self.has_line_comments_between(spans.keyword_end, spans.right_start)
            // iterable → `)`
            || self.has_line_comments_between(spans.right_end, spans.close())
    }

    /// Build for-in/for-of statement with line comments preserved in their positions
    ///
    /// This is our divergence from Prettier - we preserve line comments where
    /// the user wrote them rather than relocating them.
    fn build_for_in_of_with_line_comments(
        &self,
        right: &Expression<'_>,
        body: &Statement<'_>,
        keyword: &str, // "in" or "of"
        // Resolved header positions, computed once by the caller (see
        // `for_in_of_spans`) — shared with the structural-comment check and the
        // inline layout.
        spans: &ForInOfSpans,
        // The LHS doc (`const y`, `(async)`, …), prebuilt by the caller so the
        // bare-`async` paren decision (`for_lhs_needs_async_paren`, which needs the
        // `is_await` flag this method doesn't carry) stays on the caller's side.
        left_doc: DocId,
        // The `for ... (` opening, prebuilt by the caller (comments preserved,
        // `await` from the AST) — shared with the inline layout. Filled in place
        // (a pooled buffer owned by the caller) rather than taken by value.
        parts: &mut DocBuf,
    ) -> DocId {
        let d = self.d();

        // Inner content with hardline breaks
        let mut inner = DocBuf::new();

        // Comments before left (after open paren)
        if let Some(open) = spans.open_paren {
            for comment in comments_to_emit_in_range(self.comments, open + 1, spans.left_start) {
                inner.push(d.hardline());
                inner.push(self.build_comment_doc(comment));
            }
        }

        // Left side (const y)
        inner.push(d.hardline());
        inner.push(left_doc);

        // Comments after left, before keyword — emit all (own-line comments normalize to inline)
        for comment in comments_to_emit_in_range(self.comments, spans.left_end, spans.keyword_pos) {
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
        for comment in
            comments_to_emit_in_range(self.comments, spans.keyword_end, spans.right_start)
        {
            keyword_parts.push(d.text(" "));
            keyword_parts.push(self.build_comment_doc(comment));
        }

        // Right side (items)
        keyword_parts.push(d.hardline());
        keyword_parts.push(self.build_expression_doc(right));

        // Comments after right, before close paren
        if let Some(close) = spans.close_paren {
            for comment in comments_to_emit_in_range(self.comments, spans.right_end, close) {
                keyword_parts.push(d.text(" "));
                keyword_parts.push(self.build_comment_doc(comment));
            }
        }

        inner.push(d.indent(d.concat(&keyword_parts)));

        parts.push(d.indent(d.concat(&inner)));
        parts.push(d.hardline());

        // `)` + comments + body (shared with the inline layout)
        self.push_for_close_paren_and_body(parts, body, spans.right_end, spans.close_paren);

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
            parts.push(self.build_statement_head_doc(paren_end, block.span, || {
                self.build_block_statement_expand_empty_doc(block)
            }));
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

    /// Get the start position of the for-in/for-of binding *target* — the first
    /// declarator's pattern for a `VariableDeclaration` left (`const [a, b]` → the
    /// `[`), or the pattern itself for a bare `Pattern` left. Unlike
    /// `get_for_in_of_left_start` (which points at the `const`/`let` keyword), this
    /// skips the declaration kind so the keyword→binding gap (`const // c⏎x`) reads
    /// as a header-structural position while the binding pattern's own interior does
    /// not — see `for_in_of_header_gap_comment_forces_break`.
    fn get_for_in_of_binding_start(&self, left: &internal::ForInOfLeft<'_>) -> u32 {
        match left {
            internal::ForInOfLeft::VariableDeclaration(decl) => decl
                .declarations
                .first()
                .map_or(decl.span.start, |declarator| declarator.id.span().start),
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
        // Located once here and threaded through every consumer — see `ForParens`.
        let parens = self.for_parens(stmt.span.start);
        let keyword_comments = self.build_keyword_paren_comments(for_keyword_end, parens.open);
        let has_pre_paren_comments = keyword_comments.is_some();

        // Check for comments between ) and body (Prettier 3.7 #18108)
        let header_end = self.get_for_header_end(stmt, parens);
        let body_start = stmt.body.span().start;

        if has_pre_paren_comments || self.has_comments_to_emit_between(header_end, body_start) {
            // Build parts with proper comment handling. A comment between `)` and the
            // body does NOT force the header to break — the header decides its own
            // flat/break on its own width (prettier 3.9 collapses `for (i; c; u)` and
            // trails the comment after `)`). Only comments *inside* the parens (handled
            // in `build_for_header_doc`) or overflow expand the header.
            let mut parts: DocBuf =
                smallvec![self.build_for_header_doc(stmt, parens, keyword_comments)];

            // Post-header comments. Non-block bodies use Prettier's `adjustClause`
            // (`indent([line, body])`) wrapped with the header in an outer group, so
            // the body drops to its own indented line when the header breaks (a
            // comment hardline propagates) or the whole thing overflows — while the
            // header group still decides its own flat/break.
            let is_block_body = matches!(stmt.body, Statement::BlockStatement(_));
            // A C-style `for` collapses its empty block body (`for (…) {}`) — unless an
            // own-line directive in the `)`→body gap freezes it.
            let body_doc = self.build_statement_head_doc(header_end, stmt.body.span(), || {
                self.build_collapsing_body_doc(stmt.body)
            });

            let gap_breaks = self.header_to_body_gap_breaks(header_end, body_start);
            let (tail, group_it) = if self.has_comments_to_emit_between(header_end, body_start) {
                if gap_breaks && !is_block_body {
                    // Non-block body, and something in the gap forces the break: the
                    // shared indented emitter — each own-line comment on its own
                    // indented line, then the body — break-safe so a `//` can't swallow
                    // the next comment or the body (Prettier's `adjustClause`).
                    let mut tail = DocBuf::new();
                    self.push_indented_header_to_body_gap(
                        &mut tail, header_end, body_start, body_doc,
                    );
                    (d.concat(&tail), false)
                } else if gap_breaks {
                    // Block body, and something in the gap forces the break — the shared
                    // header→body gap. The C-style `for` pushes no anchor of its own: its
                    // `)` is already inside the header doc. Given this branch's guard, the
                    // gap's separator is the hardline that drops the block to the next line.
                    let mut tail = DocBuf::new();
                    self.push_header_to_body_gap(&mut tail, header_end, body_start);
                    tail.push(body_doc);
                    (d.concat(&tail), false)
                } else {
                    // Nothing in the gap forces a break, so every comment is a block
                    // comment already trailing the `)` — built here so the breaking
                    // paths above don't compute a doc they discard.
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
            self.build_for_statement_with_body_doc(stmt, parens)
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
                // The keyword→first-declarator gap carries a comment (`for (let /* c */
                // i = 0; …)`) that must not be dropped — see `build_for_decl_keyword_gap`.
                // Built here rather than through that helper because the declarator loop
                // needs the keyword's end as its Rule A anchor for the first declarator.
                let first_decl_start = decl.declarations[0].span.start;
                let (keyword_doc, keyword_end) = self.build_keyword_words_doc(
                    decl.kind.words(),
                    decl.span.start,
                    first_decl_start,
                );
                let item_span = |j: usize| decl.declarations[j].span;

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
                    // Rule A over the init clause's declarators, anchored exactly as the
                    // statement-level list (`build_variable_declaration_doc`): an own-line
                    // directive in the keyword→first-declarator gap or between two
                    // declarators freezes the FOLLOWING one over its own node span.
                    if self.list_item_frozen(keyword_end, &item_span, i) {
                        decl_docs.push(self.build_frozen_node_doc(declarator.span));
                        continue;
                    }
                    let mut one: DocBuf = smallvec![self.build_expression_doc(&declarator.id)];
                    if let Some(init) = &declarator.init {
                        let id_end = declarator.id.span().end;
                        let init_start = init.span().start;
                        let eq_pos = self.find_equals_position(id_end, init_start);
                        // An init declarator's `=` is a value gap, exactly as the
                        // statement-level one is (`build_variable_declaration_doc`): an
                        // own-line JSDoc cast hangs after the `=` rather than printing its
                        // hardline mid-line, which had no fixed point (pass 1 stranded the
                        // `(`, pass 2 read the comment as mid-line and collapsed the whole
                        // header). Marked before any branch below builds the value; the
                        // flag is span-keyed, so it is read wherever that build lands.
                        self.mark_jsdoc_cast_value_gap(init);
                        // The binding→`=` gap, answered exactly as the statement-level
                        // declarator answers it (`build_variable_declaration_doc`): a
                        // line comment — or a multiline block the author broke after —
                        // keeps its place and drops `= value` to a continuation line
                        // indented one level; anything else stays inline before the `=`
                        // (`let a /* c */ = 0`). A gap this printer skips is a gap whose
                        // comment is dropped outright, which is what this one used to do
                        // for every comment kind in it.
                        let before_eq = self.has_comments_to_emit_between(id_end, eq_pos);
                        let continuation = before_eq
                            .then(|| {
                                self.build_initializer_line_continuation(id_end, eq_pos, || {
                                    let value = self
                                        .wrap_for_init_in(init, self.build_expression_doc(init));
                                    self.prepend_rhs_comments(value, eq_pos + 1, init_start)
                                })
                            })
                            .flatten();
                        if let Some(cont) = continuation {
                            one.push(cont);
                        } else {
                            if before_eq {
                                one.push(self.build_inline_comments_between_doc(id_end, eq_pos));
                            }
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
                            } else if self.owned_leading_comment_effect(init)
                                == Some(OwnedCommentEffect::Hangs)
                            {
                                // The owned half of the same question: a comment the value
                                // OWNS (an own-line JSDoc cast) is glued to its first token
                                // and travels inside its doc, so the gap probe above cannot
                                // see it — docs/comments.md hazard 2. It still ends the `=`
                                // line, so the value hangs, the layout the statement
                                // declarator's break-after-operator branch gives it. Printed
                                // flat, the cast's hardline landed mid-line and the authoring
                                // had NO fixed point: pass 2 read the comment as mid-line and
                                // collapsed the whole header.
                                one.push(d.text(" ="));
                                one.push(hang_after_operator(
                                    d,
                                    self.wrap_for_init_in(init, self.build_expression_doc(init)),
                                ));
                            } else {
                                one.push(d.text(" = "));
                                if let Some(comments) =
                                    self.build_rhs_comments_glued_opt(eq_pos + 1, init_start)
                                {
                                    one.push(comments);
                                }
                                one.push(
                                    self.wrap_for_init_in(init, self.build_expression_doc(init)),
                                );
                            }
                        }
                    }
                    decl_docs.push(d.concat(&one));
                }
                // Multiple declarators break on width: they stay on one line when the init
                // clause fits and drop onto their own lines (continuation indented one
                // level) when it doesn't — matching prettier's `printVariableDeclaration`. A
                // declarator whose `=` comment forces a break also breaks the group (its
                // hardline propagates).
                let body = if decl.declarations.len() > 1 {
                    d.indent(d.concat(&decl_docs))
                } else {
                    d.concat(&decl_docs)
                };
                let header = d.concat(&[
                    keyword_doc,
                    self.build_keyword_to_name_continuation(keyword_end, first_decl_start, body),
                ]);
                if decl.declarations.len() > 1 {
                    d.group(header)
                } else {
                    header
                }
            }
            internal::ForInit::Expression(expr) => {
                // Sequence expressions in for loop init don't need outer parens
                // e.g., `for (i = 0, j = 0; ...)` not `for ((i = 0, j = 0); ...)`.
                // Same dispatch as build_for_update_doc, but each operand is `[~In]`
                // wrapped (`wrap_for_init_in`).
                //
                // The init is a statement-head position for the `let [` lookahead
                // restriction too (`for ((let)[0] = 1; ;)`), so a `let` heading it keeps
                // its parens.
                self.with_expr_stmt_paren_target(self.let_bracket_head_target(expr), || {
                    self.build_for_expr_clause(expr, |e| {
                        self.wrap_for_init_in(e, self.build_expression_doc(e))
                    })
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

    /// The for-in/for-of header's LEFT clause.
    ///
    /// Rule: an own-line directive in the `(`→left gap freezes the clause WHOLE
    /// ([`Printer::value_head_frozen_span`]) — the same delimiter-owned value head as a
    /// `for` header's init. The slice is the clause's own span, so the `in`/`of` keyword
    /// and the iterable stay parent-owned and normalize; the `(async)` clarity parens are
    /// the printer's, so they wrap the frozen slice rather than ride inside it.
    ///
    /// Span-shaped for the same reason the init clause is: a left is a
    /// `VariableDeclaration` as often as a pattern, so the freeze is resolved and emitted
    /// once here rather than per arm below.
    fn build_for_in_of_left_doc(
        &self,
        left: &internal::ForInOfLeft<'_>,
        wrap_async_paren: bool,
        spans: &ForInOfSpans,
    ) -> DocId {
        let d = self.d();
        // A `let` heading the clause keeps its parens. Both head forms restrict it, and
        // prettier draws one line across them, so tsv does too: the for-of head's own
        // `[lookahead ∉ { let }]` makes a bare `for (let of x)` a syntax error, and a
        // for-in head — restricted only on `let [` — takes the same paren, matching
        // prettier's `startsWithNoLookaheadToken` clause, which finds ANY enclosing
        // for-in/of. Unlike `(async)`, the paren can belong to a node *inside* the clause
        // (`for ((let).a of x)`), so it is handed to that node rather than wrapped here.
        let let_target = match left {
            internal::ForInOfLeft::Pattern(expr) => self.for_in_of_let_head_target(expr),
            internal::ForInOfLeft::VariableDeclaration(_) => None,
        };
        if let Some(open) = spans.open_paren
            && let Some(frozen) =
                self.value_head_frozen_span(open + 1, Span::new(spans.left_start, spans.left_end))
        {
            // A frozen slice is verbatim, so there is no interior to hand the target to:
            // the parens go around the WHOLE slice, as the expression statement's
            // nested-target path does for the same reason. Only when the target IS the
            // whole slice, though — a `let` heading a member or call is not the leftmost
            // BYTE of its node, whose span opens at the author's own `(`
            // (`for ((let).a of x)` freezes as `(let).a`), and wrapping that again would
            // double the paren INSIDE an ignored region. `(async)` never hits this: an
            // identifier's span stops at the word, so the shell really is the printer's.
            let doc = self.build_frozen_node_doc(frozen);
            let wrap_let_paren = let_target.is_some_and(|target| target.start == frozen.start);
            return if wrap_async_paren || wrap_let_paren {
                d.parens(doc)
            } else {
                doc
            };
        }
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
                let doc = self
                    .with_expr_stmt_paren_target(let_target, || self.build_expression_doc(expr));
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
