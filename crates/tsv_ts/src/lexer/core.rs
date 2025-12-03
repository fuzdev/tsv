// Core lexer implementation

use super::comments;
use super::escapes;
use super::token::{Token, TokenKind, keyword_kind};
use std::str::Chars;
use tsv_lang::ParseError;

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
        let current = chars.next();
        Self {
            source,
            chars,
            position: 0,
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

    fn advance(&mut self) {
        if let Some(ch) = self.current {
            self.position += ch.len_utf8();
            self.current = self.chars.next();
        }
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
    /// Handles: 123, 1.5, 1e3, 1.5e-2, 1_000
    fn scan_decimal_number(&mut self) {
        // Integer part (with optional separators)
        self.scan_digits(|c| c.is_ascii_digit());

        // Decimal point and fractional part
        if self.current == Some('.') {
            // Peek ahead: if next char is a digit or if this is trailing decimal (5.)
            let next_char = self.source[self.position + 1..].chars().next();
            if next_char.is_some_and(|c| c.is_ascii_digit()) {
                // Normal decimal: 3.14
                self.advance(); // consume '.'
                self.scan_digits(|c| c.is_ascii_digit());
            } else if next_char.is_none()
                || !next_char.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.')
            {
                // Trailing decimal: 5. (followed by ; or space or end)
                // But not: 5.toString() or 5..toString()
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
                        _ => {
                            // Regular number or float starting with 0
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
            Some(ch) if ch.is_alphabetic() || ch == '_' || ch == '$' => {
                while let Some(ch) = self.current {
                    if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                        self.advance();
                    } else {
                        break;
                    }
                }
                let raw = &self.source[start..self.position];
                let kind = if let Some(kw) = keyword_kind(raw) {
                    TokenKind::Keyword(kw)
                } else {
                    TokenKind::Identifier
                };
                Ok(self.make_token(kind, start))
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
                let peek1 = self.source[self.position + 1..].chars().next();
                let peek2 = self.source[self.position + 2..].chars().next();
                if peek1 == Some('.') && peek2 == Some('.') {
                    // Spread operator: ...
                    self.advance(); // consume first .
                    self.advance(); // consume second .
                    self.advance(); // consume third .
                    Ok(self.make_token(TokenKind::DotDotDot, start))
                } else if peek1.is_some_and(|c| c.is_ascii_digit()) {
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
                        // Update lexer state
                        self.position = pos;
                        self.chars = self.source[pos..].chars();
                        self.current = self.chars.next();
                        Ok(token)
                    }
                    Some('*') => {
                        // Block comment
                        let mut pos = self.position;
                        let token = comments::read_block_comment(self.source, &mut pos)?;
                        // Update lexer state
                        self.position = pos;
                        self.chars = self.source[pos..].chars();
                        self.current = self.chars.next();
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
            Some(ch) => Err(ParseError::InvalidSyntax {
                message: format!("Unexpected character: '{ch}'"),
                position: start,
                context: None,
            }),
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
                    let decoded = if has_escapes {
                        Some(escapes::decode_string_escapes(content)?)
                    } else {
                        None
                    };

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
                        let decoded = if has_escapes {
                            Some(escapes::decode_string_escapes(content)?)
                        } else {
                            None
                        };

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

    /// Read a regex literal starting from a `/` token.
    ///
    /// Called by the parser when it determines that `/` should be a regex, not division.
    /// The parser passes the start position of the `/` token it received.
    ///
    /// The lexer syncs to that position and reads `/pattern/flags`.
    ///
    /// Pattern and flags are stored in token.decoded as "pattern\0flags" (null-separated).
    pub fn read_regex_literal(&mut self, slash_start: usize) -> Result<Token, ParseError> {
        // Sync to just after the opening /
        self.position = slash_start + 1;
        self.chars = self.source[self.position..].chars();
        self.current = self.chars.next();

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

        // Read flags (identifier characters)
        // TODO: Validate flags are only valid regex flags (d, g, i, m, s, u, v, y)
        // TODO: Reject duplicate flags (e.g., /test/gg)
        // TODO: Support Unicode escape sequences in flags (e.g., /test/\u0067 for 'g')
        let flags_start = self.position;
        while let Some(ch) = self.current {
            if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                self.advance();
            } else {
                break;
            }
        }
        let flags = &self.source[flags_start..self.position];

        // Store pattern and flags as "pattern\0flags"
        let decoded = format!("{pattern}\0{flags}");

        Ok(Token {
            kind: TokenKind::RegexLiteral,
            start: slash_start,
            end: self.position,
            decoded: Some(decoded),
        })
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
        self.position = brace_end;
        self.chars = self.source[brace_end..].chars();
        self.current = self.chars.next();

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
                    let decoded = if has_escapes {
                        Some(escapes::decode_string_escapes(content)?)
                    } else {
                        None
                    };

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
                        let decoded = if has_escapes {
                            Some(escapes::decode_string_escapes(content)?)
                        } else {
                            None
                        };

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
}
