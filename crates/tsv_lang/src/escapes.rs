//! Shared escape sequence utilities for string literals
//!
//! Provides utilities for manipulating escape sequences in raw string content,
//! used by printers when changing quote styles while preserving all other
//! escape sequences exactly as they appear in the source.

/// Swap quote escaping when changing quote styles in raw string content
///
/// When a printer changes the quote style of a string literal, it needs to:
/// 1. Unescape the old quote character (no longer needs escaping)
/// 2. Escape the new quote character (now needs escaping)
/// 3. Preserve all other escape sequences exactly as-is (\n, \t, \\, \u{...}, etc.)
///
/// This function operates on **raw content** (the string content between quotes,
/// with escape sequences still encoded as backslash sequences from the source).
///
/// # Arguments
/// * `content` - Raw string content without surrounding quotes (with escape sequences)
/// * `old_quote` - The quote character being changed from (`'` or `"`)
/// * `new_quote` - The quote character being changed to (`'` or `"`)
///
/// # Returns
/// String with quote escaping adjusted for the new quote style
///
/// # Examples
/// ```ignore
/// use tsv_lang::escapes::swap_quote_escaping;
///
/// // Single-quoted string with escaped single quote → double quotes
/// // Source: 'it\'s great' → raw content: it\'s great
/// // Result: "it's great" → raw content: it's great
/// let input = r"it\'s great";
/// let result = swap_quote_escaping(input, '\'', '"');
/// assert_eq!(result, r"it's great");  // \' → ' (unescaped)
///
/// // Single-quoted string with unescaped double quote → double quotes
/// // Source: 'has "double" quotes' → raw content: has "double" quotes
/// // Result: "has \"double\" quotes" → raw content: has \"double\" quotes
/// let input = r#"has "double" quotes"#;
/// let result = swap_quote_escaping(input, '\'', '"');
/// assert_eq!(result, r#"has \"double\" quotes"#);  // " → \" (escaped for new quotes)
/// ```
pub fn swap_quote_escaping(content: &str, old_quote: char, new_quote: char) -> String {
    if old_quote == new_quote {
        return content.to_string();
    }

    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(&next) = chars.peek() {
                if next == old_quote {
                    // Unescape old quote: \' → ' or \" → "
                    chars.next();
                    result.push(old_quote);
                } else if next == new_quote {
                    // Keep already-escaped new quote as-is: \" stays \" or \' stays \'
                    chars.next();
                    result.push('\\');
                    result.push(new_quote);
                } else {
                    // Keep all other escapes as-is: \n, \t, \\, \u{...}, etc.
                    chars.next();
                    result.push('\\');
                    result.push(next);
                }
            } else {
                // Trailing backslash (malformed, but preserve it)
                result.push('\\');
            }
        } else if ch == new_quote {
            // Escape unescaped new quote character: " → \" or ' → \'
            result.push('\\');
            result.push(new_quote);
        } else {
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_quote_escaping_same_quote() {
        // No change if quote stays the same
        assert_eq!(swap_quote_escaping("test", '\'', '\''), "test");
        assert_eq!(swap_quote_escaping(r"it\'s", '\'', '\''), r"it\'s");
    }

    #[test]
    fn test_swap_quote_escaping_single_to_double() {
        // Scenario: "has 'single' quotes" in single quotes with escapes
        // Input: 'has \'single\' quotes' → raw content: has \'single\' quotes
        // After swap to double quotes: "has 'single' quotes" → raw content: has 'single' quotes
        let input = "has \\'single\\' quotes";
        let expected = "has 'single' quotes";
        assert_eq!(swap_quote_escaping(input, '\'', '"'), expected);
    }

    #[test]
    fn test_swap_quote_escaping_double_to_single() {
        // Scenario: 'has "double" quotes' (single quotes, unescaped " inside)
        // Raw content: has "double" quotes (no escapes needed for " in single quotes)
        // Swap to double quotes: "has \"double\" quotes"
        // Raw content becomes: has \"double\" quotes (escape " for double quotes)
        let input = "has \"double\" quotes";
        let expected = "has \\\"double\\\" quotes";
        assert_eq!(swap_quote_escaping(input, '\'', '"'), expected);
    }

    #[test]
    fn test_swap_quote_escaping_with_both_quotes() {
        // Scenario: "has 'both' \"types\"" in single quotes
        // Input: 'has \'both\' "types"' → raw content: has \'both\' "types"
        // After swap to double quotes: "has 'both' "types"" → raw content: has 'both' \"types\"
        let input = "has \\'both\\' \"types\"";
        let expected = "has 'both' \\\"types\\\"";
        assert_eq!(swap_quote_escaping(input, '\'', '"'), expected);
    }

    #[test]
    fn test_swap_quote_escaping_preserves_other_escapes() {
        // Other escapes are preserved exactly (including unicode, hex, special chars)
        assert_eq!(swap_quote_escaping(r"test\n\t\\", '\'', '"'), r"test\n\t\\");
        assert_eq!(swap_quote_escaping(r"\u0041\x42", '\'', '"'), r"\u0041\x42");
        assert_eq!(
            swap_quote_escaping(r"line\nbreak", '"', '\''),
            r"line\nbreak"
        );
    }

    #[test]
    fn test_swap_quote_escaping_already_escaped_new_quote() {
        // Edge case: Input has escaped new quote even though using old quote
        // Example: 'has \"double\" quotes' (using single quotes but \" is escaped)
        // After swap to double quotes: should keep \" as-is
        let input = "has \\\"double\\\" quotes";
        let expected = "has \\\"double\\\" quotes";
        assert_eq!(swap_quote_escaping(input, '\'', '"'), expected);
    }

    #[test]
    fn test_swap_quote_escaping_mixed_with_other_escapes() {
        // Combination of quote escapes and other escapes
        let input = "it\\'s\\ngreat";
        let expected = "it's\\ngreat";
        assert_eq!(swap_quote_escaping(input, '\'', '"'), expected);
    }
}
