// Comment handling for TypeScript printer
//
// This module handles all comment-related operations:
// - Building Doc representations for comments
// - Printing comments directly to buffer
// - Finding and filtering comments in ranges
// - Handling leading/trailing/inline comments

use super::Printer;
use super::analysis::{find_char_skipping_comments, skip_string_or_comment};
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

impl CommentSpacing {
    /// `Trailing` when followed by type params (`/* c */ <T>`),
    /// `Leading` when followed by parens (` /* c */()`).
    pub(crate) fn for_type_params(has_type_params: bool) -> Self {
        if has_type_params {
            Self::Trailing
        } else {
            Self::Leading
        }
    }
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

    /// Find the position of the LAST comma in `[start, end)`, or `None`.
    ///
    /// Walks forward via `find_comma_after`, so it correctly skips commas
    /// inside strings and comments. Used to anchor comments emitted past the
    /// last separator in trailing-elision arrays (e.g. `[, , ,/* c */]`).
    pub(crate) fn find_last_comma_before(&self, start: u32, end: u32) -> Option<u32> {
        let mut last = None;
        let mut pos = start;
        while let Some(c) = self.find_comma_after(pos) {
            if c >= end {
                break;
            }
            last = Some(c);
            pos = c + 1;
        }
        last
    }

    /// Check for a blank line after the first comma in `(prev_end, upper)`,
    /// accounting for stripped grouping parens.
    ///
    /// If no comma is found before `upper`, the check starts at `prev_end`.
    /// Callers must pass `prev_end <= upper`.
    pub(crate) fn has_blank_line_after_comma(&self, prev_end: u32, upper: u32) -> bool {
        let check_start = self
            .find_comma_after(prev_end)
            .filter(|&c| c < upper)
            .map_or(prev_end, |c| c + 1);
        let check_end = super::calls::skip_stripped_open_paren(self.source, check_start, upper);
        self.has_blank_line_between(check_start, check_end)
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

    /// Build a Doc for trailing comments where a line comment must force the
    /// following content onto a new line.
    ///
    /// Like `build_comments_between(_, _, Trailing)` for block comments, but
    /// for line comments emits a hardline after the comment instead of a space.
    /// Use when the comment precedes content that must not be swallowed by the
    /// line comment (e.g., `=> // leading\nT`, `: // leading\nT`).
    pub(crate) fn build_trailing_comments_break_for_line(&self, start: u32, end: u32) -> DocId {
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
        if parts.is_empty() {
            d.empty()
        } else {
            d.concat(&parts)
        }
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

    /// Build a Doc for comments between a keyword and the following name/token.
    ///
    /// Handles line comments safely: emits hardline after line comments to prevent
    /// absorbing following code. Block comments get a leading space + trailing space.
    /// Returns `" // c" + hardline` for line comments, or `" /* c */ "` for block.
    ///
    /// Used for: `function // c\nname`, `class // c\nname`, `export // c\ndecl`,
    /// `enum // c\nname`, etc. — any keyword-to-name/code gap.
    pub(crate) fn build_keyword_to_name_comments(&self, start: u32, end: u32) -> DocId {
        let d = self.d();
        if self.has_line_comments_between(start, end) {
            self.build_name_to_type_params_comments(start, end, CommentSpacing::Trailing)
        } else {
            let comments = self.build_inline_comments_between_doc_trailing_space(start, end);
            d.concat(&[d.text(" "), comments])
        }
    }

    /// Build a Doc for inline comments between a name/key and type params or parens.
    ///
    /// Like `build_comments_between` but handles line comments safely:
    /// block comments use the given `block_spacing`, line comments always get
    /// a leading space and hardline after (to prevent absorbing following code).
    ///
    /// Used for: declaration name → type params, method key → type params/parens,
    /// getter/setter key → parens.
    ///
    /// Example: `class A // c\n<T> {}` stays multi-line instead of collapsing to
    /// `class A// c <T> {}` where `<T> {}` would be lost in the comment.
    pub(crate) fn build_name_to_type_params_comments(
        &self,
        start: u32,
        end: u32,
        block_spacing: CommentSpacing,
    ) -> DocId {
        let d = self.d();
        let first_idx = tsv_lang::find_first_comment_from(self.comments, start);
        let mut parts = Vec::new();
        for comment in self.comments[first_idx..]
            .iter()
            .take_while(|c| c.span.end <= end)
        {
            if comment.is_block {
                // Block comment: use caller-specified spacing
                match block_spacing {
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
            } else {
                // Line comment: leading space + hardline after
                // `class A // c\n<T> {}`
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
                parts.push(d.hardline());
            }
        }
        d.concat(&parts)
    }

    /// Like `build_name_to_type_params_comments`, but returns `None` when there
    /// are no comments in the range (avoids the separate `has_comments_between` check).
    pub(crate) fn build_name_to_type_params_comments_opt(
        &self,
        start: u32,
        end: u32,
        block_spacing: CommentSpacing,
    ) -> Option<DocId> {
        if self.has_comments_between(start, end) {
            Some(self.build_name_to_type_params_comments(start, end, block_spacing))
        } else {
            None
        }
    }

    /// Split heritage-preceding comments into inline and indented parts.
    ///
    /// For comments between a declaration name/type-params and a heritage keyword
    /// (extends/implements), comments before the first line comment stay inline at the
    /// declaration level, while comments after a line comment go into the heritage indent.
    ///
    /// Returns `(inline_parts, indent_parts)`:
    /// - `inline_parts`: `[" ", comment, " ", comment, ...]` at declaration level
    /// - `indent_parts`: `[hardline, comment, hardline, comment, ...]` for heritage indent
    pub(crate) fn build_heritage_leading_comment_parts(
        &self,
        start: u32,
        end: u32,
    ) -> (Vec<DocId>, Vec<DocId>) {
        let d = self.d();
        let mut inline_parts = Vec::new();
        let mut indent_parts = Vec::new();
        let mut saw_line_comment = false;
        for comment in comments_in_range(self.comments, start, end) {
            if saw_line_comment {
                indent_parts.push(d.hardline());
                indent_parts.push(self.build_comment_doc(comment));
            } else {
                inline_parts.push(d.text(" "));
                inline_parts.push(self.build_comment_doc(comment));
                if !comment.is_block {
                    saw_line_comment = true;
                }
            }
        }
        (inline_parts, indent_parts)
    }

    /// Build a heritage clause doc: `keyword` + indented, comma-separated heritage items.
    ///
    /// Handles line comments between items (SAFETY): when a line comment appears after
    /// a heritage item, the comma is placed before the comment to prevent the comment
    /// from absorbing subsequent items. Block comments keep the comma after.
    ///
    /// Used by both class `implements` and interface `extends` clauses.
    pub(crate) fn build_heritage_clause_doc(
        &self,
        keyword: &'static str,
        items: &[internal::TSInterfaceHeritage],
        group_mode: bool,
        keyword_start: Option<u32>,
    ) -> DocId {
        let d = self.d();

        // Track which items have trailing line comments (between this item and the next).
        // Line comments consume the rest of the line, so the comma must go before them.
        let has_trailing_line_comment: Vec<bool> = items
            .windows(2)
            .map(|pair| {
                self.has_line_comments_between(heritage_item_end(&pair[0]), pair[1].span.start)
            })
            .collect();
        let has_any_item_line_comments = has_trailing_line_comment.iter().any(|&v| v);

        let item_docs: Vec<_> = items
            .iter()
            .enumerate()
            .map(|(i, heritage)| {
                let mut h_parts = vec![self.build_entity_name_doc(&heritage.expression)];
                if let Some(type_args) = &heritage.type_arguments {
                    // Preserve comments: `implements Foo/* c */ <T>`
                    let gap_start = heritage.expression.span().end;
                    let gap_end = type_args.span.start;
                    if let Some(doc) = self.build_name_to_type_params_comments_opt(
                        gap_start,
                        gap_end,
                        CommentSpacing::Trailing,
                    ) {
                        h_parts.push(doc);
                    }
                    h_parts.push(self.build_type_arguments_doc_wrapping(type_args));
                }
                if let Some(next) = items.get(i + 1) {
                    let item_end = heritage_item_end(heritage);
                    let comments: Vec<_> =
                        comments_in_range(self.comments, item_end, next.span.start).collect();

                    if has_trailing_line_comment[i] {
                        // Has line comment(s): comma must go before the first line comment.
                        // Block comments before the first line comment go before the comma.
                        // e.g. `I /* c1 */,\n// c2\nJ` or `I, // c1\n// c2\nJ`
                        let first_line_idx = comments.iter().position(|c| !c.is_block).unwrap_or(0);

                        // Block comments before the first line comment
                        for comment in &comments[..first_line_idx] {
                            h_parts.push(d.text(" "));
                            h_parts.push(self.build_comment_doc(comment));
                        }

                        // Comma before the first line comment
                        h_parts.push(d.text(","));

                        // Remaining comments (starting with the first line comment)
                        // `needs_hardline` starts true when block comments precede
                        // (comma sits between block and line, needs newline after)
                        let mut needs_hardline = first_line_idx > 0;
                        for comment in &comments[first_line_idx..] {
                            if needs_hardline {
                                h_parts.push(d.hardline());
                            } else {
                                h_parts.push(d.text(" "));
                            }
                            h_parts.push(self.build_comment_doc(comment));
                            needs_hardline = !comment.is_block;
                        }
                    } else {
                        // No line comments: emit block comments inline with leading space
                        for comment in &comments {
                            h_parts.push(d.text(" "));
                            h_parts.push(self.build_comment_doc(comment));
                        }
                    }
                }
                d.concat(&h_parts)
            })
            .collect();

        // Optional comments between keyword and first item: `extends /* c */ Item`
        let kw_comments = keyword_start
            .and_then(|kw_start| {
                let kw_end = kw_start + keyword.len() as u32;
                self.build_comments_between_filtered_opt(
                    kw_end,
                    items[0].span.start,
                    CommentSpacing::Trailing,
                    CommentFilter::All,
                )
            })
            .unwrap_or_else(|| d.empty());

        if group_mode {
            if has_any_item_line_comments {
                // Line comments force hardline breaks. Items with line comments have
                // commas baked in; others get commas from the separator.
                let comma_hardline = d.concat(&[d.text(","), d.hardline()]);
                let hardline = d.hardline();
                let mut joined_parts = vec![item_docs[0]];
                for (idx, &item_doc) in item_docs.iter().enumerate().skip(1) {
                    // Previous item had baked-in comma + line comment → just hardline
                    // Otherwise → comma + hardline
                    joined_parts.push(if has_trailing_line_comment[idx - 1] {
                        hardline
                    } else {
                        comma_hardline
                    });
                    joined_parts.push(item_doc);
                }
                let types_joined = d.concat(&joined_parts);
                let inner = d.indent(d.concat(&[d.hardline(), kw_comments, types_joined]));
                d.concat(&[d.text(keyword), inner])
            } else {
                let comma_line = d.concat(&[d.text(","), d.line()]);
                let types_joined = d.join_doc(item_docs, comma_line);
                d.concat(&[
                    d.text(keyword),
                    d.group(d.indent(d.concat(&[d.line(), kw_comments, types_joined]))),
                ])
            }
        } else {
            let keyword_space = match keyword {
                "implements" => "implements ",
                "extends" => "extends ",
                _ => unreachable!(),
            };
            d.concat(&[d.text(keyword_space), kw_comments, d.join(item_docs, ", ")])
        }
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

    /// Append trailing comments from stripped grouping parens to a parts vec.
    ///
    /// When the parser strips grouping parens (e.g., `await (x /* c */)` → arg is `x`),
    /// comments between the argument end and the expression span end are orphaned.
    /// This method emits them with appropriate layout:
    /// - Same-line block comments: inline with leading space (`x /* c */`)
    /// - Line comments: deferred via `line_suffix` to appear after the semicolon (`x; // c`)
    /// - Own-line block comments: deferred via `line_suffix` with hardline (`x;\n/* c */`)
    ///
    /// Used by await, yield, return, throw, and export default.
    pub(crate) fn append_trailing_paren_comments(
        &self,
        parts: &mut Vec<DocId>,
        argument_end: u32,
        span_end: u32,
    ) {
        let d = self.d();
        for comment in comments_in_range(self.comments, argument_end, span_end) {
            if comment.is_block && !self.has_newline_between(argument_end, comment.span.start) {
                // Same-line block comment: `expr /* c */`
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else if !comment.is_block {
                // Line comment: defer to after semicolon via line_suffix
                let suffix = d.concat(&[d.text(" "), self.build_comment_doc(comment)]);
                parts.push(d.line_suffix(suffix));
            } else {
                // Own-line block comment: defer to own line after semicolon
                let suffix = d.concat(&[d.hardline(), self.build_comment_doc(comment)]);
                parts.push(d.line_suffix(suffix));
            }
        }
    }

    /// Append trailing comments from stripped grouping parens in spread elements,
    /// excluding own-line block comments (which are handled by the parent array/call).
    ///
    /// Own-line block comments in spread (`...(x\n/* c */)`) need to become siblings
    /// in the parent list, after the spread's comma. Using `line_suffix` would defer
    /// them past the enclosing `]`/`)` bracket. Instead, the parent formatter picks
    /// them up via `spread_own_line_block_comments()`.
    pub(crate) fn append_spread_trailing_paren_comments(
        &self,
        parts: &mut Vec<DocId>,
        argument_end: u32,
        span_end: u32,
    ) {
        let d = self.d();
        for comment in comments_in_range(self.comments, argument_end, span_end) {
            if comment.is_block && !self.has_newline_between(argument_end, comment.span.start) {
                // Same-line block comment: `...x /* c */`
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else if !comment.is_block {
                // Line comment: defer to after semicolon via line_suffix
                let suffix = d.concat(&[d.text(" "), self.build_comment_doc(comment)]);
                parts.push(d.line_suffix(suffix));
            }
            // Own-line block comments: skip (handled by parent array/call)
        }
    }

    /// Get own-line block comments from stripped parens in a spread element.
    ///
    /// When the parser strips grouping parens (e.g., `...(x\n/* c */)`), own-line
    /// block comments between `argument.end` and `spread.span.end` need to be emitted
    /// by the parent formatter (array/call) as siblings after the spread's comma,
    /// not by the spread doc itself.
    pub(crate) fn spread_own_line_block_comments(
        &self,
        expr: &internal::Expression,
    ) -> Vec<&tsv_lang::Comment> {
        if let internal::Expression::SpreadElement(spread) = expr {
            let arg_end = spread.argument.span().end;
            comments_in_range(self.comments, arg_end, spread.span.end)
                .filter(|c| c.is_block && self.has_newline_between(arg_end, c.span.start))
                .collect()
        } else {
            vec![]
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

    /// Check if stripped grouping parens left trailing comments.
    ///
    /// Returns true when there are comments between `expr_end` and `boundary_end`
    /// AND a `)` exists in the source after those comments (confirming that the
    /// parser stripped a `ParenthesizedExpression`). Without the `)` check, this
    /// would false-positive on normal operator comments (e.g. ternary `? c /* comment */ :`).
    pub(crate) fn has_trailing_paren_comments(&self, expr_end: u32, boundary_end: u32) -> bool {
        if !self.has_comments_between(expr_end, boundary_end) {
            return false;
        }
        // Find the last comment's end, then check for `)` between there and boundary
        let last_comment_end = comments_in_range(self.comments, expr_end, boundary_end)
            .last()
            .map_or(expr_end as usize, |c| c.span.end as usize);
        self.source[last_comment_end..boundary_end as usize]
            .bytes()
            .any(|b| b == b')')
    }

    /// Build expression doc preserving trailing comments from stripped grouping parens.
    ///
    /// When the parser strips parens from `(expr /* c */)`, comments between
    /// `expr.span().end` and `boundary_end` are lost. This re-adds parens when
    /// trailing comments exist, producing `(expr /* c */)` or `(\n\texpr // c\n)`.
    ///
    /// Used for variable init, assignment RHS, ternary branches, and sequence members.
    pub(crate) fn build_expression_doc_with_paren_comments(
        &self,
        expr: &internal::Expression,
        boundary_end: u32,
    ) -> DocId {
        let d = self.d();
        let expr_end = expr.span().end;

        if !self.has_trailing_paren_comments(expr_end, boundary_end) {
            return self.build_expression_doc(expr);
        }

        let inner = self.build_expression_doc(expr);

        // Determine if multiline layout is needed
        let has_multiline = comments_in_range(self.comments, expr_end, boundary_end)
            .any(|c| !c.is_block || self.has_newline_between(expr_end, c.span.start));

        if has_multiline {
            let mut indent_parts = vec![d.hardline()];
            indent_parts.push(inner);
            for comment in comments_in_range(self.comments, expr_end, boundary_end) {
                if !comment.is_block || !self.has_newline_between(expr_end, comment.span.start) {
                    indent_parts.push(d.text(" "));
                    indent_parts.push(self.build_comment_doc(comment));
                } else {
                    indent_parts.push(d.hardline());
                    indent_parts.push(self.build_comment_doc(comment));
                }
            }
            d.concat(&[
                d.text("("),
                d.indent(d.concat(&indent_parts)),
                d.hardline(),
                d.text(")"),
            ])
        } else {
            let mut parts = vec![d.text("(")];
            parts.push(inner);
            for comment in comments_in_range(self.comments, expr_end, boundary_end) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }
            parts.push(d.text(")"));
            d.concat(&parts)
        }
    }

    /// Promote block comments that appear before an assignment operator to the LHS.
    ///
    /// In `a /* comment */ = b`, the comment is between `left.span().end` and `right.span().start`
    /// but positioned before the `=` in source. Prettier places such comments before the operator,
    /// so we promote them to the LHS doc.
    ///
    /// Returns the promoted comments doc (with leading space) and the new RHS comment start
    /// position, or None if no comments need promoting.
    pub(crate) fn promote_comments_before_operator(
        &self,
        start: u32,
        end: u32,
        operator: &str,
    ) -> Option<(DocId, u32)> {
        let d = self.d();
        // Find the operator position by scanning forward, skipping whitespace and comments
        let op_pos = self.find_operator_in_source(start, end, operator)?;

        // Collect block comments that appear before the operator
        let mut promoted_parts = Vec::new();
        let mut last_promoted_end = start;
        for comment in comments_in_range(self.comments, start, op_pos) {
            if comment.is_block {
                promoted_parts.push(d.text(" "));
                promoted_parts.push(self.build_comment_doc(comment));
                last_promoted_end = comment.span.end;
            }
        }

        if promoted_parts.is_empty() {
            None
        } else {
            Some((d.concat(&promoted_parts), last_promoted_end))
        }
    }

    /// Find the position of an operator string between two positions, skipping
    /// whitespace and comments in the source.
    fn find_operator_in_source(&self, start: u32, end: u32, operator: &str) -> Option<u32> {
        let bytes = self.source.as_bytes();
        let op_bytes = operator.as_bytes();
        let op_len = op_bytes.len();
        let end_usize = end as usize;
        let mut i = start as usize;

        while i + op_len <= end_usize {
            let b = bytes[i];
            if b.is_ascii_whitespace() {
                i += 1;
                continue;
            }
            if b == b'/' && i + 1 < end_usize {
                match bytes[i + 1] {
                    b'*' => {
                        i += 2;
                        while i + 1 < end_usize && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                            i += 1;
                        }
                        i += 2;
                        continue;
                    }
                    b'/' => {
                        while i < end_usize && bytes[i] != b'\n' {
                            i += 1;
                        }
                        i += 1;
                        continue;
                    }
                    _ => {}
                }
            }
            if &bytes[i..i + op_len] == op_bytes {
                return Some(i as u32);
            }
            i += 1;
        }
        None
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
    /// Comments between `start` and `end` (where `end` is the element start):
    /// - Block comments on the same line as the element: `/*content*/ ` (inline with trailing space)
    /// - Block comments on their own line: `/*content*/` + hardline
    /// - Line comments: `//content` + hardline (always on own line)
    ///
    /// Used when expanding comments force multiline formatting (unions, tuples, etc.)
    pub(crate) fn build_leading_comments_multiline(&self, start: u32, end: u32) -> Vec<DocId> {
        let d = self.d();
        let mut parts = Vec::new();
        for comment in comments_in_range(self.comments, start, end) {
            parts.push(self.build_comment_doc(comment));
            if comment.is_block && self.is_same_line(comment.span.end, end) {
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
        // Track line reference — follows multi-line block comments to their
        // closing */ line (same logic as build_trailing_same_line_comments_doc in mod.rs)
        let mut line_ref = after_pos;
        for comment in comments_in_range(self.comments, after_pos, upper_bound) {
            if self.is_same_line(line_ref, comment.span.start) {
                if comment.is_block {
                    // Block comments are inline, affect width
                    docs.push(d.text(" "));
                    docs.push(self.build_comment_doc(comment));
                    // Follow multi-line block comments to their closing line
                    if !self.is_same_line(comment.span.start, comment.span.end) {
                        line_ref = comment.span.end;
                    }
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

            // Check if the next item (comment or target) is on the same line as this comment's end.
            // This handles multi-line block comments where the closing */ is followed by another
            // comment on the same line: `/*\nmulti\n*/ /* after */`
            let next_on_same_line = if is_last_comment {
                self.is_same_line(comment.span.end, target_start)
            } else {
                self.is_same_line(comment.span.end, comments[j + 1].span.start)
            };

            // For subsequent comments, determine separator from previous comment
            if j > 0 {
                if self.is_same_line(last_pos, comment.span.start) {
                    // Same line as previous comment's end — keep inline (space is
                    // handled by the previous comment's suffix, so no space here)
                } else if self.has_blank_line_between(last_pos, comment.span.start) {
                    docs.push(d.literalline());
                    docs.push(d.hardline());
                }
                // else: no separator needed (previous comment's suffix handled it)
            }

            docs.push(self.build_comment_doc(comment));

            if !comment.is_block {
                // Line comment: add hardline after unless there's a blank line after
                // (the blank line separator will handle it)
                if !has_blank_after {
                    docs.push(d.hardline());
                }
            } else if next_on_same_line {
                // Block comment on same line as next item - space before next
                docs.push(d.text(" "));
            } else if !has_blank_after {
                // Block comment on its own line: add hardline unless there's blank after
                docs.push(d.hardline());
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

    /// Append comments between type params `>` and `(` to parts.
    ///
    /// Block comments are emitted inline with a leading space. Line comments
    /// use `line_suffix` so they're deferred to end of the rendered line
    /// (avoids corruption where `// c` would swallow `(x: T)`).
    pub(crate) fn append_type_params_to_paren_comments(
        &self,
        parts: &mut Vec<DocId>,
        type_params_end: u32,
        paren_pos: u32,
    ) {
        let d = self.d();
        for comment in comments_in_range(self.comments, type_params_end, paren_pos) {
            if comment.is_block {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else {
                parts.push(self.build_trailing_line_comment_doc(comment));
            }
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

    /// Append comments between a generator `*` marker and the method/function key.
    ///
    /// Searches for `*` in the source between `search_start` and `key_start`,
    /// then emits any comments found after it (e.g., `*/* comment */ gen()`).
    /// Call after pushing `d.text("*")` to parts.
    pub(crate) fn append_generator_star_comments(
        &self,
        parts: &mut Vec<DocId>,
        search_start: u32,
        key_start: u32,
    ) {
        if let Some(star_pos) = self.source[search_start as usize..key_start as usize].find('*') {
            let after_star = search_start + star_pos as u32 + 1;
            for comment in comments_in_range(self.comments, after_star, key_start) {
                parts.push(self.build_comment_doc(comment));
                parts.push(self.d().text(" "));
            }
        }
    }

    /// Append a function/method body with comment splitting between signature and body.
    ///
    /// Block comments stay inline: `gen() /* c */ {}`
    /// Line comments get absorbed into the block body as leading content.
    pub(crate) fn append_body_with_sig_comments(
        &self,
        parts: &mut Vec<DocId>,
        sig_end: u32,
        body: &internal::BlockStatement,
    ) {
        let d = self.d();
        let body_start = body.span.start;
        if self.has_comments_between(sig_end, body_start) {
            let mut absorbed = Vec::new();
            for comment in comments_in_range(self.comments, sig_end, body_start) {
                if comment.is_block {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                } else {
                    absorbed.push(self.build_comment_doc(comment));
                }
            }
            parts.push(d.text(" "));
            parts.push(self.build_block_statement_with_outer_comments_doc(body, absorbed));
        } else {
            parts.push(d.text(" "));
            parts.push(self.build_block_statement_doc(body));
        }
    }

    /// Find the comma position between two adjacent list elements,
    /// skipping over any comments in between.
    #[allow(clippy::expect_used)]
    pub(crate) fn find_list_comma(&self, elem_end: u32, next_start: u32) -> u32 {
        find_char_skipping_comments(
            self.source.as_bytes(),
            elem_end as usize,
            next_start as usize,
            b',',
        )
        .expect("comma must exist between list elements") as u32
    }

    /// Append trailing inline block comments (` /*content*/` format) between two positions.
    ///
    /// Only emits block comments; line comments are skipped (they would have been
    /// detected earlier and routed to the multiline path).
    pub(crate) fn append_trailing_inline_block_comments(
        &self,
        parts: &mut Vec<DocId>,
        start: u32,
        end: u32,
    ) {
        let d = self.d();
        for comment in comments_in_range(self.comments, start, end) {
            if comment.is_block {
                parts.push(d.text_owned(format!(" /*{}*/", comment.content)));
            }
        }
    }

    /// Emit comma with surrounding comments for a non-last element in a forced-multiline list.
    ///
    /// Handles comment positioning around the comma between `elem_end` and `next_start`:
    /// 1. Trailing comments before comma (multiline layout)
    /// 2. Comma text
    /// 3. Same-line trailing comments after comma (line comments)
    /// 4. Hardline separator
    ///
    /// Returns the new `prev_end` position.
    pub(crate) fn emit_multiline_comma_with_comments(
        &self,
        parts: &mut Vec<DocId>,
        elem_end: u32,
        next_start: u32,
    ) -> u32 {
        let d = self.d();
        let comma_pos = self.find_list_comma(elem_end, next_start);

        // Trailing comments before comma
        parts.extend(self.build_trailing_comments_multiline(elem_end, comma_pos));

        // Comma
        parts.push(d.text(","));

        // Same-line trailing comments after comma (line comments that consume the line)
        let mut after_comma_end = comma_pos + 1;
        for comment in comments_in_range(self.comments, comma_pos + 1, next_start) {
            if self.is_same_line(elem_end, comment.span.start) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
                after_comma_end = comment.span.end;
            }
        }

        // Hardline to separate from next element
        parts.push(d.hardline());

        after_comma_end
    }
}

/// End position of a heritage item (after type arguments if present).
fn heritage_item_end(item: &internal::TSInterfaceHeritage) -> u32 {
    item.type_arguments
        .as_ref()
        .map_or_else(|| item.expression.span().end, |ta| ta.span.end)
}
