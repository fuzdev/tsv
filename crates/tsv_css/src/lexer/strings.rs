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
/// fall through the run untouched, landing on the same terminator the former
/// per-char decode found. A `\` consumes itself plus the first byte of whatever it
/// escapes — enough, since the escaped char's remaining continuation bytes can match
/// neither target.
///
/// The run is [`tsv_lang::swar::next_byte_of`], eight bytes a word, behind a two-compare
/// test of its FIRST byte — and that pre-test is the whole lever, not an optimization of
/// it. A stylesheet's string runs are bimodal twice over: **half of them are empty**,
/// because the `\` of an icon-font escape (`content: "\e901"`) sits against the opening
/// quote and the escape step lands three bytes from the close; and while 98% of the runs
/// are under a word, 86% of the *bytes* sit in runs of 128 or more, which are data URIs.
/// A per-byte reading of that histogram sizes only the second mode. What the machine
/// actually pays is the word loop's entry, about fifteen instructions, on every one of
/// the empty runs — so retiring those on two ALU compares is what makes the word loop
/// win on a stylesheet holding no long string at all.
///
/// ⚠️ **The first-byte test is deliberately NOT a 256-entry skip table**, which is what
/// this scan's whole run used to be. A table is the right rung for a *run* — six
/// instructions a byte against a compare chain's thirteen — but for a single byte it
/// puts a dependent L1 load at the head of every call, and the same spelling with the
/// table measured a point of cycles slower on both CSS populations while removing
/// marginally *more* instructions. Rung by run length, not by habit.
pub(crate) fn string_end(bytes: &[u8], open: usize) -> Result<usize, StringScanEnd> {
    let quote_byte = bytes[open];
    debug_assert!(quote_byte == b'"' || quote_byte == b'\'');
    let len = bytes.len();
    let mut p = open + 1; // past the opening quote
    loop {
        if p < len && bytes[p] != quote_byte && bytes[p] != b'\\' {
            p = tsv_lang::swar::next_byte_of(bytes, p + 1, [quote_byte, b'\\']);
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
