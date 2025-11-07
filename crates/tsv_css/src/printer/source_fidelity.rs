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
// use crate::printer::source_fidelity::{extract_raw_value, needs_source_extraction};
//
// if needs_source_extraction(&value, value_comments) {
//     let raw = extract_raw_value(source, value.span());
//     output.write(raw);
// } else {
//     format_value_semantic(&value, output);
// }
// ```

use crate::ast::internal::{CssComment, CssValue};
use std::collections::HashMap;
use tsv_lang::Span;

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
    &source[span.start as usize..span.end as usize]
}

/// Check if value needs source extraction (has comments, complex escapes, etc.)
///
/// Use this to decide between source extraction (fidelity) vs semantic formatting (normalization).
///
/// # Arguments
/// * `value` - The CSS value to check
/// * `value_comments` - Side table of value comments (span.start -> comments)
///
/// # Returns
/// * `true` if raw source extraction is needed
/// * `false` if semantic formatting is safe
///
/// # Decision Logic
/// 1. Has value comments (/* comment */ inside property value)? → extract source
/// 2. String with complex escapes? → extract source (TODO: improve detection)
/// 3. Dimension with leading zeros? → extract source (handled via span extraction)
/// 4. Otherwise → use semantic formatting
///
/// # Example
/// ```ignore
/// if needs_source_extraction(&decl.value, &value_comments) {
///     let raw = extract_raw_value(source, decl.span());
///     printer.write(raw);
/// } else {
///     format_value_semantic(&decl.value, printer);
/// }
/// ```
pub fn needs_source_extraction(
    _value: &CssValue,
    value_comments: &HashMap<u32, Vec<CssComment>>,
    decl_span_start: u32,
) -> bool {
    // Check if declaration has value comments
    if value_comments.contains_key(&decl_span_start) {
        return true;
    }

    // TODO: Add detection for:
    // - Complex escape sequences (unicode, newlines, etc.)
    // - Unusual formatting that should be preserved
    // - Other quirks identified in testing

    // For now, default to semantic formatting
    false
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
///
/// # Example
/// ```ignore
/// assert_eq!(format_dimension_value(10.0, "px"), "10px");
/// assert_eq!(format_dimension_value(0.5, "em"), "0.5em");
/// assert_eq!(format_dimension_value(0.0, ""), "0");
/// ```
pub fn format_dimension_value(value: f64, unit: &str) -> String {
    if unit.is_empty() {
        value.to_string()
    } else {
        format!("{}{}", value, unit)
    }
}

/// Format a color value semantically
///
/// Converts a Color AST node to its string representation.
///
/// # Example
/// ```ignore
/// let color = Color::Named("red".to_string());
/// assert_eq!(format_color_value(&color), "red");
///
/// let color = Color::Hex("#ff0000".to_string());
/// assert_eq!(format_color_value(&color), "#ff0000");
/// ```
pub fn format_color_value(color: &crate::ast::internal::Color) -> String {
    use crate::ast::internal::Color;
    match color {
        Color::Named(name) => name.clone(),
        Color::Hex(hex) => hex.clone(),
        Color::Rgb { r, g, b, alpha } => {
            if let Some(a) = alpha {
                format!("rgba({}, {}, {}, {})", r, g, b, a)
            } else {
                format!("rgb({}, {}, {})", r, g, b)
            }
        }
        Color::Hsl {
            hue,
            saturation,
            lightness,
            alpha,
        } => {
            if let Some(a) = alpha {
                format!("hsla({}, {}%, {}%, {})", hue, saturation, lightness, a)
            } else {
                format!("hsl({}, {}%, {}%)", hue, saturation, lightness)
            }
        }
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
    use tsv_lang::printing::{format_string_literal, StringFormatOptions};
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
                format!("{} {}", before_comment, comment)
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
    use tsv_lang::printing::{format_string_literal, StringFormatOptions};

    if let Some(colon_pos) = decl_source.find(':') {
        let value_part = decl_source[colon_pos + 1..].trim();
        // String should be quoted
        if value_part.len() >= 2 {
            let raw_content = &value_part[1..value_part.len() - 1];
            let formatted = format_string_literal(raw_content, quote, StringFormatOptions::default());
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

/// Normalize spacing in a value containing comments
///
/// Collapses multiple spaces while preserving comments.
///
/// # Arguments
/// * `value` - Value string that may contain comments
///
/// # Returns
/// * Normalized value with single spaces between tokens
///
/// # Example
/// ```ignore
/// assert_eq!(
///     normalize_value_spacing("10px  /* test */  20px"),
///     "10px /* test */ 20px"
/// );
/// ```
pub fn normalize_value_spacing(value: &str) -> String {
    let mut result = String::new();
    let mut chars = value.chars().peekable();
    let mut in_comment = false;
    let mut pending_space = false;

    while let Some(c) = chars.next() {
        if !in_comment && c == '/' && chars.peek() == Some(&'*') {
            // Start of comment - add space if needed
            if !result.is_empty() {
                result.push(' ');
            }
            result.push('/');
            result.push(chars.next().unwrap()); // consume '*'
            in_comment = true;
            pending_space = false;
        } else if in_comment && c == '*' && chars.peek() == Some(&'/') {
            // End of comment
            result.push('*');
            result.push(chars.next().unwrap()); // consume '/'
            in_comment = false;
            pending_space = true; // Mark that we need a space before next token
        } else if in_comment {
            // Inside comment - preserve everything
            result.push(c);
        } else if c.is_whitespace() {
            // Outside comment - mark that we might need a space
            if !result.is_empty() {
                pending_space = true;
            }
        } else {
            // Regular character - add pending space if needed
            if pending_space && !result.is_empty() {
                result.push(' ');
                pending_space = false;
            }
            result.push(c);
        }
    }

    result.trim().to_string()
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
    fn test_needs_source_extraction_with_comments() {
        let mut value_comments = HashMap::new();
        value_comments.insert(
            10,
            vec![CssComment {
                content: " test ".to_string(),
                span: Span { start: 15, end: 25 },
            }],
        );

        let value = CssValue::Identifier {
            name: "red".to_string(),
            span: Span { start: 10, end: 13 },
        };

        assert!(needs_source_extraction(&value, &value_comments, 10));
    }

    #[test]
    fn test_needs_source_extraction_no_comments() {
        let value_comments = HashMap::new();

        let value = CssValue::Identifier {
            name: "red".to_string(),
            span: Span { start: 10, end: 13 },
        };

        assert!(!needs_source_extraction(&value, &value_comments, 10));
    }
}
