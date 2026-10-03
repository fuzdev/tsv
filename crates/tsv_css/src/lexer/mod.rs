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
// approach would save — the token front (`next_token_into_local`) runs byte-first, decoding a
// `char` only at the non-ASCII branches, and the per-identifier decode allocation is lazy (see
// `read_identifier`).
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
use identifiers::{
    IDENT_CHUNK, ascii_identifier_chunk_stop, ascii_identifier_run_end, is_ascii_identifier_start,
    is_identifier_start, read_identifier, run_stop_ends_identifier,
};
pub(crate) use identifiers::{
    IDENT_CONTINUE_LUT, hyphen_starts_own_token, is_non_ascii_identifier_codepoint,
};
use numbers::read_number_into;
use strings::read_string;
pub(crate) use strings::string_end;
pub use token::{Token, TokenKind};
// Shared lexer-error constructor: the scanner submodules reach it via `super::lex_err`.
use tsv_lang::{ParseError, lex_err};

pub struct Lexer<'a> {
    source: &'a str,
    /// `source.as_bytes()`, cached so the hot `next_token_into_local` front reads a byte
    /// without re-slicing + UTF-8-decoding a char per call. Char decoding is done only at
    /// the non-ASCII branches (the dispatch's non-ASCII arm, non-ASCII whitespace, the
    /// `$`-ident peek, the url-token scan), which go through `source` at `pos`. Mirrors
    /// `tsv_ts`'s lexer.
    bytes: &'a [u8],
    pos: usize,
    /// Out-of-band decoded value for the **last token produced**, populated only when
    /// an identifier actually contained an escape sequence (the no-escape common case
    /// leaves `has_decoded` false, so the token's text is recovered as a verbatim
    /// source slice). Mirrors `tsv_ts`'s lexer.
    ///
    /// The decoded bytes live in `decode_scratch`, a buffer parked on the lexer and
    /// **reused across the file** (cleared per escape, capacity retained), so no
    /// per-identifier `String` (plus its `Box`) allocates on the rare escape path.
    /// `has_decoded` is the presence flag `decoded_str` reads; it is cleared at the
    /// top of every token's scan (`next_token_into_local`) and by `seek`, so it
    /// reflects only the current token, and set by the escaped-identifier path.
    /// `advance`/`new` copy the borrowed scratch into the AST arena right after
    /// lexing; `peek_kind` leaves it parked here for the matching `advance`-from-cache to
    /// claim, so a peeked escaped identifier keeps its decode (nothing re-lexes
    /// between the peek and its consume, so the scratch is intact). The scratch is
    /// never read while `has_decoded` is false, so its stale contents are inert.
    decode_scratch: String,
    has_decoded: bool,
    /// Byte offset of `source` within the document this lexer's ERRORS are rendered
    /// against — zero when that document *is* `source`, the island start for a Svelte
    /// `<style>`, the slice start for the parser's declaration-value scanners. Token
    /// positions are unaffected (the parser shifts those itself when it builds spans).
    /// See [`Lexer::host_err`].
    base_offset: usize,
}

impl<'a> Lexer<'a> {
    /// A lexer over `source`, which sits at `base_offset` in the document its errors will
    /// be rendered against.
    ///
    /// The offset is a required argument rather than a `new(source)` default because a
    /// silent zero is the failure mode, and this crate lexes short slices routinely
    /// (`source[from..]` for the declaration-value scan): a lexer reporting its own
    /// coordinates there puts the caret near the top of the file for an error in the
    /// middle of a stylesheet.
    pub fn at_offset(source: &'a str, base_offset: usize) -> Self {
        // Skip UTF-8 BOM (U+FEFF) at start of file if present.
        // BOM is a legacy artifact; we strip it (like deno fmt, VS Code).
        // Position starts after BOM so token spans reflect actual file bytes; the WIRE
        // elides it at emission (`LeadingBom::Elided` in `WIRE_COORDINATES`), since `parseCss`
        // strips it before parsing and its offsets index the BOM-less string.
        let pos = tsv_lang::leading_bom_len(source);
        Self {
            source,
            bytes: source.as_bytes(),
            pos,
            decode_scratch: String::new(),
            has_decoded: false,
            base_offset,
        }
    }

    /// Lift an error this lexer produced into the coordinates of the document it will be
    /// rendered against (`ParseError::shift_position`).
    ///
    /// Applied once, at [`Lexer::next_token_into`] — this lexer's only fallible scan (the
    /// by-value [`Lexer::next_token`] wraps it), and so its only producer.
    #[cold]
    #[inline(never)]
    fn host_err(&self, err: ParseError) -> ParseError {
        err.shift_position(self.base_offset)
    }

    /// The decoded value of the most recently produced token, if it required escape
    /// processing (only escaped identifiers do); `None` for the common escape-free
    /// token. Borrows the parked `decode_scratch` — valid until the next token is
    /// lexed (which may overwrite it), so the parser copies it into its AST arena
    /// immediately after each lex (`CssParser::decoded_to_arena`) rather than holding
    /// the borrow.
    #[inline]
    pub fn decoded_str(&self) -> Option<&str> {
        if self.has_decoded {
            Some(&self.decode_scratch)
        } else {
            None
        }
    }

    /// Reposition the cursor to an absolute byte offset (a char boundary of
    /// `source`) and drop any parked decode. The parser's jumps go through it: the
    /// boundary re-read (`token_at`), a rewind or resume, seating a scanned
    /// terminator, and the skip past a legacy `<!-- ... -->` HTML-comment span
    /// (CDO/CDC) — the only construct where Svelte's `parseCss` consumes a token
    /// range the context-free token dispatch does not. Temporary scan lexers seek to
    /// their start the same way.
    #[inline]
    pub(crate) fn seek(&mut self, pos: usize) {
        self.pos = pos;
        self.has_decoded = false;
    }

    /// The byte at the cursor, or `None` at EOF; non-ASCII bytes (`>= 0x80`) are decoded to
    /// a `char` only where a branch needs one (`current_char`).
    #[inline]
    fn cur_byte(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    /// Decode the full character at the cursor (for the non-ASCII branches);
    /// `None` at EOF. The hot ASCII paths read bytes and never call this.
    #[inline]
    fn current_char(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    #[inline]
    fn peek_char(&self, offset: usize) -> Option<char> {
        self.source[self.pos..].chars().nth(offset)
    }

    /// Lex a token starting at `at`, re-reading from a position the parser has stepped the
    /// cursor back to — the boundary-whitespace skip `parseCss` performs with
    /// `allow_whitespace()` before a token starts (`CssParser::skip_boundary_whitespace`).
    ///
    /// It exists because the two readings of a code point ≥ U+00A0 disagree and only position
    /// resolves them: inside a value or glued to a name it is identifier content, which is
    /// what the lexer assumes, and at a boundary it is a separator.
    ///
    /// ⚠️ Routed through the **whole token scan** (`next_token`), not through
    /// `read_identifier` alone. The run was consumed as the head of an identifier, but what
    /// follows it need not be one: a `<NBSP>` glues to a digit as readily as to a letter, so
    /// `{<NBSP>0%` lexes as one identifier and the byte past the run opens a
    /// `<percentage-token>`. Re-reading that as an identifier yields a token the ordinary
    /// scan would never have produced —
    /// `parseCss` reaches the same byte through `allow_whitespace()` and then takes its
    /// `REGEX_PERCENTAGE` arm — and the keyframe stop became a parse error. The remainder
    /// can never be whitespace or a comment (the identifier lexer would not have consumed
    /// one), so the extra arms only add the readings this position really has.
    pub(crate) fn token_at(&mut self, at: usize) -> Result<Token, ParseError> {
        self.seek(at);
        self.next_token()
    }

    /// [`Lexer::next_token_dispatch`]'s identifier reader, for a name the front
    /// ([`Lexer::next_token_into_local`]) does not settle: one led by a `$` or a non-ASCII
    /// code point, one led by `u` / `U`, which may be a `url` opening a `<url-token>`, and an
    /// ASCII-led one within a chunk of the end of input.
    ///
    /// The escape-free identifier whose ASCII run (past an optional `$`) stops on an ASCII
    /// byte other than `\`, or at end of input, is read here; the rest goes to the cold
    /// [`Lexer::read_decoded_identifier_into`], which re-reads it from its start.
    ///
    /// css-syntax "consume an ident-like token": an ident whose value is an
    /// ASCII-case-insensitive `url`, immediately followed by `(` whose first
    /// non-whitespace content isn't a quote, is a `<url-token>` — consumed opaquely to the
    /// matching `)` so an interior `/*`, `:`, `,` etc. is literal content, not a comment /
    /// colon / separator. Quoted `url("…")` stays ident + `(` + string (a function-token).
    /// An unescaped spelling is its own source bytes, three of them to be a `url` at all.
    fn identifier_into(&mut self, dst: &mut Token) -> Result<(), ParseError> {
        let bytes = self.bytes;
        let start = self.pos;
        let run_from = start + usize::from(bytes.get(start) == Some(&b'$'));
        let end = ascii_identifier_run_end(bytes, run_from);
        if !run_stop_ends_identifier(bytes.get(end).copied()) {
            return self.read_decoded_identifier_into(dst);
        }
        self.pos = end;
        let identifier = Token {
            kind: TokenKind::Identifier,
            start: start as u32,
            end: end as u32,
        };
        *dst = if end - start == 3
            && bytes[start..end].eq_ignore_ascii_case(b"url")
            && self.cur_byte() == Some(b'(')
        {
            self.consume_url_token(start as u32).unwrap_or(identifier)
        } else {
            identifier
        };
        Ok(())
    }

    /// The general identifier reader, for an identifier whose ASCII run stops on a
    /// non-ASCII byte or a `\`, or that opens with a `\`: the free
    /// `identifiers::read_identifier` from the identifier's start. The front
    /// ([`Lexer::next_token_into_local`]) and [`Lexer::identifier_into`] hand it such a
    /// name with the cursor still on its first byte.
    #[cold]
    #[inline(never)]
    fn read_decoded_identifier_into(&mut self, dst: &mut Token) -> Result<(), ParseError> {
        let (token, decoded) = read_identifier(self.source, &mut self.pos)?;
        // The same `<url-token>` fork as `identifier_into`'s, matched on the decoded value so
        // an escaped spelling (`u\rl(`) still counts.
        let is_url = match decoded.as_deref() {
            None => {
                token.end - token.start == 3
                    && self.source.as_bytes()[token.start as usize..token.end as usize]
                        .eq_ignore_ascii_case(b"url")
            }
            Some(text) => text.eq_ignore_ascii_case("url"),
        };
        if is_url
            && self.cur_byte() == Some(b'(')
            && let Some(url) = self.consume_url_token(token.start)
        {
            // The url-token text is recovered verbatim from its span — no decode.
            self.has_decoded = false;
            *dst = url;
            return Ok(());
        }
        // Escaped identifiers are near-zero in real code; funnel the rare local
        // buffer into the parked scratch so `decoded_str` reads it uniformly.
        match decoded {
            Some(s) => {
                self.decode_scratch.clear();
                self.decode_scratch.push_str(&s);
                self.has_decoded = true;
            }
            None => self.has_decoded = false,
        }
        *dst = token;
        Ok(())
    }

    /// From `self.pos` at the `(` after a `url` ident, try to consume an opaque
    /// `<url-token>` (css-syntax §4.3.6). Returns `None` — leaving `self.pos` unmoved,
    /// so the caller lexes `(` normally — when the parens open a quoted string (that's a
    /// function-token, `url("…")`). Otherwise consumes to the matching **unescaped** `)`
    /// (or EOF: an unterminated url-token is taken as-is; tsv doesn't model bad-url
    /// recovery) and returns the whole `url(...)` as one `TokenKind::Url`.
    fn consume_url_token(&mut self, url_start: u32) -> Option<Token> {
        let after_paren = self.pos + 1; // `(` is one byte
        // A quote opens a string arg → `url("…")` is a function-token, not a url-token.
        if url_arg_is_quoted(self.source, after_paren) {
            return None;
        }
        // Opaque scan from just inside `(` to one past the matching unescaped `)`. An
        // unterminated url-token is taken as-is to EOF (tsv doesn't model bad-url
        // recovery).
        let j = url_token_close(self.source.as_bytes(), after_paren).unwrap_or(self.source.len());
        self.pos = j;
        Some(Token {
            kind: TokenKind::Url,
            start: url_start,
            end: j as u32,
        })
    }

    /// Lex the next token straight into `*dst` — the parser's hot path, which writes its
    /// current-token slot (`advance`) and its lookahead slot (`peek_kind`) in place. A
    /// `Result<Token, ParseError>` does not come back in registers: it is returned through
    /// a stack slot the caller then reloads, copies and re-scatters, once per token.
    /// Writing through the caller's slot leaves only the error pointer to return. `*dst`
    /// is written only on success, so an error leaves the caller's token as it was.
    ///
    /// The lexer's one fallible scan, so the error path is lifted into host coordinates here
    /// ([`Lexer::host_err`]); the scan itself reports in the lexer's own.
    #[inline]
    pub fn next_token_into(&mut self, dst: &mut Token) -> Result<(), ParseError> {
        self.next_token_into_local(dst)
            .map_err(|err| self.host_err(err))
    }

    /// By-value next-token for every caller but the parser's hot path: its bootstrap
    /// (`CssParser::new`, lexing the first token) and its boundary re-read
    /// ([`Lexer::token_at`], whose token the parser then assigns to its current slot), the
    /// short-lived lexers the parser opens to scan ahead or re-read a slice (the
    /// `peek_past_*` scans, `decl_scan`, the selector scans), the printer's own token
    /// scans, and `debug_token_stream`. The hot path — `advance` and `peek_kind` — writes
    /// the current token and the lookahead slot in place through [`Lexer::next_token_into`].
    pub fn next_token(&mut self) -> Result<Token, ParseError> {
        let mut token = Token {
            kind: TokenKind::Eof,
            start: 0,
            end: 0,
        };
        self.next_token_into(&mut token)?;
        Ok(token)
    }

    /// [`Lexer::next_token_into`]'s scan, reporting an error at its position in `self.source`.
    ///
    /// This is the scan's **front**: it lexes every token whose path makes no call — end of
    /// input, an ASCII whitespace run, the common identifier (ASCII-led, its ASCII run
    /// stopping on an ASCII byte other than `\` or at end of input) and the punctuation,
    /// which together are nearly every token — and hands the rest to a function that
    /// finishes the token, entered as the front's last act so the handoff compiles to a
    /// jump: [`Lexer::next_token_dispatch`] for the scanners (comments, strings, numbers),
    /// a `$`, the non-ASCII-led, `u`-led and near-the-end names, a whitespace run that reaches
    /// a non-ASCII byte and the errors, [`Lexer::finish_long_identifier_into`] for a common
    /// identifier longer than the front's one chunk, and the cold
    /// [`Lexer::read_decoded_identifier_into`] for a name an escape opens or continues, or a
    /// non-ASCII code point continues. Every
    /// handoff leaves the cursor on the token's first byte, and the finisher lexes the token
    /// from there. The split is what keeps the front free of a frame: a function making the
    /// scanners' calls saves registers on entry and restores them at every exit — on every
    /// token, the ones that call nothing included.
    ///
    /// ⚠️ So nothing the front reaches may make a call it then continues past. A handoff must
    /// stay in tail position and return this function's own `Result` (a one-word niche,
    /// returned in a register); a callee that returns anything else — or one the optimizer
    /// can see always succeeds, whose `Ok` it then materializes here after a call — puts the
    /// call and the frame back. And the front runs on the caller-saved registers alone, so
    /// what it keeps live across the identifier and whitespace runs is budgeted too: the
    /// whitespace run walks a table ([`ascii_whitespace_run_end`]) because the class's own
    /// spelling, a subtract and two compares a byte, took one register more than the front
    /// has to spare.
    ///
    /// `#[inline(never)]`: small and frame-free, the front would otherwise be a candidate
    /// for inlining into its callers — `CssParser::advance`, `peek_kind` and the parser's
    /// lookahead scans — and a lexer inlined into a frame the parser's recursion stacks
    /// grows that frame.
    ///
    /// The match yields a `TokenKind` only for the tokens the front builds, whose kinds carry
    /// no payload; every handoff returns early. A payload kind yielded by the match is not
    /// free even where it is rare: the arms merge into one value, so every token would then
    /// copy the payload bytes through the merged stack temporary into `dst`.
    #[inline(never)]
    fn next_token_into_local(&mut self, dst: &mut Token) -> Result<(), ParseError> {
        // Start each token with a clean decoded flag. Callers copy the prior token's
        // decode out (`advance`/`new` at once, `peek` via its matching
        // `advance`-from-cache), so a stale decode never leaks onto a later token.
        // Only an escaped identifier sets it again (reusing `decode_scratch`).
        self.has_decoded = false;

        let bytes = self.bytes;
        let start = self.pos;
        let Some(&b) = bytes.get(start) else {
            *dst = Token {
                kind: TokenKind::Eof,
                start: start as u32,
                end: start as u32,
            };
            return Ok(());
        };

        // A single-byte (ASCII) token: the cursor steps to one past the byte the arm
        // matched.
        macro_rules! single_byte_token {
            ($kind:expr) => {{
                self.pos = start + 1;
                $kind
            }};
        }
        // Byte-first: the ASCII cases (the overwhelming majority of CSS bytes) branch on the
        // raw byte with no UTF-8 decode; a `char` is materialized only by the dispatch's
        // non-ASCII arm. A guarded lookahead arm (`/*`, a `.` or a sign opening a number)
        // precedes the unguarded arm for the same byte, so it is asked first.
        let kind = match b {
            // Whitespace (ASCII subset of `char::is_whitespace`). A run that reaches a
            // non-ASCII byte goes to the dispatch, which finishes it against the non-ASCII
            // class.
            b'\t' | b'\n' | 0x0B | 0x0C | b'\r' | b' ' => {
                let end = ascii_whitespace_run_end(bytes, start + 1);
                if bytes.get(end).is_some_and(|b| !b.is_ascii()) {
                    return self.next_token_dispatch(dst);
                }
                self.pos = end;
                TokenKind::Whitespace
            }

            // Comments, strings and numbers: the dispatch's scanners.
            b'/' if bytes.get(start + 1) == Some(&b'*') => return self.next_token_dispatch(dst),
            b'"' | b'\'' | b'0'..=b'9' => return self.next_token_dispatch(dst),
            b'.' if bytes.get(start + 1).is_some_and(u8::is_ascii_digit) => {
                return self.next_token_dispatch(dst);
            }
            // Signed numbers: -10px, -100%, -.5em, +10px, +.5em. A sign glued to `.` must be
            // followed by a digit to open one (`-.class` is an identifier prefix, `+.class`
            // a combinator and a class).
            b'-' | b'+' if sign_opens_number(bytes, start) => {
                return self.next_token_dispatch(dst);
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
            // A `$`-prefixed identifier, or a bare `$`.
            b'$' => return self.next_token_dispatch(dst),
            b'!' => single_byte_token!(TokenKind::Bang),
            b'|' => {
                // Check for || (column combinator)
                if bytes.get(start + 1) == Some(&b'|') {
                    self.pos = start + 2;
                    TokenKind::ColumnCombinator
                } else {
                    single_byte_token!(TokenKind::Pipe)
                }
            }

            // A name led by `u` / `U` may be a `url(` opening a `<url-token>`, which the
            // dispatch decides: it is handed on unread. Few names open with the letter, and
            // with no `url` test left to make here, the walk below keeps no lead byte live —
            // the register its chunk needs.
            b'u' | b'U' => return self.next_token_dispatch(dst),
            // The common identifier: led by an ASCII letter other than `u`, `-` or `_` (all
            // [`IDENT_CONTINUE_LUT`] members, so the run walks on from the next byte), its run
            // stopping on an ASCII byte other than `\` or at end of input. A `\` or a
            // non-ASCII stop byte may continue the name, so it hands the name on unread.
            //
            // The front walks one chunk of the run, [`IDENT_CHUNK`] bytes behind one bound
            // check, which ends most names; a run that fills it goes on in
            // [`Lexer::finish_long_identifier_into`], and a name within a chunk of the end of
            // input goes to the dispatch. The whole run is not walked here because its loop's
            // bookkeeping takes a register more than the front has to spare.
            b'a'..=b'z' | b'A'..=b'Z' | b'-' | b'_' => {
                let from = start + 1;
                let Some(chunk) = bytes.get(from..from + IDENT_CHUNK) else {
                    return self.next_token_dispatch(dst);
                };
                let Some(i) = ascii_identifier_chunk_stop(chunk) else {
                    return self.finish_long_identifier_into(dst, from);
                };
                let end = from + i;
                if !run_stop_ends_identifier(bytes.get(end).copied()) {
                    return self.read_decoded_identifier_into(dst);
                }
                self.pos = end;
                TokenKind::Identifier
            }
            // An identifier opening with an escape.
            b'\\' => return self.read_decoded_identifier_into(dst),

            // Non-ASCII lead bytes and the ASCII bytes no token starts with.
            _ => return self.next_token_dispatch(dst),
        };
        *dst = Token {
            kind,
            start: start as u32,
            end: self.pos as u32,
        };
        Ok(())
    }

    /// The front's handoff for a common identifier whose run filled its first chunk, the
    /// [`IDENT_CHUNK`] bytes from `from`: the run walked on past it, then settled as the
    /// front settles it. The cursor is still on the name's first byte. (The chunk's end is
    /// found here rather than passed, so the front keeps no register live for it.) A `\` or
    /// non-ASCII stop byte, which may continue the name, goes to
    /// [`Lexer::read_decoded_identifier_into`], so the handoff's `Result` is not one the
    /// optimizer can compute in the front.
    #[inline(never)]
    fn finish_long_identifier_into(
        &mut self,
        dst: &mut Token,
        from: usize,
    ) -> Result<(), ParseError> {
        let start = self.pos;
        let end = ascii_identifier_run_end(self.bytes, from + IDENT_CHUNK);
        if !run_stop_ends_identifier(self.bytes.get(end).copied()) {
            return self.read_decoded_identifier_into(dst);
        }
        self.pos = end;
        *dst = Token {
            kind: TokenKind::Identifier,
            start: start as u32,
            end: end as u32,
        };
        Ok(())
    }

    /// Lex the token at the cursor into `*dst` — every token the front
    /// ([`Lexer::next_token_into_local`]) hands on: the ones whose scanners it would have to
    /// call (comments, strings, numbers, a whitespace run reaching a non-ASCII byte), the
    /// names it does not settle (`$`-led, non-ASCII-led, `u`-led — which may open a
    /// `<url-token>` — and an ASCII-led one within a chunk of the end of input), a bare `$`,
    /// and the errors. Each arm is the front's for its byte
    /// narrowed to what the front hands on, which the `debug_assert!`s state.
    ///
    /// `#[inline(never)]` because the front's frame-free shape depends on it: inlined, its
    /// calls would put their register saves back on every token.
    #[inline(never)]
    fn next_token_dispatch(&mut self, dst: &mut Token) -> Result<(), ParseError> {
        let bytes = self.bytes;
        let start = self.pos;
        let first = bytes[start];
        let kind = match first {
            // A whitespace run the front's ASCII walk stopped on a non-ASCII byte.
            _ if is_ascii_css_whitespace(first) => {
                let end = ascii_whitespace_run_end(bytes, start + 1);
                debug_assert!(bytes.get(end).is_some_and(|b| !b.is_ascii()));
                self.pos = non_ascii_whitespace_run_end(self.source, end);
                TokenKind::Whitespace
            }
            b'/' => {
                debug_assert_eq!(bytes.get(start + 1), Some(&b'*'));
                *dst = read_comment(self.source, &mut self.pos)?;
                return Ok(());
            }
            b'"' | b'\'' => {
                *dst = read_string(self.source, &mut self.pos, first as char)?;
                return Ok(());
            }
            // Numbers (including percentage and dimension); the front hands on a `.` or a
            // `+` only when it opens one.
            b'0'..=b'9' | b'.' | b'+' => {
                debug_assert!(
                    first.is_ascii_digit()
                        || (first == b'.' && bytes.get(start + 1).is_some_and(u8::is_ascii_digit))
                        || (first == b'+' && sign_opens_number(bytes, start))
                );
                read_number_into(self.source, &mut self.pos, dst);
                return Ok(());
            }
            b'-' if sign_opens_number(bytes, start) => {
                read_number_into(self.source, &mut self.pos, dst);
                return Ok(());
            }
            // `$`-prefixed identifier (SCSS variable / property name like `$foo`).
            // Svelte's parseCss treats it as a single identifier. A bare `$` (e.g.
            // the `$=` attribute selector) is the Dollar token. The peek keeps `char`
            // form: the char after `$` can be a non-ASCII identifier code point (`$♥`).
            b'$' if self.peek_char(1).is_some_and(is_identifier_start) => {
                return self.identifier_into(dst);
            }
            b'$' => {
                self.pos = start + 1;
                TokenKind::Dollar
            }
            // An ASCII-led name the front did not settle: a `u…`, or one within a chunk of the
            // end of input.
            _ if is_ascii_identifier_start(first) => {
                debug_assert!(first | 0x20 == b'u' || bytes.len() - start <= IDENT_CHUNK);
                return self.identifier_into(dst);
            }
            // Any other ASCII byte is not a valid token start — error. `first as char` is
            // the exact character at `start` (ASCII round-trips).
            _ if first < 0x80 => {
                return Err(lex_err(
                    format!("Unexpected character in CSS: '{}'", first as char),
                    start,
                ));
            }
            // Non-ASCII lead byte: decode the full char and dispatch. Every code point
            // ≥ U+00A0 opens an identifier — `parseCss`'s `read_identifier` takes it, and a
            // value is exactly where that reading is right. The boundary reading is the
            // parser's (`CssParser::skip_boundary_whitespace`), because only it knows
            // whether an `allow_whitespace()` would have run here first.
            _ => match self.current_char() {
                Some(ch) if is_identifier_start(ch) => return self.identifier_into(dst),
                Some(ch) if ch.is_whitespace() => {
                    self.pos = non_ascii_whitespace_run_end(self.source, start);
                    TokenKind::Whitespace
                }
                Some(ch) => {
                    return Err(lex_err(
                        format!("Unexpected character in CSS: '{ch}'"),
                        start,
                    ));
                }
                // Unreachable: the cursor is on a byte, so a char decodes here — and the
                // cursor has not moved, so this is the empty token at `start`.
                None => TokenKind::Eof,
            },
        };
        *dst = Token {
            kind,
            start: start as u32,
            end: self.pos as u32,
        };
        Ok(())
    }
}

/// [`is_ascii_css_whitespace`] as a 256-entry table, for [`ascii_whitespace_run_end`]'s walk:
/// one load and one compare a byte, where the six-member class spells a subtract, a range
/// compare and a second compare.
const ASCII_WHITESPACE_LUT: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = is_ascii_css_whitespace(i as u8);
        i += 1;
    }
    t
};

/// The end of the run of ASCII whitespace bytes ([`is_ascii_css_whitespace`]) from `from` on
/// — `from` itself when none is there.
#[inline]
fn ascii_whitespace_run_end(bytes: &[u8], from: usize) -> usize {
    let mut pos = from;
    while pos < bytes.len() && ASCII_WHITESPACE_LUT[bytes[pos] as usize] {
        pos += 1;
    }
    pos
}

/// Whether the `-` or `+` at `bytes[sign]` opens a number: a digit follows it, or a `.` and
/// then a digit.
#[inline]
fn sign_opens_number(bytes: &[u8], sign: usize) -> bool {
    match bytes.get(sign + 1) {
        Some(b) if b.is_ascii_digit() => true,
        Some(b'.') => bytes.get(sign + 2).is_some_and(u8::is_ascii_digit),
        _ => false,
    }
}

/// The end of a whitespace run from `pos` on, where the byte at `pos` is non-ASCII — the
/// run's head, or where the ASCII walk ([`ascii_whitespace_run_end`]) stopped.
///
/// The run's whole class, both halves: ASCII whitespace, and the non-ASCII whitespace that is
/// not an identifier code point. CSS whitespace is ASCII-only (CSS Syntax 3 §4.2), and tsv
/// follows `parseCss` in treating every code point ≥ U+00A0 (NBSP, em space, ideographic
/// space, …) as an *identifier* code point — value/ident content, never a separator, and
/// deliberately broader than the CSS Syntax ident set (which excludes these look-alike
/// whitespace chars). So a whitespace run stops at one; it lexes as identifier content
/// instead. Only the sub-U+00A0 non-ASCII whitespace (the C1 controls, e.g. NEL U+0085 — not
/// identifier code points here) stays whitespace.
///
/// ⚠️ That is the right class HERE and the wrong one at a **boundary**, where `parseCss` runs
/// `allow_whitespace()` (JS `\s`) before the token starts and would step over the very same
/// character. The lexer cannot tell the two apart — only the parser knows which juncture it
/// is at — so the boundary half lives in `CssParser::skip_boundary_whitespace`, called from
/// the parser's own whitespace skips. Keeping it out of the lexer is what leaves a
/// declaration VALUE alone, where a non-ASCII space is content
/// (`css/values/boundary_nonascii_space_prettier_divergence`).
///
/// A free function over the source rather than a `&mut self` method, so the call cannot write
/// the lexer as far as the optimizer knows; the caller stores the end.
#[cold]
#[inline(never)]
fn non_ascii_whitespace_run_end(source: &str, mut pos: usize) -> usize {
    let bytes = source.as_bytes();
    loop {
        match bytes.get(pos) {
            Some(&b) if is_ascii_css_whitespace(b) => pos += 1,
            Some(&b) if b.is_ascii() => return pos,
            Some(_) => match source[pos..].chars().next() {
                Some(ch) if ch.is_whitespace() && !is_non_ascii_identifier_codepoint(ch) => {
                    pos += ch.len_utf8();
                }
                _ => return pos,
            },
            None => return pos,
        }
    }
}

/// Whether `b` is an ASCII byte that `char::is_whitespace()` treats as whitespace:
/// `<TAB>` U+0009, `<LF>` U+000A, `<VT>` U+000B, `<FF>` U+000C, `<CR>` U+000D, and
/// `<SP>` U+0020 — the Unicode `White_Space` code points below U+0080. Deliberately
/// **not** `u8::is_ascii_whitespace()`, which omits `<VT>` (U+000B), a `White_Space`
/// code point.
///
/// Shared with the declaration value's boundary scan, which trims a value's span back to
/// its last non-whitespace token — a trim that is only exact if it means whitespace here.
#[inline]
pub(crate) const fn is_ascii_css_whitespace(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | 0x0B | 0x0C | b'\r' | b' ')
}

/// Whether the `url(` opening just past `(` at `after_paren` takes a **quoted**
/// argument — `url("…")` lexes as a function-token (ident + `(` + string), not a
/// url-token (css-syntax §4.3.6's fork). Skips Unicode whitespace (`char::is_whitespace`,
/// matching `parseCss`) before classifying the first content char. The single statement
/// of the fork, shared by `Lexer::consume_url_token` and `decl_scan::paren_open_kind`
/// (the second reader of this grammar).
pub(crate) fn url_arg_is_quoted(source: &str, after_paren: usize) -> bool {
    let mut i = after_paren;
    while let Some(ch) = source[i..].chars().next() {
        if ch.is_whitespace() {
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    matches!(source[i..].chars().next(), Some('"' | '\''))
}

/// End of the opaque url-token content opening just past `(` at `after_paren`: one past
/// the matching **unescaped** `)`, or `None` when the token runs to end-of-source
/// unterminated (the lexer takes it as-is; `decl_scan` declines). The two scan targets,
/// `\` and `)`, are ASCII, so neither can occur as a UTF-8 continuation byte — a
/// multi-byte code point's trailing bytes are all >= 0x80 and fall through the run.
pub(crate) fn url_token_close(bytes: &[u8], after_paren: usize) -> Option<usize> {
    let len = bytes.len();
    let mut j = after_paren;
    loop {
        while j < len && bytes[j] != b'\\' && bytes[j] != b')' {
            j += 1;
        }
        if j >= len {
            return None; // EOF before `)` — unterminated
        }
        if bytes[j] == b')' {
            return Some(j + 1);
        }
        // Escaped code point: the `\` and what it escapes are both content. Stepping
        // one byte past the `\` is enough — the escaped char's continuation bytes can
        // match neither target, so the run passes over them.
        j += 1;
        if j < len {
            j += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whitespace run's end from `pos` as one character loop over both halves of the
    /// class — the shape the lexer's split walk (the front's ASCII table walk, the
    /// dispatch's cold non-ASCII tail) must reproduce.
    fn reference_run_end(source: &str, mut pos: usize) -> usize {
        while let Some(ch) = source[pos..].chars().next() {
            let is_run = if ch.is_ascii() {
                is_ascii_css_whitespace(ch as u8)
            } else {
                ch.is_whitespace() && !is_non_ascii_identifier_codepoint(ch)
            };
            if !is_run {
                break;
            }
            pos += ch.len_utf8();
        }
        pos
    }

    /// One character for every byte a token boundary can meet: each ASCII byte, each
    /// two-byte `U+0080..=U+00FF` character, and a character opening with every other UTF-8
    /// lead byte — plus the non-ASCII whitespace and look-alike spaces the class decides on.
    fn followers() -> Vec<String> {
        let mut out: Vec<String> = (0u32..=0xFF)
            .map(|c| char::from_u32(c).expect("a Latin-1 code point").to_string())
            .collect();
        for lead in 0xC4u8..=0xF4 {
            let bytes: &[u8] = match lead {
                0xE0 => &[0xE0, 0xA0, 0x80],
                0xE1..=0xEF => &[lead, 0x80, 0x80],
                0xF0 => &[0xF0, 0x90, 0x80, 0x80],
                0xF1..=0xF4 => &[lead, 0x80, 0x80, 0x80],
                _ => &[lead, 0x80],
            };
            let bytes = bytes.to_vec();
            out.push(String::from_utf8(bytes).expect("a valid UTF-8 sequence"));
        }
        for c in [
            0x1680, 0x2000, 0x200A, 0x2028, 0x2029, 0x202F, 0x205F, 0x3000, 0xFEFF, 0x1F600,
        ] {
            out.push(char::from_u32(c).expect("a code point").to_string());
        }
        out
    }

    /// Every whitespace token ends where the class says it does: runs of every length over
    /// every member (the six ASCII bytes the lexer takes — SP, TAB, LF, CR, VT, FF — CRLF
    /// among them, and NEL, the non-ASCII member), opening at offset 0 and mid-document,
    /// against every byte that can follow them.
    #[test]
    fn whitespace_runs_end_where_the_class_ends() {
        let members = [" ", "\t", "\n", "\r", "\u{b}", "\u{c}", "\r\n", "\u{85}"];
        // and end of input, which ends a run as surely as a byte does
        let mut followers = followers();
        followers.push(String::new());
        let mut graded = 0;
        for len in 1..=9 {
            for rot in 0..members.len() {
                let run: String = (0..len)
                    .map(|i| members[(rot + i * 3) % members.len()])
                    .collect();
                for follower in &followers {
                    for prefix in ["", "a", "a{b:"] {
                        let tail = if follower.is_empty() { "" } else { "x" };
                        let source = format!("{prefix}{run}{follower}{tail}");
                        let mut lexer = Lexer::at_offset(&source, 0);
                        let token = loop {
                            let token = lexer.next_token().expect("the prefix lexes");
                            if token.start as usize >= prefix.len() {
                                break token;
                            }
                        };
                        assert_eq!(token.start as usize, prefix.len(), "{source:?}");
                        assert_eq!(token.kind, TokenKind::Whitespace, "{source:?}");
                        assert_eq!(
                            token.end as usize,
                            reference_run_end(&source, prefix.len()),
                            "{source:?}"
                        );
                        graded += 1;
                    }
                }
            }
        }
        assert!(graded > 60_000, "{graded}");
    }

    /// Every ASCII-led name ends where its run of continuation bytes does, whichever of the
    /// front's walks reaches it: the one chunk the front tests, the handoff that walks on past
    /// it a chunk at a time and then byte by byte, and the dispatch's reader for a `u`-led
    /// name or one within a chunk of the end of input. Names of 1 to 26 bytes — each
    /// continuation byte at one position per length, behind each lead byte class — opening a
    /// document and mid-declaration, each followed by a stop that ends it (any ASCII byte
    /// that neither continues a name nor escapes, and end of input) or that continues it (a
    /// non-ASCII code point, a hex escape), with and without text after the stop, so the
    /// name's end falls at every distance from the end of input.
    #[test]
    fn every_identifier_length_ends_where_its_run_does() {
        let class: Vec<u8> = (b'a'..=b'z')
            .chain(b'A'..=b'Z')
            .chain(b'0'..=b'9')
            .chain([b'-', b'_'])
            .collect();
        // (stop, how many bytes of it the name takes)
        let stops = [
            ("", 0),
            (" ", 0),
            (";", 0),
            (":", 0),
            ("(", 0),
            (")", 0),
            (",", 0),
            (".", 0),
            ("\u{7f}", 0),
            ("é", 2),
            ("\u{a0}", 2),
            ("\\72 ", 4),
            ("\\72;", 3),
        ];
        let mut graded = 0;
        for len in 1..=26usize {
            for lead in [b'x', b'u', b'U', b'-', b'_', b'Q'] {
                for (k, &c) in class.iter().enumerate() {
                    let mut name = vec![b'e'; len];
                    name[0] = lead;
                    if len > 1 {
                        name[1 + k % (len - 1)] = c;
                    }
                    // A `-` lead before a digit opens a number, and before a second `-` or a
                    // letter an identifier: keep the name an identifier.
                    if lead == b'-' && len > 1 && name[1].is_ascii_digit() {
                        name[1] = b'-';
                    }
                    let name = String::from_utf8(name).expect("ASCII");
                    if name.eq_ignore_ascii_case("url") {
                        continue;
                    }
                    for (stop, taken) in stops {
                        for prefix in ["", "a{b:"] {
                            for tail in ["", " x", " padding-past-a-chunk;"] {
                                if stop.is_empty() && !tail.is_empty() {
                                    continue;
                                }
                                let source = format!("{prefix}{name}{stop}{tail}");
                                let mut lexer = Lexer::at_offset(&source, 0);
                                let token = loop {
                                    let token = lexer.next_token().expect("the prefix lexes");
                                    if token.start as usize >= prefix.len() {
                                        break token;
                                    }
                                };
                                assert_eq!(token.kind, TokenKind::Identifier, "{source:?}");
                                assert_eq!(
                                    (token.start as usize, token.end as usize),
                                    (prefix.len(), prefix.len() + name.len() + taken),
                                    "{source:?}"
                                );
                                graded += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(graded > 100_000, "{graded}");
    }

    /// Every short name lexes to the token its decoded value calls for, whichever reader its
    /// spelling reaches — the front's common identifier, the dispatch's possible-`url` name,
    /// the cold escape and non-ASCII reader — so each of them decides `<url-token>` the same
    /// way. Names of one to four symbols over
    /// the letters of `url` in both cases, a letter that spells nothing, the two
    /// non-letter continuation bytes, an escaped `r` and a non-ASCII code point, each
    /// followed by every context that decides the url reading.
    ///
    /// The model is the lexer's url rule (css-syntax "consume an ident-like token"): the
    /// first token is a `<url-token>` iff the name's decoded value is `url` ASCII
    /// case-insensitively, a `(` follows it directly, and the argument, past any whitespace,
    /// does not open with a quote; the token then runs one past the first `)`, or to end of
    /// input. Otherwise it is the identifier, which ends with the name — and, when the name
    /// ends in the hex escape, with the one whitespace byte that terminates it.
    #[test]
    fn names_route_to_the_url_reading_their_decoded_value_calls_for() {
        // (source spelling, decoded value)
        let symbols = [
            ("u", "u"),
            ("U", "U"),
            ("r", "r"),
            ("R", "R"),
            ("l", "l"),
            ("L", "L"),
            ("x", "x"),
            ("-", "-"),
            ("_", "_"),
            ("\\72", "r"),
            ("é", "é"),
        ];
        // (follower, whether its `(` opens a quoted argument)
        let followers = [
            ("(", false),
            ("(\"", true),
            ("( '", true),
            ("(a) b", false),
            ("", false),
            (" ", false),
            ("\\", false),
        ];
        let mut graded = 0;
        for len in 1..=4u32 {
            for mut index in 0..symbols.len().pow(len) {
                let mut name = String::new();
                let mut decoded = String::new();
                let mut ends_in_hex_escape = false;
                for _ in 0..len {
                    let (spelling, value) = symbols[index % symbols.len()];
                    index /= symbols.len();
                    name.push_str(spelling);
                    decoded.push_str(value);
                    ends_in_hex_escape = spelling.starts_with('\\');
                }
                for (follower, quoted) in followers {
                    let source = format!("{name}{follower}");
                    let is_url =
                        decoded.eq_ignore_ascii_case("url") && follower.starts_with('(') && !quoted;
                    let (kind, end) = if is_url {
                        let close = source[name.len()..].find(')');
                        let end = close.map_or(source.len(), |close| name.len() + close + 1);
                        (TokenKind::Url, end)
                    } else {
                        let terminator = ends_in_hex_escape && follower.starts_with(' ');
                        (TokenKind::Identifier, name.len() + usize::from(terminator))
                    };
                    let token = Lexer::at_offset(&source, 0)
                        .next_token()
                        .unwrap_or_else(|err| panic!("{source:?}: {err:?}"));
                    assert_eq!(token.kind, kind, "{source:?}");
                    assert_eq!((token.start, token.end as usize), (0, end), "{source:?}");
                    graded += 1;
                }
            }
        }
        assert!(graded > 100_000, "{graded}");
    }
}
