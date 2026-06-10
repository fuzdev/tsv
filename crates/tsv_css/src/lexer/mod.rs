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

mod comments;
mod identifiers;
mod numbers;
mod strings;
pub mod token;

use comments::read_comment;
use identifiers::{is_identifier_start, read_identifier};
use numbers::read_number;
use strings::read_string;
pub use token::{Token, TokenKind};
use tsv_lang::ParseError;

pub struct Lexer<'a> {
    source: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        // Skip UTF-8 BOM (U+FEFF) at start of file if present.
        // BOM is a legacy artifact; we strip it (like deno fmt, VS Code).
        // Position starts after BOM so token spans reflect actual file bytes.
        let pos = if source.starts_with('\u{feff}') {
            '\u{feff}'.len_utf8()
        } else {
            0
        };
        Self { source, pos }
    }

    #[inline]
    fn current_char(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    #[inline]
    fn peek_char(&self, offset: usize) -> Option<char> {
        self.source[self.pos..].chars().nth(offset)
    }

    fn skip_whitespace(&mut self) -> Token {
        let start = self.pos;
        while let Some(ch) = self.current_char() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        Token {
            kind: TokenKind::Whitespace,
            start,
            end: self.pos,
            decoded: None,
        }
    }

    pub fn next_token(&mut self) -> Result<Token, ParseError> {
        if self.pos >= self.source.len() {
            return Ok(Token {
                kind: TokenKind::Eof,
                start: self.pos,
                end: self.pos,
                decoded: None,
            });
        }

        let Some(ch) = self.current_char() else {
            return Ok(Token {
                kind: TokenKind::Eof,
                start: self.pos,
                end: self.pos,
                decoded: None,
            });
        };

        // Helper macro for single-character tokens
        macro_rules! single_char_token {
            ($kind:expr) => {{
                let start = self.pos;
                self.pos += ch.len_utf8();
                Ok(Token {
                    kind: $kind,
                    start,
                    end: self.pos,
                    decoded: None,
                })
            }};
        }

        match ch {
            // Whitespace
            _ if ch.is_whitespace() => Ok(self.skip_whitespace()),

            // Comments
            '/' if self.peek_char(1) == Some('*') => read_comment(self.source, &mut self.pos),

            // Strings
            '"' => read_string(self.source, &mut self.pos, '"'),
            '\'' => read_string(self.source, &mut self.pos, '\''),

            // Numbers (including percentage and dimension)
            _ if ch.is_ascii_digit() => read_number(self.source, &mut self.pos),
            '.' if self.peek_char(1).is_some_and(|ch| ch.is_ascii_digit()) => {
                read_number(self.source, &mut self.pos)
            }
            // Negative numbers: -10px, -100%, -.5em (lookahead to distinguish from identifier)
            // Note: -. must be followed by digit (-.5), otherwise it's identifier prefix (-.class is combinator + class)
            '-' if matches!(self.peek_char(1), Some(c) if c.is_ascii_digit())
                || (self.peek_char(1) == Some('.')
                    && matches!(self.peek_char(2), Some(c) if c.is_ascii_digit())) =>
            {
                read_number(self.source, &mut self.pos)
            }
            // Positive numbers with explicit + sign: +10px, +100%, +.5em
            // Note: +. must be followed by digit (+.5), otherwise it's combinator + class (+.class)
            '+' if matches!(self.peek_char(1), Some(c) if c.is_ascii_digit())
                || (self.peek_char(1) == Some('.')
                    && matches!(self.peek_char(2), Some(c) if c.is_ascii_digit())) =>
            {
                read_number(self.source, &mut self.pos)
            }

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
            '<' => single_char_token!(TokenKind::LessThan),
            '+' => single_char_token!(TokenKind::Plus),
            '~' => single_char_token!(TokenKind::Tilde),
            '*' => single_char_token!(TokenKind::Asterisk),
            '&' => single_char_token!(TokenKind::Ampersand),
            '@' => single_char_token!(TokenKind::AtSign),
            '/' => single_char_token!(TokenKind::Slash),
            '=' => single_char_token!(TokenKind::Equals),
            '%' => single_char_token!(TokenKind::Percent),
            '^' => single_char_token!(TokenKind::Caret),
            // `?` is a query-string char in unquoted url() (e.g. `url(a.ttf?x=1)`).
            // Per css-syntax-3 it's a valid <delim-token>; grammar enforces validity
            // later, so the value reassembler emits it raw like other punctuation.
            '?' => single_char_token!(TokenKind::Question),
            // `$`-prefixed identifier (SCSS variable / property name like `$foo`).
            // Svelte's parseCss treats it as a single identifier. A bare `$` (e.g.
            // the `$=` attribute selector) falls through to the Dollar token below.
            '$' if self.peek_char(1).is_some_and(is_identifier_start) => {
                read_identifier(self.source, &mut self.pos)
            }
            '$' => single_char_token!(TokenKind::Dollar),
            '!' => single_char_token!(TokenKind::Bang),
            '|' => {
                // Check for || (column combinator)
                if self.peek_char(1) == Some('|') {
                    let start = self.pos;
                    self.pos += 1; // skip first |
                    self.pos += 1; // skip second |
                    Ok(Token {
                        kind: TokenKind::ColumnCombinator,
                        start,
                        end: self.pos,
                        decoded: None,
                    })
                } else {
                    single_char_token!(TokenKind::Pipe)
                }
            }

            // Identifiers (including those with unicode escapes)
            _ if is_identifier_start(ch) => read_identifier(self.source, &mut self.pos),

            // Unknown character
            _ => Err(ParseError::InvalidSyntax {
                message: format!("Unexpected character in CSS: '{ch}'"),
                position: self.pos,
                context: None,
            }),
        }
    }
}
