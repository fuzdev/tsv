// CSS value parsing - Phase 4
//
// Parses CSS values into structured AST (CssValue enum).
// Handles identifiers, strings, numbers/dimensions, colors, functions, and lists.

pub mod colors;
pub mod dimensions;
pub mod functions;
pub mod lists;
pub mod spacing;
pub mod strings;

use crate::ast::internal::CssValue;

// Re-export public functions
pub use colors::{parse_color, parse_color_function};
pub use dimensions::parse_dimension;
pub use functions::parse_function_arguments;
pub use lists::{
    contains_comma, contains_space_separator, parse_comma_separated_values,
    parse_space_separated_values,
};
pub use spacing::should_add_space_between;
pub use strings::parse_string_literal;

/// Parse a CSS value string into a structured CssValue
///
/// Takes the raw string representation and parses it into the appropriate
/// CssValue variant (identifier, string, number, color, function, or list).
pub fn parse_value_string(value_str: &str) -> CssValue {
    let trimmed = value_str.trim();

    if trimmed.is_empty() {
        return CssValue::Identifier(String::new());
    }

    // Check for comma-separated values first (these are unambiguous)
    if contains_comma(trimmed)
        && let Some(list) = parse_comma_separated_values(trimmed)
    {
        return list;
    }

    // Try single value
    if let Some(single) = parse_single_value(trimmed) {
        return single;
    }

    // Try space-separated list as fallback
    if contains_space_separator(trimmed)
        && let Some(list) = parse_space_separated_values(trimmed)
    {
        return list;
    }

    // Fallback to identifier
    CssValue::Identifier(trimmed.to_string())
}

/// Parse a single CSS value (no lists)
pub(crate) fn parse_single_value(s: &str) -> Option<CssValue> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // String literal
    if let Some(val) = parse_string_literal(s) {
        return Some(val);
    }

    // Function call or color function
    if let Some(paren_pos) = s.find('(')
        && let Some((name, args, true)) = extract_function_parts(s, paren_pos)
    {
        // Try color function first
        if let Some(color) = parse_color_function(&name.to_lowercase(), &args) {
            return Some(CssValue::Color(color));
        }
        // Fall back to generic function
        return Some(CssValue::Function {
            name,
            args: parse_function_arguments(&args),
        });
    }

    // Hex or named color
    if let Some(color) = parse_color(s) {
        return Some(CssValue::Color(color));
    }

    // Dimension (number with optional unit)
    if let Some(dim) = parse_dimension(s) {
        return Some(dim);
    }

    // Default to identifier
    Some(CssValue::Identifier(s.to_string()))
}

/// Extract function name and arguments, validating balanced parentheses
fn extract_function_parts(s: &str, paren_pos: usize) -> Option<(String, String, bool)> {
    let name_part = s[..paren_pos].trim();

    // Validate function name: alphanumeric, hyphens, underscores only
    if name_part.is_empty()
        || !name_part
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }

    // Find matching closing paren
    let mut paren_count = 0;
    let mut closing_paren_pos = None;

    for (i, ch) in s[paren_pos..].char_indices() {
        match ch {
            '(' => paren_count += 1,
            ')' => {
                paren_count -= 1;
                if paren_count == 0 {
                    closing_paren_pos = Some(paren_pos + i);
                    break;
                }
            }
            _ => {}
        }
    }

    // Closing paren must be at end of string
    let close_pos = closing_paren_pos?;
    if close_pos != s.len() - 1 {
        return None;
    }

    let args = s[paren_pos + 1..close_pos].to_string();
    Some((name_part.to_string(), args, true))
}
