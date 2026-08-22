// CSS parser - parse CSS content from <style> tags

mod atrules;
pub(crate) use atrules::is_keyframes_atrule;
mod attributes;
mod decl_scan;
mod declarations;
mod pseudo;
mod selectors;
mod value;

use crate::ast::internal::{Comment, CssNode, CssStyleSheet};
use crate::lexer::{Lexer, Token, TokenKind};
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use decl_scan::{TerminatorKind, ValueFacts};
use std::cell::Cell;
use tsv_lang::{ParseError, Span};

pub(crate) struct CssParser<'a, 'arena> {
    source: &'a str,
    lexer: Lexer<'a>,
    pub(crate) current_kind: TokenKind,
    pub(crate) current_start: usize,
    pub(crate) current_end: usize,
    /// Decoded value for the current token (only set for escaped identifiers),
    /// copied into the AST arena at receipt so it is a `Copy` `&'arena str` rather
    /// than an owned `String` — the lexer decodes into a reused scratch buffer and
    /// the parser arena-copies it immediately, so no per-identifier `String` survives.
    current_decoded: Option<&'arena str>,
    /// One-token lookahead. Holds the raw lexer token; the decoded value of an
    /// escaped peeked identifier stays **parked on the lexer scratch** (claimed at
    /// consume time in `advance`), so this slot carries no decode.
    peek: Option<Token>,
    base_offset: usize, // Offset in full source (when parsing embedded CSS)
    /// True while parsing inside functional pseudo-class arguments (`:is(...)`,
    /// `:not(...)`, an unknown `:foo(...)`), where a bare `<number>`/`<an+b>` token is
    /// an `Nth` simple selector — Svelte's `read_selector` gates its Nth production on
    /// `inside_pseudo_class` the same way, so top-level selectors keep rejecting bare
    /// numbers. Saved/restored around the two selector-list arg arms in `pseudo.rs`.
    pub(crate) in_pseudo_args: bool,
    pub(crate) comments: Vec<Comment>,
    /// Bump arena that owns every AST node this parser allocates. Supplied by
    /// the caller (caller-owns-`Bump`); the returned `CssStyleSheet<'arena>`
    /// borrows from it. `&'arena Bump` is `Copy`; nodes are gathered via
    /// `self.bvec()` and strings via `self.alloc_str_in()` (CSS has no single-node
    /// `alloc` — every node lands in a child slice).
    pub(crate) arena: &'arena Bump,
    /// Value facts computed speculatively by the rule/declaration disambiguation scan, for
    /// the `parse_declaration` that immediately follows to reuse instead of re-walking the
    /// value. Set (or cleared) at every `scan_rule_or_declaration`; keyed on the value's
    /// start offset so a stale entry — e.g. from a custom-property declaration that bypasses
    /// the scan — can never be mistaken for the current value's facts. `Cell` because the
    /// scan runs behind `&CssParser` (the disambiguation is a read-only lookahead).
    speculative_value_facts: Cell<Option<(usize, ValueFacts)>>,
}

impl<'a, 'arena> CssParser<'a, 'arena> {
    pub(crate) fn new(
        source: &'a str,
        base_offset: usize,
        arena: &'arena Bump,
    ) -> Result<Self, ParseError> {
        // The lexer scans `source` (the island) but reports errors against the document
        // it sits in, so it carries the same `base_offset` the spans below are shifted by.
        let mut lexer = Lexer::at_offset(source, base_offset);
        let token = lexer.next_token()?;
        let decoded = lexer
            .decoded_str()
            .map(|s| -> &'arena str { arena.alloc_str(s) });
        Ok(Self {
            source,
            lexer,
            current_kind: token.kind,
            current_start: token.start as usize,
            current_end: token.end as usize,
            current_decoded: decoded,
            peek: None,
            base_offset,
            in_pseudo_args: false,
            comments: Vec::new(),
            arena,
            speculative_value_facts: Cell::new(None),
        })
    }

    /// Record (or clear) the value facts the disambiguation scan produced for the
    /// declaration it just settled — see [`take_value_facts`](Self::take_value_facts).
    pub(in crate::parser) fn stash_value_facts(&self, facts: Option<(usize, ValueFacts)>) {
        self.speculative_value_facts.set(facts);
    }

    /// Take the stashed value facts, but only if they were computed for the value starting at
    /// `value_start` — the guard that makes reuse safe. Offsets increase monotonically
    /// through the parse, so a stale entry (a smaller start) never matches; a match means the
    /// disambiguation scan and this declaration are looking at the same value.
    pub(in crate::parser) fn take_value_facts(&self, value_start: usize) -> Option<ValueFacts> {
        match self.speculative_value_facts.take() {
            Some((start, facts)) if start == value_start => Some(facts),
            _ => None,
        }
    }

    /// Create an empty `BumpVec` whose backing buffer lives in the **arena** —
    /// the preferred way to gather children. Build it in the parse loop, then
    /// `.into_bump_slice()` to store the field (zero-copy: the buffer is already
    /// arena-owned). Carries its own `Copy` `&'arena Bump`, so pushing
    /// `parse_x(self)?` inside the loop does not borrow `self`.
    #[inline]
    pub(crate) fn bvec<T>(&self) -> BumpVec<'arena, T> {
        BumpVec::new_in(self.arena)
    }

    /// Copy a string (a decoded value or a verbatim source slice) into the
    /// arena. One copy into the arena; the returned `&'arena str` is stored
    /// inline on the AST node in place of an owned `String`.
    #[inline]
    pub(crate) fn alloc_str_in(&self, s: &str) -> &'arena str {
        self.arena.alloc_str(s)
    }

    /// Copy the lexer's just-produced decoded value (escaped identifiers only) into
    /// the AST arena, yielding a `Copy` `&'arena str`. The lexer decodes into a
    /// reused scratch that the next lex overwrites, so this runs immediately after
    /// each lex; `None` for the common escape-free token. One copy into the arena —
    /// the same copy the old owned-`String` path made, now the only allocation the
    /// parser retains on the escape path.
    #[inline]
    fn decoded_to_arena(&self) -> Option<&'arena str> {
        self.lexer
            .decoded_str()
            .map(|s| -> &'arena str { self.arena.alloc_str(s) })
    }

    /// Add a comment to the comments Vec
    pub(crate) fn add_comment(&mut self, comment: Comment) {
        self.comments.push(comment);
    }

    /// Build a `Comment` for the current block-comment token, delimiters excluded
    /// from `content_span`. Does not advance — callers decide whether to register it
    /// (`register_current_comment`) or consume and return it (`parse_block_comment`).
    fn build_current_comment(&self) -> Comment {
        debug_assert!(matches!(self.current_kind, TokenKind::Comment));
        // Content excludes the `/* */` delimiters; recovered on demand as a
        // source slice rather than copied.
        let multiline = Comment::content_is_multiline(
            true,
            &self.source[self.current_start + 2..self.current_end - 2],
        );
        Comment {
            content_span: Span {
                start: self.span_pos(self.current_start + 2),
                end: self.span_pos(self.current_end - 2),
            },
            is_block: true,
            multiline,
            span: Span {
                start: self.span_pos(self.current_start),
                end: self.span_pos(self.current_end),
            },
            emit_character_field: false,
            bump_pattern_columns: false,
            owned_by_node: false,
        }
    }

    /// Register the current token as a comment.
    /// Assumes current token is a Comment. Extracts content without `/* */` delimiters.
    pub(crate) fn register_current_comment(&mut self) {
        let comment = self.build_current_comment();
        self.add_comment(comment);
    }

    pub(crate) fn advance(&mut self) -> Result<(), ParseError> {
        // The token comes either from the lookahead slot (lexed during a prior
        // `peek_kind()`) or fresh from the lexer. In both cases the decoded escape
        // value of the most-recently-lexed token is parked on the lexer's scratch and
        // arena-copied below — for the peeked token nothing re-lexes between the peek
        // and this consume, so the scratch is still intact. Without this copy a
        // peeked-then-consumed escaped identifier would silently lose its decode and
        // fall back to the verbatim slice. Near-free: `decoded_str` is `None` for the
        // common no-escape token, so `decoded_to_arena` allocates nothing.
        let token = match self.peek.take() {
            Some(token) => token,
            None => self.lexer.next_token()?,
        };
        self.current_kind = token.kind;
        self.current_start = token.start as usize;
        self.current_end = token.end as usize;
        self.current_decoded = self.decoded_to_arena();
        Ok(())
    }

    /// Make a declaration value's terminator the current token, without lexing it.
    ///
    /// The counterpart to a scan that ran ahead of the parser: the value's boundary scan
    /// walks bytes to its terminator, and in stopping there it already established which
    /// token that is. Re-lexing the byte would only re-derive the kind and the extent that
    /// [`TerminatorKind`] already pins, so seat the token directly and leave the lexer just
    /// past it, exactly where `advance()` would have. Any lookahead is dropped: it was
    /// lexed from before the jump.
    ///
    /// Under debug the terminator is re-lexed and checked against the seated token, so the
    /// test suite re-proves that a construction still agrees with the lexer it replaces.
    pub(in crate::parser) fn seat_at_terminator(
        &mut self,
        terminator: usize,
        terminator_kind: TerminatorKind,
    ) {
        let (kind, width) = terminator_kind.token();
        let end = terminator + width;

        #[cfg(debug_assertions)]
        {
            let mut probe = Lexer::at_offset(self.source(), self.base_offset);
            probe.seek(terminator);
            let relexed = probe.next_token();
            assert!(
                relexed.as_ref().is_ok_and(|token| token.kind == kind
                    && token.start as usize == terminator
                    && token.end as usize == end),
                "seated terminator disagreed with the lexer at {terminator}: seated \
                 {kind:?} [{terminator}, {end}), lexer said {relexed:?}"
            );
        }

        self.peek = None;
        // `seek` also drops the lexer's parked decode — a `;` / `}` / EOF never carries one.
        self.lexer.seek(end);
        self.current_kind = kind;
        self.current_start = terminator;
        self.current_end = end;
        self.current_decoded = None;
    }

    /// Byte length of the boundary whitespace run at the head of the current token, or `0`.
    ///
    /// The non-consuming half of [`skip_boundary_whitespace`](Self::skip_boundary_whitespace),
    /// split out so that loop reads as "measure the run, then step it" rather than doing both
    /// in one expression. Private: the compound chain loop is the caller that would want the
    /// measurement alone, and it deliberately does not break on a run yet — see the note at
    /// that loop in `parser/selectors.rs`.
    fn boundary_run_len(&self) -> usize {
        if self.current_kind != TokenKind::Identifier {
            return 0;
        }
        self.source[self.current_start..self.current_end]
            .chars()
            .take_while(|c| crate::whitespace::is_boundary_only_whitespace(*c))
            .map(char::len_utf8)
            .sum()
    }

    /// Step over `parseCss`'s `allow_whitespace()` — whose class is JS `\s`
    /// ([`tsv_lang::is_js_whitespace`]) and therefore includes every code point at or above
    /// U+00A0 that the lexer has just read as the head of an identifier, alongside the
    /// ordinary whitespace the lexer does tokenize.
    ///
    /// The two readings are both right and only position separates them. `read_identifier`
    /// takes `<NBSP>`, every `Zs`, `<LS>`, `<PS>` and `<ZWNBSP>` as identifier content —
    /// correct inside a value (`css/values/boundary_nonascii_space_prettier_divergence` pins
    /// exactly that) and correct for a name glued to its `.` / `#` / `:` / `|` / `@` sigil,
    /// where Svelte calls `read_identifier` with no skip in front of it. At a **boundary** —
    /// a selector-list start, after a `,`, inside a `[`, after a combinator, before a
    /// declaration's property — the skip runs first and the same character is a separator.
    /// Only the parser knows which it is at, which is why this lives here and not in the
    /// lexer: putting the class there changed values and the BOM too.
    ///
    /// This is the **whole** `allow_whitespace()`, not the non-ASCII half: it skips ordinary
    /// whitespace tokens too, and loops, because a boundary run is one run however its
    /// members are spelled. `<NBSP><SP>` is a single `allow_whitespace()` to `parseCss`;
    /// stepping only the non-ASCII half leaves an ASCII gap standing where a name is due,
    /// which moved every offset captured after it and made `[<NBSP> a]` and `a ><NBSP> b`
    /// parse **errors** on input canonical accepts. So callers use this in place of
    /// [`skip_whitespace`](Self::skip_whitespace) at a boundary rather than beside it.
    ///
    /// ⚠️ It stops **on** a comment, exactly where the plain skip does. That is what keeps
    /// the disposition of a comment with its juncture — the stylesheet body registers one,
    /// a block pushes it as a child, a selector list has its own rule — and it is why the
    /// call belongs at the top of those loops rather than after their comment arm: a run
    /// stepped later lands the cursor on a comment the arm has already been passed, where
    /// the "skip unexpected token" tail silently eats it.
    ///
    /// ⚠️ Deliberately blind to `<NEL>` (U+0085), which is `White_Space` to Rust and **not**
    /// JS `\s`. The lexer still reads it as whitespace, so tsv accepts a selector Svelte
    /// rejects; correcting that means giving a declaration's property and value their own raw
    /// readers, since `<NEL>` is content there. Tracked with that family — see
    /// [`tests/css_boundary_whitespace.rs`](../../../../tests/css_boundary_whitespace.rs).
    pub(in crate::parser) fn skip_boundary_whitespace(&mut self) -> Result<(), ParseError> {
        loop {
            self.skip_whitespace()?;
            let run = self.boundary_run_len();
            if run == 0 {
                return Ok(());
            }
            let at = self.current_start + run;
            // Any lookahead was lexed from past this token and is void once the cursor moves.
            self.peek = None;
            if at != self.current_end {
                // A name follows the run inside this same token: re-read from past it.
                let token = self.lexer.token_at(at)?;
                self.current_kind = token.kind;
                self.current_start = token.start as usize;
                self.current_end = token.end as usize;
                self.current_decoded = self.decoded_to_arena();
                return Ok(());
            }
            // The whole identifier was the run — there is no name here at all. Consume it
            // and ask again from the top: the two classes can alternate any number of times
            // inside one run, and the loop's own leading skip is what steps the ASCII half.
            self.lexer.seek(at);
            self.advance()?;
        }
    }

    /// Peek at the next token's kind without consuming it. Returns the kind by
    /// value (`TokenKind` is `Copy`) — like `tsv_ts`'s `peek_kind`, not a borrow of
    /// `self`. Result is cached so repeated peeks are efficient. (Named `peek_kind`,
    /// not `peek`, to match `tsv_ts` and avoid shadowing the `peek` field.)
    pub(crate) fn peek_kind(&mut self) -> Result<TokenKind, ParseError> {
        if let Some(token) = &self.peek {
            return Ok(token.kind);
        }
        let token = self.lexer.next_token()?;
        let kind = token.kind;
        self.peek = Some(token);
        Ok(kind)
    }

    /// Peek past whitespace and comments to find the next significant token.
    /// This creates a temporary lexer to look ahead without modifying parser state.
    /// Used for disambiguating declarations vs nested rules.
    pub(crate) fn peek_past_whitespace(&self) -> Result<TokenKind, ParseError> {
        self.peek_past(true)
    }

    /// Peek past a run of `/* */` comments — and **only** comments — to the next
    /// token's kind, without consuming anything.
    ///
    /// The comments-only twin of [`Self::peek_past_whitespace`], for the selector
    /// positions where a comment is inter-token trivia but a `<whitespace-token>` is
    /// forbidden by the grammar: the components of a `<wq-name>` (`svg/* c */|rect`) and
    /// of an `<attr-matcher>` (`[attr~/* c */='value']`). Skipping whitespace here would
    /// widen the accepted grammar, not just the trivia.
    ///
    /// Answers from the cached [`Self::peek_kind`] slot whenever the very next token is
    /// not a comment, which is every type selector in a real stylesheet: the temp-lexer
    /// walk below caches nothing, so taking it unconditionally would re-lex that token
    /// again at the following `advance()`.
    pub(crate) fn peek_past_comments(&mut self) -> Result<TokenKind, ParseError> {
        let next = self.peek_kind()?;
        if !matches!(next, TokenKind::Comment) {
            return Ok(next);
        }
        self.peek_past(false)
    }

    /// The lookahead both `peek_past_*` spell: a temporary lexer from the current token's
    /// end, so parser state (including the `peek` slot) is untouched.
    fn peek_past(&self, whitespace: bool) -> Result<TokenKind, ParseError> {
        let remaining = &self.source()[self.current_end..];
        let mut temp_lexer = Lexer::at_offset(remaining, self.base_offset + self.current_end);
        loop {
            let token = temp_lexer.next_token()?;
            match &token.kind {
                TokenKind::Comment => continue,
                TokenKind::Whitespace if whitespace => continue,
                _ => return Ok(token.kind),
            }
        }
    }

    /// Advance past a run of `/* */` comments — and **only** comments — registering each
    /// into `self.comments`.
    ///
    /// The consuming counterpart of [`Self::peek_past_comments`], with the same reason for
    /// leaving whitespace alone. Registration (rather than
    /// [`Self::skip_whitespace_and_comments`]'s drop) is what lets the printer re-emit the
    /// comment at its authored position.
    pub(crate) fn register_and_skip_comments(&mut self) -> Result<(), ParseError> {
        while matches!(&self.current_kind, TokenKind::Comment) {
            self.register_current_comment();
            self.advance()?;
        }
        Ok(())
    }

    pub(crate) fn check(&self, kind: TokenKind) -> bool {
        self.current_kind == kind
    }

    /// True at the end of an at-rule prelude: a block `{`, a statement `;`, or EOF.
    /// The shared stop condition for the prelude-consuming loops.
    pub(crate) fn at_prelude_end(&self) -> bool {
        matches!(
            self.current_kind,
            TokenKind::LeftBrace | TokenKind::Semicolon | TokenKind::Eof
        )
    }

    pub(crate) fn expect(&mut self, kind: TokenKind) -> Result<(), ParseError> {
        if !self.check(kind) {
            return Err(self.error_expected_found(&kind.to_string()));
        }
        self.advance()
    }

    /// Expect a token and capture its end position before advancing.
    /// Used for nodes whose span should end at the delimiter token.
    pub(crate) fn expect_and_capture(&mut self, kind: TokenKind) -> Result<u32, ParseError> {
        if !self.check(kind) {
            return Err(self.error_expected_found(&kind.to_string()));
        }
        let end = self.span_pos(self.current_end);
        self.advance()?;
        Ok(end)
    }

    pub(crate) fn skip_whitespace(&mut self) -> Result<(), ParseError> {
        while self.check(TokenKind::Whitespace) {
            self.advance()?;
        }
        Ok(())
    }

    /// Skip whitespace and comments, **dropping** the comments.
    ///
    /// A comment skipped here never reaches `self.comments`, so the printer's
    /// `comments_to_emit_in_range` lookups cannot reconstruct it — in any gap the printer
    /// rebuilds from the AST (rather than emitting verbatim source), that is
    /// silent content loss. Use `skip_whitespace_registering_comments` in those
    /// positions; this variant is only safe where the skipped range is re-emitted
    /// verbatim or comments are recovered by other means (e.g. the declaration
    /// property→colon gap, reconstructed by the svelte-compat property split).
    ///
    /// Returns whether any comment was skipped — the declaration property→colon
    /// gap uses this to fold into `CssDeclaration::has_block_comment` without a
    /// re-scan.
    pub(crate) fn skip_whitespace_and_comments(&mut self) -> Result<bool, ParseError> {
        let mut saw_comment = false;
        loop {
            if self.check(TokenKind::Whitespace) {
                self.advance()?;
            } else if matches!(&self.current_kind, TokenKind::Comment) {
                saw_comment = true;
                self.advance()?;
            } else {
                break;
            }
        }
        Ok(saw_comment)
    }

    /// Skip whitespace and **register** any comments encountered into `self.comments`.
    ///
    /// Used by structured preludes (e.g. `@import`) where comments are valid between
    /// the parsed tokens and must survive for the printer to reconstruct, even though
    /// they're stripped from the public-AST prelude string (matching Svelte). Unlike
    /// `skip_whitespace_and_comments`, this preserves the comments rather than dropping
    /// them.
    ///
    /// Returns whether a `<whitespace-token>` was among what it skipped. The attribute
    /// selector's name→`|` gap is the caller that needs it: the same gap is
    /// spacing-safe when the `|` turns out to open an `<attr-matcher>` (`[attr |= 'v']`)
    /// and whitespace-forbidden when it separates a `<wq-name>` (`[svg |attr]`), and
    /// which it is isn't known until the token *after* the `|` is in hand.
    pub(crate) fn skip_whitespace_registering_comments(&mut self) -> Result<bool, ParseError> {
        let mut saw_whitespace = false;
        loop {
            if self.check(TokenKind::Whitespace) {
                saw_whitespace = true;
                self.advance()?;
            } else if matches!(&self.current_kind, TokenKind::Comment) {
                self.register_current_comment();
                self.advance()?;
            } else {
                break;
            }
        }
        Ok(saw_whitespace)
    }

    /// Abandon a speculative parse: reposition at `pos` (a `source`-relative byte
    /// offset) and re-lex from there, discarding every comment registered since
    /// `comments_len`.
    ///
    /// Both halves are the operation — a rewind that keeps the registrations would
    /// leave phantom entries in `self.comments` for a region the caller is about to
    /// re-read, and the printer's `comments_to_emit_in_range` lookups are keyed on
    /// source position, not on which attempt produced them, so a comment inside the
    /// abandoned region would print twice. Take the snapshot with
    /// `self.comments.len()` **before** the trial call.
    ///
    /// Clearing `peek` is likewise not optional: a lookahead lexed from the
    /// abandoned position would otherwise be consumed by the next `advance`.
    pub(in crate::parser) fn rewind_to(
        &mut self,
        pos: usize,
        comments_len: usize,
    ) -> Result<(), ParseError> {
        self.comments.truncate(comments_len);
        self.peek = None;
        self.lexer.seek(pos);
        self.advance()
    }

    /// Skip a run of legacy HTML-comment markers `<!-- ... -->` (CDO/CDC) at a
    /// stylesheet statement or selector-list boundary, mirroring Svelte `parseCss`'s
    /// `allow_comment_or_whitespace`. The whole span — **including any CSS between the
    /// markers** — is discarded (no AST node), diverging from the CSS Syntax spec
    /// (where `<!--`/`-->` are independent no-op tokens and content between them parses
    /// as ordinary CSS); tsv matches `parseCss`. See `../../docs/conformance_svelte.md`
    /// §CSS Compat Behaviors.
    ///
    /// Recognized only where the current token begins `<!--`, so a bare `<` (a
    /// container-query range operator) is untouched, and `<!--`/`-->` in value or
    /// at-rule-prelude position stay raw text — those readers scan raw and never call
    /// this, so a `;`/`{` between the markers there stays significant, matching
    /// `parseCss`. Unterminated (`-->` missing) is an error, like Svelte's
    /// `eat('-->', true)`.
    ///
    /// Skips leading whitespace itself, so it is a self-sufficient drop-in at any
    /// boundary (the `<!--`-preceding whitespace need not already be consumed) — and it
    /// skips it through [`skip_boundary_whitespace`](Self::skip_boundary_whitespace),
    /// because every call site of this is an `allow_comment_or_whitespace` juncture, which
    /// is where the two whitespace classes are one class. Does **not** handle `/* */`
    /// comments — their disposition (register vs. push as a block child) is context-specific
    /// and stays with each call site, which is precisely why the boundary run has to be
    /// stepped *here*, before that site's comment arm, rather than after it.
    pub(crate) fn skip_html_comment_markers(&mut self) -> Result<(), ParseError> {
        self.skip_boundary_whitespace()?;
        while self.check(TokenKind::LessThan)
            && self.source[self.current_start..].starts_with("<!--")
        {
            // Scan raw for the required `-->` terminator — trivia-unaware, exactly like
            // Svelte's `read_until(/-->/)`: a `-->` inside a string/comment between the
            // markers still ends the span. ASCII, so a plain byte scan is boundary-safe.
            let bytes = self.source.as_bytes();
            let mut i = self.current_start + 4; // past `<!--`
            let after = loop {
                if bytes[i..].starts_with(b"-->") {
                    break i + 3;
                }
                if i >= bytes.len() {
                    return Err(self.error_msg("Unterminated HTML comment"));
                }
                i += 1;
            };
            self.peek = None; // any lookahead was lexed from before the marker
            self.lexer.seek(after);
            self.advance()?;
            self.skip_boundary_whitespace()?;
        }
        Ok(())
    }

    /// Get the current token's value from source (for most tokens)
    #[inline]
    pub(crate) fn current_value(&self) -> &str {
        &self.source[self.current_start..self.current_end]
    }

    /// Get the current identifier's resolved text.
    ///
    /// Returns the decoded value when the identifier contained escapes, otherwise
    /// the verbatim source slice (the no-escape common case, where the lexer keeps
    /// `current_decoded` `None` to avoid an allocation). Only meaningful when the
    /// current token is an `Identifier`; for other tokens it returns the raw token
    /// slice, so callers gate on the kind first (as they already did).
    #[inline]
    pub(crate) fn current_identifier(&self) -> &str {
        self.current_decoded.unwrap_or_else(|| self.current_value())
    }

    /// The current identifier's resolved text as an `&'arena str`, for callers that
    /// store it as an owned node field. When the identifier was escaped the decoded
    /// value already lives in the arena (`current_decoded`), so it is returned
    /// directly — no second copy; otherwise the verbatim source slice is copied in.
    /// Prefer over `alloc_str_in(current_identifier())`, which re-copies the decoded
    /// value on the escape path.
    #[inline]
    pub(crate) fn current_identifier_in_arena(&self) -> &'arena str {
        self.current_decoded
            .unwrap_or_else(|| self.arena.alloc_str(self.current_value()))
    }

    pub(crate) fn current_start(&self) -> usize {
        self.current_start
    }

    pub(crate) fn base_offset(&self) -> usize {
        self.base_offset
    }

    pub(crate) fn source(&self) -> &'a str {
        self.source
    }

    /// Get current position (base_offset + current_start)
    #[inline]
    pub(crate) fn current_pos(&self) -> usize {
        self.base_offset + self.current_start
    }

    /// Convert a raw `source`-relative offset into an absolute `Span` coordinate:
    /// `base_offset`-shifted and narrowed to `u32`. Raw offsets index `self.source`
    /// (e.g. `current_start`, a captured scan position); `Span` fields store the
    /// shifted `u32`. Centralizes the `(base_offset + pos) as u32` boundary cast.
    #[inline]
    pub(crate) fn span_pos(&self, raw: usize) -> u32 {
        (self.base_offset + raw) as u32
    }

    /// Parse the current comment token into a `Comment` and advance past it.
    /// Caller must verify `current_kind` is `TokenKind::Comment` before calling.
    pub(crate) fn parse_block_comment(&mut self) -> Result<Comment, ParseError> {
        let comment = self.build_current_comment();
        self.advance()?;
        self.skip_whitespace()?;
        Ok(comment)
    }

    // Error Helpers

    /// Create an error with custom message at current position
    pub(crate) fn error_msg(&self, message: &str) -> ParseError {
        ParseError::invalid_syntax(message.to_string(), self.current_pos())
    }

    /// Create an error with custom message at custom position
    pub(crate) fn error_msg_at(&self, message: &str, position: usize) -> ParseError {
        ParseError::invalid_syntax(message.to_string(), position)
    }

    /// Create an error: "Expected X"
    pub(crate) fn error_expected(&self, what: &str) -> ParseError {
        ParseError::invalid_syntax(format!("Expected {what}"), self.current_pos())
    }

    /// Create an error: "Expected X" at custom position
    pub(crate) fn error_expected_at(&self, what: &str, position: usize) -> ParseError {
        ParseError::invalid_syntax(format!("Expected {what}"), position)
    }

    /// Create an error: "Expected X, found Y"
    pub(crate) fn error_expected_found(&self, what: &str) -> ParseError {
        let kind = &self.current_kind;
        ParseError::invalid_syntax(format!("Expected {what}, found {kind}"), self.current_pos())
    }

    /// Create an error: "Expected X after 'Y'"
    pub(crate) fn error_expected_after(&self, what: &str, after: &str) -> ParseError {
        ParseError::invalid_syntax(
            format!("Expected {what} after '{after}'"),
            self.current_pos(),
        )
    }

    /// Create an error: "Unexpected X"
    pub(crate) fn error_unexpected(&self, what: &str) -> ParseError {
        ParseError::invalid_syntax(format!("Unexpected {what}"), self.current_pos())
    }

    pub(crate) fn parse(&mut self) -> Result<CssStyleSheet<'arena>, ParseError> {
        let mut nodes = self.bvec();

        // The stylesheet body is a `read_body` boundary: whitespace, `/*` comments,
        // and legacy `<!-- ... -->` markers may separate items. `skip_html_comment_markers`
        // covers whitespace + markers; `/*` comments are registered inside the loop.
        self.skip_html_comment_markers()?;

        while !self.check(TokenKind::Eof) {
            // Handle comments at top level - add to comments Vec
            if matches!(&self.current_kind, TokenKind::Comment) {
                self.register_current_comment();
                self.advance()?;
                self.skip_html_comment_markers()?;
                continue;
            }

            // Handle at-rules (@media, @keyframes, etc.)
            if self.check(TokenKind::AtSign) {
                // Top-level at-rules are not nested in rules
                let atrule = atrules::parse_atrule(self, false)?;
                nodes.push(CssNode::Atrule(atrule));
                self.skip_html_comment_markers()?;
                continue;
            }

            // Parse rules (selector { declarations })
            let node = declarations::parse_rule(self, false)?;
            nodes.push(CssNode::Rule(node));

            self.skip_html_comment_markers()?;
        }

        // Comments are already sorted by span.start since we add them in order during parsing

        Ok(CssStyleSheet {
            nodes: nodes.into_bump_slice(),
            comments: std::mem::take(&mut self.comments),
        })
    }
}

/// Parse CSS source into AST nodes
/// base_offset is the position of the CSS source in a larger file (for embedded CSS)
pub fn parse_css<'arena>(
    source: &str,
    base_offset: usize,
    arena: &'arena Bump,
) -> Result<CssStyleSheet<'arena>, ParseError> {
    let mut parser = CssParser::new(source, base_offset, arena)?;
    parser.parse()
}
