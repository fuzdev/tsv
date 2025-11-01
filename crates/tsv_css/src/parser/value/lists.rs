use crate::ast::internal::CssValue;

/// Parse comma-separated values: "a, b, c"
pub fn parse_comma_separated_values(s: &str) -> Option<CssValue> {
    let parts = split_values_at_delimiter(s, |c| c == ',');
    if parts.is_empty() {
        return None;
    }
    Some(CssValue::CommaSeparated { values: parts })
}

/// Parse space-separated values: "a b c"
pub fn parse_space_separated_values(s: &str) -> Option<CssValue> {
    let values = split_values_at_delimiter(s, |c| c == ' ');
    if values.len() < 2 {
        return None;
    }
    Some(CssValue::List { values })
}

/// Split CSS values at a delimiter, respecting nested parentheses
///
/// Used by both comma and space-separated parsing. Ignores delimiters inside
/// function calls (nested parens).
pub fn split_values_at_delimiter<F>(s: &str, is_delimiter: F) -> Vec<CssValue>
where
    F: Fn(char) -> bool,
{
    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_parens = 0;

    for ch in s.chars() {
        match ch {
            '(' => {
                in_parens += 1;
                current.push(ch);
            }
            ')' => {
                in_parens -= 1;
                current.push(ch);
            }
            c if in_parens == 0 && is_delimiter(c) => {
                if !current.is_empty() {
                    if let Some(val) = super::parse_single_value(current.trim()) {
                        values.push(val);
                    }
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty()
        && let Some(val) = super::parse_single_value(current.trim())
    {
        values.push(val);
    }

    values
}

/// Check if a string contains a comma at the top level (not in parens)
pub fn contains_comma(s: &str) -> bool {
    let mut in_parens = 0;
    for ch in s.chars() {
        match ch {
            '(' => in_parens += 1,
            ')' => in_parens -= 1,
            ',' if in_parens == 0 => return true,
            _ => {}
        }
    }
    false
}

/// Check if a string contains a space separator (not in parens/quotes)
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
            ' ' if in_parens == 0 && !in_quote => return true,
            _ => {}
        }
    }
    false
}
