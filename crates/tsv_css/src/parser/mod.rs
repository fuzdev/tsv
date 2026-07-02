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

    /// Register the current token as a comment.
    /// Assumes current token is a Comment. Extracts content without `/* */` delimiters.
    pub(crate) fn register_current_comment(&mut self) {
        debug_assert!(matches!(self.current_kind, TokenKind::Comment));
        let comment_start = self.base_offset + self.current_start;
        let comment_end = self.base_offset + self.current_end;
        // Content excludes the `/* */` delimiters; recovered on demand as a
        // source slice rather than copied.
        let multiline = self.source[self.current_start + 2..self.current_end - 2].contains('\n');
        self.add_comment(Comment {
            content_span: Span {
                start: (comment_start + 2) as u32,
                end: (comment_end - 2) as u32,
            },
            is_block: true,
            multiline,
            span: Span {
                start: comment_start as u32,
                end: comment_end as u32,
            },
            emit_character_field: false,
        });
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
        if self.peek.is_none() {
            self.peek = Some(self.lexer.next_token()?);
        }
        // peek is guaranteed Some after the if block above
        match &self.peek {
            Some(token) => Ok(token.kind),
            #[allow(clippy::unreachable)] // peek was set Some immediately above
            None => unreachable!("peek was just populated"),
        }
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
        let end = (self.base_offset + self.current_end) as u32;
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
    pub(crate) fn skip_whitespace_and_comments(&mut self) -> Result<(), ParseError> {
        loop {
            if self.check(TokenKind::Whitespace) || matches!(&self.current_kind, TokenKind::Comment)
            {
                self.advance()?;
            } else {
                break;
            }
        }
        Ok(())
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

    /// Parse the current comment token into a `Comment` and advance past it.
    /// Caller must verify `current_kind` is `TokenKind::Comment` before calling.
    pub(crate) fn parse_block_comment(&mut self) -> Result<Comment, ParseError> {
        let comment_start = self.base_offset + self.current_start;
        let comment_end = self.base_offset + self.current_end;
        let multiline = self.source[self.current_start + 2..self.current_end - 2].contains('\n');
        self.advance()?;
        self.skip_whitespace()?;
        Ok(Comment {
            content_span: Span {
                start: (comment_start + 2) as u32,
                end: (comment_end - 2) as u32,
            },
            is_block: true,
            multiline,
            span: Span {
                start: comment_start as u32,
                end: comment_end as u32,
            },
            emit_character_field: false,
        })
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

        self.skip_whitespace()?;

        while !self.check(TokenKind::Eof) {
            // Handle comments at top level - add to comments Vec
            if matches!(&self.current_kind, TokenKind::Comment) {
                self.register_current_comment();
                self.advance()?;
                self.skip_whitespace()?;
                continue;
            }

            // Handle at-rules (@media, @keyframes, etc.)
            if self.check(TokenKind::AtSign) {
                // Top-level at-rules are not nested in rules
                let atrule = atrules::parse_atrule(self, false)?;
                nodes.push(CssNode::Atrule(atrule));
                self.skip_whitespace()?;
                continue;
            }

            // Parse rules (selector { declarations })
            let node = declarations::parse_rule(self, false)?;
            nodes.push(CssNode::Rule(node));

            self.skip_whitespace()?;
        }

        // Comments are already sorted by span.start since we add them in order during parsing

        // Build line breaks table for O(log n) line boundary lookups
        // Must add base_offset to each position since AST spans use global positions
        let base_offset_u32 = self.base_offset as u32;
        let line_breaks: Vec<u32> = tsv_lang::printing::build_line_breaks(self.source)
            .into_iter()
            .map(|pos| pos + base_offset_u32)
            .collect();

        Ok(CssStyleSheet {
            nodes: nodes.into_bump_slice(),
            comments: std::mem::take(&mut self.comments),
            line_breaks,
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
