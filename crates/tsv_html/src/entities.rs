//! HTML entity decoding
//!
//! This module provides HTML character reference decoding following the HTML5 spec.
//! It handles:
//! - Named entities (&amp;, &lt;, &nbsp;, etc.)
//! - Numeric decimal entities (&#65;)
//! - Numeric hex entities (&#x41; or &#X41;)
//! - Different parsing rules for attribute values vs text content
//!
//! ## The entity map
//!
//! - Source: <https://html.spec.whatwg.org/entities.json>, one entry per name holding
//!   the characters it stands for
//! - Generated at compile time from `src/entities.json` by `build.rs`
//! - 93 of the 2,231 names stand for **two** code points — a combining mark, a variation
//!   selector, or a second character — which is why a lookup answers a `&str`
//!
//! ## Svelte's decoder: which quirks are matched, which are corrected
//!
//! The public AST is parity with Svelte's parser, so the decoder replicates the answers
//! Svelte's `validate_code` deliberately chose over the spec's:
//! - **NUL, not U+FFFD, for a code that cannot be represented** — a surrogate half or a
//!   code past the last code point Unicode defines.
//! - **A line feed becomes a space** (`&#10;` → U+0020).
//!
//! Five others are *slips* in Svelte's implementation rather than choices, so the decoder
//! follows the spec instead — cataloged in `docs/conformance_svelte.md` §Entity Decoding
//! Corrections, and each one an upstream candidate:
//! - **Uppercase hex** — `&#X41;` decodes. The spec's numeric-character-reference state
//!   lists `U+0078 x` and `U+0058 X` side by side; Svelte's pattern spells only the
//!   lowercase one.
//! - **A zero code** — `&#0;` decodes (to NUL, per the sentinel above). Svelte's
//!   `if (!code) return match` guards against an unknown/unparseable reference, and a code
//!   of 0 is merely the falsy value caught in passing.
//! - **The attribute-value boundary** — the spec blocks a semicolon-less reference only
//!   before `=` or an ASCII alphanumeric. Svelte reaches for JS's `\b`, whose word class
//!   also holds `_`, though its own comment quotes the spec rule.
//! - **The admitted planes** — every code point but a surrogate half is emitted as itself.
//!   Svelte enumerates the planes it will emit and drops the rest to NUL, destroying
//!   assigned characters (`&#x30000;` is CJK Extension G) — see `validate_code`.
//! - **A two-code-point reference** — both are emitted. Svelte's entity table is generated
//!   with one code point per name, so the second is dropped and a negated relation decodes
//!   to the relation itself: `&NotEqualTilde;` loses the combining solidus that makes it a
//!   negation, leaving the U+2242 of `&esim;`.
//!
//! References:
//! - <https://html.spec.whatwg.org/multipage/parsing.html#named-character-reference-state>
//! - <https://html.spec.whatwg.org/entities.json>
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
// Stored in: crates/tsv_html/src/entities.json
// Generated at compile time by build.rs
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

    while let Some(ch) = html[i..].chars().next() {
        if ch != '&' {
            result.push(ch);
            i += ch.len_utf8();
            continue;
        }

        // Try to parse an entity reference starting at '&'
        let rest = &html[i + 1..];

        // Numeric character reference: &#... or &#x...
        if let Some((consumed, decoded)) = decode_numeric_entity(rest) {
            result.push(decoded);
            i += 1 + consumed; // +1 for '&'
            continue;
        }

        // Named character reference
        if let Some((consumed, decoded)) = decode_named_entity(rest, is_attribute_value) {
            result.push_str(decoded);
            i += 1 + consumed; // +1 for '&'
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
/// Returns (bytes_consumed, the character it stands for) if successful — the same
/// (span, value) order as [`decode_named_entity`], so the caller's two arms read alike.
/// Examples: &#65; &#x41; &#X41; &#0041;
fn decode_numeric_entity(rest: &str) -> Option<(usize, char)> {
    if !rest.starts_with('#') {
        return None;
    }

    let after_hash = &rest[1..];
    // Either case opens a hex reference, per the spec's numeric-character-reference state.
    // Svelte's pattern (`#(?:x[a-fA-F\d]+|\d+)`) spells only the lowercase one — a slip the
    // module docs catalog.
    let is_hex = after_hash.starts_with('x') || after_hash.starts_with('X');

    let digits_start_offset = if is_hex { 1 } else { 0 };
    let digits_start = &after_hash[digits_start_offset..];

    // Collect hex or decimal digits. A digit is ASCII, so scanning bytes ends the run at the
    // first byte of any multibyte character and every index below stays on a boundary.
    let is_digit = |b: u8| {
        if is_hex {
            b.is_ascii_hexdigit()
        } else {
            b.is_ascii_digit()
        }
    };
    let digit_count = digits_start
        .bytes()
        .position(|b| !is_digit(b))
        .unwrap_or(digits_start.len());

    if digit_count == 0 {
        return None;
    }
    let digits = &digits_start[..digit_count];

    // Parse the number. A run too long for `u32` is still just an out-of-range code — the
    // digits were collected by the ASCII scan above, so overflow is the only way parsing
    // can fail — and `validate_code` answers NUL for every code past the last code point,
    // so saturating at `u32::MAX` (itself past that) reaches the same answer.
    let code = if is_hex {
        u32::from_str_radix(digits, 16)
    } else {
        digits.parse::<u32>()
    }
    .unwrap_or(u32::MAX);

    // Check for optional semicolon
    let has_semicolon = digits_start.as_bytes().get(digit_count) == Some(&b';');

    // Total consumed: '#' + optional 'x'/'X' + digits + optional ';'
    let total_consumed = 1 + digits_start_offset + digit_count + if has_semicolon { 1 } else { 0 };

    // Validate and convert
    let validated = validate_code(code);
    let decoded = char::from_u32(validated)?;

    Some((total_consumed, decoded))
}

/// Decode a named character reference
///
/// Returns (consumed_length, the characters it stands for) if successful — one character
/// for all but the 93 references the table gives two.
fn decode_named_entity(rest: &str, is_attribute_value: bool) -> Option<(usize, &'static str)> {
    // Every named character reference is ASCII, so the name run ends at the first byte that
    // could not appear in one — which for a multibyte character is its first byte, so every
    // slice below stays on a character boundary. That is load-bearing rather than tidy:
    // `&rest[..len]` is a BYTE range, and a multibyte character reaching one of these lines
    // slices mid-character and panics (`&a中pos;` in an attribute value did, all the way out
    // through `tsv parse`). It also keeps the run's byte length equal to its character
    // count, which the attribute-context look-ahead at the bottom relies on.
    let run_len = rest
        .bytes()
        .position(|b| !b.is_ascii_alphanumeric())
        .unwrap_or(rest.len());
    let run = &rest[..run_len];

    // A semicolon closes the reference (the common case). Try the name with it, then — for
    // the legacy references that also have a semicolon-less spelling — without; either way
    // the semicolon is consumed.
    if rest[run_len..].starts_with(';') {
        let consumed = run_len + 1;
        for name in [&rest[..consumed], run] {
            if let Some(&characters) = ENTITIES.get(name) {
                return Some((consumed, characters));
            }
        }
    }

    // Otherwise (and for a semicolon that closed no reference — `&notit;` is `&not` + `it;`)
    // the longest prefix of the run that names one wins.
    let (longest_len, characters) = (1..=run_len).rev().find_map(|len| {
        ENTITIES
            .get(&run[..len])
            .map(|&characters| (len, characters))
    })?;

    // Check if we should decode in attribute context: don't decode if followed by '=' or an
    // ASCII alphanumeric — the spec's named-character-reference state, verbatim. The class is
    // deliberately narrower than `char::is_alphanumeric` (which would hold `中`, `٣`, …, so
    // `&AMP中` would stop decoding) and than JS's `\b` word class (which also holds `_`, the
    // shorthand Svelte reaches for).
    if is_attribute_value
        && let Some(next) = rest[longest_len..].chars().next()
        && (next == '=' || next.is_ascii_alphanumeric())
    {
        return None;
    }

    Some((longest_len, characters))
}

/// Validate and normalize a Unicode code point per HTML5 spec
///
/// Some code points are verboten and must be replaced or normalized:
/// - Line feed (10) becomes space (32)
/// - Code points 128-159 use Windows-1252 replacements
/// - UTF-16 surrogate halves (D800-DFFF) become NUL
/// - Code points past U+10FFFF become NUL
///
/// The two NUL answers are Svelte's sentinel where the spec asks for U+FFFD — a
/// deliberate choice of its decoder, replicated here (see the module docs). Every other
/// code point is emitted as itself: the spec replaces only surrogates and values past
/// U+10FFFF, so a plane carrying assigned characters (U+30000 is CJK Extension G) or a
/// private-use one is text like any other. Svelte instead enumerates the planes it
/// admits, which drops the rest to NUL — a slip it has been patching one plane at a time
/// ([sveltejs/svelte#15823](https://github.com/sveltejs/svelte/pull/15823) added the
/// supplementary special-purpose ranges), so tsv follows the spec here.
///
/// References:
/// - <http://en.wikipedia.org/wiki/Character_encodings_in_HTML#Illegal_characters>
/// - <https://html.spec.whatwg.org/multipage/parsing.html#numeric-character-reference-end-state>
/// - <https://html.spec.whatwg.org/multipage/parsing.html#preprocessing-the-input-stream>
fn validate_code(code: u32) -> u32 {
    const NUL: u32 = 0;
    const MAX_CODE_POINT: u32 = 0x10FFFF;
    const SURROGATES: std::ops::RangeInclusive<u32> = 0xD800..=0xDFFF;

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

    // A surrogate half is not a character, and neither is anything past the last code
    // point Unicode defines — both are unrepresentable, so they take the sentinel
    if SURROGATES.contains(&code) || code > MAX_CODE_POINT {
        return NUL;
    }

    code
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generated table's shape, which the decoder reads as a given: a name is an
    /// ASCII run with at most a trailing semicolon (the byte scan in
    /// `decode_named_entity` ends every name run on `is_ascii_alphanumeric`, and slices
    /// by byte), and a value is the one or two characters the reference stands for.
    /// A regeneration that dropped the second character would leave this the only guard
    /// — no corpus file in any gate uses one of the 93 two-character references.
    #[test]
    fn test_entity_table_shape() {
        let mut two_character = 0;
        for (name, characters) in &ENTITIES {
            let bare = name.strip_suffix(';').unwrap_or(name);
            assert!(
                !bare.is_empty() && bare.bytes().all(|b| b.is_ascii_alphanumeric()),
                "{name:?} is not an ASCII name run plus an optional semicolon"
            );
            match characters.chars().count() {
                1 => {}
                2 => two_character += 1,
                n => panic!("{name:?} stands for {n} characters"),
            }
        }
        assert_eq!(ENTITIES.len(), 2231);
        assert_eq!(two_character, 93);
    }

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

        // The blocking class is ASCII alphanumeric, nothing wider and nothing narrower:
        // an underscore does not block (JS's `\b` word class, which Svelte reaches for,
        // would), and neither does a non-ASCII letter or digit (`char::is_alphanumeric`
        // would). Both decode in text content too, where no rule applies at all.
        assert_eq!(decode_character_references("&AMP_x", true), "&_x");
        assert_eq!(
            decode_character_references("&COPY\u{4e2d}", true),
            "©\u{4e2d}"
        );
        assert_eq!(
            decode_character_references("&COPY\u{663}", true),
            "©\u{663}"
        );
        assert_eq!(
            decode_character_references("&COPY\u{ff11}", true),
            "©\u{ff11}"
        );
        assert_eq!(decode_character_references("&AMP_x", false), "&_x");
        assert_eq!(
            decode_character_references("&COPY\u{4e2d}", false),
            "©\u{4e2d}"
        );
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

    /// Every scan in this module walks a `&str` by byte offset while the runs it slices are
    /// found with `char` predicates, so a multibyte character in any position is a slicing
    /// hazard — `&a中pos;` in an attribute value once panicked all the way out through
    /// `tsv parse`. Sweep every short string over an alphabet mixing the syntax characters
    /// with 2-, 3- and 4-byte ones, in both contexts.
    #[test]
    fn test_multibyte_scan_sweep() {
        const ALPHABET: [char; 14] = [
            '&',
            '#',
            'x',
            'X',
            ';',
            '1',
            'a',
            'A',
            '_',
            '\u{4e2d}',
            '\u{663}',
            '\u{1f4a9}',
            '\u{feff}',
            '\u{301}',
        ];
        let base = ALPHABET.len() as u64;

        let mut input = String::new();
        for len in 1..=4 {
            for n in 0..base.pow(len) {
                input.clear();
                let mut rest = n;
                for _ in 0..len {
                    input.push(ALPHABET[(rest % base) as usize]);
                    rest /= base;
                }

                for is_attribute_value in [false, true] {
                    let decoded = decode_character_references(&input, is_attribute_value);
                    // Decoding replaces a whole reference with the one or two characters
                    // it stands for and copies every other character through. The
                    // shortest reference is four characters (`&lt;`), so the run can
                    // only ever shorten.
                    assert!(
                        decoded.chars().count() <= input.chars().count(),
                        "{input:?} grew to {decoded:?} (attribute value: {is_attribute_value})"
                    );
                    // Nothing but a reference is rewritten, and a reference needs an '&'.
                    if !input.contains('&') {
                        assert_eq!(
                            decoded, input,
                            "rewrote {input:?} with no reference in it (attribute value: {is_attribute_value})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_multibyte_name_run() {
        // A non-ASCII character can only end a name run — no such run can name an entity,
        // so the source is preserved verbatim, semicolon or not.
        assert_eq!(
            decode_character_references("a&b\u{4e2d}pos;", true),
            "a&b\u{4e2d}pos;"
        );
        assert_eq!(
            decode_character_references("a&b\u{4e2d}pos", true),
            "a&b\u{4e2d}pos"
        );
        // A well-formed reference beside one still decodes.
        assert_eq!(
            decode_character_references("&amp; a&b\u{4e2d}pos;", true),
            "& a&b\u{4e2d}pos;"
        );
        // A numeric reference's digit run ends at the same place for the same reason, but
        // the prefix it collected is still a reference (the spec's missing-semicolon path),
        // so the decode takes it and the multibyte character starts the text after it.
        assert_eq!(
            decode_character_references("&#4\u{663}1;", false),
            "\u{4}\u{663}1;"
        );
        assert_eq!(
            decode_character_references("&#x4\u{1f4a9}1;", false),
            "\u{4}\u{1f4a9}1;"
        );
        // With no ASCII digit at all there is no reference to take.
        assert_eq!(
            decode_character_references("&#x\u{663}1;", false),
            "&#x\u{663}1;"
        );
    }

    /// 93 references stand for two code points. Svelte's generated table carries the
    /// first alone, so the second — a combining mark, a variation selector, or a second
    /// character — is dropped; tsv emits both (see the module docs).
    #[test]
    fn test_multi_codepoint_entities() {
        // A combining mark, which is what makes the reference mean what it names.
        assert_eq!(
            decode_character_references("&NotEqualTilde;", false),
            "\u{2242}\u{338}"
        );
        assert_eq!(
            decode_character_references("&nLt;", false),
            "\u{226a}\u{20d2}"
        );
        // A variation selector.
        assert_eq!(
            decode_character_references("&caps;", false),
            "\u{2229}\u{fe00}"
        );
        // Two characters outright — a ligature spelled as its letters, and two spaces.
        assert_eq!(decode_character_references("&fjlig;", false), "fj");
        assert_eq!(
            decode_character_references("&ThickSpace;", false),
            "\u{205f}\u{200a}"
        );
        // The attribute-value path decodes the same table.
        assert_eq!(decode_character_references("&bne;", true), "=\u{20e5}");
        // The base character each pairs with has its own single-code-point reference.
        assert_eq!(decode_character_references("&esim;", false), "\u{2242}");
        assert_eq!(decode_character_references("&cap;", false), "\u{2229}");
        // Every one of the 93 is a semicolon-terminated name: no legacy spelling of one
        // exists, so a semicolon-less prefix decodes to whatever shorter name it holds
        // (here none, leaving the source verbatim).
        assert_eq!(
            decode_character_references("&NotEqualTilde", false),
            "&NotEqualTilde"
        );
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
        // A value too large for u32 is just another code past the last supported plane,
        // so it normalizes to NUL like any other out-of-range one (Svelte parity — its
        // `parseInt` never overflows).
        assert_eq!(
            decode_character_references("&#99999999999999;", false),
            "\0"
        );
        assert_eq!(decode_character_references("&#xFFFFFFFFFF;", false), "\0");
        assert_eq!(decode_character_references("&#4294967296;", false), "\0");
        // The largest value that still fits, one past the largest supported code point,
        // and the plane boundary just below — all the same answer.
        assert_eq!(decode_character_references("&#4294967295;", false), "\0");
        assert_eq!(decode_character_references("&#1114112;", false), "\0");
        // Every code point Unicode defines survives, in whichever plane — an assigned one
        // (0x30000 is CJK Extension G) and a private-use one alike. Only a surrogate half
        // and a value past the last code point take the NUL sentinel.
        assert_eq!(decode_character_references("&#x2FFFF;", false), "\u{2FFFF}");
        assert_eq!(decode_character_references("&#x30000;", false), "\u{30000}");
        assert_eq!(decode_character_references("&#xF0000;", false), "\u{F0000}");
        assert_eq!(
            decode_character_references("&#x10FFFF;", false),
            "\u{10FFFF}"
        );
        assert_eq!(decode_character_references("&#x110000;", false), "\0");
    }
}
