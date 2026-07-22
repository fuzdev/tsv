// Core lexer implementation

use super::comments;
use super::escapes;
use super::ident::{is_id_continue, is_id_start};
use super::lex_err;
use super::token::{Token, TokenKind, keyword_at};
use tsv_lang::ParseError;

/// Byte length of the UTF-8 sequence whose lead byte is `lead`. Used to advance
/// the byte cursor past one character without decoding it.
#[inline]
const fn utf8_len(lead: u8) -> usize {
    match lead {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// 256-entry lookup tables for the ASCII identifier-class fast paths. Each entry is
/// computed from the same predicate the byte tests below expand to, so the tables are
/// exact — a lookup replaces the range/eq OR-chain with one L1 load on the hot
/// per-character identifier-body loop.
const ID_START_LUT: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < 256 {
        let b = i as u8;
        t[i] = b.is_ascii_alphabetic() || b == b'_' || b == b'$';
        i += 1;
    }
    t
};
const ID_CONTINUE_LUT: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < 256 {
        let b = i as u8;
        t[i] = b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
        i += 1;
    }
    t
};

/// ASCII subset of `ID_Start` (`a-z A-Z _ $`) — the byte-cursor fast path before
/// falling back to the full Unicode `is_id_start` on a decoded char.
#[inline]
const fn is_ascii_id_start(b: u8) -> bool {
    ID_START_LUT[b as usize]
}

/// ASCII subset of `ID_Continue` (`a-z A-Z 0-9 _ $`) — the byte-cursor fast path
/// for an identifier body before falling back to the full Unicode `is_id_continue`.
#[inline]
const fn is_ascii_id_continue(b: u8) -> bool {
    ID_CONTINUE_LUT[b as usize]
}

/// Try to decode a unicode escape sequence at the given position.
/// Returns Some((decoded_char, bytes_consumed)) if valid, None otherwise.
///
/// Handles both `\uXXXX` (4-digit) and `\u{X...}` (braced) formats.
fn try_decode_unicode_escape(source: &str, start: usize) -> Option<(char, usize)> {
    let bytes = source.as_bytes();

    // Need at least \u
    if start + 2 > bytes.len() || bytes[start] != b'\\' || bytes[start + 1] != b'u' {
        return None;
    }

    let after_u = start + 2;

    if after_u < bytes.len() && bytes[after_u] == b'{' {
        // Braced format: \u{XXXX}
        let content_start = after_u + 1;
        let mut end = content_start;
        while end < bytes.len() && bytes[end] != b'}' {
            if !bytes[end].is_ascii_hexdigit() {
                return None;
            }
            end += 1;
        }
        if end >= bytes.len() || end == content_start || end - content_start > 6 {
            return None;
        }
        let hex = &source[content_start..end];
        let code = u32::from_str_radix(hex, 16).ok()?;
        let ch = char::from_u32(code)?;
        Some((ch, end + 1 - start)) // +1 for closing brace
    } else {
        // 4-digit format: \uXXXX
        if after_u + 4 > bytes.len() {
            return None;
        }
        for i in 0..4 {
            if !bytes[after_u + i].is_ascii_hexdigit() {
                return None;
            }
        }
        let hex = &source[after_u..after_u + 4];
        let code = u16::from_str_radix(hex, 16).ok()?;
        let ch = char::from_u32(code as u32)?;
        Some((ch, 6)) // \uXXXX is 6 bytes
    }
}

/// How `scan_template_body` stopped: at a closing `` ` ``, at the `${` of an
/// interpolation, or at EOF (unterminated).
enum TemplateStop {
    Backtick,
    Interpolation,
    Eof,
}

pub struct Lexer<'a> {
    source: &'a str,
    /// The source as raw bytes (`source.as_bytes()`), cached so the hot dispatch
    /// peeks a byte without re-deriving the slice. Char decoding (non-ASCII branches)
    /// goes through `source` at `position`.
    bytes: &'a [u8],
    position: usize,
    /// Stack for tracking template literal nesting depth.
    /// When we enter a template interpolation `${`, we push to this stack.
    /// When we see `}`, if the stack is non-empty, we continue template reading.
    template_depth: u32,
    /// True if a line terminator was encountered while skipping whitespace to reach
    /// the current token. Used for Automatic Semicolon Insertion (ASI).
    /// Reset at start of skip_whitespace(), set when line terminators are found.
    had_line_terminator: bool,
    /// Out-of-band decoded value for the token just produced — populated only on the
    /// rare escape path (strings/templates with escapes, escaped identifiers). Kept
    /// off `Token` so the hot per-token value stays a 16-byte POD.
    ///
    /// The decoded bytes live in `decode_scratch`, a buffer parked on the lexer and
    /// **reused across the file** (cleared per escape, capacity retained), so no
    /// per-literal `String` allocates — the escaped-string decode churn a fresh
    /// `String` (plus its `Box`) produced per token is gone. `has_decoded` is the
    /// presence flag `decoded_str` reads; it is cleared at the top of
    /// every token-producing entry point (`next_token`, `continue_template_from_brace`,
    /// `read_regex_literal`) so it reflects only the current token, and set by the
    /// escape paths. The scratch is never read while `has_decoded` is false, so its
    /// stale contents are inert.
    decode_scratch: String,
    has_decoded: bool,
}

/// Returns true if `c` is an ECMAScript **WhiteSpace** code point (ES spec
/// `sec-white-space`): `<TAB>`, `<VT>`, `<FF>`, `<ZWNBSP>` (U+FEFF), and every
/// `Space_Separator` (Unicode category `Zs`, which includes `<SP>` and `<NBSP>`).
///
/// This is deliberately **not** Rust's `char::is_whitespace()` (the Unicode
/// `White_Space` property), which differs in both directions: it omits U+FEFF
/// and includes U+0085 (`<NEL>`), neither of which ECMAScript treats as
/// WhiteSpace. LineTerminators (`<LF>`/`<CR>`/`<LS>`/`<PS>`) are a separate
/// production and are matched ahead of this in `skip_whitespace`, so they are
/// intentionally absent here.
const fn is_es_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'              // <TAB>
        | '\u{000B}'            // <VT>
        | '\u{000C}'            // <FF>
        | '\u{FEFF}'            // <ZWNBSP>
        // <USP>: Unicode category Zs (Space_Separator)
        | '\u{0020}'            // SPACE
        | '\u{00A0}'            // NO-BREAK SPACE
        | '\u{1680}'
        | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        let bytes = source.as_bytes();
        // Skip UTF-8 BOM (EF BB BF / U+FEFF) at start of file if present.
        // BOM is a legacy artifact; we strip it (like deno fmt, VS Code).
        // Position starts after BOM so token spans reflect actual file bytes.
        let position = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            3
        } else {
            0
        };

        Self {
            source,
            bytes,
            position,
            template_depth: 0,
            had_line_terminator: false,
            decode_scratch: String::new(),
            has_decoded: false,
        }
    }

    /// The byte at the cursor, or `None` at EOF.
    #[inline]
    fn cur_byte(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    /// The byte `offset` bytes ahead of the cursor, or `None` past EOF.
    #[inline]
    fn byte_ahead(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.position + offset).copied()
    }

    /// Decode the full character at the cursor (for the non-ASCII branches);
    /// `None` at EOF. ASCII paths use `cur_byte` and never call this.
    #[inline]
    fn cur_char(&self) -> Option<char> {
        self.source[self.position..].chars().next()
    }

    /// Whether the cursor is on a Unicode line separator — LS (U+2028) or PS
    /// (U+2029), the two non-ASCII LineTerminators. Both lead with byte `0xE2`,
    /// so callers gate on `byte >= 0x80` before this decodes.
    #[inline]
    fn at_line_separator(&self) -> bool {
        matches!(self.cur_char(), Some('\u{2028}' | '\u{2029}'))
    }

    /// The decoded value produced for the most recently lexed token (escape paths
    /// only); `None` for the common escape-free token. Borrows the parked
    /// `decode_scratch` — valid until the next token is lexed (which may overwrite
    /// it), so the parser copies it into its AST arena immediately after each lex
    /// (`Parser::decoded_to_arena`) rather than holding the borrow.
    #[inline]
    pub fn decoded_str(&self) -> Option<&str> {
        if self.has_decoded {
            Some(&self.decode_scratch)
        } else {
            None
        }
    }

    /// Returns true if a line terminator was encountered while skipping to the current token.
    /// Used for ASI (Automatic Semicolon Insertion).
    pub fn had_line_terminator(&self) -> bool {
        self.had_line_terminator
    }

    /// Seek to a specific position and re-lex from there.
    /// Used when splitting compound tokens like `>=` into `>` + `=`.
    pub fn seek_and_next_token(&mut self, position: usize) -> Result<Token, Box<ParseError>> {
        self.set_position(position);
        self.next_token()
    }

    /// Reset the cursor to an absolute byte position (must be a char boundary).
    #[inline]
    fn set_position(&mut self, position: usize) {
        self.position = position;
    }

    /// Advance the cursor past the current character (1 byte for ASCII, more for
    /// a multi-byte UTF-8 sequence). No-op at EOF.
    #[inline]
    fn advance(&mut self) {
        if let Some(&b) = self.bytes.get(self.position) {
            self.position += utf8_len(b);
        }
    }

    /// Create a token with the current position as end
    #[inline]
    fn make_token(&self, kind: TokenKind, start: usize) -> Token {
        Token {
            kind,
            start: start as u32,
            end: self.position as u32,
        }
    }

    /// Scan an identifier that may contain unicode escapes.
    ///
    /// ECMAScript allows unicode escapes in identifiers:
    /// - `\u0066oo` → identifier `foo`
    /// - `\u{41}` → identifier `A`
    /// - `b\u0061r` → identifier `bar`
    ///
    /// The decoded name is returned in the token's `decoded` field when escapes are present.
    /// Prettier normalizes these to their decoded form.
    ///
    /// Escape-free identifiers (the overwhelmingly common case) take a byte-level
    /// ASCII fast path and never allocate: `decoded` materializes lazily on the
    /// first escape, recovering the literal prefix from the source slice.
    fn scan_identifier_into(
        &mut self,
        first_char: char,
        dst: &mut Token,
    ) -> Result<(), Box<ParseError>> {
        let start = self.position;
        // None until an actual escape is decoded; `decoded.is_some()` ⇔ has-escapes.
        let mut decoded: Option<String> = None;

        // Handle first character (already validated as valid identifier start)
        if first_char == '\\' {
            // First char is a unicode escape
            if let Some((ch, len)) = try_decode_unicode_escape(self.source, self.position) {
                if !is_id_start(ch) {
                    return Err(lex_err(
                        format!("Invalid identifier start character from unicode escape: '{ch}'"),
                        start,
                    ));
                }
                decoded = Some(String::from(ch));
                // Advance by the escape sequence length
                for _ in 0..len {
                    self.advance();
                }
            } else {
                return Err(lex_err("Invalid unicode escape in identifier", start));
            }
        } else {
            self.advance();

            // ASCII fast path: tight byte loop over `[a-zA-Z0-9_$]` (the ASCII
            // subset of IdentifierPart), then resync the cursor once.
            // Bails to the general loop on the first non-ASCII byte or `\`.
            if first_char.is_ascii() {
                let bytes = self.bytes;
                let mut pos = self.position;
                while pos < bytes.len() && is_ascii_id_continue(bytes[pos]) {
                    pos += 1;
                }
                if pos != self.position {
                    self.set_position(pos);
                }
            }
        }

        // Continue scanning identifier characters (including escapes). After the
        // fast path this also serves as the terminator check — the first iteration
        // breaks unless the identifier continues with a non-ASCII char or escape.
        loop {
            match self.cur_byte() {
                Some(b'\\') => {
                    // Potential unicode escape in identifier
                    if let Some((ch, len)) = try_decode_unicode_escape(self.source, self.position) {
                        if !is_id_continue(ch) {
                            // Not a valid identifier continue char, stop here
                            break;
                        }
                        // First escape: everything consumed so far was literal.
                        decoded
                            .get_or_insert_with(|| self.source[start..self.position].to_string())
                            .push(ch);
                        for _ in 0..len {
                            self.advance();
                        }
                    } else {
                        // Not a valid escape, stop identifier scanning
                        break;
                    }
                }
                // ASCII byte (the overwhelmingly common case): after the fast path this
                // is almost always the terminator, so settle it from the byte LUT alone —
                // no char decode, no cross-crate Unicode `is_id_continue` dispatch. The
                // LUT equals `is_id_continue` on ASCII (the same equivalence the fast path
                // above relies on), so this stays exact. The escape path (decoded is
                // `Some`) re-consumes ASCII identifier parts through here too.
                Some(b) if b < 0x80 => {
                    if is_ascii_id_continue(b) {
                        if let Some(d) = &mut decoded {
                            d.push(b as char);
                        }
                        self.advance();
                    } else {
                        break;
                    }
                }
                // Non-ASCII lead byte: decode the full char and continue while it is an
                // IdentifierPart.
                Some(_) => match self.cur_char() {
                    Some(ch) if is_id_continue(ch) => {
                        if let Some(d) = &mut decoded {
                            d.push(ch);
                        }
                        self.advance();
                    }
                    _ => break,
                },
                None => break,
            }
        }

        // Check if it's a keyword (only if no escapes - escaped keywords are identifiers;
        // without escapes the source slice IS the name, so no decoded buffer is needed).
        // SWAR recognition over the identifier's raw bytes (no `&str` reslice, no
        // hashing — SWAR covers every reserved word length); see `keyword_at`.
        let kind = if decoded.is_none() {
            match keyword_at(self.bytes, start, self.position - start) {
                Some(kw) => TokenKind::Keyword(kw),
                None => TokenKind::Identifier,
            }
        } else {
            // Escaped identifiers are never keywords: `\u0063lass` is identifier "class", not keyword
            TokenKind::Identifier
        };

        // Escaped identifiers are near-zero in real code; funnel the rare local
        // buffer into the parked scratch so `decoded_str` reads it uniformly.
        match decoded {
            Some(s) => {
                self.decode_scratch.clear();
                self.decode_scratch.push_str(&s);
                self.has_decoded = true;
            }
            None => self.has_decoded = false,
        }
        *dst = Token {
            kind,
            start: start as u32,
            end: self.position as u32,
        };
        Ok(())
    }

    /// Scan digits matching a predicate, validating numeric separators (`_`).
    /// Per the ECMAScript lexical grammar a `NumericLiteralSeparator` must sit
    /// *between two digits*, so a `_` is rejected at the start of the group, at
    /// the end, when doubled, or adjacent to a prefix/`.`/`e` — the placement
    /// over-acceptances acorn flags as "Numeric separator is not allowed …".
    /// Returns whether at least one digit was consumed (callers enforce the
    /// "≥1 digit after a radix prefix" rule).
    fn scan_digits(
        &mut self,
        is_valid_digit: impl Fn(char) -> bool,
    ) -> Result<bool, Box<ParseError>> {
        // Digits and `_` are ASCII, so a byte scan suffices: a non-ASCII byte
        // (`b as char` ∈ U+0080..=U+00FF) is never a valid digit, so the predicate
        // breaks the loop just as it would on any other terminator.
        let mut saw_digit = false;
        let mut prev_was_digit = false;
        while let Some(b) = self.cur_byte() {
            if b == b'_' {
                // A separator is valid only with a digit on each side.
                let next_is_digit = self
                    .byte_ahead(1)
                    .is_some_and(|n| is_valid_digit(n as char));
                if !prev_was_digit || !next_is_digit {
                    return Err(lex_err(
                        "Numeric separator '_' must appear between two digits",
                        self.position,
                    ));
                }
                self.advance();
                prev_was_digit = false;
            } else if is_valid_digit(b as char) {
                self.advance();
                saw_digit = true;
                prev_was_digit = true;
            } else {
                break;
            }
        }
        Ok(saw_digit)
    }

    /// Scan a decimal number (integer, float, or scientific notation).
    /// Handles: 123, 1.5, 1e3, 1.5e-2, 1_000, 1.e1. Returns whether the literal
    /// is integer-form (no fractional part and no exponent) — the only decimal
    /// shape a BigInt `n` suffix may follow (`1.5n` / `1e3n` are rejected).
    fn scan_decimal_number(&mut self) -> Result<bool, Box<ParseError>> {
        // `s` starts at the char after a `.`. Returns true when `s` begins a valid
        // exponent (`e`/`E`, optional sign, then a digit) — i.e. `1.e1` is one number.
        fn is_exponent_start(s: &str) -> bool {
            let mut chars = s.chars();
            if !matches!(chars.next(), Some('e' | 'E')) {
                return false;
            }
            let mut c = chars.next();
            if matches!(c, Some('+' | '-')) {
                c = chars.next();
            }
            c.is_some_and(|c| c.is_ascii_digit())
        }

        let mut is_integer = true;

        // Integer part (with optional separators)
        self.scan_digits(|c| c.is_ascii_digit())?;

        // Decimal point and fractional part
        if self.cur_byte() == Some(b'.') {
            // Peek ahead: the common `3.14` case only needs the next byte — an ASCII
            // digit is single-byte, so the byte test equals decoding the char, with no
            // subslice/decode. The rarer trailing-dot-exponent (`1.e1`) path below still
            // reads the `&str` tail.
            if self.byte_ahead(1).is_some_and(|b| b.is_ascii_digit()) {
                // Normal decimal: 3.14
                is_integer = false;
                self.advance(); // consume '.'
                self.scan_digits(|c| c.is_ascii_digit())?;
            } else if is_exponent_start(&self.source[self.position + 1..]) {
                // Trailing-dot exponent: `1.e1` is a single numeric literal (= 1e1).
                // Consume the '.'; the exponent block below consumes `e1`.
                // Without this, `1.e1` would lex as `1.` followed by member access `.e1`.
                is_integer = false;
                self.advance(); // consume '.'
            } else {
                // Trailing decimal: `5.` / `0.` (operator, punctuation, or end),
                // `5..foo` / `0..toString()` (the next `.` is member access). The
                // `.` is greedily the decimal point (maximal munch), so consume it.
                // The boundary check at the end of `scan_number_into` then rejects
                // an IdentifierStart abutting the `.` (`5.foo` / `10._1` / `5.in`).
                is_integer = false;
                self.advance(); // consume '.'
            }
        }

        // Exponent part: e+10, E-3, e10
        if matches!(self.cur_byte(), Some(b'e' | b'E')) {
            is_integer = false;
            self.advance(); // consume 'e' or 'E'
            // Optional sign
            if matches!(self.cur_byte(), Some(b'+' | b'-')) {
                self.advance();
            }
            self.scan_digits(|c| c.is_ascii_digit())?;
        }

        Ok(is_integer)
    }

    /// Scan a numeric literal — decimal, `0x`/`0b`/`0o` radix, float, exponent,
    /// or `BigInt` suffix — writing the `Number` token into `*dst`. `first` is the
    /// byte at `start` the dispatch matched; it is read only to detect a leading-
    /// `0` radix prefix, so it is a digit for `5`/`0x…` or `.` for a leading-dot
    /// fraction (`.5`) — both non-`0`, both routing to `scan_decimal_number`. The
    /// single number entry point, so the "identifier directly after a number"
    /// boundary rule (ecma262 12.9.3) lives here once. Mirrors the `_into`
    /// write-through of the other large scanners so the dispatch arm is one
    /// `return`. Errors on the strict-mode-illegal leading-zero literals — legacy
    /// octal (`0777`), non-octal decimal (`08`/`09`), and the separator forms
    /// (`0_0`) — all disallowed because tsv is strict-only.
    fn scan_number_into(
        &mut self,
        start: usize,
        first: u8,
        dst: &mut Token,
    ) -> Result<(), Box<ParseError>> {
        // Radix literals (`0x`/`0b`/`0o`) are always integer-form, so a BigInt
        // `n` suffix is always allowed after them; a decimal literal allows `n`
        // only when it carries no fraction and no exponent.
        let mut bigint_allowed = true;
        if first == b'0' {
            match self.byte_ahead(1) {
                Some(b'x' | b'X') => {
                    // Hex: 0xff, 0xFF
                    self.advance(); // consume '0'
                    self.advance(); // consume 'x'
                    if !self.scan_digits(|c| c.is_ascii_hexdigit())? {
                        return Err(lex_err("Missing hexadecimal digits after '0x'", start));
                    }
                }
                Some(b'b' | b'B') => {
                    // Binary: 0b1010
                    self.advance(); // consume '0'
                    self.advance(); // consume 'b'
                    if !self.scan_digits(|c| c == '0' || c == '1')? {
                        return Err(lex_err("Missing binary digits after '0b'", start));
                    }
                }
                Some(b'o' | b'O') => {
                    // Octal: 0o77
                    self.advance(); // consume '0'
                    self.advance(); // consume 'o'
                    if !self.scan_digits(|c| ('0'..='7').contains(&c))? {
                        return Err(lex_err("Missing octal digits after '0o'", start));
                    }
                }
                Some(b'0'..=b'9') => {
                    // Leading-zero decimal literals — `LegacyOctalIntegerLiteral`
                    // (`010`, all octal digits) and `NonOctalDecimalIntegerLiteral`
                    // (`08`/`09`/`089`, containing an 8 or 9) — are disallowed in
                    // strict mode (ecma262 Annex B.1.1; the strict early error at
                    // sec-additional-syntax-numeric-literals). ES modules are always
                    // strict and tsv has no sloppy mode, so both always reject. A
                    // leading `0` is a valid literal only before `.`/`e`/`n`/a radix
                    // prefix/end — never before another digit.
                    return Err(lex_err(
                        "Leading-zero literals are not allowed in strict mode. Use '0o' for octal.",
                        start,
                    ));
                }
                Some(b'_') => {
                    // A `NumericLiteralSeparator` cannot appear in a leading-zero
                    // legacy form (`0_0`/`0_8` — the `LegacyOctalLikeDecimalInteger`
                    // productions carry no `[Sep]` parameter), so a `_` immediately
                    // after a leading `0` is rejected.
                    return Err(lex_err(
                        "Numeric separators are not allowed in legacy octal-like literals",
                        start,
                    ));
                }
                _ => {
                    // Regular number or float starting with 0 (e.g. `0`, `0.5`,
                    // `0e1`, `0n`) — the leading `0` is followed by `.`/`e`/`n`/a
                    // radix prefix/end, none of which are a digit or `_`.
                    bigint_allowed = self.scan_decimal_number()?;
                }
            }
        } else {
            // Regular decimal number
            bigint_allowed = self.scan_decimal_number()?;
        }

        // BigInt suffix `n` attaches only to an integer-form literal. When `n`
        // follows a float/exponent we leave it unconsumed so the adjacent
        // identifier triggers the normal parse-level rejection (as for `5abc`),
        // matching acorn's "Identifier directly after number".
        if bigint_allowed && self.cur_byte() == Some(b'n') {
            self.advance();
        }

        // ecma262 12.9.3: "The SourceCharacter immediately following a
        // NumericLiteral must not be an IdentifierStart or DecimalDigit" — the
        // spec's own example is that `3in` is an error, not the two tokens `3` and
        // `in`. Enforcing the IdentifierStart half here (the single number entry)
        // rejects a number abutting a keyword-operator (`5in` / `1.5in` / `0xffin`
        // / `5nin` / `.5in`) rather than reading it as `5 in y`; the parser's
        // number→primary path only catches a following *identifier* (`5foo`), not
        // an infix keyword. A DecimalDigit can only follow a complete number after
        // an out-of-range radix digit (`0b12`) or a BigInt suffix (`5n3`), both of
        // which the parser already rejects as adjacent number tokens.
        // Byte-gate the IdentifierStart check: the byte after a number is almost
        // always an ASCII terminator (`;`, `)`, whitespace, an operator), settled from
        // the LUT with no char decode + no cross-crate `is_id_start` dispatch. The LUT
        // equals `is_id_start` on ASCII (the same tested invariant the identifier fast
        // paths rely on); only a non-ASCII lead byte decodes the full char.
        let id_start_follows = match self.cur_byte() {
            Some(b) if b < 0x80 => is_ascii_id_start(b),
            Some(_) => self.cur_char().is_some_and(is_id_start),
            None => false,
        };
        if id_start_follows {
            return Err(lex_err("Identifier directly after number", self.position));
        }

        *dst = self.make_token(TokenKind::Number, start);
        Ok(())
    }

    /// Scan a single- or double-quoted string literal (cursor on the opening
    /// `quote` byte), writing the `String` token into `*dst` and the decoded value
    /// out-of-band via the parked `decode_scratch` only when it contains escapes. Mirrors the
    /// `_into` write-through of the other large scanners.
    ///
    /// The inner run skips everything that is neither the close quote nor a
    /// backslash — a 2-byte search the compiler auto-vectorizes. Byte-at-a-time is
    /// sound: quote and `\` are ASCII (`< 0x80`) and so never appear as a UTF-8
    /// continuation byte. `has_escapes` gates the (rare) decode pass.
    fn scan_string_into(
        &mut self,
        start: usize,
        quote: u8,
        dst: &mut Token,
    ) -> Result<(), Box<ParseError>> {
        self.advance(); // consume opening quote
        let content_start = self.position;

        let bytes = self.bytes;
        let len = bytes.len();
        let mut p = content_start;
        let mut has_escapes = false;
        loop {
            while p < len && bytes[p] != quote && bytes[p] != b'\\' {
                p += 1;
            }
            if p >= len {
                // Unterminated string
                self.position = p;
                return Err(lex_err("Unterminated string literal", start));
            }
            if bytes[p] == quote {
                let content_end = p;
                p += 1; // consume closing quote
                self.position = p;

                let content = &self.source[content_start..content_end];

                // Decode escape sequences into the parked scratch (no per-literal
                // String/Box allocation); `has_decoded` was cleared at entry and is
                // set only when this token actually carries escapes.
                if has_escapes {
                    escapes::decode_string_escapes_into(content, &mut self.decode_scratch)
                        .map_err(Box::new)?;
                    self.has_decoded = true;
                }
                *dst = Token {
                    kind: TokenKind::String,
                    start: start as u32,
                    end: p as u32,
                };
                return Ok(());
            }
            // bytes[p] == b'\\': escape — skip the backslash and the next
            // character (decode_string_escapes validates it later). Advance
            // past a full char so a multibyte escaped char resumes the inner
            // scan on a char boundary.
            has_escapes = true;
            p += 1;
            if p < len {
                p += utf8_len(bytes[p]);
            }
        }
    }

    fn skip_whitespace(&mut self) {
        self.had_line_terminator = false;
        loop {
            match self.cur_byte() {
                // ASCII fast paths (the overwhelming common case).
                Some(b'\n') => {
                    // LF — line terminator (ES spec 12.3)
                    self.had_line_terminator = true;
                    self.advance();
                }
                Some(b'\r') => {
                    // CR — line terminator; collapse CRLF into one
                    self.had_line_terminator = true;
                    self.advance();
                    if self.cur_byte() == Some(b'\n') {
                        self.advance();
                    }
                }
                // SPACE / TAB / VT / FF — the ASCII subset of WhiteSpace
                Some(b' ' | b'\t' | 0x0B | 0x0C) => {
                    self.advance();
                }
                // Any other ASCII byte is not whitespace — stop.
                Some(b) if b < 0x80 => break,
                // Non-ASCII lead byte: decode to classify against the Unicode rules
                // (LS/PS line terminators, plus NBSP/ZWNBSP/Zs whitespace).
                Some(_) => match self.cur_char() {
                    // LS (Line Separator) / PS (Paragraph Separator)
                    Some('\u{2028}' | '\u{2029}') => {
                        self.had_line_terminator = true;
                        self.advance();
                    }
                    Some(c) if is_es_whitespace(c) => self.advance(),
                    _ => break,
                },
                None => break,
            }
        }
    }

    // TODO: Expand token support for:
    // - Operators: +, -, *, /, %, ==, !=, <, >, <=, >=, &&, ||, !, &, |, ^, ~, <<, >>, >>>, ?, ?.
    // - Delimiters: ( ) { } [ ] , . ...
    // - String literals: "..." '...' `...` (with escapes)
    // - Comments: // and /* */
    // - More number formats: floats (1.5), hex (0x10), binary (0b10), octal (0o10)
    // - Template literals: `hello ${world}`
    // - Regular expressions: /pattern/flags
    /// Lex the next token directly into `*dst` — the hot advance path. Writing
    /// through the caller's slot (`&mut self.current`) instead of returning a
    /// `Result<Token>` keeps the 16-byte token in registers and elides the sret
    /// round-trip the by-value return forces (the intermediate `Token` built on
    /// the lexer frame, returned, and re-scattered). The match yields a `Token`
    /// value only for the short punctuation/operator paths; the
    /// identifier/number/string/template/hashbang scanners and the error paths
    /// write `dst` (or propagate the error) via an early `return`.
    /// [`Lexer::next_token`] is the thin by-value wrapper kept for the
    /// peek/seek/bootstrap callers.
    pub fn next_token_into(&mut self, dst: &mut Token) -> Result<(), Box<ParseError>> {
        // Clear the decoded-value flag from the previous token so `decoded_str`
        // reflects only the token produced by this call (set by the escape paths below).
        self.has_decoded = false;
        self.skip_whitespace();

        let start = self.position;

        *dst = match self.cur_byte() {
            None => Token {
                kind: TokenKind::Eof,
                start: start as u32,
                end: start as u32,
            },
            Some(b';') => {
                self.advance();
                self.make_token(TokenKind::Semicolon, start)
            }
            Some(b':') => {
                self.advance();
                self.make_token(TokenKind::Colon, start)
            }
            Some(b'=') => {
                self.advance();
                match self.cur_byte() {
                    Some(b'>') => {
                        // =>
                        self.advance();
                        self.make_token(TokenKind::Arrow, start)
                    }
                    Some(b'=') => {
                        self.advance();
                        if self.cur_byte() == Some(b'=') {
                            // ===
                            self.advance();
                            self.make_token(TokenKind::EqualsEqualsEquals, start)
                        } else {
                            // ==
                            self.make_token(TokenKind::EqualsEquals, start)
                        }
                    }
                    _ => {
                        // =
                        self.make_token(TokenKind::Equals, start)
                    }
                }
            }
            Some(b) if b.is_ascii_digit() => return self.scan_number_into(start, b, dst),
            // ECMAScript identifiers: start with ID_Start, _, or $; continue with ID_Continue or $
            // Note: _ is in ID_Continue but not ID_Start, so we check it explicitly for start
            // Identifiers may contain unicode escapes: \u0066oo → foo, b\u0061r → bar
            Some(b) if is_ascii_id_start(b) => return self.scan_identifier_into(b as char, dst),
            // Unicode escape at start of identifier: \u0066oo → foo
            Some(b'\\') => {
                // Check if this is a valid unicode escape that decodes to an identifier start
                if let Some((ch, _)) = try_decode_unicode_escape(self.source, self.position)
                    && is_id_start(ch)
                {
                    return self.scan_identifier_into('\\', dst);
                }
                // Not a valid identifier start - error.
                return Err(lex_err("Unexpected character: '\\'", start));
            }
            Some(quote @ (b'\'' | b'"')) => return self.scan_string_into(start, quote, dst),
            Some(b',') => {
                self.advance();
                self.make_token(TokenKind::Comma, start)
            }
            Some(b'{') => {
                self.advance();
                self.make_token(TokenKind::BraceOpen, start)
            }
            Some(b'}') => {
                self.advance();
                self.make_token(TokenKind::BraceClose, start)
            }
            Some(b'[') => {
                self.advance();
                self.make_token(TokenKind::BracketOpen, start)
            }
            Some(b']') => {
                self.advance();
                self.make_token(TokenKind::BracketClose, start)
            }
            Some(b'(') => {
                self.advance();
                self.make_token(TokenKind::ParenOpen, start)
            }
            Some(b')') => {
                self.advance();
                self.make_token(TokenKind::ParenClose, start)
            }
            Some(b'.') => {
                // `.`, `..`, `...` and digits are all ASCII, so peek the next two bytes.
                if self.byte_ahead(1) == Some(b'.') && self.byte_ahead(2) == Some(b'.') {
                    // Spread operator: ...
                    self.advance(); // consume first .
                    self.advance(); // consume second .
                    self.advance(); // consume third .
                    self.make_token(TokenKind::DotDotDot, start)
                } else if self.byte_ahead(1).is_some_and(|b| b.is_ascii_digit()) {
                    // Number starting with a decimal point (`.5`, `.5e3`). Route it
                    // through the one number entry with `.` as `first` (a non-`0`
                    // byte → empty integer part → fraction/exponent), so leading-dot
                    // fractions share `scan_number_into`'s separator/exponent and
                    // boundary handling instead of a parallel scan that can drift.
                    return self.scan_number_into(start, b'.', dst);
                } else {
                    // Single dot: member access operator
                    self.advance();
                    self.make_token(TokenKind::Dot, start)
                }
            }
            Some(b'-') => {
                self.advance();
                if self.cur_byte() == Some(b'-') {
                    self.advance();
                    self.make_token(TokenKind::MinusMinus, start)
                } else if self.cur_byte() == Some(b'=') {
                    self.advance();
                    self.make_token(TokenKind::MinusEquals, start)
                } else {
                    self.make_token(TokenKind::Minus, start)
                }
            }
            Some(b'+') => {
                self.advance();
                if self.cur_byte() == Some(b'+') {
                    self.advance();
                    self.make_token(TokenKind::PlusPlus, start)
                } else if self.cur_byte() == Some(b'=') {
                    self.advance();
                    self.make_token(TokenKind::PlusEquals, start)
                } else {
                    self.make_token(TokenKind::Plus, start)
                }
            }
            Some(b'/') => {
                // Could be: // line comment, /* block comment */, or / division operator
                // Peek ahead to determine which
                let peek = self.byte_ahead(1);
                match peek {
                    Some(b'/') => {
                        // Line comment
                        let mut pos = self.position;
                        let token = comments::read_line_comment(self.source, &mut pos)?;
                        self.set_position(pos);
                        token
                    }
                    Some(b'*') => {
                        // Block comment
                        let mut pos = self.position;
                        let token = comments::read_block_comment(self.source, &mut pos)?;
                        self.set_position(pos);
                        token
                    }
                    Some(b'=') => {
                        // Division assignment operator /=
                        self.advance();
                        self.advance();
                        self.make_token(TokenKind::SlashEquals, start)
                    }
                    _ => {
                        // Division operator /
                        self.advance();
                        self.make_token(TokenKind::Slash, start)
                    }
                }
            }
            Some(b'*') => {
                self.advance();
                if self.cur_byte() == Some(b'*') {
                    self.advance();
                    if self.cur_byte() == Some(b'=') {
                        self.advance();
                        self.make_token(TokenKind::StarStarEquals, start)
                    } else {
                        self.make_token(TokenKind::StarStar, start)
                    }
                } else if self.cur_byte() == Some(b'=') {
                    self.advance();
                    self.make_token(TokenKind::StarEquals, start)
                } else {
                    self.make_token(TokenKind::Star, start)
                }
            }
            Some(b'%') => {
                self.advance();
                if self.cur_byte() == Some(b'=') {
                    self.advance();
                    self.make_token(TokenKind::PercentEquals, start)
                } else {
                    self.make_token(TokenKind::Percent, start)
                }
            }
            Some(b'^') => {
                self.advance();
                if self.cur_byte() == Some(b'=') {
                    self.advance();
                    self.make_token(TokenKind::CaretEquals, start)
                } else {
                    self.make_token(TokenKind::Caret, start)
                }
            }
            Some(b'~') => {
                self.advance();
                self.make_token(TokenKind::Tilde, start)
            }
            Some(b'<') => {
                self.advance();
                if self.cur_byte() == Some(b'=') {
                    self.advance();
                    self.make_token(TokenKind::LessThanEquals, start)
                } else if self.cur_byte() == Some(b'<') {
                    self.advance();
                    if self.cur_byte() == Some(b'=') {
                        self.advance();
                        self.make_token(TokenKind::LeftShiftEquals, start)
                    } else {
                        self.make_token(TokenKind::LeftShift, start)
                    }
                } else {
                    self.make_token(TokenKind::LessThan, start)
                }
            }
            Some(b'>') => {
                self.advance();
                if self.cur_byte() == Some(b'=') {
                    self.advance();
                    self.make_token(TokenKind::GreaterThanEquals, start)
                } else if self.cur_byte() == Some(b'>') {
                    self.advance();
                    if self.cur_byte() == Some(b'>') {
                        // >>> or >>>=
                        self.advance();
                        if self.cur_byte() == Some(b'=') {
                            self.advance();
                            self.make_token(TokenKind::UnsignedRightShiftEquals, start)
                        } else {
                            self.make_token(TokenKind::UnsignedRightShift, start)
                        }
                    } else if self.cur_byte() == Some(b'=') {
                        // >>=
                        self.advance();
                        self.make_token(TokenKind::RightShiftEquals, start)
                    } else {
                        // >>
                        self.make_token(TokenKind::RightShift, start)
                    }
                } else {
                    self.make_token(TokenKind::GreaterThan, start)
                }
            }
            Some(b'!') => {
                self.advance();
                if self.cur_byte() == Some(b'=') {
                    self.advance();
                    if self.cur_byte() == Some(b'=') {
                        self.advance();
                        self.make_token(TokenKind::BangEqualsEquals, start)
                    } else {
                        self.make_token(TokenKind::BangEquals, start)
                    }
                } else {
                    self.make_token(TokenKind::Bang, start)
                }
            }
            Some(b'&') => {
                self.advance();
                if self.cur_byte() == Some(b'&') {
                    self.advance();
                    if self.cur_byte() == Some(b'=') {
                        self.advance();
                        self.make_token(TokenKind::AmpersandAmpersandEquals, start)
                    } else {
                        self.make_token(TokenKind::AmpersandAmpersand, start)
                    }
                } else if self.cur_byte() == Some(b'=') {
                    self.advance();
                    self.make_token(TokenKind::AmpersandEquals, start)
                } else {
                    self.make_token(TokenKind::Ampersand, start)
                }
            }
            Some(b'|') => {
                self.advance();
                if self.cur_byte() == Some(b'|') {
                    self.advance();
                    if self.cur_byte() == Some(b'=') {
                        self.advance();
                        self.make_token(TokenKind::PipePipeEquals, start)
                    } else {
                        self.make_token(TokenKind::PipePipe, start)
                    }
                } else if self.cur_byte() == Some(b'=') {
                    self.advance();
                    self.make_token(TokenKind::PipeEquals, start)
                } else {
                    self.make_token(TokenKind::Pipe, start)
                }
            }
            Some(b'?') => {
                self.advance();
                if self.cur_byte() == Some(b'?') {
                    self.advance();
                    if self.cur_byte() == Some(b'=') {
                        self.advance();
                        self.make_token(TokenKind::QuestionQuestionEquals, start)
                    } else {
                        self.make_token(TokenKind::QuestionQuestion, start)
                    }
                } else if self.cur_byte() == Some(b'.') {
                    // Check for optional chaining `?.`
                    // Must not be followed by a digit (to avoid ambiguity with `?.0` which should be `?` `.0`)
                    // Cursor is on `.`; the byte after it is `position + 1`.
                    let next = self.byte_ahead(1);
                    if next.is_none_or(|b| !b.is_ascii_digit()) {
                        self.advance();
                        self.make_token(TokenKind::QuestionDot, start)
                    } else {
                        // `?.0` should be `?` followed by `.0` (number)
                        self.make_token(TokenKind::Question, start)
                    }
                } else {
                    self.make_token(TokenKind::Question, start)
                }
            }
            Some(b'`') => {
                // Template literal starting with backtick
                return self.read_template_into(start, dst);
            }
            Some(b'@') => {
                // @ for decorators
                self.advance();
                self.make_token(TokenKind::At, start)
            }
            Some(b'#') => {
                // Check for hashbang at start of file: #!/usr/bin/env node
                if start == 0 {
                    let next = self.source.get(1..2);
                    if next == Some("!") {
                        // Hashbang comment - read until end of line
                        return self.read_hashbang_into(start, dst);
                    }
                }
                // # for private identifiers
                self.advance();
                self.make_token(TokenKind::Hash, start)
            }
            // Non-ASCII lead byte: a Unicode IdentifierStart, otherwise an error.
            // (The ASCII id-start arm above handles `a-z A-Z _ $`; this decodes the
            // char for the Unicode `is_id_start` check — the one token-start decode.)
            Some(b) if b >= 0x80 => match self.cur_char() {
                Some(ch) if is_id_start(ch) => return self.scan_identifier_into(ch, dst),
                Some(ch) => {
                    return Err(lex_err(format!("Unexpected character: '{ch}'"), start));
                }
                None => return Err(lex_err("Unexpected character", start)),
            },
            Some(b) => {
                return Err(lex_err(
                    format!("Unexpected character: '{}'", b as char),
                    start,
                ));
            }
        };
        Ok(())
    }

    /// By-value next-token for the peek/seek/bootstrap callers. The hot advance
    /// path uses [`Lexer::next_token_into`] to write the parser cursor in place.
    pub fn next_token(&mut self) -> Result<Token, Box<ParseError>> {
        let mut tok = Token {
            kind: TokenKind::Eof,
            start: 0,
            end: 0,
        };
        self.next_token_into(&mut tok)?;
        Ok(tok)
    }

    /// Scan one template segment body over raw bytes, starting at `content_start`
    /// (just past the opening `` ` `` or `}`). Returns `(content_end, stop,
    /// has_escapes)`: `content_end` is the segment's content boundary and `stop`
    /// is what terminated it. On a non-EOF stop `self.position` is left just past
    /// the consumed terminator (the closing `` ` ``, or the `{` of `${`); on EOF it
    /// is left at the end.
    ///
    /// The inner run skips everything that is not `` ` `` / `$` / `\` — a 3-byte
    /// search the compiler auto-vectorizes. Byte-at-a-time is sound: all three are
    /// ASCII (`< 0x80`) and so never appear as a UTF-8 continuation byte. A `\`
    /// skips itself plus the next full char (a multibyte escaped char resumes the
    /// scan on a char boundary); the escape is validated later when the segment is
    /// decoded (`decode_string_escapes_into`). Depth tracking (`${` push) stays with the caller.
    fn scan_template_body(&mut self, content_start: usize) -> (usize, TemplateStop, bool) {
        let bytes = self.bytes;
        let len = bytes.len();
        let mut p = content_start;
        let mut has_escapes = false;
        loop {
            while p < len && bytes[p] != b'`' && bytes[p] != b'$' && bytes[p] != b'\\' {
                p += 1;
            }
            if p >= len {
                self.position = p;
                return (p, TemplateStop::Eof, has_escapes);
            }
            match bytes[p] {
                b'`' => {
                    let content_end = p;
                    self.position = p + 1; // consume closing `
                    return (content_end, TemplateStop::Backtick, has_escapes);
                }
                b'$' if bytes.get(p + 1) == Some(&b'{') => {
                    let content_end = p;
                    self.position = p + 2; // consume ${
                    return (content_end, TemplateStop::Interpolation, has_escapes);
                }
                b'$' => p += 1, // lone $
                _ => {
                    // backslash — skip it and the escaped char (full width)
                    has_escapes = true;
                    p += 1;
                    if p < len {
                        p += utf8_len(bytes[p]);
                    }
                }
            }
        }
    }

    /// Read template literal content.
    ///
    /// Called when we see a backtick (start of template) or after reading `}` in template context.
    /// Returns one of:
    /// - NoSubstitutionTemplate: Complete template with no interpolation
    /// - TemplateHead: Start of template with `${` interpolation
    /// - TemplateMiddle: Middle section between interpolations (}...${)
    /// - TemplateTail: End section after last interpolation (}...`)
    fn read_template_into(&mut self, start: usize, dst: &mut Token) -> Result<(), Box<ParseError>> {
        self.advance(); // consume opening ` or }

        let content_start = self.position;
        let (content_end, stop, has_escapes) = self.scan_template_body(content_start);

        let kind = match stop {
            TemplateStop::Eof => {
                return Err(lex_err("Unterminated template literal", start));
            }
            // Determine token type based on whether we started with ` or }. (This
            // entry point is only reached on a leading `` ` ``, so `is_head` is
            // always true here; the check is kept exact for clarity/robustness.)
            TemplateStop::Backtick => {
                if self.source[start..].starts_with('`') {
                    TokenKind::NoSubstitutionTemplate
                } else {
                    TokenKind::TemplateTail
                }
            }
            TemplateStop::Interpolation => {
                self.template_depth += 1;
                if self.source[start..].starts_with('`') {
                    TokenKind::TemplateHead
                } else {
                    TokenKind::TemplateMiddle
                }
            }
        };

        let content = &self.source[content_start..content_end];
        // Decode into the parked scratch. A bad escape (deferred for tagged
        // templates) leaves `has_decoded` false — same as a no-escape segment; the
        // parser distinguishes them by the presence of a backslash in `content`.
        if has_escapes
            && escapes::decode_string_escapes_into(content, &mut self.decode_scratch).is_ok()
        {
            self.has_decoded = true;
        }

        *dst = Token {
            kind,
            start: start as u32,
            end: self.position as u32,
        };
        Ok(())
    }

    /// Read a regex literal starting from a `/` or `/=` token.
    ///
    /// Called by the parser when it determines that `/` or `/=` should be a regex, not division.
    /// The parser passes the start position of the token it received.
    ///
    /// The lexer syncs to that position and reads `/pattern/flags`.
    /// For `/=` tokens, the `=` becomes the first character of the pattern (e.g., `/=\s*/`).
    ///
    /// Pattern and flags are verbatim source slices (escapes preserved), so the
    /// parser recovers them from spans rather than the token. Returns the token
    /// plus the position of the closing `/` (the pattern/flags boundary), letting
    /// the parser slice `[slash_start+1, close]` and `[close+1, end]` without the
    /// caller ever materializing the strings (`token.decoded` is `None`).
    pub fn read_regex_literal(
        &mut self,
        slash_start: usize,
    ) -> Result<(Token, usize), Box<ParseError>> {
        // A regex token never carries a decoded value; clear any left from the
        // previous token so the parser's `decoded_str()` after this lex sees `None`.
        self.has_decoded = false;
        // Sync to just after the opening /
        self.set_position(slash_start + 1);

        let pattern_start = self.position;
        let mut in_class = false; // Inside character class [...]
        let mut escaped = false; // Previous char was \

        // Read pattern until unescaped / outside character class.
        //
        // The pattern body is deliberately NOT validated here, and this loose scan is
        // exactly what the spec asks a lexer for: the `RegularExpressionBody` productions
        // exist so "the input element scanner [can] find the end of the regular expression
        // literal", and the body/flags "are subsequently parsed again using the more
        // stringent ECMAScript Regular Expression grammar" (ecma262
        // sec-literals-regular-expression-literals). That second parse failing is an
        // *early error* — `IsValidRegularExpressionLiteral` — not a grammar error, so it
        // belongs to the diagnostics layer with the other deferred early errors, and the
        // formatter (which only ever re-emits the body verbatim) never needs it.
        loop {
            match self.cur_byte() {
                None => {
                    return Err(lex_err(
                        "Unterminated regular expression literal",
                        slash_start,
                    ));
                }
                // Line terminators are not allowed in a regex — checked BEFORE the
                // `escaped` arm so even `\<LS>` errors (matching the original order).
                Some(b'\n' | b'\r') => {
                    return Err(lex_err(
                        "Unterminated regular expression literal",
                        slash_start,
                    ));
                }
                Some(b) if b >= 0x80 && self.at_line_separator() => {
                    return Err(lex_err(
                        "Unterminated regular expression literal",
                        slash_start,
                    ));
                }
                Some(_) if escaped => {
                    // Escaped character - consume and continue
                    escaped = false;
                    self.advance();
                }
                Some(b'\\') => {
                    escaped = true;
                    self.advance();
                }
                Some(b'[') if !in_class => {
                    in_class = true;
                    self.advance();
                }
                Some(b']') if in_class => {
                    in_class = false;
                    self.advance();
                }
                Some(b'/') if !in_class => {
                    // End of pattern
                    break;
                }
                Some(_) => {
                    self.advance();
                }
            }
        }

        let pattern_end = self.position;
        let pattern = &self.source[pattern_start..pattern_end];

        // Check for empty pattern (would be a comment)
        if pattern.is_empty() {
            return Err(lex_err(
                "Regular expression literal cannot be empty (use /(?:)/ for empty pattern)",
                slash_start,
            ));
        }

        self.advance(); // Consume closing /

        // Read flags. This IS the whole spec production — `RegularExpressionFlags ::
        // [empty] | RegularExpressionFlags IdentifierPartChar`, and `IdentifierPartChar ::
        // UnicodeIDContinue | $`. Note what that excludes: there is no backslash, so an
        // escaped flag is not a flags production at all and correctly fails here, as it
        // does in acorn — there is nothing to "support".
        //
        // Which flags are *legal* (`d`/`g`/`i`/`m`/`s`/`u`/`v`/`y`, each at most once) is
        // not this loop's business: that is the first two steps of the early error
        // `IsValidRegularExpressionLiteral`, deferred to the diagnostics layer along with
        // the pattern parse (see the body scan above). So `/a/qqq` and `/a/gg` lex fine.
        // The flags text is recovered from the span by the parser, not sliced here;
        // this loop only advances `self.position` to the token end.
        while let Some(b) = self.cur_byte() {
            // Flags are IdentifierPart (mostly ASCII d/g/i/m/s/u/v/y); decode only
            // for a non-ASCII byte before the Unicode `is_id_continue` check.
            let Some(ch) = (if b < 0x80 {
                Some(b as char)
            } else {
                self.cur_char()
            }) else {
                break;
            };
            if is_id_continue(ch) {
                self.advance();
            } else {
                break;
            }
        }

        Ok((
            Token {
                kind: TokenKind::RegexLiteral,
                start: slash_start as u32,
                end: self.position as u32,
            },
            pattern_end,
        ))
    }

    /// Continue reading template after an interpolation expression.
    ///
    /// Called by the parser after parsing the expression inside `${}`.
    /// The parser has seen the closing `}` but hasn't called advance().
    ///
    /// `brace_end` is the position just after the `}` where template content starts.
    /// The lexer will sync to this position and read the rest of the template.
    pub fn continue_template_from_brace(
        &mut self,
        brace_end: usize,
    ) -> Result<Token, Box<ParseError>> {
        // Standalone token-producing entry point — clear the out-of-band decoded slot
        // so `decoded_str()` reflects only the segment produced here (set below on escapes).
        self.has_decoded = false;
        if self.template_depth == 0 {
            return Err(lex_err(
                "continue_template called outside template context",
                self.position,
            ));
        }
        self.template_depth -= 1;

        // Sync lexer position to just after the }
        // brace_end is where template content starts
        self.set_position(brace_end);

        let content_start = brace_end;
        let brace_start = brace_end - 1; // for span tracking, } is 1 char before
        let (content_end, stop, has_escapes) = self.scan_template_body(content_start);

        let kind = match stop {
            TemplateStop::Eof => {
                return Err(lex_err("Unterminated template literal", brace_start));
            }
            TemplateStop::Backtick => TokenKind::TemplateTail,
            TemplateStop::Interpolation => {
                self.template_depth += 1;
                TokenKind::TemplateMiddle
            }
        };

        let content = &self.source[content_start..content_end];
        // Decode into the parked scratch. A bad escape (deferred for tagged
        // templates) leaves `has_decoded` false — same as a no-escape segment; the
        // parser distinguishes them by the presence of a backslash in `content`.
        if has_escapes
            && escapes::decode_string_escapes_into(content, &mut self.decode_scratch).is_ok()
        {
            self.has_decoded = true;
        }

        Ok(Token {
            kind,
            start: brace_start as u32,
            end: self.position as u32,
        })
    }

    /// Read a hashbang comment: #!...
    /// Only valid at the start of the file (position 0).
    /// Reads until end of line or end of file.
    /// Returns as a Comment token with is_block: false.
    fn read_hashbang_into(&mut self, start: usize, dst: &mut Token) -> Result<(), Box<ParseError>> {
        // Skip #!
        self.advance(); // #
        self.advance(); // !

        // Read until newline or EOF without copying. Unlike `//`, the hashbang's
        // content includes the `#!` prefix, so its content starts at `start`
        // (no delimiter stripping) — recovered on demand as a source slice.
        loop {
            match self.cur_byte() {
                // End of hashbang comment at the first LineTerminator (LF, CR,
                // LS, PS) or EOF. Don't consume the terminator - it's whitespace
                // for the next token. (Mirrors the `//` line-comment reader.)
                None | Some(b'\n' | b'\r') => break,
                Some(b) if b >= 0x80 && self.at_line_separator() => {
                    break;
                }
                Some(_) => {
                    self.advance();
                }
            }
        }

        *dst = Token {
            kind: TokenKind::Comment {
                is_block: false,
                content_start: start as u32,
            },
            start: start as u32,
            end: self.position as u32,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The byte-cursor fast path and the identifier-scan terminator arm both decide
    // ASCII identifier bytes from the `[bool; 256]` LUTs instead of decoding a char
    // and calling the Unicode predicates. That is only sound if the LUTs agree with
    // `is_id_start`/`is_id_continue` on every ASCII byte.
    #[test]
    fn ascii_id_luts_match_unicode_predicates() {
        for b in 0u8..0x80 {
            assert_eq!(
                is_ascii_id_start(b),
                is_id_start(b as char),
                "id_start mismatch at byte {b:#x}"
            );
            assert_eq!(
                is_ascii_id_continue(b),
                is_id_continue(b as char),
                "id_continue mismatch at byte {b:#x}"
            );
        }
    }
}
