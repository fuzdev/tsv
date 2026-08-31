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

/// The "this byte cannot end the string" table for a string opened by `quote`: `true`
/// for every byte that is neither the closing quote nor the `\` that opens an escape,
/// so the scan loop's whole body collapses to `p += 1` behind one L1 load. Same idiom as
/// the value scanners' `value_skip_table!` (`parser::value::scan`) — a per-byte
/// branch chain is what these scans are made of — with **all 256 entries populated**,
/// unlike that macro: a string's interior is opaque text, so a byte ≥ 0x80 is content and
/// must be skipped like every other non-target byte.
///
/// Two tables, because a string token opens with `"` or `'` and nothing else
/// (css-syntax §4.3.5). That is a **precondition**, not a fallback: given any other
/// opening byte the double-quote table would stop the loop at a `"` that [`string_end`]
/// then reads as an escape introducer. Every call site reaches here from a literal
/// `b'"' | b'\''` match, and `string_end` asserts it in debug.
const fn string_skip_table(quote: u8) -> &'static [bool; 256] {
    const fn build(quote: u8) -> [bool; 256] {
        let mut t = [true; 256];
        t[quote as usize] = false;
        t[b'\\' as usize] = false;
        t
    }
    const SINGLE: [bool; 256] = build(b'\'');
    const DOUBLE: [bool; 256] = build(b'"');
    if quote == b'\'' { &SINGLE } else { &DOUBLE }
}

/// End of the string whose opening quote sits at `open` (one past its closing quote).
/// The single statement of the string token's extent — the lexer maps the error arms to
/// its two messages, `decl_scan` (the second reader of this grammar) declines on either.
///
/// The two scan targets — the quote and `\` — are ASCII, so neither can occur as a
/// UTF-8 continuation byte: a multi-byte char's trailing bytes are all >= 0x80 and
/// fall through the skip run untouched, landing on the same terminator the former
/// per-char decode found. A `\` consumes itself plus the first byte of whatever it
/// escapes — enough, since the escaped char's remaining continuation bytes can match
/// neither target.
///
/// The run is a [`string_skip_table`] lookup rather than the two compares it reads as.
/// Spelled as compares, the codegen is branchless — a `setne` per target, a `test`, and
/// the loop's own step, about thirteen instructions a byte — and it does not vectorize
/// either, because the escape arm makes the stride data-dependent. The table asks the
/// same question in one L1 load, which is the shape every other byte scanner in this
/// crate already has.
pub(crate) fn string_end(bytes: &[u8], open: usize) -> Result<usize, StringScanEnd> {
    let quote_byte = bytes[open];
    debug_assert!(quote_byte == b'"' || quote_byte == b'\'');
    let skip = string_skip_table(quote_byte);
    let len = bytes.len();
    let mut p = open + 1; // past the opening quote
    loop {
        while p < len && skip[bytes[p] as usize] {
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
