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

use std::borrow::Cow;

use numbers::{canonical_unit, is_known_css_unit, normalize_css_number};
use tsv_lang::printing::format_string_literal;
use tsv_lang::source_scan::{TriviaProfile, find_char};

/// Find the declaration's `property : value` colon — the first `:` that is not
/// inside a comment or string. A property comment may itself contain a colon
/// (`color /* x:y */: red`), which a naive `find(':')` would mis-match. Shared by
/// the printer's declaration sites (`declarations.rs`, `mod.rs`).
pub(crate) fn find_declaration_colon(decl_source: &str) -> Option<usize> {
    find_char(
        decl_source.as_bytes(),
        0,
        decl_source.len(),
        b':',
        TriviaProfile::CSS,
    )
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
                // `canonical_unit` lowercases a known unit (`PX`→`px`) and leaves the
                // `n`/empty cases untouched (neither is a known unit).
                out.push_str(&canonical_unit(unit));
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

/// Lowercase the **feature name** in an `@media`/`@import` media-query string,
/// matching prettier — which lowercases the `media-feature` name (`MIN-WIDTH` →
/// `min-width`) but preserves media types (`SCREEN`), the `and`/`or`/`not`/`only`
/// keywords, and feature *values* (`(orientation: LANDSCAPE)` keeps `LANDSCAPE`).
/// Run **after** [`normalize_value_text`] (numbers/units/strings already
/// canonicalized); this only adjusts identifier case.
///
/// Scope: only a **simple** parenthesized feature expression has its name lowercased —
/// a `(...)` group whose only nested `(`, if any, opens a **function call** in the
/// value (`(min-width: calc(…))`, `(width: min(…))`). A grouped/complex condition — a
/// nested `(` that opens a sub-condition (`(not (hover))`, `((a) and (b))`) — is left
/// verbatim, matching prettier's media-query parser, which treats those as
/// `media-unknown`. (A function-call `(` is told apart from a sub-condition `(` by
/// whether it immediately follows an identifier; see `scan_paren_group`.) (One small
/// divergence:
/// prettier's parser partially lowercases the *first* feature in `((A) and (B))`; tsv
/// preserves the whole grouped condition for consistency — see
/// `media_grouped_feature_case_prettier_divergence`.)
///
/// Within a simple expression the feature name is the identifier in **name
/// position** — before the `:` (plain feature), or anywhere there's no `:` at all
/// (boolean `(hover)` and range `(width >= 600px)` / `(600px <= width)`, whose values
/// are numeric). The value after a `:` is preserved. A case-sensitive custom-media
/// name (`--*`) is preserved (see [`maybe_lowercase_feature_name`]).
pub(crate) fn lowercase_media_feature_names(query: &str) -> Cow<'_, str> {
    let bytes = query.as_bytes();
    // Cheap bail: nothing to lowercase without an uppercase ASCII letter.
    if !bytes.iter().any(u8::is_ascii_uppercase) {
        return Cow::Borrowed(query);
    }

    let mut out = String::with_capacity(query.len());
    let mut i = 0;
    while i < query.len() {
        // Comments/strings copy through verbatim (never lowercase their contents).
        if let Some(end) = trivia_span_at(query, i) {
            out.push_str(&query[i..end]);
            i = end;
            continue;
        }
        match bytes[i] {
            b'(' => {
                // A top-level `(` opens a media-feature-expression. A group that
                // contains a nested *sub-condition* `(` is grouped/complex — copy it
                // verbatim (prettier's parser treats it as `media-unknown`); a nested
                // function-call `(` in the value (`calc(`) does not count.
                let (end, has_nested) = scan_paren_group(query, i);
                if has_nested {
                    out.push_str(&query[i..end]);
                } else {
                    lowercase_simple_feature_expr(&query[i..end], &mut out);
                }
                i = end;
            }
            _ => {
                // Depth-0 content (media types, `and`/`or`/`not`/`only`): verbatim.
                let ch = query[i..].chars().next().unwrap_or('\0');
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    Cow::Owned(out)
}

/// Index just past a `/* … */` block comment starting at `start` (or end of input
/// for an unterminated one).
fn skip_block_comment(s: &str, start: usize) -> usize {
    let bytes = s.as_bytes();
    let mut i = start + 2;
    while i < s.len() && !(bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/')) {
        i += 1;
    }
    (i + 2).min(s.len())
}

/// Index just past a `'…'`/`"…"` string starting at `start` (backslash-aware; or end
/// of input for an unterminated one).
fn skip_string(s: &str, start: usize) -> usize {
    let bytes = s.as_bytes();
    let quote = bytes[start];
    let mut i = start + 1;
    while i < s.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == quote {
            return i + 1;
        }
        i += 1;
    }
    s.len()
}

/// If a `/* … */` comment or a `'…'`/`"…"` string starts at byte `i`, return the index
/// just past it (so `s[i..end]` is the whole trivia token); otherwise `None`. The
/// single source of truth for "what is skippable trivia" shared by the media-feature
/// scanners below — each copies it through or skips it, but they must all agree on its
/// extent and on what opens it.
fn trivia_span_at(s: &str, i: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    match bytes[i] {
        b'/' if bytes.get(i + 1) == Some(&b'*') => Some(skip_block_comment(s, i)),
        b'"' | b'\'' => Some(skip_string(s, i)),
        _ => None,
    }
}

/// Whether `b` is a byte that can end a CSS identifier (a function name, right before
/// its `(`) — ASCII alphanumeric, `-`, `_`, or any non-ASCII byte (part of a
/// multi-byte ident char). Used to tell a function-call `(` (`calc(`, `min(`) from the
/// `(` that opens a grouped sub-condition.
///
/// This mirrors the CSS Syntax 3 tokenizer (§"Consume an ident-like token"): an ident
/// sequence *immediately* followed by `(` is consumed as a `<function-token>`. So a `(`
/// preceded by an ident byte is a function call; a `(` preceded by whitespace/`(`/a
/// connector opens a `( <media-condition> )` per the Media Queries 4 grammar.
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b >= 0x80
}

/// Scan a parenthesized group starting at the `(` at `open`. Returns
/// `(index_just_past_the_matching_close_paren, contains_a_nested_sub_condition)`,
/// skipping comments/strings. A nested `(` that immediately follows an identifier byte
/// is a function call in the value (`calc(`) and does **not** set the flag; only a `(`
/// opening a sub-condition does. For an unbalanced group, returns end-of-input.
fn scan_paren_group(s: &str, open: usize) -> (usize, bool) {
    let bytes = s.as_bytes();
    let mut i = open + 1;
    let mut depth = 1usize;
    let mut has_nested = false;
    while i < s.len() {
        // A `(`/`)` inside a comment or string doesn't change paren depth.
        if let Some(end) = trivia_span_at(s, i) {
            i = end;
            continue;
        }
        match bytes[i] {
            b'(' => {
                // A `(` immediately after an identifier byte is a function call inside
                // a feature value (`calc(`, `min(`, `env(`), not a grouped
                // sub-condition — it must NOT make the feature opaque, since prettier
                // still lowercases the feature name (`(MIN-WIDTH: calc(…))` →
                // `(min-width: calc(…))`). Only a `(` that opens a real sub-condition
                // (preceded by `(`, whitespace, or a connector keyword) marks the group
                // as grouped/complex.
                let is_function_call = i > 0 && is_ident_byte(bytes[i - 1]);
                if !is_function_call {
                    has_nested = true;
                }
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
                if depth == 0 {
                    return (i, has_nested);
                }
            }
            _ => i += 1,
        }
    }
    (s.len(), has_nested)
}

/// Emit a simple media-feature expression `(…)` (no nested parens), lowercasing the
/// feature name. See [`lowercase_media_feature_names`] for the name-position rule.
fn lowercase_simple_feature_expr(group: &str, out: &mut String) {
    let bytes = group.as_bytes();
    let mut i = 0;
    let mut seen_colon = false;
    while i < group.len() {
        // Comments/strings copy through verbatim (a `:` inside one isn't the
        // name/value separator).
        if let Some(end) = trivia_span_at(group, i) {
            out.push_str(&group[i..end]);
            i = end;
            continue;
        }
        match bytes[i] {
            b':' => {
                seen_colon = true;
                out.push(':');
                i += 1;
            }
            _ => {
                let ch = group[i..].chars().next().unwrap_or('\0');
                // An identifier (incl. a leading `-` for custom media / vendor).
                if is_ident_start(ch) || ch == '-' {
                    let start = i;
                    i += ch.len_utf8();
                    while let Some(c) = group[i..].chars().next() {
                        if is_ident_continue(c) {
                            i += c.len_utf8();
                        } else {
                            break;
                        }
                    }
                    let ident = &group[start..i];
                    // Before the `:` (or no `:` at all — boolean/range, numeric
                    // values) the identifier is the feature name; after it, a value.
                    if seen_colon {
                        out.push_str(ident);
                    } else {
                        out.push_str(&maybe_lowercase_feature_name(ident));
                    }
                } else {
                    out.push(ch);
                    i += ch.len_utf8();
                }
            }
        }
    }
}

/// Lowercase a media-feature name unless it's case-sensitive: preserve a custom media
/// (`--*` / `:--*`), which is case-sensitive per CSS Variables 1. An already-lowercase
/// name borrows unchanged.
fn maybe_lowercase_feature_name(name: &str) -> Cow<'_, str> {
    if name.starts_with("--")
        || name.starts_with(":--")
        || !name.bytes().any(|b| b.is_ascii_uppercase())
    {
        return Cow::Borrowed(name);
    }
    Cow::Owned(name.to_ascii_lowercase())
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
/// The common case (a bare property name with no comment and no significant
/// trailing whitespace) returns a `Cow::Borrowed` sub-slice of `decl_source` — no
/// per-declaration allocation. Only the comment-bearing and hex-escape-terminator
/// forms reconstruct an owned `String`.
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
pub(crate) fn extract_property_name(decl_source: &str) -> Cow<'_, str> {
    let Some(colon_pos) = find_declaration_colon(decl_source) else {
        // Fallback: no colon found (malformed declaration).
        return Cow::Borrowed(decl_source.trim());
    };
    let property_part = &decl_source[..colon_pos];

    // Check if property contains a comment
    if let Some(comment_start) = property_part.find("/*") {
        if let Some(comment_end_rel) = property_part[comment_start..].find("*/") {
            let comment_end = comment_start + comment_end_rel + 2; // Include */
            // Extract parts
            let before_comment = property_part[..comment_start].trim();
            let comment = &property_part[comment_start..comment_end];

            // Normalize: property + space + comment (no trailing space)
            Cow::Owned(format!("{before_comment} {comment}"))
        } else {
            // Malformed comment - just trim
            Cow::Borrowed(property_part.trim())
        }
    } else {
        // No comment: trim insignificant whitespace, but a property name ending in a
        // hex escape (`\41`) consumes one following whitespace as the escape's
        // terminator. That whitespace is part of the identifier token, so prettier
        // keeps it before the `:` (`\41 : red`); any extra whitespace is still trimmed
        // (`color : red` → `color: red`).
        let bare = property_part.trim();
        if property_part.ends_with(char::is_whitespace) && ends_with_hex_escape(bare) {
            Cow::Owned(format!("{bare} "))
        } else {
            Cow::Borrowed(bare)
        }
    }
}

/// Canonicalize a property name's case: standard CSS property names are ASCII
/// case-insensitive (CSS Syntax 3), and prettier lowercases them (`COLOR`→`color`,
/// `Background-Color`→`background-color`, vendor `-WEBKIT-…`→`-webkit-…`). Returns
/// the input unchanged for the case-sensitive / non-standard / ambiguous forms:
/// - **custom properties** (`--*`) — case-sensitive per CSS Variables 1;
/// - **non-standard property starts** — any name not beginning with an ASCII letter
///   or a vendor-prefix `-`; these aren't CSS property names, so their case is left
///   untouched;
/// - names carrying a **comment** (`color /* c */`) or a **`\` escape** — left
///   verbatim so the lowercasing never touches comment text or an escape's hex
///   digits (both rare in a property position).
///
/// Takes the already-extracted name (see [`extract_property_name`]); only the ASCII
/// letters of a bare identifier are lowercased.
pub(crate) fn lowercase_property_name(name: Cow<'_, str>) -> Cow<'_, str> {
    // Cheapest discriminator first: an all-lowercase name (the overwhelming common
    // case) has nothing to do, so bail before the substring scans below.
    if !name.bytes().any(|b| b.is_ascii_uppercase()) {
        return name;
    }
    // A standard property starts with an ASCII letter or a single vendor-prefix `-`
    // (`--` is a custom property, handled below).
    let standard_start = name
        .as_bytes()
        .first()
        .is_some_and(|&b| b.is_ascii_alphabetic() || b == b'-');
    if !standard_start || name.starts_with("--") || name.contains("/*") || name.contains('\\') {
        return name;
    }
    Cow::Owned(name.to_ascii_lowercase())
}

/// Lowercase an at-rule name (`@MEDIA` → `@media`, `@Font-Face` → `@font-face`),
/// matching prettier — which lowercases **all** at-rule names, including
/// vendor-prefixed ones (`@-WEBKIT-KEYFRAMES` → `@-webkit-keyframes`). An escaped
/// (`\`) name is left verbatim (lowercasing would corrupt an escape's hex digits);
/// an already-lowercase name borrows unchanged. The `@` is written separately by the
/// printer, so `name` is just the keyword.
pub(crate) fn lowercase_at_rule_name(name: &str) -> Cow<'_, str> {
    if !name.bytes().any(|b| b.is_ascii_uppercase()) || name.contains('\\') {
        return Cow::Borrowed(name);
    }
    Cow::Owned(name.to_ascii_lowercase())
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
    if let Some(colon_pos) = find_declaration_colon(decl_source) {
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
    if let Some(colon_pos) = find_declaration_colon(decl_source) {
        let value_with_ws = &decl_source[colon_pos + 1..];
        let normalized = normalize_value_spacing(value_with_ws);
        Some(normalized)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_property_name_borrows_common_case() {
        // The overwhelmingly-common case: a bare property name with no comment and
        // no significant trailing whitespace borrows a sub-slice of the input — no
        // per-declaration allocation.
        for src in [
            "margin: 10px",
            "color:red",
            "--custom-prop: var(--x)",
            "grid-template-columns: 1fr 2fr",
        ] {
            let got = extract_property_name(src);
            assert!(
                matches!(got, Cow::Borrowed(_)),
                "bare property in {src:?} must borrow, got owned {got:?}"
            );
        }
        assert_eq!(extract_property_name("margin: 10px"), "margin");
        assert_eq!(extract_property_name("color:red"), "color");
        assert_eq!(
            extract_property_name("--custom-prop: var(--x)"),
            "--custom-prop"
        );
    }

    #[test]
    fn test_extract_property_name_trims_but_still_borrows() {
        // Insignificant whitespace around the property name is trimmed; the result
        // is still a borrowed sub-slice (trim returns a sub-slice, no alloc).
        let got = extract_property_name("color : red");
        assert_eq!(got, "color");
        assert!(matches!(got, Cow::Borrowed(_)));
    }

    #[test]
    fn test_extract_property_name_comment_owns() {
        // A comment in the property name reconstructs an owned, space-normalized form.
        let got = extract_property_name("color/* comment */:red");
        assert_eq!(got, "color /* comment */");
        assert!(matches!(got, Cow::Owned(_)));
    }

    #[test]
    fn test_extract_property_name_hex_escape_terminator_owns() {
        // A property name ending in a hex escape consumes one trailing whitespace as
        // the escape's terminator — that space is preserved, so the result owns.
        let got = extract_property_name("\\41 : red");
        assert_eq!(got, "\\41 ");
        assert!(matches!(got, Cow::Owned(_)));

        // A non-escape trailing space is trimmed (borrowed).
        let got = extract_property_name("color : red");
        assert_eq!(got, "color");
        assert!(matches!(got, Cow::Borrowed(_)));
    }

    #[test]
    fn test_extract_property_name_no_colon_fallback() {
        // Malformed declaration with no colon: trim the whole source (borrowed).
        let got = extract_property_name("  orphan  ");
        assert_eq!(got, "orphan");
        assert!(matches!(got, Cow::Borrowed(_)));
    }

    #[test]
    fn test_lowercase_property_name() {
        let low = |s: &str| lowercase_property_name(Cow::Borrowed(s)).into_owned();
        // Standard properties + vendor prefixes lowercase.
        assert_eq!(low("COLOR"), "color");
        assert_eq!(low("Background-Color"), "background-color");
        assert_eq!(low("-WEBKIT-Box-Shadow"), "-webkit-box-shadow");
        // Already-lowercase borrows unchanged (no allocation).
        assert!(matches!(
            lowercase_property_name(Cow::Borrowed("color")),
            Cow::Borrowed("color")
        ));
        // Case-sensitive / non-standard names are preserved.
        assert_eq!(low("--MyVar"), "--MyVar"); // custom property
        assert_eq!(low("$fontFamily"), "$fontFamily"); // non-letter start → not a CSS property
        assert_eq!(low("#{$Foo}"), "#{$Foo}"); // non-letter start → preserved
        assert_eq!(low("COLOR /* C */"), "COLOR /* C */"); // comment-bearing
        assert_eq!(low("\\43OLOR"), "\\43OLOR"); // escaped
    }

    #[test]
    fn test_lowercase_at_rule_name() {
        let low = |s: &str| lowercase_at_rule_name(s).into_owned();
        assert_eq!(low("MEDIA"), "media");
        assert_eq!(low("Font-Face"), "font-face");
        assert_eq!(low("-WEBKIT-KEYFRAMES"), "-webkit-keyframes");
        assert_eq!(low("INCLUDE"), "include"); // non-standard directive name, still lowercased
        // Already-lowercase borrows unchanged (no allocation).
        assert!(matches!(
            lowercase_at_rule_name("media"),
            Cow::Borrowed("media")
        ));
        // Escaped names preserved (lowercasing would corrupt the escape's hex digits).
        assert_eq!(low("\\4D edia"), "\\4D edia");
    }

    #[test]
    fn test_lowercase_media_feature_names() {
        let f = |s: &str| lowercase_media_feature_names(s).into_owned();
        // Simple feature expressions: name lowercased, value preserved.
        assert_eq!(f("(MIN-WIDTH: 100px)"), "(min-width: 100px)");
        assert_eq!(f("(ORIENTATION: LANDSCAPE)"), "(orientation: LANDSCAPE)");
        assert_eq!(f("(HOVER)"), "(hover)"); // boolean feature
        assert_eq!(f("(WIDTH >= 600px)"), "(width >= 600px)"); // range, name-first
        assert_eq!(f("(600px <= WIDTH)"), "(600px <= width)"); // range, value-first
        // Media type + keyword preserved (only the feature name lowercases).
        assert_eq!(
            f("SCREEN and (MIN-WIDTH: 1px)"),
            "SCREEN and (min-width: 1px)"
        );
        // Custom media is case-sensitive → preserved.
        assert_eq!(f("(--SMALL-VIEWPORT)"), "(--SMALL-VIEWPORT)");
        // A function-valued feature is still simple — the name lowercases even though
        // the value carries nested parens (`calc(`/`min(`).
        assert_eq!(
            f("(MIN-WIDTH: calc(100px + 1px))"),
            "(min-width: calc(100px + 1px))"
        );
        assert_eq!(f("(WIDTH: min(50%, 100px))"), "(width: min(50%, 100px))");
        // Grouped/complex condition (nested sub-condition parens) left verbatim — even
        // when a nested feature itself has a function value.
        assert_eq!(
            f("((MIN-WIDTH: 1px) and (MAX-WIDTH: 2px))"),
            "((MIN-WIDTH: 1px) and (MAX-WIDTH: 2px))"
        );
        assert_eq!(
            f("((MIN-WIDTH: calc(1px)) and (b))"),
            "((MIN-WIDTH: calc(1px)) and (b))"
        );
        assert_eq!(f("(NOT (HOVER))"), "(NOT (HOVER))");
        // No uppercase → borrows unchanged (no allocation).
        assert!(matches!(
            lowercase_media_feature_names("(min-width: 100px)"),
            Cow::Borrowed(_)
        ));
        // A comment in the expression is preserved; the name still lowercases.
        assert_eq!(f("(MIN-WIDTH: /* c */ 1px)"), "(min-width: /* c */ 1px)");
    }
}
