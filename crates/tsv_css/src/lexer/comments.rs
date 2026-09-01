use super::lex_err;
use super::token::{Token, TokenKind};
use tsv_lang::ParseError;

/// Read a CSS comment: /* ... */
/// Content extracted via source[start+2..end-2]
pub(crate) fn read_comment(source: &str, pos: &mut usize) -> Result<Token, ParseError> {
    let start = *pos;
    let bytes = source.as_bytes();
    let len = bytes.len();

    // Comment bodies are opaque — the content is recovered as a source slice — and the
    // only scan targets are `*` and `/`. Both are ASCII, so neither can occur as a UTF-8
    // continuation byte: stepping a byte at a time through a multi-byte char lands on
    // bytes >= 0x80, which fail the `*` test and advance exactly as the former per-char
    // decode did.
    //
    // The run to the next `*` is [`tsv_lang::swar::next_byte_of`] rather than the byte
    // loop it reads as. A `*` that opens no `*/` resumes the run, and LLVM fuses the two
    // into one scalar loop at ten instructions and three branches a byte — the word loop
    // asks the same question of eight bytes at one branch.
    let mut p = start + 2; // past `/*`
    loop {
        p = tsv_lang::swar::next_byte_of(bytes, p, [b'*']);
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
