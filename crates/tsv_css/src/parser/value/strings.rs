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
        && quoted_string_spans_all(bytes)
    {
        // Extract content without quotes
        let raw_content = &s[1..s.len() - 1];

        // Decode CSS escape sequences. No-escape strings stay `Verbatim` (zero alloc —
        // the printer recovers the text from `span`); only escaped strings own arena
        // bytes. The quote char is recovered from `source[span.start]`, not stored.
        let content = match escapes::decode_escape_sequences_in(raw_content, arena) {
            None => StringCooked::Verbatim,
            Some(decoded) => StringCooked::Decoded(decoded),
        };
        debug_assert!(
            match (&content, escapes::decode_escape_sequences(raw_content)) {
                (StringCooked::Verbatim, Cow::Borrowed(_)) => true,
                (StringCooked::Decoded(decoded), Cow::Owned(reference)) => **decoded == reference,
                _ => false,
            },
            "the arena decoder left the reference decoder on {raw_content:?}"
        );

        return Some(CssValue::String { content, span });
    }
    None
}

/// Whether the quoted string that opens at byte 0 of `bytes` spans **all** of `bytes`
/// — i.e. the opening quote is closed by an unescaped matching quote at the final byte,
/// and by none earlier. The delimiter is `bytes[0]`, which the caller has already
/// established is `"` or `'`; nothing here needs to be told which.
///
/// The naive "starts with a quote and ends with the matching quote" test is not
/// enough: a glued run like `'a'x'b'` starts and ends with `'`, but its first
/// string closes at index 2, so it is really three value tokens (string, ident,
/// string) — the same shape the CSS tokenizer produces. Treating it as a single
/// string strips the outer quotes and re-quotes the interior, turning the delimiter
/// quotes into literal content (`'a'x'b'` → `"a'x'b"`, a different value). Such runs
/// return `false` here and are kept verbatim as an opaque `Identifier` instead.
///
/// "Where does the string close" is [`string_end`](crate::lexer::string_end)'s
/// question, and this is that question with the answer compared against the end — so
/// it is asked there rather than spelled a second time here. The escape rule the two
/// need is identical (`\` covers itself and the byte after it, so an escaped interior
/// quote like `'a\'b'` is stepped over and a genuine single string still closes at the
/// end), and so is the reason a byte scan suffices for it: the delimiter and `\` are ASCII,
/// and no ASCII byte appears inside a multi-byte UTF-8 code point, so nothing can
/// false-match a delimiter and no per-char decode is owed. Two spellings of one
/// grammar also cost twice — this walk was a per-byte match arm chain over the same
/// bytes `string_end` scans a word at a time.
fn quoted_string_spans_all(bytes: &[u8]) -> bool {
    matches!(crate::lexer::string_end(bytes, 0), Ok(end) if end == bytes.len())
}

#[cfg(test)]
mod tests {
    use super::{parse_string_literal, quoted_string_spans_all};
    use crate::ast::internal::{CssValue, StringCooked};
    use crate::escapes::decode_escape_sequences;
    use bumpalo::Bump;
    use std::borrow::Cow;
    use tsv_lang::Span;

    /// A string literal's decoded payload is the reference decoder's, in both quote
    /// kinds, and an unterminated or early-closed run is no string at all — whatever
    /// escapes it holds.
    #[test]
    fn a_literal_decodes_as_the_reference_decoder_does() {
        let bodies = [
            "",
            "abc",
            "é",
            r"\41",
            r"\41 b",
            "\\41\r\nb",
            r"\110000",
            r"\D800x",
            r"\0",
            "a\\\nb",
            r"a\'b",
            r#"a\"b"#,
            r"\\",
            r"é\é",
        ];
        let arena = Bump::new();
        for body in bodies {
            for quote in ['"', '\''] {
                let literal = format!("{quote}{body}{quote}");
                let span = Span::new(0, literal.len() as u32);
                let Some(CssValue::String { content, .. }) =
                    parse_string_literal(&literal, span, &arena)
                else {
                    panic!("{literal:?} is one string");
                };
                match (content, decode_escape_sequences(body)) {
                    (StringCooked::Verbatim, Cow::Borrowed(_)) => {}
                    (StringCooked::Decoded(decoded), Cow::Owned(reference)) => {
                        assert_eq!(decoded, reference, "{literal:?}");
                    }
                    (content, reference) => panic!("{literal:?}: {content:?} vs {reference:?}"),
                }
                // Unterminated, closed by an escaped quote alone, and closed early.
                for open in [
                    format!("{quote}{body}"),
                    format!("{quote}{body}\\{quote}"),
                    format!("{quote}{body}{quote}x{quote}"),
                ] {
                    let span = Span::new(0, open.len() as u32);
                    assert!(
                        parse_string_literal(&open, span, &arena).is_none(),
                        "{open:?} is not one closed string"
                    );
                }
            }
        }
    }

    fn spans_all(s: &str) -> bool {
        quoted_string_spans_all(s.as_bytes())
    }

    #[test]
    fn single_complete_strings_span_all() {
        assert!(spans_all("'abc'"));
        assert!(spans_all("\"abc\""));
        assert!(spans_all("''")); // empty string
        assert!(spans_all("'a\"b'")); // other quote inside
        assert!(spans_all("'a\\'b'")); // escaped interior quote
        assert!(spans_all("'é\"café'")); // multi-byte content, other quote inside
    }

    #[test]
    fn glued_runs_do_not_span_all() {
        // First `'` closes at index 2, not at the end — a string+ident+string run.
        assert!(!spans_all("'a'x'b'"));
        // Two directly-adjacent strings.
        assert!(!spans_all("'a''b'"));
        assert!(!spans_all("\"a\"x\"b\""));
        // Escaped backslash then a real close, then trailing content.
        assert!(!spans_all("'a\\\\'b'"));
        // Unterminated open (final quote is escaped): not a complete single string.
        assert!(!spans_all("'a\\'"));
    }
}
