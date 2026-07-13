// CSS Lexer - tokenization for CSS content in <style> tags
//
// ARCHITECTURE DECISION: Separate Lexer vs Inline Parsing
//
// We use a separate lexer that yields tokens on demand: the parser pulls them one
// at a time (streaming, single-token lookahead, no token vector).
// Svelte's CSS parser uses inline parsing (no separate tokenization step) with read_value().
//
// We keep the separate lexer (rather than Svelte's inline single-pass parsing): it's the
// more readable/debuggable factoring and carries no per-token UTF-8-decode tax the inline
// approach would save — `next_token` dispatches byte-first (see `cur_byte`), decoding a
// `char` only at the non-ASCII branches, and the per-identifier decode allocation is lazy
// (see `read_identifier`).
//
// One caller deliberately does NOT pull tokens: a declaration's value. Its text is re-parsed
// from source by `parser::value` regardless, so tokenizing it merely to find its `;`/`}`
// boundary asked the lexer for a token stream nobody kept. `parser::decl_scan` scans those
// bytes directly instead — which makes it a SECOND reader of this grammar, and the one place
// a lexer change can silently break something far away. It mirrors the rules that decide a
// token's extent: `url(…)` opacity (and the identifier-token-start test that gates it),
// strings, comments, escapes, and what counts as whitespace. It reuses this module's own
// `IDENT_CONTINUE_LUT` and `is_ascii_css_whitespace` rather than re-spelling them, and it
// declines (falling back to a real token walk) on anything it does not fully model, so it
// never has to reproduce a lexer *error*. A debug-only assertion re-derives its every result
// from the token walk, so a lexer change that outruns it fails the test suite rather than
// corrupting a parse — but read `decl_scan` before changing token extents here.
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
pub(crate) use identifiers::IDENT_CONTINUE_LUT;
use identifiers::{is_ascii_identifier_start, is_identifier_start, read_identifier};
use numbers::read_number;
use strings::read_string;
pub use token::{Token, TokenKind};
// Shared lexer-error constructor: the scanner submodules reach it via `super::lex_err`.
use tsv_lang::{ParseError, lex_err};

pub struct Lexer<'a> {
    source: &'a str,
    /// `source.as_bytes()`, cached so the hot `next_token` dispatch peeks a byte
    /// without re-slicing + UTF-8-decoding a char per call. Char decoding is done
    /// only at the non-ASCII branches (the dispatch tail, non-ASCII whitespace, the
    /// `$`-ident peek, the url-token scan), which go through `source` at `pos`.
    /// Mirrors `tsv_ts`'s lexer.
    bytes: &'a [u8],
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
            bytes: source.as_bytes(),
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

    /// Reposition the cursor to an absolute byte offset (a char boundary of
    /// `source`) and drop any parked decode. Used by the parser to skip past a
    /// legacy `<!-- ... -->` HTML-comment span (CDO/CDC) it scanned raw — the
    /// only construct where Svelte's `parseCss` consumes a token range the
    /// context-free `next_token` dispatch does not.
    #[inline]
    pub(crate) fn seek(&mut self, pos: usize) {
        self.pos = pos;
        self.decoded = None;
    }

    /// The byte at the cursor, or `None` at EOF. Drives the hot `next_token`
    /// dispatch; non-ASCII bytes (`>= 0x80`) are decoded to a `char` only where a
    /// branch needs one (`current_char`).
    #[inline]
    fn cur_byte(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    /// The byte `offset` bytes ahead of the cursor, or `None` past EOF. Used for the
    /// ASCII lookaheads in the dispatch (`/*`, `-`/`+`/`.` number prefixes, `||`).
    #[inline]
    fn peek_byte(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.pos + offset).copied()
    }

    /// Decode the full character at the cursor (for the non-ASCII branches);
    /// `None` at EOF. The hot ASCII paths use `cur_byte` and never call this.
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
        loop {
            match self.cur_byte() {
                // ASCII whitespace fast path — the overwhelming common case. Advances
                // one byte per whitespace char, decoding nothing (see
                // `is_ascii_css_whitespace`, which matches `char::is_whitespace` for ASCII).
                Some(b) if is_ascii_css_whitespace(b) => self.pos += 1,
                // Any other ASCII byte is not whitespace — stop.
                Some(b) if b < 0x80 => break,
                // Non-ASCII lead byte: decode to test the full Unicode whitespace rule
                // (`char::is_whitespace`, which is true for NBSP, NEL, LS/PS, …). This is
                // the same predicate the char loop applied — byte-identical.
                Some(_) => match self.current_char() {
                    Some(ch) if ch.is_whitespace() => self.pos += ch.len_utf8(),
                    _ => break,
                },
                None => break,
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
            && self.cur_byte() == Some(b'(')
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
        // Opaque scan from just inside `(` to the matching unescaped `)` (or EOF). The two
        // scan targets, `\` and `)`, are ASCII, so neither can occur as a UTF-8
        // continuation byte — a multi-byte code point's trailing bytes are all >= 0x80 and
        // fall through the run, landing on the same `)` the former per-char decode found.
        let bytes = self.source.as_bytes();
        let len = bytes.len();
        let mut j = after_paren;
        loop {
            while j < len && bytes[j] != b'\\' && bytes[j] != b')' {
                j += 1;
            }
            if j >= len {
                break; // EOF before `)` — unterminated; take what we have
            }
            if bytes[j] == b')' {
                j += 1; // include the closing `)`
                break;
            }
            // Escaped code point: the `\` and what it escapes are both content. Stepping
            // one byte past the `\` is enough — the escaped char's continuation bytes can
            // match neither target, so the run passes over them.
            j += 1;
            if j < len {
                j += 1;
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

        let start = self.pos;
        let Some(b) = self.cur_byte() else {
            return Ok(Token {
                kind: TokenKind::Eof,
                start: start as u32,
                end: start as u32,
            });
        };

        // Helper macro for single-byte (ASCII) tokens — every arm below advances one
        // byte, matching the char dispatch's `ch.len_utf8()` for the ASCII punctuation
        // it fires on.
        macro_rules! single_byte_token {
            ($kind:expr) => {{
                self.pos += 1;
                Ok(Token {
                    kind: $kind,
                    start: start as u32,
                    end: self.pos as u32,
                })
            }};
        }

        // Byte-first dispatch: the ASCII cases (the overwhelming majority of CSS bytes)
        // branch on the raw byte with no UTF-8 decode; a `char` is materialized only at
        // the non-ASCII tail. The arm order mirrors the former char dispatch exactly —
        // whitespace, then the number/comment/`||` lookahead arms, then punctuation, then
        // the identifier-start catch-all — so the token stream is byte-identical.
        match b {
            // Whitespace (ASCII subset of `char::is_whitespace`; non-ASCII whitespace is
            // handled in the tail below).
            _ if is_ascii_css_whitespace(b) => Ok(self.skip_whitespace()),

            // Comments
            b'/' if self.peek_byte(1) == Some(b'*') => read_comment(self.source, &mut self.pos),

            // Strings
            b'"' => read_string(self.source, &mut self.pos, '"'),
            b'\'' => read_string(self.source, &mut self.pos, '\''),

            // Numbers (including percentage and dimension)
            _ if b.is_ascii_digit() => read_number(self.source, &mut self.pos),
            b'.' if self.peek_byte(1).is_some_and(|b| b.is_ascii_digit()) => {
                read_number(self.source, &mut self.pos)
            }
            // Negative numbers: -10px, -100%, -.5em (lookahead to distinguish from identifier)
            // Note: -. must be followed by digit (-.5), otherwise it's identifier prefix (-.class is combinator + class)
            b'-' if self.peek_byte(1).is_some_and(|b| b.is_ascii_digit())
                || (self.peek_byte(1) == Some(b'.')
                    && self.peek_byte(2).is_some_and(|b| b.is_ascii_digit())) =>
            {
                read_number(self.source, &mut self.pos)
            }
            // Positive numbers with explicit + sign: +10px, +100%, +.5em
            // Note: +. must be followed by digit (+.5), otherwise it's combinator + class (+.class)
            b'+' if self.peek_byte(1).is_some_and(|b| b.is_ascii_digit())
                || (self.peek_byte(1) == Some(b'.')
                    && self.peek_byte(2).is_some_and(|b| b.is_ascii_digit())) =>
            {
                read_number(self.source, &mut self.pos)
            }

            // Braces and delimiters
            b'{' => single_byte_token!(TokenKind::LeftBrace),
            b'}' => single_byte_token!(TokenKind::RightBrace),
            b'[' => single_byte_token!(TokenKind::LeftBracket),
            b']' => single_byte_token!(TokenKind::RightBracket),
            b'(' => single_byte_token!(TokenKind::LeftParen),
            b')' => single_byte_token!(TokenKind::RightParen),

            // Punctuation
            b':' => single_byte_token!(TokenKind::Colon),
            b';' => single_byte_token!(TokenKind::Semicolon),
            b',' => single_byte_token!(TokenKind::Comma),
            b'.' => single_byte_token!(TokenKind::Dot),
            b'#' => single_byte_token!(TokenKind::Hash),
            b'>' => single_byte_token!(TokenKind::GreaterThan),
            b'<' => single_byte_token!(TokenKind::LessThan),
            b'+' => single_byte_token!(TokenKind::Plus),
            b'~' => single_byte_token!(TokenKind::Tilde),
            b'*' => single_byte_token!(TokenKind::Asterisk),
            b'&' => single_byte_token!(TokenKind::Ampersand),
            b'@' => single_byte_token!(TokenKind::AtSign),
            b'/' => single_byte_token!(TokenKind::Slash),
            b'=' => single_byte_token!(TokenKind::Equals),
            b'%' => single_byte_token!(TokenKind::Percent),
            b'^' => single_byte_token!(TokenKind::Caret),
            // `?` is a query-string char in unquoted url() (e.g. `url(a.ttf?x=1)`).
            // Per css-syntax-3 it's a valid <delim-token>; grammar enforces validity
            // later, so the value reassembler emits it raw like other punctuation.
            b'?' => single_byte_token!(TokenKind::Question),
            // `$`-prefixed identifier (SCSS variable / property name like `$foo`).
            // Svelte's parseCss treats it as a single identifier. A bare `$` (e.g.
            // the `$=` attribute selector) falls through to the Dollar token below.
            // The peek keeps `char` form: the char after `$` can be a non-ASCII
            // identifier code point (`$♥`).
            b'$' if self.peek_char(1).is_some_and(is_identifier_start) => self.read_identifier(),
            b'$' => single_byte_token!(TokenKind::Dollar),
            b'!' => single_byte_token!(TokenKind::Bang),
            b'|' => {
                // Check for || (column combinator)
                if self.peek_byte(1) == Some(b'|') {
                    self.pos += 1; // skip first |
                    self.pos += 1; // skip second |
                    Ok(Token {
                        kind: TokenKind::ColumnCombinator,
                        start: start as u32,
                        end: self.pos as u32,
                    })
                } else {
                    single_byte_token!(TokenKind::Pipe)
                }
            }

            // Identifiers (ASCII start: letters, `-`, `_`, `\`; the non-ASCII identifier
            // code points are handled in the tail). `is_ascii_identifier_start` is false
            // for every non-ASCII byte, so a `>= 0x80` byte falls through to the tail.
            _ if is_ascii_identifier_start(b) => self.read_identifier(),

            // Any other ASCII byte is not a valid token start — error. `b as char` is the
            // exact character the char dispatch would have reported (ASCII round-trips).
            _ if b < 0x80 => Err(lex_err(
                format!("Unexpected character in CSS: '{}'", b as char),
                self.pos,
            )),

            // Non-ASCII lead byte: decode the full char and run the char tail in the
            // former arm order — whitespace (NBSP, NEL, LS/PS, …) first, then the
            // non-ASCII identifier code points, then the unknown-character error.
            _ => match self.current_char() {
                Some(ch) if ch.is_whitespace() => Ok(self.skip_whitespace()),
                Some(ch) if is_identifier_start(ch) => self.read_identifier(),
                Some(ch) => Err(lex_err(
                    format!("Unexpected character in CSS: '{ch}'"),
                    self.pos,
                )),
                // Unreachable: `cur_byte` returned `Some`, so a char decodes here.
                None => Ok(Token {
                    kind: TokenKind::Eof,
                    start: start as u32,
                    end: start as u32,
                }),
            },
        }
    }
}

/// Whether `b` is an ASCII byte that `char::is_whitespace()` treats as whitespace:
/// `<TAB>` U+0009, `<LF>` U+000A, `<VT>` U+000B, `<FF>` U+000C, `<CR>` U+000D, and
/// `<SP>` U+0020 — the Unicode `White_Space` code points below U+0080. Deliberately
/// **not** `u8::is_ascii_whitespace()`, which omits `<VT>` (U+000B) and so would
/// diverge from the char dispatch this replaces.
///
/// Shared with the declaration value's boundary scan, which trims a value's span back to
/// its last non-whitespace token — a trim that is only exact if it means whitespace here.
#[inline]
pub(crate) const fn is_ascii_css_whitespace(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | 0x0B | 0x0C | b'\r' | b' ')
}
