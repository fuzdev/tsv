// CSS number grammar — the single source of truth for how a numeric token is
// scanned, shared by the lexer (token spans), the parser (dimension splitting),
// and the printer (value/prelude normalization). Pure functions over `&str`;
// no allocation, no AST. Formatting (leading-zero/trailing-zero normalization)
// lives in `printer::value_normalization`, not here — this module is grammar only.

/// Can `ch` continue a dimension unit / CSS identifier?
///
/// Used to decide whether a trailing `.` belongs to the number: a unit char
/// after the dot (`1.px`, `1.png`) means the dot is not a number terminator.
pub(crate) fn continues_unit(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_' || ch == '-' || ch == '\\' || !ch.is_ascii()
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
                .chars()
                .next()
                .is_none_or(|ch| !continues_unit(ch))
        {
            // Trailing dot before a number terminator / EOF: `1.` → `1`.
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
}
