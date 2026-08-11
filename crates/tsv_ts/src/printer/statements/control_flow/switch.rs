// switch statement printing
//
// Switch head, case labels, and case-body layout with comment handling.

use crate::ast::internal::{self, Statement};
use crate::printer::{
    CommentFilter, CommentSpacing, CommentVec, LeadingGlue, Printer, next_printed_stmt_start,
};
use smallvec::smallvec;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::{TriviaProfile, find_char};

impl<'a> Printer<'a> {
    /// Build a doc for a switch statement with proper line-width wrapping
    ///
    /// Matches Prettier's architecture: the discriminant wraps to multiple lines
    /// when the `switch (discriminant) {` line exceeds print width.
    pub(in crate::printer::statements) fn build_switch_statement_doc(
        &self,
        stmt: &internal::SwitchStatement<'_>,
    ) -> DocId {
        let d = self.d();
        // Find paren positions for comment handling
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));

        // Preserve comments between `switch` keyword and `(` in place:
        //   switch/* c */(a){} → switch /* c */ (a) {}
        let switch_keyword_end = stmt.span.start + "switch".len() as u32;
        let keyword_comments = self.build_keyword_paren_comments(switch_keyword_end, open_paren);

        // Preserve comments between ) and { in place:
        //   switch(x)/* c */{} → switch (x) /* c */ {}
        // Scan for the body `{` outside comments — a naive find('{') matches a `{`
        // inside the gap comment (`switch (x) /* { */ {`), mis-anchoring the body
        // brace and dropping the comment.
        let body_open_brace = close_paren
            .and_then(|close| self.find_char_outside_comments(close + 1, stmt.span.end, b'{'));
        // Build condition group (handles breaking within discriminant and comments)
        let condition_group = if let (Some(open), Some(close)) = (open_paren, close_paren) {
            self.build_condition_group_with_comments(&stmt.discriminant, open, close)
        } else {
            self.build_condition_group(&stmt.discriminant)
        };

        // Build cases - they handle their own internal indentation
        // Join cases with hardlines, handling comments between cases
        let mut case_parts = d.pooled_docbuf();
        // Start after the open brace to find comments between { and first case
        let brace_start = body_open_brace
            .unwrap_or_else(|| close_paren.map_or_else(|| stmt.discriminant.span().end, |p| p + 1));
        let mut prev_end = brace_start + 1;
        // Whole-body comment presence gate (the `blocks.rs` `body_has_comments` idiom):
        // a switch body with no on-page comment (~all of them) skips the per-case /
        // per-consequent comment scans below. Fail-open — on-page counts owned, so a
        // present comment always takes the full path. Blank-line preservation between
        // consequents is independent of comments and is NOT gated.
        let switch_body_end = stmt.span.end - 1; // before '}'
        let body_has_comments = self.has_comments_on_page_between(brace_start + 1, switch_body_end);
        for (i, case) in stmt.cases.iter().enumerate() {
            // Own-line comments between the previous case and this one. Same-line
            // trailing comments on the previous case's last statement — and a
            // fallthrough case's own label comment (`case 3: // fallthrough`) — were
            // emitted by the case builder and are not seen here: `prev_end` was
            // advanced past them via `find_end_with_trailing_comments` (the case-cursor
            // update below), so this range holds only genuine own-line comments.
            let comments: CommentVec<'_> = if body_has_comments {
                comments_to_emit_in_range(self.comments, prev_end, case.span.start).collect()
            } else {
                CommentVec::new()
            };
            // The break *into* this run — from the previous case, toward whichever comes
            // first, a comment or the case. Skipped for the very first item in the body
            // (`body_doc` owns that break). Everything after it is an ordinary leading
            // run: a block the author glued to the case leads it (`/* c */ case 2:`), an
            // own-line comment keeps its line, and author blank lines are preserved.
            if i > 0 {
                let run_start = comments.first().map_or(case.span.start, |c| c.span.start);
                self.push_blank_preserving_hardline(&mut case_parts, prev_end, run_start);
            }
            self.push_leading_comment_run(
                &mut case_parts,
                comments.iter().copied(),
                case.span.start,
                LeadingGlue::Adjacent,
                d.empty(),
            );

            // Determine the end boundary for inline comments on this case
            // For empty cases (fallthrough), we need to look ahead to the next case
            let next_case_start = stmt.cases.get(i + 1).map(|c| c.span.start);
            let inline_comment_boundary = next_case_start.unwrap_or(stmt.span.end - 1);

            // Rule A over the case list: an own-line directive in the `{`→first-case or
            // between-case gap freezes the case that follows it, over the case's own node
            // span — the label rides inside the slice, the sibling cases still normalize.
            // The gap anchor is the same `prev_end` the leading run above just used.
            match self.gap_frozen_span(prev_end, case.span) {
                Some(frozen) => {
                    case_parts.push(self.build_frozen_span_doc(frozen));
                    // The frozen slice is the case's own span, so a comment TRAILING its
                    // last statement sits outside it — and the cursor below skips past
                    // such a comment on the case builder's behalf. Bypassing that builder
                    // therefore has to claim the same run here, or the comment has no
                    // emitter at all (`gaps:audit` `DROPPED );⟨⟩␣`).
                    case_parts.extend(self.build_trailing_same_line_comment_docs(
                        case.span.end,
                        inline_comment_boundary,
                    ));
                }
                None => case_parts.push(self.build_switch_case_doc_inner(
                    case,
                    inline_comment_boundary,
                    body_has_comments,
                )),
            }

            // Advance past any same-line trailing comment on the case's last
            // statement — the case builder already emitted it (trailing), so the
            // between-cases / after-last-case comment loops must not re-emit it on
            // its own line.
            prev_end = self.find_end_with_trailing_comments(case.span.end);
        }

        // Comments after the last case, before the body's `}` — or, in a body with no
        // cases at all, the body's whole content. Two different questions, so two
        // emitters, each the shared statement of its rule (docs/comments.md): with a case
        // above it the run BREAKS AWAY from that case, so every comment takes its
        // separator before it; with nothing above it the run is DANGLING and the
        // separator sits strictly *between* comments, `body_doc` supplying the break on
        // either side. Neither is spelled "separator after each comment" — that has to ask
        // a comment's kind, and its answer welds a block onto the next one
        // (`/* c1 *//* c2 */`), losslessly enough that only a prettier `compare` sees it.
        if body_has_comments {
            if stmt.cases.is_empty() {
                self.push_dangling_comment_run(
                    &mut case_parts,
                    comments_to_emit_in_range(self.comments, prev_end, switch_body_end),
                );
            } else {
                case_parts.extend(self.build_trailing_body_comments_doc(
                    prev_end,
                    switch_body_end,
                    false,
                ));
            }
        }

        // Structure: switch (...) { indent([hardline, cases...]) hardline }
        // The indent wraps the hardline so cases start at +1 indent level
        // For empty switch, just output {\n}
        let body_doc = if case_parts.is_empty() {
            d.hardline()
        } else {
            d.concat(&[d.indent_hardline(d.concat(&case_parts)), d.hardline()])
        };

        let mut switch_parts: DocBuf = DocBuf::new();
        self.push_keyword_open_paren(&mut switch_parts, "switch", keyword_comments);
        switch_parts.push(condition_group);
        // `)` + its gap, then the body brace — the same `)`→block emitter `if` / `while` /
        // for-in/of use. Emitting the gap run inline and then appending a bare `" {"` (the
        // previous shape) let a `//` **swallow the opening brace** (`switch (a) // c {`),
        // which does not reparse: content corruption, not a layout quirk.
        match (close_paren, body_open_brace) {
            (Some(close), Some(brace)) => {
                self.append_close_paren_with_comments(&mut switch_parts, close + 1, brace);
            }
            _ => switch_parts.push(d.text(") ")),
        }
        switch_parts.push(d.text("{"));
        switch_parts.push(body_doc);
        switch_parts.push(d.text("}"));
        d.group(d.concat(&switch_parts))
    }

    /// Where the case's head ends — past the test expression, or past the `default`
    /// keyword — which is where the head→`:` gap opens. The one anchor both the label
    /// scan and the label emitter measure that gap from.
    fn case_head_end(case: &internal::SwitchCase<'_>) -> u32 {
        case.test.as_ref().map_or_else(
            || case.span.start + "default".len() as u32,
            |test| test.span().end,
        )
    }

    /// Get the end position of a case label (position after the colon)
    fn get_case_label_end(&self, case: &internal::SwitchCase<'_>) -> u32 {
        // Find the label ':' after the head, skipping any ':' inside a comment in the gap
        // (`case 1 /* : */:`, `default /* : */:`). With no comment the colon follows the
        // head immediately, which is also the fallback if the scan finds nothing.
        let head_end = Self::case_head_end(case);
        find_char(
            self.source.as_bytes(),
            head_end as usize,
            self.source.len(),
            b':',
            TriviaProfile::JS,
        )
        .map_or(head_end + 1, |c| c as u32 + 1)
    }

    /// Build a doc for a switch case — the label, its trailing comments, and the
    /// consequent statement list (without the outer indent, which the switch owns).
    ///
    /// `inline_comment_boundary` bounds every scan that runs off the end of this case's
    /// own span: the next case's start, or the switch body's `}` for the last one. A
    /// fallthrough case has no consequent to bound them, and a trailing comment on the
    /// last statement falls outside the `SwitchCase` span either way.
    fn build_switch_case_doc_inner(
        &self,
        case: &internal::SwitchCase<'_>,
        inline_comment_boundary: u32,
        body_has_comments: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

        // case X: or default:
        let case_label_end = self.get_case_label_end(case);

        if let Some(test) = &case.test {
            // An assignment takes clarity parens here exactly as it does in a statement
            // test — prettier parenthesizes an `AssignmentExpression` in every position
            // outside its own allowlist (a for init/update, an expression statement, an
            // object-pattern property value, …), and a case test is not on it. The
            // parens are recomputed, not preserved, so `case a = 1:` gains them too.
            let test_doc = self.wrap_statement_test_parens(test, self.build_expression_doc(test));
            // The `case`→test gap needs its own emitter (`docs/comments.md` hazard 4): the
            // only comment here that survives on the test's own doc is the innermost block
            // GLUED to it, which ownership carries inside. Anything else has no owner at
            // all — a block sitting before a paren shell the printer strips
            // (`case /* c */ (a, b):`), or an earlier block in a run
            // (`case /* p */ /* q */ b:`) — so without this it is dropped. Emitted ahead of
            // any paren this position synthesizes, which is prettier's placement too.
            let test_gap_start = case.span.start + "case".len() as u32;
            let test_start = test.span().start;
            if self.has_line_comments_between(test_gap_start, test_start) {
                // A `//` here runs to end-of-line, so emitting the gap inline would swallow
                // the test AND its `:` into the comment (`case // c x:`, which does not
                // reparse) — the head→`:` gap's argument one construct earlier. The uniform
                // forced-continuation indent, and the same emitter: the run trails `case`
                // (its own space comes from the continuation, so the keyword sheds the one
                // the inline arm carries) and the test drops one level. Every comment in the
                // gap goes through it, so a block sharing the gap with a `//` keeps its place
                // in the run instead of being skipped along with it — a block prints inline
                // where a line comment defers, so emitting them apart would reorder the two.
                // Prettier keeps the test flush at the case's own indent instead — see
                // conformance_prettier_ts_comments.md §Comment relocation.
                parts.push(d.text("case"));
                parts.push(self.build_continuation_indent(test_gap_start, test_start, test_doc));
            } else {
                parts.push(d.text("case "));
                if let Some(comments) = self.build_comments_between_filtered_opt(
                    test_gap_start,
                    test_start,
                    CommentSpacing::Trailing,
                    CommentFilter::All,
                ) {
                    parts.push(comments);
                }
                parts.push(test_doc);
            }
        } else {
            parts.push(d.text("default"));
        }
        // The head→`:` gap is the same gap either way (`case 1 /* c */:`,
        // `default /* c */:`), so its comments are emitted once rather than per arm. The
        // colon sits exactly one byte before the label end, which `get_case_label_end`
        // already located as colon+1, so no second scan is needed.
        let colon_pos = case_label_end - 1;
        let head_end = Self::case_head_end(case);
        if self.has_line_comments_between(head_end, colon_pos) {
            // A `//` here runs to end-of-line, so emitting the gap inline would swallow
            // the colon into the comment (`case x // c:`, which does not reparse). The
            // uniform forced-continuation indent: the comments trail the head and the
            // bare `:` drops one level. Gated on line comments only — a multiline block
            // stays glued to the `:` whether or not the author broke after it (the
            // broke-after continuation rule is scoped to value-separator gaps), so
            // `comments_force_own_line_between` would be the wrong gate here.
            parts.push(self.build_continuation_indent(head_end, colon_pos, d.text(":")));
        } else {
            if let Some(comments) = self.build_inline_comments_between_doc_opt(head_end, colon_pos)
            {
                parts.push(comments);
            }
            parts.push(d.text(":"));
        }

        // Comments trailing the case label (`case 1: // comment`), on the shared same-line
        // trailing rule every statement list uses — which also walks a multiline block to
        // its CLOSING line, so a comment the author glued after the `*/`
        // (`case 1: /* a⏎b */ /* c */`) is claimed here instead of being split off onto its
        // own line by the consequent's leading run below. A line comment goes through
        // `line_suffix` (zero width) so it never forces the case test (e.g. a binary
        // expression) to break; it flushes at the consequent's hardline (prettier's
        // `lineSuffix`). A block stays inline, width counted.
        // For fallthrough cases (no consequent), use the boundary passed by the switch printer.
        let first_stmt_start = case.consequent.first().map(|s| s.span().start);
        let inline_comment_end = first_stmt_start.unwrap_or(inline_comment_boundary);
        let label_trailing_end = if body_has_comments {
            parts.extend(
                self.build_trailing_same_line_comment_docs(case_label_end, inline_comment_end),
            );
            // The in-source twin of that emitter's walk — the cursor the consequent starts
            // from, so a claimed comment is neither re-emitted by the leading run
            // (docs/comments.md hazard 3) nor read as an author blank line. The same
            // emitter/cursor pairing the block and class statement lists use.
            self.find_end_with_trailing_comments(case_label_end)
                .min(inline_comment_end)
        } else {
            case_label_end
        };
        // A `//` anywhere in that run ends the label's rendered line, so a block that would
        // otherwise hug the label has to drop below it. Asked of the whole claimed run
        // rather than of the label's own line: the walk above can carry the run onto a
        // multiline block's closing line, and it is the RENDERED line the block competes for.
        let label_trailing_line_comment = body_has_comments
            && comments_to_emit_in_range(self.comments, case_label_end, label_trailing_end)
                .any(|c| !c.is_block);

        // Consequent statements (indented from case line)
        // Handle comments between statements like block statements do
        let mut prev_end = label_trailing_end;
        let mut prev_stmt_end: Option<u32> = None;
        // Set when the statement just emitted deferred a line comment past its own `;`, so
        // its doc ends on a later line than the `;` and cannot carry that line's comments.
        let mut prev_deferred_line_comment = false;

        for (i, stmt) in case.consequent.iter().enumerate() {
            let stmt_start = stmt.span().start;

            // Standalone EmptyStatements are dropped entirely (Prettier's
            // `printStatementSequence` never prints them), but any comments
            // attached to one must survive — printed as orphaned comments with
            // nothing following them in this iteration to glue to. The next
            // iteration's own unconditional leading hardline supplies the
            // separator (mirroring `build_statement_list_docs_into`).
            if matches!(stmt, Statement::EmptyStatement(_)) {
                let stmt_end = stmt.span().end;
                let next_bound = case
                    .consequent
                    .get(i + 1)
                    .map_or(inline_comment_boundary, |s| s.span().start);
                let search_end = self
                    .find_end_with_trailing_comments(stmt_end)
                    .min(next_bound);

                let leading_comments = if body_has_comments {
                    self.collect_leading_comments(
                        prev_end,
                        search_end,
                        prev_stmt_end,
                        prev_deferred_line_comment,
                    )
                } else {
                    CommentVec::new()
                };

                if !leading_comments.is_empty() {
                    let mut stmt_parts: DocBuf = smallvec![d.hardline()];
                    if prev_stmt_end.is_some() {
                        let check_end = leading_comments[0].span.start;
                        if self.has_blank_line_between(prev_end, check_end) {
                            stmt_parts.push(d.hardline());
                        }
                    }
                    self.push_orphaned_comment_run(&mut stmt_parts, &leading_comments, search_end);
                    parts.push(d.indent(d.concat(&stmt_parts)));
                    prev_stmt_end = Some(stmt_end);
                }

                prev_end = search_end;
                continue;
            }

            // Comments between the previous position and this statement, minus the ones
            // the previous statement's trailing emitter already took — the shared
            // statement-list collector, anchored exactly as the block and class lists
            // anchor it. The FIRST statement needs no anchor of its own: the label's
            // trailing run was claimed above and `prev_end` starts past it, so nothing on
            // the label's line is in range to re-emit (docs/comments.md hazard 3).
            let leading_comments = if body_has_comments {
                self.collect_leading_comments(
                    prev_end,
                    stmt_start,
                    prev_stmt_end,
                    prev_deferred_line_comment,
                )
            } else {
                CommentVec::new()
            };

            // Trailing same-line comments on THIS statement (mirrors the block
            // statement joiner `build_statement_list_docs_into`). Without this the
            // switch-case consequent silently DROPS interior trailing comments, and
            // the last statement's trailing comment (which falls outside the
            // SwitchCase span) gets relocated to its own line by the switch printer.
            // A line comment trails via `line_suffix`; a block comment renders inline
            // — its continuation lines indent to the statement, so the docs must sit
            // INSIDE the statement's `indent`. Bound the scan at the next *printed*
            // statement's start (skipping dropped `;`s), or `inline_comment_boundary`
            // (next case / switch end) for the last one, so a comment attaches only to
            // the statement it follows — while a same-line comment trailing a dropped
            // `;` (`f();; // c`) still attaches here (the `;` emits nothing to carry it).
            // …unless this statement's doc ends with a line comment its terminator gap
            // deferred past the `;`. Nothing may share that line, so the run is left for
            // the next statement's leading collector to claim (which the `prev_end` left
            // at the `;` below hands over); trailing it here welds a following block onto
            // the line comment, making it the comment's text.
            let stmt_end = stmt.span().end;
            let next_bound = next_printed_stmt_start(case.consequent, i, inline_comment_boundary);
            // Deferring hands the `;` line's comments to the NEXT statement's leading run,
            // so it needs one to exist. For the last statement of a consequent there is no
            // such run, and the switch's own after-last-case emitter sits at CASE-LABEL
            // indent — which prettier agrees is right for a post-consequent comment, but
            // the deferred `//` is emitted inside the statement's doc at consequent indent,
            // so the pair could never share a fixed point (pass 2 dedents the `//`). That
            // shape keeps the ordinary trailing path; the class / block / program lists
            // have a body-level emitter at the right indent and defer to the end.
            let has_next_stmt = next_bound != inline_comment_boundary;
            // THIS statement's verdict — distinct from `prev_deferred_line_comment`, which
            // is the PREVIOUS statement's and is still being read below (the leading run
            // and the blank scan). The trailing gate needs its own answer here, before the
            // loop tail hands it forward, so the two must not share one variable.
            let this_defers_line_comment = body_has_comments
                && has_next_stmt
                && self.terminator_defers_line_comment(stmt_start, stmt_end);
            let trailing = if this_defers_line_comment {
                DocBuf::new()
            } else {
                self.build_trailing_same_line_comment_docs(stmt_end, next_bound)
            };

            // Rule A over the consequent list: an own-line directive in the label→first
            // gap or between statements freezes the statement that follows it. Resolved
            // BEFORE the layout choice because the hug below would pull the directive onto
            // the label's line — an inert placement, so the freeze would die on pass 2.
            let frozen = self.gap_frozen_span(prev_end, stmt.span());

            // First block statement hugs the case label: `case 'a': { ... }` — the two
            // layouts that keep something on the label's own line. Both need that line
            // free of leading comments: a comment written ON the label's line trails the
            // label (the emitter above claimed it), so anything reaching the leading run
            // sits below and claims a line of its own, which the general path prints on
            // the shared rule with the block following it at consequent indent. Pulling
            // such a run up to keep the hug would relocate it — merging own-line comments
            // onto one line — for exactly the authorings a `//` in the same gap already
            // keeps in place (`case 'b':⏎// c⏎{`, docs/conformance_prettier_ts_comments.md
            // §Comment relocation), so comment kind stops deciding the layout. A frozen statement
            // keeps its own line too: the hug would take away the directive's.
            if i == 0
                && matches!(stmt, Statement::BlockStatement(_))
                && frozen.is_none()
                && leading_comments.is_empty()
            {
                // A line comment on the label already ends that line, so the block drops
                // below it — at CASE indent, since nothing else was written there.
                parts.push(if label_trailing_line_comment {
                    d.hardline()
                } else {
                    d.text(" ")
                });
                // A SwitchCase consequent isn't a Program/BlockStatement, so a
                // bare string statement here is never directive-prologue
                // eligible — see `Printer::needs_avoid_directive_parens`.
                parts.push(self.build_statement_doc(stmt, false));
                parts.extend(trailing);
            } else {
                // Build the indented content for this statement
                let mut stmt_parts: DocBuf = smallvec![d.hardline()];

                // Preserve blank lines between statements within case consequent.
                //
                // Suppressed when the previous statement DEFERRED. The deferred `//` is
                // emitted from inside that statement's own doc, so the blank this scan
                // finds sits between it and this run — and on the next pass, with the `;`
                // glued, the pair are two ordinary LEADING comments, a position where the
                // consequent drops the blank between them. Emitting it here would make
                // pass 1 disagree with pass 2 (an F1 non-idempotency); the block and class
                // lists preserve it in both passes and so keep it.
                if prev_stmt_end.is_some() && !prev_deferred_line_comment {
                    let check_end = leading_comments
                        .first()
                        .map_or(stmt_start, |c| c.span.start);
                    if self.has_blank_line_between(prev_end, check_end) {
                        stmt_parts.push(d.hardline());
                    }
                }

                // Print leading comments before this statement, on the shared rule
                // (prettier's `printLeadingComment`) the block / class / interface / member
                // lists all use — the separator after each comment is keyed on the source
                // around *that comment*, never on where the statement starts. Keying it on
                // the statement instead splits a run the author glued
                // (`/* c1 */ /* c2 */⏎stmt`), which is the bug family docs/comments.md
                // §"Leading comments" names; the case-LABEL run above already routes here.
                self.push_leading_comments_before(&mut stmt_parts, &leading_comments, stmt_start);

                stmt_parts.push(match frozen {
                    // The freeze emitter claims the glued block comment the statement
                    // owns — the leading run above skips it (docs/comments.md hazard 1).
                    Some(span) => self.build_frozen_node_doc(span),
                    None => self.build_statement_doc(stmt, false),
                });
                stmt_parts.extend(trailing);

                parts.push(d.indent(d.concat(&stmt_parts)));
            }

            // Advance past the trailing comments so the next statement's leading
            // scan and blank-line detection start after them — but not when this
            // statement deferred, since nothing trailed and advancing would step the
            // leading scan PAST the comments it now has to claim, dropping them.
            prev_end = if this_defers_line_comment {
                stmt_end
            } else {
                self.find_end_with_trailing_comments(stmt_end)
            };
            prev_stmt_end = Some(stmt_end);
            prev_deferred_line_comment = this_defers_line_comment;
        }

        // Note: a same-line trailing comment on the *last* statement is consumed
        // above; the switch printer advances its case cursor past it (via
        // `find_end_with_trailing_comments`) so it is not re-emitted there.

        d.concat(&parts)
    }
}
