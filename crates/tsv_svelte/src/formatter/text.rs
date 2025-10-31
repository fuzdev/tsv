// Text formatting and whitespace normalization for Svelte templates
//
// Handles whitespace collapsing according to HTML rendering semantics:
// - Block context: trim completely
// - Inline context: preserve single space at boundaries
// - Pre elements: preserve exactly as-is

use crate::ast::internal;
use crate::formatter_core::Formatter;

// Helper trait for text analysis
pub trait TextAnalysis {
    fn is_whitespace_only(&self) -> bool;
    fn has_content(&self) -> bool;
    fn count_newlines(&self) -> usize;
    fn has_blank_line(&self) -> bool;
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
}

impl Formatter {
    /// Format a Text node
    ///
    /// # Parameters
    /// - `parent_is_block`: Whether the parent element is a block element
    ///   - `true`: trim text completely (block context)
    ///   - `false`: preserve single space at boundaries (inline context)
    /// - `parent_preserves_ws`: Whether the parent preserves whitespace (like `<pre>`)
    pub(super) fn format_text(
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
        text.chars().next().is_some_and(|c| c.is_whitespace())
    }

    /// Check if text has trailing whitespace
    ///
    /// Returns true if the last character is whitespace.
    fn has_trailing_whitespace(text: &str) -> bool {
        text.chars().last().is_some_and(|c| c.is_whitespace())
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

        // Collapse consecutive whitespace to single space
        // TODO(performance): Pre-allocating text.len() may be excessive since whitespace
        // typically collapses to ~1/3 or less. Consider `text.len() / 3 + 10` if profiling
        // shows many reallocations, though modern allocators handle this well.
        let mut result = String::with_capacity(text.len());
        let mut prev_was_whitespace = false;

        for ch in text.chars() {
            let is_whitespace = ch.is_whitespace();

            if is_whitespace {
                if !prev_was_whitespace {
                    result.push(' '); // Collapse to single space
                    prev_was_whitespace = true;
                }
            } else {
                result.push(ch);
                prev_was_whitespace = false;
            }
        }

        if trim_completely {
            // Block elements: remove all leading/trailing whitespace
            result.trim().to_string()
        } else {
            // Inline elements: preserve single space at boundaries if original had whitespace
            let mut final_result = result.trim().to_string();

            // Special case: if original was all whitespace, return single space
            // This preserves spacing between inline elements (e.g., between two <span> tags)
            if final_result.is_empty() && (has_leading_ws || has_trailing_ws) {
                return " ".to_string();
            }

            if has_leading_ws && !final_result.is_empty() {
                final_result.insert(0, ' ');
            }
            if has_trailing_ws && !final_result.is_empty() {
                final_result.push(' ');
            }

            final_result
        }
    }
}
