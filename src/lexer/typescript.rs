use crate::error::ParseError;
use std::fmt;
use std::str::Chars;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeywordKind {
    Const,
    Let,
    Var,
    Number,
    // TODO: String, Boolean, etc.
}

impl fmt::Display for KeywordKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeywordKind::Const => write!(f, "const"),
            KeywordKind::Let => write!(f, "let"),
            KeywordKind::Var => write!(f, "var"),
            KeywordKind::Number => write!(f, "number"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind {
    Number,
    String,
    Identifier,
    Keyword(KeywordKind),
    Equals,
    Colon,
    Semicolon,
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Number => write!(f, "number"),
            TokenKind::String => write!(f, "string"),
            TokenKind::Identifier => write!(f, "identifier"),
            TokenKind::Keyword(kw) => write!(f, "'{}'", kw),
            TokenKind::Equals => write!(f, "'='"),
            TokenKind::Colon => write!(f, "':'"),
            TokenKind::Semicolon => write!(f, "';'"),
            TokenKind::Eof => write!(f, "end of file"),
        }
    }
}

// Zero-allocation token design: value borrows from source string
// No heap allocations during lexing - all tokens just hold slices
#[derive(Debug, Clone)]
pub struct Token<'a> {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
    #[expect(dead_code, reason = "Used for debugging and testing")]
    pub value: &'a str,
}

pub struct Lexer<'a> {
    source: &'a str,
    chars: Chars<'a>,
    position: usize,
    current: Option<char>,
}

// TODO: Expand keyword list for:
// - Type keywords: interface, type, enum, namespace, etc.
// - Control flow: if, else, while, for, switch, case, break, continue, return
// - Other: function, class, import, export, async, await, etc.
fn keyword_kind(s: &str) -> Option<KeywordKind> {
    match s {
        "const" => Some(KeywordKind::Const),
        "let" => Some(KeywordKind::Let),
        "var" => Some(KeywordKind::Var),
        "number" => Some(KeywordKind::Number),
        _ => None,
    }
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
        }
    }

    fn advance(&mut self) {
        if let Some(ch) = self.current {
            self.position += ch.len_utf8();
            self.current = self.chars.next();
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
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
    pub fn next_token(&mut self) -> Result<Token<'_>, ParseError> {
        self.skip_whitespace();

        let start = self.position;

        match self.current {
            None => Ok(Token {
                kind: TokenKind::Eof,
                start,
                end: start,
                value: "",
            }),
            Some(';') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Semicolon,
                    start,
                    end: self.position,
                    value: &self.source[start..self.position],
                })
            }
            Some(':') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Colon,
                    start,
                    end: self.position,
                    value: &self.source[start..self.position],
                })
            }
            Some('=') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Equals,
                    start,
                    end: self.position,
                    value: &self.source[start..self.position],
                })
            }
            Some(ch) if ch.is_ascii_digit() => {
                // TODO: Support floats (1.5, 1e10), hex (0x10), binary (0b10), octal (0o10)
                while let Some(ch) = self.current {
                    if ch.is_ascii_digit() {
                        self.advance();
                    } else {
                        break;
                    }
                }
                Ok(Token {
                    kind: TokenKind::Number,
                    start,
                    end: self.position,
                    value: &self.source[start..self.position],
                })
            }
            Some(ch) if ch.is_alphabetic() || ch == '_' || ch == '$' => {
                while let Some(ch) = self.current {
                    if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                        self.advance();
                    } else {
                        break;
                    }
                }
                let value = &self.source[start..self.position];
                let kind = if let Some(kw) = keyword_kind(value) {
                    TokenKind::Keyword(kw)
                } else {
                    TokenKind::Identifier
                };
                Ok(Token {
                    kind,
                    start,
                    end: self.position,
                    value,
                })
            }
            Some(quote @ '\'' | quote @ '"') => {
                // String literal - single or double quoted
                self.advance(); // consume opening quote
                while let Some(ch) = self.current {
                    if ch == quote {
                        self.advance(); // consume closing quote
                        return Ok(Token {
                            kind: TokenKind::String,
                            start,
                            end: self.position,
                            value: &self.source[start..self.position],
                        });
                    }
                    // TODO(Sprint 7+): Handle escape sequences
                    // Currently missing: \n, \t, \\, \', \", \r, \b, \f, \v
                    // Also missing: \x00 (hex), \u0000 (unicode), \u{0} (unicode code point)
                    // For proper implementation:
                    // 1. Check for backslash
                    // 2. If found, consume next char and validate it's a valid escape
                    // 3. Store both raw (with escapes) and cooked (processed) strings
                    // Note: Parser currently extracts content by removing quotes, which
                    // won't work correctly with escape sequences. Need to process during
                    // lexing and store the cooked string separately.
                    // For Sprint 6, simple strings are sufficient for test case.
                    self.advance();
                }
                // Unterminated string
                Err(ParseError::InvalidSyntax {
                    message: "Unterminated string literal".to_string(),
                    position: start,
                    context: None,
                })
            }
            Some(ch) => Err(ParseError::InvalidSyntax {
                message: format!("Unexpected character: '{}'", ch),
                position: start,
                context: None,
            }),
        }
    }
}
