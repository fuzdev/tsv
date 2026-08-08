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
/// - Codepoint escapes: `\u{X...}` (any number of hex digits, value ≤ U+10FFFF —
///   the spec caps the VALUE, so leading zeros are unbounded)
/// - Surrogate pairs: a lead escape followed by a trail escape joins into one code
///   point, in either spelling and in any combination (`\uD83D\uDE00`,
///   `\u{D83D}\u{DE00}`, and the two mixed forms all decode to `😀`). An UNPAIRED
///   half becomes U+FFFD — a UTF-8 `String` cannot hold it
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
                        return Err(ParseError::invalid_syntax(
                            format!("Invalid hex escape: \\x{code:02X}"),
                            0,
                        ));
                    }
                }

                // Unicode escape: \uXXXX or \u{XXXXXX}
                Some('u') => {
                    let code = read_unicode_escape_value(&mut chars)?;

                    // A LEAD surrogate joins a following TRAIL surrogate into one
                    // code point. The pairing is a property of the code UNITS, not
                    // of how they were spelled — `\uD83D` and `\u{D83D}` denote the
                    // same unit — so all four lead/trail spelling combinations pair,
                    // and only a genuinely unpaired half falls through below.
                    if (0xD800..=0xDBFF).contains(&code)
                        && let Some(trail) = take_trail_surrogate(&mut chars)
                    {
                        let paired = 0x10000 + (code - 0xD800) * 0x400 + (trail - 0xDC00);
                        // A paired value is 0x10000..=0x10FFFF by construction, so
                        // it is always a scalar value.
                        debug_assert!(char::from_u32(paired).is_some());
                        result.push(char::from_u32(paired).unwrap_or(char::REPLACEMENT_CHARACTER));
                    } else {
                        // `code` is ≤ 0x10FFFF, so the only value `char::from_u32`
                        // refuses is an unpaired surrogate (U+D800..=U+DFFF) — well-formed
                        // grammar that only the well-formed-unicode proposals reject, and
                        // that a UTF-8 Rust `String` cannot represent. Substitute U+FFFD;
                        // `raw` is a source slice, so printed output is unaffected.
                        result.push(char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER));
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
/// Read the body of a `\u` escape — the caller has consumed the `\` and the `u` —
/// in either spelling, returning the code point it denotes (always ≤ 0x10FFFF).
///
/// One reader for both spellings is what lets the surrogate-pairing step above stay
/// spelling-agnostic; splitting it was how `'\u{D83D}\u{DE00}'` came to decode as two
/// U+FFFDs where the 4-digit spelling of the same pair gave the astral character.
fn read_unicode_escape_value<I>(chars: &mut std::iter::Peekable<I>) -> Result<u32, ParseError>
where
    I: Iterator<Item = char>,
{
    if chars.peek() != Some(&'{') {
        // Standard unicode escape: \uXXXX (4 digits → 0..=0xFFFF)
        return read_hex_value(chars, 4);
    }

    // Codepoint escape: \u{X...}
    chars.next(); // consume '{'

    // `CodePoint :: HexDigits but only if MV of HexDigits ≤ 0x10FFFF` caps the
    // VALUE, not the digit count — leading zeros are unbounded, so
    // `\u{0000000000000041}` is a valid `A`. Accumulate in `u64` and stop once past
    // the cap: an arbitrarily long escape then can't overflow, while the check below
    // still sees a value greater than 0x10FFFF (and reports it unchanged for the
    // ordinary `\u{110000}` spelling).
    let mut code: u64 = 0;
    let mut any_digit = false;
    loop {
        match chars.peek() {
            Some(&'}') => {
                chars.next(); // consume '}'
                break;
            }
            Some(&ch) => match ch.to_digit(16) {
                Some(d) => {
                    chars.next();
                    if code <= 0x10FFFF {
                        code = code * 16 + u64::from(d);
                    }
                    any_digit = true;
                }
                None => {
                    return Err(ParseError::invalid_syntax(
                        "Invalid unicode codepoint escape".to_string(),
                        0,
                    ));
                }
            },
            // End of input before the closing `}` — a `\u{…` escape must be
            // terminated (matches acorn).
            None => {
                return Err(ParseError::invalid_syntax(
                    "Unterminated unicode codepoint escape".to_string(),
                    0,
                ));
            }
        }
    }

    if !any_digit {
        return Err(ParseError::invalid_syntax(
            "Invalid unicode codepoint escape length".to_string(),
            0,
        ));
    }

    if code > 0x10FFFF {
        return Err(ParseError::invalid_syntax(
            format!("Invalid unicode codepoint: U+{code:X}"),
            0,
        ));
    }

    Ok(code as u32)
}

/// Consume a following `\u` escape when — and only when — it denotes a TRAIL
/// surrogate (U+DC00..=U+DFFF), for joining onto a lead the caller just read.
///
/// Anything else leaves `chars` exactly where it was, so the main decode loop reaches
/// that `\` itself: a *malformed* escape therefore still reports its own error from
/// the ordinary path rather than being swallowed here.
fn take_trail_surrogate<I>(chars: &mut std::iter::Peekable<I>) -> Option<u32>
where
    I: Iterator<Item = char> + Clone,
{
    if chars.peek() != Some(&'\\') {
        return None;
    }
    let saved = chars.clone();
    chars.next(); // consume '\\'
    if chars.peek() == Some(&'u') {
        chars.next(); // consume 'u'
        if let Ok(trail) = read_unicode_escape_value(chars)
            && (0xDC00..=0xDFFF).contains(&trail)
        {
            return Some(trail);
        }
    }
    *chars = saved;
    None
}

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
                    return Err(ParseError::invalid_syntax(
                        format!("Expected hex digit, found '{ch}'"),
                        0,
                    ));
                }
            },
            None => {
                return Err(ParseError::invalid_syntax(
                    "Unexpected end of string in escape sequence".to_string(),
                    0,
                ));
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
        // `\u{1234567}` is rejected on its VALUE (0x1234567 > U+10FFFF), not on its
        // digit count — `CodePoint :: HexDigits but only if MV of HexDigits ≤ 0x10FFFF`
        // caps the value alone. See `test_codepoint_escape_leading_zeros_unbounded`.
        assert!(decode_string_escapes("\\u{1234567}").is_err());
        assert!(decode_string_escapes("\\u{110000}").is_err()); // > U+10FFFF
    }

    #[test]
    fn test_codepoint_escape_leading_zeros_unbounded() {
        // Leading zeros carry no value, so an arbitrarily long escape is valid.
        assert_eq!(decode_string_escapes("\\u{0041}").unwrap(), "A");
        assert_eq!(decode_string_escapes("\\u{0000000000000041}").unwrap(), "A");
        // Padding a value that is itself in range never pushes it out of range.
        assert_eq!(
            decode_string_escapes("\\u{00000010FFFF}").unwrap(),
            "\u{10FFFF}"
        );
        // Accumulation stops once past the cap, so an absurd escape can't overflow
        // the `u64` — it still reports as out of range.
        let long = format!("\\u{{{}}}", "f".repeat(500));
        assert!(decode_string_escapes(&long).is_err());
    }

    #[test]
    fn test_surrogate_pair_joins_in_every_spelling() {
        // The pairing is per code UNIT, so the spelling of each half is irrelevant
        // and all four combinations denote the same character.
        for pair in [
            "\\uD83D\\uDE00",
            "\\u{D83D}\\u{DE00}",
            "\\uD83D\\u{DE00}",
            "\\u{D83D}\\uDE00",
        ] {
            assert_eq!(decode_string_escapes(pair).unwrap(), "\u{1F600}", "{pair}");
        }
        // Leading zeros don't disturb the halves either.
        assert_eq!(
            decode_string_escapes("\\u{0000D83D}\\u{0DE00}").unwrap(),
            "\u{1F600}"
        );
    }

    #[test]
    fn test_unpaired_surrogate_halves_do_not_join() {
        // Two leads, or a trail before a lead, are not a pair in any spelling.
        assert_eq!(
            decode_string_escapes("\\u{D83D}\\u{D83D}").unwrap(),
            "\u{FFFD}\u{FFFD}"
        );
        assert_eq!(
            decode_string_escapes("\\u{DE00}\\u{D83D}").unwrap(),
            "\u{FFFD}\u{FFFD}"
        );
        // A lead followed by an ordinary escape keeps both: the lookahead restores
        // the iterator, so the second escape decodes through the normal path.
        assert_eq!(
            decode_string_escapes("\\uD83D\\u{41}").unwrap(),
            "\u{FFFD}A"
        );
        assert_eq!(
            decode_string_escapes("\\u{D83D}\\u0041").unwrap(),
            "\u{FFFD}A"
        );
        assert_eq!(decode_string_escapes("\\u{D83D}x").unwrap(), "\u{FFFD}x");
    }

    #[test]
    fn test_lead_surrogate_followed_by_malformed_escape_still_errors() {
        // The trail lookahead must not SWALLOW a malformed escape: it restores the
        // iterator, so the main loop reaches that `\` and reports it as it always did.
        assert!(decode_string_escapes("\\u{D83D}\\u{110000}").is_err());
        assert!(decode_string_escapes("\\uD83D\\u{}").is_err());
        assert!(decode_string_escapes("\\uD83D\\uZZZZ").is_err());
    }

    #[test]
    fn test_braced_lone_surrogate_falls_back_to_replacement() {
        // The braced spelling agrees with the 4-digit one: a lone surrogate is
        // well-formed grammar that Rust's UTF-8 `char` cannot represent.
        assert_eq!(decode_string_escapes("\\u{D800}").unwrap(), "\u{FFFD}");
        assert_eq!(decode_string_escapes("\\u{DFFF}").unwrap(), "\u{FFFD}");
        assert_eq!(decode_string_escapes("\\u{0000D800}").unwrap(), "\u{FFFD}");
        // A code point just outside the surrogate range is unaffected.
        assert_eq!(decode_string_escapes("\\u{E000}").unwrap(), "\u{E000}");
    }
}
