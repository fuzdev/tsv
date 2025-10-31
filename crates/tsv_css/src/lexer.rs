// CSS Lexer - tokenization for CSS content in <style> tags

use tsv_lang::ParseError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind {
    Identifier, // div, color, red, etc.
    LeftBrace,  // {
    RightBrace, // }
    Colon,      // :
    Semicolon,  // ;
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

    #[expect(dead_code, reason = "Reserved for future CSS lexer lookahead")]
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

    fn read_identifier(&mut self) -> Token {
        let start = self.pos;
        while let Some(ch) = self.current_char() {
            // CSS identifiers: alphanumeric, hyphen, underscore
            // For minimal impl, be permissive
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }
        Token {
            kind: TokenKind::Identifier,
            start,
            end: self.pos,
        }
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

        match ch {
            '{' => {
                let start = self.pos;
                self.advance();
                Ok(Token {
                    kind: TokenKind::LeftBrace,
                    start,
                    end: self.pos,
                })
            }
            '}' => {
                let start = self.pos;
                self.advance();
                Ok(Token {
                    kind: TokenKind::RightBrace,
                    start,
                    end: self.pos,
                })
            }
            ':' => {
                let start = self.pos;
                self.advance();
                Ok(Token {
                    kind: TokenKind::Colon,
                    start,
                    end: self.pos,
                })
            }
            ';' => {
                let start = self.pos;
                self.advance();
                Ok(Token {
                    kind: TokenKind::Semicolon,
                    start,
                    end: self.pos,
                })
            }
            _ if ch.is_whitespace() => Ok(self.skip_whitespace()),
            _ if ch.is_alphanumeric() || ch == '-' || ch == '_' => Ok(self.read_identifier()),
            _ => Err(ParseError::InvalidSyntax {
                message: format!("Unexpected character in CSS: '{}'", ch),
                position: self.pos,
                context: None,
            }),
        }
    }
}
