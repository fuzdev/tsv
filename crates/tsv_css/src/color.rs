// CSS hex-color classification, shared by the value parser (which tags a `#`-hash
// as `Color::Hex`) and the printer (which lowercases a hex color it scans out of a
// raw at-rule prelude) so both agree on what a hex color is. Keeping the rule in
// one place is what stops the two from drifting — a drift between a parser-side and
// a printer-side copy is exactly how `#ZZZ` once lowercased on one path but not the
// other.

/// Whether `body` (the chars after `#`) is a hex **color**: 3, 4, 6, or 8 ASCII hex
/// digits and nothing else — prettier's lowercased lengths (`#rgb`, `#rgba`,
/// `#rrggbb`, `#rrggbbaa`). An off-length run (`#abcde`) or any non-hex char
/// (`#ZZZ`, `#12G`) is a valid hash token but not a color, so it keeps its case.
#[inline]
pub(crate) fn is_hex_color_body(body: &str) -> bool {
    matches!(body.len(), 3 | 4 | 6 | 8) && body.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_digit_bodies_of_color_length_are_hex() {
        // 3/4/6/8 hex-digit bodies, either case.
        for body in [
            "abc", "ABC", "abcd", "ABCD", "aabbcc", "AABBCC", "aabbccdd", "AABBCCDD", "123ABC",
        ] {
            assert!(is_hex_color_body(body), "#{body} is a hex color");
        }
    }

    #[test]
    fn non_hex_char_is_not_a_color() {
        // Color length but not all hex digits — a valid hash token, not a color.
        for body in ["ZZZ", "GGGG", "12G", "XYZABC", "ZZZZZZZZ", "12345G"] {
            assert!(!is_hex_color_body(body), "#{body} is not a color");
        }
    }

    #[test]
    fn off_length_is_not_a_color() {
        // Only 3/4/6/8-char bodies can be a hex color.
        for body in ["", "a", "ab", "abcde", "abcdefg"] {
            assert!(!is_hex_color_body(body), "#{body} is not a hex color");
        }
    }
}
