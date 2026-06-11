use super::token::{Token, TokenKind};
use tsv_lang::ParseError;

/// Read a TypeScript line comment: // ...
/// Returns the comment content WITHOUT the // prefix
/// Reads until end of line or end of file
///
/// NOTE: Content is preserved exactly as written. Indentation stripping for multi-line
/// block comments happens in the conversion layer (matching Svelte's behavior).
pub(crate) fn read_line_comment(source: &str, pos: &mut usize) -> Result<Token, ParseError> {
    let start = *pos;

    // Skip //
    *pos += 1; // /
    *pos += 1; // /

    let mut content = String::new();

    // Read until line terminator or EOF — U+2028/U+2029 terminate line
    // comments like LF/CR per the spec
    loop {
        let current_char = source[*pos..].chars().next();
        match current_char {
            None | Some('\n' | '\r' | '\u{2028}' | '\u{2029}') => {
                // End of line comment
                // Don't consume the line terminator - it's whitespace for the next token
                break;
            }
            Some(ch) => {
                content.push(ch);
                *pos += ch.len_utf8();
            }
        }
    }

    Ok(Token {
        kind: TokenKind::Comment {
            content,
            is_block: false,
        },
        start,
        end: *pos,
        decoded: None,
    })
}

/// Read a TypeScript block comment: /* ... */
/// Returns the comment content WITHOUT the /* */ delimiters
/// Note: Unlike CSS, JS/TypeScript does NOT support nested block comments
///
/// NOTE: Content is preserved exactly as written. Indentation stripping for multi-line
/// comments happens in the conversion layer (matching Svelte's behavior).
pub(crate) fn read_block_comment(source: &str, pos: &mut usize) -> Result<Token, ParseError> {
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
                    message: "Unterminated block comment".to_string(),
                    position: start,
                    context: None,
                });
            }
            Some('*') => {
                // Check for closing */
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

    Ok(Token {
        kind: TokenKind::Comment {
            content,
            is_block: true,
        },
        start,
        end: *pos,
        decoded: None,
    })
}
