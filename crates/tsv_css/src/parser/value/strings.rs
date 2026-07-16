use crate::ast::internal::{CssValue, StringCooked};
use crate::escapes;
use bumpalo::Bump;
use std::borrow::Cow;
use tsv_lang::Span;

/// Parse CSS string with proper quote handling and escape decoding
///
/// Extracts content between quotes and decodes CSS escape sequences.
/// The internal AST stores fully decoded strings for semantic correctness.
///
/// # Examples
/// - `"test"` → content: `test`, quote: `"`
/// - `"test\\n"` → content: `test\n` (decoded newline), quote: `"`
/// - `"\\41"` → content: `A` (decoded unicode U+0041), quote: `"`
///
/// # Architecture
/// - Lexer: Preserves raw escape sequences exactly as written
/// - Parser: Decodes standard CSS escapes into clean internal AST
/// - Conversion: Re-applies Svelte quirks when generating public JSON AST
///
/// This matches TypeScript's architecture and keeps the internal AST clean.
pub fn parse_string_literal<'arena>(
    s: &str,
    span: Span,
    arena: &'arena Bump,
) -> Option<CssValue<'arena>> {
    let bytes = s.as_bytes();
    if let Some(&quote) = bytes.first()
        && (quote == b'"' || quote == b'\'')
        && quoted_string_spans_all(bytes, quote)
    {
        // Extract content without quotes
        let raw_content = &s[1..s.len() - 1];

        // Decode CSS escape sequences. No-escape strings stay `Verbatim` (zero alloc —
        // the printer recovers the text from `span`); only escaped strings own arena
        // bytes. The quote char is recovered from `source[span.start]`, not stored.
        let content = match escapes::decode_escape_sequences(raw_content) {
            Cow::Borrowed(_) => StringCooked::Verbatim,
            Cow::Owned(decoded) => StringCooked::Decoded(arena.alloc_str(&decoded)),
        };

        return Some(CssValue::String { content, span });
    }
    None
}

/// Whether the quoted string that opens at byte 0 of `bytes` (with delimiter
/// `quote`) spans **all** of `bytes` — i.e. the opening quote is closed by an
/// unescaped matching quote at the final byte, and by none earlier.
///
/// The naive "starts with a quote and ends with the matching quote" test is not
/// enough: a glued run like `'a'x'b'` starts and ends with `'`, but its first
/// string closes at index 2, so it is really three value tokens (string, ident,
/// string) — the same shape the CSS tokenizer produces. Treating it as a single
/// string strips the outer quotes and re-quotes the interior, turning the delimiter
/// quotes into literal content (`'a'x'b'` → `"a'x'b"`, a different value). Such runs
/// return `false` here and are kept verbatim as an opaque `Identifier` instead.
///
/// Scans **bytes**, not chars: `quote` and `\` are ASCII, and no ASCII byte ever
/// appears inside a multi-byte UTF-8 code point (continuation bytes are all ≥ 0x80),
/// so a byte scan can't false-match a delimiter and skips the per-char decode this
/// per-value-token path would otherwise pay. A `\` escapes the following byte (a CSS
/// string escape / line continuation), so an escaped interior quote (`'a\'b'`) is
/// stepped over and the string still closes at the end — a genuine single string is
/// unaffected. Skipping only the one byte after `\` suffices: if that escaped code
/// point is multi-byte, its trailing bytes are ≥ 0x80 and can't match either.
fn quoted_string_spans_all(bytes: &[u8], quote: u8) -> bool {
    let mut i = 1; // past the opening quote at byte 0
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2, // escape introducer + the byte it escapes
            b if b == quote => return i == bytes.len() - 1, // first unescaped close
            _ => i += 1,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::quoted_string_spans_all;

    fn spans_all(s: &str, quote: u8) -> bool {
        quoted_string_spans_all(s.as_bytes(), quote)
    }

    #[test]
    fn single_complete_strings_span_all() {
        assert!(spans_all("'abc'", b'\''));
        assert!(spans_all("\"abc\"", b'"'));
        assert!(spans_all("''", b'\'')); // empty string
        assert!(spans_all("'a\"b'", b'\'')); // other quote inside
        assert!(spans_all("'a\\'b'", b'\'')); // escaped interior quote
        assert!(spans_all("'é\"café'", b'\'')); // multi-byte content, other quote inside
    }

    #[test]
    fn glued_runs_do_not_span_all() {
        // First `'` closes at index 2, not at the end — a string+ident+string run.
        assert!(!spans_all("'a'x'b'", b'\''));
        // Two directly-adjacent strings.
        assert!(!spans_all("'a''b'", b'\''));
        assert!(!spans_all("\"a\"x\"b\"", b'"'));
        // Escaped backslash then a real close, then trailing content.
        assert!(!spans_all("'a\\\\'b'", b'\''));
        // Unterminated open (final quote is escaped): not a complete single string.
        assert!(!spans_all("'a\\'", b'\''));
    }
}
