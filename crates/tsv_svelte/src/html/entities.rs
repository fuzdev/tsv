//! HTML character reference decoding, as Svelte's parser decodes — the HTML spec's, with
//! the deviations listed below. It handles:
//! - Named references (`&amp;`, `&lt;`, `&nbsp;`, …), matched against the spec's table by
//!   `tsv_html::match_named_character_reference`
//! - Numeric decimal references (`&#65;`)
//! - Numeric hex references (`&#x41;` or `&#X41;`)
//! - Different rules for attribute values and text content
//!
//! ## Svelte's decoder: which quirks are matched, which are corrected
//!
//! The public AST is parity with Svelte's parser, so the decoder replicates the answers
//! Svelte's `validate_code` deliberately chose over the spec's:
//! - **NUL, not U+FFFD, for a code that cannot be represented** — a surrogate half or a
//!   code past the last code point Unicode defines.
//! - **A line feed becomes a space in text** (`&#10;` → U+0020). In an attribute value
//!   it stays a line feed — the spec's answer in both places, which Svelte keeps where
//!   the character is significant and trades for a space where whitespace collapses.
//!
//! Four others are *slips* in Svelte's implementation rather than choices, so the decoder
//! follows the spec instead — cataloged in `docs/conformance_svelte.md` §Entity Decoding
//! Corrections, and each one an upstream candidate:
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

use std::borrow::Cow;

// Windows-1252 code point replacements for the range 128-159 — the Encoding Standard's
// index-windows-1252 (https://encoding.spec.whatwg.org/#index-windows-1252), which HTML's
// numeric character reference end state applies; Svelte's `windows_1252` holds the same.
const WINDOWS_1252: [u32; 32] = [
    8364, 129, 8218, 402, 8222, 8230, 8224, 8225, 710, 8240, 352, 8249, 338, 141, 381, 143, 144,
    8216, 8217, 8220, 8221, 8226, 8211, 8212, 732, 8482, 353, 8250, 339, 157, 382, 376,
];

/// Where a run of text whose character references are decoded sits — the one fact about
/// it [`decode_character_references`] reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceContext {
    /// Text content: every valid reference is decoded.
    Text,
    /// An attribute value: a named reference without its `;` is decoded only when not
    /// followed by `=` or an alphanumeric character (HTML's attribute-value rule).
    AttributeValue,
}

/// Decode HTML character references in a string, as Svelte's parser decodes them.
/// Borrows `html` when nothing in it decodes (no `&`, or none that starts a reference).
///
/// Named and numeric (decimal and hex) references both decode. The behavior differs by
/// `context` ([`ReferenceContext`]):
///
/// - In an attribute value, a named reference with no `;` is decoded only when the
///   next character is neither `=` nor an ASCII alphanumeric.
/// - In text, every valid reference is decoded, and `&#10;` becomes a space (U+0020); in
///   an attribute value it stays a line feed.
///
/// Svelte's answers are kept where they differ from the HTML spec on purpose: a numeric
/// reference to a surrogate half or past U+10FFFF decodes to NUL rather than U+FFFD;
/// 128–159 take their Windows-1252 characters. Where Svelte's decoder slips, tsv does not
/// follow it: `&#0;` decodes (to that NUL) where Svelte leaves it as written, the
/// attribute-value boundary above is ASCII alphanumeric (not the word class of JS's `\b`,
/// which adds `_`), every other code point is emitted as itself (Svelte drops some planes
/// to NUL), and a name that stands for two code points emits both.
pub fn decode_character_references(html: &str, context: ReferenceContext) -> Cow<'_, str> {
    // No `&`, no reference: the text is its own decode, borrowed.
    let Some(first) = html.find('&') else {
        return Cow::Borrowed(html);
    };
    let is_attribute_value = context == ReferenceContext::AttributeValue;
    let mut result = String::with_capacity(html.len());
    result.push_str(&html[..first]);
    let mut i = first;
    let mut decoded_any = false;

    while let Some(ch) = html[i..].chars().next() {
        if ch != '&' {
            result.push(ch);
            i += ch.len_utf8();
            continue;
        }

        // Try to parse an entity reference starting at '&'
        let rest = &html[i + 1..];

        // Numeric character reference: &#... or &#x...
        if let Some((consumed, decoded)) = decode_numeric_entity(rest, is_attribute_value) {
            result.push(decoded);
            decoded_any = true;
            i += 1 + consumed; // +1 for '&'
            continue;
        }

        // Named character reference
        if let Some((consumed, decoded)) = decode_named_entity(rest, is_attribute_value) {
            result.push_str(decoded);
            decoded_any = true;
            i += 1 + consumed; // +1 for '&'
            continue;
        }

        // Not a valid entity, keep the '&'
        result.push('&');
        i += 1;
    }

    if decoded_any {
        Cow::Owned(result)
    } else {
        Cow::Borrowed(html)
    }
}

/// Decode a numeric character reference (decimal or hex)
///
/// Returns (bytes_consumed, the character it stands for) if successful — the same
/// (span, value) order as [`decode_named_entity`], so the caller's two arms read alike.
/// Examples: &#65; &#x41; &#X41; &#0041;
fn decode_numeric_entity(rest: &str, is_attribute_value: bool) -> Option<(usize, char)> {
    if !rest.starts_with('#') {
        return None;
    }

    let after_hash = &rest[1..];
    // Either case opens a hex reference, per the spec's numeric-character-reference state.
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
    let validated = validate_code(code, is_attribute_value);
    let decoded = char::from_u32(validated)?;

    Some((total_consumed, decoded))
}

/// Decode a named character reference
///
/// Returns (consumed_length, the characters it stands for) if successful — one character
/// for all but the references the table gives two. The match is the spec's
/// ([`tsv_html::match_named_character_reference`]: the longest name in the table); what is left to
/// decide here is the attribute-value rule.
#[inline]
fn decode_named_entity(rest: &str, is_attribute_value: bool) -> Option<(usize, &'static str)> {
    let (len, characters) = tsv_html::match_named_character_reference(rest)?;

    // In an attribute value, a match not closed by `;` is left as written when followed by
    // `=` or an ASCII alphanumeric — the spec's named-character-reference state, verbatim. The
    // class is deliberately narrower than `char::is_alphanumeric` (which would hold `中`, `٣`,
    // …, so `&AMP中` would stop decoding) and than JS's `\b` word class (which also holds `_`,
    // the shorthand Svelte reaches for). A match ends on a character boundary (names are
    // ASCII), so `rest[len..]` cannot slice mid-character.
    if is_attribute_value
        && rest.as_bytes()[len - 1] != b';'
        && let Some(next) = rest[len..].chars().next()
        && (next == '=' || next.is_ascii_alphanumeric())
    {
        return None;
    }

    Some((len, characters))
}

/// Validate and normalize a numeric reference's code point, as Svelte's decoder does
///
/// Some code points are verboten and must be replaced or normalized:
/// - Line feed (10) becomes space (32) outside an attribute value
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
fn validate_code(code: u32, is_attribute_value: bool) -> u32 {
    const NUL: u32 = 0;
    const MAX_CODE_POINT: u32 = 0x10FFFF;
    const SURROGATES: std::ops::RangeInclusive<u32> = 0xD800..=0xDFFF;

    // Line feed becomes generic whitespace in text, where it collapses with the
    // whitespace around it anyway; in an attribute value it is significant, so it stays
    if code == 10 && !is_attribute_value {
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

    /// The decoder's headline behaviors, one of each.
    #[test]
    fn test_decode_overview() {
        // Named entities
        assert_eq!(
            decode_character_references("&lt;tag&gt;", ReferenceContext::Text),
            "<tag>"
        );
        assert_eq!(
            decode_character_references("&amp; &quot;", ReferenceContext::Text),
            "& \""
        );

        // Numeric decimal
        assert_eq!(
            decode_character_references("&#65;", ReferenceContext::Text),
            "A"
        );

        // Numeric hex
        assert_eq!(
            decode_character_references("&#x41;", ReferenceContext::Text),
            "A"
        );
        assert_eq!(
            decode_character_references("&#X41;", ReferenceContext::Text),
            "A"
        );

        // A named reference with no `;` before `=` stays as written in an attribute value
        assert_eq!(
            decode_character_references("&amp=", ReferenceContext::AttributeValue),
            "&amp="
        );

        // Nothing decodes: the input itself, borrowed
        assert!(matches!(
            decode_character_references("a & b", ReferenceContext::Text),
            Cow::Borrowed(_)
        ));
    }

    /// The longest name decodes: the name search is bounded by the table's longest key.
    #[test]
    fn test_longest_entity_name() {
        assert_eq!(
            decode_character_references(
                "&CounterClockwiseContourIntegral;",
                ReferenceContext::Text
            ),
            "\u{2233}"
        );
    }

    /// A long alphanumeric run after an `&` names no reference and decodes to itself; the
    /// search is bounded by the longest name, not the run.
    #[test]
    fn test_long_name_run() {
        let text = format!("&{}", "a".repeat(10_000));
        assert_eq!(
            decode_character_references(&text, ReferenceContext::Text),
            text
        );
        let text = format!("&amp{}", "a".repeat(10_000));
        assert_eq!(
            decode_character_references(&text, ReferenceContext::Text),
            format!("&{}", "a".repeat(10_000))
        );
    }

    #[test]
    fn test_basic_named_entities() {
        assert_eq!(
            decode_character_references("&lt;", ReferenceContext::Text),
            "<"
        );
        assert_eq!(
            decode_character_references("&gt;", ReferenceContext::Text),
            ">"
        );
        assert_eq!(
            decode_character_references("&amp;", ReferenceContext::Text),
            "&"
        );
        assert_eq!(
            decode_character_references("&quot;", ReferenceContext::Text),
            "\""
        );
        assert_eq!(
            decode_character_references("&nbsp;", ReferenceContext::Text),
            "\u{00A0}"
        );
        assert_eq!(
            decode_character_references("&copy;", ReferenceContext::Text),
            "©"
        );
    }

    #[test]
    fn test_numeric_decimal() {
        assert_eq!(
            decode_character_references("&#65;", ReferenceContext::Text),
            "A"
        );
        assert_eq!(
            decode_character_references("&#8364;", ReferenceContext::Text),
            "€"
        );
        assert_eq!(
            decode_character_references("&#128169;", ReferenceContext::Text),
            "💩"
        );
    }

    #[test]
    fn test_numeric_hex() {
        assert_eq!(
            decode_character_references("&#x41;", ReferenceContext::Text),
            "A"
        );
        assert_eq!(
            decode_character_references("&#X41;", ReferenceContext::Text),
            "A"
        );
        assert_eq!(
            decode_character_references("&#x20AC;", ReferenceContext::Text),
            "€"
        );
        assert_eq!(
            decode_character_references("&#x1F4A9;", ReferenceContext::Text),
            "💩"
        );
    }

    #[test]
    fn test_mixed_content() {
        assert_eq!(
            decode_character_references("&lt;tag&gt;", ReferenceContext::Text),
            "<tag>"
        );
        assert_eq!(
            decode_character_references("&amp; &quot; &copy;", ReferenceContext::Text),
            "& \" ©"
        );
    }

    #[test]
    fn test_invalid_entities() {
        // Unknown entity - keep as is
        assert_eq!(
            decode_character_references("&unknown;", ReferenceContext::Text),
            "&unknown;"
        );
    }

    #[test]
    fn test_validate_code() {
        // Line feed -> space in text, kept in an attribute value
        assert_eq!(validate_code(10, false), 32);
        assert_eq!(validate_code(10, true), 10);

        // ASCII
        assert_eq!(validate_code(65, false), 65); // 'A'

        // Windows-1252 replacement
        assert_eq!(validate_code(128, false), 8364); // Euro sign

        // Valid BMP
        assert_eq!(validate_code(0x00C6, false), 0x00C6); // Æ

        // Surrogate halves -> NUL
        assert_eq!(validate_code(0xD800, false), 0);
        assert_eq!(validate_code(0xDFFF, false), 0);

        // Valid supplementary plane
        assert_eq!(validate_code(0x1F4A9, false), 0x1F4A9); // 💩
    }

    #[test]
    fn test_legacy_entities_no_semicolon() {
        // Legacy entities without semicolon should decode in content
        assert_eq!(
            decode_character_references("&AMP", ReferenceContext::Text),
            "&"
        );
        assert_eq!(
            decode_character_references("&COPY", ReferenceContext::Text),
            "©"
        );
        assert_eq!(
            decode_character_references("&LT", ReferenceContext::Text),
            "<"
        );
        assert_eq!(
            decode_character_references("&GT", ReferenceContext::Text),
            ">"
        );
    }

    #[test]
    fn test_attribute_context_rules() {
        // In attributes, entities without semicolon must not be followed by '=' or alphanumeric

        // Should NOT decode (followed by '=')
        assert_eq!(
            decode_character_references("&AMP=value", ReferenceContext::AttributeValue),
            "&AMP=value"
        );

        // Should NOT decode (followed by alphanumeric)
        assert_eq!(
            decode_character_references("&AMPersand", ReferenceContext::AttributeValue),
            "&AMPersand"
        );

        // Should decode (followed by punctuation)
        assert_eq!(
            decode_character_references("&AMP&more", ReferenceContext::AttributeValue),
            "&&more"
        );

        // Should decode (followed by space)
        assert_eq!(
            decode_character_references("&AMP ", ReferenceContext::AttributeValue),
            "& "
        );

        // Should always decode with semicolon
        assert_eq!(
            decode_character_references("&AMP;=test", ReferenceContext::AttributeValue),
            "&=test"
        );

        // The blocking class is ASCII alphanumeric, nothing wider and nothing narrower:
        // an underscore does not block (JS's `\b` word class, which Svelte reaches for,
        // would), and neither does a non-ASCII letter or digit (`char::is_alphanumeric`
        // would). Both decode in text content too, where no rule applies at all.
        assert_eq!(
            decode_character_references("&AMP_x", ReferenceContext::AttributeValue),
            "&_x"
        );
        assert_eq!(
            decode_character_references("&COPY\u{4e2d}", ReferenceContext::AttributeValue),
            "©\u{4e2d}"
        );
        assert_eq!(
            decode_character_references("&COPY\u{663}", ReferenceContext::AttributeValue),
            "©\u{663}"
        );
        assert_eq!(
            decode_character_references("&COPY\u{ff11}", ReferenceContext::AttributeValue),
            "©\u{ff11}"
        );
        assert_eq!(
            decode_character_references("&AMP_x", ReferenceContext::Text),
            "&_x"
        );
        assert_eq!(
            decode_character_references("&COPY\u{4e2d}", ReferenceContext::Text),
            "©\u{4e2d}"
        );
    }

    #[test]
    fn test_incomplete_entities() {
        // Incomplete entity names should stay as literal text
        assert_eq!(
            decode_character_references("&am", ReferenceContext::Text),
            "&am"
        );
        assert_eq!(
            decode_character_references("&l", ReferenceContext::Text),
            "&l"
        );
        assert_eq!(
            decode_character_references("&", ReferenceContext::Text),
            "&"
        );
    }

    #[test]
    fn test_numeric_edge_cases() {
        // Zero codepoint -> NUL character
        assert_eq!(
            decode_character_references("&#0;", ReferenceContext::Text),
            "\0"
        );

        // Surrogate halves -> NUL
        assert_eq!(
            decode_character_references("&#xD800;", ReferenceContext::Text),
            "\0"
        );
        assert_eq!(
            decode_character_references("&#55296;", ReferenceContext::Text),
            "\0"
        ); // 0xD800 in decimal

        // Line feed -> space
        assert_eq!(
            decode_character_references("&#10;", ReferenceContext::Text),
            " "
        );

        // Windows-1252 range -> Euro sign
        assert_eq!(
            decode_character_references("&#128;", ReferenceContext::Text),
            "€"
        );
    }

    #[test]
    fn test_uppercase_hex_entity() {
        // Enhancement over Svelte: we support uppercase X
        assert_eq!(
            decode_character_references("&#X41;", ReferenceContext::Text),
            "A"
        );
        assert_eq!(
            decode_character_references("&#X1F4A9;", ReferenceContext::Text),
            "💩"
        );

        // Lowercase still works
        assert_eq!(
            decode_character_references("&#x41;", ReferenceContext::Text),
            "A"
        );
        assert_eq!(
            decode_character_references("&#x1F4A9;", ReferenceContext::Text),
            "💩"
        );
    }

    #[test]
    fn test_numeric_no_semicolon() {
        // Numeric entities work without semicolons too
        assert_eq!(
            decode_character_references("&#65", ReferenceContext::Text),
            "A"
        );
        assert_eq!(
            decode_character_references("&#x41", ReferenceContext::Text),
            "A"
        );
        assert_eq!(
            decode_character_references("&#X41", ReferenceContext::Text),
            "A"
        );
    }

    #[test]
    fn test_degenerate_references() {
        // Empty/zero-length references collect no name or digits and stay literal —
        // these are the boundary cases for the name/digit scanners.
        assert_eq!(
            decode_character_references("&;", ReferenceContext::Text),
            "&;"
        );
        assert_eq!(
            decode_character_references("&#;", ReferenceContext::Text),
            "&#;"
        );
        assert_eq!(
            decode_character_references("&#x;", ReferenceContext::Text),
            "&#x;"
        );
        assert_eq!(
            decode_character_references("&#X;", ReferenceContext::Text),
            "&#X;"
        );
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

                for context in [ReferenceContext::Text, ReferenceContext::AttributeValue] {
                    let decoded = decode_character_references(&input, context);
                    // Decoding replaces a whole reference with the one or two characters
                    // it stands for and copies every other character through. The
                    // shortest reference is four characters (`&lt;`), so the run can
                    // only ever shorten.
                    assert!(
                        decoded.chars().count() <= input.chars().count(),
                        "{input:?} grew to {decoded:?} (context: {context:?})"
                    );
                    // Nothing but a reference is rewritten, and a reference needs an '&'.
                    if !input.contains('&') {
                        assert_eq!(
                            decoded, input,
                            "rewrote {input:?} with no reference in it (context: {context:?})"
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
            decode_character_references("a&b\u{4e2d}pos;", ReferenceContext::AttributeValue),
            "a&b\u{4e2d}pos;"
        );
        assert_eq!(
            decode_character_references("a&b\u{4e2d}pos", ReferenceContext::AttributeValue),
            "a&b\u{4e2d}pos"
        );
        // A well-formed reference beside one still decodes.
        assert_eq!(
            decode_character_references("&amp; a&b\u{4e2d}pos;", ReferenceContext::AttributeValue),
            "& a&b\u{4e2d}pos;"
        );
        // A numeric reference's digit run ends at the same place for the same reason, but
        // the prefix it collected is still a reference (the spec's missing-semicolon path),
        // so the decode takes it and the multibyte character starts the text after it.
        assert_eq!(
            decode_character_references("&#4\u{663}1;", ReferenceContext::Text),
            "\u{4}\u{663}1;"
        );
        assert_eq!(
            decode_character_references("&#x4\u{1f4a9}1;", ReferenceContext::Text),
            "\u{4}\u{1f4a9}1;"
        );
        // With no ASCII digit at all there is no reference to take.
        assert_eq!(
            decode_character_references("&#x\u{663}1;", ReferenceContext::Text),
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
            decode_character_references("&NotEqualTilde;", ReferenceContext::Text),
            "\u{2242}\u{338}"
        );
        assert_eq!(
            decode_character_references("&nLt;", ReferenceContext::Text),
            "\u{226a}\u{20d2}"
        );
        // A variation selector.
        assert_eq!(
            decode_character_references("&caps;", ReferenceContext::Text),
            "\u{2229}\u{fe00}"
        );
        // Two characters outright — a ligature spelled as its letters, and two spaces.
        assert_eq!(
            decode_character_references("&fjlig;", ReferenceContext::Text),
            "fj"
        );
        assert_eq!(
            decode_character_references("&ThickSpace;", ReferenceContext::Text),
            "\u{205f}\u{200a}"
        );
        // The attribute-value path decodes the same table.
        assert_eq!(
            decode_character_references("&bne;", ReferenceContext::AttributeValue),
            "=\u{20e5}"
        );
        // The base character each pairs with has its own single-code-point reference.
        assert_eq!(
            decode_character_references("&esim;", ReferenceContext::Text),
            "\u{2242}"
        );
        assert_eq!(
            decode_character_references("&cap;", ReferenceContext::Text),
            "\u{2229}"
        );
        // Every one of the 93 is a semicolon-terminated name: no legacy spelling of one
        // exists, so a semicolon-less prefix decodes to whatever shorter name it holds
        // (here none, leaving the source verbatim).
        assert_eq!(
            decode_character_references("&NotEqualTilde", ReferenceContext::Text),
            "&NotEqualTilde"
        );
    }

    #[test]
    fn test_longest_match_fallthrough() {
        // No-semicolon longest match: "COPY" is a legacy entity but "COPYRIGHT" is
        // not, so only the "COPY" prefix decodes and "RIGHT" is left as text.
        assert_eq!(
            decode_character_references("&COPYRIGHT", ReferenceContext::Text),
            "©RIGHT"
        );
        // Semicolon present but neither "notit;" nor "notit" is an entity; the
        // legacy longest-match loop (which runs after the semicolon-first loop
        // breaks) still finds "not", matching Svelte's decoder — &not → ¬, "it;" stays.
        assert_eq!(
            decode_character_references("&notit;", ReferenceContext::Text),
            "¬it;"
        );
    }

    #[test]
    fn test_numeric_overflow_and_plane_boundaries() {
        // A value too large for u32 is just another code past the last supported plane,
        // so it normalizes to NUL like any other out-of-range one (Svelte parity — its
        // `parseInt` never overflows).
        assert_eq!(
            decode_character_references("&#99999999999999;", ReferenceContext::Text),
            "\0"
        );
        assert_eq!(
            decode_character_references("&#xFFFFFFFFFF;", ReferenceContext::Text),
            "\0"
        );
        assert_eq!(
            decode_character_references("&#4294967296;", ReferenceContext::Text),
            "\0"
        );
        // The largest value that still fits, one past the largest supported code point,
        // and the plane boundary just below — all the same answer.
        assert_eq!(
            decode_character_references("&#4294967295;", ReferenceContext::Text),
            "\0"
        );
        assert_eq!(
            decode_character_references("&#1114112;", ReferenceContext::Text),
            "\0"
        );
        // Every code point Unicode defines survives, in whichever plane — an assigned one
        // (0x30000 is CJK Extension G) and a private-use one alike. Only a surrogate half
        // and a value past the last code point take the NUL sentinel.
        assert_eq!(
            decode_character_references("&#x2FFFF;", ReferenceContext::Text),
            "\u{2FFFF}"
        );
        assert_eq!(
            decode_character_references("&#x30000;", ReferenceContext::Text),
            "\u{30000}"
        );
        assert_eq!(
            decode_character_references("&#xF0000;", ReferenceContext::Text),
            "\u{F0000}"
        );
        assert_eq!(
            decode_character_references("&#x10FFFF;", ReferenceContext::Text),
            "\u{10FFFF}"
        );
        assert_eq!(
            decode_character_references("&#x110000;", ReferenceContext::Text),
            "\0"
        );
    }
}
