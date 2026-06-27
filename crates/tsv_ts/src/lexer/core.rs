// Core lexer implementation

use super::comments;
use super::escapes;
use super::ident::{is_id_continue, is_id_start};
use super::token::{Token, TokenKind, keyword_kind};
use std::str::Chars;
use tsv_lang::ParseError;

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

pub struct Lexer<'a> {
    source: &'a str,
    chars: Chars<'a>,
    position: usize,
    current: Option<char>,
    /// Stack for tracking template literal nesting depth.
    /// When we enter a template interpolation `${`, we push to this stack.
    /// When we see `}`, if the stack is non-empty, we continue template reading.
    template_depth: u32,
    /// True if a line terminator was encountered while skipping whitespace to reach
    /// the current token. Used for Automatic Semicolon Insertion (ASI).
    /// Reset at start of skip_whitespace(), set when line terminators are found.
    had_line_terminator: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        let mut chars = source.chars();
        let mut current = chars.next();
        let mut position = 0;

        // Skip UTF-8 BOM (U+FEFF) at start of file if present.
        // BOM is a legacy artifact; we strip it (like deno fmt, VS Code).
        // Position starts after BOM so token spans reflect actual file bytes.
        if current == Some('\u{feff}') {
            position = '\u{feff}'.len_utf8();
            current = chars.next();
        }

        Self {
            source,
            chars,
            position,
            current,
            template_depth: 0,
            had_line_terminator: false,
        }
    }

    /// Returns true if a line terminator was encountered while skipping to the current token.
    /// Used for ASI (Automatic Semicolon Insertion).
    pub fn had_line_terminator(&self) -> bool {
        self.had_line_terminator
    }

    /// Seek to a specific position and re-lex from there.
    /// Used when splitting compound tokens like `>=` into `>` + `=`.
    pub fn seek_and_next_token(&mut self, position: usize) -> Result<Token, ParseError> {
        self.set_position(position);
        self.next_token()
    }

    /// Reset the char cursor to an absolute byte position (must be a char boundary).
    #[inline]
    fn set_position(&mut self, position: usize) {
        self.position = position;
        self.chars = self.source[position..].chars();
        self.current = self.chars.next();
    }

    #[inline]
    fn advance(&mut self) {
        if let Some(ch) = self.current {
            self.position += ch.len_utf8();
            self.current = self.chars.next();
        }
    }

    /// Peek at the next n characters without consuming them.
    /// Returns a string slice containing up to n characters (may be fewer if EOF).
    fn peek_chars(&self, n: usize) -> &str {
        let remaining = &self.source[self.position..];
        // Count n characters (not bytes) to find the correct byte offset
        let byte_count: usize = remaining.chars().take(n).map(char::len_utf8).sum();
        &remaining[..byte_count]
    }

    /// Create a token with the current position as end
    #[inline]
    fn make_token(&self, kind: TokenKind, start: usize) -> Token {
        Token {
            kind,
            start,
            end: self.position,
            decoded: None,
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
    fn scan_identifier_with_escapes(&mut self, first_char: char) -> Result<Token, ParseError> {
        let start = self.position;
        // None until an actual escape is decoded; `decoded.is_some()` ⇔ has-escapes.
        let mut decoded: Option<String> = None;

        // Handle first character (already validated as valid identifier start)
        if first_char == '\\' {
            // First char is a unicode escape
            if let Some((ch, len)) = try_decode_unicode_escape(self.source, self.position) {
                if !is_id_start(ch) {
                    return Err(ParseError::InvalidSyntax {
                        message: format!(
                            "Invalid identifier start character from unicode escape: '{ch}'"
                        ),
                        position: start,
                        context: None,
                    });
                }
                decoded = Some(String::from(ch));
                // Advance by the escape sequence length
                for _ in 0..len {
                    self.advance();
                }
            } else {
                return Err(ParseError::InvalidSyntax {
                    message: "Invalid unicode escape in identifier".to_string(),
                    position: start,
                    context: None,
                });
            }
        } else {
            self.advance();

            // ASCII fast path: tight byte loop over `[a-zA-Z0-9_$]` (the ASCII
            // subset of IdentifierPart), then resync the char cursor once.
            // Bails to the general loop on the first non-ASCII byte or `\`.
            if first_char.is_ascii() {
                let bytes = self.source.as_bytes();
                let mut pos = self.position;
                while pos < bytes.len()
                    && (bytes[pos].is_ascii_alphanumeric()
                        || bytes[pos] == b'_'
                        || bytes[pos] == b'$')
                {
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
            match self.current {
                Some(ch) if is_id_continue(ch) => {
                    if let Some(d) = &mut decoded {
                        d.push(ch);
                    }
                    self.advance();
                }
                Some('\\') => {
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
                _ => break,
            }
        }

        // Check if it's a keyword (only if no escapes - escaped keywords are identifiers;
        // without escapes the source slice IS the name, so no decoded buffer is needed)
        let kind = if decoded.is_none() {
            if let Some(kw) = keyword_kind(&self.source[start..self.position]) {
                TokenKind::Keyword(kw)
            } else {
                TokenKind::Identifier
            }
        } else {
            // Escaped identifiers are never keywords: `\u0063lass` is identifier "class", not keyword
            TokenKind::Identifier
        };

        Ok(Token {
            kind,
            start,
            end: self.position,
            decoded,
        })
    }

    /// Scan digits matching a predicate, allowing numeric separators (_)
    fn scan_digits(&mut self, is_valid_digit: impl Fn(char) -> bool) {
        while let Some(ch) = self.current {
            if is_valid_digit(ch) || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Scan a decimal number (integer, float, or scientific notation)
    /// Handles: 123, 1.5, 1e3, 1.5e-2, 1_000, 1.e1
    fn scan_decimal_number(&mut self) {
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

        // Integer part (with optional separators)
        self.scan_digits(|c| c.is_ascii_digit());

        // Decimal point and fractional part
        if self.current == Some('.') {
            // Peek ahead: if next char is a digit or if this is trailing decimal (5.)
            let rest = &self.source[self.position + 1..];
            let next_char = rest.chars().next();
            if next_char.is_some_and(|c| c.is_ascii_digit()) {
                // Normal decimal: 3.14
                self.advance(); // consume '.'
                self.scan_digits(|c| c.is_ascii_digit());
            } else if is_exponent_start(rest) {
                // Trailing-dot exponent: `1.e1` is a single numeric literal (= 1e1).
                // Consume the '.'; the exponent block below consumes `e1`.
                // Without this, `1.e1` would lex as `1.` followed by member access `.e1`.
                self.advance(); // consume '.'
            } else if next_char.is_none() || !next_char.is_some_and(is_id_start) {
                // Trailing decimal: 5. or 0. (followed by operator, punctuation, or end)
                // Don't consume if followed by identifier: 5.toString() is invalid anyway
                // Do consume for 0..toString() so the number is "0." and second dot is member access
                self.advance(); // consume '.'
            }
        }

        // Exponent part: e+10, E-3, e10
        if matches!(self.current, Some('e' | 'E')) {
            self.advance(); // consume 'e' or 'E'
            // Optional sign
            if matches!(self.current, Some('+' | '-')) {
                self.advance();
            }
            self.scan_digits(|c| c.is_ascii_digit());
        }
    }

    fn skip_whitespace(&mut self) {
        self.had_line_terminator = false;
        while let Some(ch) = self.current {
            match ch {
                // ECMAScript line terminators (ES spec 12.3)
                '\n' | '\u{2028}' | '\u{2029}' => {
                    // LF (Line Feed), LS (Line Separator), PS (Paragraph Separator)
                    self.had_line_terminator = true;
                    self.advance();
                }
                '\r' => {
                    // CR (Carriage Return) - handle CRLF as single line terminator
                    self.had_line_terminator = true;
                    self.advance();
                    // Consume following LF if present (CRLF)
                    if self.current == Some('\n') {
                        self.advance();
                    }
                }
                c if c.is_whitespace() => {
                    // Other whitespace (space, tab, etc.)
                    self.advance();
                }
                _ => break,
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
    pub fn next_token(&mut self) -> Result<Token, ParseError> {
        self.skip_whitespace();

        let start = self.position;

        match self.current {
            None => Ok(Token {
                kind: TokenKind::Eof,
                start,
                end: start,
                decoded: None,
            }),
            Some(';') => {
                self.advance();
                Ok(self.make_token(TokenKind::Semicolon, start))
            }
            Some(':') => {
                self.advance();
                Ok(self.make_token(TokenKind::Colon, start))
            }
            Some('=') => {
                self.advance();
                match self.current {
                    Some('>') => {
                        // =>
                        self.advance();
                        Ok(self.make_token(TokenKind::Arrow, start))
                    }
                    Some('=') => {
                        self.advance();
                        if self.current == Some('=') {
                            // ===
                            self.advance();
                            Ok(self.make_token(TokenKind::EqualsEqualsEquals, start))
                        } else {
                            // ==
                            Ok(self.make_token(TokenKind::EqualsEquals, start))
                        }
                    }
                    _ => {
                        // =
                        Ok(self.make_token(TokenKind::Equals, start))
                    }
                }
            }
            Some(ch) if ch.is_ascii_digit() => {
                // Handle different number formats
                if ch == '0' {
                    let next = self.source[self.position + 1..].chars().next();
                    match next {
                        Some('x' | 'X') => {
                            // Hex: 0xff, 0xFF
                            self.advance(); // consume '0'
                            self.advance(); // consume 'x'
                            self.scan_digits(|c| c.is_ascii_hexdigit());
                        }
                        Some('b' | 'B') => {
                            // Binary: 0b1010
                            self.advance(); // consume '0'
                            self.advance(); // consume 'b'
                            self.scan_digits(|c| c == '0' || c == '1');
                        }
                        Some('o' | 'O') => {
                            // Octal: 0o77
                            self.advance(); // consume '0'
                            self.advance(); // consume 'o'
                            self.scan_digits(|c| ('0'..='7').contains(&c));
                        }
                        Some('0'..='7') => {
                            // Legacy octal (0777) - reject in strict mode (ES modules)
                            // ES modules are always strict, so this is always an error
                            return Err(ParseError::InvalidSyntax {
                                message: "Octal literals are not allowed in strict mode. Use the syntax '0o' instead.".to_string(),
                                position: start,
                                context: None,
                            });
                        }
                        _ => {
                            // Regular number or float starting with 0 (e.g., 0.5, 08, 09)
                            // Note: 08 and 09 are valid decimal literals (non-octal digits)
                            self.scan_decimal_number();
                        }
                    }
                } else {
                    // Regular decimal number
                    self.scan_decimal_number();
                }

                // Check for BigInt suffix: 123n, 0xffn
                if self.current == Some('n') {
                    self.advance();
                }

                Ok(self.make_token(TokenKind::Number, start))
            }
            // ECMAScript identifiers: start with ID_Start, _, or $; continue with ID_Continue or $
            // Note: _ is in ID_Continue but not ID_Start, so we check it explicitly for start
            // Identifiers may contain unicode escapes: \u0066oo → foo, b\u0061r → bar
            Some(ch) if is_id_start(ch) => self.scan_identifier_with_escapes(ch),
            // Unicode escape at start of identifier: \u0066oo → foo
            Some('\\') => {
                // Check if this is a valid unicode escape that decodes to an identifier start
                if let Some((ch, _)) = try_decode_unicode_escape(self.source, self.position)
                    && is_id_start(ch)
                {
                    return self.scan_identifier_with_escapes('\\');
                }
                // Not a valid identifier start - fall through to error at end of match
                Err(ParseError::InvalidSyntax {
                    message: "Unexpected character: '\\'".to_string(),
                    position: start,
                    context: None,
                })
            }
            Some(quote @ '\'' | quote @ '"') => {
                // String literal - single or double quoted
                self.advance(); // consume opening quote
                let content_start = self.position;

                // Check if string contains escape sequences
                let mut has_escapes = false;
                while let Some(ch) = self.current {
                    if ch == quote {
                        // Found closing quote
                        let content_end = self.position;
                        self.advance(); // consume closing quote

                        let content = &self.source[content_start..content_end];

                        // Decode escape sequences if present
                        let decoded = if has_escapes {
                            Some(escapes::decode_string_escapes(content)?)
                        } else {
                            // No escapes - use content as-is
                            None
                        };

                        return Ok(Token {
                            kind: TokenKind::String,
                            start,
                            end: self.position,
                            decoded,
                        });
                    } else if ch == '\\' {
                        has_escapes = true;
                        self.advance(); // consume backslash
                        // Skip next character (part of escape sequence)
                        // Note: decode_string_escapes will validate the escape later
                        if self.current.is_some() {
                            self.advance();
                        }
                    } else {
                        self.advance();
                    }
                }
                // Unterminated string
                Err(ParseError::InvalidSyntax {
                    message: "Unterminated string literal".to_string(),
                    position: start,
                    context: None,
                })
            }
            Some(',') => {
                self.advance();
                Ok(self.make_token(TokenKind::Comma, start))
            }
            Some('{') => {
                self.advance();
                Ok(self.make_token(TokenKind::BraceOpen, start))
            }
            Some('}') => {
                self.advance();
                Ok(self.make_token(TokenKind::BraceClose, start))
            }
            Some('[') => {
                self.advance();
                Ok(self.make_token(TokenKind::BracketOpen, start))
            }
            Some(']') => {
                self.advance();
                Ok(self.make_token(TokenKind::BracketClose, start))
            }
            Some('(') => {
                self.advance();
                Ok(self.make_token(TokenKind::ParenOpen, start))
            }
            Some(')') => {
                self.advance();
                Ok(self.make_token(TokenKind::ParenClose, start))
            }
            Some('.') => {
                // Use peek_chars to safely check for spread operator (...) without UTF-8 boundary issues
                let peek = self.peek_chars(3); // Peek at current '.' plus next 2 chars
                if peek == "..." {
                    // Spread operator: ...
                    self.advance(); // consume first .
                    self.advance(); // consume second .
                    self.advance(); // consume third .
                    Ok(self.make_token(TokenKind::DotDotDot, start))
                } else {
                    // Check if next char is a digit (for decimal numbers like .5)
                    let next_char = peek.chars().nth(1); // Skip current '.' and get next char
                    if next_char.is_some_and(|c| c.is_ascii_digit()) {
                        // Number starting with decimal: .5
                        self.advance(); // consume '.'
                        self.scan_digits(|c| c.is_ascii_digit());
                        // Check for exponent
                        if matches!(self.current, Some('e' | 'E')) {
                            self.advance();
                            if matches!(self.current, Some('+' | '-')) {
                                self.advance();
                            }
                            self.scan_digits(|c| c.is_ascii_digit());
                        }
                        Ok(self.make_token(TokenKind::Number, start))
                    } else {
                        // Single dot: member access operator
                        self.advance();
                        Ok(self.make_token(TokenKind::Dot, start))
                    }
                }
            }
            Some('-') => {
                self.advance();
                if self.current == Some('-') {
                    self.advance();
                    Ok(self.make_token(TokenKind::MinusMinus, start))
                } else if self.current == Some('=') {
                    self.advance();
                    Ok(self.make_token(TokenKind::MinusEquals, start))
                } else {
                    Ok(self.make_token(TokenKind::Minus, start))
                }
            }
            Some('+') => {
                self.advance();
                if self.current == Some('+') {
                    self.advance();
                    Ok(self.make_token(TokenKind::PlusPlus, start))
                } else if self.current == Some('=') {
                    self.advance();
                    Ok(self.make_token(TokenKind::PlusEquals, start))
                } else {
                    Ok(self.make_token(TokenKind::Plus, start))
                }
            }
            Some('/') => {
                // Could be: // line comment, /* block comment */, or / division operator
                // Peek ahead to determine which
                let peek = self.source[self.position + 1..].chars().next();
                match peek {
                    Some('/') => {
                        // Line comment
                        let mut pos = self.position;
                        let token = comments::read_line_comment(self.source, &mut pos)?;
                        self.set_position(pos);
                        Ok(token)
                    }
                    Some('*') => {
                        // Block comment
                        let mut pos = self.position;
                        let token = comments::read_block_comment(self.source, &mut pos)?;
                        self.set_position(pos);
                        Ok(token)
                    }
                    Some('=') => {
                        // Division assignment operator /=
                        self.advance();
                        self.advance();
                        Ok(self.make_token(TokenKind::SlashEquals, start))
                    }
                    _ => {
                        // Division operator /
                        self.advance();
                        Ok(self.make_token(TokenKind::Slash, start))
                    }
                }
            }
            Some('*') => {
                self.advance();
                if self.current == Some('*') {
                    self.advance();
                    if self.current == Some('=') {
                        self.advance();
                        Ok(self.make_token(TokenKind::StarStarEquals, start))
                    } else {
                        Ok(self.make_token(TokenKind::StarStar, start))
                    }
                } else if self.current == Some('=') {
                    self.advance();
                    Ok(self.make_token(TokenKind::StarEquals, start))
                } else {
                    Ok(self.make_token(TokenKind::Star, start))
                }
            }
            Some('%') => {
                self.advance();
                if self.current == Some('=') {
                    self.advance();
                    Ok(self.make_token(TokenKind::PercentEquals, start))
                } else {
                    Ok(self.make_token(TokenKind::Percent, start))
                }
            }
            Some('^') => {
                self.advance();
                if self.current == Some('=') {
                    self.advance();
                    Ok(self.make_token(TokenKind::CaretEquals, start))
                } else {
                    Ok(self.make_token(TokenKind::Caret, start))
                }
            }
            Some('~') => {
                self.advance();
                Ok(self.make_token(TokenKind::Tilde, start))
            }
            Some('<') => {
                self.advance();
                if self.current == Some('=') {
                    self.advance();
                    Ok(self.make_token(TokenKind::LessThanEquals, start))
                } else if self.current == Some('<') {
                    self.advance();
                    if self.current == Some('=') {
                        self.advance();
                        Ok(self.make_token(TokenKind::LeftShiftEquals, start))
                    } else {
                        Ok(self.make_token(TokenKind::LeftShift, start))
                    }
                } else {
                    Ok(self.make_token(TokenKind::LessThan, start))
                }
            }
            Some('>') => {
                self.advance();
                if self.current == Some('=') {
                    self.advance();
                    Ok(self.make_token(TokenKind::GreaterThanEquals, start))
                } else if self.current == Some('>') {
                    self.advance();
                    if self.current == Some('>') {
                        // >>> or >>>=
                        self.advance();
                        if self.current == Some('=') {
                            self.advance();
                            Ok(self.make_token(TokenKind::UnsignedRightShiftEquals, start))
                        } else {
                            Ok(self.make_token(TokenKind::UnsignedRightShift, start))
                        }
                    } else if self.current == Some('=') {
                        // >>=
                        self.advance();
                        Ok(self.make_token(TokenKind::RightShiftEquals, start))
                    } else {
                        // >>
                        Ok(self.make_token(TokenKind::RightShift, start))
                    }
                } else {
                    Ok(self.make_token(TokenKind::GreaterThan, start))
                }
            }
            Some('!') => {
                self.advance();
                if self.current == Some('=') {
                    self.advance();
                    if self.current == Some('=') {
                        self.advance();
                        Ok(self.make_token(TokenKind::BangEqualsEquals, start))
                    } else {
                        Ok(self.make_token(TokenKind::BangEquals, start))
                    }
                } else {
                    Ok(self.make_token(TokenKind::Bang, start))
                }
            }
            Some('&') => {
                self.advance();
                if self.current == Some('&') {
                    self.advance();
                    if self.current == Some('=') {
                        self.advance();
                        Ok(self.make_token(TokenKind::AmpersandAmpersandEquals, start))
                    } else {
                        Ok(self.make_token(TokenKind::AmpersandAmpersand, start))
                    }
                } else if self.current == Some('=') {
                    self.advance();
                    Ok(self.make_token(TokenKind::AmpersandEquals, start))
                } else {
                    Ok(self.make_token(TokenKind::Ampersand, start))
                }
            }
            Some('|') => {
                self.advance();
                if self.current == Some('|') {
                    self.advance();
                    if self.current == Some('=') {
                        self.advance();
                        Ok(self.make_token(TokenKind::PipePipeEquals, start))
                    } else {
                        Ok(self.make_token(TokenKind::PipePipe, start))
                    }
                } else if self.current == Some('=') {
                    self.advance();
                    Ok(self.make_token(TokenKind::PipeEquals, start))
                } else {
                    Ok(self.make_token(TokenKind::Pipe, start))
                }
            }
            Some('?') => {
                self.advance();
                if self.current == Some('?') {
                    self.advance();
                    if self.current == Some('=') {
                        self.advance();
                        Ok(self.make_token(TokenKind::QuestionQuestionEquals, start))
                    } else {
                        Ok(self.make_token(TokenKind::QuestionQuestion, start))
                    }
                } else if self.current == Some('.') {
                    // Check for optional chaining `?.`
                    // Must not be followed by a digit (to avoid ambiguity with `?.0` which should be `?` `.0`)
                    let next = self.chars.clone().next();
                    if next.is_none_or(|ch| !ch.is_ascii_digit()) {
                        self.advance();
                        Ok(self.make_token(TokenKind::QuestionDot, start))
                    } else {
                        // `?.0` should be `?` followed by `.0` (number)
                        Ok(self.make_token(TokenKind::Question, start))
                    }
                } else {
                    Ok(self.make_token(TokenKind::Question, start))
                }
            }
            Some('`') => {
                // Template literal starting with backtick
                self.read_template_content(start)
            }
            Some('@') => {
                // @ for decorators
                self.advance();
                Ok(self.make_token(TokenKind::At, start))
            }
            Some('#') => {
                // Check for hashbang at start of file: #!/usr/bin/env node
                if start == 0 {
                    let next = self.source.get(1..2);
                    if next == Some("!") {
                        // Hashbang comment - read until end of line
                        return self.read_hashbang_comment(start);
                    }
                }
                // # for private identifiers
                self.advance();
                Ok(self.make_token(TokenKind::Hash, start))
            }
            Some(ch) => Err(ParseError::InvalidSyntax {
                message: format!("Unexpected character: '{ch}'"),
                position: start,
                context: None,
            }),
        }
    }

    /// Decode a template segment's escape sequences for the token's cooked value.
    ///
    /// Unlike a string literal, an **invalid** escape is not a lex error here: per
    /// the ES2018 template-literals revision it is allowed in a *tagged* template
    /// (cooked `null`) and is a syntax error only in an untagged template / template
    /// type. The lexer can't know which (the tag precedes the backtick), so it
    /// defers: `.ok()` yields `None` on a bad escape, exactly like a no-escape
    /// segment, and the parser (`template_cooked`) distinguishes the two by the
    /// presence of a backslash and decides based on tagged-ness.
    fn decode_template_segment(content: &str, has_escapes: bool) -> Option<String> {
        if has_escapes {
            escapes::decode_string_escapes(content).ok()
        } else {
            None
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
    fn read_template_content(&mut self, start: usize) -> Result<Token, ParseError> {
        self.advance(); // consume opening ` or }

        let content_start = self.position;
        let mut has_escapes = false;

        loop {
            match self.current {
                Some('`') => {
                    // End of template
                    let content_end = self.position;
                    self.advance(); // consume closing `

                    // Determine token type based on whether we started with ` or }
                    let is_head = self.source[start..].starts_with('`');
                    let kind = if is_head {
                        TokenKind::NoSubstitutionTemplate
                    } else {
                        TokenKind::TemplateTail
                    };

                    let content = &self.source[content_start..content_end];
                    let decoded = Self::decode_template_segment(content, has_escapes);

                    return Ok(Token {
                        kind,
                        start,
                        end: self.position,
                        decoded,
                    });
                }
                Some('$') => {
                    // Check for interpolation: ${
                    let next = self.source[self.position + 1..].chars().next();
                    if next == Some('{') {
                        // Start of interpolation
                        let content_end = self.position;
                        self.advance(); // consume $
                        self.advance(); // consume {
                        self.template_depth += 1;

                        // Determine token type
                        let is_head = self.source[start..].starts_with('`');
                        let kind = if is_head {
                            TokenKind::TemplateHead
                        } else {
                            TokenKind::TemplateMiddle
                        };

                        let content = &self.source[content_start..content_end];
                        let decoded = Self::decode_template_segment(content, has_escapes);

                        return Ok(Token {
                            kind,
                            start,
                            end: self.position,
                            decoded,
                        });
                    }
                    // Regular $ character
                    self.advance();
                }
                Some('\\') => {
                    // Escape sequence
                    has_escapes = true;
                    self.advance(); // consume backslash
                    if self.current.is_some() {
                        self.advance(); // consume escaped character
                    }
                }
                Some(_) => {
                    self.advance();
                }
                None => {
                    return Err(ParseError::InvalidSyntax {
                        message: "Unterminated template literal".to_string(),
                        position: start,
                        context: None,
                    });
                }
            }
        }
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
    pub fn read_regex_literal(&mut self, slash_start: usize) -> Result<(Token, usize), ParseError> {
        // Sync to just after the opening /
        self.set_position(slash_start + 1);

        let pattern_start = self.position;
        let mut in_class = false; // Inside character class [...]
        let mut escaped = false; // Previous char was \

        // Read pattern until unescaped / outside character class
        // TODO: Validate pattern syntax (e.g., reject invalid escape sequences like \c without letter)
        loop {
            match self.current {
                None => {
                    return Err(ParseError::InvalidSyntax {
                        message: "Unterminated regular expression literal".to_string(),
                        position: slash_start,
                        context: None,
                    });
                }
                Some('\n' | '\r' | '\u{2028}' | '\u{2029}') => {
                    // Line terminators not allowed in regex
                    return Err(ParseError::InvalidSyntax {
                        message: "Unterminated regular expression literal".to_string(),
                        position: slash_start,
                        context: None,
                    });
                }
                Some(_) if escaped => {
                    // Escaped character - consume and continue
                    escaped = false;
                    self.advance();
                }
                Some('\\') => {
                    escaped = true;
                    self.advance();
                }
                Some('[') if !in_class => {
                    in_class = true;
                    self.advance();
                }
                Some(']') if in_class => {
                    in_class = false;
                    self.advance();
                }
                Some('/') if !in_class => {
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
            return Err(ParseError::InvalidSyntax {
                message:
                    "Regular expression literal cannot be empty (use /(?:)/ for empty pattern)"
                        .to_string(),
                position: slash_start,
                context: None,
            });
        }

        self.advance(); // Consume closing /

        // Read flags (IdentifierPartChar = ID_Continue, plus $ for ECMAScript)
        // TODO: Validate flags are only valid regex flags (d, g, i, m, s, u, v, y)
        // TODO: Reject duplicate flags (e.g., /test/gg)
        // TODO: Support Unicode escape sequences in flags (e.g., /test/\u0067 for 'g')
        // The flags text is recovered from the span by the parser, not sliced here;
        // this loop only advances `self.position` to the token end.
        while let Some(ch) = self.current {
            if is_id_continue(ch) {
                self.advance();
            } else {
                break;
            }
        }

        Ok((
            Token {
                kind: TokenKind::RegexLiteral,
                start: slash_start,
                end: self.position,
                decoded: None,
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
    pub fn continue_template_from_brace(&mut self, brace_end: usize) -> Result<Token, ParseError> {
        if self.template_depth == 0 {
            return Err(ParseError::InvalidSyntax {
                message: "continue_template called outside template context".to_string(),
                position: self.position,
                context: None,
            });
        }
        self.template_depth -= 1;

        // Sync lexer position to just after the }
        // brace_end is where template content starts
        self.set_position(brace_end);

        let content_start = brace_end;
        let brace_start = brace_end - 1; // for span tracking, } is 1 char before
        let mut has_escapes = false;

        loop {
            match self.current {
                Some('`') => {
                    // End of template
                    let content_end = self.position;
                    self.advance(); // consume closing `

                    let content = &self.source[content_start..content_end];
                    let decoded = Self::decode_template_segment(content, has_escapes);

                    return Ok(Token {
                        kind: TokenKind::TemplateTail,
                        start: brace_start,
                        end: self.position,
                        decoded,
                    });
                }
                Some('$') => {
                    // Check for interpolation: ${
                    let next = self.source[self.position + 1..].chars().next();
                    if next == Some('{') {
                        // Start of interpolation
                        let content_end = self.position;
                        self.advance(); // consume $
                        self.advance(); // consume {
                        self.template_depth += 1;

                        let content = &self.source[content_start..content_end];
                        let decoded = Self::decode_template_segment(content, has_escapes);

                        return Ok(Token {
                            kind: TokenKind::TemplateMiddle,
                            start: brace_start,
                            end: self.position,
                            decoded,
                        });
                    }
                    // Regular $ character
                    self.advance();
                }
                Some('\\') => {
                    // Escape sequence
                    has_escapes = true;
                    self.advance(); // consume backslash
                    if self.current.is_some() {
                        self.advance(); // consume escaped character
                    }
                }
                Some(_) => {
                    self.advance();
                }
                None => {
                    return Err(ParseError::InvalidSyntax {
                        message: "Unterminated template literal".to_string(),
                        position: brace_start,
                        context: None,
                    });
                }
            }
        }
    }

    /// Read a hashbang comment: #!...
    /// Only valid at the start of the file (position 0).
    /// Reads until end of line or end of file.
    /// Returns as a Comment token with is_block: false.
    fn read_hashbang_comment(&mut self, start: usize) -> Result<Token, ParseError> {
        // Skip #!
        self.advance(); // #
        self.advance(); // !

        // Read until newline or EOF without copying. Unlike `//`, the hashbang's
        // content includes the `#!` prefix, so its content starts at `start`
        // (no delimiter stripping) — recovered on demand as a source slice.
        loop {
            match self.current {
                None | Some('\n') | Some('\r') => {
                    // End of hashbang comment
                    // Don't consume the newline - it's whitespace for the next token
                    break;
                }
                Some(_) => {
                    self.advance();
                }
            }
        }

        Ok(Token {
            kind: TokenKind::Comment {
                is_block: false,
                content_start: start,
            },
            start,
            end: self.position,
            decoded: None,
        })
    }
}
