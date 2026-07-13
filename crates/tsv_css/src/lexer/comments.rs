use super::lex_err;
use super::token::{Token, TokenKind};
use tsv_lang::ParseError;

/// Read a CSS comment: /* ... */
/// Content extracted via source[start+2..end-2]
pub(crate) fn read_comment(source: &str, pos: &mut usize) -> Result<Token, Box<ParseError>> {
    let start = *pos;
    let bytes = source.as_bytes();
    let len = bytes.len();

    // Comment bodies are opaque — the content is recovered as a source slice — and the
    // only scan targets are `*` and `/`. Both are ASCII, so neither can occur as a UTF-8
    // continuation byte: stepping a byte at a time through a multi-byte char lands on
    // bytes >= 0x80, which fail the `*` test and advance exactly as the former per-char
    // decode did. The inner run is a single-byte search the compiler auto-vectorizes.
    let mut p = start + 2; // past `/*`
    loop {
        while p < len && bytes[p] != b'*' {
            p += 1;
        }
        if p >= len {
            return Err(lex_err("Unterminated comment", start));
        }
        if bytes.get(p + 1) == Some(&b'/') {
            p += 2; // past `*/`
            break;
        }
        p += 1;
    }

    *pos = p;
    Ok(Token {
        kind: TokenKind::Comment,
        start: start as u32,
        end: p as u32,
    })
}
