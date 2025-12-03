use std::fmt;
use std::str::Chars;
use tsv_lang::ParseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    LeftAngle,     // <
    RightAngle,    // >
    Slash,         // /
    LeftBrace,     // {
    RightBrace,    // }
    BlockOpen,     // {#
    BlockClose,    // {/
    BlockContinue, // {:
    TagOpen,       // {@
    Equals,        // =
    String,        // "..." attribute values
    Identifier,    // Tag names, attribute names
    Comment,       // <!-- ... -->
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
            TokenKind::BlockOpen => write!(f, "'{{#'"),
            TokenKind::BlockClose => write!(f, "'{{/'"),
            TokenKind::BlockContinue => write!(f, "'{{:'"),
            TokenKind::TagOpen => write!(f, "'{{@'"),
            TokenKind::Equals => write!(f, "'='"),
            TokenKind::String => write!(f, "string"),
            TokenKind::Identifier => write!(f, "identifier"),
            TokenKind::Comment => write!(f, "comment"),
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

    /// Create a new lexer starting at a given position.
    /// The source slice starts from the given position, but positions
    /// are reported relative to the start of the slice (i.e., starting from 0).
    pub fn new_at(source: &'a str, _start_offset: usize) -> Self {
        // Note: _start_offset is informational only - the caller handles
        // adding the base offset to positions. We just lexer the provided slice.
        Self::new(source)
    }

    fn advance(&mut self) {
        if let Some(ch) = self.current {
            self.position += ch.len_utf8();
            self.current = self.chars.next();
        }
    }

    /// Create a token with the current position as end and value extracted from source
    #[inline]
    fn make_token(&self, kind: TokenKind, start: usize) -> Token<'a> {
        Token {
            kind,
            start,
            end: self.position,
            value: &self.source[start..self.position],
        }
    }

    /// Peek at the next n characters without consuming them
    fn peek_chars(&self, n: usize) -> &str {
        let end = (self.position + n).min(self.source.len());
        &self.source[self.position..end]
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
                // Check for HTML comment: <!--
                if self.peek_chars(4) == "<!--" {
                    // Consume "<!--"
                    self.advance(); // <
                    self.advance(); // !
                    self.advance(); // -
                    self.advance(); // -

                    // Scan until "-->"
                    while self.current.is_some() {
                        if self.peek_chars(3) == "-->" {
                            // Consume "-->"
                            self.advance();
                            self.advance();
                            self.advance();
                            return Ok(self.make_token(TokenKind::Comment, start));
                        }
                        self.advance();
                    }

                    // Unterminated comment
                    return Err(ParseError::InvalidSyntax {
                        message: "Unterminated HTML comment".to_string(),
                        position: start,
                        context: None,
                    });
                }

                self.inside_tag = true; // Enter tag mode
                self.advance();
                Ok(self.make_token(TokenKind::LeftAngle, start))
            }
            Some('>') => {
                self.inside_tag = false; // Exit tag mode, back to template mode
                self.advance();
                Ok(self.make_token(TokenKind::RightAngle, start))
            }
            Some('/') => {
                self.advance();
                Ok(self.make_token(TokenKind::Slash, start))
            }
            Some('{') => {
                self.advance();
                // Check for block tokens: {#, {:, {/
                match self.current {
                    Some('#') => {
                        self.advance();
                        Ok(self.make_token(TokenKind::BlockOpen, start))
                    }
                    Some(':') => {
                        self.advance();
                        Ok(self.make_token(TokenKind::BlockContinue, start))
                    }
                    Some('/') => {
                        self.advance();
                        Ok(self.make_token(TokenKind::BlockClose, start))
                    }
                    Some('@') => {
                        self.advance();
                        Ok(self.make_token(TokenKind::TagOpen, start))
                    }
                    _ => Ok(self.make_token(TokenKind::LeftBrace, start)),
                }
            }
            Some('}') => {
                self.advance();
                Ok(self.make_token(TokenKind::RightBrace, start))
            }
            Some('=') => {
                self.advance();
                Ok(self.make_token(TokenKind::Equals, start))
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
                        return Ok(self.make_token(TokenKind::String, start));
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
                // Also include : and | for directive syntax (on:click|preventDefault)
                // and -- for CSS custom properties (style:--custom)
                while let Some(ch) = self.current {
                    if ch.is_alphanumeric()
                        || ch == '_'
                        || ch == '$'
                        || ch == '-'
                        || ch == ':'
                        || ch == '|'
                    {
                        self.advance();
                    } else {
                        break;
                    }
                }
                Ok(self.make_token(TokenKind::Identifier, start))
            }
            Some(ch) => Err(ParseError::InvalidSyntax {
                message: format!("Unexpected character in template: '{ch}'"),
                position: start,
                context: None,
            }),
        }
    }
}
