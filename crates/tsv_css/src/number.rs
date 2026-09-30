// CSS number grammar — the single source of truth for how a numeric token is
// scanned, shared by the lexer (token spans), the parser (dimension classification),
// and the printer (value/prelude normalization). Pure functions over `&str`;
// no allocation, no AST. Formatting (leading-zero/trailing-zero normalization)
// lives in `printer::value_normalization`, not here — this module is grammar only.

/// Can `ch` continue a dimension unit / CSS identifier?
///
/// Used to decide whether a trailing `.` belongs to the number: a unit char
/// after the dot (`1.px`, `1.png`) means the dot is not a number terminator.
///
/// Every non-ASCII char continues one, so the question is ASCII-only and never needs
/// `char::is_alphabetic`'s Unicode table ([`continues_unit_byte`] asks it of a UTF-8
/// lead byte).
#[inline]
pub(crate) fn continues_unit(ch: char) -> bool {
    !ch.is_ascii() || continues_unit_byte(ch as u8)
}

/// [`continues_unit`] asked of the first byte of the text after the dot — exact, since
/// every byte at or above 0x80 leads (or continues) a non-ASCII char.
#[inline]
const fn continues_unit_byte(b: u8) -> bool {
    b >= 0x80 || b.is_ascii_alphabetic() || matches!(b, b'_' | b'-' | b'\\')
}

/// Byte length of a scientific-notation exponent at the start of `s`
/// (`[eE][+-]?\d+`), or 0 if none. Tells `1e10` (exponent) apart from `1em`
/// (a unit): an exponent requires a digit after the optional sign.
pub(crate) fn exponent_len(s: &str) -> usize {
    let bytes = s.as_bytes();
    if bytes.is_empty() || (bytes[0] != b'e' && bytes[0] != b'E') {
        return 0;
    }
    let mut i = 1;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    if i >= bytes.len() || !bytes[i].is_ascii_digit() {
        return 0;
    }
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    i
}

/// Does `bytes` open with a number's DIGITS — a digit, or a `.` then a digit? The head
/// [`number_part_len`] accepts past its optional sign, and all of css-syntax-3 §4.3.10's
/// "Would start a number" past the sign: every longer shape (an exponent, a trailing `.`)
/// extends a prefix that already qualified.
#[inline]
fn opens_number_digits(bytes: &[u8]) -> bool {
    match bytes.first() {
        Some(b) if b.is_ascii_digit() => true,
        Some(b'.') => bytes.get(1).is_some_and(u8::is_ascii_digit),
        _ => false,
    }
}

/// Does `s` open with a number — `number_part_len(s) > 0`, answered from its first
/// three bytes at most (an optional sign, then [`opens_number_digits`]) rather than by
/// scanning the number to its end. For the callers that need only the classification
/// (is this leaf a dimension? can an operator stand here?); the length stays
/// [`number_part_len`]'s.
#[inline]
pub(crate) fn starts_number(s: &str) -> bool {
    let bytes = s.as_bytes();
    let sign = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let answer = opens_number_digits(&bytes[sign..]);
    debug_assert_eq!(answer, number_part_len(s) > 0, "starts_number({s:?})");
    answer
}

/// css-syntax-3 §4.3.10 "Would start a number", asked of a `sign` and the text after it:
/// [`starts_number`] over the pair, whose sign is already in hand.
///
/// The sign and `after`'s head are asked together because neither answers alone: `-.5`
/// is a number where `.5`'s own reading says nothing about the pair.
#[inline]
pub(crate) fn sign_starts_number(sign: u8, after: &str) -> bool {
    debug_assert!(matches!(sign, b'+' | b'-'), "a sign, not {sign:?}");
    opens_number_digits(after.as_bytes())
}

/// Byte length of the numeric prefix of `s`, or 0 if it doesn't start with a
/// number. Mirrors prettier's `\d*\.\d+ | \d+\.?` plus a scientific-notation
/// exponent (`[eE][+-]?\d+`) and an optional leading sign. A bare trailing `.`
/// is part of the number only before a terminator or exponent (`1.`, `1.e1`),
/// not before a unit char, so `1.png` keeps the `.` with the unit.
pub(crate) fn number_part_len(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut i = 0;

    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let int_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let has_integer_digits = i > int_start;
    let mut has_fraction_digits = false;

    if i < bytes.len() && bytes[i] == b'.' {
        let after_dot = &s[i + 1..];
        if i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            has_fraction_digits = true;
        } else if exponent_len(after_dot) > 0 {
            // Trailing dot before an exponent: `1.e1`.
            i += 1;
        } else if has_integer_digits
            && after_dot
                .as_bytes()
                .first()
                .is_none_or(|&b| !continues_unit_byte(b) && b != b'.')
        {
            // Trailing dot before a number terminator / EOF: `1.` → `1`. A *second*
            // dot (`1..`) is excluded (`b != b'.'`): consuming this dot would strip it
            // (`1.` → `1`) and re-glue the leftover onto the next token
            // (`1..e-10` → `1.e-10`, `1..` → `1.`), which re-parses as a number and
            // normalizes again — an F1 non-idempotency. A malformed consecutive-double-
            // dot number is instead left verbatim (like `..5` / `1.5.5` / `1.px`
            // already are; prettier normalizes `1..` → `1`, a cataloged divergence).
            i += 1;
        }
    }

    if !has_integer_digits && !has_fraction_digits {
        return 0; // Just a sign and/or a lone dot — not a number.
    }

    i + exponent_len(&s[i..])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every string of up to three characters over all of ASCII plus a few non-ASCII
    /// leads (two two-byte ones, a three-byte and a four-byte one): [`starts_number`] is
    /// exactly `number_part_len > 0`, and [`sign_starts_number`] exactly that over the
    /// sign and what follows it. Three characters is all either head reads.
    #[test]
    fn starts_number_is_number_part_len_nonzero() {
        let alphabet: Vec<char> = (0u8..0x80)
            .map(char::from)
            .chain(['\u{a0}', '\u{e9}', '\u{3000}', '\u{1f4a9}'])
            .collect();
        let mut s = String::new();
        let check = |s: &str| {
            let expected = number_part_len(s) > 0;
            assert_eq!(starts_number(s), expected, "starts_number({s:?})");
            if let Some(rest) = s.strip_prefix(['+', '-']) {
                let sign = s.as_bytes()[0];
                assert_eq!(sign_starts_number(sign, rest), expected, "sign {s:?}");
            }
        };
        check("");
        for &a in &alphabet {
            for &b in &alphabet {
                for &c in &alphabet {
                    s.clear();
                    s.extend([a, b, c]);
                    check(&s);
                }
                s.clear();
                s.extend([a, b]);
                check(&s);
            }
            s.clear();
            s.push(a);
            check(&s);
        }
    }

    /// [`continues_unit`] is the Unicode-aware reading it replaced, on every char, and
    /// [`continues_unit_byte`] agrees with it on each char's UTF-8 lead byte.
    #[test]
    fn continues_unit_is_its_unicode_reading() {
        for ch in (0..=0x10_FFFF).filter_map(char::from_u32) {
            let unicode =
                ch.is_alphabetic() || ch == '_' || ch == '-' || ch == '\\' || !ch.is_ascii();
            assert_eq!(continues_unit(ch), unicode, "{ch:?}");
            let mut lead = [0u8; 4];
            let lead = ch.encode_utf8(&mut lead).as_bytes()[0];
            assert_eq!(continues_unit_byte(lead), unicode, "{ch:?}");
        }
    }

    #[test]
    fn number_part_len_basics() {
        assert_eq!(number_part_len("1"), 1);
        assert_eq!(number_part_len("123"), 3);
        assert_eq!(number_part_len("1.5"), 3);
        assert_eq!(number_part_len(".5"), 2);
        assert_eq!(number_part_len("-1.5"), 4);
        assert_eq!(number_part_len("+.5"), 3);
        assert_eq!(number_part_len("1.50"), 4);
    }

    #[test]
    fn number_part_len_excludes_unit() {
        // The unit is not part of the number.
        assert_eq!(number_part_len("1px"), 1);
        assert_eq!(number_part_len("1.5px"), 3);
        // `em`/`ex` are units, not exponents (no digit after `e`).
        assert_eq!(number_part_len("1em"), 1);
        assert_eq!(number_part_len("2ex"), 1);
    }

    #[test]
    fn number_part_len_exponents() {
        assert_eq!(number_part_len("1e1"), 3);
        assert_eq!(number_part_len("1e+1"), 4);
        assert_eq!(number_part_len("1.5e10"), 6);
        assert_eq!(number_part_len("1.5E10"), 6);
        assert_eq!(number_part_len("1.5e-0010"), 9);
        // Trailing dot before an exponent belongs to the number.
        assert_eq!(number_part_len("1.e1"), 4);
        // `1e3px` is a dimension: `1e3` number, `px` unit.
        assert_eq!(number_part_len("1e3px"), 3);
    }

    #[test]
    fn number_part_len_trailing_dot() {
        // Trailing dot before a terminator/EOF is part of the number.
        assert_eq!(number_part_len("1."), 2);
        assert_eq!(number_part_len("10."), 3);
        // Before a unit char the dot stays with the unit (`1.px` → `1` + `.px`).
        assert_eq!(number_part_len("1.px"), 1);
        assert_eq!(number_part_len("1.foo"), 1);
    }

    #[test]
    fn number_part_len_non_numbers() {
        assert_eq!(number_part_len(""), 0);
        assert_eq!(number_part_len("abc"), 0);
        assert_eq!(number_part_len("."), 0);
        assert_eq!(number_part_len("+"), 0);
        assert_eq!(number_part_len("-px"), 0);
    }

    #[test]
    fn exponent_len_distinguishes_units() {
        assert_eq!(exponent_len("e1"), 2);
        assert_eq!(exponent_len("E+10"), 4);
        assert_eq!(exponent_len("e-5"), 3);
        assert_eq!(exponent_len("em"), 0); // unit, not exponent
        assert_eq!(exponent_len("e"), 0);
        assert_eq!(exponent_len("px"), 0);
    }

    #[test]
    fn number_part_len_malformed_boundaries() {
        // A second dot terminates the number.
        assert_eq!(number_part_len("1.2.3"), 3);
        // A consecutive double dot is not absorbed into the number: only the
        // integer part is taken, so the malformed run is left verbatim rather than
        // trimmed one dot per pass (F1). `1..`/`1..e-10`/`1..5` → `1` + `..…`.
        assert_eq!(number_part_len("1.."), 1);
        assert_eq!(number_part_len("1..e-10"), 1);
        assert_eq!(number_part_len("1..5"), 1);
        // A single trailing dot is still consumed (`1.` → `1`), and a trailing dot
        // before an exponent (`1.e1`) too — those stay idempotent.
        assert_eq!(number_part_len("1."), 2);
        assert_eq!(number_part_len("1.e1"), 4);
        // Only the first exponent is consumed.
        assert_eq!(number_part_len("5e10e10"), 4);
        // Leading-dot fraction plus exponent.
        assert_eq!(number_part_len(".5e3"), 4);
        // A lone dot before an exponent (no int/fraction digit) is not a number.
        assert_eq!(number_part_len(".e3"), 0);
        // An incomplete exponent (sign, no digit) is rejected — just the `1`.
        assert_eq!(number_part_len("1e+"), 1);
        // Underscore is not a CSS number char.
        assert_eq!(number_part_len("1_000"), 1);
    }
}
