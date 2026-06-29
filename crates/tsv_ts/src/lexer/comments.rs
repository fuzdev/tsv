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
    let bytes = source.as_bytes();
    let len = bytes.len();

    // Scan to the end of the comment over raw bytes — the content is recovered on
    // demand as a source slice (`[start + 2, end)`), never copied here.
    //
    // A line comment ends at the first LineTerminator — LF, CR, or the UTF-8
    // line/paragraph separators U+2028/U+2029 (`e2 80 a8`/`a9`) — or EOF. The
    // terminator is NOT consumed; it's whitespace for the next token.
    //
    // Byte-at-a-time is sound: none of LF/CR/`0xe2` ever appears as a UTF-8
    // continuation byte (those are `0x80..=0xbf`), and in valid UTF-8 `0xe2` is
    // always a 3-byte lead, so the LS/PS peek lands on a char boundary. This
    // tight loop auto-vectorizes (vs the former per-char `chars().next()` decode).
    let mut p = start + 2; // skip //
    while p < len {
        match bytes[p] {
            b'\n' | b'\r' => break,
            0xe2 if p + 2 < len && bytes[p + 1] == 0x80 && matches!(bytes[p + 2], 0xa8 | 0xa9) => {
                break;
            }
            _ => p += 1,
        }
    }
    *pos = p;

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
    let bytes = source.as_bytes();
    let len = bytes.len();

    // Scan to the closing `*/` over raw bytes — the content is recovered on demand
    // as a source slice (`[start + 2, end - 2)`), never copied here. The inner
    // `!= b'*'` run is a single-byte search the compiler auto-vectorizes; `*`
    // (`0x2a`) is ASCII and never a UTF-8 continuation byte, so byte-at-a-time is
    // sound (vs the former per-char `chars().next()` decode).
    let mut p = start + 2; // skip /*
    loop {
        while p < len && bytes[p] != b'*' {
            p += 1;
        }
        if p >= len {
            return Err(Box::new(ParseError::InvalidSyntax {
                message: "Unterminated block comment".to_string(),
                position: start,
                context: None,
            }));
        }
        // bytes[p] == b'*'
        if bytes.get(p + 1) == Some(&b'/') {
            p += 2; // consume */
            break;
        }
        p += 1; // a `*` not followed by `/` — keep scanning
    }
    *pos = p;

    Ok(Token {
        kind: TokenKind::Comment {
            is_block: true,
            content_start: (start + 2) as u32,
        },
        start: start as u32,
        end: *pos as u32,
    })
}
