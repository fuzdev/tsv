// Text formatting and whitespace normalization for Svelte templates
//
// Handles whitespace collapsing according to HTML rendering semantics:
// - Block context: trim completely
// - Inline context: preserve single space at boundaries
// - Pre elements: preserve exactly as-is

use crate::ast::internal;
use crate::printer::Printer;

// Helper trait for text analysis
pub trait TextAnalysis {
    fn is_whitespace_only(&self) -> bool;
    fn has_content(&self) -> bool;
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
    /// Check if string contains only whitespace
    fn is_whitespace_only(&self) -> bool {
        self.trim().is_empty()
    }

    /// Check if string has non-whitespace content
    fn has_content(&self) -> bool {
        !self.trim().is_empty()
    }

    /// Count newlines in the string
    fn count_newlines(&self) -> usize {
        self.chars().filter(|&c| c == '\n').count()
    }

    /// Check if string contains a blank line (2+ newlines)
    fn has_blank_line(&self) -> bool {
        self.count_newlines() >= 2
    }

    /// Get the leading whitespace portion of the string
    fn leading_whitespace(&self) -> &str {
        let trimmed = self.trim_start();
        &self[..self.len() - trimmed.len()]
    }

    /// Get the trailing whitespace portion of the string
    fn trailing_whitespace(&self) -> &str {
        let trimmed = self.trim_end();
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
    /// Format a Text node
    ///
    /// # Parameters
    /// - `parent_is_block`: Whether the parent element is a block element
    ///   - `true`: trim text completely (block context)
    ///   - `false`: preserve single space at boundaries (inline context)
    /// - `parent_preserves_ws`: Whether the parent preserves whitespace (like `<pre>`)
    pub(super) fn print_text(
        &mut self,
        text: &internal::Text,
        parent_is_block: bool,
        parent_preserves_ws: bool,
    ) {
        // If parent preserves whitespace (like <pre>), write text exactly as-is
        if parent_preserves_ws {
            self.write(&text.raw);
            return;
        }

        // Otherwise, normalize whitespace (collapse to single space, trim based on context)
        let normalized = self.normalize_whitespace(&text.raw, parent_is_block);

        // TODO: Handle entity escaping (&nbsp;, &lt;, etc.)
        self.write(&normalized);
    }

    /// Check if text has leading whitespace
    ///
    /// Returns true if the first character is whitespace.
    fn has_leading_whitespace(text: &str) -> bool {
        text.chars().next().is_some_and(char::is_whitespace)
    }

    /// Check if text has trailing whitespace
    ///
    /// Returns true if the last character is whitespace.
    fn has_trailing_whitespace(text: &str) -> bool {
        text.chars().last().is_some_and(char::is_whitespace)
    }

    /// Normalize whitespace in text content
    ///
    /// Collapses consecutive whitespace (spaces, tabs, newlines) to a single space.
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

        // Fast path: whitespace-only text
        if text.trim().is_empty() {
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
            if ch.is_whitespace() {
                // Only emit space if we have content and weren't already in whitespace
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
