// CSS Lexer - tokenization for CSS content in <style> tags
//
// ARCHITECTURE DECISION: Separate Lexer vs Inline Parsing
//
// We use a traditional separate lexer that produces tokens, then the parser consumes them.
// Svelte's CSS parser uses inline parsing (no separate tokenization step) with read_value().
//
// TODO: Benchmark both approaches on large stylesheets (10k+ lines) to determine if inline
// parsing offers significant performance benefits. Current recommendation: keep separate lexer
// for better debuggability and maintainability until proven performance issue exists.
//
// Pros of separate lexer:
//   - Easier to debug (can inspect token stream)
//   - Clearer separation of concerns
//   - Easier to add new token types
//   - Better error messages (can point to specific tokens)
//
// Pros of inline parsing (Svelte's approach):
//   - Potentially faster (fewer allocations, single pass)
//   - String slicing over token objects (lower memory)
//   - No need to store token positions separately
//
// See SVELTE_CSS_PARSING.md "Next Steps & Development Priorities" for details.

use tsv_lang::ParseError;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Identifiers and keywords
    Identifier, // div, color, red, etc.

    // Braces and delimiters
    LeftBrace,    // {
    RightBrace,   // }
    LeftBracket,  // [
    RightBracket, // ]
    LeftParen,    // (
    RightParen,   // )

    // Punctuation
    Colon,       // :
    Semicolon,   // ;
    Comma,       // ,
    Dot,         // .
    Hash,        // #
    GreaterThan, // >
    Plus,        // +
    Tilde,       // ~
    Asterisk,    // *
    Ampersand,   // &
    AtSign,      // @
    Slash,       // / (division operator)

    // Values - composite tokens per CSS Syntax Level 3
    String {
        content: String, // Raw content with escapes preserved: hel\"lo
        quote: char,     // ' or "
    },
    Number(f64),            // 123, 1.5, .5
    Percentage(f64),        // 50% - number + % sign as single token
    Dimension(f64, String), // 16px, 1.5em - number + unit as single token

    // Comments - preserved for formatters
    Comment(String), // /* ... */ - content without delimiters

    // Whitespace
    Whitespace, // spaces, tabs, newlines

    Eof,
}

#[derive(Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

pub struct Lexer<'a> {
    source: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source, pos: 0 }
    }

    fn current_char(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    fn peek_char(&self, offset: usize) -> Option<char> {
        self.source[self.pos..].chars().nth(offset)
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.current_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn skip_whitespace(&mut self) -> Token {
        let start = self.pos;
        while let Some(ch) = self.current_char() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
        Token {
            kind: TokenKind::Whitespace,
            start,
            end: self.pos,
        }
    }

    fn read_identifier(&mut self) -> Result<Token, ParseError> {
        let start = self.pos;

        // CSS identifiers can contain unicode escapes
        while let Some(ch) = self.current_char() {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                self.advance();
            } else if ch == '\\' {
                // Unicode escape in identifier
                if let Some(next_ch) = self.peek_char(1)
                    && next_ch.is_ascii_hexdigit()
                {
                    // Decode and continue - we're building the source representation
                    let _ = self.decode_unicode_escape()?;
                    continue;
                }
                // Not a unicode escape, end identifier
                break;
            } else {
                break;
            }
        }

        Ok(Token {
            kind: TokenKind::Identifier,
            start,
            end: self.pos,
        })
    }

    /// Read a CSS comment: /* ... */
    /// Returns the comment content WITHOUT the /* */ delimiters
    fn read_comment(&mut self) -> Result<Token, ParseError> {
        let start = self.pos;

        // Skip /*
        self.advance(); // /
        self.advance(); // *

        let mut content = String::new();

        loop {
            match self.current_char() {
                None => {
                    return Err(ParseError::InvalidSyntax {
                        message: "Unterminated comment".to_string(),
                        position: start,
                        context: None,
                    });
                }
                Some('*') => {
                    if self.peek_char(1) == Some('/') {
                        self.advance(); // *
                        self.advance(); // /
                        break;
                    } else {
                        content.push('*');
                        self.advance();
                    }
                }
                Some(ch) => {
                    content.push(ch);
                    self.advance();
                }
            }
        }

        // Preserve comment content EXACTLY as written (prettier doesn't normalize)
        Ok(Token {
            kind: TokenKind::Comment(content),
            start,
            end: self.pos,
        })
    }

    /// Decode a CSS unicode escape sequence: \XXXXXX (1-6 hex digits)
    /// Advances position past the escape sequence
    fn decode_unicode_escape(&mut self) -> Result<char, ParseError> {
        let start = self.pos;
        self.advance(); // skip \

        let mut hex_str = String::new();

        // Read 1-6 hex digits
        for _ in 0..6 {
            match self.current_char() {
                Some(ch) if ch.is_ascii_hexdigit() => {
                    hex_str.push(ch);
                    self.advance();
                }
                _ => break,
            }
        }

        if hex_str.is_empty() {
            return Err(ParseError::InvalidSyntax {
                message: "Invalid unicode escape sequence".to_string(),
                position: start,
                context: None,
            });
        }

        // Skip optional whitespace after unicode escape
        if let Some(ch) = self.current_char()
            && ch.is_whitespace()
        {
            self.advance();
        }

        let code_point =
            u32::from_str_radix(&hex_str, 16).map_err(|_| ParseError::InvalidSyntax {
                message: format!("Invalid unicode code point: {}", hex_str),
                position: start,
                context: None,
            })?;

        char::from_u32(code_point).ok_or_else(|| ParseError::InvalidSyntax {
            message: format!("Invalid unicode code point: U+{:X}", code_point),
            position: start,
            context: None,
        })
    }

    /// Read a CSS string: "..." or '...'
    /// Preserves escape sequences in raw form for formatting
    /// Returns the raw string content (with escapes) and quote character
    fn read_string(&mut self, quote: char) -> Result<Token, ParseError> {
        let start = self.pos;
        self.advance(); // skip opening quote

        let content_start = self.pos; // Track start of content (after opening quote)

        // Scan through string to validate and find end
        loop {
            match self.current_char() {
                None => {
                    return Err(ParseError::InvalidSyntax {
                        message: format!("Unterminated string starting with {}", quote),
                        position: start,
                        context: None,
                    });
                }
                Some(ch) if ch == quote => {
                    let content_end = self.pos; // End of content (before closing quote)
                    self.advance(); // skip closing quote

                    // Slice source to get raw content with escapes preserved
                    let content = self.source[content_start..content_end].to_string();

                    return Ok(Token {
                        kind: TokenKind::String { content, quote },
                        start,
                        end: self.pos,
                    });
                }
                Some('\\') => {
                    // Skip escape sequence without decoding
                    self.advance(); // skip \

                    // Check if it's a unicode escape
                    if let Some(next_ch) = self.current_char() {
                        if next_ch.is_ascii_hexdigit() {
                            // Skip 1-6 hex digits for unicode escape
                            for _ in 0..6 {
                                match self.current_char() {
                                    Some(ch) if ch.is_ascii_hexdigit() => {
                                        self.advance();
                                    }
                                    _ => break,
                                }
                            }
                            // Skip optional whitespace after unicode escape
                            if let Some(ch) = self.current_char()
                                && ch.is_whitespace()
                            {
                                self.advance();
                            }
                        } else {
                            // Regular escape - skip the escaped character
                            self.advance();
                        }
                    } else {
                        return Err(ParseError::InvalidSyntax {
                            message: "Unexpected end of string after backslash".to_string(),
                            position: self.pos,
                            context: None,
                        });
                    }
                }
                Some(_) => {
                    self.advance();
                }
            }
        }
    }

    /// Read a CSS number, percentage, or dimension
    /// Numbers: 42, 1.5, .5
    /// Percentages: 50%
    /// Dimensions: 16px, 1.5em
    fn read_number(&mut self) -> Result<Token, ParseError> {
        let start = self.pos;
        let mut num_str = String::new();

        // Read integer part
        while let Some(ch) = self.current_char() {
            if ch.is_ascii_digit() {
                num_str.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        // Read decimal part
        if self.current_char() == Some('.')
            && self.peek_char(1).is_some_and(|ch| ch.is_ascii_digit())
        {
            num_str.push('.');
            self.advance();

            while let Some(ch) = self.current_char() {
                if ch.is_ascii_digit() {
                    num_str.push(ch);
                    self.advance();
                } else {
                    break;
                }
            }
        }

        let number = num_str
            .parse::<f64>()
            .map_err(|_| ParseError::InvalidSyntax {
                message: format!("Invalid number: {}", num_str),
                position: start,
                context: None,
            })?;

        // Check for percentage
        if self.current_char() == Some('%') {
            self.advance();
            return Ok(Token {
                kind: TokenKind::Percentage(number),
                start,
                end: self.pos,
            });
        }

        // Check for dimension (unit)
        if let Some(ch) = self.current_char()
            && (ch.is_alphabetic() || ch == '-')
        {
            let unit_start = self.pos;
            let mut unit = String::new();

            while let Some(ch) = self.current_char() {
                if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                    unit.push(ch);
                    self.advance();
                } else {
                    break;
                }
            }

            if !unit.is_empty() {
                return Ok(Token {
                    kind: TokenKind::Dimension(number, unit),
                    start,
                    end: self.pos,
                });
            }

            // Reset position if we didn't find a valid unit
            self.pos = unit_start;
        }

        // Just a number
        Ok(Token {
            kind: TokenKind::Number(number),
            start,
            end: self.pos,
        })
    }

    pub fn next_token(&mut self) -> Result<Token, ParseError> {
        if self.pos >= self.source.len() {
            return Ok(Token {
                kind: TokenKind::Eof,
                start: self.pos,
                end: self.pos,
            });
        }

        let ch = match self.current_char() {
            Some(c) => c,
            None => {
                return Ok(Token {
                    kind: TokenKind::Eof,
                    start: self.pos,
                    end: self.pos,
                });
            }
        };

        // Helper macro for single-character tokens
        macro_rules! single_char_token {
            ($kind:expr) => {{
                let start = self.pos;
                self.advance();
                Ok(Token {
                    kind: $kind,
                    start,
                    end: self.pos,
                })
            }};
        }

        match ch {
            // Whitespace
            _ if ch.is_whitespace() => Ok(self.skip_whitespace()),

            // Comments
            '/' if self.peek_char(1) == Some('*') => self.read_comment(),

            // Strings
            '"' => self.read_string('"'),
            '\'' => self.read_string('\''),

            // Numbers (including percentage and dimension)
            _ if ch.is_ascii_digit() => self.read_number(),
            '.' if self.peek_char(1).is_some_and(|ch| ch.is_ascii_digit()) => self.read_number(),

            // Braces and delimiters
            '{' => single_char_token!(TokenKind::LeftBrace),
            '}' => single_char_token!(TokenKind::RightBrace),
            '[' => single_char_token!(TokenKind::LeftBracket),
            ']' => single_char_token!(TokenKind::RightBracket),
            '(' => single_char_token!(TokenKind::LeftParen),
            ')' => single_char_token!(TokenKind::RightParen),

            // Punctuation
            ':' => single_char_token!(TokenKind::Colon),
            ';' => single_char_token!(TokenKind::Semicolon),
            ',' => single_char_token!(TokenKind::Comma),
            '.' => single_char_token!(TokenKind::Dot),
            '#' => single_char_token!(TokenKind::Hash),
            '>' => single_char_token!(TokenKind::GreaterThan),
            '+' => single_char_token!(TokenKind::Plus),
            '~' => single_char_token!(TokenKind::Tilde),
            '*' => single_char_token!(TokenKind::Asterisk),
            '&' => single_char_token!(TokenKind::Ampersand),
            '@' => single_char_token!(TokenKind::AtSign),
            '/' => single_char_token!(TokenKind::Slash),

            // Identifiers (including those with unicode escapes)
            _ if ch.is_alphabetic() || ch == '-' || ch == '_' || ch == '\\' => {
                self.read_identifier()
            }

            // Unknown character
            _ => Err(ParseError::InvalidSyntax {
                message: format!("Unexpected character in CSS: '{}'", ch),
                position: self.pos,
                context: None,
            }),
        }
    }
}
