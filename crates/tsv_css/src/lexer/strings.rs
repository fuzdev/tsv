use super::lex_err;
use super::token::{Token, TokenKind};
use tsv_lang::ParseError;

/// Read a CSS string: "..." or '...'
/// Preserves raw escape sequences exactly as written (no quirks applied)
/// Content extracted via source[start+1..end-1]
///
/// **Architecture**: Lexer preserves raw content → Parser decodes → Conversion applies Svelte quirks
/// This matches TypeScript's approach and keeps the lexer simple and consistent.
pub(crate) fn read_string(source: &str, pos: &mut usize, quote: char) -> Result<Token, ParseError> {
    let start = *pos;
    match string_end(source.as_bytes(), start) {
        Ok(p) => {
            *pos = p;
            Ok(Token {
                kind: TokenKind::String { quote },
                start: start as u32,
                end: p as u32,
            })
        }
        Err(StringScanEnd::Unterminated) => Err(lex_err(
            format!("Unterminated string starting with {quote}"),
            start,
        )),
        Err(StringScanEnd::TrailingBackslash(at)) => {
            Err(lex_err("Unexpected end of string after backslash", at))
        }
    }
}

/// Why [`string_end`]'s scan failed, carrying the position the lexer's error wants.
pub(crate) enum StringScanEnd {
    /// End-of-source before the closing quote.
    Unterminated,
    /// A `\` as the final byte; the payload is one past it (the error position).
    TrailingBackslash(usize),
}

/// End of the string whose opening quote sits at `open` (one past its closing quote).
/// The single statement of the string token's extent — the lexer maps the error arms to
/// its two messages, `decl_scan` (the second reader of this grammar) declines on either.
///
/// The two scan targets — the quote and `\` — are ASCII, so neither can occur as a
/// UTF-8 continuation byte: a multi-byte char's trailing bytes are all >= 0x80 and
/// fall through the skip run untouched, landing on the same terminator the former
/// per-char decode found. The run is a two-byte search the compiler auto-vectorizes.
/// A `\` consumes itself plus the first byte of whatever it escapes — enough, since the
/// escaped char's remaining continuation bytes can match neither target.
pub(crate) fn string_end(bytes: &[u8], open: usize) -> Result<usize, StringScanEnd> {
    let quote_byte = bytes[open];
    let len = bytes.len();
    let mut p = open + 1; // past the opening quote
    loop {
        while p < len && bytes[p] != quote_byte && bytes[p] != b'\\' {
            p += 1;
        }
        if p >= len {
            return Err(StringScanEnd::Unterminated);
        }
        if bytes[p] == quote_byte {
            return Ok(p + 1); // one past the closing quote
        }
        if p + 1 >= len {
            return Err(StringScanEnd::TrailingBackslash(p + 1));
        }
        p += 2;
    }
}
