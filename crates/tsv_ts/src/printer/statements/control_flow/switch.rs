// switch statement printing
//
// Switch head, case labels, and case-body layout with comment handling.

use crate::ast::internal::{self, Statement};
use crate::printer::Printer;
use smallvec::smallvec;
use tsv_lang::comments_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::{TriviaProfile, find_char, find_char_skipping_comments};

impl<'a> Printer<'a> {
    /// Build a doc for a switch statement with proper line-width wrapping
    ///
    /// Matches Prettier's architecture: the discriminant wraps to multiple lines
    /// when the `switch (discriminant) {` line exceeds print width.
    fn build_switch_statement_with_wrapping_doc(
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
        let paren_brace_comments = match (close_paren, body_open_brace) {
            (Some(close), Some(brace)) if self.has_comments_between(close + 1, brace) => {
                self.build_inline_comments_between_doc_opt(close + 1, brace)
            }
            _ => None,
        };

        // Build condition group (handles breaking within discriminant and comments)
        let condition_group = if let (Some(open), Some(close)) = (open_paren, close_paren) {
            self.build_condition_group_with_comments(&stmt.discriminant, open, close)
        } else {
            self.build_condition_group(&stmt.discriminant)
        };

        // Build cases - they handle their own internal indentation
        // Join cases with hardlines, handling comments between cases
        let mut case_parts = Vec::new();
        // Start after the open brace to find comments between { and first case
        let brace_start = body_open_brace
            .unwrap_or_else(|| close_paren.map_or_else(|| stmt.discriminant.span().end, |p| p + 1));
        let mut prev_end = brace_start + 1;
        let mut prev_case_label_end: Option<u32> = None;
        let mut is_first_item = true;
        for (i, case) in stmt.cases.iter().enumerate() {
            // Handle comments between previous position and this case
            // (includes comments before first case, and between subsequent cases)
            // Skip inline comments that belong to the previous case label (fallthrough cases)
            let comments: Vec<_> =
                comments_in_range(self.comments, prev_end, case.span.start).collect();
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
                // Preserve blank lines before comments (e.g., between `return;` and `// comment`)
                if !is_first_item {
                    if self.has_blank_line_between(last_content_end, comment.span.start) {
                        case_parts.push(d.literalline());
                    }
                    case_parts.push(d.hardline());
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
                    case_parts.push(d.literalline());
                }
                case_parts.push(d.hardline());
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
        let mut last_trailing_end = prev_end;
        for comment in comments_in_range(self.comments, prev_end, switch_end) {
            if !is_first_item {
                if self.has_blank_line_between(last_trailing_end, comment.span.start) {
                    case_parts.push(d.literalline());
                }
                case_parts.push(d.hardline());
            }
            is_first_item = false;
            case_parts.push(self.build_comment_doc(comment));
            last_trailing_end = comment.span.end;
        }

        // Structure: switch (...) { indent([hardline, cases...]) hardline }
        // The indent wraps the hardline so cases start at +1 indent level
        // For empty switch, just output {\n}
        let body_doc = if case_parts.is_empty() {
            d.hardline()
        } else {
            d.concat(&[
                d.indent(d.concat(&[d.hardline(), d.concat(&case_parts)])),
                d.hardline(),
            ])
        };

        let mut switch_parts: DocBuf = smallvec![d.text("switch")];
        if let Some(kc) = keyword_comments {
            switch_parts.push(kc);
        }
        switch_parts.push(d.text(" ("));
        switch_parts.push(condition_group);
        switch_parts.push(d.text(")"));
        if let Some(pbc) = paren_brace_comments {
            switch_parts.push(pbc);
        }
        switch_parts.push(d.text(" {"));
        switch_parts.push(body_doc);
        switch_parts.push(d.text("}"));
        d.group(d.concat(&switch_parts))
    }

    /// Get the end position of a case label (position after the colon)
    fn get_case_label_end(&self, case: &internal::SwitchCase<'_>) -> u32 {
        let bytes = self.source.as_bytes();
        if let Some(test) = &case.test {
            // Find the label ':' after the test expression, skipping any ':' inside
            // a comment (`case 1 /* : */:`).
            let test_end = test.span().end;
            find_char(
                bytes,
                test_end as usize,
                bytes.len(),
                b':',
                TriviaProfile::JS,
            )
            .map_or(test_end + 1, |c| c as u32 + 1)
        } else {
            // "default:" - find the actual ':' position (comment-skipping).
            let start = case.span.start;
            find_char(bytes, start as usize, bytes.len(), b':', TriviaProfile::JS)
                .map_or(start + "default:".len() as u32, |c| c as u32 + 1)
        }
    }

    /// Build a doc for a switch case (without outer indent - that's handled by switch)
    ///
    /// `inline_comment_boundary` is the position up to which we should look for inline comments
    /// on this case label (typically the next case start or switch body end).
    fn build_switch_case_doc_inner(
        &self,
        case: &internal::SwitchCase<'_>,
        inline_comment_boundary: u32,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // case X: or default:
        let case_label_end = self.get_case_label_end(case);

        if let Some(test) = &case.test {
            parts.push(d.text("case "));
            parts.push(self.build_expression_doc(test));
            // Comments between expression and colon: `case 1 /* c */:`
            let test_end = test.span().end;
            let colon_pos = find_char_skipping_comments(
                self.source.as_bytes(),
                test_end as usize,
                case_label_end as usize,
                b':',
            )
            .unwrap_or(case_label_end as usize - 1);
            parts.push(self.build_inline_comments_between_doc(test_end, colon_pos as u32));
            parts.push(d.text(":"));
        } else {
            // Comments between `default` keyword and colon: `default /* c */:`
            let default_keyword_end = case.span.start + "default".len() as u32;
            let colon_pos = find_char_skipping_comments(
                self.source.as_bytes(),
                default_keyword_end as usize,
                case_label_end as usize,
                b':',
            )
            .unwrap_or(case_label_end as usize - 1);
            parts.push(d.text("default"));
            parts.push(
                self.build_inline_comments_between_doc(default_keyword_end, colon_pos as u32),
            );
            parts.push(d.text(":"));
        }

        // Handle inline comments after case label (e.g., `case 1: // comment`)
        // For fallthrough cases (no consequent), use the boundary passed by the switch printer
        let first_stmt_start = case.consequent.first().map(|s| s.span().start);
        let inline_comment_end = first_stmt_start.unwrap_or(inline_comment_boundary);
        let mut has_inline_line_comment = false;
        for comment in comments_in_range(self.comments, case_label_end, inline_comment_end) {
            if self.is_same_line(case_label_end, comment.span.start) {
                // A line comment goes through `line_suffix` (zero width) so it never
                // forces the case test (e.g. a binary expression) to break; it flushes
                // at the consequent's hardline (prettier's `lineSuffix`). A block stays
                // inline, width counted.
                parts.push(self.build_trailing_comment_doc(comment));
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
            let comments: Vec<_> = comments_in_range(self.comments, prev_end, stmt_start).collect();

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
                        parts.push(d.text(" "));
                        parts.push(self.build_comment_doc(comment));
                    }
                    parts.push(d.text(" "));
                    parts.push(self.build_statement_doc(stmt));
                } else if has_inline_line_comment && leading_comments.is_empty() {
                    // Inline line comment, no leading: `case 'a': // comment\n{`
                    // Block at case level (no indent)
                    parts.push(d.hardline());
                    parts.push(self.build_statement_doc(stmt));
                } else {
                    // Leading comments exist - indent both comments and block
                    // e.g., `case 'b':\n  // comment\n  {`
                    let mut stmt_parts: DocBuf = smallvec![d.hardline()];
                    for comment in &leading_comments {
                        stmt_parts.push(self.build_comment_doc(comment));
                        stmt_parts.push(d.hardline());
                    }
                    stmt_parts.push(self.build_statement_doc(stmt));
                    parts.push(d.indent(d.concat(&stmt_parts)));
                }
            } else {
                // Build the indented content for this statement
                let mut stmt_parts: DocBuf = smallvec![d.hardline()];

                // Preserve blank lines between statements within case consequent
                if prev_stmt_end.is_some() {
                    let check_end = leading_comments
                        .first()
                        .map_or(stmt_start, |c| c.span.start);
                    if self.has_blank_line_between(prev_end, check_end) {
                        stmt_parts.push(d.hardline());
                    }
                }

                // Print leading comments before this statement
                for comment in &leading_comments {
                    stmt_parts.push(self.build_comment_doc(comment));
                    if !comment.is_block {
                        // Line comment: add hardline after
                        stmt_parts.push(d.hardline());
                    } else if !self.is_same_line(comment.span.end, stmt_start) {
                        // Block comment not on same line as statement - add hardline
                        stmt_parts.push(d.hardline());
                    } else {
                        // Block comment on same line as statement - add space
                        stmt_parts.push(d.text(" "));
                    }
                }

                stmt_parts.push(self.build_statement_doc(stmt));

                parts.push(d.indent(d.concat(&stmt_parts)));
            }

            prev_end = stmt.span().end;
            prev_stmt_end = Some(stmt.span().end);
        }

        // Note: Trailing comments after the last statement are handled by the switch statement
        // printer since they fall outside the SwitchCase span.

        d.concat(&parts)
    }

    #[inline]
    pub(in crate::printer::statements) fn build_switch_statement_doc(
        &self,
        stmt: &internal::SwitchStatement<'_>,
    ) -> DocId {
        // Delegate to the wrapping version which handles proper indentation structure
        self.build_switch_statement_with_wrapping_doc(stmt)
    }
}
