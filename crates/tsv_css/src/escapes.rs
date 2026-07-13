//! CSS escape sequence decoding.

use crate::whitespace::is_css_whitespace;
use std::borrow::Cow;

/// Decode CSS escape sequences in a string.
///
/// Converts CSS escape sequences to their actual character values:
/// - `\\` → `\` (escaped backslash)
/// - `\"` → `"` (escaped quote)
/// - `\'` → `'` (escaped quote)
/// - `\n` → newline (escaped newline)
/// - `\XXXXXX` → Unicode character (1-6 hex digits)
///
/// Returns a borrowed `Cow` for the common escape-free string (no allocation);
/// only an input that actually contains `\` is decoded into an owned `String`.
pub fn decode_escape_sequences(source: &str) -> Cow<'_, str> {
    if !source.contains('\\') {
        return Cow::Borrowed(source);
    }

    let mut result = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(&next_ch) = chars.peek() {
                if next_ch.is_ascii_hexdigit() {
                    // Unicode escape sequence
                    let mut hex_digits = String::new();
                    for _ in 0..6 {
                        match chars.peek() {
                            Some(&digit) if digit.is_ascii_hexdigit() => {
                                hex_digits.push(digit);
                                chars.next();
                            }
                            _ => break,
                        }
                    }
                    // Optional whitespace terminator — the same CSS whitespace set
                    // `escape_len` uses (a hand-rolled subset here once omitted `\r`
                    // and form feed, disagreeing with every other escape scanner).
                    if let Some(&ws) = chars.peek()
                        && is_css_whitespace(ws)
                    {
                        chars.next();
                    }
                    if let Ok(code_point) = u32::from_str_radix(&hex_digits, 16)
                        && let Some(c) = char::from_u32(code_point)
                    {
                        result.push(c);
                    }
                } else {
                    // Simple escape: \X → X
                    result.push(next_ch);
                    chars.next();
                }
            } else {
                // Trailing backslash
                result.push('\\');
            }
        } else {
            result.push(ch);
        }
    }

    Cow::Owned(result)
}

/// Whether the `\` immediately before `text`'s end starts a valid escape — i.e. the
/// run of backslashes ending `text` has **odd** length, so the last one is itself
/// unescaped and escapes whatever comes next. An even run is a completed `\\`.
fn ends_with_open_escape(text: &str) -> bool {
    text.bytes().rev().take_while(|&b| b == b'\\').count() % 2 == 1
}

/// Whether `c` is a character [`trim_end_preserving_escape`] must hand back to an
/// escape as its **payload**.
///
/// §4.3.7's final branch escapes *any* code point, so an escape's payload is not
/// restricted to CSS whitespace — `\<NBSP>` is a perfectly good escape. The set here is
/// therefore keyed to what the trim can *eat*: `str::trim_end` follows Unicode
/// `White_Space`, so anything it strips must be recoverable, NBSP included. Using the
/// narrower [`is_css_whitespace`] here would let the Unicode trim swallow a payload the
/// rule then refused to restore — the very loss this exists to prevent.
///
/// The one exclusion is §4.3.4's: a `\` followed by a **newline** is not a valid escape,
/// so a newline is never a payload. (A newline *can* terminate a hex escape, which is why
/// [`escape_len`] uses [`is_css_whitespace`] there instead. Two spec rules, two
/// predicates — and neither is the duplicate that `whitespace::is_css_whitespace` already
/// owns.)
fn is_escapable_whitespace(c: char) -> bool {
    c.is_whitespace() && !matches!(c, '\n' | '\r' | '\x0C')
}

/// The byte length of the CSS escape starting at `s[i]` (which must be a `\`), or
/// `None` when that `\` does not start a valid escape.
///
/// Per CSS Syntax 3 §4.3.7 "Consume an escaped code point":
///
/// - a **hex** escape takes up to **six** hex digits and then, optionally, **one**
///   whitespace character — the *terminator*, which belongs to the escape rather than
///   separating it from what follows. So `\41 2px` is the single ident `A2px`, not
///   `\41` followed by `2px`, and a splitter that treats that space as a separator
///   tears one token in half;
/// - any other code point is escaped literally (`\ ` is a space, `\,` is a comma),
///   consuming exactly that one code point;
/// - a `\` before a newline is **not** an escape at all (§4.3.4) — `None`.
///
/// The single definition of "how far does this escape reach", so every scanner that walks
/// a CSS value — the whitespace normalizer, the top-level splitter, the value cursor —
/// steps over escapes identically and none of them can mistake an escape's interior for
/// structure.
///
/// A CRLF terminator is taken as the `\r` only, leaving the `\n` outside the escape. That
/// is one code point short of §4.2's preprocessing (which folds CRLF to a single newline),
/// but harmless rather than merely tolerable: the leftover `\n` is then ordinary
/// whitespace, and every consumer of this function normalizes a whitespace run to one
/// space, so `\41<CR><LF>2px` still emits `\41 2px` — the same ident `A2px`, which
/// re-parses identically. The safety comes from that downstream join, not from this arm.
pub(crate) fn escape_len(s: &str, i: usize) -> Option<usize> {
    debug_assert_eq!(s.as_bytes().get(i), Some(&b'\\'));
    let rest = s.get(i + 1..)?;
    let first = rest.chars().next()?;
    if matches!(first, '\n' | '\r' | '\x0C') {
        return None;
    }
    if !first.is_ascii_hexdigit() {
        return Some(1 + first.len_utf8());
    }
    let hex = rest
        .bytes()
        .take(6)
        .take_while(u8::is_ascii_hexdigit)
        .count();
    let terminator = rest[hex..]
        .chars()
        .next()
        .filter(|&c| is_css_whitespace(c))
        .map_or(0, char::len_utf8);
    Some(1 + hex + terminator)
}

/// Trim trailing whitespace, but never the whitespace a CSS **escape** owns.
///
/// `\` followed by whitespace is a valid escape whose escaped code point *is* that
/// whitespace character (CSS Syntax 3 §4.3.4, and §4.3.7 "Consume an escaped code
/// point", whose final branch returns the code point itself). So the trailing space
/// in `width: 50px\ ;` is value *content*, not padding.
///
/// Trimming it strands the backslash, which then escapes whatever follows — the
/// declaration's `;`, a function's `)` — and the result no longer parses. Exactly one
/// whitespace character is therefore kept when an odd-length backslash run precedes
/// it; any further whitespace past the escaped one is ordinary padding and still goes.
///
/// A newline is never kept: `\` + newline is the one shape §4.3.4 excludes, so it is not
/// an escape and the backslash owns nothing. The lexer rejects that input before this is
/// reached, so the arm never fires today; it exists because the rule, not its
/// reachability, is what this function encodes.
///
/// The single definition of this rule. The parser's value/argument spans, the printer's
/// whitespace normalizer and top-level splitter, the selector leaf/pseudo printers, and
/// `url()` trimming all route through it, so they cannot drift.
pub(crate) fn trim_end_preserving_escape(s: &str) -> &str {
    let trimmed = s.trim_end();
    if trimmed.len() == s.len() || !ends_with_open_escape(trimmed) {
        return trimmed;
    }
    // Give the escape back its one payload character — unless that character is a
    // newline, which cannot be one.
    match s[trimmed.len()..].chars().next() {
        Some(c) if is_escapable_whitespace(c) => &s[..trimmed.len() + c.len_utf8()],
        _ => trimmed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_escape_sequences_simple() {
        assert_eq!(decode_escape_sequences(r"\\"), r"\");
        assert_eq!(decode_escape_sequences(r#"\""#), r#"""#);
        assert_eq!(decode_escape_sequences(r"\'"), r"'");
    }

    #[test]
    fn test_decode_escape_sequences_unicode() {
        // Simple ASCII letter (2 hex digits)
        assert_eq!(decode_escape_sequences(r"\41"), "A");

        // Emoji (5 hex digits)
        assert_eq!(decode_escape_sequences(r"\1F4A9"), "💩");

        // With ONE leading zero (6 hex digits - CSS maximum)
        assert_eq!(decode_escape_sequences(r"\01F4A9"), "💩");
    }
}

#[cfg(test)]
mod escape_scan_tests {
    use super::{escape_len, trim_end_preserving_escape};

    #[test]
    fn trim_keeps_an_escaped_space() {
        // The escape's payload survives; ordinary padding past it does not.
        assert_eq!(trim_end_preserving_escape(r"50px\ "), r"50px\ ");
        assert_eq!(trim_end_preserving_escape("50px\\ \t  "), r"50px\ ");
        assert_eq!(trim_end_preserving_escape("a\\\t"), "a\\\t");
    }

    #[test]
    fn trim_drops_ordinary_whitespace() {
        assert_eq!(trim_end_preserving_escape("50px   "), "50px");
        assert_eq!(trim_end_preserving_escape("50px"), "50px");
        // A hex escape's terminator is not a payload — it is trimmable here (whatever
        // follows a trimmed value re-terminates the escape).
        assert_eq!(trim_end_preserving_escape(r"\41 "), r"\41");
    }

    #[test]
    fn trim_counts_the_backslash_run_parity() {
        // Even run = a completed `\\`, so the space after it is ordinary padding.
        assert_eq!(trim_end_preserving_escape(r"a\\ "), r"a\\");
        // Odd run = the last `\` is unescaped and owns the space.
        assert_eq!(trim_end_preserving_escape(r"a\\\ "), r"a\\\ ");
        assert_eq!(trim_end_preserving_escape(r"a\\\\ "), r"a\\\\");
    }

    #[test]
    fn trim_edges() {
        assert_eq!(trim_end_preserving_escape(""), "");
        assert_eq!(trim_end_preserving_escape("   "), "");
        // The backslash run starts at index 0 — nothing precedes it.
        assert_eq!(trim_end_preserving_escape(r"\ "), r"\ ");
        assert_eq!(trim_end_preserving_escape(r"\\ "), r"\\");
        // A lone trailing backslash has no payload to keep.
        assert_eq!(trim_end_preserving_escape(r"a\"), r"a\");
    }

    #[test]
    fn trim_never_keeps_a_newline() {
        // `\` + newline is NOT a valid escape (§4.3.4), so the backslash owns nothing.
        assert_eq!(trim_end_preserving_escape("a\\\n"), "a\\");
        assert_eq!(trim_end_preserving_escape("a\\\r\n"), "a\\");
    }

    #[test]
    fn trim_keeps_a_multi_byte_payload() {
        // A non-breaking space is whitespace to `str::trim_end` and 2 bytes wide; the
        // slice must be cut on its char boundary, not one byte in.
        assert_eq!(trim_end_preserving_escape("a\\\u{a0}"), "a\\\u{a0}");
    }

    #[test]
    fn escape_len_identity_escapes() {
        assert_eq!(escape_len(r"\ x", 0), Some(2)); // `\ `
        assert_eq!(escape_len(r"\,y", 0), Some(2)); // `\,`
        assert_eq!(escape_len(r"\)b", 0), Some(2)); // `\)`
        // A multi-byte escaped code point is consumed whole.
        assert_eq!(escape_len("\\\u{a0}b", 0), Some(3));
    }

    #[test]
    fn escape_len_hex_escapes_swallow_one_terminator() {
        // Up to SIX hex digits, then optionally ONE whitespace terminator.
        assert_eq!(escape_len(r"\41 2px", 0), Some(4)); // `\41 ` — the space belongs to it
        assert_eq!(escape_len(r"\41b", 0), Some(4)); // `\41b` — `b` is a hex digit
        assert_eq!(escape_len(r"\41z", 0), Some(3)); // `\41` — `z` is not
        assert_eq!(escape_len(r"\0000411", 0), Some(7)); // six digits max, then `1` stops it
        assert_eq!(escape_len(r"\41  x", 0), Some(4)); // only the FIRST space is the terminator
        // A newline can terminate a hex escape (it is CSS whitespace), unlike being a
        // payload.
        assert_eq!(escape_len("\\41\n", 0), Some(4));
    }

    #[test]
    fn escape_len_rejects_a_non_escape() {
        assert_eq!(escape_len("\\\n", 0), None); // `\` + newline is not an escape
        assert_eq!(escape_len("\\\r", 0), None);
        assert_eq!(escape_len(r"\", 0), None); // a lone trailing backslash
    }
}
