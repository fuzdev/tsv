use super::token::{Token, TokenKind};
use tsv_lang::ParseError;

/// Read a CSS comment: /* ... */
/// Returns the comment content WITHOUT the /* */ delimiters
pub(crate) fn read_comment(source: &str, pos: &mut usize) -> Result<Token, ParseError> {
    let start = *pos;

    // Skip /*
    *pos += 1; // /
    *pos += 1; // *

    let mut content = String::new();

    loop {
        let current_char = source[*pos..].chars().next();
        match current_char {
            None => {
                return Err(ParseError::InvalidSyntax {
                    message: "Unterminated comment".to_string(),
                    position: start,
                    context: None,
                });
            }
            Some('*') => {
                let peek_char = source[*pos + 1..].chars().next();
                if peek_char == Some('/') {
                    *pos += 1; // *
                    *pos += 1; // /
                    break;
                }
                content.push('*');
                *pos += 1;
            }
            Some(ch) => {
                content.push(ch);
                *pos += ch.len_utf8();
            }
        }
    }

    // Preserve comment content EXACTLY as written (prettier doesn't normalize)
    Ok(Token {
        kind: TokenKind::Comment(content),
        start,
        end: *pos,
        decoded: None,
    })
}
