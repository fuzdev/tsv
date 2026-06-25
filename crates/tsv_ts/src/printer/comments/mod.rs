// Comment handling for TypeScript printer
//
// This module handles all comment-related operations:
// - Building Doc representations for comments
// - Printing comments directly to buffer
// - Finding and filtering comments in ranges
// - Handling leading/trailing/inline comments
//
// ## Module Organization
//
// - **mod.rs** (this file): The `CommentSpacing` / `CommentFilter` enums and the
//   generic comment-emission primitives every other module builds on.
// - **render.rs**: Single-comment text-layout leaves (block-comment framing,
//   indentable / preserved block comments, trailing line/block comment docs).
// - **paren.rs**: Stripped-grouping-paren comment handling (promotion across `=`
//   / operators, trailing-paren comment preservation, removed-paren prepends).
// - **scan.rs**: Pure source span-math helpers (comma/angle/blank-line scanning).
// - **declarations.rs**: Member-keyword / modifier-marker / marker→colon /
//   heritage / keyword→name comment emitters.
// - **lists.rs**: List- and body-level comment emitters (leading/trailing body
//   comments, delimiter-line prefixes, empty-container comments, comma emission).
// - **element_comma.rs**: The single source of the `trailingComma: 'none'`
//   comment-position contract for inline element lists (block-before / comma /
//   block-after-on-last / line-suffix), shared by the object/array pattern and
//   object-literal builders.

mod declarations;
mod element_comma;
mod lists;
mod paren;
mod render;
mod scan;

// Re-export for submodules to use `super::X` instead of `super::super::X`.
pub(super) use super::{Printer, calls, layout};

use tsv_lang::comments_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

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
        let mut parts = DocBuf::new();
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
        let mut parts = DocBuf::new();
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
        let mut parts = DocBuf::new();
        for comment in comments_in_range(self.comments, start, end) {
            parts.push(self.build_comment_doc(comment));
            if comment.is_block {
                if comment.multiline {
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

    /// Prepend optional RHS leading comments — block comments in the gap between an
    /// `=`/`:` and the value (`build_rhs_comments_opt`) — to an already-built
    /// `value_doc`, returning `value_doc` unchanged when the gap carries none.
    /// Centralizes the `match build_rhs_comments_opt { Some(c) => concat([c, v]),
    /// None => v }` idiom shared by the initializer/property value sites (variable
    /// declarators, class properties, enum members, object property values).
    pub(crate) fn prepend_rhs_comments(
        &self,
        value_doc: DocId,
        start: u32,
        value_start: u32,
    ) -> DocId {
        match self.build_rhs_comments_opt(start, value_start) {
            Some(comments_doc) => self.d().concat(&[comments_doc, value_doc]),
            None => value_doc,
        }
    }
}
