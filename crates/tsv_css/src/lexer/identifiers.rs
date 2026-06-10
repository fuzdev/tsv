use super::token::{Token, TokenKind};
use tsv_lang::ParseError;

/// Whether `ch` is a non-ASCII CSS identifier code point.
///
/// Mirrors Svelte's `parseCss` (`read_identifier`: `codePointAt(0) >= 160`): any
/// code point at or above U+00A0 is a valid identifier code point, so symbols and
/// emoji (`♥`, `💩`) are accepted while the C1 control range U+0080–U+009F stays
/// excluded. Single source for the threshold shared by `is_identifier_start` and
/// the continuation guard in `read_identifier`.
#[inline]
fn is_non_ascii_identifier_codepoint(ch: char) -> bool {
    ch as u32 >= 0xA0
}

/// Whether `ch` can begin a CSS identifier token in the lexer dispatch.
///
/// Covers ASCII letters, non-ASCII identifier code points (see
/// `is_non_ascii_identifier_codepoint`), `-`, `_`, and the `\` escape introducer.
/// Digits and a leading `$` have their own dispatch arms (numbers, and `$`-prefixed
/// identifiers), so they're intentionally excluded here — but a `$` arm uses this to
/// confirm the *next* char begins an identifier.
pub(crate) fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic()
        || is_non_ascii_identifier_codepoint(ch)
        || ch == '-'
        || ch == '_'
        || ch == '\\'
}

/// Read a CSS identifier
/// CSS identifiers can contain unicode escapes; the characters a-z, A-Z, 0-9, -, _;
/// any non-ASCII code point at or above U+00A0 (symbols and emoji, matching Svelte's
/// `>= 160` rule); plus an optional leading `$` (SCSS-style; the lexer dispatch only
/// routes `$` here when it begins an identifier).
/// Per CSS Syntax Level 3 spec, escape sequences are decoded to their actual characters
pub(crate) fn read_identifier(source: &str, pos: &mut usize) -> Result<Token, ParseError> {
    let start = *pos;
    let mut decoded = String::new();

    // Optional leading `$` (SCSS-style variable / property identifiers). Svelte's
    // `parseCss` treats `$foo` as a single identifier; a bare `$` (e.g. the `$=`
    // attribute selector) is kept as a Dollar token by the lexer dispatch, so this
    // arm is only reached when `$` begins an identifier.
    if source[*pos..].starts_with('$') {
        decoded.push('$');
        *pos += 1;
    }

    // CSS identifiers can contain escape sequences that must be decoded
    loop {
        let current_char = source[*pos..].chars().next();
        match current_char {
            // Continuation char: ASCII alphanumeric, a non-ASCII identifier code point,
            // `-`, or `_` — the same predicate Svelte's `read_identifier` applies per
            // char (`codePointAt(0) >= 160 || [a-zA-Z0-9_-]`).
            Some(ch)
                if ch.is_ascii_alphanumeric()
                    || is_non_ascii_identifier_codepoint(ch)
                    || ch == '-'
                    || ch == '_' =>
            {
                decoded.push(ch);
                *pos += ch.len_utf8();
            }
            Some('\\') => {
                // Check if this is a valid escape sequence
                let Some(next_ch) = source[*pos + 1..].chars().next() else {
                    // Backslash at end of input - end identifier
                    break;
                };

                if next_ch.is_ascii_hexdigit() {
                    // Unicode escape: \XXXXXX (1-6 hex digits)
                    let ch = decode_unicode_escape(source, pos)?;
                    decoded.push(ch);
                } else if next_ch == '\n' || next_ch == '\r' || next_ch == '\x0C' {
                    // Newline after backslash - invalid escape, end identifier
                    break;
                } else {
                    // Single character escape: backslash followed by any character
                    // The character itself is the escaped value
                    *pos += 1; // skip backslash
                    decoded.push(next_ch);
                    *pos += next_ch.len_utf8();
                }
            }
            _ => {
                break;
            }
        }
    }

    Ok(Token {
        kind: TokenKind::Identifier,
        start,
        end: *pos,
        decoded: Some(decoded),
    })
}

/// Decode a CSS unicode escape sequence: \XXXXXX (1-6 hex digits)
/// Advances position past the escape sequence
pub(crate) fn decode_unicode_escape(source: &str, pos: &mut usize) -> Result<char, ParseError> {
    let start = *pos;
    *pos += 1; // skip \

    let mut hex_str = String::new();

    // Read 1-6 hex digits
    for _ in 0..6 {
        match source[*pos..].chars().next() {
            Some(ch) if ch.is_ascii_hexdigit() => {
                hex_str.push(ch);
                *pos += ch.len_utf8();
            }
            _ => break,
        }
    }

    if hex_str.is_empty() {
        return Err(ParseError::InvalidSyntax {
            message: "Invalid unicode escape sequence".to_string(),
            position: start,
            context: None,
        });
    }

    // Skip optional whitespace after unicode escape
    if let Some(ch) = source[*pos..].chars().next()
        && ch.is_whitespace()
    {
        *pos += ch.len_utf8();
    }

    let code_point = u32::from_str_radix(&hex_str, 16).map_err(|_| ParseError::InvalidSyntax {
        message: format!("Invalid unicode code point: {hex_str}"),
        position: start,
        context: None,
    })?;

    char::from_u32(code_point).ok_or_else(|| ParseError::InvalidSyntax {
        message: format!("Invalid unicode code point: U+{code_point:X}"),
        position: start,
        context: None,
    })
}
