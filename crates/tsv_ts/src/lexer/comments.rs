use super::is_es_line_terminator_at;
use super::lex_err;
use super::token::{Token, TokenKind};
use tsv_lang::ParseError;

/// Read a TypeScript line comment: // ...
/// Records `content_start` after the `//` prefix; the content (the source slice
/// `[content_start, end)`) is recovered on demand, not copied here.
/// Reads until end of line or end of file
///
/// NOTE: Content is preserved exactly as written. Indentation stripping for multi-line
/// block comments happens in the conversion layer (matching Svelte's behavior).
pub(crate) fn read_line_comment(source: &str, pos: &mut usize) -> Result<Token, ParseError> {
    let start = *pos;
    let bytes = source.as_bytes();
    let len = bytes.len();

    // Scan to the end of the comment over raw bytes — the content is recovered on
    // demand as a source slice (`[start + 2, end)`), never copied here.
    //
    // A line comment ends at the first `LineTerminator` — or EOF; the terminator is NOT
    // consumed, it's whitespace for the next token. The class is
    // [`is_es_line_terminator_at`], the one byte-level spelling of the production, shared
    // with the parser's scans so the three scanners that each hand-rolled the LS/PS peek
    // cannot drift apart again. Byte-at-a-time is sound: none of its bytes ever appears
    // as a UTF-8 continuation byte, so the peek always lands on a char boundary.
    //
    // The hand-rolled predecessor was held back on a *suspected* vectorization advantage
    // that no measurement had ever graded. Settled by codegen, not wall clock: under the
    // `corpus` profile this function is 139 bytes where the hand-rolled loop was 152, the
    // per-byte hot path is the same nine instructions either way, and **neither**
    // spelling vectorizes. The saving is in the rare `0xE2` arm — the old form tested
    // `p + 2 < len` against a hoisted local, so LLVM could not fold the `bytes[p + 1]`
    // bounds check and kept a panic landing pad; the helper compares against
    // `bytes.len()` directly and both checks fold away. Sharing the production is the
    // cheaper spelling, not a concession.
    let mut p = start + 2; // skip //
    while p < len && !is_es_line_terminator_at(bytes, p) {
        p += 1;
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
pub(crate) fn read_block_comment(source: &str, pos: &mut usize) -> Result<Token, ParseError> {
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
            return Err(lex_err("Unterminated block comment", start));
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
