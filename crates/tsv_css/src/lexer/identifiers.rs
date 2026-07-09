use super::lex_err;
use super::token::{Token, TokenKind};
use tsv_lang::ParseError;

/// Whether `ch` is a non-ASCII CSS identifier code point.
///
/// Mirrors Svelte's `parseCss` (`read_identifier`: `codePointAt(0) >= 160`): any
/// code point at or above U+00A0 is a valid identifier code point, so symbols and
/// emoji (`♥`, `💩`) are accepted while the C1 control range U+0080–U+009F stays
/// excluded. Single source for the threshold shared by `is_identifier_start` and
/// the continuation guard in `read_identifier`.
#[inline]
fn is_non_ascii_identifier_codepoint(ch: char) -> bool {
    ch as u32 >= 0xA0
}

/// 256-entry lookup table for the ASCII identifier-continuation fast path in
/// `read_identifier`. Each entry is computed from the same byte predicate the
/// per-char continuation arm expands to for ASCII (`[a-zA-Z0-9_-]`), so a lookup
/// replaces the alnum/eq OR-chain plus a full UTF-8 decode with one L1 load on the
/// hot identifier-body loop. Non-ASCII bytes are all `false`, so the fast path stops
/// at the first non-ASCII byte and the char loop decodes it — byte-identical.
const IDENT_CONTINUE_LUT: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < 256 {
        let b = i as u8;
        t[i] = b.is_ascii_alphanumeric() || b == b'-' || b == b'_';
        i += 1;
    }
    t
};

/// Whether `ch` can begin a CSS identifier token in the lexer dispatch.
///
/// Covers ASCII letters, non-ASCII identifier code points (see
/// `is_non_ascii_identifier_codepoint`), `-`, `_`, and the `\` escape introducer.
/// Digits and a leading `$` have their own dispatch arms (numbers, and `$`-prefixed
/// identifiers), so they're intentionally excluded here — but a `$` arm uses this to
/// confirm the *next* char begins an identifier.
pub(crate) fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic()
        || is_non_ascii_identifier_codepoint(ch)
        || ch == '-'
        || ch == '_'
        || ch == '\\'
}

/// The ASCII byte form of `is_identifier_start`, for the lexer's byte-first dispatch.
///
/// Equal to `is_identifier_start(b as char)` for every ASCII byte (`b < 0x80`), and
/// `false` for every non-ASCII byte — the non-ASCII identifier code points (`>= 0xA0`)
/// are `>= 0x80`, so they're handled by the dispatch's char tail, not here. Covers the
/// ASCII letters, `-`, `_`, and the `\` escape introducer (digits and a leading `$`
/// have their own dispatch arms).
#[inline]
pub(crate) fn is_ascii_identifier_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'-' || b == b'_' || b == b'\\'
}

/// Read a CSS identifier
/// CSS identifiers can contain unicode escapes; the characters a-z, A-Z, 0-9, -, _;
/// any non-ASCII code point at or above U+00A0 (symbols and emoji, matching Svelte's
/// `>= 160` rule); plus an optional leading `$` (SCSS-style; the lexer dispatch only
/// routes `$` here when it begins an identifier).
/// Per CSS Syntax Level 3 spec, escape sequences are decoded to their actual characters.
///
/// Returns the token plus the **decoded value only when an escape was present**:
/// the common no-escape identifier returns `None` (no allocation — its text is the
/// verbatim source slice `source[start..end]`). The decoded buffer is materialized
/// lazily from the verbatim run scanned so far the first time a `\` escape is seen.
#[allow(clippy::box_collection)]
pub(crate) fn read_identifier(
    source: &str,
    pos: &mut usize,
) -> Result<(Token, Option<Box<String>>), Box<ParseError>> {
    let start = *pos;
    // `None` until an escape forces a decoded buffer; then materialized from the
    // verbatim run `source[start..pos]` scanned so far and appended to per escape.
    let mut decoded: Option<String> = None;

    // Optional leading `$` (SCSS-style variable / property identifiers). Svelte's
    // `parseCss` treats `$foo` as a single identifier; a bare `$` (e.g. the `$=`
    // attribute selector) is kept as a Dollar token by the lexer dispatch, so this
    // arm is only reached when `$` begins an identifier. No push: the `$` is part of
    // the verbatim run captured if/when an escape later materializes the buffer.
    if source[*pos..].starts_with('$') {
        *pos += 1;
    }

    // Byte view for the ASCII continuation fast path (coexists with the `source`
    // `&str` slices the escape arms take — both are immutable borrows).
    let bytes = source.as_bytes();

    // CSS identifiers can contain escape sequences that must be decoded
    loop {
        // ASCII fast path: while no `\` escape has yet forced a decoded buffer, scan a
        // run of ASCII identifier-continuation bytes (`[a-zA-Z0-9_-]`) via the lookup
        // table instead of decoding a UTF-8 char per byte. The table is `true` for
        // exactly the ASCII bytes the char match arm below accepts and `false` for every
        // non-ASCII byte, `\`, and terminator, so this advances to precisely the first
        // byte that arm would not consume — byte-identical, skipping the per-char decode.
        // Once an escape materializes the decoded buffer, each following char must be
        // pushed, so the fast path yields to the char loop.
        if decoded.is_none() {
            let mut p = *pos;
            while p < bytes.len() && IDENT_CONTINUE_LUT[bytes[p] as usize] {
                p += 1;
            }
            *pos = p;
        }

        let current_char = source[*pos..].chars().next();
        match current_char {
            // Continuation char: ASCII alphanumeric, a non-ASCII identifier code point,
            // `-`, or `_` — the same predicate Svelte's `read_identifier` applies per
            // char (`codePointAt(0) >= 160 || [a-zA-Z0-9_-]`). The ASCII cases are
            // handled by the fast path above; this arm now fires for non-ASCII code
            // points (and for the first char after an escape materialized the buffer).
            Some(ch)
                if ch.is_ascii_alphanumeric()
                    || is_non_ascii_identifier_codepoint(ch)
                    || ch == '-'
                    || ch == '_' =>
            {
                if let Some(buf) = decoded.as_mut() {
                    buf.push(ch);
                }
                *pos += ch.len_utf8();
            }
            Some('\\') => {
                // Check if this is a valid escape sequence
                let Some(next_ch) = source[*pos + 1..].chars().next() else {
                    // Backslash at end of input. If the identifier already has
                    // content, end it before the backslash; but when the `\` is
                    // the FIRST char scanned, breaking would return a zero-width
                    // token — the caller's token loop would re-dispatch at the
                    // same position forever (and accumulate tokens unboundedly
                    // on the token-tree paths). A lone `\` at EOF is a parse
                    // error per css-syntax §4.3.7 (consume an escaped code
                    // point); the strict-throw model rejects it.
                    if *pos == start {
                        return Err(lex_err("Unexpected end of input after backslash", *pos));
                    }
                    break;
                };

                if next_ch.is_ascii_hexdigit() {
                    // Unicode escape: \XXXXXX (1-6 hex digits)
                    let buf = decoded.get_or_insert_with(|| source[start..*pos].to_string());
                    let ch = decode_unicode_escape(source, pos)?;
                    buf.push(ch);
                } else if next_ch == '\n' || next_ch == '\r' || next_ch == '\x0C' {
                    // Newline after backslash - invalid escape. Same zero-width
                    // hazard as the EOF branch above: error rather than spin
                    // when the `\` is the first char scanned. (Svelte's parseCss
                    // is lenient here and reads the `\` into the value; tsv
                    // follows the css-syntax parse-error posture — see
                    // conformance_svelte.md §CSS Parser Scope & Error Model.)
                    if *pos == start {
                        return Err(lex_err("Invalid escape: backslash before newline", *pos));
                    }
                    break;
                } else {
                    // Single character escape: backslash followed by any character
                    // The character itself is the escaped value
                    let buf = decoded.get_or_insert_with(|| source[start..*pos].to_string());
                    *pos += 1; // skip backslash
                    buf.push(next_ch);
                    *pos += next_ch.len_utf8();
                }
            }
            _ => {
                break;
            }
        }
    }

    let token = Token {
        kind: TokenKind::Identifier,
        start: start as u32,
        end: *pos as u32,
    };
    Ok((token, decoded.map(Box::new)))
}

/// Decode a CSS unicode escape sequence: \XXXXXX (1-6 hex digits)
/// Advances position past the escape sequence.
///
/// Per CSS Syntax 3 §4.3.7 (consume an escaped code point), a value that is
/// zero, is for a surrogate, or is greater than the maximum allowed code point
/// (U+10FFFF) decodes to U+FFFD REPLACEMENT CHARACTER — it is not a parse error.
pub(crate) fn decode_unicode_escape(
    source: &str,
    pos: &mut usize,
) -> Result<char, Box<ParseError>> {
    let start = *pos;
    *pos += 1; // skip \

    let mut hex_str = String::new();

    // Read 1-6 hex digits
    for _ in 0..6 {
        match source[*pos..].chars().next() {
            Some(ch) if ch.is_ascii_hexdigit() => {
                hex_str.push(ch);
                *pos += ch.len_utf8();
            }
            _ => break,
        }
    }

    if hex_str.is_empty() {
        return Err(lex_err("Invalid unicode escape sequence", start));
    }

    // Skip optional whitespace after unicode escape
    if let Some(ch) = source[*pos..].chars().next()
        && ch.is_whitespace()
    {
        *pos += ch.len_utf8();
    }

    // The 1–6 hex digits always fit a u32. Per CSS Syntax 3 §4.3.7, zero / a surrogate /
    // an above-maximum code point decodes to U+FFFD REPLACEMENT CHARACTER rather than being
    // a parse error: `char::from_u32` already returns `None` for the surrogate / above-maximum
    // cases, and zero is a valid `char` (NUL) so it needs the explicit guard.
    let code_point = u32::from_str_radix(&hex_str, 16).unwrap_or(0xFFFD);
    let decoded = if code_point == 0 {
        '\u{FFFD}'
    } else {
        char::from_u32(code_point).unwrap_or('\u{FFFD}')
    };
    Ok(decoded)
}
