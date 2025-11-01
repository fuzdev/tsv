use crate::ast::internal::CssValue;

/// Parse dimension value: "10px", "1.5em", "50%", or unitless number
pub fn parse_dimension(s: &str) -> Option<CssValue> {
    let (number, unit, source) = parse_dimension_parts(s)?;
    Some(CssValue::Dimension {
        value: number,
        unit,
        source,
    })
}

/// Extract numeric part and unit from a dimension string
fn parse_dimension_parts(s: &str) -> Option<(f64, String, String)> {
    let mut num_end = 0;
    let mut found_dot = false;
    let bytes = s.as_bytes();

    // Handle optional sign
    if num_end < bytes.len() && (bytes[num_end] == b'-' || bytes[num_end] == b'+') {
        num_end += 1;
    }

    // Parse digits and decimal point
    while num_end < bytes.len() {
        if bytes[num_end].is_ascii_digit() {
            num_end += 1;
        } else if bytes[num_end] == b'.'
            && !found_dot
            && num_end + 1 < bytes.len()
            && bytes[num_end + 1].is_ascii_digit()
        {
            found_dot = true;
            num_end += 1;
        } else {
            break;
        }
    }

    // At least one digit required
    if num_end == 0 || (num_end == 1 && (bytes[0] == b'-' || bytes[0] == b'+')) {
        return None;
    }

    let num_str = &s[..num_end];
    let number = num_str.parse::<f64>().ok()?;
    let unit = s[num_end..].to_string();
    let source = s.to_string(); // preserve full source including leading zeros

    Some((number, unit, source))
}
