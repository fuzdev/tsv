// Old list parsing functions removed - replaced by ValueParser with same-source recursion
// - parse_comma_separated_values() → handled by ValueParser::parse_comma_separated()
// - parse_space_separated_values() → handled by ValueParser::parse_space_separated()
// - split_values_at_delimiter() → replaced by ValueCursor usage in ValueParser
// See: crates/tsv_css/src/parser/value/parser.rs for the new implementation

/// Check if a string contains a comma at the top level (not in parens, quotes, or comments)
pub fn contains_comma(s: &str) -> bool {
    contains_at_top_level(s, |ch| ch == ',')
}

/// Check if a string contains a space separator (not in parens, quotes, or comments)
///
/// Note: This checks for ANY whitespace character (space, tab, newline, etc.),
/// not just literal spaces. This is important for handling multiline values.
/// Skips over block comments so whitespace inside `/* a b */` isn't treated as a separator.
pub fn contains_space_separator(s: &str) -> bool {
    contains_at_top_level(s, char::is_whitespace)
}

/// Check if a string contains a character matching the predicate at the top level
///
/// "Top level" means not inside:
/// - Parentheses: `(...)`
/// - Quotes: `'...'` or `"..."`
/// - Block comments: `/* ... */`
fn contains_at_top_level<F>(s: &str, predicate: F) -> bool
where
    F: Fn(char) -> bool,
{
    let mut in_parens: u32 = 0;
    let mut in_quote = false;
    let mut in_comment = false;
    let mut quote_char = '\0';
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
            c if in_parens == 0 && !in_quote && predicate(c) => return true,
            _ => {}
        }
        i += 1;
    }
    false
}
