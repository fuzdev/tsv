// Comment handling for TypeScript printer
//
// This module handles all comment-related operations:
// - Building Doc representations for comments
// - Printing comments directly to buffer
// - Finding and filtering comments in ranges
// - Handling leading/trailing/inline comments

use super::Printer;
use super::analysis::skip_string_or_comment;
use super::layout::hang_after_operator;
use crate::ast::internal;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;
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

    /// Emit a member keyword (modifier like `static ` / `readonly `, or
    /// accessor `get ` / `set `) preserving comments BEFORE it: the range
    /// `(cursor, keyword_pos)` is emitted ahead of the keyword text, so a chain
    /// of calls keeps each comment at the user's position
    /// (`static /* c */ readonly p`). Advances `cursor` past the keyword.
    ///
    /// Callers finish the chain with [`Self::push_pre_name_comments_doc`] for
    /// the final `(cursor, name_start)` range.
    pub(crate) fn push_member_keyword_doc(
        &self,
        parts: &mut Vec<DocId>,
        kind_text: &'static str,
        cursor: &mut u32,
        bound: u32,
    ) {
        let keyword = kind_text.trim_end();
        if let Some(kw_pos) = self.find_keyword_in_range(*cursor, bound, keyword) {
            if self.has_comments_between(*cursor, kw_pos) {
                parts.push(self.build_trailing_comments_break_for_line(*cursor, kw_pos));
            }
            *cursor = kw_pos + keyword.len() as u32;
        }
        parts.push(self.d().text(kind_text));
    }

    /// Emit comments between the last member keyword and the member name
    /// (e.g., `get /* c */ a()`); block comments get a trailing space, line
    /// comments a hardline.
    pub(crate) fn push_pre_name_comments_doc(
        &self,
        parts: &mut Vec<DocId>,
        cursor: u32,
        name_start: u32,
    ) {
        if self.has_comments_between(cursor, name_start) {
            parts.push(self.build_trailing_comments_break_for_line(cursor, name_start));
        }
    }

    /// Emit an accessor keyword (`get ` / `set `) preserving comments between
    /// the keyword and the key (e.g., `get /* c */ a()`).
    ///
    /// Single-keyword convenience over [`Self::push_member_keyword_doc`] +
    /// [`Self::push_pre_name_comments_doc`]; `search_from` is the member's start.
    pub(crate) fn push_accessor_keyword_doc(
        &self,
        parts: &mut Vec<DocId>,
        kind_text: &'static str,
        search_from: u32,
        key_start: u32,
    ) {
        let mut cursor = search_from;
        self.push_member_keyword_doc(parts, kind_text, &mut cursor, key_start);
        self.push_pre_name_comments_doc(parts, cursor, key_start);
    }

    /// Emit an optional/definite modifier marker (`?` or `!`) that follows a key
    /// or name, preserving comments between the name and the marker
    /// (e.g., `a /* c */?: number`). Returns the position after the marker.
    ///
    /// Scans for the first `marker` byte outside comments, unbounded to the end
    /// of source: the AST flag is only set when the parser consumed the marker
    /// directly after the name (whitespace and comments only in between), so the
    /// first non-comment occurrence is always the right one. Callers must NOT
    /// derive a search bound from spans — spans exclude the marker in some shapes
    /// (`let a! = x`, `interface I { a? }`), which is how past panics happened.
    pub(crate) fn push_modifier_marker_doc(
        &self,
        parts: &mut Vec<DocId>,
        after: u32,
        marker: u8,
    ) -> u32 {
        let d = self.d();
        #[allow(clippy::expect_used)] // Parser guarantees the marker exists when the flag is set
        let pos = find_char_skipping_comments(
            self.source.as_bytes(),
            after as usize,
            self.source.len(),
            marker,
        )
        .expect("modifier marker (`?`/`!`) not found") as u32;
        if self.has_comments_between(after, pos) {
            parts.push(self.build_inline_comments_between_doc(after, pos));
        }
        parts.push(d.text(if marker == b'?' { "?" } else { "!" }));
        pos + 1
    }

    /// Emit comments in the gap between an optional `?`/`!` marker and a member's
    /// type annotation `:`, preserving the user's placement *after* the marker.
    ///
    /// A block comment stays inline with a trailing space before `:`
    /// (`a? /* c */ : T`); a line comment forces a hardline so the `: T`
    /// annotation drops to the next line instead of being swallowed as comment
    /// text (`a? // c⏎: T`) — a content-loss / non-idempotency fix. Prettier
    /// instead relocates such comments (a block before `?`, a line after the
    /// member `;`), so the preserved forms are `_prettier_divergence`s
    /// ([conformance_prettier.md](../../../../docs/conformance_prettier.md)
    /// §Comment relocation).
    ///
    /// Shared by the three type-element property arms (type-literal, interface,
    /// class). Returns `None` when the range has no comments.
    pub(crate) fn build_marker_to_colon_comments_doc(
        &self,
        after: u32,
        colon_start: u32,
    ) -> Option<DocId> {
        let comments = self.build_name_to_type_params_comments_opt(
            after,
            colon_start,
            CommentSpacing::Leading,
        )?;
        let d = self.d();
        if self.has_line_comments_between(after, colon_start) {
            // A line comment already ended its line with a hardline; `:` follows
            // on the next line, so no extra space.
            Some(comments)
        } else {
            // Block-only: single space before `:` (matches bare `?:` spacing).
            Some(d.concat(&[comments, d.text(" ")]))
        }
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

    /// Emit the comments in `[start, end)` between a class/interface header
    /// (after the last heritage item or type params) and the body `{`, preserving
    /// each comment on its own line. A line comment ends its line, so any comment
    /// following one is pushed to its own line via `hardline` — otherwise it would
    /// be absorbed into the line comment's text (`// c1 // c2` reparses as a single
    /// comment, a content/boundary loss). The first comment, and a block following a
    /// block, keep a leading space, matching the single-comment heritage form
    /// `J // c`.
    ///
    /// Returns `None` when the range has no comments. The caller appends the pre-`{`
    /// separator itself (`hardline` for a line comment, space/`line` otherwise).
    pub(crate) fn build_pre_body_comments_doc(&self, start: u32, end: u32) -> Option<DocId> {
        let d = self.d();
        let mut parts = Vec::new();
        let mut prev_is_line = false;
        for comment in comments_in_range(self.comments, start, end) {
            if prev_is_line {
                parts.push(d.hardline());
            } else {
                parts.push(d.text(" "));
            }
            parts.push(self.build_comment_doc(comment));
            prev_is_line = !comment.is_block;
        }
        if parts.is_empty() {
            None
        } else {
            Some(d.concat(&parts))
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

    /// Build a declaration header's keyword→name gap comment followed by the rest
    /// of the declaration (`continuation`), indenting that continuation one level
    /// when a *line* comment forces the break.
    ///
    /// `keyword_end` is the byte offset just past the final keyword before the name
    /// (`function`/`*`, `class`, `enum`, `const`, …); `name_start` is the start of
    /// the name (or first declarator). The preceding keyword token must be emitted
    /// **without** a trailing space — the leading space is supplied here.
    ///
    /// - **Line comment**: ends its line with a hardline, so the whole continuation
    ///   is wrapped in `indent` to read as a statement continuation rather than a
    ///   second statement (the uniform declaration-header rule): `function // c⏎\tf()`.
    /// - **Block comment**: trails inline (` /* c */ ` + continuation), no break.
    /// - **No comment**: just a leading space before the continuation.
    ///
    /// Block and no-comment output is byte-identical to the prior
    /// `" " + build_keyword_to_name_comments(...)` form. Shared by the
    /// `function`/`class`/`enum`/`declare function`/variable declaration printers
    /// and the `export` / `export default`→declaration printers in
    /// `statements/modules.rs`.
    pub(crate) fn build_keyword_to_name_continuation(
        &self,
        keyword_end: u32,
        name_start: u32,
        continuation: DocId,
    ) -> DocId {
        let d = self.d();
        let has_line = self.has_line_comments_between(keyword_end, name_start);
        let comment_doc = if has_line {
            self.build_name_to_type_params_comments(
                keyword_end,
                name_start,
                CommentSpacing::Leading,
            )
        } else if let Some(c) = self.build_inline_comments_between_doc_opt(keyword_end, name_start)
        {
            c
        } else {
            d.empty()
        };
        // After a line comment the hardline provides separation; otherwise a space.
        let space_after = if has_line { d.empty() } else { d.text(" ") };
        let body = d.concat(&[comment_doc, space_after, continuation]);
        if has_line { d.indent(body) } else { body }
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

        // A line comment between the keyword and the first item is kept trailing
        // the keyword (preserve-in-place; prettier relocates it before the
        // keyword), with the items pushed onto the next line — mirroring the
        // as/satisfies + type-param keyword→value handling. The keyword stays
        // inline; only the items are pushed down (no whole-heritage break).
        if let Some(kw_start) = keyword_start {
            let kw_end = kw_start + keyword.len() as u32;
            if self.has_line_comments_between(kw_end, items[0].span.start) {
                let value_doc = d.join(item_docs, ", ");
                let mut parts = vec![d.text(keyword)];
                self.append_keyword_value_line_comments(
                    &mut parts,
                    kw_end,
                    items[0].span.start,
                    value_doc,
                );
                return d.concat(&parts);
            }
        }

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
                    hang_after_operator(d, d.concat(&[kw_comments, types_joined])),
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

    /// Build the leading-comment doc for comments between an opening `(` and the
    /// value that follows, concatenated with `value_doc`. Returns the combined doc
    /// plus whether a line or own-line block comment forces the enclosing parens to
    /// break across lines.
    ///
    /// An own-line block comment requires a newline BOTH before and after it —
    /// prettier keeps `(\n/* c */value)` inline because nothing separates the comment
    /// from the value. Shared by dynamic `import(...)` and TS `import(...)` types.
    pub(crate) fn build_paren_leading_value_doc(
        &self,
        open_paren_end: u32,
        value_start: u32,
        value_doc: DocId,
    ) -> (DocId, bool) {
        let d = self.d();
        let own_line = comments_in_range(self.comments, open_paren_end, value_start).any(|c| {
            c.is_block
                && self.has_newline_between(open_paren_end, c.span.start)
                && self.has_newline_between(c.span.end, value_start)
        });
        let line = self.has_line_comments_between(open_paren_end, value_start);
        let force_break = own_line || line;

        let doc = if force_break {
            // Each comment on its own line inside the broken parens.
            let mut parts = Vec::new();
            for comment in comments_in_range(self.comments, open_paren_end, value_start) {
                parts.push(self.build_comment_doc(comment));
                parts.push(d.hardline());
            }
            parts.push(value_doc);
            d.concat(&parts)
        } else if let Some(lead) = self.build_rhs_comments_opt(open_paren_end, value_start) {
            // Inline block comment(s): `/* c */ value`
            d.concat(&[lead, value_doc])
        } else {
            value_doc
        };
        (doc, force_break)
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

    /// Append comments between a declaration's last content token and its
    /// terminating `;`, preserving the user's placement (consistent with the
    /// before-semicolon and do-while `)`→`;` divergences — see
    /// `conformance_prettier.md` §Comment relocation). A same-line block comment
    /// trails the content inline (` /* c */`); line comments and own-line block
    /// comments stay on their own line, forcing the `;` onto a following line.
    ///
    /// Returns `true` if any comment forced a line break, so the caller emits the
    /// `;` after a `hardline` (and keeps these comments outside the content group
    /// so the break doesn't expand the specifier braces).
    pub(crate) fn append_pre_semi_comments(
        &self,
        parts: &mut Vec<DocId>,
        start: u32,
        end: u32,
    ) -> bool {
        let d = self.d();
        let mut prev_end = start;
        let mut broke = false;
        for comment in comments_in_range(self.comments, start, end) {
            let same_line = self.is_same_line(prev_end, comment.span.start);
            if comment.is_block && same_line {
                // Same-line block comment trails inline.
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else if same_line {
                // Trailing line comment: stays on the content line, forces a break.
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
                broke = true;
            } else {
                // Own-line comment (line or block): preserve its own line.
                if self.has_blank_line_between(prev_end, comment.span.start) {
                    parts.push(d.literalline());
                }
                parts.push(d.hardline());
                parts.push(self.build_comment_doc(comment));
                broke = true;
            }
            prev_end = comment.span.end;
        }
        broke
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

    /// Build expression doc, stripping a redundant grouping paren around a trailing
    /// comment and keeping the comment inline after the expression.
    ///
    /// When the parser strips parens from `(expr /* c */)`, comments between
    /// `expr.span().end` and `boundary_end` would be lost. For an inline same-line
    /// block comment we keep it trailing the expression (`expr /* c */`), matching
    /// prettier — stripping the redundant parens does not move the comment. Line /
    /// own-line comments need the parens (a bare line comment would swallow the
    /// following token), so those defer to `build_expression_doc_keep_paren_comments`.
    ///
    /// Used for variable init, assignment RHS, and ternary branches.
    pub(crate) fn build_expression_doc_with_paren_comments(
        &self,
        expr: &internal::Expression,
        boundary_end: u32,
    ) -> DocId {
        let expr_end = expr.span().end;

        if !self.has_trailing_paren_comments(expr_end, boundary_end) {
            return self.build_expression_doc(expr);
        }

        // Line / own-line comments need the paren wrapping (a bare line comment
        // would swallow the following `;`); defer those to the keep variant.
        let has_multiline = comments_in_range(self.comments, expr_end, boundary_end)
            .any(|c| !c.is_block || self.has_newline_between(expr_end, c.span.start));
        if has_multiline {
            return self.build_expression_doc_keep_paren_comments(expr, boundary_end);
        }

        let d = self.d();
        let inner = self.build_expression_doc(expr);
        let comments = self.build_comments_between(expr_end, boundary_end, CommentSpacing::Leading);
        d.concat(&[inner, comments])
    }

    /// Build expression doc re-adding the stripped grouping parens around trailing
    /// comments, producing `(expr /* c */)` or `(\n\texpr // c\n)`.
    ///
    /// Used where stripping the parens would relocate the comment: arrow bodies
    /// (prettier moves the comment into the params) and sequence operands (prettier
    /// floats it out of the sequence). Keeping the parens preserves the comment where
    /// the user wrote it.
    pub(crate) fn build_expression_doc_keep_paren_comments(
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
        self.build_leading_comments_multiline_opt(start, end, None)
    }

    /// Like `build_leading_comments_multiline`, but skips comments on the same
    /// source line as `delim_pos`.
    ///
    /// Used for the first element of a forced-multiline list when those same-line
    /// comments were already emitted as a trailing prefix on the opening delimiter's
    /// line (see `delimiter_line_comment_prefix`) — calling this for the first element
    /// avoids emitting them twice. `delim_pos` is the opening `<`/`(`/etc.
    pub(crate) fn build_leading_comments_multiline_after_delim(
        &self,
        start: u32,
        end: u32,
        delim_pos: u32,
    ) -> Vec<DocId> {
        self.build_leading_comments_multiline_opt(start, end, Some(delim_pos))
    }

    /// Shared body for `build_leading_comments_multiline` and its `_after_delim`
    /// variant. When `skip_delim` is `Some(pos)`, comments sharing `pos`'s source
    /// line are skipped (already emitted as the delimiter-line prefix).
    fn build_leading_comments_multiline_opt(
        &self,
        start: u32,
        end: u32,
        skip_delim: Option<u32>,
    ) -> Vec<DocId> {
        let d = self.d();
        let mut parts = Vec::new();
        for comment in comments_in_range(self.comments, start, end) {
            if skip_delim.is_some_and(|pos| self.comment_on_delimiter_line(pos, comment)) {
                continue; // pulled onto the delimiter line
            }
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

    /// True when a block comment in `(search_start, end)` sits on its own line —
    /// i.e. not on the same source line as `line_ref`.
    ///
    /// Used to force a parameter/element list to multiline when an own-line block
    /// comment follows the last element (`line_ref` = `search_start` = last elem end)
    /// or fills the opening-delimiter→first-element gap (`line_ref` = the delimiter,
    /// `search_start` = just past it). Line comments in the same position are detected
    /// separately (they always force a break).
    pub(crate) fn has_own_line_block_comment_after(
        &self,
        line_ref: u32,
        search_start: u32,
        end: u32,
    ) -> bool {
        comments_in_range(self.comments, search_start, end)
            .any(|c| c.is_block && !self.is_same_line(line_ref, c.span.start))
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

    /// Find the end position including any trailing same-line comments
    ///
    /// Used to correctly detect blank lines - need to check from after trailing
    /// comments, not just after the statement.
    pub(in crate::printer) fn find_end_with_trailing_comments(&self, after_pos: u32) -> u32 {
        let first_idx = tsv_lang::find_first_comment_from(self.comments, after_pos);
        let mut end = after_pos;
        // Track the "current line" reference — follows multi-line block comments
        // to their closing */ line (same logic as build_trailing_same_line_comment_docs)
        let mut line_ref = after_pos;

        for comment in &self.comments[first_idx..] {
            if self.is_same_line(line_ref, comment.span.start) {
                end = comment.span.end;
                // Follow multi-line block comments to their closing line
                if comment.is_block && !self.is_same_line(comment.span.start, comment.span.end) {
                    line_ref = comment.span.end;
                }
            } else {
                break;
            }
        }
        end
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

    /// Compute the "delimiter-line prefix" for the open-delimiter trailing-comment
    /// divergence (object literals, array literals, and block bodies).
    ///
    /// A comment on the same source line as the opening delimiter at `delim_pos`
    /// is kept on that line — instead of being relocated to its own line as the
    /// first element's leading comment (prettier's behavior). Returns the emitted
    /// prefix docs (` /* c */` / ` // c`, leading-space convention) and, when the
    /// pull fired, `Some(delim_pos)` — the position the caller passes back to
    /// exclude those same-line comments from the first element's leading set
    /// (`None` when nothing was pulled, so the prefix is empty).
    ///
    /// Gated on `should_force_expansion_for_comments`, so an inline block comment
    /// hugging the first element (`{ /* c */ a: 1 }`, `[/* c */ x]`) is left in
    /// place and the result is `(empty, None)`. See conformance_prettier.md
    /// §Comment relocation.
    pub(in crate::printer) fn delimiter_line_comment_prefix(
        &self,
        delim_pos: u32,
        first_elem_start: u32,
    ) -> (Vec<DocId>, Option<u32>) {
        let pc = super::calls::PartitionedComments::new(
            self.comments,
            self.line_breaks,
            delim_pos,
            first_elem_start,
        );
        let pull = (!pc.trailing_block.is_empty() || !pc.trailing_line.is_empty())
            && super::calls::should_force_expansion_for_comments(self, delim_pos, first_elem_start);
        let mut prefix = Vec::new();
        if pull {
            pc.emit_trailing_comments(&mut prefix, self);
        }
        (prefix, pull.then_some(delim_pos))
    }

    /// Whether `comment` was pulled onto the opening delimiter's line by
    /// `delimiter_line_comment_prefix` — i.e. it shares a source line with the
    /// delimiter at `delim_pos`.
    ///
    /// The prefix helper emits these comments on the delimiter's line; every
    /// consumer must then drop the same comments from the first element's
    /// leading-comment set so they aren't emitted twice. Centralizing the test
    /// keeps that exclusion in lockstep with what the prefix actually pulls.
    pub(in crate::printer) fn comment_on_delimiter_line(
        &self,
        delim_pos: u32,
        comment: &internal::Comment,
    ) -> bool {
        self.is_same_line(delim_pos, comment.span.start)
    }

    /// A first element/member's leading comments with the delimiter-line
    /// comments removed.
    ///
    /// `delimiter_line_comment_prefix` emits the comments sharing the opening
    /// delimiter's line as a prefix on that line, so every member-loop consumer
    /// must drop the same comments from the first element's leading set to avoid
    /// emitting them twice (see `comment_on_delimiter_line`). Returns `comments`
    /// unchanged when `delimiter_pull_pos` is `None` (nothing was pulled).
    pub(in crate::printer) fn first_member_leading_comments<'c>(
        &self,
        comments: Vec<&'c internal::Comment>,
        delimiter_pull_pos: Option<u32>,
    ) -> Vec<&'c internal::Comment> {
        match delimiter_pull_pos {
            Some(dpos) => comments
                .into_iter()
                .filter(|c| !self.comment_on_delimiter_line(dpos, c))
                .collect(),
            None => comments,
        }
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
            if !comment.content.contains('\n') {
                // Single-line block comment
                d.text_owned(format!("/*{}*/", comment.content))
            } else if printing::is_indentable_block_comment(&comment.content) {
                self.build_indentable_block_comment_doc(&comment.content)
            } else {
                self.build_preserved_block_comment_doc(comment)
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

    /// Frame a multi-line block comment's continuation docs (`inner`) with the
    /// `/*<first_line>` opener and the `*/` closer. `first_line` is the content
    /// of the line right after `/*` (trailing whitespace trimmed).
    fn frame_block_comment_doc(&self, first_line: &str, inner: Vec<DocId>) -> DocId {
        let d = self.d();
        let mut docs = Vec::with_capacity(inner.len() + 2);
        docs.push(d.text_owned(format!("/*{}", first_line.trim_end())));
        docs.extend(inner);
        docs.push(d.text("*/"));
        d.concat(&docs)
    }

    /// Build a multi-line *indentable* block comment (JSDoc `/** … */` and
    /// `*`-aligned `/* … */`, where every line begins with `*`).
    ///
    /// Continuation lines are reindented to a single leading space before the
    /// `*` — the context indent is supplied by the `hardline`, and content after
    /// the `*` is untouched. Mirrors prettier's `printIndentableBlockComment`.
    fn build_indentable_block_comment_doc(&self, content: &str) -> DocId {
        let d = self.d();
        // ≥2 lines: `build_comment_doc` only routes newline-containing content here.
        let lines: Vec<&str> = content.split('\n').collect();
        let [first, middle @ .., last] = lines.as_slice() else {
            unreachable!("multi-line comment");
        };

        let mut inner = Vec::with_capacity((middle.len() + 1) * 2);
        for line in middle {
            inner.push(d.hardline());
            inner.push(d.text_owned(format!(" {}", line.trim())));
        }
        // The last line (before `*/`) keeps trailing content via `trim_start`.
        inner.push(d.hardline());
        inner.push(d.text_owned(format!(" {}", last.trim_start())));

        self.frame_block_comment_doc(first, inner)
    }

    /// Build a multi-line *non-indentable* block comment (at least one line does
    /// not begin with `*`) — preserved with its original interior layout rather
    /// than reindented.
    ///
    /// The comment's own leading indentation is stripped, then re-applied via
    /// `hardline` for `/**`-prefixed comments or comments whose lines were
    /// indented; other comments preserve their lines at column 0 (`literalline`).
    fn build_preserved_block_comment_doc(&self, comment: &internal::Comment) -> DocId {
        let d = self.d();
        let stripped =
            printing::strip_comment_indentation(self.source, &comment.content, comment.span.start);

        // A `/**`-prefixed comment that reached here is only partially starred
        // (some line lacks `*`); it still gets context indent. Otherwise use
        // context indent only when the comment's lines were indented.
        let use_context_indent =
            comment.content.starts_with('*') || stripped.len() != comment.content.len();

        // ≥2 lines: `build_comment_doc` only routes newline-containing content here.
        let lines: Vec<&str> = stripped.split('\n').collect();
        let [first, middle @ .., last] = lines.as_slice() else {
            unreachable!("multi-line comment");
        };

        let mut inner = Vec::with_capacity((middle.len() + 1) * 2);
        for line in middle {
            // Blank lines stay truly empty (column 0); otherwise apply context
            // indent. Trailing whitespace is trimmed (matches prettier).
            inner.push(if line.is_empty() || !use_context_indent {
                d.literalline()
            } else {
                d.hardline()
            });
            inner.push(d.text_owned(line.trim_end().to_string()));
        }
        // Closing line gets context indent; its content (the space before `*/`)
        // is preserved verbatim.
        inner.push(if use_context_indent {
            d.hardline()
        } else {
            d.literalline()
        });
        inner.push(d.text_owned((*last).to_string()));

        self.frame_block_comment_doc(first, inner)
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
        for comment in comments_in_range(self.comments, type_params_end, paren_pos) {
            parts.push(self.build_trailing_comment_doc(comment));
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

    /// Build a doc for a single trailing comment (`expr /* c */` or `expr; // c`).
    ///
    /// A **block** comment is inline with a leading space — its width counts toward
    /// the line. A **line** comment goes through `line_suffix` (zero width), so a
    /// long trailing comment never forces a preceding group (e.g. a member's union
    /// type) to break. Shared by every spot that trails a comment on a member or
    /// inner type without semicolon-relative positioning.
    pub(crate) fn build_trailing_comment_doc(&self, comment: &internal::Comment) -> DocId {
        if comment.is_block {
            let d = self.d();
            d.concat(&[d.text(" "), self.build_comment_doc(comment)])
        } else {
            self.build_trailing_line_comment_doc(comment)
        }
    }

    /// Emit leading comments in `[keyword_end, value_start)` followed by
    /// `value_doc` broken onto its own indented line. Use when at least one line
    /// comment sits in the gap (a line comment forces the value down). The caller
    /// pushes the keyword/operator itself first, **without** a trailing space.
    ///
    /// A comment on the **same source line** as `keyword_end` trails the keyword
    /// inline — a block as ` /* c */`, a line comment via `line_suffix` (zero
    /// width, so a long trailing comment never forces a *preceding* group, e.g. a
    /// constraint/annotation union, to break — matching prettier's `lineSuffix`).
    /// Each **own-line** comment goes on its own line before the value; they are
    /// never joined onto one line (which would make a following `//` stop being a
    /// delimiter — a boundary loss). Shared by type-parameter constraint/default
    /// values (`= `/`extends`) and class-property initializers (`= `).
    pub(crate) fn append_keyword_value_line_comments(
        &self,
        parts: &mut Vec<DocId>,
        keyword_end: u32,
        value_start: u32,
        value_doc: DocId,
    ) {
        let d = self.d();
        let mut value_block = vec![d.hardline()];
        let mut on_own_line = false;
        for comment in comments_in_range(self.comments, keyword_end, value_start) {
            let same_line = !on_own_line && self.is_same_line(keyword_end, comment.span.start);
            if same_line {
                if comment.is_block {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                } else {
                    parts.push(self.build_trailing_line_comment_doc(comment));
                    on_own_line = true; // a line comment ends its line
                }
            } else {
                on_own_line = true;
                value_block.push(self.build_comment_doc(comment));
                value_block.push(d.hardline());
            }
        }
        value_block.push(value_doc);
        parts.push(d.indent(d.concat(&value_block)));
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

    /// Append the comments between a signature's last content token and the
    /// member's end (typically right before the printed `;`): after the return
    /// type, or after the params' closing `)` when there is no return type.
    ///
    /// Shared by method/call/construct signatures in interfaces and type
    /// literals, abstract/overload class methods, and declare functions.
    pub(crate) fn append_signature_end_comments(
        &self,
        parts: &mut Vec<DocId>,
        return_type: Option<&internal::TSTypeAnnotation>,
        paren_pos: Option<u32>,
        span_end: u32,
    ) {
        let d = self.d();
        let content_end = return_type.map_or_else(
            || {
                paren_pos
                    .and_then(|p| self.find_closing_paren(p, span_end))
                    .unwrap_or(span_end)
            },
            |rt| rt.span.end,
        );
        for comment in comments_in_range(self.comments, content_end, span_end) {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }
    }

    /// Append leading inline block comments (`/*content*/ ` format) between two positions.
    ///
    /// Only emits block comments; line comments are skipped (they would have been
    /// detected earlier and routed to the multiline path). Counterpart of
    /// [`Self::append_trailing_inline_block_comments`].
    pub(crate) fn append_leading_inline_block_comments(
        &self,
        parts: &mut Vec<DocId>,
        start: u32,
        end: u32,
    ) {
        let d = self.d();
        for comment in comments_in_range(self.comments, start, end) {
            if comment.is_block {
                parts.push(d.text_owned(format!("/*{}*/ ", comment.content)));
            }
        }
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

    /// Split the last list-member's trailing inline block comments around a source
    /// trailing comma (in `elem_end..end_boundary`): comments before the comma go
    /// to `before`, comments after it to `after`. Callers emit `after` past the
    /// synthetic trailing comma so the comment is preserved after the comma rather
    /// than relocated before it (see conformance_prettier.md §Comment relocation).
    pub(crate) fn append_last_trailing_block_comments_split(
        &self,
        before: &mut Vec<DocId>,
        after: &mut Vec<DocId>,
        elem_end: u32,
        end_boundary: u32,
    ) {
        match self
            .find_comma_after(elem_end)
            .filter(|cp| *cp < end_boundary)
        {
            Some(comma_pos) => {
                self.append_trailing_inline_block_comments(before, elem_end, comma_pos);
                self.append_trailing_inline_block_comments(after, comma_pos, end_boundary);
            }
            None => self.append_trailing_inline_block_comments(before, elem_end, end_boundary),
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
