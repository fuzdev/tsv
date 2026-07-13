// Program-level printing for TypeScript
//
// Top-level orchestration: statement iteration with blank-line preservation,
// leading/trailing comment placement, and format-ignore raw emission.

use crate::ast::internal;
use tsv_lang::doc::DocBuf;
use tsv_lang::{
    CommentPosition, classify_comment_fast, comments_after, comments_in_range, doc::arena::DocId,
};

use super::Printer;

impl<'a> Printer<'a> {
    /// Print a TypeScript program
    ///
    /// Delegates to `build_program_doc` to build the doc tree, then renders it.
    /// This is the same path used by Svelte's `<script>` formatting, ensuring
    /// consistent behavior (e.g., trailing whitespace trimming in comments).
    pub(crate) fn print_program(&mut self, program: &internal::Program<'_>) {
        let doc = self.build_program_doc(program);
        self.write_arena_doc(doc);
    }

    /// Build a DocId tree for a TypeScript program
    ///
    /// Returns a DocId that can be wrapped with `indent()` and rendered.
    /// Used both for standalone TS/JS formatting (via `print_program`) and
    /// when embedding TypeScript in other formats like Svelte's `<script>`.
    ///
    /// The Doc structure preserves:
    /// - Statement separation with hardline
    /// - Blank line preservation between statements using literalline
    /// - Leading comments with proper spacing
    /// - Trailing same-line comments using line_suffix
    /// - Program trailing comments after the last statement
    pub(crate) fn build_program_doc(&self, program: &internal::Program<'_>) -> DocId {
        let d = self.d();
        let mut parts = d.pooled_docbuf();
        let mut prev_end = 0u32;
        let mut has_output = false;

        for (stmt_idx, statement) in program.body.iter().enumerate() {
            // Skip standalone EmptyStatements but preserve blank lines and comments around them
            if matches!(statement, internal::Statement::EmptyStatement(_)) {
                // Extend the search range to include trailing same-line comments of the
                // empty statement. Without this, `; /* comment */` loses the comment.
                let stmt_end = statement.span().end;
                let trailing_end = self.find_end_with_trailing_comments(stmt_end).max(stmt_end);
                // Use the extended range (covers same-line trailing comments) but cap at
                // next statement's start to avoid capturing comments that belong to the next stmt.
                let next_start = program
                    .body
                    .get(stmt_idx + 1)
                    .map_or(program.span.end, |s| s.span().start);
                let search_end = trailing_end.max(stmt_end).min(next_start);

                // Force non-inline: since we're skipping the semicolon, any "inline" comments
                // (on same line as the semicolon) have nothing to be inline with
                let comments_doc =
                    self.build_leading_comments_doc(prev_end, search_end, !has_output, true);
                if let Some(comments_doc) = comments_doc {
                    if has_output {
                        // Check for blank line before the first comment (same as regular statements)
                        let first_comment_start =
                            comments_in_range(self.comments, prev_end, search_end)
                                .next()
                                .map(|c| c.span.start);
                        let check_end = first_comment_start.unwrap_or_else(|| statement.span().end);

                        if self.has_blank_line_between(prev_end, check_end) {
                            parts.push(d.literalline()); // Blank line at column 0
                        }
                        parts.push(d.hardline()); // Separator with indent
                    }
                    parts.push(comments_doc);
                    has_output = true;
                }
                prev_end = search_end;
                continue;
            }

            // Separator between statements
            if has_output {
                // Check for blank line before the next item:
                // - If there are comments, check before the first comment
                // - If no comments, check before the statement
                let first_comment_start =
                    comments_in_range(self.comments, prev_end, statement.span().start)
                        .next()
                        .map(|c| c.span.start);
                let check_end = first_comment_start.unwrap_or_else(|| statement.span().start);

                if self.has_blank_line_between(prev_end, check_end) {
                    parts.push(d.literalline()); // Blank line at column 0
                }

                parts.push(d.hardline()); // Separator with indent
            }

            // Leading comments (allow inline comments since statement will be printed)
            let has_ignore = self.has_format_ignore_in_range(prev_end, statement.span().start);
            if let Some(leading_doc) = self.build_leading_comments_doc(
                prev_end,
                statement.span().start,
                !has_output,
                false,
            ) {
                parts.push(leading_doc);
            }

            // Statement — if preceded by a format-ignore directive, emit raw source.
            // A Program's body is always directive-prologue eligible.
            if has_ignore {
                parts.push(self.raw_source_doc(statement.span()));
            } else {
                parts.push(self.build_statement_doc(statement, true));
            }

            // Trailing same-line comments. Bound the scan by the next statement's
            // start so a comment only attaches to the statement it immediately
            // follows — multiple statements on one source line (`a(); b(); // c`)
            // must not each grab the trailing comment.
            let next_start = program
                .body
                .get(stmt_idx + 1)
                .map_or(program.span.end, |s| s.span().start);
            let trailing_docs =
                self.build_trailing_same_line_comment_docs(statement.span().end, next_start);
            parts.extend(trailing_docs);

            // Update prev_end to be after any trailing same-line comments
            // This ensures blank line detection works correctly
            prev_end = self.find_end_with_trailing_comments(statement.span().end);
            has_output = true;
        }

        // Trailing program comments
        let trailing_comments_doc = self.build_program_trailing_comments_doc(prev_end);
        if !trailing_comments_doc.is_empty() {
            has_output = true;
        }
        parts.extend(trailing_comments_doc);

        // Trailing newline (only if there's content — empty files stay empty)
        if has_output {
            parts.push(d.hardline());
        }

        d.concat(&parts)
    }

    /// Build doc for leading comments between prev_end and curr_start
    ///
    /// Returns a Doc containing all leading comments with proper blank line handling.
    /// Returns empty doc if no comments.
    ///
    /// Structure: Each comment is output WITHOUT a trailing hardline.
    /// Separators (hardline or literalline+hardline) are added BEFORE each subsequent
    /// comment and AFTER the last comment (to separate from the statement).
    ///
    /// When `force_non_inline` is true, all comments are treated as non-inline (own line).
    /// This is used for empty statements that will be skipped - their inline comments
    /// have nothing to be inline with.
    fn build_leading_comments_doc(
        &self,
        prev_end: u32,
        curr_start: u32,
        is_first: bool,
        force_non_inline: bool,
    ) -> Option<DocId> {
        let d = self.d();
        let mut parts = d.pooled_docbuf();
        let mut last_comment_end = prev_end;
        let mut printed_any = false;
        let mut last_was_inline = false;

        for comment in comments_in_range(self.comments, prev_end, curr_start) {
            let position =
                classify_comment_fast(comment, prev_end, curr_start, self.comment_line_breaks);

            // Skip trailing comments EXCEPT for first statement (file start)
            if !is_first && matches!(position, CommentPosition::Trailing) {
                last_comment_end = comment.span.end;
                continue;
            }

            // Handle inline leading comments (same line as statement)
            // These stay on the same line, so DON'T set printed_any (no separator needed)
            // Skip this behavior when force_non_inline is true (e.g., empty statements being skipped)
            //
            // Also handle block comments classified as Trailing that are on the same line as
            // curr_start when is_first. This happens with consecutive inline block comments
            // at file start: `/** @type {A} */ /** @type {B} */ expr;` — classify_comment_fast
            // returns Trailing (same line as prev_end=0) but these should stay inline with
            // the expression since they're also on the same line as curr_start.
            let is_inline = matches!(position, CommentPosition::LeadingInline)
                || (is_first
                    && comment.is_block
                    && matches!(position, CommentPosition::Trailing)
                    && self.is_same_line(comment.span.end, curr_start));
            if !force_non_inline && is_inline {
                // If a previous comment was printed on a DIFFERENT line, add a line break.
                // E.g., `// line comment\n/** @type {A} */ expr;` — needs newline after
                // the line comment. But consecutive inline comments on the SAME line
                // should stay inline: `/** @type {A} */ /** @type {B} */ expr;`.
                if printed_any && !self.is_same_line(last_comment_end, comment.span.start) {
                    let has_blank = comment.span.start > last_comment_end
                        && self.has_blank_line_between(last_comment_end, comment.span.start);
                    if has_blank {
                        parts.push(d.literalline());
                    }
                    parts.push(d.hardline());
                }
                parts.push(self.build_comment_doc(comment));
                parts.push(d.text(" "));
                // DON'T set printed_any - inline comments don't need separators
                last_comment_end = comment.span.end;
                last_was_inline = true;
                continue;
            }

            // Comment on its own line: check for blank lines BETWEEN comments
            // Note: blank line before FIRST comment is handled by the parent (build_program_doc)
            // We only handle blank lines between subsequent comments here
            //
            // Special case: when the previous comment was a multi-line block comment,
            // a comment on the same line as its closing */ stays inline (e.g.,
            // `/*\ncomment\n*/ /* after */` keeps `/* after */` on the `*/` line).
            if printed_any && self.is_same_line(last_comment_end, comment.span.start) {
                // Same line as previous comment's end — keep inline
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else {
                let has_blank_before = printed_any
                    && comment.span.start > last_comment_end
                    && self.has_blank_line_between(last_comment_end, comment.span.start);

                // Add separator BEFORE this comment (first comment has no separator - parent's hardline handles it)
                if has_blank_before {
                    parts.push(d.literalline()); // Blank line at column 0
                    parts.push(d.hardline()); // Indent for this comment
                } else if printed_any {
                    parts.push(d.hardline()); // Separator from previous comment
                }

                parts.push(self.build_comment_doc(comment));
            }
            // NO hardline after comment - let post-loop or next iteration handle it

            last_comment_end = comment.span.end;
            printed_any = true;
            last_was_inline = false;
        }

        // After all comments: add separator for the statement (if one follows)
        // Skip this when force_non_inline is true - that means the statement is being skipped
        // and there's nothing for the separator to separate from.
        // Skip when last comment was inline - it already has trailing space and the
        // statement continues on the same line: `/** @type {A} */ expr;`
        if printed_any && !force_non_inline && !last_was_inline {
            // Check if there's a blank line after the last comment
            let has_blank_after = last_comment_end < curr_start
                && self.has_blank_line_between(last_comment_end, curr_start);

            if has_blank_after {
                parts.push(d.literalline()); // Blank line at column 0
            }
            parts.push(d.hardline()); // Indent for statement
        }

        if parts.is_empty() {
            None
        } else {
            Some(d.concat(&parts))
        }
    }

    /// Build docs for trailing comments at the end of the program
    ///
    /// Handles comments that appear after all statements but before end of file.
    fn build_program_trailing_comments_doc(&self, prev_end: u32) -> DocBuf {
        let d = self.d();
        let mut docs = DocBuf::new();
        let mut last_comment_end = prev_end;
        let mut is_first_comment = true;

        for comment in comments_after(self.comments, prev_end) {
            // Skip comments on same line as prev_end - those are inline trailing comments
            // already handled by build_trailing_same_line_comment_docs
            // BUT: When prev_end == 0 (no statements), there's no previous statement to be
            // trailing from, so comments at position 0 should NOT be skipped.
            if prev_end > 0 && self.is_same_line(prev_end, comment.span.start) {
                last_comment_end = comment.span.end;
                continue;
            }

            // For comments-only files (no statements), don't add leading newline for first comment
            if prev_end > 0 || !is_first_comment {
                // Blank line before this comment (add literalline BEFORE hardline)
                if self.has_blank_line_between(last_comment_end, comment.span.start) {
                    docs.push(d.literalline());
                }

                docs.push(d.hardline());
            }

            docs.push(self.build_comment_doc(comment));
            last_comment_end = comment.span.end;
            is_first_comment = false;
        }

        docs
    }
}
