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
                Ok(Token {
                    kind: TokenKind::Semicolon,
                    start,
                    end: self.position,
                    decoded: None,
                })
            }
            Some(':') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Colon,
                    start,
                    end: self.position,
                    decoded: None,
                })
            }
            Some('=') => {
                self.advance();
                match self.current {
                    Some('>') => {
                        // =>
                        self.advance();
                        Ok(Token {
                            kind: TokenKind::Arrow,
                            start,
                            end: self.position,
                            decoded: None,
                        })
                    }
                    Some('=') => {
                        self.advance();
                        if self.current == Some('=') {
                            // ===
                            self.advance();
                            Ok(Token {
                                kind: TokenKind::EqualsEqualsEquals,
                                start,
                                end: self.position,
                                decoded: None,
                            })
                        } else {
                            // ==
                            Ok(Token {
                                kind: TokenKind::EqualsEquals,
                                start,
                                end: self.position,
                                decoded: None,
                            })
                        }
                    }
                    _ => {
                        // =
                        Ok(Token {
                            kind: TokenKind::Equals,
                            start,
                            end: self.position,
                            decoded: None,
                        })
                    }
                }
            }
            Some(ch) if ch.is_ascii_digit() => {
                // Parse integer part
                while let Some(ch) = self.current {
                    if ch.is_ascii_digit() {
                        self.advance();
                    } else {
                        break;
                    }
                }

                // Check for decimal point (float: 1.5)
                if self.current == Some('.') {
                    // Peek ahead to ensure it's not a method call like 1.toString()
                    let next_char = self.source[self.position + 1..].chars().next();
                    if next_char.is_some_and(|c| c.is_ascii_digit()) {
                        self.advance(); // consume '.'
                        while let Some(ch) = self.current {
                            if ch.is_ascii_digit() {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                }

                // TODO: Support scientific notation (1e10, 1.5e-3)
                // TODO: Support hex (0x10), binary (0b10), octal (0o10)

                Ok(Token {
                    kind: TokenKind::Number,
                    start,
                    end: self.position,
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
                Ok(Token {
                    kind: TokenKind::Comma,
                    start,
                    end: self.position,
                    decoded: None,
                })
            }
            Some('{') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::BraceOpen,
                    start,
                    end: self.position,
                    decoded: None,
                })
            }
            Some('}') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::BraceClose,
                    start,
                    end: self.position,
                    decoded: None,
                })
            }
            Some('[') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::BracketOpen,
                    start,
                    end: self.position,
                    decoded: None,
                })
            }
            Some(']') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::BracketClose,
                    start,
                    end: self.position,
                    decoded: None,
                })
            }
            Some('(') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::ParenOpen,
                    start,
                    end: self.position,
                    decoded: None,
                })
            }
            Some(')') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::ParenClose,
                    start,
                    end: self.position,
                    decoded: None,
                })
            }
            Some('.') => {
                // Could be: ... (spread) or . (member access - not yet implemented)
                let peek1 = self.source[self.position + 1..].chars().next();
                let peek2 = self.source[self.position + 2..].chars().next();
                if peek1 == Some('.') && peek2 == Some('.') {
                    self.advance(); // consume first .
                    self.advance(); // consume second .
                    self.advance(); // consume third .
                    Ok(Token {
                        kind: TokenKind::DotDotDot,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                } else {
                    // Single dot: member access operator
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::Dot,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                }
            }
            Some('-') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Minus,
                    start,
                    end: self.position,
                    decoded: None,
                })
            }
            Some('+') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Plus,
                    start,
                    end: self.position,
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
                        // Division operator /
                        self.advance();
                        Ok(Token {
                            kind: TokenKind::Slash,
                            start,
                            end: self.position,
                            decoded: None,
                        })
                    }
                }
            }
            Some('*') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Star,
                    start,
                    end: self.position,
                    decoded: None,
                })
            }
            Some('%') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Percent,
                    start,
                    end: self.position,
                    decoded: None,
                })
            }
            Some('<') => {
                self.advance();
                if self.current == Some('=') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::LessThanEquals,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::LessThan,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                }
            }
            Some('>') => {
                self.advance();
                if self.current == Some('=') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::GreaterThanEquals,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::GreaterThan,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                }
            }
            Some('!') => {
                self.advance();
                if self.current == Some('=') {
                    self.advance();
                    if self.current == Some('=') {
                        self.advance();
                        Ok(Token {
                            kind: TokenKind::BangEqualsEquals,
                            start,
                            end: self.position,
                            decoded: None,
                        })
                    } else {
                        Ok(Token {
                            kind: TokenKind::BangEquals,
                            start,
                            end: self.position,
                            decoded: None,
                        })
                    }
                } else {
                    Ok(Token {
                        kind: TokenKind::Bang,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                }
            }
            Some('&') => {
                self.advance();
                if self.current == Some('&') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::AmpersandAmpersand,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Ampersand,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                }
            }
            Some('|') => {
                self.advance();
                if self.current == Some('|') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::PipePipe,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Pipe,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                }
            }
            Some('?') => {
                self.advance();
                if self.current == Some('?') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::QuestionQuestion,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Question,
                        start,
                        end: self.position,
                        decoded: None,
                    })
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
