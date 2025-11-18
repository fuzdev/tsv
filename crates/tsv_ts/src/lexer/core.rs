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
                raw: "",
                decoded: None,
            }),
            Some(';') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Semicolon,
                    start,
                    end: self.position,
                    raw: &self.source[start..self.position],
                    decoded: None,
                })
            }
            Some(':') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Colon,
                    start,
                    end: self.position,
                    raw: &self.source[start..self.position],
                    decoded: None,
                })
            }
            Some('=') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Equals,
                    start,
                    end: self.position,
                    raw: &self.source[start..self.position],
                    decoded: None,
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
                    raw: &self.source[start..self.position],
                    decoded: None,
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
                let raw = &self.source[start..self.position];
                let kind = if let Some(kw) = keyword_kind(raw) {
                    TokenKind::Keyword(kw)
                } else {
                    TokenKind::Identifier
                };
                Ok(Token {
                    kind,
                    start,
                    end: self.position,
                    raw,
                    decoded: None,
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

                        let raw = &self.source[start..self.position];
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
                            raw,
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
                Ok(Token {
                    kind: TokenKind::Comma,
                    start,
                    end: self.position,
                    raw: &self.source[start..self.position],
                    decoded: None,
                })
            }
            Some('{') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::BraceOpen,
                    start,
                    end: self.position,
                    raw: &self.source[start..self.position],
                    decoded: None,
                })
            }
            Some('}') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::BraceClose,
                    start,
                    end: self.position,
                    raw: &self.source[start..self.position],
                    decoded: None,
                })
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
                    _ => {
                        // TODO: Division operator / or /= (not yet implemented)
                        Err(ParseError::InvalidSyntax {
                            message: "Division operator not yet implemented".to_string(),
                            position: start,
                            context: None,
                        })
                    }
                }
            }
            Some(ch) => Err(ParseError::InvalidSyntax {
                message: format!("Unexpected character: '{}'", ch),
                position: start,
                context: None,
            }),
        }
    }
}
