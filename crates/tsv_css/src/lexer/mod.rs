// CSS Lexer - tokenization for CSS content in <style> tags
//
// ARCHITECTURE DECISION: Separate Lexer vs Inline Parsing
//
// We use a separate lexer that yields tokens on demand: the parser pulls them one
// at a time (streaming, single-token lookahead, no token vector).
// Svelte's CSS parser uses inline parsing (no separate tokenization step) with read_value().
//
// We keep the separate lexer: it's the more readable/debuggable factoring, and current
// profiling doesn't favor inlining it. On the files profiled so far the token cursor is a
// small share (~2.5%) and the CSS-parse hotspot is the value parser's repeated structural
// re-scan (`ValueParser::parse` and the `contains_*` walks), so the inline single-pass
// approach has nothing to win against right now — and the per-identifier decode allocation is
// already lazy (see `read_identifier`). This is a snapshot, not a closed door: if the
// value-parser re-scan is collapsed (the bigger lever), the cursor's relative share rises and
// a byte-cursor / inline tokenization may become worth re-measuring.
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
    /// slice). `advance`/`new` drain it with `take_decoded` right after lexing;
    /// `peek` leaves it parked here for the matching `advance`-from-cache to claim,
    /// so a peeked escaped identifier keeps its decode. `Box<String>` keeps the slot
    /// pointer-sized. Mirrors `tsv_ts`'s lexer.
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
        // css-syntax "consume an ident-like token": an ident whose value is an
        // ASCII-case-insensitive `url`, immediately followed by `(` whose first
        // non-whitespace content isn't a quote, is a `<url-token>` — consume it opaquely
        // to the matching `)` so an interior `/*`, `:`, `,` etc. is literal content, not
        // a comment / colon / separator. Quoted `url("…")` stays ident + `(` + string (a
        // function-token). Match the decoded value so an escaped spelling still counts.
        let ident_text = decoded.as_deref().map_or(
            &self.source[token.start as usize..token.end as usize],
            |s| s.as_str(),
        );
        if ident_text.eq_ignore_ascii_case("url")
            && self.current_char() == Some('(')
            && let Some(url) = self.consume_url_token(token.start)
        {
            // The url-token text is recovered verbatim from its span — no decode.
            self.decoded = None;
            return Ok(url);
        }
        self.decoded = decoded;
        Ok(token)
    }

    /// From `self.pos` at the `(` after a `url` ident, try to consume an opaque
    /// `<url-token>` (css-syntax §4.3.6). Returns `None` — leaving `self.pos` unmoved,
    /// so the caller lexes `(` normally — when the parens open a quoted string (that's a
    /// function-token, `url("…")`). Otherwise consumes to the matching **unescaped** `)`
    /// (or EOF: an unterminated url-token is taken as-is; tsv doesn't model bad-url
    /// recovery) and returns the whole `url(...)` as one `TokenKind::Url`.
    fn consume_url_token(&mut self, url_start: u32) -> Option<Token> {
        // Peek past `(` and any leading whitespace to classify the first content char.
        let after_paren = self.pos + 1; // `(` is one byte
        let mut i = after_paren;
        while let Some(ch) = self.source[i..].chars().next() {
            if ch.is_whitespace() {
                i += ch.len_utf8();
            } else {
                break;
            }
        }
        // A quote opens a string arg → `url("…")` is a function-token, not a url-token.
        if matches!(self.source[i..].chars().next(), Some('"' | '\'')) {
            return None;
        }
        // Opaque scan from just inside `(` to the matching unescaped `)` (or EOF).
        let mut j = after_paren;
        loop {
            match self.source[j..].chars().next() {
                None => break, // EOF before `)` — unterminated; take what we have
                Some('\\') => {
                    // Escaped code point: the `\` and the next char are both content.
                    j += 1;
                    if let Some(esc) = self.source[j..].chars().next() {
                        j += esc.len_utf8();
                    }
                }
                Some(')') => {
                    j += 1; // include the closing `)`
                    break;
                }
                Some(ch) => j += ch.len_utf8(),
            }
        }
        self.pos = j;
        Some(Token {
            kind: TokenKind::Url,
            start: url_start,
            end: j as u32,
        })
    }

    pub fn next_token(&mut self) -> Result<Token, Box<ParseError>> {
        // Start each token with a clean decoded slot. Callers drain the prior
        // token's decode (`advance`/`new` at once, `peek` via its matching
        // `advance`-from-cache), so this is normally already `None` — cheap
        // insurance that a stale decode can never leak onto a later token. Only an
        // escaped identifier below sets it again.
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
