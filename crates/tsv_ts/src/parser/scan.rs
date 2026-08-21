// Low-level byte scanning utilities for parser lookahead
//
// These are generic helpers for scanning raw bytes, used by both expression
// parsing (arrow function detection) and type parsing (index signature detection).

use std::borrow::Cow;

use crate::lexer::{is_es_line_terminator, is_es_line_terminator_at, is_es_whitespace};

/// 256-entry lookup table for the lookahead whitespace class. `In` is its ASCII half —
/// `<SP>`, `<TAB>`, `<VT>`, `<FF>` from `WhiteSpace` plus `<LF>`, `<CR>` from
/// `LineTerminator`; `Maybe` is the five [`is_es_whitespace_lead`] bytes, which only a
/// decode can settle. Const-derived from the same `matches!` the test below re-spells,
/// so the table and its documented membership can't drift.
///
/// One table rather than a `bool` half plus a separate lead test, so
/// [`skip_whitespace`] and [`skip_identifier`] ask their byte the same way: one load,
/// one three-way verdict. The two scans classify the same bytes for opposite purposes —
/// what ends a name is what a gap is made of — so a byte that is `Maybe` here is `Maybe`
/// there, by construction.
const LOOKAHEAD_WS_LUT: [ScanByte; 256] = {
    let mut t = [ScanByte::Out; 256];
    let mut i = 0;
    while i < 256 {
        let b = i as u8;
        t[i] = if matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C) {
            ScanByte::In
        } else if is_es_whitespace_lead(b) {
            ScanByte::Maybe
        } else {
            ScanByte::Out
        };
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
/// The scan is a table lookup (the same idiom, and now the same table shape, as the
/// identifier classes below); a decode is reached only at the five
/// [`is_es_whitespace_lead`] bytes, and every other byte `>= 0x80` ends the run without
/// one, so the branch is off the hot path.
#[inline]
pub(super) fn skip_whitespace(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() {
        match LOOKAHEAD_WS_LUT[bytes[pos] as usize] {
            ScanByte::In => pos += 1,
            // One of the five leads that can begin NBSP/ZWNBSP/a `Zs`/LS/PS — but each
            // also leads ordinary non-ASCII token characters, so only a decode tells
            // them apart, and a partial decode must not advance: step by the character's
            // own width, which is what the shared helper returns.
            ScanByte::Maybe => match non_ascii_es_whitespace_len_at(bytes, pos) {
                0 => break,
                len => pos += len,
            },
            ScanByte::Out => break,
        }
    }
    pos
}

/// The UTF-8 length of the **non-ASCII** ECMAScript `WhiteSpace` ∪ `LineTerminator`
/// character beginning at `bytes[pos]`, or `0` when none does.
///
/// The one decode both non-ASCII askers share — [`skip_whitespace`], which needs the
/// width to advance, and [`id_class_holds`], which needs only the verdict — so the class
/// keeps exactly one spelling ([`is_es_whitespace`] / [`is_es_line_terminator`], the
/// lexer's own). The ASCII members are answered by table lookup instead
/// ([`LOOKAHEAD_WS_LUT`], and by their `ScanByte::Out` entries in the identifier tables),
/// which is why this never sees an ASCII byte: [`decode_char`] returns `None` for one.
#[inline]
fn non_ascii_es_whitespace_len_at(bytes: &[u8], pos: usize) -> usize {
    match decode_char(bytes, pos) {
        Some(c) if is_es_whitespace(c) || is_es_line_terminator(c) => c.len_utf8(),
        _ => 0,
    }
}

/// The **multi-byte** `char` starting at `pos`, or `None` when no complete one does —
/// including for an ASCII byte, which has no multi-byte lead to match.
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
/// (`lexer::core::ID_START_LUT` / `ID_CONTINUE_LUT`): a lookahead only needs to step
/// over a multi-byte UTF-8 sequence, not validate it, so almost every byte `> 127`
/// counts as an identifier character here where the lexer would decode and test
/// `ID_Start` / `ID_Continue`. That extra term is also why the table beats the chain by
/// more here than it does in the lexer — `> 127` cannot fold into a 64-bit bitmask test,
/// so LLVM emits the full arithmetic chain, and it orders the common case (a letter)
/// *last*, behind the `> 127`, `$`, `_` and digit tests.
///
/// ⚠️ **"Almost" every byte `> 127`, and the exception is the whole reason these are
/// tables of [`ScanByte`] rather than of `bool`.** A lookahead asks the identifier classes
/// two different questions — *step over this name* and *does the word end here?* — and
/// the second is a WORD BOUNDARY, where reading a non-ASCII whitespace character as
/// "the identifier continues" merges a keyword into its neighbour. A byte test cannot
/// settle it: `0xC2` leads both `<NBSP>` (`c2 a0`) and ordinary identifier characters
/// (`µ` is `c2 b5`), so the five lead bytes that can begin ECMAScript whitespace are
/// entered as [`ScanByte::Maybe`] and resolved by [`non_ascii_es_whitespace_len_at`]
/// against the same productions [`skip_whitespace`] crosses.
const LOOKAHEAD_ID_START_LUT: [ScanByte; 256] = build_id_lut(false);
const LOOKAHEAD_ID_CONTINUE_LUT: [ScanByte; 256] = build_id_lut(true);

/// What a byte contributes to a lookahead scan — of an identifier, or of a whitespace
/// run. An enum rather than a `u8` sentinel so the verdict arms are exhaustive: a fourth
/// state could not be added and silently fall into someone's `_`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ScanByte {
    /// The run ends at this byte.
    Out,
    /// This byte is part of the run, unconditionally.
    In,
    /// A UTF-8 lead byte that begins EITHER an ECMAScript whitespace character or an
    /// ordinary non-ASCII identifier character — only the bytes after it can say which.
    Maybe,
}

/// Build one of the two tables above. `continuing` picks the `ID_Continue` membership
/// (digits admitted) over the `ID_Start` one.
const fn build_id_lut(continuing: bool) -> [ScanByte; 256] {
    let mut t = [ScanByte::Out; 256];
    let mut i = 0;
    while i < 256 {
        let b = i as u8;
        let ascii_member = b == b'_'
            || b == b'$'
            || if continuing {
                b.is_ascii_alphanumeric()
            } else {
                b.is_ascii_alphabetic()
            };
        t[i] = if is_es_whitespace_lead(b) {
            ScanByte::Maybe
        } else if b > 127 || ascii_member {
            ScanByte::In
        } else {
            ScanByte::Out
        };
        i += 1;
    }
    t
}

/// The five UTF-8 lead bytes that can begin a non-ASCII ECMAScript `WhiteSpace` or
/// `LineTerminator` character: `c2` (U+00A0 `<NBSP>`), `e1` (U+1680), `e2`
/// (U+2000..U+200A, U+2028 `<LS>`, U+2029 `<PS>`, U+202F, U+205F), `e3` (U+3000) and
/// `ef` (U+FEFF `<ZWNBSP>`).
///
/// A conservative *filter*, never the answer: every one of them also leads ordinary
/// identifier characters, which is why a [`ScanByte::Maybe`] entry is resolved by a decode
/// rather than by this. Kept exact by `es_whitespace_leads_cover_the_class` below, which walks
/// the whole code-point space so a widened whitespace class cannot leave a lead behind.
const fn is_es_whitespace_lead(b: u8) -> bool {
    matches!(b, 0xC2 | 0xE1 | 0xE2 | 0xE3 | 0xEF)
}

/// Whether an identifier character begins at `bytes[pos]` — the lookahead's `ID_Start`
/// class, asked of a POSITION rather than of a byte.
///
/// `false` past the end of `bytes`: nothing starts where there is nothing.
#[inline]
pub(super) fn identifier_starts_at(bytes: &[u8], pos: usize) -> bool {
    id_class_holds(&LOOKAHEAD_ID_START_LUT, bytes, pos)
}

/// Whether an identifier character continues at `bytes[pos]` — the lookahead's
/// `ID_Continue` class, asked of a POSITION rather than of a byte.
///
/// ⚠️ The position is load-bearing, and asking this of a bare byte is the bug this
/// signature exists to prevent: at a **word boundary** (`is_word_at`, the `in` of a
/// mapped type) a non-ASCII whitespace character must END the word, while the `0xC2`
/// that leads `<NBSP>` also leads `µ`. `false` past the end of `bytes`.
///
/// Private on purpose: the word-boundary question has exactly one caller-facing
/// spelling, [`is_word_at`]. A lookahead that reaches for the raw class instead is
/// re-deriving that boundary, which is what let this one go wrong.
#[inline]
fn identifier_continues_at(bytes: &[u8], pos: usize) -> bool {
    id_class_holds(&LOOKAHEAD_ID_CONTINUE_LUT, bytes, pos)
}

/// The shared body of the two predicates above: one table load, and a decode only at the
/// [`ScanByte::Maybe`] leads.
#[inline]
fn id_class_holds(lut: &[ScanByte; 256], bytes: &[u8], pos: usize) -> bool {
    match bytes.get(pos) {
        Some(&b) => match lut[b as usize] {
            ScanByte::In => true,
            ScanByte::Maybe => non_ascii_es_whitespace_len_at(bytes, pos) == 0,
            ScanByte::Out => false,
        },
        None => false,
    }
}

/// Check if `word` sits at `pos` as a whole identifier, not as the prefix of a longer
/// one — `is_word_at(b"extendsFoo", 0, b"extends")` is false.
///
/// A bare `starts_with` is the trap this exists to close: what follows the word decides
/// whether a lookahead is looking at a keyword or at an ordinary identifier that happens
/// to share its opening bytes. That "what follows" is a *character*, not a byte — a
/// keyword followed by `<NBSP>` is still the keyword — so the boundary test goes through
/// [`identifier_continues_at`].
#[inline]
pub(super) fn is_word_at(bytes: &[u8], pos: usize, word: &[u8]) -> bool {
    bytes[pos..].starts_with(word) && !identifier_continues_at(bytes, pos + word.len())
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

/// Skip an identifier, returning the position after it. Assumes `pos` is at the start of
/// one.
///
/// The loop is the lookahead's hot path, so it reads the table directly rather than
/// calling [`identifier_continues_at`]: the fast arm stays one L1 load and one compare
/// per byte, and the decode is reached only at the [`ScanByte::Maybe`] leads — in practice
/// once per call, on the byte that ends the run.
#[inline]
pub(super) fn skip_identifier(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() {
        match LOOKAHEAD_ID_CONTINUE_LUT[bytes[pos] as usize] {
            ScanByte::In => pos += 1,
            ScanByte::Maybe if non_ascii_es_whitespace_len_at(bytes, pos) == 0 => pos += 1,
            _ => break,
        }
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

    /// Encode `c` on its own, plus the trailing bytes some callers need past it.
    fn utf8(c: char) -> Vec<u8> {
        let mut buf = [0u8; 4];
        c.encode_utf8(&mut buf).as_bytes().to_vec()
    }

    /// ECMAScript `WhiteSpace` ∪ `LineTerminator`, spelled from the spec's two tables
    /// rather than from the predicates under test — the reference the whole file grades
    /// against, so a widening has to be made in two places to pass.
    fn in_js_s_class(cp: u32) -> bool {
        matches!(
            cp,
            0x0009 // <TAB>
            | 0x000B // <VT>
            | 0x000C // <FF>
            | 0xFEFF // <ZWNBSP>
            | 0x0020 | 0x00A0 | 0x1680 | 0x2000
                ..=0x200A | 0x202F | 0x205F | 0x3000 // <USP>: Zs
            | 0x000A | 0x000D | 0x2028 | 0x2029 // LineTerminator
        )
    }

    // The lookahead scanners decide identifier bytes from the `[u8; 256]` tables rather
    // than the OR-chain they were written as. The tables are const-derived from that
    // chain, so this grades the lookup against a plain re-spelling of the predicate —
    // the guard against a table and its documented membership drifting. The `Maybe`
    // leads are excluded here and graded per code point below; a byte test cannot
    // answer for them, which is the point of the third state.
    #[test]
    fn lookahead_id_luts_match_the_predicates_they_replace() {
        for b in 0..=u8::MAX {
            if is_es_whitespace_lead(b) {
                assert_eq!(LOOKAHEAD_ID_START_LUT[b as usize], ScanByte::Maybe);
                assert_eq!(LOOKAHEAD_ID_CONTINUE_LUT[b as usize], ScanByte::Maybe);
                continue;
            }
            assert_eq!(
                LOOKAHEAD_ID_START_LUT[b as usize] == ScanByte::In,
                b.is_ascii_alphabetic() || b == b'_' || b == b'$' || b > 127,
                "id_start mismatch at byte {b:#x}"
            );
            assert_eq!(
                LOOKAHEAD_ID_CONTINUE_LUT[b as usize] == ScanByte::In,
                b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b > 127,
                "id_continue mismatch at byte {b:#x}"
            );
        }
    }

    /// `is_es_whitespace_lead` is a *filter* in front of the decode, so it must cover
    /// the class exactly: a member left out is a whitespace character the identifier
    /// scans would swallow (the bug this file's `Maybe` state exists to fix), and a
    /// non-member left in is a decode on the hot path buying nothing.
    #[test]
    fn es_whitespace_leads_cover_the_class() {
        let mut expected = [false; 256];
        for cp in 0..=0x10ffff_u32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            if in_js_s_class(cp) && cp > 0x7f {
                expected[utf8(c)[0] as usize] = true;
            }
        }
        for b in 0..=u8::MAX {
            assert_eq!(
                is_es_whitespace_lead(b),
                expected[b as usize],
                "whitespace-lead filter is wrong at byte {b:#x}"
            );
        }
    }

    /// The word-BOUNDARY claim, graded at every code point rather than at the handful a
    /// fixture can reach: an identifier ends at exactly the ECMAScript whitespace and
    /// line-terminator characters, and at nothing else the lexer would accept inside a
    /// name.
    ///
    /// Both directions matter and both have bitten. Under-matching was the live bug —
    /// `0xC2` was read as "the identifier continues", so `[K in<NBSP>keyof T]` and
    /// `f<T extends<NBSP>string ? 1 : 2>(1)` were rejected and
    /// `f<new<NBSP>(v: T) => R>(1)` was silently reprinted as a comparison. Over-matching
    /// is the failure the naive fix produces: excluding the `0xC2` lead wholesale would
    /// end an identifier inside `µ` (`c2 b5`), so every real Unicode name sharing a
    /// whitespace lead byte is checked against the lexer's own `ID_Continue`.
    #[test]
    fn identifier_scan_ends_at_exactly_the_js_s_class() {
        for cp in 0..=0x10ffff_u32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            let bytes = utf8(c);
            let is_space = in_js_s_class(cp);

            // A whitespace character never continues (or starts) an identifier…
            if is_space {
                assert!(
                    !identifier_continues_at(&bytes, 0),
                    "U+{cp:04X} must end an identifier"
                );
                assert!(
                    !identifier_starts_at(&bytes, 0),
                    "U+{cp:04X} must not start an identifier"
                );
                assert_eq!(
                    skip_identifier(&bytes, 0),
                    0,
                    "U+{cp:04X} must not be skipped as an identifier character"
                );
                continue;
            }

            // …and every character the LEXER admits inside a name still does, which is
            // what keeps the whitespace-lead filter from eating real identifiers.
            if crate::lexer::ident::is_id_continue(c) {
                assert!(
                    identifier_continues_at(&bytes, 0),
                    "U+{cp:04X} is `ID_Continue` and must not end an identifier"
                );
                assert_eq!(
                    skip_identifier(&bytes, 0),
                    bytes.len(),
                    "U+{cp:04X} is `ID_Continue` and must be skipped whole"
                );
            }
            if crate::lexer::ident::is_id_start(c) {
                assert!(
                    identifier_starts_at(&bytes, 0),
                    "U+{cp:04X} is `ID_Start` and must start an identifier"
                );
            }
        }
    }

    /// The same class, asked through [`is_word_at`] — the shape that decides whether a
    /// `<` opens type arguments. A keyword followed by any whitespace character is still
    /// that keyword; a keyword followed by any `ID_Continue` character is not.
    #[test]
    fn is_word_at_ends_the_word_at_exactly_the_js_s_class() {
        for cp in 0..=0x10ffff_u32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            let mut bytes = b"extends".to_vec();
            bytes.extend_from_slice(&utf8(c));
            if in_js_s_class(cp) {
                assert!(
                    is_word_at(&bytes, 0, b"extends"),
                    "`extends` followed by U+{cp:04X} is still the keyword"
                );
            } else if crate::lexer::ident::is_id_continue(c) {
                assert!(
                    !is_word_at(&bytes, 0, b"extends"),
                    "`extends` followed by U+{cp:04X} is one longer identifier"
                );
            }
        }
        // The end of input is a boundary too.
        assert!(is_word_at(b"extends", 0, b"extends"));
    }

    /// A whitespace character glued to a keyword must leave the scan positioned on the
    /// *next* token, not inside the character — the composition every caller relies on
    /// (`skip_identifier` then `skip_whitespace_and_comments`) and the one a byte-wise
    /// step would break.
    #[test]
    fn identifier_scan_hands_off_to_the_whitespace_scan() {
        for cp in 0..=0x10ffff_u32 {
            if !in_js_s_class(cp) {
                continue;
            }
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            let s = format!("in{c}keyof");
            let bytes = s.as_bytes();
            let after_word = skip_identifier(bytes, 0);
            assert_eq!(after_word, 2, "U+{cp:04X} was absorbed into `in`");
            let after_gap = skip_whitespace(bytes, after_word);
            assert_eq!(
                &s[after_gap..],
                "keyof",
                "U+{cp:04X} left the cursor off the next token"
            );
        }
    }

    /// The whitespace table, graded the same way as the identifier ones: each entry
    /// against a plain re-spelling of the membership its doc claims. The `Maybe` leads
    /// are the same five bytes the identifier tables carry — a byte test cannot answer
    /// for them, which `identifier_scan_hands_off_to_the_whitespace_scan` grades per
    /// code point.
    #[test]
    fn lookahead_ws_lut_matches_the_predicate_it_replaces() {
        for b in 0..=u8::MAX {
            let expected = if matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C) {
                ScanByte::In
            } else if is_es_whitespace_lead(b) {
                ScanByte::Maybe
            } else {
                ScanByte::Out
            };
            assert_eq!(
                LOOKAHEAD_WS_LUT[b as usize], expected,
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
