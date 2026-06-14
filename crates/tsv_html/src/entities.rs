//! HTML entity decoding
//!
//! This module provides HTML character reference decoding following the HTML5 spec.
//! It handles:
//! - Named entities (&amp;, &lt;, &nbsp;, etc.)
//! - Numeric decimal entities (&#65;)
//! - Numeric hex entities (&#x41; or &#X41;)
//! - Different parsing rules for attribute values vs text content
//!
//! ## Entity Map Simplification
//!
//! The entity map uses a simplified format matching Svelte's implementation:
//! - Multi-codepoint entities use only the first codepoint (base character)
//! - ~200 entities with combining marks are simplified (e.g., NotEqualTilde)
//! - Source: https://html.spec.whatwg.org/entities.json (first codepoint only)
//! - Generated at compile time from `src/entities.json` by `build.rs`
//!
//! ## Enhancement over Svelte
//!
//! Our decoder is more thorough than Svelte's in one area:
//! - We support uppercase hex entities: `&#X41;` → 'A' (HTML5 spec-compliant)
//! - Svelte only supports lowercase: `&#x41;` → 'A', treats `&#X41;` as literal text
//!
//! References:
//! - https://html.spec.whatwg.org/multipage/parsing.html#named-character-reference-state
//! - https://html.spec.whatwg.org/entities.json
//! - Svelte implementation: packages/svelte/src/compiler/phases/1-parse/utils/html.js

use phf::phf_map;

// Windows-1252 code point replacements for the range 128-159
// Source: http://en.wikipedia.org/wiki/Character_encodings_in_HTML#Illegal_characters
// Also: https://html.spec.whatwg.org/multipage/parsing.html#preprocessing-the-input-stream
const WINDOWS_1252: [u32; 32] = [
    8364, 129, 8218, 402, 8222, 8230, 8224, 8225, 710, 8240, 352, 8249, 338, 141, 381, 143, 144,
    8216, 8217, 8220, 8221, 8226, 8211, 8212, 732, 8482, 353, 8250, 339, 157, 382, 376,
];

// Entity data sourced from: https://html.spec.whatwg.org/entities.json
// Simplified to match Svelte's implementation (first codepoint only)
// Stored in: crates/tsv_html/src/entities.json
// Generated at compile time by build.rs
//
// Note: Multi-codepoint entities with combining marks are simplified to just
// the base character. This matches Svelte's behavior for drop-in compatibility.
// See: scripts/generate_simplified_entities.ts for details
include!(concat!(env!("OUT_DIR"), "/entities_map.rs"));

/// Decode HTML character references in a string.
///
/// This function decodes both named entities and numeric character references.
/// The behavior differs slightly based on whether the input is an attribute value:
///
/// - In attribute values: Named entities without semicolons are only decoded if not
///   followed by `=` or alphanumeric characters (per HTML5 spec)
/// - In text content: All valid entities are decoded
///
/// # Examples
///
/// ```
/// use tsv_html::decode_character_references;
///
/// // Named entities
/// assert_eq!(decode_character_references("&lt;tag&gt;", false), "<tag>");
/// assert_eq!(decode_character_references("&amp; &quot;", false), "& \"");
///
/// // Numeric decimal
/// assert_eq!(decode_character_references("&#65;", false), "A");
///
/// // Numeric hex
/// assert_eq!(decode_character_references("&#x41;", false), "A");
/// assert_eq!(decode_character_references("&#X41;", false), "A");
/// ```
pub fn decode_character_references(html: &str, is_attribute_value: bool) -> String {
    let mut result = String::with_capacity(html.len());
    let mut i = 0;

    while i < html.len() {
        // SAFETY: i < html.len() guarantees at least one char exists
        #[allow(clippy::unwrap_used)]
        let ch = html[i..].chars().next().unwrap();

        if ch != '&' {
            result.push(ch);
            i += ch.len_utf8();
            continue;
        }

        // Try to parse an entity reference starting at '&'
        let rest = &html[i + 1..];

        // Numeric character reference: &#... or &#x...
        if let Some((decoded, consumed)) = decode_numeric_entity(rest) {
            result.push(decoded);
            i += 1 + consumed; // +1 for '&'
            continue;
        }

        // Named character reference
        if let Some((_entity_name, entity_len, decoded_char)) =
            decode_named_entity(rest, is_attribute_value)
        {
            result.push(decoded_char);
            i += 1 + entity_len; // +1 for '&'
            continue;
        }

        // Not a valid entity, keep the '&'
        result.push('&');
        i += 1;
    }

    result
}

/// Decode a numeric character reference (decimal or hex)
///
/// Returns (decoded_char, bytes_consumed) if successful
/// Examples: &#65; &#x41; &#X41; &#0041;
fn decode_numeric_entity(rest: &str) -> Option<(char, usize)> {
    if !rest.starts_with('#') {
        return None;
    }

    let after_hash = &rest[1..];
    let is_hex = after_hash.starts_with('x') || after_hash.starts_with('X');

    let digits_start_offset = if is_hex { 1 } else { 0 };
    let digits_start = &after_hash[digits_start_offset..];

    // Collect hex or decimal digits
    let mut digit_count = 0;
    let mut value_str = String::new();

    for ch in digits_start.chars() {
        if (is_hex && ch.is_ascii_hexdigit()) || (!is_hex && ch.is_ascii_digit()) {
            value_str.push(ch);
            digit_count += 1;
        } else {
            break;
        }
    }

    if digit_count == 0 {
        return None;
    }

    // Parse the number
    let code = if is_hex {
        u32::from_str_radix(&value_str, 16).ok()?
    } else {
        value_str.parse::<u32>().ok()?
    };

    // Check for optional semicolon
    let has_semicolon = digits_start.chars().nth(digit_count) == Some(';');

    // Total consumed: '#' + optional 'x'/'X' + digits + optional ';'
    let total_consumed = 1 + digits_start_offset + digit_count + if has_semicolon { 1 } else { 0 };

    // Validate and convert
    let validated = validate_code(code);
    let decoded = char::from_u32(validated)?;

    Some((decoded, total_consumed))
}

/// Decode a named character reference
///
/// Returns (entity_name, consumed_length, decoded_char) if successful
fn decode_named_entity(rest: &str, is_attribute_value: bool) -> Option<(&str, usize, char)> {
    // Try to match entity names
    // We need to handle both with and without semicolons

    // First, try with semicolon (most common case)
    for (i, ch) in rest.char_indices() {
        if ch == ';' {
            // Try lookup with semicolon included
            let entity_name_with_semi = &rest[..=i];
            if let Some(&codepoint) = ENTITIES.get(entity_name_with_semi) {
                let decoded = char::from_u32(codepoint)?;
                return Some((entity_name_with_semi, i + 1, decoded)); // +1 for semicolon
            }

            // Also try without semicolon for legacy entities
            let entity_name = &rest[..i];
            if let Some(&codepoint) = ENTITIES.get(entity_name) {
                let decoded = char::from_u32(codepoint)?;
                return Some((entity_name, i + 1, decoded)); // +1 for semicolon
            }
            break;
        }
        // Stop at non-alphanumeric
        if !ch.is_alphanumeric() {
            break;
        }
    }

    // Try without semicolon (legacy entities)
    // Per HTML5 spec: in attribute values, only decode if not followed by '=' or alphanumeric
    let mut longest_match: Option<(&str, u32)> = None;
    let mut longest_len = 0;

    for (i, ch) in rest.char_indices() {
        if !ch.is_alphanumeric() {
            break;
        }
        let entity_name = &rest[..=i];
        if let Some(&codepoint) = ENTITIES.get(entity_name) {
            longest_match = Some((entity_name, codepoint));
            longest_len = i + 1;
        }
    }

    if let Some((entity_name, codepoint)) = longest_match {
        // Check if we should decode in attribute context
        if is_attribute_value {
            let next_char = rest.chars().nth(longest_len);
            if let Some(next) = next_char {
                // Don't decode if followed by '=' or alphanumeric
                if next == '=' || next.is_alphanumeric() {
                    return None;
                }
            }
        }

        let decoded = char::from_u32(codepoint)?;
        return Some((entity_name, longest_len, decoded));
    }

    None
}

/// Validate and normalize a Unicode code point per HTML5 spec
///
/// Some code points are verboten and must be replaced or normalized:
/// - Line feed (10) becomes space (32)
/// - Code points 128-159 use Windows-1252 replacements
/// - UTF-16 surrogate halves (D800-DFFF) become NUL
/// - Invalid/unsupported planes become NUL
///
/// References:
/// - http://en.wikipedia.org/wiki/Character_encodings_in_HTML#Illegal_characters
/// - https://en.wikipedia.org/wiki/Plane_(Unicode)
/// - https://html.spec.whatwg.org/multipage/parsing.html#preprocessing-the-input-stream
fn validate_code(code: u32) -> u32 {
    const NUL: u32 = 0;

    // Line feed becomes generic whitespace
    if code == 10 {
        return 32;
    }

    // ASCII range (below 128)
    if code < 128 {
        return code;
    }

    // Code points 128-159: Windows-1252 replacements
    // Browsers handle these leniently, but they're technically incorrect
    if code <= 159 {
        return WINDOWS_1252[(code - 128) as usize];
    }

    // Basic multilingual plane (below D800)
    if code < 0xD800 {
        return code;
    }

    // UTF-16 surrogate halves (D800-DFFF) are invalid
    if code <= 0xDFFF {
        return NUL;
    }

    // Rest of basic multilingual plane (E000-FFFF)
    if code <= 0xFFFF {
        return code;
    }

    // Supplementary multilingual plane (10000-1FFFF)
    if (0x10000..=0x1FFFF).contains(&code) {
        return code;
    }

    // Supplementary ideographic plane (20000-2FFFF)
    if (0x20000..=0x2FFFF).contains(&code) {
        return code;
    }

    // Supplementary special-purpose plane (E0000-E007F and E0100-E01EF)
    if (0xE0000..=0xE007F).contains(&code) || (0xE0100..=0xE01EF).contains(&code) {
        return code;
    }

    // Everything else is invalid
    NUL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_named_entities() {
        assert_eq!(decode_character_references("&lt;", false), "<");
        assert_eq!(decode_character_references("&gt;", false), ">");
        assert_eq!(decode_character_references("&amp;", false), "&");
        assert_eq!(decode_character_references("&quot;", false), "\"");
        assert_eq!(decode_character_references("&nbsp;", false), "\u{00A0}");
        assert_eq!(decode_character_references("&copy;", false), "©");
    }

    #[test]
    fn test_numeric_decimal() {
        assert_eq!(decode_character_references("&#65;", false), "A");
        assert_eq!(decode_character_references("&#8364;", false), "€");
        assert_eq!(decode_character_references("&#128169;", false), "💩");
    }

    #[test]
    fn test_numeric_hex() {
        assert_eq!(decode_character_references("&#x41;", false), "A");
        assert_eq!(decode_character_references("&#X41;", false), "A");
        assert_eq!(decode_character_references("&#x20AC;", false), "€");
        assert_eq!(decode_character_references("&#x1F4A9;", false), "💩");
    }

    #[test]
    fn test_mixed_content() {
        assert_eq!(decode_character_references("&lt;tag&gt;", false), "<tag>");
        assert_eq!(
            decode_character_references("&amp; &quot; &copy;", false),
            "& \" ©"
        );
    }

    #[test]
    fn test_invalid_entities() {
        // Unknown entity - keep as is
        assert_eq!(decode_character_references("&unknown;", false), "&unknown;");
    }

    #[test]
    fn test_validate_code() {
        // Line feed -> space
        assert_eq!(validate_code(10), 32);

        // ASCII
        assert_eq!(validate_code(65), 65); // 'A'

        // Windows-1252 replacement
        assert_eq!(validate_code(128), 8364); // Euro sign

        // Valid BMP
        assert_eq!(validate_code(0x00C6), 0x00C6); // Æ

        // Surrogate halves -> NUL
        assert_eq!(validate_code(0xD800), 0);
        assert_eq!(validate_code(0xDFFF), 0);

        // Valid supplementary plane
        assert_eq!(validate_code(0x1F4A9), 0x1F4A9); // 💩
    }

    #[test]
    fn test_legacy_entities_no_semicolon() {
        // Legacy entities without semicolon should decode in content
        assert_eq!(decode_character_references("&AMP", false), "&");
        assert_eq!(decode_character_references("&COPY", false), "©");
        assert_eq!(decode_character_references("&LT", false), "<");
        assert_eq!(decode_character_references("&GT", false), ">");
    }

    #[test]
    fn test_attribute_context_rules() {
        // In attributes, entities without semicolon must not be followed by '=' or alphanumeric

        // Should NOT decode (followed by '=')
        assert_eq!(
            decode_character_references("&AMP=value", true),
            "&AMP=value"
        );

        // Should NOT decode (followed by alphanumeric)
        assert_eq!(
            decode_character_references("&AMPersand", true),
            "&AMPersand"
        );

        // Should decode (followed by punctuation)
        assert_eq!(decode_character_references("&AMP&more", true), "&&more");

        // Should decode (followed by space)
        assert_eq!(decode_character_references("&AMP ", true), "& ");

        // Should always decode with semicolon
        assert_eq!(decode_character_references("&AMP;=test", true), "&=test");
    }

    #[test]
    fn test_incomplete_entities() {
        // Incomplete entity names should stay as literal text
        assert_eq!(decode_character_references("&am", false), "&am");
        assert_eq!(decode_character_references("&l", false), "&l");
        assert_eq!(decode_character_references("&", false), "&");
    }

    #[test]
    fn test_numeric_edge_cases() {
        // Zero codepoint -> NUL character
        assert_eq!(decode_character_references("&#0;", false), "\0");

        // Surrogate halves -> NUL
        assert_eq!(decode_character_references("&#xD800;", false), "\0");
        assert_eq!(decode_character_references("&#55296;", false), "\0"); // 0xD800 in decimal

        // Line feed -> space
        assert_eq!(decode_character_references("&#10;", false), " ");

        // Windows-1252 range -> Euro sign
        assert_eq!(decode_character_references("&#128;", false), "€");
    }

    #[test]
    fn test_uppercase_hex_entity() {
        // Enhancement over Svelte: we support uppercase X
        assert_eq!(decode_character_references("&#X41;", false), "A");
        assert_eq!(decode_character_references("&#X1F4A9;", false), "💩");

        // Lowercase still works
        assert_eq!(decode_character_references("&#x41;", false), "A");
        assert_eq!(decode_character_references("&#x1F4A9;", false), "💩");
    }

    #[test]
    fn test_numeric_no_semicolon() {
        // Numeric entities work without semicolons too
        assert_eq!(decode_character_references("&#65", false), "A");
        assert_eq!(decode_character_references("&#x41", false), "A");
        assert_eq!(decode_character_references("&#X41", false), "A");
    }

    #[test]
    fn test_degenerate_references() {
        // Empty/zero-length references collect no name or digits and stay literal —
        // these are the boundary cases for the name/digit scanners.
        assert_eq!(decode_character_references("&;", false), "&;");
        assert_eq!(decode_character_references("&#;", false), "&#;");
        assert_eq!(decode_character_references("&#x;", false), "&#x;");
        assert_eq!(decode_character_references("&#X;", false), "&#X;");
    }

    #[test]
    fn test_longest_match_fallthrough() {
        // No-semicolon longest match: "COPY" is a legacy entity but "COPYRIGHT" is
        // not, so only the "COPY" prefix decodes and "RIGHT" is left as text.
        assert_eq!(decode_character_references("&COPYRIGHT", false), "©RIGHT");
        // Semicolon present but neither "notit;" nor "notit" is an entity; the
        // legacy longest-match loop (which runs after the semicolon-first loop
        // breaks) still finds "not", matching Svelte's decoder — &not → ¬, "it;" stays.
        assert_eq!(decode_character_references("&notit;", false), "¬it;");
    }

    #[test]
    fn test_numeric_overflow_and_plane_boundaries() {
        // Values that overflow u32 fail to parse and remain literal text.
        assert_eq!(
            decode_character_references("&#99999999999999;", false),
            "&#99999999999999;"
        );
        assert_eq!(
            decode_character_references("&#xFFFFFFFFFF;", false),
            "&#xFFFFFFFFFF;"
        );
        // Plane boundary: 0x2FFFF (end of plane 2) is preserved, but the unlisted
        // higher planes and beyond-Unicode-max both normalize to NUL (Svelte parity).
        assert_eq!(decode_character_references("&#x2FFFF;", false), "\u{2FFFF}");
        assert_eq!(decode_character_references("&#x30000;", false), "\0");
        assert_eq!(decode_character_references("&#x110000;", false), "\0");
    }
}
