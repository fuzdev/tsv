// Value normalization utilities: semantic value formatting for the CSS printer.
//
// Internal AST stores semantic data + spans. When formatting we usually format
// semantically (normalize spacing, apply prettier rules); raw-source extraction
// uses `span.extract(source)` directly at the callsite when needed.

mod colors;
mod numbers;
mod splitting;

pub(crate) use colors::format_color_from_source;
pub(crate) use numbers::normalize_dimension_from_source;
pub(crate) use splitting::{
    extract_function_args, normalize_css_whitespace, normalize_value_spacing, split_args_by_comma,
    split_by_space_preserving_parens,
};

use numbers::{is_known_css_unit, normalize_css_number};
use tsv_lang::printing::format_string_literal;

/// Format an identifier value semantically
///
/// # Example
/// ```ignore
/// assert_eq!(format_identifier_value("red"), "red");
/// assert_eq!(format_identifier_value("auto"), "auto");
/// ```
pub(crate) fn format_identifier_value(name: &str) -> String {
    name.to_string()
}

/// Normalize CSS numbers and string quotes within a raw prelude string,
/// mirroring prettier's `adjustNumbers(adjustStrings(...))` for at-rule preludes
/// it parses as values (`@media`/`@supports`). `/* */` comments and `#`-prefixed
/// tokens (hex colors) are copied verbatim. A number is normalized only when it
/// isn't part of an identifier (`min-width` is untouched) and its trailing unit
/// is a known CSS unit or empty (so `1abc` is left alone); unit casing is
/// preserved. Quoted strings get prettier's quote normalization (prefer single,
/// swapping only to minimize escaping) — `@container`, which isn't value-parsed,
/// never reaches this function so its prelude stays raw.
pub(crate) fn normalize_value_text(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];

        // Normalize quoted-string quotes (handling backslash escapes). A properly
        // closed string runs through prettier's quote chooser; an unterminated run
        // (malformed input) is copied verbatim.
        if b == b'"' || b == b'\'' {
            let start = i;
            i += 1;
            let mut closed = false;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    // Skip the backslash and the escaped byte (if any).
                    i += if i + 1 < bytes.len() { 2 } else { 1 };
                    continue;
                }
                closed = bytes[i] == b;
                i += 1;
                if closed {
                    break;
                }
            }
            let literal = &input[start..i];
            if closed {
                let content = &literal[1..literal.len() - 1];
                out.push_str(&format_string_value(content, b as char));
            } else {
                out.push_str(literal);
            }
            continue;
        }

        // Copy block comments verbatim.
        if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let start = i;
            i += 2;
            while i < bytes.len() && !(bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/')) {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            out.push_str(&input[start..i]);
            continue;
        }

        let Some(ch) = input[i..].chars().next() else {
            break;
        };

        // Copy identifiers verbatim — including any digits they contain, so a
        // number attached to a word (`foo2`, `min-width`) is never normalized.
        if is_ident_start(ch) {
            let start = i;
            i += ch.len_utf8();
            while let Some(c) = input[i..].chars().next() {
                if is_ident_continue(c) {
                    i += c.len_utf8();
                } else {
                    break;
                }
            }
            out.push_str(&input[start..i]);
            continue;
        }

        // Copy `#`-prefixed tokens (hex colors) verbatim so exponent-looking
        // hex like `#1e2` isn't mangled.
        if b == b'#' {
            let start = i;
            i += 1;
            while let Some(c) = input[i..].chars().next() {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    i += c.len_utf8();
                } else {
                    break;
                }
            }
            out.push_str(&input[start..i]);
            continue;
        }

        // A number (not attached to an identifier — those are consumed above).
        let num_len = crate::number::number_part_len(&input[i..]);
        if num_len > 0 {
            let num = &input[i..i + num_len];
            i += num_len;
            // Trailing unit: ASCII letters only (matches prettier's unit regex;
            // `%` and operators are not part of the unit).
            let unit_start = i;
            while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                i += 1;
            }
            let unit = &input[unit_start..i];
            if unit.is_empty() || unit.eq_ignore_ascii_case("n") || is_known_css_unit(unit) {
                out.push_str(&normalize_css_number(num));
                out.push_str(unit);
            } else {
                out.push_str(num);
                out.push_str(unit);
            }
            continue;
        }

        out.push(ch);
        i += ch.len_utf8();
    }

    out
}

/// Can `ch` begin a CSS identifier? (letter, `_`, `$`, `@`, or non-ASCII)
fn is_ident_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_' || ch == '$' || ch == '@' || !ch.is_ascii()
}

/// Can `ch` continue a CSS identifier? (`is_ident_start` plus digits and `-`)
fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit() || ch == '-'
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
pub(crate) fn format_string_value(content: &str, quote: char) -> String {
    format_string_literal(content, quote)
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
pub(crate) fn extract_property_name(decl_source: &str) -> String {
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
            // No comment: trim insignificant whitespace, but a property name ending in a
            // hex escape (`\41`) consumes one following whitespace as the escape's
            // terminator. That whitespace is part of the identifier token, so prettier
            // keeps it before the `:` (`\41 : red`); any extra whitespace is still trimmed
            // (`color : red` → `color: red`).
            let bare = property_part.trim();
            if property_part.ends_with(char::is_whitespace) && ends_with_hex_escape(bare) {
                format!("{bare} ")
            } else {
                bare.to_string()
            }
        }
    } else {
        // Fallback: no colon found (malformed declaration)
        decl_source.trim().to_string()
    }
}

/// Returns true if `name` ends with a CSS hex escape (`\` + 1..=6 hex digits).
///
/// Such an escape consumes a single following whitespace as its terminator; that
/// whitespace is part of the identifier token and must be preserved (e.g. the space
/// before `:` in a property name `\41 : red`). A literal char after the escape
/// (`ab\44 cd`) or an escaped backslash (`\\41`) does not end with a live escape.
fn ends_with_hex_escape(name: &str) -> bool {
    let bytes = name.as_bytes();
    // Consume up to 6 trailing hex digits.
    let mut i = bytes.len();
    let mut digits = 0;
    while i > 0 && digits < 6 && bytes[i - 1].is_ascii_hexdigit() {
        i -= 1;
        digits += 1;
    }
    if digits == 0 || i == 0 || bytes[i - 1] != b'\\' {
        return false;
    }
    // The introducing `\` must itself be unescaped: an odd run of backslashes ending
    // here means the last one starts the escape (`\41` yes, `\\41` no).
    let mut backslashes = 0;
    let mut j = i;
    while j > 0 && bytes[j - 1] == b'\\' {
        backslashes += 1;
        j -= 1;
    }
    backslashes % 2 == 1
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
pub(crate) fn extract_string_value(decl_source: &str, quote: char) -> Option<String> {
    if let Some(colon_pos) = decl_source.find(':') {
        let value_part = decl_source[colon_pos + 1..].trim();
        // String should be quoted
        if value_part.len() >= 2 {
            let raw_content = &value_part[1..value_part.len() - 1];
            let formatted = format_string_literal(raw_content, quote);
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
pub(crate) fn extract_value_with_comments(decl_source: &str) -> Option<String> {
    if let Some(colon_pos) = decl_source.find(':') {
        let value_with_ws = &decl_source[colon_pos + 1..];
        let normalized = normalize_value_spacing(value_with_ws);
        Some(normalized)
    } else {
        None
    }
}
