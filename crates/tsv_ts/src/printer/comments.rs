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
use tsv_lang::doc::arena::DocId;
use tsv_lang::{comments_in_range, printing};

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
    ) -> DocId {
        self.build_comments_between_filtered(start, end, spacing, CommentFilter::All)
    }

    /// Build a Doc for inline comments with filtering
    pub(crate) fn build_comments_between_filtered(
        &self,
        start: u32,
        end: u32,
        spacing: CommentSpacing,
        filter: CommentFilter,
    ) -> DocId {
        self.build_comments_between_filtered_opt(start, end, spacing, filter)
            .unwrap_or_else(|| self.d().empty())
    }

    /// Build a Doc for inline comments with filtering, returning None if no comments.
    ///
    /// This is more efficient than `has_comments_between` + `build_comments_between`
    /// because it uses a single binary search instead of two.
    pub(crate) fn build_comments_between_filtered_opt(
        &self,
        start: u32,
        end: u32,
        spacing: CommentSpacing,
        filter: CommentFilter,
    ) -> Option<DocId> {
        let d = self.d();
        // Single binary search to find first comment
        let first_idx = tsv_lang::find_first_comment_from(self.comments, start);

        // Check if any comments exist in range (considering filter)
        let has_comments = self.comments[first_idx..]
            .iter()
            .take_while(|c| c.span.end <= end)
            .any(|c| !matches!(filter, CommentFilter::BlockOnly) || c.is_block);

        if !has_comments {
            return None;
        }

        // Build docs for matching comments
        let mut parts = Vec::new();
        for comment in self.comments[first_idx..]
            .iter()
            .take_while(|c| c.span.end <= end)
        {
            // Apply filter
            if matches!(filter, CommentFilter::BlockOnly) && !comment.is_block {
                continue;
            }

            match spacing {
                CommentSpacing::Leading => {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
                CommentSpacing::Trailing => {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.text(" "));
                }
                CommentSpacing::None => {
                    parts.push(self.build_comment_doc(comment));
                }
            }
        }
        Some(d.concat(&parts))
    }

    /// Build a Doc for inline comments between two positions (leading space)
    #[inline]
    pub(crate) fn build_inline_comments_between_doc(&self, start: u32, end: u32) -> DocId {
        self.build_comments_between(start, end, CommentSpacing::Leading)
    }

    /// Build a Doc for inline comments, returning None if no comments.
    ///
    /// Use this instead of `has_comments_between` + `build_inline_comments_between_doc`
    /// to avoid redundant binary searches.
    #[inline]
    pub(crate) fn build_inline_comments_between_doc_opt(
        &self,
        start: u32,
        end: u32,
    ) -> Option<DocId> {
        self.build_comments_between_filtered_opt(
            start,
            end,
            CommentSpacing::Leading,
            CommentFilter::All,
        )
    }

    /// Build a Doc for inline comments between two positions (no spaces)
    #[inline]
    pub(crate) fn build_inline_comments_between_doc_no_leading_space(
        &self,
        start: u32,
        end: u32,
    ) -> DocId {
        self.build_comments_between(start, end, CommentSpacing::None)
    }

    /// Build a Doc for inline comments (no spaces), returning None if no comments.
    ///
    /// Use this instead of `has_comments_between` + `build_inline_comments_between_doc_no_leading_space`
    /// to avoid redundant binary searches.
    #[inline]
    pub(crate) fn build_inline_comments_between_doc_no_leading_space_opt(
        &self,
        start: u32,
        end: u32,
    ) -> Option<DocId> {
        self.build_comments_between_filtered_opt(
            start,
            end,
            CommentSpacing::None,
            CommentFilter::All,
        )
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
    ) -> DocId {
        self.build_comments_between(start, end, CommentSpacing::Trailing)
    }

    /// Build inline comments between two positions with line-comment-safe trailing spacing.
    ///
    /// Block comments get a trailing space: `/* comment */ expr`
    /// Line comments get a hardline: `// comment\nexpr`
    ///
    /// This prevents line comments from absorbing the following expression as comment text.
    /// Use for any position where a comment appears before an expression (RHS of `=`,
    /// after keywords like `return`/`await`, after operators like `!`/`...`, etc.).
    pub(crate) fn build_rhs_comments_opt(&self, start: u32, end: u32) -> Option<DocId> {
        let d = self.d();
        let mut parts = Vec::new();
        for comment in comments_in_range(self.comments, start, end) {
            parts.push(self.build_comment_doc(comment));
            if comment.is_block {
                if comment.content.contains('\n') {
                    // Multiline block comment: value starts on next line
                    // Prettier ref: hasLeadingOwnLineComment → break-after-operator
                    parts.push(d.hardline());
                } else {
                    parts.push(d.text(" "));
                }
            } else {
                parts.push(d.hardline());
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(d.concat(&parts))
        }
    }

    /// Detect a block comment that should be promoted from after `=` to before `=`.
    ///
    /// When JSDoc cast parens are stripped (e.g., `var a = /** @type {T} */ (\n\texpr\n)`),
    /// multiple block comments end up after `=`. Prettier places the first one before `=`
    /// when it's on a different source line than the second. Returns the promoted comment's
    /// doc (with leading space) and the end position to use as the new RHS comment start.
    pub(crate) fn promote_block_comment_before_eq(
        &self,
        start: u32,
        end: u32,
    ) -> Option<(DocId, u32)> {
        let d = self.d();
        let blocks: Vec<_> = comments_in_range(self.comments, start, end)
            .filter(|c| c.is_block)
            .collect();
        if blocks.len() >= 2 && !self.is_same_line(blocks[0].span.start, blocks[1].span.start) {
            let doc = d.concat(&[d.text(" "), self.build_comment_doc(blocks[0])]);
            Some((doc, blocks[0].span.end))
        } else {
            None
        }
    }

    /// Prepend comments from removed parentheses to a doc.
    ///
    /// When parentheses are removed during parsing (e.g., `(/* comment */ expr)` becomes `expr`),
    /// the expression's span extends to include the removed parens. Comments between
    /// `outer_start` (the paren) and `inner_start` (the expression) need to be preserved.
    ///
    /// Returns the original doc unchanged if no comments or if `outer_start >= inner_start`.
    #[inline]
    pub(crate) fn prepend_removed_paren_comments(
        &self,
        outer_start: u32,
        inner_start: u32,
        doc: DocId,
    ) -> DocId {
        if outer_start < inner_start {
            if let Some(comments) = self.build_rhs_comments_opt(outer_start, inner_start) {
                let d = self.d();
                d.concat(&[comments, doc])
            } else {
                doc
            }
        } else {
            doc
        }
    }

    /// Build docs for leading comments in a forced-multiline context.
    ///
    /// Block comments: `/*content*/ ` (inline with trailing space)
    /// Line comments: `//content` + hardline (on own line)
    ///
    /// Used when line comments force multiline formatting (unions, tuples, etc.)
    pub(crate) fn build_leading_comments_multiline(&self, start: u32, end: u32) -> Vec<DocId> {
        let d = self.d();
        let mut parts = Vec::new();
        for comment in comments_in_range(self.comments, start, end) {
            parts.push(self.build_comment_doc(comment));
            if comment.is_block {
                parts.push(d.text(" "));
            } else {
                parts.push(d.hardline());
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
    pub(crate) fn build_trailing_comments_multiline(&self, start: u32, end: u32) -> Vec<DocId> {
        let d = self.d();
        let mut parts = Vec::new();
        for comment in comments_in_range(self.comments, start, end) {
            if self.is_same_line(start, comment.span.start) {
                // Same line as start: trailing comment (both block and line)
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else {
                // Own line comment (block or line)
                parts.push(d.hardline());
                parts.push(self.build_comment_doc(comment));
            }
        }
        parts
    }
    /// Filter block comments between two positions based on whether they're on the same line as start
    ///
    /// # Arguments
    /// * `start` - Start position (e.g., end of previous chain element)
    /// * `end` - End position (e.g., start of next chain element)
    /// * `same_line` - If true, returns comments on same line as start; if false, returns comments on their own lines
    pub(crate) fn filter_block_comments(
        &self,
        start: u32,
        end: u32,
        same_line: bool,
    ) -> Vec<&internal::Comment> {
        comments_in_range(self.comments, start, end)
            .filter(|c| c.is_block)
            .filter(|c| same_line == self.is_same_line(start, c.span.start))
            .collect()
    }

    /// Check if there's a newline between start position and the first comment in the range
    ///
    /// Returns true if there's at least one comment in the range and a newline
    /// exists between `start` and the first comment's start position.
    /// Check if ALL comments in the range are inline block comments on the same line as `end`.
    ///
    /// Returns true when every comment is a block comment AND on the same line as `end`
    /// (the next expression). Used to keep `/** @type {T} */ arg` as a unit.
    /// Returns false for line comments or block comments on their own line.
    pub(crate) fn all_comments_are_inline_block(&self, start: u32, end: u32) -> bool {
        let first_idx = tsv_lang::find_first_comment_from(self.comments, start);
        let mut found_any = false;
        for comment in self.comments[first_idx..]
            .iter()
            .take_while(|c| c.span.end <= end)
        {
            found_any = true;
            if !comment.is_block || !self.is_same_line(comment.span.end, end) {
                return false;
            }
        }
        found_any
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
            if self.is_same_line(after_open_brace, c.span.start) {
                return false;
            }
            // Must not be on same line as any item
            !item_spans.iter().any(|s| {
                self.is_same_line(s.start, c.span.start) || self.is_same_line(s.end, c.span.start)
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
    ) -> Vec<DocId> {
        let d = self.d();
        let mut docs = Vec::new();
        for comment in comments_in_range(self.comments, after_pos, upper_bound) {
            if self.is_same_line(after_pos, comment.span.start) {
                if comment.is_block {
                    // Block comments are inline, affect width
                    docs.push(d.text(" "));
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
    ) -> Vec<DocId> {
        if comments.is_empty() {
            return Vec::new();
        }

        let d = self.d();

        // Check if there's a blank line after the last comment
        let has_blank_after_last_comment = comments
            .last()
            .is_some_and(|c| self.has_blank_line_between(c.span.end, target_start));

        let mut docs = Vec::new();
        let mut last_pos = comments[0].span.start;

        for (j, comment) in comments.iter().enumerate() {
            let is_last_comment = j == comments.len() - 1;

            // Check if there's a blank line after this comment
            // (to next comment or to target if last comment)
            let has_blank_after = if is_last_comment {
                has_blank_after_last_comment
            } else {
                self.has_blank_line_between(comment.span.end, comments[j + 1].span.start)
            };

            // For subsequent comments, check for blank lines between them
            if j > 0 && self.has_blank_line_between(last_pos, comment.span.start) {
                docs.push(d.literalline());
                docs.push(d.hardline());
            }

            docs.push(self.build_comment_doc(comment));

            if !comment.is_block {
                // Line comment: add hardline after unless there's a blank line after
                // (the blank line separator will handle it)
                if !has_blank_after {
                    docs.push(d.hardline());
                }
            } else if !self.is_same_line(comment.span.end, target_start) {
                // Block comment on its own line: add hardline unless there's blank after
                if !has_blank_after {
                    docs.push(d.hardline());
                }
            } else {
                // Block comment on same line as target - space before
                docs.push(d.text(" "));
            }
            last_pos = comment.span.end;
        }

        // Add blank line after last comment if present
        if has_blank_after_last_comment {
            docs.push(d.literalline());
            docs.push(d.hardline());
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
    ) -> Vec<DocId> {
        let trailing_comments: Vec<_> = comments_in_range(self.comments, prev_end, body_end)
            .filter(|c| !self.is_same_line(prev_end, c.span.start))
            .collect();

        if trailing_comments.is_empty() {
            return Vec::new();
        }

        let d = self.d();
        let mut docs = Vec::new();

        // Check for blank line before the first trailing comment
        let first_comment = trailing_comments[0];
        if self.has_blank_line_between(prev_end, first_comment.span.start) {
            docs.push(d.literalline());
        }
        docs.push(d.hardline());

        // Process each trailing comment
        let mut last_pos = prev_end;
        for (j, comment) in trailing_comments.iter().enumerate() {
            let is_last = j == trailing_comments.len() - 1;

            // Check for blank lines between comments
            if j > 0 && self.has_blank_line_between(last_pos, comment.span.start) {
                docs.push(d.literalline());
                docs.push(d.hardline());
            }

            // Check if there's a blank line after this comment (to next comment)
            let has_blank_after = !is_last
                && self
                    .has_blank_line_between(comment.span.end, trailing_comments[j + 1].span.start);

            docs.push(self.build_comment_doc(comment));

            // Line comment - add hardline after unless:
            // - It's the last comment (closing brace follows)
            // - There's a blank line after (the blank line separator handles it)
            if !comment.is_block && !is_last && !has_blank_after {
                docs.push(d.hardline());
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
    pub(crate) fn build_comment_doc(&self, comment: &internal::Comment) -> DocId {
        let d = self.d();
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
                            line_docs.push(d.literalline());
                        } else if use_context_indent {
                            // Apply context indent for content lines and closing line
                            line_docs.push(d.hardline());
                        } else {
                            // Preserve at column 0
                            line_docs.push(d.literalline());
                        }
                    }
                    if i == 0 {
                        line_docs.push(d.text_owned(format!("/*{}", line.trim_end())));
                    } else if is_last {
                        // Preserve last line content (space before */)
                        line_docs.push(d.text_owned((*line).to_string()));
                    } else {
                        // Strip trailing whitespace from middle lines (matches prettier)
                        line_docs.push(d.text_owned(line.trim_end().to_string()));
                    }
                }
                line_docs.push(d.text("*/"));
                d.concat(&line_docs)
            } else {
                // Single-line block comment
                d.text_owned(format!("/*{}*/", comment.content))
            }
        } else if comment.span.start == 0 && comment.content.starts_with("#!") {
            // Hashbang comment: #!/usr/bin/env node (no // prefix)
            // Content already includes the #! prefix
            d.text_owned(comment.content.clone())
        } else {
            // Line comment: // content
            d.text_owned(format!("//{}", comment.content))
        }
    }

    /// Build a line_suffix doc for a trailing line comment (space + comment)
    ///
    /// Wrapping in line_suffix excludes the comment from width calculations,
    /// so elements can stay compact even when the trailing comment would push
    /// the line over print_width.
    pub(crate) fn build_trailing_line_comment_doc(&self, comment: &internal::Comment) -> DocId {
        let d = self.d();
        d.line_suffix(d.concat(&[d.text(" "), self.build_comment_doc(comment)]))
    }

    /// Build a line_suffix doc for all comments between two positions
    ///
    /// Used for trailing comments on call arguments, where comments should stay
    /// on the same line but not affect width calculations for breaking decisions.
    /// Returns None if no comments exist in the range.
    ///
    /// Example: `fn(arg // comment)` - the comment becomes a line_suffix
    pub(crate) fn build_trailing_comments_line_suffix(
        &self,
        start: u32,
        end: u32,
    ) -> Option<DocId> {
        let d = self.d();
        // Single binary search to find first comment
        let first_idx = tsv_lang::find_first_comment_from(self.comments, start);
        let first = self.comments.get(first_idx).filter(|c| c.span.end <= end)?;

        // Build parts starting from found comment
        let mut parts = Vec::new();
        for comment in std::iter::once(first).chain(
            self.comments[first_idx + 1..]
                .iter()
                .take_while(|c| c.span.end <= end),
        ) {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }

        Some(d.line_suffix(d.concat(&parts)))
    }

    /// Build a Doc for an empty body (`{}`) that may contain comments.
    ///
    /// If comments exist between the braces, formats as:
    /// ```text
    /// {
    ///     // comment
    /// }
    /// ```
    ///
    /// If no comments, returns `{}`.
    ///
    /// Used by: interface body, class body, enum body, namespace body, object literal, object pattern.
    pub(crate) fn build_empty_body_with_comments_doc(&self, body_span: tsv_lang::Span) -> DocId {
        self.build_empty_delimited_with_comments_doc(body_span.start, body_span.end, "{", "}")
    }

    /// Build a Doc for an empty bracket body (`[]`) that may contain comments.
    ///
    /// If comments exist between the brackets, formats as:
    /// ```text
    /// [
    ///     // comment
    /// ]
    /// ```
    ///
    /// If no comments, returns `[]`.
    ///
    /// Used by: array literal, tuple type.
    pub(crate) fn build_empty_brackets_with_comments_doc(&self, span: tsv_lang::Span) -> DocId {
        self.build_empty_delimited_with_comments_doc(span.start, span.end, "[", "]")
    }

    /// Build a Doc for an empty bracket body with explicit bounds.
    ///
    /// Used when the bracket body ends before the full span (e.g., array pattern with type annotation).
    pub(crate) fn build_empty_brackets_with_comments_doc_range(
        &self,
        body_start: u32,
        body_end: u32,
    ) -> DocId {
        self.build_empty_delimited_with_comments_doc(body_start, body_end, "[", "]")
    }

    /// Build a Doc for an empty delimited container that may contain comments.
    ///
    /// Generic helper for both `{}` and `[]` containers.
    fn build_empty_delimited_with_comments_doc(
        &self,
        span_start: u32,
        span_end: u32,
        open: &'static str,
        close: &'static str,
    ) -> DocId {
        let d = self.d();
        let body_start = span_start + 1; // After opening delimiter
        let body_end = span_end.saturating_sub(1); // Before closing delimiter

        // Single binary search to find comments
        let first_idx = tsv_lang::find_first_comment_from(self.comments, body_start);
        let comments: Vec<_> = self.comments[first_idx..]
            .iter()
            .take_while(|c| c.span.end <= body_end)
            .collect();

        if comments.is_empty() {
            return d.text_owned(format!("{open}{close}"));
        }
        let mut comment_parts = Vec::new();

        for (i, comment) in comments.iter().enumerate() {
            comment_parts.push(self.build_comment_doc(comment));
            // Add hardline after line comments, except for the last one
            // (the hardline before closing delimiter handles that)
            if !comment.is_block && i < comments.len() - 1 {
                comment_parts.push(d.hardline());
            }
        }

        d.concat(&[
            d.text(open),
            d.indent(d.concat(&[d.hardline(), d.concat(&comment_parts)])),
            d.hardline(),
            d.text(close),
        ])
    }
}
