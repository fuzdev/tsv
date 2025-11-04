//! Shared quote handling utilities for string literals
//!
//! Provides smart quote selection logic that minimizes escaping,
//! following prettier's `singleQuote: true` behavior.
//!
//! Used by TypeScript, CSS, and Svelte printers for consistent
//! quote handling across all contexts (except HTML static attributes,
//! which always use double quotes per HTML spec).
//!
//! # Future Work
//!
//! TODO: Research if string normalization utilities should be added here.
//! CSS currently has `normalize_css_string()` for handling Svelte parser quirks
//! (duplicate hex digits in unicode escapes). TypeScript and Svelte may need
//! similar normalization. Consider creating a shared normalization module if
//! patterns emerge across languages. See TODO_FIXES.md Category 1 followup item.

/// Quote counts for smart quote style selection
#[derive(Debug)]
pub struct QuoteCounts {
    escaped_single: usize,
    escaped_double: usize,
    unescaped_single: usize,
    unescaped_double: usize,
}

impl QuoteCounts {
    /// Count quote occurrences in normalized string content
    ///
    /// # Arguments
    /// * `content` - String content without surrounding quotes
    ///
    /// # Note
    /// Content should be normalized (escape sequences decoded) before counting.
    pub fn from_content(content: &str) -> Self {
        let escaped_single = content.matches("\\'").count();
        let escaped_double = content.matches("\\\"").count();
        let unescaped_single = content.matches('\'').count() - escaped_single;
        let unescaped_double = content.matches('"').count() - escaped_double;

        Self {
            escaped_single,
            escaped_double,
            unescaped_single,
            unescaped_double,
        }
    }

    /// Choose optimal quote style that minimizes escaping
    ///
    /// # Prettier Rules (singleQuote: true)
    ///
    /// 1. Default: single quotes `'hello'`
    /// 2. Contains `'`: use double quotes `"has a single quote"` (avoid escaping)
    /// 3. Contains `"`: use single quotes `'has "double quotes"'` (already optimal)
    /// 4. Contains both: use single + escape `'has \'both\' "types"'` (prefer single)
    ///
    /// # Returns
    /// The optimal quote character (`'` or `"`)
    pub fn choose_quote_style(&self) -> char {
        let single_quotes_needing_escape = self.escaped_single + self.unescaped_single;
        let double_quotes_needing_escape = self.escaped_double + self.unescaped_double;

        if single_quotes_needing_escape > 0 && double_quotes_needing_escape == 0 {
            // Only has ' (escaped or unescaped) → use double quotes
            '"'
        } else if double_quotes_needing_escape > 0 && single_quotes_needing_escape == 0 {
            // Only has " → use single quotes
            '\''
        } else if single_quotes_needing_escape > double_quotes_needing_escape {
            // More ' than " → use double quotes
            '"'
        } else {
            // Default or equal → prefer single quotes
            '\''
        }
    }
}

/// Adjust string escapes based on chosen quote style
///
/// # Arguments
/// * `content` - String content without surrounding quotes
/// * `quote` - The quote character to use (`'` or `"`)
/// * `counts` - Pre-computed quote counts (for optimization)
///
/// # Returns
/// String with escapes adjusted for the chosen quote style
///
/// # Examples
/// ```
/// use tsv_lang::quotes::{QuoteCounts, adjust_escapes_for_quote};
///
/// let content = "has\\' a single quote";
/// let counts = QuoteCounts::from_content(content);
/// let quote = counts.choose_quote_style(); // → '"'
/// let result = adjust_escapes_for_quote(content, quote, &counts);
/// assert_eq!(result, "has' a single quote"); // Removed unnecessary \' escape
/// ```
pub fn adjust_escapes_for_quote(content: &str, quote: char, counts: &QuoteCounts) -> String {
    if quote == '"' {
        // Using double quotes: remove \' escapes, ensure " is escaped
        let mut result = content.replace("\\'", "'");
        if counts.unescaped_double > 0 {
            result = result.replace('"', "\\\"");
            // Prevent double-escaping
            result = result.replace("\\\\\"", "\\\"");
        }
        result
    } else {
        // Using single quotes: remove \" escapes, ensure ' is escaped
        let mut result = content.replace("\\\"", "\"");
        if counts.unescaped_single > 0 {
            result = result.replace('\'', "\\'");
            // Prevent double-escaping
            result = result.replace("\\\\'", "\\'");
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_quotes() {
        let counts = QuoteCounts::from_content("str");
        assert_eq!(counts.choose_quote_style(), '\''); // Default single
    }

    #[test]
    fn test_contains_single_quote() {
        let counts = QuoteCounts::from_content("has 'single' quotes");
        assert_eq!(counts.choose_quote_style(), '"'); // Use double to avoid escaping the apostrophe
    }

    #[test]
    fn test_contains_double_quote() {
        let counts = QuoteCounts::from_content("has \"double quotes\"");
        assert_eq!(counts.choose_quote_style(), '\''); // Use single
    }

    #[test]
    fn test_contains_both_quotes() {
        let counts = QuoteCounts::from_content("has 'both' \"types\"");
        assert_eq!(counts.choose_quote_style(), '\''); // Prefer single
    }

    #[test]
    fn test_adjust_escapes_to_double() {
        let content = "has \\'single\\' quotes";
        let counts = QuoteCounts::from_content(content);
        let result = adjust_escapes_for_quote(content, '"', &counts);
        assert_eq!(result, "has 'single' quotes"); // Removed \' escape
    }

    #[test]
    fn test_adjust_escapes_to_single() {
        let content = "has \\\"double quotes\\\"";
        let counts = QuoteCounts::from_content(content);
        let result = adjust_escapes_for_quote(content, '\'', &counts);
        assert_eq!(result, "has \"double quotes\""); // Removed \" escape
    }

    #[test]
    fn test_adjust_escapes_add_single() {
        let content = "has 'both' \"types\"";
        let counts = QuoteCounts::from_content(content);
        let result = adjust_escapes_for_quote(content, '\'', &counts);
        assert_eq!(result, "has \\'both\\' \"types\""); // Added \' escape
    }

    #[test]
    fn test_adjust_escapes_add_double() {
        let content = "has 'both' \"types\"";
        let counts = QuoteCounts::from_content(content);
        let result = adjust_escapes_for_quote(content, '"', &counts);
        assert_eq!(result, "has 'both' \\\"types\\\""); // Added \" escape
    }
}
