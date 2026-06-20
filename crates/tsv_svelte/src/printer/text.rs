// Text analysis helpers for Svelte template whitespace
//
// Leading/trailing-whitespace and newline/blank-line predicates over `str`, used by the
// printer's boundary-trim and blank-line decisions. (Whitespace *collapsing* itself lives
// in the doc-based content path — `build_text_fill_doc_trimmed` and friends.)

// Helper trait for text analysis
pub(crate) trait TextAnalysis {
    fn is_whitespace_only(&self) -> bool;
    fn count_newlines(&self) -> usize;
    fn has_blank_line(&self) -> bool;

    // Leading/trailing whitespace analysis
    fn leading_whitespace(&self) -> &str;
    fn trailing_whitespace(&self) -> &str;
    fn has_leading_space_only(&self) -> bool;
    fn has_trailing_space_only(&self) -> bool;
}

impl TextAnalysis for str {
    /// Check if string contains only collapsible (ASCII) whitespace
    ///
    /// Uses the ASCII whitespace class `[\t\n\f\r ]` — matching
    /// prettier-plugin-svelte's text split (`splitTextToDocs`: `text.split(/[\t\n\f\r ]+/)`).
    /// Non-breaking spaces (U+00A0 / U+202F) and other Unicode whitespace (NEL,
    /// em-spaces, ideographic space, vertical tab) are content, not collapsible
    /// whitespace, so a node made of only those is NOT whitespace-only.
    fn is_whitespace_only(&self) -> bool {
        self.bytes().all(|b| b.is_ascii_whitespace())
    }

    /// Count newlines in the string
    fn count_newlines(&self) -> usize {
        self.chars().filter(|&c| c == '\n').count()
    }

    /// Check if string contains a blank line (2+ newlines)
    fn has_blank_line(&self) -> bool {
        self.count_newlines() >= 2
    }

    /// Get the leading (ASCII) whitespace portion of the string
    ///
    /// Stops at the first non-ASCII-whitespace byte, so a leading non-breaking
    /// space counts as content, not whitespace (see `is_whitespace_only`).
    fn leading_whitespace(&self) -> &str {
        let trimmed = self.trim_ascii_start();
        &self[..self.len() - trimmed.len()]
    }

    /// Get the trailing (ASCII) whitespace portion of the string
    ///
    /// Stops at the last non-ASCII-whitespace byte, so a trailing non-breaking
    /// space counts as content, not whitespace (see `is_whitespace_only`).
    fn trailing_whitespace(&self) -> &str {
        let trimmed = self.trim_ascii_end();
        &self[trimmed.len()..]
    }

    /// Check if leading whitespace is space/tab only (no newline)
    fn has_leading_space_only(&self) -> bool {
        let ws = self.leading_whitespace();
        !ws.is_empty() && !ws.contains('\n')
    }

    /// Check if trailing whitespace is space/tab only (no newline)
    fn has_trailing_space_only(&self) -> bool {
        let ws = self.trailing_whitespace();
        !ws.is_empty() && !ws.contains('\n')
    }
}

#[cfg(test)]
mod tests {
    use super::TextAnalysis;

    #[test]
    fn whitespace_only_uses_ascii_class() {
        assert!("  \t\n ".is_whitespace_only());
        assert!("".is_whitespace_only());
        // NBSP (U+00A0) is content, not collapsible whitespace.
        assert!(!"\u{00A0}".is_whitespace_only());
        assert!(!"a".is_whitespace_only());
    }

    #[test]
    fn newline_counting_and_blank_line() {
        assert_eq!("a\nb".count_newlines(), 1);
        assert_eq!("a\n\nb".count_newlines(), 2);
        assert!("a\n\nb".has_blank_line());
        assert!(!"a\nb".has_blank_line());
    }

    #[test]
    fn leading_and_trailing_whitespace_analysis() {
        assert_eq!("  \nx".leading_whitespace(), "  \n");
        assert!(!"  \nx".has_leading_space_only());

        assert_eq!("x  ".trailing_whitespace(), "  ");
        assert!("x  ".has_trailing_space_only());
    }
}
