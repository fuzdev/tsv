// Value normalization utilities: semantic value formatting for the CSS printer.
//
// Internal AST stores semantic data + spans. When formatting we usually format
// semantically (normalize spacing, apply prettier rules); raw-source extraction
// uses `span.extract(source)` directly at the callsite when needed.

use crate::ast::internal::{Color, ColorChannel};
use tsv_lang::Span;
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
pub(crate) fn normalize_dimension_from_source(raw: &str) -> String {
    let (num_part, unit_part) = split_number_and_unit(raw);

    // Not a number we recognize (e.g. a bare identifier) — leave untouched.
    if num_part.is_empty() {
        return raw.to_string();
    }

    let normalized_num = normalize_css_number(num_part);
    format!("{normalized_num}{unit_part}")
}

/// Split a dimension into its numeric part and trailing unit, e.g.
/// `1.5px` → (`1.5`, `px`), `1.png` → (`1`, `.png`).
fn split_number_and_unit(raw: &str) -> (&str, &str) {
    raw.split_at(crate::number::number_part_len(raw))
}

/// Normalize a CSS number to match prettier's `printNumber` / `printCssNumber`.
///
/// Mantissa: add a leading zero (`.5` → `0.5`), trim trailing fraction zeros
/// and a trailing dot (`1.50` → `1.5`, `1.` → `1`), preserve sign and leading
/// integer zeros. Exponent: lowercase `e`, drop a `+` sign, strip leading
/// zeros (`e+0010` → `e10`), and drop a zero exponent entirely (`5e0` → `5`).
fn normalize_css_number(num: &str) -> String {
    let (mantissa, exponent) = match num.find(['e', 'E']) {
        Some(idx) => (&num[..idx], &num[idx + 1..]),
        None => (num, ""),
    };

    let normalized_mantissa = normalize_decimal_preserving_prefix(mantissa);

    if exponent.is_empty() {
        return normalized_mantissa;
    }

    let (exp_sign, exp_digits) = if let Some(rest) = exponent.strip_prefix('-') {
        ("-", rest)
    } else if let Some(rest) = exponent.strip_prefix('+') {
        ("", rest)
    } else {
        ("", exponent)
    };

    let trimmed_digits = exp_digits.trim_start_matches('0');
    if trimmed_digits.is_empty() {
        // Exponent is zero (`5e0`, `5e-00`) — drop it entirely.
        return normalized_mantissa;
    }

    format!("{normalized_mantissa}e{exp_sign}{trimmed_digits}")
}

/// Known CSS units (lowercase), used to gate number normalization in raw
/// prelude text — only a number with a known unit (or no unit) is normalized,
/// matching prettier's `adjustNumbers` (which checks `css-units-list`).
static CSS_UNITS: phf::Set<&'static str> = phf::phf_set! {
    // Absolute length
    "px", "cm", "mm", "in", "pt", "pc", "q",
    // Font-relative length
    "em", "rem", "ex", "rex", "ch", "rch", "cap", "rcap", "ic", "ric", "lh", "rlh",
    // Viewport-relative length
    "vw", "vh", "vi", "vb", "vmin", "vmax",
    "svw", "svh", "svi", "svb", "svmin", "svmax",
    "lvw", "lvh", "lvi", "lvb", "lvmin", "lvmax",
    "dvw", "dvh", "dvi", "dvb", "dvmin", "dvmax",
    // Container-relative length
    "cqw", "cqh", "cqi", "cqb", "cqmin", "cqmax",
    // Angle
    "deg", "grad", "rad", "turn",
    // Time
    "s", "ms",
    // Frequency
    "hz", "khz",
    // Resolution
    "dpi", "dpcm", "dppx", "x",
    // Flex / grid
    "fr",
};

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

fn is_known_css_unit(unit: &str) -> bool {
    // Fast path: units arrive lowercase, so probe directly and only allocate a
    // lowercased copy when the input actually has uppercase ASCII.
    CSS_UNITS.contains(unit)
        || (unit.bytes().any(|b| b.is_ascii_uppercase())
            && CSS_UNITS.contains(unit.to_ascii_lowercase().as_str()))
}

/// Can `ch` begin a CSS identifier? (letter, `_`, `$`, `@`, or non-ASCII)
fn is_ident_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_' || ch == '$' || ch == '@' || !ch.is_ascii()
}

/// Can `ch` continue a CSS identifier? (`is_ident_start` plus digits and `-`)
fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit() || ch == '-'
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
pub(crate) fn format_color_value(color: &Color) -> String {
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

/// Format a computed `f64` for CSS output: a whole number drops its fraction
/// (`1.0` → `1`), otherwise the default float rendering. (Distinct from
/// `normalize_css_number`, which canonicalizes a number's *source text*.)
fn format_css_f64(v: f64) -> String {
    if v.fract() == 0.0 {
        (v as i64).to_string()
    } else {
        v.to_string()
    }
}

/// Format a ColorChannel value
fn format_color_channel(channel: &ColorChannel) -> String {
    match channel {
        ColorChannel::Number(n) => format_css_f64(*n),
        ColorChannel::Percentage(p) => format!("{}%", format_css_f64(*p)),
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
pub(crate) fn format_color_from_source(color: &Color, source: &str, span: Span) -> String {
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
                        // Preserve original function name with slash syntax
                        format!("{func_name}({r_str} {g_str} {b_str} / {a_str})")
                    } else {
                        // Preserve original function name with comma syntax
                        format!("{func_name}({r_str}, {g_str}, {b_str}, {a_str})")
                    }
                } else if has_comma {
                    format!("{func_name}({r_str}, {g_str}, {b_str})")
                } else {
                    format!("{func_name}({r_str} {g_str} {b_str})")
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
                        // Preserve original function name with slash syntax
                        format!("{func_name}({hue_str} {sat_str} {light_str} / {a_str})")
                    } else {
                        // Preserve original function name with comma syntax
                        format!("{func_name}({hue_str}, {sat_str}, {light_str}, {a_str})")
                    }
                } else if has_comma {
                    format!("{func_name}({hue_str}, {sat_str}, {light_str})")
                } else {
                    format!("{func_name}({hue_str} {sat_str} {light_str})")
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
pub(crate) fn normalize_css_whitespace(s: &str) -> String {
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

        // Comma - no space before, single space after (CSS never wants a space
        // before a comma, e.g. a media-query list `projection, tv`).
        if ch == ',' {
            while result.ends_with(' ') {
                result.pop();
            }
            result.push(ch);
            pending_space = true;
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
pub(crate) fn normalize_value_spacing(value: &str) -> String {
    normalize_css_whitespace(value)
}

/// Extract the content between a function's parentheses from source
///
/// Given source like `property: func_name(arg1, arg2)` and func_name `func_name`,
/// returns `Some("arg1, arg2")`. Returns `None` if the function can't be found.
pub(crate) fn extract_function_args<'a>(source: &'a str, func_name: &str) -> Option<&'a str> {
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
pub(crate) fn split_by_space_preserving_parens(content: &str) -> Vec<&str> {
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
pub(crate) fn split_args_by_comma(content: &str) -> Vec<&str> {
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
    fn test_normalize_css_number_table() {
        // Mantissa: leading-zero insertion, trailing-zero/dot trimming, with
        // leading integer zeros and the negative-zero sign preserved.
        assert_eq!(normalize_css_number(".5"), "0.5");
        assert_eq!(normalize_css_number("5."), "5");
        assert_eq!(normalize_css_number("1.50"), "1.5");
        assert_eq!(normalize_css_number("00.500"), "00.5");
        assert_eq!(normalize_css_number("-0.0"), "-0");
        // Exponent: lowercase `e`, drop `+`, strip leading zeros, drop a zero exponent.
        assert_eq!(normalize_css_number("5e0"), "5");
        assert_eq!(normalize_css_number("1e+0010"), "1e10");
        assert_eq!(normalize_css_number("1.5E-3"), "1.5e-3");
        // A bare trailing `e` (no exponent digits) drops to the mantissa.
        assert_eq!(normalize_css_number("1e"), "1");
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
            split_args_by_comma(r"'a, b', 'c, d'"),
            vec!["'a, b'", " 'c, d'"]
        );
    }
}
