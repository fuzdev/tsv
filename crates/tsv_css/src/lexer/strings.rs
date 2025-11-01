use super::token::{Token, TokenKind};
use tsv_lang::ParseError;

/// Read a CSS string: "..." or '...'
/// Preserves escape sequences in raw form for formatting
/// Returns the raw string content (with escapes) and quote character
pub(crate) fn read_string(source: &str, pos: &mut usize, quote: char) -> Result<Token, ParseError> {
    let start = *pos;
    *pos += 1; // skip opening quote

    let content_start = *pos; // Track start of content (after opening quote)

    // Scan through string to validate and find end
    loop {
        let current_char = source[*pos..].chars().next();
        match current_char {
            None => {
                return Err(ParseError::InvalidSyntax {
                    message: format!("Unterminated string starting with {}", quote),
                    position: start,
                    context: None,
                });
            }
            Some(ch) if ch == quote => {
                let content_end = *pos; // End of content (before closing quote)
                *pos += 1; // skip closing quote

                // Slice source to get raw content with escapes preserved
                let content = source[content_start..content_end].to_string();

                return Ok(Token {
                    kind: TokenKind::String { content, quote },
                    start,
                    end: *pos,
                });
            }
            Some('\\') => {
                // Skip escape sequence without decoding
                *pos += 1; // skip \

                // Check if it's a unicode escape
                if let Some(next_ch) = source[*pos..].chars().next() {
                    if next_ch.is_ascii_hexdigit() {
                        // Skip 1-6 hex digits for unicode escape
                        for _ in 0..6 {
                            match source[*pos..].chars().next() {
                                Some(ch) if ch.is_ascii_hexdigit() => {
                                    *pos += ch.len_utf8();
                                }
                                _ => break,
                            }
                        }
                        // Skip optional whitespace after unicode escape
                        if let Some(ch) = source[*pos..].chars().next()
                            && ch.is_whitespace()
                        {
                            *pos += ch.len_utf8();
                        }
                    } else {
                        // Regular escape - skip the escaped character
                        *pos += next_ch.len_utf8();
                    }
                } else {
                    return Err(ParseError::InvalidSyntax {
                        message: "Unexpected end of string after backslash".to_string(),
                        position: *pos,
                        context: None,
                    });
                }
            }
            Some(ch) => {
                *pos += ch.len_utf8();
            }
        }
    }
}
