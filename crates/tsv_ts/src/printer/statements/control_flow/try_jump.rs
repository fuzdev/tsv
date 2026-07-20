// try/catch/finally, throw, break/continue, and labeled statement printing

use crate::ast::internal::{self, Statement};
use crate::printer::{CommentVec, Printer};
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

impl<'a> Printer<'a> {
    /// Append a space (or comments + space/hardline) between a keyword/token end and body start.
    ///
    /// Used for `try /* c */ {`, `catch (e) /* c */ {`, `catch /* c */ {`, `finally /* c */ {`.
    fn append_keyword_to_body_comments(&self, parts: &mut DocBuf, token_end: u32, body_start: u32) {
        let d = self.d();
        if self.has_comments_to_emit_between(token_end, body_start) {
            let has_line = self.has_line_comments_between(token_end, body_start);
            parts.push(self.build_inline_comments_between_doc(token_end, body_start));
            if has_line {
                parts.push(d.hardline());
            } else {
                parts.push(d.text(" "));
            }
        } else {
            parts.push(d.text(" "));
        }
    }

    /// Append ` keyword`, preserving where the author put any comments in the gap
    /// before it: one that trailed the previous `}` stays trailing, one on its own
    /// line keeps its own line. The keyword-preceding mirror of
    /// `append_keyword_to_body_comments`, shared by the `catch` and `finally` heads
    /// (`gap_start` = previous block end, `keyword_pos` = the comment-skipping
    /// keyword position).
    ///
    /// This is the `}`→continuation-keyword gap, so it partitions by line exactly as
    /// the `}`→`else` path does — the authored position is the whole signal, and a
    /// blank line above an own-line comment is authoring intent
    /// ([`ControlFlowGap::BlockToKeyword`]). Prettier is no oracle here: it relocates
    /// these comments into the following block's body, which it does *not* do at
    /// `else`. See `conformance_prettier.md` §Comment relocation.
    fn append_comments_then_keyword(
        &self,
        parts: &mut DocBuf,
        gap_start: u32,
        keyword_pos: u32,
        keyword: &'static str,
    ) {
        // A `try`/`catch` body is always a block, so the keyword can hug `}`.
        self.push_block_to_keyword_gap(parts, gap_start, keyword_pos, true);
        parts.push(self.d().text(keyword));
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
            // `handler.span.start` is the position of the "catch" keyword.
            let try_end = stmt.block.span.end;
            let catch_keyword_pos = handler.span.start;
            self.append_comments_then_keyword(&mut parts, try_end, catch_keyword_pos, "catch");
            if let Some(param) = &handler.param {
                // Find paren positions for comment handling
                let catch_keyword_end = handler.span.start + "catch".len() as u32;
                let open_paren = self.find_open_paren_after(stmt.block.span.end);
                let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));

                // Preserve comments between catch keyword and ( in place:
                //   catch/* comment */(e) → catch /* comment */ (e)
                let keyword_comments =
                    self.build_keyword_paren_comments(catch_keyword_end, open_paren);
                if let Some(kc) = keyword_comments {
                    parts.push(kc);
                    parts.push(d.text("("));
                } else {
                    parts.push(d.text(" ("));
                }

                // Check for comments in catch parameter
                if let (Some(open), Some(close)) = (open_paren, close_paren)
                    && (self.has_comments_to_emit_between(open + 1, param.span().start)
                        || self.has_comments_to_emit_between(param.span().end, close))
                {
                    parts.push(self.build_condition_group_with_comments(param, open, close));
                } else {
                    parts.push(self.build_expression_doc(param));
                }
                parts.push(d.text(")"));

                // Comments between ) and body: `catch (e) /* comment */ {`
                let paren_end = close_paren.unwrap_or_else(|| param.span().end) + 1;
                self.append_keyword_to_body_comments(
                    &mut parts,
                    paren_end,
                    handler.body.span.start,
                );
            } else {
                // No param: comments between catch keyword and body: `catch /* comment */ {`
                let catch_keyword_end = handler.span.start + "catch".len() as u32;
                self.append_keyword_to_body_comments(
                    &mut parts,
                    catch_keyword_end,
                    handler.body.span.start,
                );
            }
            // Catch block stays inline (`catch (e) {}`) UNLESS a `finally`
            // follows, in which case it expands empty like `try`/`finally` do
            // (Prettier's `block.js`: `parent.type === "CatchClause" &&
            // !parentParent.finalizer` is the only case that stays collapsed).
            if stmt.finalizer.is_some() {
                parts.push(self.build_block_statement_expand_empty_doc(&handler.body));
            } else {
                parts.push(self.build_block_statement_doc(&handler.body));
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
            self.append_comments_then_keyword(&mut parts, prev_end, finally_keyword_pos, "finally");
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
        d.concat(&parts)
    }

    pub(in crate::printer::statements) fn build_throw_statement_doc(
        &self,
        stmt: &internal::ThrowStatement<'_>,
    ) -> DocId {
        self.build_keyword_argument_doc("throw", stmt.span.start, stmt.span.end, &stmt.argument)
    }

    pub(in crate::printer::statements) fn build_break_statement_doc(
        &self,
        stmt: &internal::BreakStatement<'_>,
    ) -> DocId {
        self.build_jump_statement_doc("break", stmt.span, stmt.label.as_ref())
    }

    pub(in crate::printer::statements) fn build_continue_statement_doc(
        &self,
        stmt: &internal::ContinueStatement<'_>,
    ) -> DocId {
        self.build_jump_statement_doc("continue", stmt.span, stmt.label.as_ref())
    }

    /// Shared builder for break/continue statements with optional label and trailing comments.
    fn build_jump_statement_doc(
        &self,
        keyword: &'static str,
        span: Span,
        label: Option<&internal::Identifier<'_>>,
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
            // an own-line comment on its own line after. See `split_separator_gap_comments`.
            let semicolon_pos = span.end.saturating_sub(1);
            self.push_semicolon_with_gap_comments(&mut parts, label.span.end, semicolon_pos, true);
            d.concat(&parts)
        } else {
            // No label: a bare keyword closed by `;`. It swallows a following explicit
            // `;` as its terminator (no `[no LineTerminator]` issue once the label is
            // absent), so any comment between the keyword and that `;` is interior to the
            // span — the shared helper preserves it (own-line aware, blank line kept). The
            // previous inline-only emission merged consecutive own-line comments onto one
            // line (`break; // c1 // c2`, swallowing the second).
            self.build_bare_keyword_terminator_doc(keyword, span)
        }
    }

    pub(in crate::printer::statements) fn build_labeled_statement_doc(
        &self,
        stmt: &internal::LabeledStatement<'_>,
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
        if self.has_comments_to_emit_between(colon_end, body_start) {
            let has_line_comment = self.has_line_comments_between(colon_end, body_start);
            tail_parts.push(d.text(":"));
            tail_parts.push(self.build_inline_comments_between_doc(colon_end, body_start));
            tail_parts.push(if has_line_comment {
                d.hardline()
            } else {
                d.text(" ")
            });
            tail_parts.push(self.build_statement_doc(stmt.body, false));
        } else {
            // No space before empty statement: `label:;` not `label: ;`
            let separator = if matches!(stmt.body, Statement::EmptyStatement(_)) {
                ":"
            } else {
                ": "
            };
            tail_parts.push(d.text(separator));
            tail_parts.push(self.build_statement_doc(stmt.body, false));
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
        let gap_comments: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, label_end, colon_pos as u32).collect();
        let relocate = gap_comments.iter().any(|c| self.is_own_line_comment(c));

        let mut parts: DocBuf = smallvec![];
        if relocate {
            for (i, comment) in gap_comments.iter().enumerate() {
                parts.push(self.build_comment_doc(comment));
                // A space keeps a same-line block + line pair together (`/* c */ // d`);
                // otherwise break. The last comment always breaks before the label.
                match gap_comments.get(i + 1) {
                    Some(next) if self.is_same_line(comment.span.end, next.span.start) => {
                        parts.push(d.text(" "));
                    }
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
