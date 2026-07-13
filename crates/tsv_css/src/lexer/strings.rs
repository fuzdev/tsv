use super::lex_err;
use super::token::{Token, TokenKind};
use tsv_lang::ParseError;

/// Read a CSS string: "..." or '...'
/// Preserves raw escape sequences exactly as written (no quirks applied)
/// Content extracted via source[start+1..end-1]
///
/// **Architecture**: Lexer preserves raw content → Parser decodes → Conversion applies Svelte quirks
/// This matches TypeScript's approach and keeps the lexer simple and consistent.
pub(crate) fn read_string(
    source: &str,
    pos: &mut usize,
    quote: char,
) -> Result<Token, Box<ParseError>> {
    let start = *pos;
    let bytes = source.as_bytes();
    let len = bytes.len();
    // The dispatch only ever passes `"` or `'`, both ASCII.
    let quote_byte = quote as u8;

    // The two scan targets — the quote and `\` — are ASCII, so neither can occur as a
    // UTF-8 continuation byte: a multi-byte char's trailing bytes are all >= 0x80 and
    // fall through the skip run untouched, landing on the same terminator the former
    // per-char decode found. The run is a two-byte search the compiler auto-vectorizes.
    let mut p = start + 1; // past the opening quote
    loop {
        while p < len && bytes[p] != quote_byte && bytes[p] != b'\\' {
            p += 1;
        }
        if p >= len {
            return Err(lex_err(
                format!("Unterminated string starting with {quote}"),
                start,
            ));
        }
        if bytes[p] == quote_byte {
            p += 1; // past the closing quote
            *pos = p;
            return Ok(Token {
                kind: TokenKind::String { quote },
                start: start as u32,
                end: p as u32,
            });
        }
        // A `\`: consume it plus the first byte of whatever it escapes. Skipping only
        // that first byte suffices — the escaped char's remaining continuation bytes can
        // match neither the quote nor `\`, so the run passes over them unchanged.
        if p + 1 >= len {
            return Err(lex_err("Unexpected end of string after backslash", p + 1));
        }
        p += 2;
    }
}
