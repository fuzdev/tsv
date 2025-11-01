use crate::ast::internal::CssValue;

/// Parse CSS string with proper quote handling
pub fn parse_string_literal(s: &str) -> Option<CssValue> {
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        let raw_content = &s[1..s.len() - 1];
        let content = decode_string_escapes(raw_content);
        return Some(CssValue::String(content));
    }
    None
}

/// Decode CSS string escapes: \", \', and preserve other escape sequences
///
/// Only decodes quote escapes (\", \') to allow proper quote style selection.
/// Preserves other escapes like \\, \n, etc. as literal characters in CSS output.
fn decode_string_escapes(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(&next_ch) = chars.peek() {
                match next_ch {
                    '"' | '\'' => {
                        result.push(next_ch);
                        chars.next();
                    }
                    _ => {
                        // Keep other escape sequences: \\, \n, \r, \t, \XXXXXX, etc.
                        result.push(ch);
                    }
                }
            } else {
                result.push(ch);
            }
        } else {
            result.push(ch);
        }
    }

    result
}
