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

/// Classify a CSS value's top-level separators in a single pass.
///
/// This fuses the former `contains_comma(s)`-then-`contains_space_separator(s)`
/// pair into one walk over `s`. Behavior is identical: comma detection
/// short-circuits on the first top-level comma (commas win even when whitespace
/// appears earlier), and whitespace is merely recorded while scanning so a later
/// comma still takes priority. Block-comment bodies are skipped so a `,` or space
/// inside `/* ... */` is not treated as a separator.
///
/// Callers pass the already-trimmed value text — leading/trailing whitespace must
/// not classify as a separator.
///
/// Note: the [`crate::parser::value::cursor::ValueCursor`] that performs the
/// actual split is intentionally comment-*blind* (it tracks paren/quote nesting
/// through comment bodies). This classifier is comment-*aware*. The two only
/// disagree on values with unbalanced parens/quotes inside comments, and folding
/// the split into this pass would change that behavior for negligible gain — the
/// redundant work being removed here is the two full classification scans on
/// non-separator content (every leaf), not the split itself.
pub fn classify_separators(s: &str) -> ValueSeparator {
    let mut in_parens: u32 = 0;
    let mut in_quote = false;
    let mut in_comment = false;
    let mut quote_char = '\0';
    let mut whitespace_seen = false;
    let bytes = s.as_bytes();

    let mut i = 0;
    while i < bytes.len() {
        // Check for comment start (outside quotes)
        if !in_quote
            && !in_comment
            && i + 1 < bytes.len()
            && bytes[i] == b'/'
            && bytes[i + 1] == b'*'
        {
            in_comment = true;
            i += 2;
            continue;
        }

        // Check for comment end
        if in_comment && i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
            in_comment = false;
            i += 2;
            continue;
        }

        // Skip content inside comments
        if in_comment {
            i += 1;
            continue;
        }

        let ch = bytes[i] as char;
        match ch {
            '\'' | '"' if !in_quote => {
                in_quote = true;
                quote_char = ch;
            }
            c if in_quote && c == quote_char => {
                in_quote = false;
            }
            '(' if !in_quote => in_parens += 1,
            ')' if !in_quote => in_parens = in_parens.saturating_sub(1),
            // Comma wins immediately — matches the original early-out in
            // `contains_comma`, before whitespace is ever consulted.
            ',' if in_parens == 0 && !in_quote => return ValueSeparator::Comma,
            c if in_parens == 0 && !in_quote && c.is_whitespace() => whitespace_seen = true,
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
}
