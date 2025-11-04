// Shared printing utilities for printers
//
// This module provides common printing logic used across language printers
// (TypeScript, CSS, Svelte) to eliminate code duplication.

use crate::escapes::swap_quote_escaping;

/// Options for string literal formatting
#[derive(Debug, Clone, Copy)]
pub struct StringFormatOptions {
    /// Prefer single quotes over double quotes (when counts are equal)
    /// Default: true (matches prettier-plugin-svelte)
    pub prefer_single_quotes: bool,
}

impl Default for StringFormatOptions {
    fn default() -> Self {
        Self {
            prefer_single_quotes: true,
        }
    }
}

/// Format a string literal with optimal quote selection
///
/// Takes raw string content (with escape sequences preserved) and formats it
/// by choosing the optimal quote character to minimize escaping.
///
/// # Algorithm
///
/// 1. Count single and double quotes in the content
/// 2. Choose quote that appears less frequently (minimize escaping)
/// 3. On tie, prefer single quotes (prettier default)
/// 4. If quote changed, swap escape sequences
/// 5. Return formatted string with quotes
///
/// # Arguments
///
/// * `raw_content` - String content without surrounding quotes (with escapes preserved)
/// * `original_quote` - The quote character in the original source (`'` or `"`)
/// * `options` - Formatting options (quote preference)
///
/// # Returns
///
/// Formatted string literal including surrounding quotes
///
/// # Examples
///
/// ```
/// use tsv_lang::printing::{format_string_literal, StringFormatOptions};
///
/// // String with no quotes - uses preferred quote (single)
/// let result = format_string_literal("hello", '"', StringFormatOptions::default());
/// assert_eq!(result, "'hello'");
///
/// // String with single quotes - switches to double to avoid escaping
/// let result = format_string_literal("it's nice", '\'', StringFormatOptions::default());
/// assert_eq!(result, r#""it's nice""#);
///
/// // String with double quotes - stays single to minimize escaping
/// let result = format_string_literal(r#"say "hi""#, '\'', StringFormatOptions::default());
/// assert_eq!(result, r#"'say "hi"'"#);
///
/// // Preserves escape sequences
/// let result = format_string_literal(r"\u0041\n", '"', StringFormatOptions::default());
/// assert_eq!(result, r"'\u0041\n'");
/// ```
pub fn format_string_literal(
    raw_content: &str,
    original_quote: char,
    options: StringFormatOptions,
) -> String {
    // Count quotes in the raw content (with escapes) to make the best choice
    let single_count = raw_content.matches('\'').count();
    let double_count = raw_content.matches('"').count();

    // Choose optimal quote: less frequent quote = less escaping needed
    let optimal_quote = if double_count < single_count {
        '"'
    } else if single_count < double_count {
        '\''
    } else {
        // Tie-breaker: use preference (default: single quotes)
        if options.prefer_single_quotes {
            '\''
        } else {
            '"'
        }
    };

    // Swap quote escaping if needed, or use raw content as-is
    let final_content = if optimal_quote == original_quote {
        raw_content.to_string()
    } else {
        swap_quote_escaping(raw_content, original_quote, optimal_quote)
    };

    // Return formatted string with quotes
    format!("{}{}{}", optimal_quote, final_content, optimal_quote)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_quotes_uses_preferred() {
        let result = format_string_literal("hello", '"', StringFormatOptions::default());
        assert_eq!(result, "'hello'");
    }

    #[test]
    fn test_switches_to_minimize_escaping() {
        // Has single quote - switch to double
        let result = format_string_literal("it's", '\'', StringFormatOptions::default());
        assert_eq!(result, r#""it's""#);

        // Has double quote - stay single
        let result = format_string_literal(r#"say "hi""#, '\'', StringFormatOptions::default());
        assert_eq!(result, r#"'say "hi"'"#);
    }

    #[test]
    fn test_preserves_escape_sequences() {
        let result = format_string_literal(r"\u0041\n\t", '"', StringFormatOptions::default());
        assert_eq!(result, r"'\u0041\n\t'");
    }

    #[test]
    fn test_swaps_quote_escaping_when_changing_quotes() {
        // Original: "it\'s" with single quote
        // After: "it's" with double quote (unescape the single quote)
        let result = format_string_literal(r"it\'s", '\'', StringFormatOptions::default());
        assert_eq!(result, r#""it's""#);
    }

    #[test]
    fn test_prefer_double_quotes_option() {
        let options = StringFormatOptions {
            prefer_single_quotes: false,
        };
        let result = format_string_literal("hello", '\'', options);
        assert_eq!(result, r#""hello""#);
    }

    #[test]
    fn test_already_optimal_quote() {
        // Already using single quotes, no change needed
        let result = format_string_literal("hello", '\'', StringFormatOptions::default());
        assert_eq!(result, "'hello'");
    }

    #[test]
    fn test_many_quotes_chooses_less_frequent() {
        // 3 double quotes vs 1 single quote - choose single (minimize escaping)
        // Original (with double quotes): "a "b" "c" "d" e's"
        // After switching to single: 'a "b" "c" "d" e\'s' (single quote gets escaped)
        let content = r#"a "b" "c" "d" e's"#;
        let result = format_string_literal(content, '"', StringFormatOptions::default());
        // Expected: single quote wrapper, double quotes unescaped, single quote escaped
        assert_eq!(result, "'a \"b\" \"c\" \"d\" e\\'s'");
    }
}
