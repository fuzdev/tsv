// Source fidelity utilities - extracting raw source text while preserving quirks
//
// This module centralizes logic for deciding when to extract raw source text vs
// using semantic AST formatting. This is part of Sprint 1 cleanup to remove
// source text duplication from the AST (see TODO_CSS_AST_CLEANUP.md).
//
// ## Architecture
//
// Internal AST stores semantic data + spans. When formatting, we decide:
// - Extract raw source: preserve escapes, comments, quirks (source fidelity)
// - Format from AST: normalize spacing, apply prettier rules (semantic formatting)
//
// ## Usage
//
// ```rust
// use crate::printer::source_fidelity::extract_raw_value;
//
// let raw = extract_raw_value(source, value.span());
// output.write(raw);
// ```

use crate::ast::internal::{Color, ColorChannel};
use tsv_lang::Span;
use tsv_lang::printing::{StringFormatOptions, format_string_literal};

/// Extract raw source text for a value (preserves escapes, comments, quirks)
///
/// Use this when you need source fidelity (e.g., complex escape sequences, value comments).
///
/// # Arguments
/// * `source` - Original CSS source text
/// * `span` - Span of the value to extract
///
/// # Returns
/// * Raw source text slice
///
/// # Example
/// ```ignore
/// let raw = extract_raw_value(source, value.span());
/// printer.write(raw);  // Preserves "01px" not "1px", "\0041" not "A"
/// ```
pub fn extract_raw_value(source: &str, span: Span) -> &str {
    span.extract(source)
}

/// Format an identifier value semantically
///
/// # Example
/// ```ignore
/// assert_eq!(format_identifier_value("red"), "red");
/// assert_eq!(format_identifier_value("auto"), "auto");
/// ```
pub fn format_identifier_value(name: &str) -> String {
    name.to_string()
}

/// Format a dimension value semantically
///
/// Formats a numeric value with its unit (or just the number if unitless).
/// Normalizes decimal representation to match prettier:
/// - Removes trailing zeros: 1.50 → 1.5, 100.0 → 100
/// - Adds leading zero: .5 → 0.5 (handled by f64 representation)
///
/// # Example
/// ```ignore
/// assert_eq!(format_dimension_value(10.0, "px"), "10px");
/// assert_eq!(format_dimension_value(0.5, "em"), "0.5em");
/// assert_eq!(format_dimension_value(1.5, "em"), "1.5em");
/// assert_eq!(format_dimension_value(100.0, "px"), "100px");
/// ```
pub fn format_dimension_value(value: f64, unit: &str) -> String {
    let normalized = normalize_number(value);
    if unit.is_empty() {
        normalized
    } else {
        format!("{normalized}{unit}")
    }
}

/// Normalize a number's decimal representation to match prettier
///
/// Prettier's decimal normalization rules:
/// - Removes trailing zeros after decimal: 1.50 → 1.5, 100.0 → 100
/// - Adds leading zero for decimals: .5 → 0.5 (handled by f64 formatting)
/// - Preserves leading zeros before decimal: 01 → 01, 007 → 007
///
/// The key insight: f64 representation loses leading zeros like `01`,
/// so we can safely use f64::to_string() and only normalize trailing zeros.
fn normalize_number(value: f64) -> String {
    // Handle special cases first
    if value == 0.0 {
        return "0".to_string();
    }

    // Convert to string - this automatically adds leading zero for .5 → 0.5
    // but loses leading zeros like 01 (becomes 1.0)
    let mut s = value.to_string();

    // If no decimal point, return as-is
    if !s.contains('.') {
        return s;
    }

    // Remove trailing zeros after decimal point
    // Keep the decimal point initially to detect whole numbers
    while s.ends_with('0') && s.contains('.') {
        s.pop();
    }

    // If we removed all digits after decimal, remove the decimal point too
    if s.ends_with('.') {
        s.pop();
    }

    s
}

/// Normalize a dimension value from raw source string
///
/// This function matches prettier's exact behavior:
/// - Preserves leading zeros: `01.5px` → `01.5px`
/// - Preserves signs: `+10.0px` → `+10px`, `-0.0px` → `-0px`
/// - Removes trailing zeros: `1.50px` → `1.5px`, `100.0px` → `100px`
/// - Adds leading zero: `.5px` → `0.5px`
///
/// # Arguments
/// * `raw` - The raw dimension string from source (e.g., "01.5px", "+10.0em")
///
/// # Returns
/// Normalized dimension string matching prettier's output
pub fn normalize_dimension_from_source(raw: &str) -> String {
    // If no decimal point, return as-is (preserves leading zeros, signs)
    if !raw.contains('.') {
        return raw.to_string();
    }

    // Split into number part and unit part
    // Find where the number ends (first non-numeric, non-sign, non-decimal char)
    let unit_start = raw
        .chars()
        .position(|c| !c.is_ascii_digit() && c != '.' && c != '+' && c != '-')
        .unwrap_or(raw.len());

    let num_part = &raw[..unit_start];
    let unit_part = &raw[unit_start..];

    // Edge case: if number ends with a decimal point and there's a unit after,
    // this is likely not a dimension (e.g., "1.png" where .png is a file extension)
    // In valid CSS, dimensions with units must have digits after the decimal: "1.5px" not "1.px"
    if num_part.ends_with('.') && !unit_part.is_empty() {
        return raw.to_string();
    }

    // Normalize the number part (preserve sign, leading zeros, add leading zero, trim trailing zeros)
    let normalized_num = normalize_decimal_preserving_prefix(num_part);

    format!("{normalized_num}{unit_part}")
}

/// Normalize decimal number while preserving sign and leading zeros
///
/// Examples:
/// - `01.50` → `01.5` (preserve leading zero, trim trailing)
/// - `+10.0` → `+10` (preserve sign, trim trailing)
/// - `-0.0` → `-0` (preserve negative zero)
/// - `.5` → `0.5` (add leading zero)
fn normalize_decimal_preserving_prefix(num: &str) -> String {
    // Extract sign if present
    let (sign, rest) = if let Some(stripped) = num.strip_prefix('-') {
        ("-", stripped)
    } else if let Some(stripped) = num.strip_prefix('+') {
        ("+", stripped)
    } else {
        ("", num)
    };

    // Add leading zero if starts with decimal point
    let with_leading = if rest.starts_with('.') {
        format!("0{rest}")
    } else {
        rest.to_string()
    };

    // Remove trailing zeros after decimal point
    let trimmed = if with_leading.contains('.') {
        let mut s = with_leading;
        // Remove trailing zeros
        while s.ends_with('0') && s.contains('.') {
            s.pop();
        }
        // If we removed all digits after decimal, remove the decimal point too
        if s.ends_with('.') {
            s.pop();
        }
        s
    } else {
        with_leading
    };

    format!("{sign}{trimmed}")
}

/// Format a color value semantically
///
/// Converts a Color AST node to its string representation.
/// Hex colors are normalized to lowercase to match prettier's behavior.
///
/// # Example
/// ```ignore
/// let color = Color::Named("red".to_string());
/// assert_eq!(format_color_value(&color), "red");
///
/// let color = Color::Hex("#FF0000".to_string());
/// assert_eq!(format_color_value(&color), "#ff0000");
/// ```
pub fn format_color_value(color: &Color) -> String {
    match color {
        Color::Named(name) => name.clone(),
        Color::Hex(hex) => hex.to_lowercase(),
        Color::Rgb { r, g, b, alpha } => {
            let r_str = format_color_channel(r);
            let g_str = format_color_channel(g);
            let b_str = format_color_channel(b);

            if let Some(a) = alpha {
                let a_str = format_color_channel(a);
                format!("rgba({r_str}, {g_str}, {b_str}, {a_str})")
            } else {
                format!("rgb({r_str}, {g_str}, {b_str})")
            }
        }
        Color::Hsl {
            hue,
            hue_unit,
            saturation,
            lightness,
            alpha,
        } => {
            // Format hue with optional unit
            let hue_str = if let Some(unit) = hue_unit {
                format!("{}{}", format_color_channel(hue), unit.as_str())
            } else {
                format_color_channel(hue)
            };
            let sat_str = format_color_channel(saturation);
            let light_str = format_color_channel(lightness);

            if let Some(a) = alpha {
                let a_str = format_color_channel(a);
                format!("hsla({hue_str}, {sat_str}, {light_str}, {a_str})")
            } else {
                format!("hsl({hue_str}, {sat_str}, {light_str})")
            }
        }
    }
}

/// Format a ColorChannel value
fn format_color_channel(channel: &ColorChannel) -> String {
    match channel {
        ColorChannel::Number(n) => {
            // Format number, removing unnecessary decimals
            if n.fract() == 0.0 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        ColorChannel::Percentage(p) => {
            // Format percentage
            if p.fract() == 0.0 {
                format!("{}%", *p as i64)
            } else {
                format!("{p}%")
            }
        }
        ColorChannel::None => "none".to_string(),
    }
}

/// Format a color value with syntax preservation
///
/// Extracts the original syntax from source and reformats with proper spacing
/// while preserving the syntax choice (rgb vs rgba, comma vs space, / vs not).
///
/// # Arguments
/// * `color` - The parsed color
/// * `source` - The original source code
/// * `span` - The span of the color in source
pub fn format_color_from_source(color: &Color, source: &str, span: Span) -> String {
    // Named and hex colors don't need syntax detection
    match color {
        Color::Named(name) => return name.clone(),
        Color::Hex(hex) => return hex.to_lowercase(),
        _ => {}
    }

    // Extract raw text to detect syntax
    let raw = span.extract(source);

    // Detect function name and syntax
    if let Some(open_paren) = raw.find('(') {
        let func_name = &raw[..open_paren];
        let has_slash = raw.contains('/');
        let has_comma = raw.contains(',');

        match color {
            Color::Rgb { r, g, b, alpha } => {
                let r_str = format_color_channel(r);
                let g_str = format_color_channel(g);
                let b_str = format_color_channel(b);

                if let Some(a) = alpha {
                    let a_str = format_color_channel(a);
                    if has_slash {
                        // rgb(r g b / a) syntax
                        format!("rgb({r_str} {g_str} {b_str} / {a_str})")
                    } else if func_name == "rgba" {
                        // rgba(r, g, b, a) syntax
                        format!("rgba({r_str}, {g_str}, {b_str}, {a_str})")
                    } else {
                        // Fallback: rgba with comma
                        format!("rgba({r_str}, {g_str}, {b_str}, {a_str})")
                    }
                } else if has_comma {
                    format!("rgb({r_str}, {g_str}, {b_str})")
                } else {
                    format!("rgb({r_str} {g_str} {b_str})")
                }
            }
            Color::Hsl {
                hue,
                hue_unit,
                saturation,
                lightness,
                alpha,
            } => {
                // Format hue with optional unit
                let hue_str = if let Some(unit) = hue_unit {
                    format!("{}{}", format_color_channel(hue), unit.as_str())
                } else {
                    format_color_channel(hue)
                };
                let sat_str = format_color_channel(saturation);
                let light_str = format_color_channel(lightness);

                if let Some(a) = alpha {
                    let a_str = format_color_channel(a);
                    if has_slash {
                        // hsl(h s% l% / a) syntax
                        format!("hsl({hue_str} {sat_str} {light_str} / {a_str})")
                    } else if func_name == "hsla" {
                        // hsla(h, s%, l%, a) syntax
                        format!("hsla({hue_str}, {sat_str}, {light_str}, {a_str})")
                    } else {
                        // Fallback: hsla with comma
                        format!("hsla({hue_str}, {sat_str}, {light_str}, {a_str})")
                    }
                } else if has_comma {
                    format!("hsl({hue_str}, {sat_str}, {light_str})")
                } else {
                    format!("hsl({hue_str} {sat_str} {light_str})")
                }
            }
            // Fallback for any other color types (future-proofing)
            _ => format_color_value(color),
        }
    } else {
        // Fallback to basic formatting
        format_color_value(color)
    }
}

/// Format a string value semantically
///
/// Formats a string with the specified quote character, properly escaping content.
///
/// # Example
/// ```ignore
/// assert_eq!(format_string_value("hello", '\''), "'hello'");
/// assert_eq!(format_string_value("world", '"'), "\"world\"");
/// ```
pub fn format_string_value(content: &str, quote: char) -> String {
    format_string_literal(content, quote, StringFormatOptions::default())
}

/// Extract and normalize property name from declaration source
///
/// Handles property names with comments, preserving raw escapes (Svelte quirk).
/// Normalizes spacing around comments for readability.
///
/// # Arguments
/// * `decl_source` - Full declaration source (e.g., `color /* test */: red;`)
///
/// # Returns
/// * Normalized property name (e.g., `color /* test */`)
///
/// # Example
/// ```ignore
/// let source = "color/* comment */:red;";
/// assert_eq!(extract_property_name(source), "color /* comment */");
///
/// let source = "margin: 10px;";
/// assert_eq!(extract_property_name(source), "margin");
/// ```
///
/// # Svelte Quirk
/// Property names preserve raw escapes without decoding.
/// Example: `\00e9motion` stays as `\00e9motion`, not `émotion`
///
/// # Formatter Divergence
/// We add spaces around comments for readability:
/// - Input: `color/* comment */:red;`
/// - Output: `color /* comment */` (normalized spacing)
/// - Prettier: `color/* comment */` (no space before comment)
pub fn extract_property_name(decl_source: &str) -> String {
    if let Some(colon_pos) = decl_source.find(':') {
        let property_part = &decl_source[..colon_pos];

        // Check if property contains a comment
        if let Some(comment_start) = property_part.find("/*") {
            if let Some(comment_end_rel) = property_part[comment_start..].find("*/") {
                let comment_end = comment_start + comment_end_rel + 2; // Include */
                // Extract parts
                let before_comment = property_part[..comment_start].trim();
                let comment = &property_part[comment_start..comment_end];

                // Normalize: property + space + comment (no trailing space)
                format!("{before_comment} {comment}")
            } else {
                // Malformed comment - just trim
                property_part.trim().to_string()
            }
        } else {
            // No comment - just trim
            property_part.trim().to_string()
        }
    } else {
        // Fallback: no colon found (malformed declaration)
        decl_source.trim().to_string()
    }
}

/// Extract and format string value from declaration source
///
/// Preserves escape sequences by working with original source text.
///
/// # Arguments
/// * `decl_source` - Full declaration source (e.g., `content: "test\n";`)
/// * `quote` - Quote character to use (' or ")
///
/// # Returns
/// * `Some(formatted_string)` if extraction successful
/// * `None` if extraction failed (malformed source)
///
/// # Example
/// ```ignore
/// let source = "content: 'hello\\nworld';";
/// assert_eq!(extract_string_value(source, '\''), Some("'hello\\nworld'".to_string()));
/// ```
pub fn extract_string_value(decl_source: &str, quote: char) -> Option<String> {
    if let Some(colon_pos) = decl_source.find(':') {
        let value_part = decl_source[colon_pos + 1..].trim();
        // String should be quoted
        if value_part.len() >= 2 {
            let raw_content = &value_part[1..value_part.len() - 1];
            let formatted =
                format_string_literal(raw_content, quote, StringFormatOptions::default());
            return Some(formatted);
        }
    }

    None
}

/// Extract and normalize value with comments from declaration source
///
/// Extracts the value part of a declaration and normalizes spacing around comments.
///
/// # Arguments
/// * `decl_source` - Full declaration source (e.g., `margin: 10px /* test */ 20px;`)
///
/// # Returns
/// * `Some(normalized_value)` if extraction successful (e.g., `10px /* test */ 20px`)
/// * `None` if extraction failed (no colon found)
///
/// # Example
/// ```ignore
/// let source = "margin: 10px  /* test */  20px;";
/// assert_eq!(extract_value_with_comments(source), Some("10px /* test */ 20px".to_string()));
/// ```
pub fn extract_value_with_comments(decl_source: &str) -> Option<String> {
    if let Some(colon_pos) = decl_source.find(':') {
        let value_with_ws = &decl_source[colon_pos + 1..];
        let normalized = normalize_value_spacing(value_with_ws);
        Some(normalized)
    } else {
        None
    }
}

/// Normalize CSS whitespace in extracted source text
///
/// Single-pass normalization that:
/// - Collapses consecutive whitespace to single spaces
/// - Removes spaces after opening parentheses: `( expr` → `(expr`
/// - Removes spaces before closing parentheses: `expr )` → `expr)`
/// - Preserves content inside quoted strings (`'...'` and `"..."`)
/// - Preserves content inside CSS comments (`/* ... */`)
///
/// This matches Prettier's normalization for calc(), var(), and other CSS functions.
///
/// # Example
/// ```ignore
/// assert_eq!(
///     normalize_css_whitespace("10px  /* test */  20px"),
///     "10px /* test */ 20px"
/// );
/// assert_eq!(
///     normalize_css_whitespace("var( --a, /* comment */ red )"),
///     "var(--a, /* comment */ red)"
/// );
/// assert_eq!(
///     normalize_css_whitespace("url( 'path with spaces' )"),
///     "url('path with spaces')"
/// );
/// ```
pub fn normalize_css_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut in_string = false;
    let mut string_delim = '\0';
    let mut in_comment = false;
    let mut pending_space = false;

    while let Some(ch) = chars.next() {
        // Check for comment start (outside strings)
        if !in_string && !in_comment && ch == '/' && chars.peek() == Some(&'*') {
            // Add space before comment if preceded by non-whitespace (except `(`)
            // This normalizes `foo,/*` → `foo, /*`
            if !result.is_empty() && !result.ends_with(' ') && !result.ends_with('(') {
                result.push(' ');
            }
            pending_space = false;
            result.push('/');
            chars.next(); // consume '*'
            result.push('*');
            in_comment = true;
            continue;
        }

        // Check for comment end
        if in_comment && ch == '*' && chars.peek() == Some(&'/') {
            result.push('*');
            chars.next(); // consume '/'
            result.push('/');
            in_comment = false;
            pending_space = true; // Space needed before next token
            continue;
        }

        // Inside comment - preserve everything
        if in_comment {
            result.push(ch);
            continue;
        }

        // String delimiter handling (outside comments)
        if !in_string && (ch == '\'' || ch == '"') {
            if pending_space && !result.is_empty() && !result.ends_with('(') {
                result.push(' ');
            }
            pending_space = false;
            in_string = true;
            string_delim = ch;
            result.push(ch);
            continue;
        }

        if in_string && ch == string_delim {
            in_string = false;
            result.push(ch);
            pending_space = false;
            continue;
        }

        // Inside string - preserve everything
        if in_string {
            result.push(ch);
            continue;
        }

        // Opening paren - skip following whitespace
        if ch == '(' {
            if pending_space && !result.is_empty() {
                result.push(' ');
            }
            pending_space = false;
            result.push(ch);
            // Skip whitespace after opening paren
            while chars.peek().is_some_and(|&c| c.is_whitespace()) {
                chars.next();
            }
            continue;
        }

        // Closing paren - remove trailing whitespace
        if ch == ')' {
            while result.ends_with(' ') {
                result.pop();
            }
            result.push(ch);
            pending_space = false;
            continue;
        }

        // Whitespace - mark pending (collapse consecutive)
        if ch.is_whitespace() {
            if !result.is_empty() && !result.ends_with('(') {
                pending_space = true;
            }
            continue;
        }

        // Regular character - add pending space if needed
        if pending_space && !result.is_empty() {
            result.push(' ');
            pending_space = false;
        }
        result.push(ch);
    }

    result.trim().to_string()
}

/// Normalize spacing in a value containing comments (alias for backward compatibility)
#[inline]
pub fn normalize_value_spacing(value: &str) -> String {
    normalize_css_whitespace(value)
}

/// Extract the content between a function's parentheses from source
///
/// Given source like `property: func_name(arg1, arg2)` and func_name `func_name`,
/// returns `Some("arg1, arg2")`. Returns `None` if the function can't be found.
pub fn extract_function_args<'a>(source: &'a str, func_name: &str) -> Option<&'a str> {
    let func_start = source.find(func_name)?;
    let after_name = &source[func_start + func_name.len()..];
    let open_paren = after_name.find('(')?;

    let inner_start = func_start + func_name.len() + open_paren + 1;
    let inner_content = &source[inner_start..];

    // Find closing paren (handle nested parens)
    let mut depth = 1;
    for (i, c) in inner_content.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&inner_content[..i]);
                }
            }
            _ => {}
        }
    }

    None
}

/// Split by top-level spaces, preserving content inside parentheses, quotes, and comments
///
/// Used for space-separated values like `var(--b) color-mix(...)`.
/// Returns individual values that can be wrapped independently.
pub fn split_by_space_preserving_parens(content: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: u32 = 0;
    let mut start = 0;
    let mut in_comment = false;
    let mut in_quote = false;
    let mut quote_char = b'\0';
    let bytes = content.as_bytes();

    let mut i = 0;
    while i < content.len() {
        // Check for comment start (outside quotes)
        if !in_quote
            && !in_comment
            && i + 1 < content.len()
            && bytes[i] == b'/'
            && bytes[i + 1] == b'*'
        {
            in_comment = true;
            i += 2;
            continue;
        }
        // Check for comment end
        if in_comment && i + 1 < content.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
            in_comment = false;
            i += 2;
            continue;
        }
        if in_comment {
            i += 1;
            continue;
        }

        // Handle quotes
        if !in_quote && (bytes[i] == b'\'' || bytes[i] == b'"') {
            in_quote = true;
            quote_char = bytes[i];
            i += 1;
            continue;
        }
        if in_quote && bytes[i] == quote_char {
            in_quote = false;
            i += 1;
            continue;
        }
        if in_quote {
            i += 1;
            continue;
        }

        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b' ' | b'\t' if depth == 0 => {
                let part = &content[start..i];
                if !part.trim().is_empty() {
                    parts.push(part.trim());
                }
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    // Don't forget the last part
    if start < content.len() {
        let part = &content[start..];
        if !part.trim().is_empty() {
            parts.push(part.trim());
        }
    }

    parts
}

/// Split function arguments by top-level commas, preserving nested parens, quotes, and comments
///
/// Used when extracting function arguments from source while preserving comments.
/// Handles nested parentheses correctly so `func(a, b)` inside an arg isn't split.
/// Skips over block comments so commas inside `/* a, b */` aren't treated as separators.
/// Skips over quoted strings so commas inside `"a, b"` aren't treated as separators.
pub fn split_args_by_comma(content: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut depth: u32 = 0;
    let mut start = 0;
    let mut in_comment = false;
    let mut in_quote = false;
    let mut quote_char = b'\0';
    let bytes = content.as_bytes();

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

        // Handle quotes
        if !in_quote && (bytes[i] == b'\'' || bytes[i] == b'"') {
            in_quote = true;
            quote_char = bytes[i];
            i += 1;
            continue;
        }
        if in_quote && bytes[i] == quote_char {
            in_quote = false;
            i += 1;
            continue;
        }
        if in_quote {
            i += 1;
            continue;
        }

        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                args.push(&content[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    // Don't forget the last argument
    if start < content.len() {
        args.push(&content[start..]);
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_raw_value() {
        let source = "color: #ff0000;";
        let span = Span { start: 7, end: 14 };
        assert_eq!(extract_raw_value(source, span), "#ff0000");
    }

    #[test]
    fn test_extract_function_args() {
        assert_eq!(
            extract_function_args("prop: var(--a, red)", "var"),
            Some("--a, red")
        );
        assert_eq!(
            extract_function_args("prop: var(--a, /* comment */ red)", "var"),
            Some("--a, /* comment */ red")
        );
        // Nested parens
        assert_eq!(
            extract_function_args("prop: var(--a, calc(1 + 2))", "var"),
            Some("--a, calc(1 + 2)")
        );
        // Function not found
        assert_eq!(extract_function_args("prop: red", "var"), None);
    }

    #[test]
    fn test_split_args_by_comma() {
        assert_eq!(split_args_by_comma("a, b, c"), vec!["a", " b", " c"]);
        assert_eq!(split_args_by_comma("--a, red"), vec!["--a", " red"]);
        // Nested parens preserved
        assert_eq!(
            split_args_by_comma("--a, calc(1, 2)"),
            vec!["--a", " calc(1, 2)"]
        );
        // Single arg
        assert_eq!(split_args_by_comma("--a"), vec!["--a"]);
        // Empty
        assert_eq!(split_args_by_comma(""), Vec::<&str>::new());
        // Commas inside comments are NOT separators
        assert_eq!(
            split_args_by_comma("--a, /* with, comma */ red"),
            vec!["--a", " /* with, comma */ red"]
        );
        assert_eq!(
            split_args_by_comma("/* a, b */ value"),
            vec!["/* a, b */ value"]
        );
        // Commas inside quotes are NOT separators
        assert_eq!(
            split_args_by_comma(r#"--font, "Font, Name""#),
            vec!["--font", r#" "Font, Name""#]
        );
        assert_eq!(
            split_args_by_comma(r#"'a, b', 'c, d'"#),
            vec!["'a, b'", " 'c, d'"]
        );
    }
}
