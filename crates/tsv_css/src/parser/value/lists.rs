// Old list parsing functions removed - replaced by ValueParser with same-source recursion
// - parse_comma_separated_values() → handled by ValueParser::parse_comma_separated()
// - parse_space_separated_values() → handled by ValueParser::parse_space_separated()
// - split_values_at_delimiter() → replaced by ValueCursor usage in ValueParser
// See: crates/tsv_css/src/parser/value/parser.rs for the new implementation

/// Check if a string contains a comma at the top level (not in parens or quotes)
pub fn contains_comma(s: &str) -> bool {
    let mut in_parens = 0;
    let mut in_quote = false;
    let mut quote_char = '\0';

    for ch in s.chars() {
        match ch {
            '\'' | '"' if !in_quote => {
                in_quote = true;
                quote_char = ch;
            }
            c if in_quote && c == quote_char => {
                in_quote = false;
            }
            '(' if !in_quote => in_parens += 1,
            ')' if !in_quote => in_parens -= 1,
            ',' if in_parens == 0 && !in_quote => return true,
            _ => {}
        }
    }
    false
}

/// Check if a string contains a space separator (not in parens/quotes)
///
/// Note: This checks for ANY whitespace character (space, tab, newline, etc.),
/// not just literal spaces. This is important for handling multiline values.
pub fn contains_space_separator(s: &str) -> bool {
    let mut in_parens = 0;
    let mut in_quote = false;
    let mut quote_char = '\0';

    for ch in s.chars() {
        match ch {
            '\'' | '"' if !in_quote => {
                in_quote = true;
                quote_char = ch;
            }
            c if in_quote && c == quote_char => {
                in_quote = false;
            }
            '(' if !in_quote => in_parens += 1,
            ')' if !in_quote => in_parens -= 1,
            c if c.is_whitespace() && in_parens == 0 && !in_quote => return true,
            _ => {}
        }
    }
    false
}
