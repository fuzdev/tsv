// JS/TypeScript string escape decoding
//
// Implements ECMAScript string escape sequences as specified in:
// https://tc39.es/ecma262/#sec-literals-string-literals
//
// Reference implementation: acorn (node_modules/acorn/dist/acorn.mjs)
// - readString() - lines 5925-5949
// - readEscapedChar() - lines 6052-6112
// - readCodePoint() - lines 5910-5923
// - readHexChar() - lines 6116-6121
//
// Template literal escapes follow similar rules with additional:
// - \` - escaped backtick (template delimiter)
// - \$ - escaped dollar (to prevent interpolation)

use tsv_lang::ParseError;

/// Decode JS/TypeScript string escape sequences
///
/// Converts escape sequences in a string literal to their actual character values.
/// Input should be the string content WITHOUT quotes.
///
/// # Escape types supported:
/// - Simple escapes: `\n`, `\t`, `\r`, `\b`, `\f`, `\v`, `\\`, `\'`, `\"`
/// - Hex escapes: `\xHH` (2 hex digits)
/// - Unicode escapes: `\uXXXX` (4 hex digits)
/// - Codepoint escapes: `\u{XXXXXX}` (1-6 hex digits)
/// - Octal escapes: `\0` (null), legacy octals (`\101` → 'A')
/// - Line continuations: `\<newline>` → empty string
/// - Invalid escapes: `\z` → 'z' (backslash ignored per spec)
///
/// # Examples:
/// ```ignore
/// let mut out = String::new();
/// decode_string_escapes_into("test\\n", &mut out)?;
/// assert_eq!(out, "test\n");
/// decode_string_escapes_into("\\u{1F4A9}", &mut out)?;
/// assert_eq!(out, "💩");
/// ```
pub fn decode_string_escapes_into(s: &str, out: &mut String) -> Result<(), ParseError> {
    out.clear();
    out.reserve(s.len());
    let result = out;
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // Handle escape sequence
            match chars.next() {
                // Simple single-character escapes
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('b') => result.push('\u{0008}'), // backspace
                Some('f') => result.push('\u{000C}'), // form feed
                Some('v') => result.push('\u{000B}'), // vertical tab
                Some('\\') => result.push('\\'),
                Some('\'') => result.push('\''),
                Some('"') => result.push('"'),

                // Null character
                Some('0') if !matches!(chars.peek(), Some('0'..='9')) => {
                    result.push('\0');
                }

                // Hex escape: \xHH
                Some('x') => {
                    // 2 hex digits → 0..=0xFF, always a valid Unicode scalar (no
                    // surrogate range), so `from_u32` never fails here.
                    let code = read_hex_value(&mut chars, 2)?;
                    if let Some(ch) = char::from_u32(code) {
                        result.push(ch);
                    } else {
                        return Err(ParseError::InvalidSyntax {
                            message: format!("Invalid hex escape: \\x{code:02X}"),
                            position: 0,
                            context: None,
                        });
                    }
                }

                // Unicode escape: \uXXXX or \u{XXXXXX}
                Some('u') => {
                    if chars.peek() == Some(&'{') {
                        // Codepoint escape: \u{X...XXXXXX}
                        chars.next(); // consume '{'
                        let mut code: u32 = 0;
                        let mut digits: usize = 0;
                        loop {
                            match chars.peek() {
                                Some(&'}') => {
                                    chars.next(); // consume '}'
                                    break;
                                }
                                Some(&ch) => match ch.to_digit(16) {
                                    Some(d) => {
                                        chars.next();
                                        // Accumulate the first 6 digits only; a 7th
                                        // trips the length check below, so overlong
                                        // input can't overflow `code`.
                                        if digits < 6 {
                                            code = code * 16 + d;
                                        }
                                        digits += 1;
                                    }
                                    None => {
                                        return Err(ParseError::InvalidSyntax {
                                            message: "Invalid unicode codepoint escape".to_string(),
                                            position: 0,
                                            context: None,
                                        });
                                    }
                                },
                                // End of input before the closing `}` — a `\u{…`
                                // escape must be terminated (matches acorn).
                                None => {
                                    return Err(ParseError::InvalidSyntax {
                                        message: "Unterminated unicode codepoint escape"
                                            .to_string(),
                                        position: 0,
                                        context: None,
                                    });
                                }
                            }
                        }

                        if digits == 0 || digits > 6 {
                            return Err(ParseError::InvalidSyntax {
                                message: "Invalid unicode codepoint escape length".to_string(),
                                position: 0,
                                context: None,
                            });
                        }

                        if let Some(ch) = char::from_u32(code) {
                            result.push(ch);
                        } else {
                            return Err(ParseError::InvalidSyntax {
                                message: format!("Invalid unicode codepoint: U+{code:X}"),
                                position: 0,
                                context: None,
                            });
                        }
                    } else {
                        // Standard unicode escape: \uXXXX (4 digits → 0..=0xFFFF)
                        let code = read_hex_value(&mut chars, 4)?;
                        // Handle surrogate pairs
                        if (0xD800..=0xDBFF).contains(&code) {
                            // High surrogate - check for low surrogate
                            if chars.peek() == Some(&'\\') {
                                let saved_pos = chars.clone();
                                chars.next(); // consume '\\'
                                if chars.peek() == Some(&'u') {
                                    chars.next(); // consume 'u'
                                    if let Ok(low) = read_hex_value(&mut chars, 4)
                                        && (0xDC00..=0xDFFF).contains(&low)
                                    {
                                        // Valid surrogate pair
                                        let codepoint =
                                            0x10000 + (code - 0xD800) * 0x400 + (low - 0xDC00);
                                        if let Some(ch) = char::from_u32(codepoint) {
                                            result.push(ch);
                                            continue;
                                        }
                                    }
                                }
                                // Not a valid surrogate pair, restore position
                                chars = saved_pos;
                            }
                        }
                        // Single UTF-16 code unit
                        if let Some(ch) = char::from_u32(code) {
                            result.push(ch);
                        } else {
                            result.push(char::REPLACEMENT_CHARACTER);
                        }
                    }
                }

                // Line continuation: backslash followed by a line terminator
                // (LF, CR, CRLF, U+2028, U+2029) — consumed, contributes nothing
                Some('\n' | '\u{2028}' | '\u{2029}') => {}
                Some('\r') => {
                    // Line continuation - consume \r and optional \n
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                }

                // Octal escapes (legacy, strict mode errors on non-zero octals)
                // For now, we support them for compatibility
                Some(ch @ '0'..='7') => {
                    let mut code = ch as u32 - '0' as u32;
                    // Read up to 2 more octal digits
                    for _ in 0..2 {
                        match chars.peek() {
                            Some(&next_ch @ '0'..='7') => {
                                chars.next();
                                code = code * 8 + (next_ch as u32 - '0' as u32);
                            }
                            _ => break,
                        }
                    }
                    // 1–3 octal digits → 0..=0o777 (511), always a valid scalar.
                    if let Some(ch) = char::from_u32(code) {
                        result.push(ch);
                    }
                }

                // Invalid escape - per spec, backslash is ignored (e.g., \z → z)
                Some(ch) => {
                    result.push(ch);
                }

                // End of string after backslash (shouldn't happen in valid input)
                None => {
                    result.push('\\');
                }
            }
        } else {
            // Regular character
            result.push(ch);
        }
    }

    Ok(())
}

/// Decode escapes into a freshly allocated `String` — the ergonomic wrapper over
/// [`decode_string_escapes_into`] for tests and the cold template-error path.
/// The hot lexer path calls `_into` directly against a parked scratch buffer.
pub fn decode_string_escapes(s: &str) -> Result<String, ParseError> {
    let mut out = String::new();
    decode_string_escapes_into(s, &mut out)?;
    Ok(out)
}

/// Read exactly N hex digits from the iterator, accumulating their value directly
/// into a `u32` — no intermediate `String` + `from_str_radix` allocation. N is at
/// most 4 (the `\xHH` / `\uXXXX` escapes), so the value never overflows.
fn read_hex_value<I>(chars: &mut std::iter::Peekable<I>, count: usize) -> Result<u32, ParseError>
where
    I: Iterator<Item = char>,
{
    let mut value: u32 = 0;
    for _ in 0..count {
        match chars.next() {
            Some(ch) => match ch.to_digit(16) {
                Some(d) => value = value * 16 + d,
                None => {
                    return Err(ParseError::InvalidSyntax {
                        message: format!("Expected hex digit, found '{ch}'"),
                        position: 0,
                        context: None,
                    });
                }
            },
            None => {
                return Err(ParseError::InvalidSyntax {
                    message: "Unexpected end of string in escape sequence".to_string(),
                    position: 0,
                    context: None,
                });
            }
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_escapes() {
        assert_eq!(decode_string_escapes("test\\n").unwrap(), "test\n");
        assert_eq!(decode_string_escapes("test\\t\\r").unwrap(), "test\t\r");
        assert_eq!(
            decode_string_escapes("test\\\\slash").unwrap(),
            "test\\slash"
        );
        assert_eq!(decode_string_escapes("test\\'quote").unwrap(), "test'quote");
        assert_eq!(
            decode_string_escapes("test\\\"quote").unwrap(),
            "test\"quote"
        );
    }

    #[test]
    fn test_null() {
        assert_eq!(decode_string_escapes("\\0").unwrap(), "\0");
        assert_eq!(decode_string_escapes("test\\0end").unwrap(), "test\0end");
    }

    #[test]
    fn test_hex_escapes() {
        assert_eq!(decode_string_escapes("\\x41\\x42").unwrap(), "AB");
        assert_eq!(
            decode_string_escapes("test\\x20space").unwrap(),
            "test space"
        );
    }

    #[test]
    fn test_unicode_4digit() {
        assert_eq!(decode_string_escapes("\\u0041").unwrap(), "A");
        assert_eq!(decode_string_escapes("\\u00A9").unwrap(), "©");
    }

    #[test]
    fn test_unicode_codepoint() {
        assert_eq!(decode_string_escapes("\\u{41}").unwrap(), "A");
        assert_eq!(decode_string_escapes("\\u{1F4A9}").unwrap(), "💩");
    }

    #[test]
    fn test_surrogate_pairs() {
        // 💩 = U+1F4A9 = surrogate pair \uD83D\uDCA9
        assert_eq!(decode_string_escapes("\\uD83D\\uDCA9").unwrap(), "💩");
    }

    #[test]
    fn test_invalid_escape() {
        // Invalid escapes: backslash is ignored
        assert_eq!(decode_string_escapes("\\z").unwrap(), "z");
        assert_eq!(decode_string_escapes("test\\qend").unwrap(), "testqend");
    }

    #[test]
    fn test_line_continuation() {
        assert_eq!(decode_string_escapes("test\\\nline").unwrap(), "testline");
    }

    #[test]
    fn test_legacy_octal_escapes() {
        // Legacy octal escapes decode to their code point.
        assert_eq!(decode_string_escapes("\\101").unwrap(), "A"); // 0o101 = 65
        assert_eq!(decode_string_escapes("\\141").unwrap(), "a"); // 0o141 = 97
        // `\0` followed by a digit is an octal escape, not the null shortcut.
        assert_eq!(decode_string_escapes("\\012").unwrap(), "\n"); // 0o12 = 10
    }

    #[test]
    fn test_crlf_and_unicode_line_continuations() {
        // `\` + CRLF consumes both and contributes nothing.
        assert_eq!(decode_string_escapes("a\\\r\nb").unwrap(), "ab");
        // `\` + U+2028 / U+2029 are line continuations too.
        assert_eq!(decode_string_escapes("a\\\u{2028}b").unwrap(), "ab");
        assert_eq!(decode_string_escapes("a\\\u{2029}b").unwrap(), "ab");
    }

    #[test]
    fn test_lone_high_surrogate_falls_back_to_replacement() {
        // A high surrogate with no following low surrogate becomes U+FFFD.
        assert_eq!(decode_string_escapes("\\uD83D").unwrap(), "\u{FFFD}");
        // ...followed by a plain char: replacement, then the char.
        assert_eq!(decode_string_escapes("\\uD83Dx").unwrap(), "\u{FFFD}x");
        // ...followed by a `\u` that is NOT a low surrogate: the lookahead is
        // restored and the second escape decodes on its own.
        assert_eq!(
            decode_string_escapes("\\uD83D\\u0041").unwrap(),
            "\u{FFFD}A"
        );
    }

    #[test]
    fn test_codepoint_escape_length_and_range_errors() {
        assert!(decode_string_escapes("\\u{}").is_err()); // empty
        assert!(decode_string_escapes("\\u{1234567}").is_err()); // > 6 digits
        assert!(decode_string_escapes("\\u{110000}").is_err()); // > U+10FFFF
    }
}
