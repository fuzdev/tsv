//! CSS escape sequence handling utilities
//!
//! This module provides centralized escape handling for CSS, consolidating logic
//! previously spread across lexer, parser, and printer.
//!
//! # Architecture
//!
//! - **Decode**: Parse CSS escape sequences into their semantic meaning
//! - **Encode**: Convert semantic strings back to valid CSS
//!
//! # Svelte Quirks (historical)
//!
//! [`apply_svelte_quirks()`] implements backslash doubling and unicode first-digit
//! duplication that older Svelte versions applied during CSS value parsing. Current
//! Svelte versions no longer apply these quirks, so the conversion layer passes raw
//! source values through unchanged. The function is retained for reference and testing.

/// Apply Svelte compatibility quirks to CSS value string.
///
/// **Used by**: AST conversion layer (JSON output) - extracts source via span, applies quirks
/// **Not used by**: Printer (which normalizes these quirks away)
///
/// This function applies two transformations:
/// 1. **Backslash doubling**: All `\` become `\\`
/// 2. **Unicode first-digit duplication**: `\0001F4A9` becomes `\00001F4A9`
///
/// # Why These Quirks Exist
///
/// - **Backslash doubling**: Svelte's `read_value()` manually escapes backslashes
///   when building the value string (see style.js:514)
/// - **Unicode duplication**: Likely a bug in Svelte's CSS parser, but exact
///   location unknown
///
/// We replicate these quirks ONLY in AST/JSON output for 100% compatibility.
/// The printer produces clean CSS by normalizing these quirks away.
///
/// # Examples
///
/// ```
/// use tsv_css::escapes::apply_svelte_quirks;
///
/// // Backslash doubling + character duplication
/// assert_eq!(
///     apply_svelte_quirks(r#""test \ slash""#),
///     r#""test \\  slash""#  // Backslash doubled + space duplicated
/// );
///
/// // Unicode first-digit duplication + backslash doubling
/// assert_eq!(
///     apply_svelte_quirks(r"'\0001F4A9'"),
///     r"'\\00001F4A9'"  // 2 backslashes + 5 zeros (first 0 duplicated)
/// );
///
/// // No escapes - pass through unchanged
/// assert_eq!(
///     apply_svelte_quirks("normal text"),
///     "normal text"
/// );
/// ```
///
/// # Implementation Note
///
/// This function preserves all other content exactly, including whitespace,
/// quotes, and special characters. Only backslashes are modified.
pub fn apply_svelte_quirks(source: &str) -> String {
    let mut result = String::with_capacity(source.len() * 2);
    let mut chars = source.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // Svelte's quirk: when it sees `\X...`, it outputs: `\` + `\\X` + `X...`
            // This happens because:
            // 1. When it sees `\`, it adds `\` and sets escaped=true
            // 2. When it sees `X`, it adds `\\X` (because escaped=true), then adds `X` again
            // Result: backslash doubled + FIRST CHARACTER duplicated
            //
            // Examples:
            // - `\'` → `\\'\'` (backslash doubled, `'` duplicated)
            // - `\41` → `\\441` (backslash doubled, `4` duplicated, `1` preserved)
            // - `\0001F4A9` → `\\00001F4A9` (backslash doubled, first `0` duplicated, rest preserved)

            if let Some(first_ch) = chars.next() {
                // Add backslash twice
                result.push('\\');
                result.push('\\');

                // Add first character after backslash twice
                result.push(first_ch); // First occurrence
                result.push(first_ch); // Duplicated (Svelte quirk)

                // For unicode escapes, continue collecting remaining hex digits
                if first_ch.is_ascii_hexdigit() {
                    // Collect remaining hex digits (up to 5 more, for max 6 total)
                    for _ in 0..5 {
                        match chars.peek() {
                            Some(&digit) if digit.is_ascii_hexdigit() => {
                                chars.next();
                                result.push(digit);
                            }
                            _ => break,
                        }
                    }
                }
            } else {
                // Backslash at end of string - double it
                result.push('\\');
                result.push('\\');
            }
        } else {
            // Regular character - copy as-is
            result.push(ch);
        }
    }

    result
}

/// Decode CSS escape sequences in a string.
///
/// Converts CSS escape sequences to their actual character values:
/// - `\\` → `\` (escaped backslash)
/// - `\"` → `"` (escaped quote)
/// - `\'` → `'` (escaped quote)
/// - `\n` → newline (escaped newline)
/// - `\XXXXXX` → Unicode character (1-6 hex digits)
///
/// # Examples
///
/// ```
/// use tsv_css::escapes::decode_escape_sequences;
///
/// // Simple escapes
/// assert_eq!(decode_escape_sequences(r#"\"hello\""#), r#""hello""#);
/// assert_eq!(decode_escape_sequences(r"\'world\'"), r"'world'");
/// assert_eq!(decode_escape_sequences(r"test\\slash"), r"test\slash");
///
/// // Unicode escapes (CSS supports max 6 hex digits)
/// assert_eq!(decode_escape_sequences(r"\41"), "A");  // U+0041 = A
/// assert_eq!(decode_escape_sequences(r"\1F4A9"), "💩");  // U+1F4A9 = pile of poo emoji (5 digits)
/// ```
///
/// # Note
///
/// This function does NOT apply Svelte quirks - it performs standard CSS
/// escape decoding. Use [`apply_svelte_quirks()`] for Svelte compatibility.
pub fn decode_escape_sequences(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(&next_ch) = chars.peek() {
                if next_ch.is_ascii_hexdigit() {
                    // Unicode escape sequence
                    let mut hex_digits = String::new();
                    for _ in 0..6 {
                        match chars.peek() {
                            Some(&digit) if digit.is_ascii_hexdigit() => {
                                chars.next();
                                hex_digits.push(digit);
                            }
                            _ => break,
                        }
                    }

                    // Convert hex to character
                    if let Ok(code_point) = u32::from_str_radix(&hex_digits, 16)
                        && let Some(unicode_char) = char::from_u32(code_point)
                    {
                        result.push(unicode_char);

                        // Skip optional whitespace after unicode escape
                        if let Some(&ws) = chars.peek()
                            && ws.is_whitespace()
                        {
                            chars.next();
                        }
                        continue;
                    }

                    // Invalid unicode - keep as-is
                    result.push('\\');
                    result.push_str(&hex_digits);
                } else if let Some(escaped) = chars.next() {
                    // Simple escape - consume next char
                    match escaped {
                        'n' => result.push('\n'),
                        'r' => result.push('\r'),
                        't' => result.push('\t'),
                        '\\' | '"' | '\'' => result.push(escaped),
                        _ => {
                            // Unknown escape - keep both characters
                            result.push('\\');
                            result.push(escaped);
                        }
                    }
                }
            } else {
                // Backslash at end - keep it
                result.push('\\');
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Encode a string for use in CSS, escaping special characters.
///
/// Escapes characters that have special meaning in CSS:
/// - Backslash (`\`) → `\\`
/// - Quote characters (based on `prefer_quote`) → `\"`/`\'`
/// - Newlines → `\n` (or preserve as-is if `preserve_newlines` is true)
///
/// # Quote Selection
///
/// The function intelligently selects which quote character to use:
/// 1. If `prefer_quote` is specified, use that (and escape it)
/// 2. Otherwise, count single vs double quotes and use the less frequent one
/// 3. Tie-breaker: prefer double quotes
///
/// # Examples
///
/// ```
/// use tsv_css::escapes::encode_for_css;
///
/// // Prefer double quotes
/// assert_eq!(
///     encode_for_css("hello world", Some('"')),
///     (r#""hello world""#.to_string(), '"')
/// );
///
/// // Prefer single quotes
/// assert_eq!(
///     encode_for_css("hello world", Some('\'')),
///     (r"'hello world'".to_string(), '\'')
/// );
///
/// // Auto-select based on content
/// let (encoded, quote) = encode_for_css(r#"it's "quoted" text"#, None);
/// // Will prefer single quotes since there are fewer single quotes (1) than double (2)
/// assert_eq!(quote, '\'');
/// ```
pub fn encode_for_css(content: &str, prefer_quote: Option<char>) -> (String, char) {
    // Determine which quote character to use
    let quote = if let Some(q) = prefer_quote {
        q
    } else {
        // Count quotes to decide which is better
        let single_count = content.chars().filter(|&c| c == '\'').count();
        let double_count = content.chars().filter(|&c| c == '"').count();

        if single_count < double_count {
            '\''
        } else {
            '"' // Tie-breaker: prefer double quotes
        }
    };

    let mut result = String::with_capacity(content.len() + 2);
    result.push(quote);

    for ch in content.chars() {
        match ch {
            '\\' => result.push_str(r"\\"),
            '\n' => result.push_str(r"\n"),
            '\r' => result.push_str(r"\r"),
            '\t' => result.push_str(r"\t"),
            c if c == quote => {
                result.push('\\');
                result.push(quote);
            }
            c => result.push(c),
        }
    }

    result.push(quote);
    (result, quote)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_svelte_quirks_no_escapes() {
        assert_eq!(apply_svelte_quirks("normal text"), "normal text");
        assert_eq!(apply_svelte_quirks("'hello world'"), "'hello world'");
    }

    #[test]
    fn test_apply_svelte_quirks_backslash_doubling() {
        // Backslash + space: \ → \\  (backslash doubled + space duplicated)
        assert_eq!(apply_svelte_quirks(r"test \ slash"), r"test \\  slash");

        // Multiple backslash-space sequences
        assert_eq!(
            apply_svelte_quirks(r"test \ and \ more"),
            r"test \\  and \\  more"
        );
    }

    #[test]
    fn test_apply_svelte_quirks_unicode_escapes() {
        // Unicode with 2 hex digits: \41 → \\441 (backslash doubled + first digit `4` duplicated)
        assert_eq!(apply_svelte_quirks(r"\41"), r"\\441");

        // Unicode with 8 hex chars: \0001F4A9 → \\00001F4A9 (backslash doubled + first `0` duplicated)
        assert_eq!(apply_svelte_quirks(r"'\0001F4A9'"), r"'\\00001F4A9'");

        // Unicode with 6 hex digits: \0000FF → \\00000FF (backslash doubled + first `0` duplicated)
        assert_eq!(apply_svelte_quirks(r"\0000FF"), r"\\00000FF");
    }

    #[test]
    fn test_apply_svelte_quirks_non_hex_escapes() {
        // Escaped quote: \' → \\'' (backslash doubled + quote duplicated)
        assert_eq!(apply_svelte_quirks(r"\'"), r"\\''");

        // Escaped newline: \n → \\nn (backslash doubled + `n` duplicated)
        assert_eq!(apply_svelte_quirks(r"\n"), r"\\nn");

        // Mixed: has \'both\' → has \\''both\\''
        assert_eq!(apply_svelte_quirks(r"has \'both\'"), r"has \\''both\\''");
    }

    #[test]
    fn test_apply_svelte_quirks_mixed() {
        // Unicode escape + backslash-space
        assert_eq!(
            apply_svelte_quirks(r"\41 and \ slash"),
            r"\\441 and \\  slash"
        );
    }

    #[test]
    fn test_decode_escape_sequences_simple() {
        assert_eq!(decode_escape_sequences(r"\\"), r"\");
        assert_eq!(decode_escape_sequences(r#"\""#), r#"""#);
        assert_eq!(decode_escape_sequences(r"\'"), r"'");
    }

    #[test]
    fn test_decode_escape_sequences_unicode() {
        // Simple ASCII letter (2 hex digits)
        assert_eq!(decode_escape_sequences(r"\41"), "A");

        // Emoji (5 hex digits)
        assert_eq!(decode_escape_sequences(r"\1F4A9"), "💩");

        // With ONE leading zero (6 hex digits - CSS maximum)
        assert_eq!(decode_escape_sequences(r"\01F4A9"), "💩");

        // Note: \0001F4A9 has 8 hex digits, exceeding CSS spec limit of 6.
        // It would be interpreted as \00001F (6 digits = U+1F4 = 'Ǵ') + "4A9" (regular text)
    }

    #[test]
    fn test_encode_for_css_basic() {
        let (encoded, quote) = encode_for_css("hello", Some('"'));
        assert_eq!(encoded, r#""hello""#);
        assert_eq!(quote, '"');

        let (encoded, quote) = encode_for_css("hello", Some('\''));
        assert_eq!(encoded, r"'hello'");
        assert_eq!(quote, '\'');
    }

    #[test]
    fn test_encode_for_css_with_escapes() {
        let (encoded, _) = encode_for_css(r"test \ slash", Some('"'));
        assert_eq!(encoded, r#""test \\ slash""#);

        let (encoded, _) = encode_for_css("line\nbreak", Some('"'));
        assert_eq!(encoded, r#""line\nbreak""#);
    }

    #[test]
    fn test_encode_for_css_auto_quote_selection() {
        // More double quotes → use single quotes
        let (_, quote) = encode_for_css(r#"has "double" and "more quotes""#, None);
        assert_eq!(quote, '\'');

        // More single quotes → use double quotes
        let (_, quote) = encode_for_css(r"has single 'quotes'", None);
        assert_eq!(quote, '"');

        // Equal or no quotes → prefer double quotes
        let (_, quote) = encode_for_css("no quotes here", None);
        assert_eq!(quote, '"');
    }
}
