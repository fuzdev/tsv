use super::token::{Token, TokenKind};
use tsv_lang::ParseError;

/// Read a CSS identifier
/// CSS identifiers can contain unicode escapes and the characters a-z, A-Z, 0-9, -, _
/// Per CSS Syntax Level 3 spec, escape sequences are decoded to their actual characters
pub(crate) fn read_identifier(source: &str, pos: &mut usize) -> Result<Token, ParseError> {
    let start = *pos;
    let mut decoded = String::new();

    // CSS identifiers can contain escape sequences that must be decoded
    loop {
        let current_char = source[*pos..].chars().next();
        match current_char {
            Some(ch) if ch.is_alphanumeric() || ch == '-' || ch == '_' => {
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
