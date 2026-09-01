use super::ES_LINE_TERMINATOR_LEADS;
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
    // The scan runs word-at-a-time over the class's LEADS
    // ([`ES_LINE_TERMINATOR_LEADS`]) and re-tests each hit against the exact production,
    // resuming on a `0xE2` that opens some other character —
    // [`tsv_lang::swar::next_byte_of`]'s loose-class-plus-exact-fallback shape, so the
    // terminator rule is still stated once. Asking the exact predicate per byte, as the
    // loop reads, costs ten instructions and four branches a byte and vectorizes at
    // none of them; the word loop asks it of eight bytes at one branch.
    let mut p = start + 2; // skip //
    loop {
        p = tsv_lang::swar::next_byte_of(bytes, p, ES_LINE_TERMINATOR_LEADS);
        if p >= len || is_es_line_terminator_at(bytes, p) {
            break;
        }
        // A `0xE2` that leads some character other than `<LS>` / `<PS>` — comment
        // content, so the run resumes past it.
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
    // as a source slice (`[start + 2, end - 2)`), never copied here. `*` (`0x2a`) is
    // ASCII and never a UTF-8 continuation byte, so a byte scan is sound (vs the former
    // per-char `chars().next()` decode) — but a `*` that opens no `*/` resumes the run,
    // which is why the compare-chain spelling stayed scalar: LLVM fused the run with
    // the resume test and emitted ten instructions and three branches a byte, and a
    // JSDoc block hits that resume on every line. [`tsv_lang::swar::next_byte_of`]
    // asks the same question of eight bytes at once.
    let mut p = start + 2; // skip /*
    loop {
        p = tsv_lang::swar::next_byte_of(bytes, p, [b'*']);
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
