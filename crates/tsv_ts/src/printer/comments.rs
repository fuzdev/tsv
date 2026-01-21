// Comment handling for TypeScript printer
//
// This module handles all comment-related operations:
// - Building Doc representations for comments
// - Printing comments directly to buffer
// - Finding and filtering comments in ranges
// - Handling leading/trailing/inline comments

use super::Printer;
use super::analysis::skip_string_or_comment;
use crate::ast::internal;
use tsv_lang::doc::{self, Doc};
use tsv_lang::{CommentPosition, classify_comment, comments_in_range, printing};

/// Spacing style for comments in doc building
#[derive(Debug, Clone, Copy)]
pub(crate) enum CommentSpacing {
    /// Space before comment: ` /* c */`
    Leading,
    /// Space after comment: `/* c */ `
    Trailing,
    /// No spacing: `/* c */`
    None,
}

/// Filter for which comment types to include
#[derive(Debug, Clone, Copy)]
pub(crate) enum CommentFilter {
    /// Include all comments (block and line)
    All,
    /// Only include block comments (/* */)
    BlockOnly,
}

impl<'a> Printer<'a> {
    /// Find the position of the next comma delimiter after the given position
    ///
    /// Used to distinguish trailing comments (before comma) from leading comments (after comma)
    /// in arrays and objects. Skips over comments and strings to find the actual delimiter comma.
    ///
    /// Returns None if no comma found.
    ///
    /// Example: `[A /* , */ , B]` - finds the second comma, not the one in the comment
    pub(crate) fn find_comma_after(&self, pos: u32) -> Option<u32> {
        let source = self.source.as_bytes();
        let mut i = pos as usize;
        let end = source.len();

        while i < end {
            match source[i] {
                b',' => return Some(i as u32),
                _ => {
                    if let Some(skip) = skip_string_or_comment(source, i, end) {
                        i = skip;
                    }
                }
            }
            i += 1;
        }
        None
    }

    /// Get the search start position for leading comments on list elements
    ///
    /// For the first element, returns `prev_end` (search starts after opening delimiter).
    /// For subsequent elements, returns position after the comma, or `prev_end` if no comma found.
    ///
    /// This ensures that comments after a comma are treated as leading on the next element,
    /// not trailing on the previous element.
    pub(crate) fn leading_comment_search_start(&self, prev_end: u32, is_first: bool) -> u32 {
        if is_first {
            prev_end
        } else {
            self.find_comma_after(prev_end)
                .map_or(prev_end, |pos| pos + 1)
        }
    }

    /// Print a TypeScript comment
    pub(crate) fn print_comment(&mut self, comment: &internal::Comment) {
        if comment.is_block {
            // Block comment: /* content */
            self.write("/*");

            if comment.content.contains('\n') {
                // JSDoc comments (start with *) get re-indented to match context
                // Other multi-line comments preserve original indentation
                let is_jsdoc = comment.content.starts_with('*');

                if is_jsdoc {
                    // JSDoc: strip original indentation and re-apply context indent
                    let stripped = printing::strip_comment_indentation(
                        self.source,
                        &comment.content,
                        comment.span.start,
                    );
                    let lines: Vec<&str> = stripped.split('\n').collect();

                    for (i, line) in lines.iter().enumerate() {
                        let is_last = i == lines.len() - 1;
                        if i > 0 {
                            self.write("\n");
                            // Blank lines inside comments should be truly empty (no indentation)
                            // But the closing line (last line before */) needs context indent
                            if !line.is_empty() || is_last {
                                self.write_indent();
                            }
                        }
                        self.write(line);
                    }
                } else {
                    // Regular block comment: preserve original indentation
                    // The outer formatter (e.g., Svelte) is responsible for not
                    // adding extra indent to continuation lines.
                    let lines: Vec<&str> = comment.content.split('\n').collect();

                    // Prettier adds a space before */ when:
                    // 1. There are exactly 2 lines (just newline between /* and */)
                    // 2. The closing line is empty
                    let last_line = lines.last().unwrap_or(&"");
                    let add_closing_space = lines.len() == 2 && last_line.is_empty();

                    for (i, line) in lines.iter().enumerate() {
                        let is_last = i == lines.len() - 1;
                        if i > 0 {
                            self.write("\n");
                        }
                        if is_last && add_closing_space {
                            // Skip empty last line content, space will be added below
                        } else {
                            self.write(line);
                        }
                    }

                    // Add space before */ for empty closing line (prettier behavior)
                    if add_closing_space {
                        self.write(" ");
                    }
                }
            } else {
                // Single-line block comment
                self.write(&comment.content);
            }

            self.write("*/");
        } else if comment.span.start == 0 && comment.content.starts_with("#!") {
            // Hashbang comment: #!/usr/bin/env node (no // prefix)
            // Content already includes the #! prefix
            self.write(&comment.content);
        } else {
            // Line comment: // content (no closing delimiter)
            self.write("//");
            self.write(&comment.content);
        }
    }

    /// Print leading comments (comments between prev_end and curr_start)
    /// Returns true if any comments were printed
    ///
    /// - `prev_end`: Position after the previous statement (or 0 for first statement)
    /// - `curr_start`: Position of the current statement
    /// - `is_first`: True if this is the first statement (prev_end is start of file)
    ///
    /// Uses binary search to find starting point: O(log n + k)
    pub(crate) fn print_leading_comments(
        &mut self,
        prev_end: u32,
        curr_start: u32,
        is_first: bool,
    ) -> bool {
        let mut last_comment_end = prev_end;
        let mut printed_any = false;

        for comment in comments_in_range(self.comments, prev_end, curr_start) {
            let position = classify_comment(comment, prev_end, curr_start, self.source);

            // Skip trailing comments EXCEPT for first statement (file start)
            // Still track the comment end so blank line preservation works after trailing comments
            if !is_first && matches!(position, CommentPosition::Trailing) {
                last_comment_end = comment.span.end;
                continue;
            }

            // Handle inline leading comments (same line as statement)
            if matches!(position, CommentPosition::LeadingInline) {
                self.write_indent();
                self.print_comment(comment);
                self.write(" ");
                printed_any = true;
                last_comment_end = comment.span.end;
                continue;
            }

            // Comment on its own line: check for blank lines
            // Skip blank line preservation at the very start of the program
            // (before the first comment/statement). Prettier removes leading blank lines.
            let is_program_start = is_first && !printed_any;
            if !is_program_start
                && comment.span.start > last_comment_end
                && printing::has_blank_line_between(
                    self.source,
                    last_comment_end,
                    comment.span.start,
                )
            {
                self.write("\n");
            }

            self.write_indent();
            self.print_comment(comment);
            self.write("\n");

            last_comment_end = comment.span.end;
            printed_any = true;
        }

        // Check if there's a blank line after the last comment and before curr_start
        // Use last_comment_end > prev_end to detect if we encountered any comments
        // (including trailing comments that were skipped but still updated last_comment_end)
        if last_comment_end > prev_end
            && last_comment_end < curr_start
            && printing::has_blank_line_between(self.source, last_comment_end, curr_start)
        {
            self.write("\n");
        }

        printed_any
    }

    /// Build a Doc for inline comments between two positions with specified spacing and filter
    ///
    /// Returns a Doc containing all comments in the range with the specified spacing.
    /// Returns empty concat if no comments found.
    ///
    /// Uses binary search to find starting point: O(log n + k)
    pub(crate) fn build_comments_between(
        &self,
        start: u32,
        end: u32,
        spacing: CommentSpacing,
    ) -> Doc {
        self.build_comments_between_filtered(start, end, spacing, CommentFilter::All)
    }

    /// Build a Doc for inline comments with filtering
    pub(crate) fn build_comments_between_filtered(
        &self,
        start: u32,
        end: u32,
        spacing: CommentSpacing,
        filter: CommentFilter,
    ) -> Doc {
        let mut parts = Vec::new();
        for comment in comments_in_range(self.comments, start, end) {
            // Apply filter
            if matches!(filter, CommentFilter::BlockOnly) && !comment.is_block {
                continue;
            }

            match spacing {
                CommentSpacing::Leading => {
                    parts.push(doc::text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
                CommentSpacing::Trailing => {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(doc::text(" "));
                }
                CommentSpacing::None => {
                    parts.push(self.build_comment_doc(comment));
                }
            }
        }
        doc::concat(parts)
    }

    /// Build a Doc for inline comments between two positions (leading space)
    #[inline]
    pub(crate) fn build_inline_comments_between_doc(&self, start: u32, end: u32) -> Doc {
        self.build_comments_between(start, end, CommentSpacing::Leading)
    }

    /// Build a Doc for inline comments between two positions (no spaces)
    #[inline]
    pub(crate) fn build_inline_comments_between_doc_no_leading_space(
        &self,
        start: u32,
        end: u32,
    ) -> Doc {
        self.build_comments_between(start, end, CommentSpacing::None)
    }

    /// Build a Doc for inline comments between two positions (trailing space)
    ///
    /// Used when comments appear before an element and need a space after.
    /// Example: `{a, /* comment */ b}` - the comment needs a space after it.
    #[inline]
    pub(crate) fn build_inline_comments_between_doc_trailing_space(
        &self,
        start: u32,
        end: u32,
    ) -> Doc {
        self.build_comments_between(start, end, CommentSpacing::Trailing)
    }

    /// Build docs for leading comments in a forced-multiline context.
    ///
    /// Block comments: `/*content*/ ` (inline with trailing space)
    /// Line comments: `//content` + hardline (on own line)
    ///
    /// Used when line comments force multiline formatting (unions, tuples, etc.)
    pub(crate) fn build_leading_comments_multiline(&self, start: u32, end: u32) -> Vec<Doc> {
        let mut parts = Vec::new();
        for comment in comments_in_range(self.comments, start, end) {
            parts.push(self.build_comment_doc(comment));
            if comment.is_block {
                parts.push(doc::text(" "));
            } else {
                parts.push(doc::hardline());
            }
        }
        parts
    }

    /// Build docs for trailing comments in a forced-multiline context.
    ///
    /// Same-line comments (block or line): ` /*content*/` or ` //content` (inline with leading space)
    /// Own-line comments: hardline + comment (on their own line)
    ///
    /// Used when line comments force multiline formatting (unions, tuples, etc.)
    pub(crate) fn build_trailing_comments_multiline(&self, start: u32, end: u32) -> Vec<Doc> {
        let mut parts = Vec::new();
        for comment in comments_in_range(self.comments, start, end) {
            if printing::is_same_line(self.source, start, comment.span.start) {
                // Same line as start: trailing comment (both block and line)
                parts.push(doc::text(" "));
                parts.push(self.build_comment_doc(comment));
            } else {
                // Own line comment (block or line)
                parts.push(doc::hardline());
                parts.push(self.build_comment_doc(comment));
            }
        }
        parts
    }

    /// Filter line comments between two positions based on whether they're on the same line as start
    ///
    /// # Arguments
    /// * `start` - Start position (e.g., end of previous chain element)
    /// * `end` - End position (e.g., start of next chain element)
    /// * `same_line` - If true, returns comments on same line as start; if false, returns comments on their own lines
    pub(crate) fn filter_line_comments(
        &self,
        start: u32,
        end: u32,
        same_line: bool,
    ) -> Vec<&internal::Comment> {
        comments_in_range(self.comments, start, end)
            .filter(|c| !c.is_block)
            .filter(|c| same_line == printing::is_same_line(self.source, start, c.span.start))
            .collect()
    }

    /// Check if there's a newline between start position and the first comment in the range
    ///
    /// Returns true if there's at least one comment in the range and a newline
    /// exists between `start` and the first comment's start position.
    pub(crate) fn has_newline_before_comment(&self, start: u32, end: u32) -> bool {
        let first_idx = tsv_lang::find_first_comment_from(self.comments, start);
        if let Some(comment) = self.comments.get(first_idx)
            && comment.span.end <= end
        {
            // Check if there's a newline between start and comment start
            let between = &self.source[start as usize..comment.span.start_usize()];
            return between.contains('\n');
        }
        false
    }

    /// Check if there's a block comment on its own line within a container.
    ///
    /// A "standalone" block comment is one that:
    /// - Is not on the same line as the opening brace
    /// - Is not on the same line as any item (start or end)
    ///
    /// Used to force multiline formatting for objects/type literals.
    pub(crate) fn has_standalone_block_comment(
        &self,
        container_start: u32,
        container_end: u32,
        item_spans: &[tsv_lang::Span],
    ) -> bool {
        let after_open_brace = container_start + 1;
        comments_in_range(self.comments, container_start, container_end).any(|c| {
            if !c.is_block {
                return false; // Line comments handled separately
            }
            // Must not be on same line as opening brace
            if printing::is_same_line(self.source, after_open_brace, c.span.start) {
                return false;
            }
            // Must not be on same line as any item
            !item_spans.iter().any(|s| {
                printing::is_same_line(self.source, s.start, c.span.start)
                    || printing::is_same_line(self.source, s.end, c.span.start)
            })
        })
    }

    /// Build docs for trailing same-line comments after a node
    ///
    /// Line comments are wrapped in `line_suffix` so they don't affect width
    /// calculations for preceding groups (matches Prettier behavior).
    /// Block comments are inline and do affect width.
    ///
    /// Returns a Vec of docs to append to the current parts.
    pub(crate) fn build_trailing_same_line_comment_docs(
        &self,
        after_pos: u32,
        upper_bound: u32,
    ) -> Vec<Doc> {
        let mut docs = Vec::new();
        for comment in comments_in_range(self.comments, after_pos, upper_bound) {
            if printing::is_same_line(self.source, after_pos, comment.span.start) {
                if comment.is_block {
                    // Block comments are inline, affect width
                    docs.push(doc::text(" "));
                    docs.push(self.build_comment_doc(comment));
                } else {
                    // Line comments go in line_suffix, don't affect width
                    docs.push(self.build_trailing_line_comment_doc(comment));
                }
            } else {
                break; // Only same-line comments
            }
        }
        docs
    }

    /// Build docs for leading comments before a node with blank line preservation.
    ///
    /// Handles comments that appear before a member/statement, preserving blank lines
    /// between consecutive comments and after the last comment. Returns a Vec of docs
    /// to append directly before the target node.
    ///
    /// Used by: class body members, block statement bodies, interface members, type literals.
    pub(crate) fn build_leading_comments_with_blank_lines(
        &self,
        comments: &[&internal::Comment],
        target_start: u32,
    ) -> Vec<Doc> {
        if comments.is_empty() {
            return Vec::new();
        }

        // Check if there's a blank line after the last comment
        let has_blank_after_last_comment = comments.last().is_some_and(|c| {
            printing::has_blank_line_between(self.source, c.span.end, target_start)
        });

        let mut docs = Vec::new();
        let mut last_pos = comments[0].span.start;

        for (j, comment) in comments.iter().enumerate() {
            let is_last_comment = j == comments.len() - 1;

            // Check if there's a blank line after this comment
            // (to next comment or to target if last comment)
            let has_blank_after = if is_last_comment {
                has_blank_after_last_comment
            } else {
                printing::has_blank_line_between(
                    self.source,
                    comment.span.end,
                    comments[j + 1].span.start,
                )
            };

            // For subsequent comments, check for blank lines between them
            if j > 0 && printing::has_blank_line_between(self.source, last_pos, comment.span.start)
            {
                docs.push(doc::literalline());
                docs.push(doc::hardline());
            }

            docs.push(self.build_comment_doc(comment));

            if !comment.is_block {
                // Line comment: add hardline after unless there's a blank line after
                // (the blank line separator will handle it)
                if !has_blank_after {
                    docs.push(doc::hardline());
                }
            } else if !printing::is_same_line(self.source, comment.span.end, target_start) {
                // Block comment on its own line: add hardline unless there's blank after
                if !has_blank_after {
                    docs.push(doc::hardline());
                }
            } else {
                // Block comment on same line as target - space before
                docs.push(doc::text(" "));
            }
            last_pos = comment.span.end;
        }

        // Add blank line after last comment if present
        if has_blank_after_last_comment {
            docs.push(doc::literalline());
            docs.push(doc::hardline());
        }

        docs
    }

    /// Build docs for trailing comments at the end of a body (before closing `}`).
    ///
    /// Handles comments that appear after the last member/statement in a body,
    /// with blank line preservation between them. Returns a Vec of docs to append.
    ///
    /// Used by: class body, interface body, enum body, type literal, namespace body.
    pub(crate) fn build_trailing_body_comments_doc(
        &self,
        prev_end: u32,
        body_end: u32,
    ) -> Vec<Doc> {
        let trailing_comments: Vec<_> = comments_in_range(self.comments, prev_end, body_end)
            .filter(|c| !printing::is_same_line(self.source, prev_end, c.span.start))
            .collect();

        if trailing_comments.is_empty() {
            return Vec::new();
        }

        let mut docs = Vec::new();

        // Check for blank line before the first trailing comment
        let first_comment = trailing_comments[0];
        if printing::has_blank_line_between(self.source, prev_end, first_comment.span.start) {
            docs.push(doc::literalline());
        }
        docs.push(doc::hardline());

        // Process each trailing comment
        let mut last_pos = prev_end;
        for (j, comment) in trailing_comments.iter().enumerate() {
            let is_last = j == trailing_comments.len() - 1;

            // Check for blank lines between comments
            if j > 0 && printing::has_blank_line_between(self.source, last_pos, comment.span.start)
            {
                docs.push(doc::literalline());
                docs.push(doc::hardline());
            }

            // Check if there's a blank line after this comment (to next comment)
            let has_blank_after = !is_last
                && printing::has_blank_line_between(
                    self.source,
                    comment.span.end,
                    trailing_comments[j + 1].span.start,
                );

            docs.push(self.build_comment_doc(comment));

            // Line comment - add hardline after unless:
            // - It's the last comment (closing brace follows)
            // - There's a blank line after (the blank line separator handles it)
            if !comment.is_block && !is_last && !has_blank_after {
                docs.push(doc::hardline());
            }
            // Block comments don't need hardline after in this context
            // (the closing brace follows immediately)

            last_pos = comment.span.end;
        }

        docs
    }

    /// Build a Doc for a single comment
    ///
    /// For multi-line block comments:
    /// - JSDoc comments (/**) always use hardline to apply context indent
    /// - Other comments: if continuation lines had indentation, use hardline; otherwise literalline
    pub(crate) fn build_comment_doc(&self, comment: &internal::Comment) -> Doc {
        if comment.is_block {
            // Block comment: /* content */
            if comment.content.contains('\n') {
                // Multi-line block comment - strip original indentation
                let stripped = printing::strip_comment_indentation(
                    self.source,
                    &comment.content,
                    comment.span.start,
                );

                // JSDoc comments (start with *) always get context indent
                // Other comments: use hardline if indentation was stripped, literalline otherwise
                let is_jsdoc = comment.content.starts_with('*');
                let had_indentation = stripped.len() != comment.content.len();
                let use_context_indent = is_jsdoc || had_indentation;

                let lines: Vec<&str> = stripped.split('\n').collect();
                let mut line_docs = Vec::new();
                for (i, line) in lines.iter().enumerate() {
                    let is_last = i == lines.len() - 1;
                    if i > 0 {
                        // Blank lines inside comments should be truly empty (no indentation)
                        // But the closing line (last line before */) needs context indent
                        if line.is_empty() && !is_last {
                            line_docs.push(doc::literalline());
                        } else if use_context_indent {
                            // Apply context indent for content lines and closing line
                            line_docs.push(doc::hardline());
                        } else {
                            // Preserve at column 0
                            line_docs.push(doc::literalline());
                        }
                    }
                    if i == 0 {
                        line_docs.push(doc::text_owned(format!("/*{line}")));
                    } else {
                        line_docs.push(doc::text_owned((*line).to_string()));
                    }
                }
                line_docs.push(doc::text("*/"));
                doc::concat(line_docs)
            } else {
                // Single-line block comment
                doc::text_owned(format!("/*{}*/", comment.content))
            }
        } else {
            // Line comment: // content
            doc::text_owned(format!("//{}", comment.content))
        }
    }

    /// Build a line_suffix doc for a trailing line comment (space + comment)
    ///
    /// Wrapping in line_suffix excludes the comment from width calculations,
    /// so elements can stay compact even when the trailing comment would push
    /// the line over print_width.
    pub(crate) fn build_trailing_line_comment_doc(&self, comment: &internal::Comment) -> Doc {
        doc::line_suffix(doc::concat(vec![
            doc::text(" "),
            self.build_comment_doc(comment),
        ]))
    }
}
