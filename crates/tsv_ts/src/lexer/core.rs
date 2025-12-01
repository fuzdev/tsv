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
                if self.current == Some('-') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::MinusMinus,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Minus,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                }
            }
            Some('+') => {
                self.advance();
                if self.current == Some('+') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::PlusPlus,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Plus,
                        start,
                        end: self.position,
                        decoded: None,
                    })
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
                if self.current == Some('*') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::StarStar,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Star,
                        start,
                        end: self.position,
                        decoded: None,
                    })
                }
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
            Some('^') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Caret,
                    start,
                    end: self.position,
                    decoded: None,
                })
            }
            Some('~') => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Tilde,
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
                } else if self.current == Some('<') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::LeftShift,
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
                } else if self.current == Some('>') {
                    self.advance();
                    if self.current == Some('>') {
                        // >>>
                        self.advance();
                        Ok(Token {
                            kind: TokenKind::UnsignedRightShift,
                            start,
                            end: self.position,
                            decoded: None,
                        })
                    } else {
                        // >>
                        Ok(Token {
                            kind: TokenKind::RightShift,
                            start,
                            end: self.position,
                            decoded: None,
                        })
                    }
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
                } else if self.current == Some('.') {
                    // Check for optional chaining `?.`
                    // Must not be followed by a digit (to avoid ambiguity with `?.0` which should be `?` `.0`)
                    let next = self.chars.clone().next();
                    if next.is_none_or(|ch| !ch.is_ascii_digit()) {
                        self.advance();
                        Ok(Token {
                            kind: TokenKind::QuestionDot,
                            start,
                            end: self.position,
                            decoded: None,
                        })
                    } else {
                        // `?.0` should be `?` followed by `.0` (number)
                        Ok(Token {
                            kind: TokenKind::Question,
                            start,
                            end: self.position,
                            decoded: None,
                        })
                    }
                } else {
                    Ok(Token {
                        kind: TokenKind::Question,
                        start,
                        end: self.position,
                        decoded: None,
                    })
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
