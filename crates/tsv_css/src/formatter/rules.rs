// CSS rule and declaration formatting
//
// Handles formatting of:
// - CSS rules (selector + declarations block)
// - CSS declarations (property: value;)
// - Future: At-rules (@media, @keyframes, etc.)

use crate::ast::internal;
use crate::formatter::Formatter;

impl Formatter {
    /// Format a CSS rule (selector + declarations block)
    pub(super) fn format_css_rule(&mut self, rule: &internal::CssRule) {
        // Format selector
        self.write(&rule.selector);
        self.write(" {\n");

        // Format declarations with indentation
        self.indent_level += 1;
        for decl in &rule.declarations {
            self.format_css_declaration(decl);
        }
        self.indent_level -= 1;

        // Closing brace
        self.write("}");
    }

    /// Format a CSS declaration (property: value;)
    fn format_css_declaration(&mut self, decl: &internal::CssDeclaration) {
        self.write_indent();
        self.write(&decl.property);
        self.write(": ");

        // Normalize quotes in value
        let normalized_value = normalize_value_quotes(&decl.value);
        self.write(&normalized_value);

        self.write(";\n");
    }
}

/// Normalize quote style in CSS values (matches prettier behavior)
///
/// Rules:
/// 1. Prefer single quotes by default
/// 2. Use double quotes if string contains single quote (fewer escapes)
/// 3. Keep whichever quote style minimizes escapes
fn normalize_value_quotes(value: &str) -> String {
    let mut result = String::new();
    let mut chars = value.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '"' || ch == '\'' {
            // Found a quoted string - extract and normalize it
            let quote = ch;
            let (content, new_quote) = extract_and_normalize_string(&mut chars, quote);
            result.push(new_quote);
            result.push_str(&content);
            result.push(new_quote);
        } else {
            result.push(ch);
        }
    }

    result
}

/// Extract string content and determine optimal quote style
fn extract_and_normalize_string(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    opening_quote: char,
) -> (String, char) {
    let mut content = String::new();
    let mut decoded = String::new();

    while let Some(ch) = chars.next() {
        if ch == opening_quote {
            // End of string found
            break;
        } else if ch == '\\' {
            // Escape sequence
            content.push(ch);
            if let Some(escaped) = chars.next() {
                content.push(escaped);

                // Decode escape for analysis
                match escaped {
                    '\'' => decoded.push('\''),
                    '"' => decoded.push('"'),
                    '\\' => decoded.push('\\'),
                    'n' => decoded.push('\n'),
                    'r' => decoded.push('\r'),
                    't' => decoded.push('\t'),
                    // For other escapes (like \0001F4A9), keep them as-is in decoded
                    _ => {
                        decoded.push('\\');
                        decoded.push(escaped);
                    }
                }
            }
        } else {
            content.push(ch);
            decoded.push(ch);
        }
    }

    // Count quotes in decoded content
    let single_count = decoded.chars().filter(|&c| c == '\'').count();
    let double_count = decoded.chars().filter(|&c| c == '"').count();

    // Choose optimal quote style
    let optimal_quote = if single_count > 0 && double_count == 0 {
        // Contains ' but no " → use double quotes
        '"'
    } else if double_count > 0 && single_count == 0 {
        // Contains " but no ' → use single quotes
        '\''
    } else if single_count > double_count {
        // More ' than " → use double quotes
        '"'
    } else {
        // Default or equal → prefer single quotes
        '\''
    };

    // Re-encode content with optimal quotes
    let reencoded = reencode_string_content(&decoded, optimal_quote);

    (reencoded, optimal_quote)
}

/// Re-encode string content with the given quote style
///
/// This preserves CSS escape sequences (like \0001F4A9) that were kept during decoding
fn reencode_string_content(decoded: &str, quote: char) -> String {
    let mut result = String::new();
    let mut chars = decoded.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\'' if quote == '\'' => result.push_str("\\'"),
            '"' if quote == '"' => result.push_str("\\\""),
            '\\' => {
                // Backslash in decoded content - check if it's part of a preserved CSS escape
                if let Some(&next_ch) = chars.peek() {
                    if next_ch.is_ascii_digit() || next_ch.is_ascii_hexdigit() {
                        // This is a CSS unicode escape like \0001F4A9 - preserve it
                        result.push('\\');
                        result.push(chars.next().unwrap());
                    } else if next_ch == '\\' {
                        // This is \\  in the decoded content - another backslash was preserved
                        result.push('\\');
                        result.push(chars.next().unwrap());
                    } else {
                        // Regular backslash - needs escaping
                        result.push_str("\\\\");
                    }
                } else {
                    // Backslash at end - needs escaping
                    result.push_str("\\\\");
                }
            }
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            _ => result.push(ch),
        }
    }

    result
}
