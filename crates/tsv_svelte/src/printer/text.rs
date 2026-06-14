// Text formatting and whitespace normalization for Svelte templates
//
// Handles whitespace collapsing according to HTML rendering semantics:
// - Block context: trim completely
// - Inline context: preserve single space at boundaries
// - Pre elements: preserve exactly as-is

use crate::printer::Printer;

// Helper trait for text analysis
pub(crate) trait TextAnalysis {
    fn is_whitespace_only(&self) -> bool;
    fn count_newlines(&self) -> usize;
    fn has_blank_line(&self) -> bool;

    // Leading/trailing whitespace analysis
    fn leading_whitespace(&self) -> &str;
    fn trailing_whitespace(&self) -> &str;
    fn has_leading_newline(&self) -> bool;
    fn has_trailing_newline(&self) -> bool;
    fn has_leading_space_only(&self) -> bool;
    fn has_trailing_space_only(&self) -> bool;
    fn has_trailing_blank_line(&self) -> bool;
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

    /// Check if leading whitespace contains a newline
    fn has_leading_newline(&self) -> bool {
        self.leading_whitespace().contains('\n')
    }

    /// Check if trailing whitespace contains a newline
    fn has_trailing_newline(&self) -> bool {
        self.trailing_whitespace().contains('\n')
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

    /// Check if trailing whitespace contains a blank line (2+ newlines)
    fn has_trailing_blank_line(&self) -> bool {
        self.trailing_whitespace().has_blank_line()
    }
}

impl<'a> Printer<'a> {
    /// Check if text has leading ASCII whitespace
    ///
    /// Returns true if the first character is ASCII whitespace. Non-breaking
    /// spaces (U+00A0 / U+202F) are content, not collapsible whitespace, matching
    /// Prettier's `/[\t\n\f\r ]+/` whitespace class.
    fn has_leading_whitespace(text: &str) -> bool {
        text.starts_with(|c: char| c.is_ascii_whitespace())
    }

    /// Check if text has trailing ASCII whitespace
    ///
    /// Returns true if the last character is ASCII whitespace. Non-breaking spaces
    /// are content, not collapsible whitespace.
    fn has_trailing_whitespace(text: &str) -> bool {
        text.ends_with(|c: char| c.is_ascii_whitespace())
    }

    /// Normalize whitespace in text content
    ///
    /// Collapses consecutive ASCII whitespace (spaces, tabs, newlines) to a single
    /// space. Non-breaking spaces (U+00A0 / U+202F) are preserved verbatim — they
    /// are content, not collapsible whitespace (matching Prettier's word split).
    ///
    /// # Parameters
    /// - `trim_completely`: If true, also trims leading/trailing whitespace (block context).
    ///   If false, preserves single space at boundaries (inline context).
    ///
    /// # Examples
    /// ```text
    /// Block context (trim_completely = true):
    ///   "  hello   world  " → "hello world"
    ///
    /// Inline context (trim_completely = false):
    ///   "  hello   world  " → " hello world "
    ///   "   " → " " (preserves spacing between inline elements)
    /// ```
    pub(super) fn normalize_whitespace(&self, text: &str, trim_completely: bool) -> String {
        if text.is_empty() {
            return String::new();
        }

        let has_leading_ws = Self::has_leading_whitespace(text);
        let has_trailing_ws = Self::has_trailing_whitespace(text);

        // Fast path: ASCII-whitespace-only text (a non-breaking space counts as
        // content and falls through to the loop below, preserved verbatim).
        if text.bytes().all(|b| b.is_ascii_whitespace()) {
            return if trim_completely || (!has_leading_ws && !has_trailing_ws) {
                String::new()
            } else {
                " ".to_string()
            };
        }

        // Single-pass: collapse whitespace, optionally preserving boundary spaces
        let mut result = String::with_capacity(text.len());
        let mut in_whitespace = true; // Start true to handle leading whitespace
        let mut has_content = false;

        for ch in text.chars() {
            if ch.is_ascii_whitespace() {
                // Only emit space if we have content and weren't already in whitespace.
                // Non-breaking spaces are not ASCII whitespace, so they fall to the
                // else branch and are preserved verbatim as content.
                if has_content && !in_whitespace {
                    result.push(' ');
                }
                in_whitespace = true;
            } else {
                // First non-whitespace: add leading space for inline mode
                if !has_content && !trim_completely && has_leading_ws {
                    result.push(' ');
                }
                result.push(ch);
                in_whitespace = false;
                has_content = true;
            }
        }

        // Remove trailing collapsed space (we'll add it back if needed for inline mode)
        if in_whitespace && result.ends_with(' ') && has_content {
            result.pop();
        }

        // Add trailing space for inline mode if original had trailing whitespace
        if !trim_completely && has_trailing_ws && has_content {
            result.push(' ');
        }

        result
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
        assert!("  \nx".has_leading_newline());
        assert!(!"  \nx".has_leading_space_only());

        assert_eq!("x  ".trailing_whitespace(), "  ");
        assert!("x  ".has_trailing_space_only());
        assert!(!"x  ".has_trailing_newline());

        // A trailing blank line needs 2+ newlines in the trailing whitespace.
        assert!("x\n\n".has_trailing_blank_line());
        assert!(!"x\n".has_trailing_blank_line());
    }

    /// Build a bare `Printer` (no comments) just to reach `normalize_whitespace`,
    /// which uses no interner/comment/source state of its own.
    fn printer(source: &str) -> crate::printer::Printer<'_> {
        use std::cell::RefCell;
        use std::rc::Rc;
        use string_interner::DefaultStringInterner;
        let interner = Rc::new(RefCell::new(DefaultStringInterner::new()));
        crate::printer::Printer::new(source, interner, &[])
    }

    #[test]
    fn normalize_whitespace_block_vs_inline() {
        let p = printer("");
        // Block context (trim_completely = true): collapse runs and trim both ends.
        assert_eq!(
            p.normalize_whitespace("  hello   world  ", true),
            "hello world"
        );
        // Inline context: collapse runs but keep one boundary space each side.
        assert_eq!(
            p.normalize_whitespace("  hello   world  ", false),
            " hello world "
        );
    }

    #[test]
    fn normalize_whitespace_all_whitespace_fast_path() {
        let p = printer("");
        // Inline all-whitespace collapses to a single space; block to empty.
        assert_eq!(p.normalize_whitespace("   ", false), " ");
        assert_eq!(p.normalize_whitespace("   ", true), "");
        // Empty stays empty.
        assert_eq!(p.normalize_whitespace("", false), "");
    }

    #[test]
    fn normalize_whitespace_preserves_nbsp_as_content() {
        let p = printer("");
        // A non-breaking space is content, not collapsible whitespace.
        assert_eq!(p.normalize_whitespace("a\u{00A0}b", true), "a\u{00A0}b");
        assert_eq!(
            p.normalize_whitespace("\u{00A0}hello\u{00A0}", false),
            "\u{00A0}hello\u{00A0}"
        );
    }
}
