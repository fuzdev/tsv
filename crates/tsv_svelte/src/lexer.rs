use std::fmt;
use std::str::Chars;
use tsv_lang::ParseError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind {
    LeftAngle,  // <
    RightAngle, // >
    Slash,      // /
    LeftBrace,  // {
    RightBrace, // }
    Equals,     // =
    String,     // "..." attribute values
    Identifier, // Tag names, attribute names
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
    pub value: &'a str,
}

pub struct Lexer<'a> {
    source: &'a str,
    chars: Chars<'a>,
    position: usize,
    current: Option<char>,
    pub inside_tag: bool, // Track if we're inside <...>
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
            inside_tag: false,
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

    /// Skip everything until we hit a special character (<, {, })
    /// Used in template mode to treat text content as gaps
    fn skip_to_special_char(&mut self) {
        while let Some(ch) = self.current {
            match ch {
                '<' | '{' | '}' => break,
                _ => self.advance(),
            }
        }
    }

    pub fn next_token(&mut self) -> Result<Token<'_>, ParseError> {
        // Template mode (outside tags): skip text content, only tokenize special chars
        // Tag mode (inside <...>): tokenize everything including identifiers
        if self.inside_tag {
            self.skip_whitespace();
        } else {
            self.skip_to_special_char();
        }

        let start = self.position;

        match self.current {
            None => Ok(Token {
                kind: TokenKind::Eof,
                start,
                end: start,
                value: "",
            }),
            Some('<') => {
                self.inside_tag = true; // Enter tag mode
                self.advance();
                Ok(Token {
                    kind: TokenKind::LeftAngle,
                    start,
                    end: self.position,
                    value: &self.source[start..self.position],
                })
            }
            Some('>') => {
                self.inside_tag = false; // Exit tag mode, back to template mode
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
                // TODO: Handle escape sequences in attribute values
                // Currently missing: \n, \t, \\, \', \", HTML entities (&lt;, &quot;, etc.)
                // Simple quoted strings work for current test cases.
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
                    context: None,
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
                context: None,
            }),
        }
    }
}
