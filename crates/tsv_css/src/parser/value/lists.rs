// Top-level separator classification for CSS values.
//
// Old list parsing functions removed - replaced by ValueParser with same-source recursion
// - parse_comma_separated_values() → handled by ValueParser::parse_comma_separated()
// - parse_space_separated_values() → handled by ValueParser::parse_space_separated()
// - split_values_at_delimiter() → replaced by ValueCursor usage in ValueParser
// See: crates/tsv_css/src/parser/value/parser.rs for the new implementation

/// Top-level separator found in a CSS value string.
///
/// "Top level" means not inside parentheses, quotes, or block comments. Commas
/// take priority over whitespace (a value with both is comma-separated), so the
/// variants are ordered by that precedence.
#[derive(Debug, PartialEq, Eq)]
pub enum ValueSeparator {
    /// At least one top-level comma — parse as a comma-separated list.
    Comma,
    /// No top-level comma, but at least one top-level whitespace run — parse as
    /// a space-separated list.
    Whitespace,
    /// Neither — a single leaf value.
    None,
}

/// Classify a CSS value's top-level separators in a single pass over `s`. Comma
/// detection short-circuits on the first top-level comma (commas win even when
/// whitespace appears earlier); whitespace is merely recorded while scanning, so
/// a later comma still takes priority. Block-comment bodies are skipped, so a `,`
/// or space inside `/* ... */` is not treated as a separator. Callers pass the
/// already-trimmed value text — leading/trailing whitespace must not classify as
/// a separator.
///
/// Scans raw bytes: every CSS separator — `,`, ASCII whitespace, and the `(` `)`
/// quote `/*` structure that gates them — is ASCII, so a non-ASCII byte (including
/// a multibyte char's continuation byte, e.g. the `0xA0` in U+4E20 that a careless
/// `as char` cast would alias to NBSP) is never a separator and falls through as
/// content. This reaches the same verdict as the real-`char` splitting in
/// [`crate::parser::value::cursor::ValueCursor`] (which tests `is_css_whitespace`),
/// without decoding.
///
/// That `ValueCursor` is intentionally comment-*blind* (it tracks paren/quote
/// nesting through comment bodies) while this classifier is comment-*aware*, so
/// the two can still disagree on a value with an unbalanced paren/quote inside a
/// comment. `ValueParser::split_top_level`'s progress guard makes that
/// disagreement safe — it parses such a range as a single leaf instead of
/// re-splitting it forever.
pub fn classify_separators(s: &str) -> ValueSeparator {
    let mut in_parens: u32 = 0;
    let mut in_quote = false;
    let mut in_comment = false;
    let mut quote_char = 0u8;
    let mut whitespace_seen = false;
    let bytes = s.as_bytes();

    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];

        // Comment start (outside quotes)
        if !in_quote && !in_comment && b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            in_comment = true;
            i += 2;
            continue;
        }

        // Comment end
        if in_comment && b == b'*' && bytes.get(i + 1) == Some(&b'/') {
            in_comment = false;
            i += 2;
            continue;
        }

        // Skip content inside comments
        if in_comment {
            i += 1;
            continue;
        }

        // An escaped paren (`\(` / `\)`) is a content code point (css-syntax §4.3.7), not a
        // nesting delimiter, so it must not change `in_parens` — otherwise an escaped `)`
        // inside `url()` mis-drops the depth and exposes a false top-level separator. Skip
        // both bytes. Kept identical in the twin `fast_scan` / `ValueCursor` trackers.
        if b == b'\\' && matches!(bytes.get(i + 1), Some(b'(' | b')')) {
            i += 2;
            continue;
        }

        match b {
            b'\'' | b'"' if !in_quote => {
                in_quote = true;
                quote_char = b;
            }
            _ if in_quote && b == quote_char => in_quote = false,
            b'(' if !in_quote => in_parens += 1,
            b')' if !in_quote => in_parens = in_parens.saturating_sub(1),
            // Comma wins immediately, before whitespace is ever consulted.
            b',' if in_parens == 0 && !in_quote => return ValueSeparator::Comma,
            // CSS whitespace is ASCII-only; a non-ASCII byte is never a separator.
            _ if in_parens == 0 && !in_quote && b.is_ascii_whitespace() => whitespace_seen = true,
            _ => {}
        }
        i += 1;
    }

    if whitespace_seen {
        ValueSeparator::Whitespace
    } else {
        ValueSeparator::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_value_has_no_separators() {
        assert_eq!(classify_separators("auto"), ValueSeparator::None);
        assert_eq!(classify_separators("10px"), ValueSeparator::None);
        assert_eq!(classify_separators("#fff"), ValueSeparator::None);
    }

    #[test]
    fn comma_is_detected() {
        assert_eq!(classify_separators("red, blue"), ValueSeparator::Comma);
        assert_eq!(classify_separators("a,b,c"), ValueSeparator::Comma);
    }

    #[test]
    fn whitespace_is_detected() {
        assert_eq!(
            classify_separators("1px solid red"),
            ValueSeparator::Whitespace
        );
        assert_eq!(classify_separators("a\tb"), ValueSeparator::Whitespace);
    }

    #[test]
    fn comma_takes_priority_over_earlier_whitespace() {
        // whitespace appears first, but a later top-level comma still wins
        assert_eq!(
            classify_separators("red blue, green"),
            ValueSeparator::Comma
        );
    }

    #[test]
    fn separators_inside_parens_are_ignored() {
        assert_eq!(classify_separators("rgba(1, 2, 3)"), ValueSeparator::None);
        assert_eq!(classify_separators("calc(1px + 2px)"), ValueSeparator::None);
        assert_eq!(
            classify_separators("rgba(1, 2, 3), blue"),
            ValueSeparator::Comma
        );
    }

    #[test]
    fn separators_inside_quotes_are_ignored() {
        assert_eq!(classify_separators(r#""foo, bar""#), ValueSeparator::None);
        assert_eq!(classify_separators(r#""foo bar""#), ValueSeparator::None);
        assert_eq!(
            classify_separators(r#""foo, bar", baz"#),
            ValueSeparator::Comma
        );
    }

    #[test]
    fn separators_inside_comments_are_ignored() {
        // a comma inside a comment must not be classified as a comma separator
        assert_eq!(classify_separators("red/*,*/blue"), ValueSeparator::None);
        // whitespace inside a comment must not classify as whitespace either
        assert_eq!(classify_separators("red/* x */blue"), ValueSeparator::None);
    }

    #[test]
    fn whitespace_outside_comment_still_counts() {
        // the space between `*/` and `red` is a real top-level separator
        assert_eq!(
            classify_separators("/* x */ red"),
            ValueSeparator::Whitespace
        );
    }

    #[test]
    fn multibyte_token_is_not_a_separator() {
        // U+4E20 encodes as `E4 B8 A0`; its `0xA0` continuation byte must not be
        // mistaken for U+00A0/NBSP whitespace, so a lone non-ASCII token is a
        // single value, not a separator.
        assert_eq!(classify_separators("丠"), ValueSeparator::None);
        // genuine separators around such tokens are still detected
        assert_eq!(classify_separators("丠 中"), ValueSeparator::Whitespace);
        assert_eq!(classify_separators("丠, 中"), ValueSeparator::Comma);
        // the aliasing byte inside parens stays contained
        assert_eq!(classify_separators("calc(丠)"), ValueSeparator::None);
    }
}
