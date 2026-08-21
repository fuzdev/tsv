// Low-level byte scanning utilities for parser lookahead
//
// These are generic helpers for scanning raw bytes, used by both expression
// parsing (arrow function detection) and type parsing (index signature detection).

use std::borrow::Cow;

use crate::lexer::{is_es_line_terminator, is_es_line_terminator_at, is_es_whitespace};

/// 256-entry lookup table for the ASCII half of the lookahead whitespace class —
/// `<SP>`, `<TAB>`, `<VT>`, `<FF>` from `WhiteSpace` plus `<LF>`, `<CR>` from
/// `LineTerminator`. Const-derived from the same `matches!` the test below
/// re-spells, so the table and its documented membership can't drift.
const LOOKAHEAD_WS_LUT: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < 256 {
        let b = i as u8;
        t[i] = matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C);
        i += 1;
    }
    t
};

/// Skip the whitespace run at `pos`, returning the position of the next
/// significant byte.
///
/// The class is ECMAScript `WhiteSpace` ∪ `LineTerminator` — equivalently what
/// JS's `\s` matches, the spec being explicit that "line terminators are included
/// in the set of white space code points that are matched by the `\s` class in
/// regular expressions". That is the same question the lexer's own
/// [`Lexer::skip_whitespace`](crate::lexer::Lexer) answers, and this scan must
/// agree with it by construction: a lookahead that stops **earlier** than the
/// lexer mis-reads the shape it is classifying and rejects input the canonical
/// parser accepts. So the two share one spelling of the productions —
/// [`is_es_whitespace`] and [`is_es_line_terminator`] — rather than restating them.
///
/// ⚠️ Crossing a line terminator here is correct and is **not** how a
/// `[no LineTerminator here]` restriction is enforced. These predicates only
/// locate the next token; the restriction is enforced downstream off the lexer's
/// `had_line_terminator` (see `Parser::expect_arrow`), which is why `(a)\n=> a`
/// has always been rejected even though `\n` was in the old, narrower class.
/// Narrowing the class to make some construct reject "works" only by accident and
/// takes every legal whitespace character down with it.
///
/// The ASCII path is a table lookup (the idiom the identifier classes below use);
/// only a byte ≥ `0x80` — which ends the run under any classification, so the
/// branch is off the hot path — decodes a `char` to test the non-ASCII members.
#[inline]
pub(super) fn skip_whitespace(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() {
        let b = bytes[pos];
        if LOOKAHEAD_WS_LUT[b as usize] {
            pos += 1;
        } else if b >= 0x80 {
            // A lead byte here may begin NBSP/ZWNBSP/a `Zs`/LS/PS, or an ordinary
            // non-ASCII token character. Only a decode can tell them apart, and a
            // partial decode must not advance — hence stepping by `len_utf8`.
            match decode_char(bytes, pos) {
                Some(c) if is_es_whitespace(c) || is_es_line_terminator(c) => pos += c.len_utf8(),
                _ => break,
            }
        } else {
            break;
        }
    }
    pos
}

/// The `char` starting at `pos`, or `None` when the bytes there are not a
/// complete UTF-8 sequence.
///
/// The scans work on `&[u8]` taken from a `&str`, so a lead byte is always
/// followed by its continuation bytes; the fallible form exists so a malformed
/// slice ends the run instead of panicking or advancing onto a continuation byte.
#[inline]
fn decode_char(bytes: &[u8], pos: usize) -> Option<char> {
    let width = match bytes[pos] {
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => return None,
    };
    let end = pos.checked_add(width)?;
    std::str::from_utf8(bytes.get(pos..end)?)
        .ok()?
        .chars()
        .next()
}

/// Skip a line comment (// ...), returning position after the newline
/// Assumes `pos` is at the first `/`
#[inline]
pub(super) fn skip_line_comment(bytes: &[u8], mut pos: usize) -> usize {
    // Skip //
    pos += 2;
    // Read until a LineTerminator or EOF — U+2028/U+2029 end a line comment just
    // as LF/CR do, per the spec. The terminator is not consumed.
    while pos < bytes.len() && !is_es_line_terminator_at(bytes, pos) {
        pos += 1;
    }
    pos
}

/// Skip a block comment (/* ... */), returning position after the closing */
/// Assumes `pos` is at the first `/`
#[inline]
pub(super) fn skip_block_comment(bytes: &[u8], mut pos: usize) -> usize {
    // Skip /*
    pos += 2;
    while pos + 1 < bytes.len() {
        if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
            return pos + 2;
        }
        pos += 1;
    }
    pos
}

/// Skip whitespace and comments, returning new position
#[inline]
pub(super) fn skip_whitespace_and_comments(bytes: &[u8], mut pos: usize) -> usize {
    loop {
        let start = pos;
        pos = skip_whitespace(bytes, pos);
        // Check for comments
        if pos + 1 < bytes.len() && bytes[pos] == b'/' {
            if bytes[pos + 1] == b'/' {
                pos = skip_line_comment(bytes, pos);
            } else if bytes[pos + 1] == b'*' {
                pos = skip_block_comment(bytes, pos);
            } else {
                break;
            }
        } else {
            break;
        }
        // Continue loop to handle whitespace after comment
        if pos == start {
            break;
        }
    }
    pos
}

/// 256-entry lookup tables for the lookahead identifier classes. Each entry is computed
/// from the same predicate the byte tests below expand to, so the tables are exact — a
/// lookup replaces the OR-chain with one L1 load.
///
/// These are the **lookahead** classes, deliberately *not* the lexer's
/// (`lexer::core::ID_START_LUT` / `ID_CONTINUE_LUT`): both include every byte `> 127`,
/// since a lookahead only needs to step over a multi-byte UTF-8 sequence, not validate
/// it. That extra term is also why the table beats the chain by more here than it does
/// in the lexer — `> 127` cannot fold into a 64-bit bitmask test, so LLVM emits the
/// full arithmetic chain, and it orders the common case (a letter) *last*, behind the
/// `> 127`, `$`, `_` and digit tests.
const LOOKAHEAD_ID_START_LUT: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < 256 {
        let b = i as u8;
        t[i] = b.is_ascii_alphabetic() || b == b'_' || b == b'$' || b > 127;
        i += 1;
    }
    t
};
const LOOKAHEAD_ID_CONTINUE_LUT: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < 256 {
        let b = i as u8;
        t[i] = b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b > 127;
        i += 1;
    }
    t
};

/// Check if a byte can start an identifier (letter, underscore, dollar sign, or non-ASCII)
///
/// Non-ASCII bytes (> 127) are included for lookahead purposes - they're part of multi-byte
/// UTF-8 sequences that are likely unicode identifier chars. The actual lexer uses proper
/// `ID_Start` validation (`lexer::ident::is_id_start`) on the decoded char.
#[inline]
pub(super) const fn is_identifier_start(b: u8) -> bool {
    LOOKAHEAD_ID_START_LUT[b as usize]
}

/// Check if a byte can continue an identifier (alphanumeric, underscore, dollar sign, or non-ASCII)
///
/// Non-ASCII bytes (> 127) are included for lookahead purposes - they're part of multi-byte
/// UTF-8 sequences that are likely unicode identifier chars. The actual lexer uses proper
/// `ID_Continue` validation (`lexer::ident::is_id_continue`) on the decoded char.
#[inline]
pub(super) const fn is_identifier_continue(b: u8) -> bool {
    LOOKAHEAD_ID_CONTINUE_LUT[b as usize]
}

/// Check if `word` sits at `pos` as a whole identifier, not as the prefix of a longer
/// one — `is_word_at(b"extendsFoo", 0, b"extends")` is false.
///
/// A bare `starts_with` is the trap this exists to close: the byte after the word decides
/// whether a lookahead is looking at a keyword or at an ordinary identifier that happens
/// to share its opening bytes.
#[inline]
pub(super) fn is_word_at(bytes: &[u8], pos: usize, word: &[u8]) -> bool {
    bytes[pos..].starts_with(word)
        && bytes
            .get(pos + word.len())
            .is_none_or(|&b| !is_identifier_continue(b))
}

/// Skip a numeric literal, returning the position after it — or `pos` unchanged when no
/// literal starts there. Handles a leading `-` (the only sign a literal *type* may carry —
/// `A[+1]` is not one), radix prefixes (`0x`), separators (`1_000`), a BigInt `n`, and an
/// exponent whose own sign (`1e-3`) would otherwise end the scan.
///
/// Deliberately loose about a literal's INTERIOR — it accepts more than the grammar does.
/// Callers use it to find where a literal ENDS, then check what follows, so over-consuming
/// a malformed literal only makes that follow-check fail. The FIRST character is the one
/// place it is strict, and must stay so: `-b` would otherwise scan as a literal ending at
/// the very `]` a real one ends at, leaving the follow-check no way to tell a negated
/// identifier from a negative number.
#[inline]
pub(super) fn skip_numeric_literal(bytes: &[u8], pos: usize) -> usize {
    let mut cursor = pos;
    if bytes.get(cursor) == Some(&b'-') {
        cursor += 1;
    }
    // A literal starts with a digit, or a bare `.` for `-.5`. Anything else (`-b`, `-$b`)
    // is a unary negation; reporting "nothing skipped" is how the caller learns that.
    if !matches!(bytes.get(cursor), Some(b'0'..=b'9' | b'.')) {
        return pos;
    }
    while cursor < bytes.len() {
        let b = bytes[cursor];
        if b.is_ascii_alphanumeric() || b == b'.' || b == b'_' {
            // An exponent's sign belongs to the literal, not to a following operator.
            if matches!(b, b'e' | b'E') && matches!(bytes.get(cursor + 1), Some(b'-' | b'+')) {
                cursor += 1;
            }
            cursor += 1;
        } else {
            break;
        }
    }
    cursor
}

/// Skip an identifier, returning position after the identifier
/// Assumes `pos` is at the start of an identifier
#[inline]
pub(super) fn skip_identifier(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && is_identifier_continue(bytes[pos]) {
        pos += 1;
    }
    pos
}

/// Parse a JS number literal (hex, binary, octal, scientific, BigInt)
/// Returns f64 (BigInt suffix 'n' is ignored for value, preserved in raw)
///
/// Note: Precision loss for large integers (>2^52) matches JS behavior.
pub(crate) fn parse_number_literal(raw: &str) -> Result<f64, std::num::ParseFloatError> {
    // Numeric separators (`_`) are uncommon; only allocate to strip them when
    // they're actually present. The common literal (`42`, `0xff`, `3.14`) carries
    // no separator and borrows the source slice directly — no per-literal alloc.
    let clean: Cow<'_, str> = if raw.as_bytes().contains(&b'_') {
        Cow::Owned(raw.chars().filter(|&c| c != '_').collect())
    } else {
        Cow::Borrowed(raw)
    };

    // Strip BigInt suffix
    let clean = clean.strip_suffix('n').unwrap_or(&clean);

    if clean.len() >= 2 {
        let prefix = &clean[..2];
        let digits = &clean[2..];
        match prefix {
            // Hex: 0xff
            "0x" | "0X" => return Ok(parse_radix_f64(digits, 16)),
            // Binary: 0b1010
            "0b" | "0B" => return Ok(parse_radix_f64(digits, 2)),
            // Octal: 0o77
            "0o" | "0O" => return Ok(parse_radix_f64(digits, 8)),
            _ => {}
        }
    }

    // Regular decimal (including scientific notation)
    clean.parse::<f64>()
}

/// Fold radix digits into an `f64`, rounding at each digit — exactly acorn's
/// `readInt` accumulation, which past 2^53 can land one ulp below the
/// correctly rounded value (e.g. `0x47874750d3a412a2`); matching acorn is the
/// conformance target, so don't "fix" this with a u128 cast. An integer-typed
/// accumulator would also overflow to 0 on long literals like
/// `0x123abcdef456ABCDEF`.
fn parse_radix_f64(digits: &str, radix: u32) -> f64 {
    digits.chars().fold(0f64, |acc, c| {
        // These radixes are all powers of two, so `acc * radix` only rescales the
        // exponent and is exact — making this byte-identical to `mul_add`. Keep the
        // explicit multiply-and-add as a faithful transcription of acorn's
        // `total = total * radix + val` rather than fusing it.
        #[allow(clippy::suboptimal_flops)]
        {
            acc * f64::from(radix) + f64::from(c.to_digit(radix).unwrap_or(0))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The lookahead scanners decide identifier bytes from the `[bool; 256]` tables
    // rather than the OR-chain they were written as. The tables are const-derived
    // from that chain, so this grades the lookup against a plain re-spelling of the
    // predicate — the guard against a table and its documented membership drifting.
    #[test]
    fn lookahead_id_luts_match_the_predicates_they_replace() {
        for b in 0..=u8::MAX {
            assert_eq!(
                is_identifier_start(b),
                b.is_ascii_alphabetic() || b == b'_' || b == b'$' || b > 127,
                "id_start mismatch at byte {b:#x}"
            );
            assert_eq!(
                is_identifier_continue(b),
                b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b > 127,
                "id_continue mismatch at byte {b:#x}"
            );
        }
    }

    /// The ASCII half of the whitespace class, graded the same way: the table
    /// against a plain re-spelling of the membership its doc claims.
    #[test]
    fn lookahead_ws_lut_matches_the_predicate_it_replaces() {
        for b in 0..=u8::MAX {
            assert_eq!(
                LOOKAHEAD_WS_LUT[b as usize],
                matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C),
                "ws lut mismatch at byte {b:#x}"
            );
        }
    }

    /// `skip_whitespace` must cross exactly ECMAScript `WhiteSpace` ∪
    /// `LineTerminator` — JS's `\s` — at **every** code point, not just the ones
    /// some fixture happens to contain. The reference set is spelled out from the
    /// spec's two tables rather than reusing the predicates under test, so this
    /// grades the class instead of restating it.
    ///
    /// The exact membership is the whole point of the test: the class was once
    /// `[ \t\n\r]`, which rejected `(a)<NBSP>=> a` — legal ECMAScript the
    /// canonical parser accepts. Both over- and under-matching are caught, so
    /// reaching for Rust's `char::is_whitespace` (which drops U+FEFF and adds
    /// U+0085) fails here rather than in a corpus months later.
    #[test]
    fn skip_whitespace_crosses_exactly_the_js_s_class() {
        for cp in 0..=0x10ffff_u32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            // ES `WhiteSpace` (table-white-space-code-points) ∪ `LineTerminator`
            // (table-line-terminator-code-points), spelled from the spec.
            let expected = matches!(
                cp,
                0x0009 // <TAB>
                | 0x000B // <VT>
                | 0x000C // <FF>
                | 0xFEFF // <ZWNBSP>
                | 0x0020 | 0x00A0 | 0x1680 | 0x2000
                    ..=0x200A | 0x202F | 0x205F | 0x3000 // <USP>: Zs
                | 0x000A | 0x000D | 0x2028 | 0x2029 // LineTerminator
            );
            let mut buf = [0u8; 4];
            let encoded = c.encode_utf8(&mut buf).len();
            let bytes = &buf[..encoded];
            let crossed = skip_whitespace(bytes, 0) == encoded;
            assert_eq!(
                crossed, expected,
                "U+{cp:04X} is on the wrong side of `\\s`"
            );
        }
    }

    /// A whitespace character glued to a following token must leave the cursor on
    /// that token, never inside it — the failure a byte-wise `pos += 1` over a
    /// multi-byte character would produce, landing on a continuation byte.
    #[test]
    fn skip_whitespace_lands_on_a_character_boundary() {
        for s in [
            "\u{a0}=>",
            "\u{feff}=>",
            "\u{3000}=>",
            "\u{2028}=>",
            "\u{b}=>",
            " \u{a0}\t=>",
        ] {
            let bytes = s.as_bytes();
            let pos = skip_whitespace(bytes, 0);
            assert!(
                s.is_char_boundary(pos),
                "{s:?} stopped mid-character at {pos}"
            );
            assert_eq!(&s[pos..], "=>", "{s:?} did not stop at the token");
        }
    }

    /// A non-ASCII character that is *not* whitespace ends the run — including
    /// U+0085 (`<NEL>`), which Rust calls whitespace and ECMAScript does not, and
    /// an ordinary identifier character sharing NBSP's `0xC2` lead byte.
    #[test]
    fn skip_whitespace_stops_at_non_es_whitespace() {
        for s in ["\u{85}x", "\u{b5}x", "\u{180e}x", "x"] {
            assert_eq!(
                skip_whitespace(s.as_bytes(), 0),
                0,
                "{s:?} should not be crossed"
            );
        }
    }
}
