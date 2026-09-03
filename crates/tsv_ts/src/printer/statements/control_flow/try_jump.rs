// try/catch/finally, throw, break/continue, and labeled statement printing

use super::OpenParenLineBlockComment;
use crate::ast::internal::{self, Statement};
use crate::printer::statements::StatementContext;
use crate::printer::{CommentVec, LeadingGlue, Printer};
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

impl<'a> Printer<'a> {
    /// Append a space (or comments + space/hardline) between a keyword/token end and body start.
    ///
    /// Used for `try /* c */ {`, `catch (e) /* c */ {`, `catch /* c */ {`, `finally /* c */ {`.
    ///
    /// The keyword anchor for `push_header_to_body_gap` — see there for the gap's comment
    /// rules. Emitting this gap inline instead would relocate an
    /// own-line `//` up onto the keyword line (`try⏎// c⏎{` → `try // c⏎{`), which the
    /// `if`/`while` `)`→`{` siblings never do.
    fn append_keyword_to_body_comments(&self, parts: &mut DocBuf, token_end: u32, body_start: u32) {
        if self.has_comments_to_emit_between(token_end, body_start) {
            self.push_header_to_body_gap(parts, token_end, body_start);
        } else {
            parts.push(self.d().text(" "));
        }
    }

    /// Append the `}`→continuation-keyword gap and then the clause it introduces — `catch` or
    /// `finally`. Comments in the gap keep where the author put them: one that trailed the
    /// previous `}` stays trailing, one on its own line keeps its own line. The
    /// keyword-preceding mirror of [`Self::append_keyword_to_body_comments`].
    ///
    /// The gap partitions by line exactly as the `}`→`else` path does — the authored position is
    /// the whole signal, and a blank line above an own-line comment is authoring intent
    /// ([`Self::push_block_to_keyword_gap`]). Prettier is no oracle here: it relocates these
    /// comments into the following block's body, which it does *not* do at `else`. See
    /// `conformance_prettier_ts_comments.md` §Comment relocation.
    ///
    /// An own-line directive in the gap freezes the WHOLE clause — keyword, any binding and the
    /// body all ride inside the verbatim slice — and the return is then `true`, telling the
    /// caller to emit nothing further for this clause. `clause_end` rather than a span because
    /// the two clauses anchor differently: a `CatchClause`'s span already starts at its keyword,
    /// while a finalizer's starts at its `{` (the keyword sits in the gap), so the frozen slice
    /// is keyed on `keyword_pos` for both. Prettier instead relocates the directive into the
    /// clause body and freezes the first statement there
    /// (`handler_prettier_ignore_head_prettier_divergence`).
    fn append_clause_head(
        &self,
        parts: &mut DocBuf,
        gap_start: u32,
        keyword_pos: u32,
        keyword: &'static str,
        clause_end: u32,
    ) -> bool {
        // A `try`/`catch` body is always a block, so the keyword can hug `}`.
        self.push_block_to_keyword_gap(parts, gap_start, keyword_pos, true, false);
        match self.gap_frozen_span(gap_start, Span::new(keyword_pos, clause_end)) {
            Some(frozen) => {
                parts.push(self.build_frozen_span_doc(frozen));
                true
            }
            None => {
                parts.push(self.d().text(keyword));
                false
            }
        }
    }

    /// Append a `catch` clause's binding and body — everything after the keyword, which
    /// [`Self::append_clause_head`] has already emitted. Split out of
    /// `build_try_statement_doc` so the freeze guard reads as one branch rather than wrapping
    /// this whole run.
    fn append_catch_clause_body(
        &self,
        parts: &mut DocBuf,
        stmt: &internal::TryStatement<'_>,
        handler: &internal::CatchClause<'_>,
    ) {
        let d = self.d();
        let catch_keyword_end = handler.span.start + "catch".len() as u32;
        if let Some(param) = &handler.param {
            // Find paren positions for comment handling
            let open_paren = self.find_open_paren_after(stmt.block.span.end);
            let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));

            // Preserve comments between catch keyword and ( in place:
            //   catch/* comment */(e) → catch /* comment */ (e)
            // The keyword itself was emitted by `append_clause_head` (possibly as a
            // frozen span), so this takes the opener's `(` half.
            let keyword_comments = self.build_keyword_paren_comments(catch_keyword_end, open_paren);
            self.push_open_paren_after_keyword(parts, keyword_comments);

            // Check for comments in catch parameter
            if let (Some(open), Some(close)) = (open_paren, close_paren)
                && self.header_parens_hold_comments(open, close, param)
            {
                parts.push(self.build_condition_group_with_comments(
                    param,
                    open,
                    close,
                    OpenParenLineBlockComment::JoinsRun,
                ));
            } else {
                parts.push(self.build_expression_doc(param));
            }
            parts.push(d.text(")"));

            // Comments between ) and body: `catch (e) /* comment */ {`
            let paren_end = close_paren.unwrap_or_else(|| param.span().end) + 1;
            self.append_keyword_to_body_comments(parts, paren_end, handler.body.span.start);
        } else {
            // No param: comments between catch keyword and body: `catch /* comment */ {`
            self.append_keyword_to_body_comments(parts, catch_keyword_end, handler.body.span.start);
        }
        // Catch block stays inline (`catch (e) {}`) UNLESS a `finally` follows, in which case
        // it expands empty like `try`/`finally` do (Prettier's `block.js`:
        // `parent.type === "CatchClause" && !parentParent.finalizer` is the only case that
        // stays collapsed).
        if stmt.finalizer.is_some() {
            parts.push(self.build_block_statement_expand_empty_doc(&handler.body));
        } else {
            parts.push(self.build_block_statement_doc(&handler.body));
        }
    }

    pub(in crate::printer::statements) fn build_try_statement_doc(
        &self,
        stmt: &internal::TryStatement<'_>,
    ) -> DocId {
        let d = self.d();

        // try keyword to block: `try /* comment */ {`
        let try_keyword_end = stmt.span.start + "try".len() as u32;
        let block_start = stmt.block.span.start;
        let mut parts = d.pooled_docbuf();
        parts.push(d.text("try"));
        self.append_keyword_to_body_comments(&mut parts, try_keyword_end, block_start);
        // Try block expands empty: `try {\n}` not `try {}`
        parts.push(self.build_block_statement_expand_empty_doc(&stmt.block));

        if let Some(handler) = &stmt.handler {
            // `handler.span.start` is the position of the "catch" keyword. A frozen clause
            // rides out whole in the verbatim slice — keyword, binding and body — so the
            // per-part emission is skipped entirely for it.
            let try_end = stmt.block.span.end;
            if !self.append_clause_head(
                &mut parts,
                try_end,
                handler.span.start,
                "catch",
                handler.span.end,
            ) {
                self.append_catch_clause_body(&mut parts, stmt, handler);
            }
        }
        if let Some(finalizer) = &stmt.finalizer {
            // Check for comments before finally (after catch block or try block)
            let prev_end = stmt
                .handler
                .as_ref()
                .map_or(stmt.block.span.end, |h| h.body.span.end);
            // The finalizer span starts at the "finally" block `{`; the keyword sits
            // in the gap after the previous block. It's the only real keyword there,
            // so the first whole-word match wins — trivia-aware so a `/* finally */`
            // comment before or after the keyword can't be mistaken for it (a raw
            // `rfind` matched the one inside such a comment and dropped it).
            let finally_keyword_pos = self
                .find_keyword_in_range(prev_end, finalizer.span.start, "finally")
                .unwrap_or(finalizer.span.start);
            if !self.append_clause_head(
                &mut parts,
                prev_end,
                finally_keyword_pos,
                "finally",
                finalizer.span.end,
            ) {
                // Comments between finally keyword and body: `finally /* comment */ {`
                let finally_keyword_end = finally_keyword_pos + "finally".len() as u32;
                self.append_keyword_to_body_comments(
                    &mut parts,
                    finally_keyword_end,
                    finalizer.span.start,
                );
                // Finally block expands empty: `finally {\n}` not `finally {}`
                parts.push(self.build_block_statement_expand_empty_doc(finalizer));
            }
        }
        d.concat(&parts)
    }

    pub(in crate::printer::statements) fn build_throw_statement_doc(
        &self,
        stmt: &internal::ThrowStatement<'_>,
        clause_tail: Option<u8>,
    ) -> DocId {
        self.build_keyword_argument_doc(
            "throw",
            stmt.span.start,
            stmt.span.end,
            stmt.argument,
            clause_tail,
        )
    }

    pub(in crate::printer::statements) fn build_break_statement_doc(
        &self,
        stmt: &internal::BreakStatement<'_>,
        clause_tail: Option<u8>,
    ) -> DocId {
        self.build_jump_statement_doc("break", stmt.span, stmt.label.as_ref(), clause_tail)
    }

    pub(in crate::printer::statements) fn build_continue_statement_doc(
        &self,
        stmt: &internal::ContinueStatement<'_>,
        clause_tail: Option<u8>,
    ) -> DocId {
        self.build_jump_statement_doc("continue", stmt.span, stmt.label.as_ref(), clause_tail)
    }

    /// Shared builder for break/continue statements with optional label and trailing comments.
    fn build_jump_statement_doc(
        &self,
        keyword: &'static str,
        span: Span,
        label: Option<&internal::Identifier<'_>>,
        clause_tail: Option<u8>,
    ) -> DocId {
        let d = self.d();
        if let Some(label) = label {
            let keyword_end = span.start + keyword.len() as u32;
            // Comments between keyword and label (e.g., `break /* c */ loop;`)
            let pre_label_comment =
                self.build_inline_comments_between_doc_opt(keyword_end, label.span.start);

            let mut parts = DocBuf::new();
            parts.push(d.text(keyword));
            if let Some(comment_doc) = pre_label_comment {
                parts.push(comment_doc);
            }
            parts.push(d.text(" "));
            parts.push(self.identifier_name_doc(label));
            // Comments between label and `;`: a same-line block trails *after* the `;`
            // (`break loop; /* c */`, prettier 3.9), a same-line line via `line_suffix`,
            // an own-line comment on its own line after. See `push_semicolon_with_gap_comments`.
            self.push_semicolon_with_gap_comments(
                &mut parts,
                label.span.end,
                span.end,
                true,
                clause_tail,
            );
            d.concat(&parts)
        } else {
            // No label: a bare keyword closed by `;`. It swallows a following explicit
            // `;` as its terminator (no `[no LineTerminator]` issue once the label is
            // absent), so any comment between the keyword and that `;` is interior to the
            // span — the shared helper preserves it (own-line aware, blank line kept). The
            // previous inline-only emission merged consecutive own-line comments onto one
            // line (`break; // c1 // c2`, swallowing the second).
            self.build_bare_keyword_terminator_doc(keyword, span, clause_tail)
        }
    }

    pub(in crate::printer::statements) fn build_labeled_statement_doc(
        &self,
        stmt: &internal::LabeledStatement<'_>,
        ctx: StatementContext,
    ) -> DocId {
        let d = self.d();
        let label_end = stmt.label.span.end;
        let body_start = stmt.body.span().start;

        // Find actual colon position (skip comments between label and colon)
        let colon_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            label_end as usize,
            body_start as usize,
            b':',
        )
        .unwrap_or(label_end as usize);
        let colon_end = colon_pos as u32 + 1;

        // Build the `: body` tail (including any colon→body comments).
        let mut tail_parts: DocBuf = smallvec![];
        if self.gap_frozen_span(colon_end, stmt.body.span()).is_some() {
            // An own-line directive in the `:`→body gap freezes the body. The inline
            // emission below would trail the run on the label's line (`lll: // c`), an
            // inert placement that loses the freeze on the second pass, so route through
            // the own-line-preserving header→body emitter instead — the declaration-header
            // rule of `conformance_prettier_ignore.md` §On module and declarator lists.
            // The statement-aware emitter restores the `;` an ASI-reliant body owes
            // ([`Printer::build_frozen_statement_doc`]).
            tail_parts.push(d.text(":"));
            self.push_header_to_body_gap(&mut tail_parts, colon_end, body_start);
            tail_parts.push(self.build_frozen_statement_doc(stmt.body));
        } else if self.has_comments_to_emit_between(colon_end, body_start) {
            // The run is pinned to the `:` line (prettier hoists it above the whole labeled
            // statement instead — the sanctioned position divergence), but its INTERNAL
            // shape is not part of that licence: the lines the author gave the run survive,
            // as they do in every other run. Emitting it with one separator keyed on
            // `has_line_comments_between` WELDED them (`l: /* c1 */⏎// c2` → one line), a
            // divergence riding inside the cataloged one and matching neither formatter —
            // prettier keeps those breaks in its own position.
            //
            // `AdjacentAnchorLine` for the same reason the empty-body gap uses it: the run's
            // first comment shares the `:` line by construction, so reading its authored
            // newline-before would force a break the next pass removes.
            //
            // The group holds the run ALONE, not the body: with no oracle here (prettier is
            // somewhere else entirely), the body's width must not decide where a comment
            // separator breaks. The run's own hardline still opens it.
            tail_parts.push(d.text(":"));
            tail_parts.push(d.text(" "));
            let mut run = DocBuf::new();
            self.push_leading_comment_run(
                &mut run,
                self.comments_to_emit_between(colon_end, body_start),
                body_start,
                LeadingGlue::AdjacentAnchorLine,
                None,
            );
            tail_parts.push(d.group(d.concat(&run)));
            tail_parts.push(self.build_statement_doc(stmt.body, ctx.labeled_body()));
        } else {
            // No space before empty statement: `label:;` not `label: ;`
            let separator = if matches!(stmt.body, Statement::EmptyStatement(_)) {
                ":"
            } else {
                ": "
            };
            tail_parts.push(d.text(separator));
            tail_parts.push(self.build_statement_doc(stmt.body, ctx.labeled_body()));
        }
        let tail = d.concat(&tail_parts);

        // An **own-line** comment in the label→`:` gap — a line comment, or a block
        // comment the author put on its own line — is relocated onto its own line(s)
        // before the label (matching prettier). A line comment must move (emitting it
        // inline would let the `//` swallow the `:` + body); an own-line block follows
        // the same rule rather than reflowing inline. A purely **same-line** block
        // stays inline before `:` (`label /* c */: body`), matching prettier.
        // **to emit**: this set is printed below, and `relocate` is derived from it — so the
        // two agree by construction. Nothing can be owned here anyway: an owned comment binds
        // to the token that follows it, and `:` begins no node.
        let gap_comments: CommentVec<'_> = self
            .comments_to_emit_between(label_end, colon_pos as u32)
            .collect();
        let relocate = gap_comments.iter().any(|c| self.is_own_line_comment(c));

        let mut parts: DocBuf = smallvec![];
        if relocate {
            for (i, comment) in gap_comments.iter().enumerate() {
                parts.push(self.build_comment_doc(comment));
                // A space keeps a pair the author glued together (`/* c */ // d`);
                // otherwise break. The glue question is `comment_hugs_next`, the single
                // glue test — not an `is_same_line` restatement of it, which only
                // coincides here because this gap holds nothing but trivia between two
                // comments (`docs/comments.md` §Own-line-ness is a SOURCE question).
                //
                // ⚠️ The **last** comment always breaks, whatever the source says: it is
                // followed by the label the run was relocated off, so hugging it would
                // undo the relocation this arm exists for.
                match gap_comments.get(i + 1) {
                    Some(_) if self.comment_hugs_next(comment) => parts.push(d.text(" ")),
                    _ => parts.push(d.hardline()),
                }
            }
            parts.push(self.identifier_name_doc(&stmt.label));
            parts.push(tail);
        } else {
            parts.push(self.identifier_name_doc(&stmt.label));
            parts.push(self.build_inline_comments_between_doc(label_end, colon_pos as u32));
            parts.push(tail);
        }
        d.concat(&parts)
    }
}
