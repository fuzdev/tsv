// while and do-while statement printing
//
// Condition-group layout and body handling for while/do-while, including the
// do-while comment-preservation divergence from Prettier.

use crate::ast::internal::{self, Statement};
use crate::printer::Printer;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build a doc for a while statement with proper line-width wrapping
    ///
    /// Matches Prettier's architecture: the condition wraps to multiple lines
    /// when the `while (condition)` line exceeds print width.
    fn build_while_statement_with_wrapping_doc(&self, stmt: &internal::WhileStatement) -> DocId {
        let d = self.d();
        // Find paren positions for comment handling
        let open_paren = self.find_open_paren_after(stmt.span.start);
        let close_paren = open_paren.and_then(|o| self.matching_close_paren(o));

        // Preserve comments between `while` keyword and `(` in place:
        //   while/* c */(a){} → while /* c */ (a) {}
        let while_keyword_end = stmt.span.start + "while".len() as u32;
        let keyword_comments = self.build_keyword_paren_comments(while_keyword_end, open_paren);

        // Build condition group (handles breaking within condition and comments)
        let condition_group = if let (Some(open), Some(close)) = (open_paren, close_paren) {
            self.build_condition_group_with_comments(&stmt.test, open, close)
        } else {
            self.build_condition_group(&stmt.test)
        };

        if let Statement::BlockStatement(block) = stmt.body.as_ref() {
            // Block body: while (cond) { ... }
            // Uses append_close_paren_with_comments for consistency with if/for-in/for-of:
            // block comments stay inline, line comments become trailing.
            let mut parts = vec![d.text("while")];
            if let Some(kc) = &keyword_comments {
                parts.push(*kc);
            }
            parts.push(d.text(" ("));
            parts.push(condition_group);
            let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end) + 1;
            self.append_close_paren_with_comments(&mut parts, paren_end, block.span.start);
            parts.push(self.build_block_statement_doc(block));
            d.group(d.concat(&parts))
        } else if matches!(stmt.body.as_ref(), Statement::EmptyStatement(_)) {
            // Empty statement: `while (cond);` or `while (cond) /* comment */ ;`
            let paren_end = close_paren.unwrap_or_else(|| stmt.test.span().end) + 1;
            let empty_start = stmt.body.span().start;

            let mut empty_parts = vec![d.text("while")];
            if let Some(kc) = &keyword_comments {
                empty_parts.push(*kc);
            }
            empty_parts.push(d.text(" ("));
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
            let body_doc = self.build_statement_doc(&stmt.body);

            let mut head_parts = vec![d.text("while")];
            if let Some(kc) = &keyword_comments {
                head_parts.push(*kc);
            }
            head_parts.push(d.text(" ("));
            head_parts.push(condition_group);
            self.build_adjust_clause_with_comments(&head_parts, paren_end, body_start, body_doc)
        }
    }

    pub(in crate::printer::statements) fn build_while_statement_doc(
        &self,
        stmt: &internal::WhileStatement,
    ) -> DocId {
        // Delegate to the wrapping version for proper condition grouping
        self.build_while_statement_with_wrapping_doc(stmt)
    }

    pub(in crate::printer::statements) fn build_do_while_statement_doc(
        &self,
        stmt: &internal::DoWhileStatement,
    ) -> DocId {
        let d = self.d();
        let is_block = matches!(stmt.body.as_ref(), Statement::BlockStatement(_));

        // Check for comments between `do` keyword and body
        let do_end = stmt.span.start + "do".len() as u32;
        let body_start = stmt.body.span().start;
        let mut parts = if self.has_comments_between(do_end, body_start) {
            let has_line = self.has_line_comments_between(do_end, body_start);
            let comment_doc =
                self.build_inline_comments_between_doc_no_leading_space(do_end, body_start);
            let body_doc = self.build_statement_doc(&stmt.body);
            let mut p = vec![d.text("do")];
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
        } else if matches!(stmt.body.as_ref(), Statement::EmptyStatement(_)) {
            // Prettier's `adjustClause` returns `";"` directly for an empty body
            // → `do;`, not `do ;`.
            vec![d.text("do"), self.build_statement_doc(&stmt.body)]
        } else {
            vec![d.text("do "), self.build_statement_doc(&stmt.body)]
        };

        // Find the while keyword position for comment handling
        // Search forward from body end, skipping over comments to find the actual keyword
        let body_end = stmt.body.span().end;
        let test_start = stmt.test.span().start;
        let while_pos = self.find_keyword_in_source(body_end, test_start, "while");

        // Check for comments between } and while, determine if while stays on same line
        let while_on_same_line = if let Some(while_start) = while_pos
            && self.has_comments_between(body_end, while_start)
        {
            let (inline_prev, own_line, inline_next) =
                self.partition_comments_by_line(body_end, while_start);

            // Merge inline_next (comments on same line as `while`) into own_line
            // so they're emitted before the `while` keyword rather than dropped.
            // e.g. `} \n /* c */ while (cond);` → `}\n/* c */\nwhile (cond);`
            let mut all_own_line = own_line;
            all_own_line.extend(inline_next);

            // Add comments preserving their position
            self.build_comments_between_parts(&mut parts, &inline_prev, &all_own_line, body_end);

            // While stays on same line only if: block body, no own-line comments, all inline are block comments
            let has_inline_line_comment = inline_prev.iter().any(|c| !c.is_block);
            is_block && all_own_line.is_empty() && !has_inline_line_comment
        } else {
            is_block
        };

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

        if while_on_same_line {
            parts.push(d.text(" while"));
        } else {
            parts.push(d.hardline());
            parts.push(d.text("while"));
        }
        if let Some(kc) = keyword_comments {
            parts.push(kc);
        }
        parts.push(d.text(" ("));

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

        // Preserve comments between the condition's `)` and the terminating `;` in
        // place: `} while (x) /* c */;` keeps the comment after `)` (Prettier
        // relocates it inside the parens — see close_paren_comment_prettier_divergence).
        // Mirrors the if-empty path's `append_close_paren_empty_stmt_with_comments`.
        if let Some(close) = close_paren {
            self.append_close_paren_empty_stmt_with_comments(&mut parts, close + 1, stmt.span.end);
        } else {
            parts.push(d.text(");"));
        }
        d.concat(&parts)
    }
}
