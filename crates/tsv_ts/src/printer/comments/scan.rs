// Pure source span-math helpers for comment handling.
//
// These scan the raw source bytes to locate delimiters (commas, the assertion
// `>`), the last comma in a range, blank-line breaks, and the end position
// including trailing same-line comments — skipping over comments and strings so
// glyphs inside them aren't mistaken for the real token.

use super::Printer;
use super::analysis::skip_string_or_comment;
use tsv_lang::source_scan::find_char_skipping_comments;

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

    /// Find an angle-bracket type assertion's closing `>` in `[start, end)`,
    /// skipping any `>` that sits inside a comment or string (`<T /* > */>x`).
    ///
    /// `start` is the type's end, `end` the asserted expression's start, so the
    /// first bare `>` between them is the cast's close. Returns `end` as a safe
    /// fallback if none is found (an impossible shape for a valid assertion) —
    /// that routes any in-range comments to the before-`>` side rather than
    /// dropping them.
    pub(crate) fn find_assertion_close_angle(&self, start: u32, end: u32) -> u32 {
        let source = self.source.as_bytes();
        let end_usize = end as usize;
        let mut i = start as usize;
        while i < end_usize {
            if source[i] == b'>' {
                return i as u32;
            }
            if let Some(skip) = skip_string_or_comment(source, i, end_usize) {
                i = skip;
            }
            i += 1;
        }
        end
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
}
