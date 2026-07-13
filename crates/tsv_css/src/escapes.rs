//! CSS escape sequence decoding.

use std::borrow::Cow;

/// Decode CSS escape sequences in a string.
///
/// Converts CSS escape sequences to their actual character values:
/// - `\\` → `\` (escaped backslash)
/// - `\"` → `"` (escaped quote)
/// - `\'` → `'` (escaped quote)
/// - `\n` → newline (escaped newline)
/// - `\XXXXXX` → Unicode character (1-6 hex digits)
///
/// Returns a borrowed `Cow` for the common escape-free string (no allocation);
/// only an input that actually contains `\` is decoded into an owned `String`.
pub fn decode_escape_sequences(source: &str) -> Cow<'_, str> {
    if !source.contains('\\') {
        return Cow::Borrowed(source);
    }

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
                                hex_digits.push(digit);
                                chars.next();
                            }
                            _ => break,
                        }
                    }
                    // Optional whitespace terminator
                    if let Some(&ws) = chars.peek()
                        && (ws == ' ' || ws == '\t' || ws == '\n')
                    {
                        chars.next();
                    }
                    if let Ok(code_point) = u32::from_str_radix(&hex_digits, 16)
                        && let Some(c) = char::from_u32(code_point)
                    {
                        result.push(c);
                    }
                } else {
                    // Simple escape: \X → X
                    result.push(next_ch);
                    chars.next();
                }
            } else {
                // Trailing backslash
                result.push('\\');
            }
        } else {
            result.push(ch);
        }
    }

    Cow::Owned(result)
}

/// Trim trailing whitespace, but never the whitespace a CSS **escape** owns.
///
/// `\` followed by whitespace is a valid escape whose payload *is* that whitespace
/// character (CSS Syntax 3 §4.3.4 "Check if two code points are a valid escape" —
/// only a newline after `\` invalidates it — and §4.3.7 "Consume an escaped code
/// point", whose final branch returns the code point itself). So the trailing
/// space in `width: 50px\ ;` is part of the value, not padding.
///
/// Trimming it strands the backslash, which then escapes whatever follows — the
/// declaration's `;`, a function's `)`, an `!important` — and the result no longer
/// parses. Exactly one whitespace character is therefore kept when an **odd**-length
/// backslash run precedes it (an even run is a completed `\\`, so the whitespace
/// after it is ordinary padding); any further whitespace past the escaped one is
/// ordinary trailing whitespace and still goes.
///
/// A *hex* escape's whitespace terminator (`\41 `) needs no such care: what follows
/// a trimmed value is always a delimiter (`;` / `)` / `!`), never a hex digit, so
/// the escape reads the same either way.
pub(crate) fn trim_end_preserving_escape(s: &str) -> &str {
    let trimmed = s.trim_end();
    if trimmed.len() == s.len() {
        return s;
    }
    let backslashes = trimmed.bytes().rev().take_while(|&b| b == b'\\').count();
    if backslashes % 2 == 0 {
        return trimmed;
    }
    // Give the escape its one payload character back.
    let escaped_len = s[trimmed.len()..].chars().next().map_or(0, char::len_utf8);
    &s[..trimmed.len() + escaped_len]
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
