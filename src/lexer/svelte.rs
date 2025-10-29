use crate::error::ParseError;
use std::fmt;
use std::str::Chars;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind {
    LeftAngle,   // <
    RightAngle,  // >
    Slash,       // /
    LeftBrace,   // {
    RightBrace,  // }
    Equals,      // =
    String,      // "..." attribute values
    Identifier,  // Tag names, attribute names
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::LeftAngle => write!(f, "'<'"),
            TokenKind::RightAngle => write!(f, "'>'"),
            TokenKind::Slash => write!(f, "'/'"),
            TokenKind::LeftBrace => write!(f, "'{{'"),
            TokenKind::RightBrace => write!(f, "'}}'"),
            TokenKind::Equals => write!(f, "'='"),
            TokenKind::String => write!(f, "string"),
            TokenKind::Identifier => write!(f, "identifier"),
            TokenKind::Eof => write!(f, "end of file"),
        }
    }
}

/// Zero-allocation token design: value borrows from source string
#[derive(Debug, Clone)]
pub struct Token<'a> {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
    #[allow(dead_code)] // Used for debugging and testing
    pub value: &'a str,
}

pub struct Lexer<'a> {
    source: &'a str,
    chars: Chars<'a>,
    position: usize,
    current: Option<char>,
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
            Some('<') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::LeftAngle,
                    start,
                    end: self.position,
                    value: &self.source[start..self.position],
                })
            }
            Some('>') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::RightAngle,
                    start,
                    end: self.position,
                    value: &self.source[start..self.position],
                })
            }
            Some('/') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Slash,
                    start,
                    end: self.position,
                    value: &self.source[start..self.position],
                })
            }
            Some('{') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::LeftBrace,
                    start,
                    end: self.position,
                    value: &self.source[start..self.position],
                })
            }
            Some('}') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::RightBrace,
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
            Some(quote @ '\'' | quote @ '"') => {
                // String literal for attribute values
                // TODO(Sprint 8+): Handle escape sequences in attribute values
                // Currently missing: \n, \t, \\, \', \", HTML entities (&lt;, &quot;, etc.)
                // For Sprint 7, simple quoted strings are sufficient for test case.
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
                    self.advance();
                }
                // Unterminated string
                Err(ParseError::InvalidSyntax {
                    message: "Unterminated string literal in template".to_string(),
                    position: start,
                })
            }
            Some(ch) if ch.is_alphabetic() || ch == '_' || ch == '$' => {
                // Tag names and identifiers
                while let Some(ch) = self.current {
                    if ch.is_alphanumeric() || ch == '_' || ch == '$' || ch == '-' {
                        self.advance();
                    } else {
                        break;
                    }
                }
                let value = &self.source[start..self.position];
                Ok(Token {
                    kind: TokenKind::Identifier,
                    start,
                    end: self.position,
                    value,
                })
            }
            Some(ch) => Err(ParseError::InvalidSyntax {
                message: format!("Unexpected character in template: '{}'", ch),
                position: start,
            }),
        }
    }
}
