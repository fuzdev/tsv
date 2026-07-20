// while and do-while statement printing
//
// Condition-group layout and body handling for while/do-while, including the
// do-while comment-preservation divergence from Prettier.

use crate::ast::internal::{self, Statement};
use crate::printer::Printer;
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build a doc for a while statement with proper line-width wrapping
    ///
    /// Matches Prettier's architecture: the condition wraps to multiple lines
    /// when the `while (condition)` line exceeds print width.
    pub(in crate::printer::statements) fn build_while_statement_doc(
        &self,
        stmt: &internal::WhileStatement<'_>,
    ) -> DocId {
        let d = self.d();
        // Find paren positions for comment handling
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));

        // Preserve comments between `while` keyword and `(` in place:
        //   while/* c */(a){} → while /* c */ (a) {}
        let while_keyword_end = stmt.span.start + "while".len() as u32;
        let keyword_comments = self.build_keyword_paren_comments(while_keyword_end, open_paren);

        // Build condition group (handles breaking within condition and comments,
        // and the `!(logical)` inline-negation hug).
        let condition_group =
            self.build_statement_condition_doc(&stmt.test, open_paren, close_paren);

        if let Statement::BlockStatement(block) = stmt.body {
            // Block body: while (cond) { ... }
            // Uses append_close_paren_with_comments for consistency with if/for-in/for-of:
            // block comments stay inline, line comments become trailing.
            let mut parts: DocBuf = DocBuf::new();
            self.push_keyword_open_paren(&mut parts, "while", keyword_comments);
            parts.push(condition_group);
            let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end) + 1;
            self.append_close_paren_with_comments(&mut parts, paren_end, block.span.start);
            parts.push(self.build_block_statement_doc(block));
            d.group(d.concat(&parts))
        } else if matches!(stmt.body, Statement::EmptyStatement(_)) {
            // Empty statement: `while (cond);` or `while (cond) /* comment */ ;`
            let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end) + 1;
            let empty_start = stmt.body.span().start;

            let mut empty_parts: DocBuf = DocBuf::new();
            self.push_keyword_open_paren(&mut empty_parts, "while", keyword_comments);
            empty_parts.push(condition_group);
            self.append_close_paren_empty_stmt_with_comments(
                &mut empty_parts,
                paren_end,
                empty_start,
            );

            d.group(d.concat(&empty_parts))
        } else {
            // Non-block body: use adjustClause equivalent
            // - When flat: line becomes space -> `while (cond) a;`
            // - When broken: line becomes newline + indent -> `while (cond)\n\ta;`
            let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end) + 1;
            let body_start = stmt.body.span().start;
            let body_doc = self.build_statement_doc(stmt.body, false);

            let mut head_parts: DocBuf = DocBuf::new();
            self.push_keyword_open_paren(&mut head_parts, "while", keyword_comments);
            head_parts.push(condition_group);
            self.build_adjust_clause_with_comments(&head_parts, paren_end, body_start, body_doc)
        }
    }

    pub(in crate::printer::statements) fn build_do_while_statement_doc(
        &self,
        stmt: &internal::DoWhileStatement<'_>,
    ) -> DocId {
        let d = self.d();
        let is_block = matches!(stmt.body, Statement::BlockStatement(_));

        // A loop body collapses its empty block form (`do {} while (cond)`).
        let body_doc = self.build_collapsing_body_doc(stmt.body);

        // Check for comments between `do` keyword and body
        let do_end = stmt.span.start + "do".len() as u32;
        let body_start = stmt.body.span().start;
        let mut parts = if self.has_comments_to_emit_between(do_end, body_start) {
            let has_line = self.has_line_comments_between(do_end, body_start);
            let comment_doc =
                self.build_inline_comments_between_doc_no_leading_space(do_end, body_start);
            let mut p = smallvec![d.text("do")];
            if has_line && !is_block {
                // Line comment with non-block body: indent comment + body
                // do\n\t// c\n\texpr;
                p.push(d.indent(d.concat(&[d.hardline(), comment_doc, d.hardline(), body_doc])));
            } else if has_line {
                // Line comment with block body: keep flat
                p.push(d.text(" "));
                p.push(comment_doc);
                p.push(d.hardline());
                p.push(body_doc);
            } else {
                p.push(d.text(" "));
                p.push(comment_doc);
                p.push(d.text(" "));
                p.push(body_doc);
            }
            p
        } else if matches!(stmt.body, Statement::EmptyStatement(_)) {
            // Prettier's `adjustClause` returns `";"` directly for an empty body
            // → `do;`, not `do ;`.
            smallvec![d.text("do"), body_doc]
        } else {
            smallvec![d.text("do "), body_doc]
        };

        // Find the while keyword position for comment handling
        // Search forward from body end, skipping over comments to find the actual keyword
        let body_end = stmt.body.span().end;
        let test_start = stmt.test.span().start;
        let while_pos = self.find_keyword_in_range(body_end, test_start, "while");

        // The `}`→`while` gap: its comments and the separator before the keyword.
        // Emitted here, ahead of the paren bookkeeping below, which computes without
        // pushing; the `while` keyword itself follows it.
        if let Some(while_start) = while_pos {
            self.push_block_to_keyword_gap(&mut parts, body_end, while_start, is_block);
        } else {
            parts.push(if is_block { d.text(" ") } else { d.hardline() });
        }

        // Find paren positions for comment handling
        let open_paren = while_pos.and_then(|p| self.find_open_paren_after(p));
        let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));

        // Preserve comments between `while` keyword and `(` in place:
        //   do{}while/* c */(a); → do {} while /* c */ (a);
        let keyword_comments = if let Some(wp) = while_pos {
            let while_keyword_end = wp + "while".len() as u32;
            self.build_keyword_paren_comments(while_keyword_end, open_paren)
        } else {
            None
        };

        parts.push(d.text("while"));
        if let Some(kc) = keyword_comments {
            parts.push(kc);
            parts.push(d.text("("));
        } else {
            parts.push(d.text(" ("));
        }

        // Check for comments in the condition and use preserve_inline if present
        // Use preserve_inline for do-while to intentionally differ from Prettier
        // Prettier moves comments after `while (` to outside the parens - we keep them in place
        if let (Some(open), Some(close)) = (open_paren, close_paren)
            && (self.has_comments_to_emit_between(open + 1, stmt.test.span().start)
                || self.has_comments_to_emit_between(stmt.test.span().end, close))
        {
            parts.push(self.build_condition_group_preserve_inline(&stmt.test, open, close));
        } else {
            // Double-parens an assignment for clarity (`do {} while ((x = y))`),
            // matching if/while/for. Unlike those, keep the plain (self-grouping)
            // expression doc — the do-while condition has no enclosing group, so the
            // ungrouped-binary path would strand a broken `&&` chain.
            let test_doc =
                if self.needs_parens(&stmt.test, crate::printer::ParenContext::StatementTest) {
                    self.d().parens(self.build_expression_doc(&stmt.test))
                } else {
                    self.build_expression_doc(&stmt.test)
                };
            parts.push(test_doc);
        }

        // Comments between the condition's `)` and the do-while's terminating `;`,
        // with the `;` bound to the statement: a same-line block trails *after* it
        // (`} while (x) /* c */;` → `} while (x); /* c */`, prettier 3.9), a same-line
        // line via `line_suffix`, an own-line comment on its own line after. (Unlike
        // an empty *body* `;` — `if (a) /* c */ ;` — which keeps the comment inline;
        // the do-while `;` is the statement terminator.) See
        // `split_separator_gap_comments`.
        if let Some(close) = close_paren {
            parts.push(d.text(")"));
            let semicolon_pos = stmt.span.end.saturating_sub(1);
            self.push_semicolon_with_gap_comments(&mut parts, close + 1, semicolon_pos, true);
        } else {
            parts.push(d.text(");"));
        }
        d.concat(&parts)
    }
}
