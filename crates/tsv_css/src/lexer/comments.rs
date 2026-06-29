use super::lex_err;
use super::token::{Token, TokenKind};
use tsv_lang::ParseError;

/// Read a CSS comment: /* ... */
/// Content extracted via source[start+2..end-2]
pub(crate) fn read_comment(source: &str, pos: &mut usize) -> Result<Token, Box<ParseError>> {
    let start = *pos;

    // Skip /*
    *pos += 1; // /
    *pos += 1; // *

    loop {
        let current_char = source[*pos..].chars().next();
        match current_char {
            None => {
                return Err(lex_err("Unterminated comment", start));
            }
            Some('*') => {
                let peek_char = source[*pos + 1..].chars().next();
                if peek_char == Some('/') {
                    *pos += 1; // *
                    *pos += 1; // /
                    break;
                }
                *pos += 1;
            }
            Some(ch) => {
                *pos += ch.len_utf8();
            }
        }
    }

    Ok(Token {
        kind: TokenKind::Comment,
        start: start as u32,
        end: *pos as u32,
    })
}
