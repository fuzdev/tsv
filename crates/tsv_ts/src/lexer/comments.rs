use super::ES_LINE_TERMINATOR_LEADS;
use super::is_es_line_terminator_at;
use super::lex_err;
use tsv_lang::ParseError;

/// The end of the TypeScript line comment `// ...` opening at `start`: the offset of the
/// first `LineTerminator` after the `//`, or the end of input. The terminator is NOT part
/// of the comment — it is whitespace for the next token. The content (the source slice
/// `[start + 2, end)`) is recovered on demand, never copied here, and the caller builds
/// the token.
pub(crate) fn line_comment_end(bytes: &[u8], start: usize) -> usize {
    let len = bytes.len();

    // The terminator class is [`is_es_line_terminator_at`], the one byte-level spelling of
    // the production, shared with the parser's scans so the three scanners that each
    // hand-rolled the LS/PS peek cannot drift apart again. Byte-at-a-time is sound: none of
    // its bytes ever appears as a UTF-8 continuation byte, so the peek always lands on a
    // char boundary.
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
            return p;
        }
        // A `0xE2` that leads some character other than `<LS>` / `<PS>` — comment
        // content, so the run resumes past it.
        p += 1;
    }
}

/// The end of the TypeScript block comment `/* ... */` opening at `start`: the offset just
/// past its closing `*/`, or an error at `start` when none follows. The content (the source
/// slice `[start + 2, end - 2)`) is recovered on demand, never copied here, and the caller
/// builds the token. Block comments do not nest: the first `*/` closes the comment.
///
/// NOTE: Content is preserved exactly as written. Indentation stripping for multi-line
/// comments happens in the conversion layer (matching Svelte's behavior).
pub(crate) fn block_comment_end(bytes: &[u8], start: usize) -> Result<usize, ParseError> {
    let len = bytes.len();

    // `*` (`0x2a`) is ASCII and never a UTF-8 continuation byte, so a byte scan is sound
    // (vs the former per-char `chars().next()` decode) — but a `*` that opens no `*/`
    // resumes the run, which is why the compare-chain spelling stayed scalar: LLVM fused
    // the run with the resume test and emitted ten instructions and three branches a
    // byte, and a JSDoc block hits that resume on every line.
    // [`tsv_lang::swar::next_byte_of`] asks the same question of eight bytes at once.
    let mut p = start + 2; // skip /*
    loop {
        p = tsv_lang::swar::next_byte_of(bytes, p, [b'*']);
        if p >= len {
            return Err(lex_err("Unterminated block comment", start));
        }
        // bytes[p] == b'*'
        if bytes.get(p + 1) == Some(&b'/') {
            return Ok(p + 2); // consume */
        }
        p += 1; // a `*` not followed by `/` — keep scanning
    }
}
