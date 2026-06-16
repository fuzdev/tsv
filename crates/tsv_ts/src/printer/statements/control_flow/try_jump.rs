// try/catch/finally, throw, break/continue, and labeled statement printing

use crate::ast::internal::{self, Statement};
use crate::printer::Printer;
use tsv_lang::SymbolToU32;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

impl<'a> Printer<'a> {
    /// Append a space (or comments + space/hardline) between a keyword/token end and body start.
    ///
    /// Used for `try /* c */ {`, `catch (e) /* c */ {`, `catch /* c */ {`, `finally /* c */ {`.
    fn append_keyword_to_body_comments(
        &self,
        parts: &mut Vec<DocId>,
        token_end: u32,
        body_start: u32,
    ) {
        let d = self.d();
        if self.has_comments_between(token_end, body_start) {
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

    pub(in crate::printer::statements) fn build_try_statement_doc(
        &self,
        stmt: &internal::TryStatement,
    ) -> DocId {
        let d = self.d();

        // try keyword to block: `try /* comment */ {`
        let try_keyword_end = stmt.span.start + "try".len() as u32;
        let block_start = stmt.block.span.start;
        let mut parts = vec![d.text("try")];
        self.append_keyword_to_body_comments(&mut parts, try_keyword_end, block_start);
        // Try block expands empty: `try {\n}` not `try {}`
        parts.push(self.build_block_statement_expand_empty_doc(&stmt.block));

        if let Some(handler) = &stmt.handler {
            // Check for comments between try block and catch keyword
            let try_end = stmt.block.span.end;
            // Use handler span start which is the position of "catch" keyword
            let catch_keyword_pos = handler.span.start;
            if self.has_comments_between(try_end, catch_keyword_pos) {
                let has_line_comment = self.has_line_comments_between(try_end, catch_keyword_pos);
                parts.push(self.build_inline_comments_between_doc(try_end, catch_keyword_pos));
                if has_line_comment {
                    parts.push(d.hardline());
                } else {
                    parts.push(d.text(" "));
                }
                parts.push(d.text("catch"));
            } else {
                parts.push(d.text(" catch"));
            }
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
                }

                // Check for comments in catch parameter
                parts.push(d.text(" ("));
                if let (Some(open), Some(close)) = (open_paren, close_paren)
                    && (self.has_comments_between(open + 1, param.span().start)
                        || self.has_comments_between(param.span().end, close))
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
                    parts.push(d.hardline());
                } else {
                    parts.push(d.text(" "));
                }
                parts.push(d.text("finally"));
            } else {
                parts.push(d.text(" finally"));
            }
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
        stmt: &internal::ThrowStatement,
    ) -> DocId {
        self.build_keyword_argument_doc("throw", stmt.span.start, stmt.span.end, &stmt.argument)
    }

    pub(in crate::printer::statements) fn build_break_statement_doc(
        &self,
        stmt: &internal::BreakStatement,
    ) -> DocId {
        self.build_jump_statement_doc("break", stmt.span, stmt.label.as_ref())
    }

    pub(in crate::printer::statements) fn build_continue_statement_doc(
        &self,
        stmt: &internal::ContinueStatement,
    ) -> DocId {
        self.build_jump_statement_doc("continue", stmt.span, stmt.label.as_ref())
    }

    /// Shared builder for break/continue statements with optional label and trailing comments.
    fn build_jump_statement_doc(
        &self,
        keyword: &'static str,
        span: tsv_lang::Span,
        label: Option<&internal::Identifier>,
    ) -> DocId {
        let d = self.d();
        if let Some(label) = label {
            let keyword_end = span.start + keyword.len() as u32;
            // Comments between keyword and label (e.g., `break /* c */ loop;`)
            let pre_label_comment =
                self.build_inline_comments_between_doc_opt(keyword_end, label.span.start);
            // Comments between label and semicolon (e.g., `break loop /* c */;`)
            let post_label_comment =
                self.build_inline_comments_between_doc_opt(label.span.end, span.end);

            let mut parts = Vec::new();
            parts.push(d.text(keyword));
            if let Some(comment_doc) = pre_label_comment {
                parts.push(comment_doc);
            }
            parts.push(d.text(" "));
            parts.push(d.symbol(label.name.to_u32()));
            if let Some(comment_doc) = post_label_comment {
                parts.push(comment_doc);
            }
            parts.push(d.text(";"));
            d.concat(&parts)
        } else {
            let keyword_end = span.start + keyword.len() as u32;
            if let Some(comment_doc) =
                self.build_inline_comments_between_doc_opt(keyword_end, span.end)
            {
                d.concat(&[d.text(keyword), d.text(";"), comment_doc])
            } else {
                d.concat(&[d.text(keyword), d.text(";")])
            }
        }
    }

    pub(in crate::printer::statements) fn build_labeled_statement_doc(
        &self,
        stmt: &internal::LabeledStatement,
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

        let mut parts = vec![d.symbol(stmt.label.name.to_u32())];

        // Comments between label name and colon: `label /* c */:`
        parts.push(self.build_inline_comments_between_doc(label_end, colon_pos as u32));

        // Check for comments between colon and body
        if self.has_comments_between(colon_end, body_start) {
            let has_line_comment = self.has_line_comments_between(colon_end, body_start);
            parts.push(d.text(":"));
            parts.push(self.build_inline_comments_between_doc(colon_end, body_start));
            if has_line_comment {
                parts.push(d.hardline());
            } else {
                parts.push(d.text(" "));
            }
            parts.push(self.build_statement_doc(&stmt.body));
            d.concat(&parts)
        } else {
            // No space before empty statement: `label:;` not `label: ;`
            let separator = if matches!(stmt.body.as_ref(), Statement::EmptyStatement(_)) {
                ":"
            } else {
                ": "
            };
            parts.push(d.text(separator));
            parts.push(self.build_statement_doc(&stmt.body));
            d.concat(&parts)
        }
    }
}
