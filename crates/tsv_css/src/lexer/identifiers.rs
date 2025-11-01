use super::token::{Token, TokenKind};
use tsv_lang::ParseError;

/// Read a CSS identifier
/// CSS identifiers can contain unicode escapes and the characters a-z, A-Z, 0-9, -, _
pub(crate) fn read_identifier(source: &str, pos: &mut usize) -> Result<Token, ParseError> {
    let start = *pos;

    // CSS identifiers can contain unicode escapes
    loop {
        let current_char = source[*pos..].chars().next();
        match current_char {
            Some(ch) if ch.is_alphanumeric() || ch == '-' || ch == '_' => {
                *pos += ch.len_utf8();
            }
            Some('\\') => {
                // Unicode escape in identifier
                let peek_char = source[*pos + 1..].chars().next();
                if let Some(next_ch) = peek_char
                    && next_ch.is_ascii_hexdigit()
                {
                    // Decode and continue - we're building the source representation
                    let _ = decode_unicode_escape(source, pos)?;
                    continue;
                }
                // Not a unicode escape, end identifier
                break;
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
        message: format!("Invalid unicode code point: {}", hex_str),
        position: start,
        context: None,
    })?;

    char::from_u32(code_point).ok_or_else(|| ParseError::InvalidSyntax {
        message: format!("Invalid unicode code point: U+{:X}", code_point),
        position: start,
        context: None,
    })
}
