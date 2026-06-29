// CSS Lexer - tokenization for CSS content in <style> tags
//
// ARCHITECTURE DECISION: Separate Lexer vs Inline Parsing
//
// We use a separate lexer that yields tokens on demand: the parser pulls them one
// at a time (streaming, single-token lookahead, no token vector).
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

/// Construct a boxed lexer error. The lexer returns `Result<_, Box<ParseError>>`
/// (see `From<Box<ParseError>>` in `tsv_lang`): boxing keeps the hot `next_token`
/// Ok path pointer-sized. `#[cold]` / `#[inline(never)]` outlines the error
/// construction so it never bloats the inlined token-scan fast path. Shared by the
/// scanner submodules (each reaches it via `super::lex_err`).
#[cold]
#[inline(never)]
#[allow(clippy::unnecessary_box_returns)] // the box is the point — keeps the hot Result pointer-sized
fn lex_err(message: impl Into<String>, position: usize) -> Box<ParseError> {
    Box::new(ParseError::InvalidSyntax {
        message: message.into(),
        position,
        context: None,
    })
}

pub struct Lexer<'a> {
    source: &'a str,
    pos: usize,
    /// Out-of-band decoded value for the **last token produced**, set only when an
    /// identifier actually contained an escape sequence (the no-escape common case
    /// leaves it `None`, so the token's text is recovered as a verbatim source
    /// slice). The parser drains it with `take_decoded` right after each token.
    /// `Box<String>` keeps the slot pointer-sized. Mirrors `tsv_ts`'s lexer.
    #[allow(clippy::box_collection)]
    decoded: Option<Box<String>>,
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
        Self {
            source,
            pos,
            decoded: None,
        }
    }

    /// Take the decoded value of the most recently produced token, if it required
    /// escape processing (only escaped identifiers do). Leaves the slot `None`.
    #[allow(clippy::box_collection)]
    #[inline]
    pub fn take_decoded(&mut self) -> Option<Box<String>> {
        self.decoded.take()
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
            start: start as u32,
            end: self.pos as u32,
        }
    }

    /// Lex an identifier, stashing any decoded escape value out-of-band in
    /// `self.decoded`. Thin wrapper over the free `identifiers::read_identifier`
    /// so both dispatch arms (`$`-prefixed and plain) share the handoff.
    fn read_identifier(&mut self) -> Result<Token, Box<ParseError>> {
        let (token, decoded) = read_identifier(self.source, &mut self.pos)?;
        self.decoded = decoded;
        Ok(token)
    }

    pub fn next_token(&mut self) -> Result<Token, Box<ParseError>> {
        // Clear any decoded value carried from the previous token (the parser has
        // already drained it via `take_decoded`); only an escaped identifier below
        // sets it again.
        self.decoded = None;

        if self.pos >= self.source.len() {
            return Ok(Token {
                kind: TokenKind::Eof,
                start: self.pos as u32,
                end: self.pos as u32,
            });
        }

        let Some(ch) = self.current_char() else {
            return Ok(Token {
                kind: TokenKind::Eof,
                start: self.pos as u32,
                end: self.pos as u32,
            });
        };

        // Helper macro for single-character tokens
        macro_rules! single_char_token {
            ($kind:expr) => {{
                let start = self.pos;
                self.pos += ch.len_utf8();
                Ok(Token {
                    kind: $kind,
                    start: start as u32,
                    end: self.pos as u32,
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
            '$' if self.peek_char(1).is_some_and(is_identifier_start) => self.read_identifier(),
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
                        start: start as u32,
                        end: self.pos as u32,
                    })
                } else {
                    single_char_token!(TokenKind::Pipe)
                }
            }

            // Identifiers (including those with unicode escapes)
            _ if is_identifier_start(ch) => self.read_identifier(),

            // Unknown character
            _ => Err(lex_err(
                format!("Unexpected character in CSS: '{ch}'"),
                self.pos,
            )),
        }
    }
}
