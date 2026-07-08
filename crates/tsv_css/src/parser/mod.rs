// CSS parser - parse CSS content from <style> tags

mod atrules;
pub(crate) use atrules::is_keyframes_atrule;
mod attributes;
mod declarations;
mod pseudo;
mod selectors;
mod value;

use crate::ast::internal::{Comment, CssNode, CssStyleSheet};
use crate::lexer::{Lexer, Token, TokenKind};
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use tsv_lang::{ParseError, Span};

pub(crate) struct CssParser<'a, 'arena> {
    source: &'a str,
    lexer: Lexer<'a>,
    pub(crate) current_kind: TokenKind,
    pub(crate) current_start: usize,
    pub(crate) current_end: usize,
    current_decoded: Option<String>, // Decoded value for current token (only set for escaped identifiers)
    /// One-token lookahead. Holds the raw lexer token; the decoded value of an
    /// escaped peeked identifier stays **parked on the lexer** (claimed at consume
    /// time in `advance`), so this slot carries no `String`.
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
}

impl<'a, 'arena> CssParser<'a, 'arena> {
    pub(crate) fn new(
        source: &'a str,
        base_offset: usize,
        arena: &'arena Bump,
    ) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(source);
        let token = lexer.next_token()?;
        let decoded = lexer.take_decoded().map(|b| *b);
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
        })
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
        let multiline = self.source[self.current_start + 2..self.current_end - 2].contains('\n');
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
        // value of the most-recently-lexed token is parked on the lexer and claimed
        // below — for the peeked token nothing re-lexes between the peek and this
        // consume, so it's still parked. Without this claim a peeked-then-consumed
        // escaped identifier would silently lose its decode and fall back to the
        // verbatim slice. Near-free: `take_decoded` is `None` for the common
        // no-escape token.
        let token = match self.peek.take() {
            Some(token) => token,
            None => self.lexer.next_token()?,
        };
        self.current_kind = token.kind;
        self.current_start = token.start as usize;
        self.current_end = token.end as usize;
        self.current_decoded = self.lexer.take_decoded().map(|b| *b);
        Ok(())
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
        // Create a temporary lexer from current position
        let remaining = &self.source()[self.current_end..];
        let mut temp_lexer = Lexer::new(remaining);

        // Skip whitespace and comments
        loop {
            let token = temp_lexer.next_token()?;
            match &token.kind {
                TokenKind::Whitespace | TokenKind::Comment => continue,
                _ => return Ok(token.kind),
            }
        }
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
    /// `comments_in_range` lookups cannot reconstruct it — in any gap the printer
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
    pub(crate) fn skip_whitespace_registering_comments(&mut self) -> Result<(), ParseError> {
        loop {
            if self.check(TokenKind::Whitespace) {
                self.advance()?;
            } else if matches!(&self.current_kind, TokenKind::Comment) {
                self.register_current_comment();
                self.advance()?;
            } else {
                break;
            }
        }
        Ok(())
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
    /// boundary (the `<!--`-preceding whitespace need not already be consumed). Does
    /// **not** handle `/* */` comments — their disposition (register vs. push as a
    /// block child) is context-specific and stays with each call site.
    pub(crate) fn skip_html_comment_markers(&mut self) -> Result<(), ParseError> {
        self.skip_whitespace()?;
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
            self.skip_whitespace()?;
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
        self.current_decoded
            .as_deref()
            .unwrap_or_else(|| self.current_value())
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
        ParseError::InvalidSyntax {
            message: message.to_string(),
            position: self.current_pos(),
            context: None,
        }
    }

    /// Create an error with custom message at custom position
    pub(crate) fn error_msg_at(&self, message: &str, position: usize) -> ParseError {
        ParseError::InvalidSyntax {
            message: message.to_string(),
            position,
            context: None,
        }
    }

    /// Create an error: "Expected X"
    pub(crate) fn error_expected(&self, what: &str) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Expected {what}"),
            position: self.current_pos(),
            context: None,
        }
    }

    /// Create an error: "Expected X" at custom position
    pub(crate) fn error_expected_at(&self, what: &str, position: usize) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Expected {what}"),
            position,
            context: None,
        }
    }

    /// Create an error: "Expected X, found Y"
    pub(crate) fn error_expected_found(&self, what: &str) -> ParseError {
        let kind = &self.current_kind;
        ParseError::InvalidSyntax {
            message: format!("Expected {what}, found {kind}"),
            position: self.current_pos(),
            context: None,
        }
    }

    /// Create an error: "Expected X after 'Y'"
    pub(crate) fn error_expected_after(&self, what: &str, after: &str) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Expected {what} after '{after}'"),
            position: self.current_pos(),
            context: None,
        }
    }

    /// Create an error: "Unexpected X"
    pub(crate) fn error_unexpected(&self, what: &str) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Unexpected {what}"),
            position: self.current_pos(),
            context: None,
        }
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
