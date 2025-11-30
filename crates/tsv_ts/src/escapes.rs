//! TypeScript/JavaScript escape sequence handling utilities
//!
//! This module provides escape handling for JavaScript/TypeScript string literals,
//! implementing the full ES6+ escape sequence specification.
//!
//! # Escape Sequence Types
//!
//! JavaScript supports multiple escape sequence formats:
//!
//! 1. **Simple single-character escapes**:
//!    - `\n` → newline (U+000A)
//!    - `\t` → tab (U+0009)
//!    - `\r` → carriage return (U+000D)
//!    - `\b` → backspace (U+0008)
//!    - `\f` → form feed (U+000C)
//!    - `\v` → vertical tab (U+000B)
//!    - `\\` → backslash (U+005C)
//!    - `\"` → double quote (U+0022)
//!    - `\'` → single quote (U+0027)
//!
//! 2. **Null byte escape**: `\0` (only when NOT followed by digit)
//!
//! 3. **Hex escapes**: `\xXX` (exactly 2 hex digits, range 0x00-0xFF)
//!
//! 4. **Unicode 4-digit escapes**: `\uXXXX` (exactly 4 hex digits)
//!
//! 5. **Unicode code point escapes**: `\u{X...}` (1-6 hex digits in braces, range 0x0-0x10FFFF)
//!
//! 6. **Line continuation**: `\<newline>` (backslash followed by newline removes the newline)
//!
//! 7. **Invalid escapes**: Unknown escapes like `\q` keep the character (`q`)
//!
//! # Strict Mode Restrictions
//!
//! - **Octal escapes** (`\0` followed by digits, `\1-\7`, `\01`, etc.) are **NOT ALLOWED**
//!   in strict mode (TypeScript/ES6 modules are always strict)
//! - Exception: `\0` alone (not followed by digit) is allowed as null byte
//!
//! # Error Handling
//!
//! The following sequences cause parse errors:
//! - Incomplete unicode: `\u12` (not exactly 4 hex digits)
//! - Out-of-bounds code point: `\u{110000}` (exceeds 0x10FFFF)
//! - Octal escapes in strict mode: `\01`, `\012`, etc.
//!
//! # Examples
//!
//! ```
//! use tsv_ts::escapes::decode_string_escape;
//!
//! // Simple escapes
//! assert_eq!(decode_string_escape(r"hello\nworld", 0), Ok("hello\nworld".to_string()));
//! assert_eq!(decode_string_escape(r"tab\there", 0), Ok("tab\there".to_string()));
//!
//! // Unicode escapes
//! assert_eq!(decode_string_escape(r"\u0041", 0), Ok("A".to_string()));  // 4-digit
//! assert_eq!(decode_string_escape(r"\u{1F4A9}", 0), Ok("💩".to_string()));  // code point
//! assert_eq!(decode_string_escape(r"\x41", 0), Ok("A".to_string()));  // hex
//!
//! // Invalid sequences
//! assert!(decode_string_escape(r"\u12", 0).is_err());  // Incomplete
//! assert!(decode_string_escape(r"\01", 0).is_err());  // Octal in strict mode
//! ```

use tsv_lang::ParseError;

/// Decode JavaScript/TypeScript escape sequences in a string.
///
/// Converts JS escape sequences to their actual character values.
/// This function implements the full ECMAScript escape sequence specification.
///
/// # Arguments
///
/// * `source` - The string content to decode (WITHOUT surrounding quotes)
/// * `position` - Starting position in source file (for error reporting)
///
/// # Returns
///
/// - `Ok(String)` - Decoded string with escape sequences processed
/// - `Err(ParseError)` - Parse error for invalid escape sequences
///
/// # Examples
///
/// ```
/// use tsv_ts::escapes::decode_string_escape;
///
/// // Simple escapes
/// assert_eq!(
///     decode_string_escape(r"hello\nworld", 0).unwrap(),
///     "hello\nworld"
/// );
///
/// // Unicode escapes
/// assert_eq!(
///     decode_string_escape(r"\u0041BC", 0).unwrap(),
///     "ABC"
/// );
///
/// // Hex escapes
/// assert_eq!(
///     decode_string_escape(r"\x41\x42\x43", 0).unwrap(),
///     "ABC"
/// );
///
/// // Invalid escape (unknown) - keeps the character
/// assert_eq!(
///     decode_string_escape(r"\q", 0).unwrap(),
///     "q"
/// );
///
/// // Invalid escape (incomplete unicode) - error
/// assert!(decode_string_escape(r"\u12", 0).is_err());
/// ```
pub fn decode_string_escape(source: &str, position: usize) -> Result<String, ParseError> {
    let mut result = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut offset = 0;

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(&next_ch) = chars.peek() {
                match next_ch {
                    // Simple single-character escapes
                    'n' => {
                        chars.next();
                        result.push('\n');
                        offset += 2;
                    }
                    't' => {
                        chars.next();
                        result.push('\t');
                        offset += 2;
                    }
                    'r' => {
                        chars.next();
                        result.push('\r');
                        offset += 2;
                    }
                    'b' => {
                        chars.next();
                        result.push('\u{0008}'); // backspace
                        offset += 2;
                    }
                    'f' => {
                        chars.next();
                        result.push('\u{000C}'); // form feed
                        offset += 2;
                    }
                    'v' => {
                        chars.next();
                        result.push('\u{000B}'); // vertical tab
                        offset += 2;
                    }
                    '\\' | '"' | '\'' => {
                        chars.next();
                        result.push(next_ch);
                        offset += 2;
                    }

                    // Null byte: \0 (only when NOT followed by digit)
                    '0' => {
                        chars.next();
                        offset += 2;

                        // Check if followed by digit (would be octal - not allowed)
                        if let Some(&digit_ch) = chars.peek()
                            && digit_ch.is_ascii_digit()
                        {
                            return Err(ParseError::InvalidSyntax {
                                message: format!(
                                    "Octal escape sequences are not allowed in strict mode (found '\\0{digit_ch}')"
                                ),
                                position: position + offset - 2,
                                context: None,
                            });
                        }

                        result.push('\0');
                    }

                    // Octal escapes: \1-\7, \01-\77, etc. - NOT ALLOWED in strict mode
                    '1'..='7' => {
                        return Err(ParseError::InvalidSyntax {
                            message: format!(
                                "Octal escape sequences are not allowed in strict mode (found '\\{next_ch}')"
                            ),
                            position: position + offset,
                            context: None,
                        });
                    }

                    // Hex escape: \xXX (exactly 2 hex digits)
                    'x' => {
                        chars.next(); // consume 'x'
                        offset += 2;

                        let mut hex_digits = String::new();
                        for _ in 0..2 {
                            match chars.peek() {
                                Some(&digit) if digit.is_ascii_hexdigit() => {
                                    chars.next();
                                    hex_digits.push(digit);
                                    offset += 1;
                                }
                                _ => break,
                            }
                        }

                        if hex_digits.len() != 2 {
                            return Err(ParseError::InvalidSyntax {
                                message: format!(
                                    "Hex escape sequence must have exactly 2 hex digits, found {} (\\x{})",
                                    hex_digits.len(),
                                    hex_digits
                                ),
                                position: position + offset - hex_digits.len() - 2,
                                context: None,
                            });
                        }

                        // Validated exactly 2 hex digits above, so this cannot fail
                        #[allow(clippy::expect_used)]
                        let code = u8::from_str_radix(&hex_digits, 16)
                            .expect("valid hex digits");
                        result.push(code as char);
                    }

                    // Unicode 4-digit escape: \uXXXX (exactly 4 hex digits)
                    'u' => {
                        chars.next(); // consume 'u'
                        offset += 2;

                        // Check for code point syntax: \u{...}
                        if let Some(&'{') = chars.peek() {
                            chars.next(); // consume '{'
                            offset += 1;

                            let mut hex_digits = String::new();
                            let mut found_close = false;

                            // Read up to 6 hex digits
                            for _ in 0..6 {
                                match chars.peek() {
                                    Some(&'}') => {
                                        chars.next();
                                        offset += 1;
                                        found_close = true;
                                        break;
                                    }
                                    Some(&digit) if digit.is_ascii_hexdigit() => {
                                        chars.next();
                                        hex_digits.push(digit);
                                        offset += 1;
                                    }
                                    Some(&digit) => {
                                        return Err(ParseError::InvalidSyntax {
                                            message: format!(
                                                "Invalid character '{digit}' in unicode code point escape"
                                            ),
                                            position: position + offset,
                                            context: None,
                                        });
                                    }
                                    None => break,
                                }
                            }

                            if !found_close {
                                // Check if there's a '}' after the 6 digits
                                if let Some(&'}') = chars.peek() {
                                    chars.next();
                                    offset += 1;
                                    found_close = true;
                                }
                            }

                            if !found_close {
                                return Err(ParseError::InvalidSyntax {
                                    message: format!(
                                        "Unterminated unicode code point escape sequence (\\u{{{hex_digits})"
                                    ),
                                    position: position + offset - hex_digits.len() - 3,
                                    context: None,
                                });
                            }

                            if hex_digits.is_empty() {
                                return Err(ParseError::InvalidSyntax {
                                    message:
                                        "Unicode code point escape sequence cannot be empty (\\u{})"
                                            .to_string(),
                                    position: position + offset - 3,
                                    context: None,
                                });
                            }

                            let code_point =
                                u32::from_str_radix(&hex_digits, 16).map_err(|_| {
                                    ParseError::InvalidSyntax {
                                        message: format!(
                                            "Invalid unicode code point (\\u{{{hex_digits}}})"
                                        ),
                                        position: position + offset - hex_digits.len() - 3,
                                        context: None,
                                    }
                                })?;

                            if code_point > 0x10FFFF {
                                return Err(ParseError::InvalidSyntax {
                                    message: format!(
                                        "Unicode code point 0x{code_point:X} out of bounds (max is 0x10FFFF)"
                                    ),
                                    position: position + offset - hex_digits.len() - 3,
                                    context: None,
                                });
                            }

                            match char::from_u32(code_point) {
                                Some(unicode_char) => result.push(unicode_char),
                                None => {
                                    return Err(ParseError::InvalidSyntax {
                                        message: format!(
                                            "Invalid unicode code point: 0x{code_point:X}"
                                        ),
                                        position: position + offset - hex_digits.len() - 3,
                                        context: None,
                                    });
                                }
                            }
                        } else {
                            // Standard \uXXXX format (exactly 4 hex digits)
                            let mut hex_digits = String::new();
                            for _ in 0..4 {
                                match chars.peek() {
                                    Some(&digit) if digit.is_ascii_hexdigit() => {
                                        chars.next();
                                        hex_digits.push(digit);
                                        offset += 1;
                                    }
                                    _ => break,
                                }
                            }

                            if hex_digits.len() != 4 {
                                return Err(ParseError::InvalidSyntax {
                                    message: format!(
                                        "Unicode escape sequence must have exactly 4 hex digits, found {} (\\u{})",
                                        hex_digits.len(),
                                        hex_digits
                                    ),
                                    position: position + offset - hex_digits.len() - 2,
                                    context: None,
                                });
                            }

                            // Validated exactly 4 hex digits above, so this cannot fail
                            #[allow(clippy::expect_used)]
                            let code_point = u16::from_str_radix(&hex_digits, 16)
                                .expect("valid hex digits");

                            // Handle UTF-16 surrogate pairs
                            // High surrogate: 0xD800-0xDBFF
                            // Low surrogate: 0xDC00-0xDFFF
                            if (0xD800..=0xDBFF).contains(&code_point) {
                                // High surrogate - look ahead for low surrogate
                                let saved_chars = chars.clone();
                                let saved_offset = offset;

                                // Check for \uXXXX pattern
                                if let Some(&'\\') = chars.peek() {
                                    chars.next();
                                    offset += 1;

                                    if let Some(&'u') = chars.peek() {
                                        chars.next();
                                        offset += 1;

                                        // Read 4 hex digits
                                        let mut low_hex = String::new();
                                        for _ in 0..4 {
                                            if let Some(&ch) = chars.peek() {
                                                if ch.is_ascii_hexdigit() {
                                                    low_hex.push(ch);
                                                    chars.next();
                                                    offset += 1;
                                                } else {
                                                    break;
                                                }
                                            } else {
                                                break;
                                            }
                                        }

                                        if low_hex.len() == 4
                                            && let Ok(low_surrogate) =
                                                u16::from_str_radix(&low_hex, 16)
                                            && (0xDC00..=0xDFFF).contains(&low_surrogate)
                                        {
                                            // Valid surrogate pair - combine them
                                            let high = (code_point - 0xD800) as u32;
                                            let low = (low_surrogate - 0xDC00) as u32;
                                            let code_point_combined = (high << 10) + low + 0x10000;

                                            if let Some(ch) = char::from_u32(code_point_combined) {
                                                result.push(ch);
                                            } else {
                                                result.push('\u{FFFD}');
                                            }
                                            continue; // Successfully handled surrogate pair
                                        }
                                    }
                                }

                                // No valid low surrogate found - unpaired high surrogate
                                // Restore position and insert replacement character
                                chars = saved_chars;
                                offset = saved_offset;
                                result.push('\u{FFFD}');
                            } else if (0xDC00..=0xDFFF).contains(&code_point) {
                                // Unpaired low surrogate
                                result.push('\u{FFFD}');
                            } else {
                                // Normal BMP character
                                match char::from_u32(code_point as u32) {
                                    Some(ch) => result.push(ch),
                                    None => result.push('\u{FFFD}'),
                                }
                            }
                        }
                    }

                    // Line continuation: \<newline>
                    '\n' | '\r' => {
                        chars.next(); // consume newline
                        offset += 2;

                        // If it's \r\n, consume the \n too
                        if next_ch == '\r'
                            && let Some(&'\n') = chars.peek()
                        {
                            chars.next();
                            offset += 1;
                        }

                        // Line continuation: the newline is removed, nothing added to result
                    }

                    // Unknown escape: keep the character (e.g., \q → q)
                    _ => {
                        chars.next();
                        result.push(next_ch);
                        offset += 2;
                    }
                }
            } else {
                // Backslash at end of string - keep it
                result.push('\\');
                offset += 1;
            }
        } else {
            result.push(ch);
            offset += ch.len_utf8();
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_escapes() {
        assert_eq!(
            decode_string_escape(r"hello\nworld", 0).unwrap(),
            "hello\nworld"
        );
        assert_eq!(decode_string_escape(r"tab\there", 0).unwrap(), "tab\there");
        assert_eq!(
            decode_string_escape(r#"quote\"test"#, 0).unwrap(),
            "quote\"test"
        );
        assert_eq!(
            decode_string_escape(r"back\\slash", 0).unwrap(),
            "back\\slash"
        );
        assert_eq!(
            decode_string_escape(r"\r\n\t\b\f\v", 0).unwrap(),
            "\r\n\t\u{0008}\u{000C}\u{000B}"
        );
    }

    #[test]
    fn test_null_byte() {
        assert_eq!(decode_string_escape(r"\0", 0).unwrap(), "\0");
        assert_eq!(decode_string_escape(r"test\0end", 0).unwrap(), "test\0end");
    }

    #[test]
    fn test_null_byte_followed_by_digit_errors() {
        assert!(decode_string_escape(r"\01", 0).is_err());
        assert!(decode_string_escape(r"\012", 0).is_err());
    }

    #[test]
    fn test_octal_escapes_forbidden() {
        assert!(decode_string_escape(r"\1", 0).is_err());
        assert!(decode_string_escape(r"\7", 0).is_err());
        assert!(decode_string_escape(r"\77", 0).is_err());
    }

    #[test]
    fn test_hex_escapes() {
        assert_eq!(decode_string_escape(r"\x41", 0).unwrap(), "A");
        assert_eq!(decode_string_escape(r"\x42", 0).unwrap(), "B");
        assert_eq!(decode_string_escape(r"\xFF", 0).unwrap(), "\u{00FF}");
        assert_eq!(decode_string_escape(r"\x00", 0).unwrap(), "\0");
        assert_eq!(decode_string_escape(r"\x41\x42\x43", 0).unwrap(), "ABC");
    }

    #[test]
    fn test_hex_escapes_invalid() {
        assert!(decode_string_escape(r"\x1", 0).is_err()); // Only 1 digit
        assert!(decode_string_escape(r"\xGG", 0).is_err()); // Invalid hex
    }

    #[test]
    fn test_unicode_4digit() {
        assert_eq!(decode_string_escape(r"\u0041", 0).unwrap(), "A");
        assert_eq!(decode_string_escape(r"\u20AC", 0).unwrap(), "€");
        assert_eq!(decode_string_escape(r"\u2603", 0).unwrap(), "☃");
    }

    #[test]
    fn test_unicode_4digit_invalid() {
        assert!(decode_string_escape(r"\u12", 0).is_err()); // Only 2 digits
        assert!(decode_string_escape(r"\uGGGG", 0).is_err()); // Invalid hex
    }

    #[test]
    fn test_unicode_code_point() {
        assert_eq!(decode_string_escape(r"\u{41}", 0).unwrap(), "A");
        assert_eq!(decode_string_escape(r"\u{1F4A9}", 0).unwrap(), "💩");
        assert_eq!(
            decode_string_escape(r"\u{10FFFF}", 0).unwrap(),
            "\u{10FFFF}"
        );
        assert_eq!(decode_string_escape(r"\u{0}", 0).unwrap(), "\0");
    }

    #[test]
    fn test_unicode_code_point_invalid() {
        assert!(decode_string_escape(r"\u{}", 0).is_err()); // Empty
        assert!(decode_string_escape(r"\u{110000}", 0).is_err()); // Out of bounds
        assert!(decode_string_escape(r"\u{GGGG}", 0).is_err()); // Invalid hex
        assert!(decode_string_escape(r"\u{123", 0).is_err()); // Unterminated
    }

    #[test]
    fn test_line_continuation() {
        assert_eq!(
            decode_string_escape("line\\\ncontinued", 0).unwrap(),
            "linecontinued"
        );
        assert_eq!(
            decode_string_escape("line\\\rcontinued", 0).unwrap(),
            "linecontinued"
        );
        assert_eq!(
            decode_string_escape("line\\\r\ncontinued", 0).unwrap(),
            "linecontinued"
        );
    }

    #[test]
    fn test_unknown_escapes() {
        // Unknown escapes keep the character (not the backslash)
        assert_eq!(decode_string_escape(r"\q", 0).unwrap(), "q");
        assert_eq!(decode_string_escape(r"\z", 0).unwrap(), "z");
    }

    #[test]
    fn test_mixed_escapes() {
        assert_eq!(
            decode_string_escape(r"Hello\nWorld\t\u0041\x42C", 0).unwrap(),
            "Hello\nWorld\tABC"
        );
    }
}
