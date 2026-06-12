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

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

pub struct Lexer<'a> {
    source: &'a str,
    chars: Chars<'a>,
    position: usize,
    current: Option<char>,
    pub inside_tag: bool,    // Track if we're inside <...>
    initial_position: usize, // Position after BOM skip (0 or 3)
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
            inside_tag: false,
            initial_position: position,
        }
    }

    /// Returns the initial position after BOM skip (0 if no BOM, 3 if BOM was skipped).
    /// Used by parser to initialize gap tracking.
    pub fn initial_position(&self) -> usize {
        self.initial_position
    }

    /// Create a new lexer starting at a given position.
    /// The source slice starts from the given position, but positions
    /// are reported relative to the start of the slice (i.e., starting from 0).
    pub fn new_at(source: &'a str, _start_offset: usize) -> Self {
        // Note: _start_offset is informational only - the caller handles
        // adding the base offset to positions. We just lexer the provided slice.
        Self::new(source)
    }

    #[inline]
    fn advance(&mut self) {
        if let Some(ch) = self.current {
            self.position += ch.len_utf8();
            self.current = self.chars.next();
        }
    }

    /// Create a token with the current position as end.
    #[inline]
    fn make_token(&self, kind: TokenKind, start: usize) -> Token {
        Token {
            kind,
            start,
            end: self.position,
        }
    }

    /// Peek at the next n characters without consuming them
    fn peek_chars(&self, n: usize) -> &str {
        let remaining = &self.source[self.position..];
        // Count n characters (not bytes) to find the correct byte offset
        let byte_count: usize = remaining.chars().take(n).map(char::len_utf8).sum();
        &remaining[..byte_count]
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

    /// Skip everything until we hit a special character (<, {)
    /// Used in template mode to treat text content as gaps
    /// Note: '}' is NOT special in template mode - it's only consumed directly
    /// during expression tag parsing. This allows '}' in text (e.g., after {'{'}text})
    /// to be treated as plain text, matching Svelte's parser behavior.
    fn skip_to_special_char(&mut self) {
        while let Some(ch) = self.current {
            match ch {
                '<' | '{' => break,
                _ => self.advance(),
            }
        }
    }

    pub fn next_token(&mut self) -> Result<Token, ParseError> {
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
                        // Check if next char is '*' or '/' - that means {/* or {// (comment), not {/if (block close)
                        match self.source.as_bytes().get(self.position + 1) {
                            Some(b'*') | Some(b'/') => {
                                // Comment inside expression: {/* ... */} or {// ...} - return just '{'
                                Ok(self.make_token(TokenKind::LeftBrace, start))
                            }
                            _ => {
                                // Block close: {/if}, {/each}, etc
                                self.advance();
                                Ok(self.make_token(TokenKind::BlockClose, start))
                            }
                        }
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
                // Handle escape sequences AND embedded expression tags like {expr}
                // Inside {}, quotes are part of JS strings, not attribute delimiters
                //
                // NOTE: Similar brace/string tracking logic exists in parse_attribute_value()
                // (attribute.rs). The lexer tokenizes the whole string; the parser later
                // extracts Text and ExpressionTag parts from it. Both need to track JS
                // string contexts to handle quotes correctly.
                self.advance(); // consume opening quote

                let mut brace_depth = 0;
                let mut in_js_string = false;
                let mut js_string_char = '\0';

                while let Some(ch) = self.current {
                    if in_js_string {
                        // Inside a JS string within {expr}
                        if ch == '\\' {
                            // Escape sequence in JS string
                            self.advance();
                            if self.current.is_some() {
                                self.advance();
                            }
                        } else if ch == js_string_char {
                            // End of JS string
                            in_js_string = false;
                            self.advance();
                        } else {
                            self.advance();
                        }
                    } else if brace_depth > 0 {
                        // Inside an expression tag {expr}
                        if ch == '\'' || ch == '"' || ch == '`' {
                            // Start of JS string
                            in_js_string = true;
                            js_string_char = ch;
                            self.advance();
                        } else if ch == '{' {
                            brace_depth += 1;
                            self.advance();
                        } else if ch == '}' {
                            brace_depth -= 1;
                            self.advance();
                        } else {
                            self.advance();
                        }
                    } else {
                        // Outside expression tags
                        if ch == '\\' {
                            // Escape sequence - skip the backslash and the next character
                            self.advance();
                            if self.current.is_some() {
                                self.advance();
                            }
                        } else if ch == quote {
                            self.advance(); // consume closing quote
                            return Ok(self.make_token(TokenKind::String, start));
                        } else if ch == '{' {
                            // Start of expression tag
                            brace_depth = 1;
                            self.advance();
                        } else {
                            self.advance();
                        }
                    }
                }
                // Unterminated string
                Err(ParseError::InvalidSyntax {
                    message: "Unterminated string literal in template".to_string(),
                    position: start,
                    context: None,
                })
            }
            Some(ch) if ch.is_alphabetic() || ch == '_' || ch == '$' || ch == '-' || ch == '!' => {
                // Tag names and identifiers
                // Also include - as a start character for CSS custom property attributes (--margin)
                // and include : and | for directive syntax (on:click|preventDefault)
                // and -- for CSS custom properties (style:--custom)
                // and . for dot notation components (ns.Comp)
                // and ! for <!DOCTYPE> (Svelte treats !DOCTYPE as the element name)
                // Advance past first char — ! is a valid start but not a continuation char
                self.advance();
                while let Some(ch) = self.current {
                    if ch.is_alphanumeric()
                        || ch == '_'
                        || ch == '$'
                        || ch == '-'
                        || ch == ':'
                        || ch == '|'
                        || ch == '.'
                    {
                        self.advance();
                    } else {
                        break;
                    }
                }
                Ok(self.make_token(TokenKind::Identifier, start))
            }
            Some(ch) if ch.is_ascii_digit() => {
                // Unquoted numeric attribute values (e.g., data-count=123)
                // HTML allows unquoted values that are alphanumeric
                while let Some(ch) = self.current {
                    if ch.is_alphanumeric() || ch == '_' || ch == '-' {
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
