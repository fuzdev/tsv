use super::token::{Token, TokenKind};
use tsv_lang::ParseError;

/// Read a CSS string: "..." or '...'
/// Preserves raw escape sequences exactly as written (no quirks applied)
/// Content extracted via source[start+1..end-1]
///
/// **Architecture**: Lexer preserves raw content → Parser decodes → Conversion applies Svelte quirks
/// This matches TypeScript's approach and keeps the lexer simple and consistent.
pub(crate) fn read_string(source: &str, pos: &mut usize, quote: char) -> Result<Token, ParseError> {
    let start = *pos;
    *pos += 1; // skip opening quote

    // Scan through string to find closing quote
    loop {
        let current_char = source[*pos..].chars().next();
        match current_char {
            None => {
                return Err(ParseError::InvalidSyntax {
                    message: format!("Unterminated string starting with {quote}"),
                    position: start,
                    context: None,
                });
            }
            Some(ch) if ch == quote => {
                *pos += 1; // skip closing quote

                return Ok(Token {
                    kind: TokenKind::String { quote },
                    start,
                    end: *pos,
                    decoded: None,
                });
            }
            Some('\\') => {
                *pos += 1; // skip backslash
                // Skip the escaped character
                if let Some(next_ch) = source[*pos..].chars().next() {
                    *pos += next_ch.len_utf8();
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
