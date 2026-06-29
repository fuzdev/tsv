use super::token::{Token, TokenKind};
use tsv_lang::ParseError;

/// Read a TypeScript line comment: // ...
/// Records `content_start` after the `//` prefix; the content (the source slice
/// `[content_start, end)`) is recovered on demand, not copied here.
/// Reads until end of line or end of file
///
/// NOTE: Content is preserved exactly as written. Indentation stripping for multi-line
/// block comments happens in the conversion layer (matching Svelte's behavior).
pub(crate) fn read_line_comment(source: &str, pos: &mut usize) -> Result<Token, Box<ParseError>> {
    let start = *pos;

    // Skip //
    *pos += 1; // /
    *pos += 1; // /

    // Scan to the end of the comment without copying — the content is recovered
    // on demand as a source slice (`[start + 2, end)`).
    //
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
                *pos += ch.len_utf8();
            }
        }
    }

    Ok(Token {
        kind: TokenKind::Comment {
            is_block: false,
            content_start: (start + 2) as u32,
        },
        start: start as u32,
        end: *pos as u32,
    })
}

/// Read a TypeScript block comment: /* ... */
/// Records `content_start` after the `/*`; the content (the source slice
/// `[content_start, end - 2)`) is recovered on demand, not copied here.
/// Note: Unlike CSS, JS/TypeScript does NOT support nested block comments
///
/// NOTE: Content is preserved exactly as written. Indentation stripping for multi-line
/// comments happens in the conversion layer (matching Svelte's behavior).
pub(crate) fn read_block_comment(source: &str, pos: &mut usize) -> Result<Token, Box<ParseError>> {
    let start = *pos;

    // Skip /*
    *pos += 1; // /
    *pos += 1; // *

    // Scan to the closing `*/` without copying — the content is recovered on
    // demand as a source slice (`[start + 2, end - 2)`).
    loop {
        let current_char = source[*pos..].chars().next();
        match current_char {
            None => {
                return Err(Box::new(ParseError::InvalidSyntax {
                    message: "Unterminated block comment".to_string(),
                    position: start,
                    context: None,
                }));
            }
            Some('*') => {
                // Check for closing */
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
        kind: TokenKind::Comment {
            is_block: true,
            content_start: (start + 2) as u32,
        },
        start: start as u32,
        end: *pos as u32,
    })
}
