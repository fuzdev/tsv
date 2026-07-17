// SvelteParser struct and helper methods

use crate::ast::internal::FragmentNode;
use crate::lexer::{Lexer, Token, TokenKind};
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use std::cell::RefCell;
use std::rc::Rc;
use string_interner::{DefaultStringInterner, DefaultSymbol};
use tsv_lang::{Comment, ParseError, SharedInterner, Span};
use tsv_ts::Expression;

/// Build an expression `Comment` from its already-shifted `span` / `content_span`.
/// `content` is the comment body, read only to compute the `multiline` flag (whether
/// it holds a line terminator). Centralizes the `Comment` shape built by the
/// live-lexer (`try_read_js_comment`) path.
fn expression_comment(
    span: Span,
    content_span: Span,
    is_block: bool,
    content: &str,
    emit_character_field: bool,
) -> Comment {
    Comment {
        content_span,
        is_block,
        multiline: content.contains('\n'),
        span,
        emit_character_field,
        bump_pattern_columns: false,
        owned_by_node: false,
    }
}

pub(crate) struct SvelteParser<'a, 'arena> {
    /// Bump arena that owns every AST node this parser allocates — the template
    /// AST and (via the embedding APIs that receive `&'arena Bump`) the embedded
    /// TS `<script>`/`{expr}` ASTs. Supplied by the caller; the returned
    /// `Root<'arena>` borrows from it. `&'arena Bump` is `Copy`, so `self.alloc(owned)`
    /// and `self.arena.alloc(self.parse_x()?)` (even while `&mut self` is held — the
    /// field read borrows the `Bump`, not `self`) both work directly; lift it into a
    /// local (`let arena = self.arena;`) only when several allocations in one method
    /// share it.
    pub(crate) arena: &'arena Bump,
    pub(crate) source: &'a str, // Full original source
    pub(crate) lexer: Lexer<'a>,
    pub(crate) current_kind: TokenKind,
    pub(crate) current_start: usize, // Global position in full source
    pub(crate) current_end: usize,   // Global position in full source
    /// One-token lookahead. Holds the raw lexer token (positions are
    /// **slice-relative** — `base_offset` is added when it's consumed, exactly
    /// as for a freshly lexed token); cleared whenever the lexer is re-seeked.
    pub(crate) peek: Option<Token>,
    pub(crate) interner: SharedInterner,
    pub(crate) base_offset: usize, // Offset of lexer's source in full source
    /// TS comments collected from template expressions (e.g., {@debug /* comment */ a})
    pub(crate) expression_comments: Vec<Comment>,
    /// True while the nearest *element* ancestor is `<svelte:head>` — mirrors Svelte's
    /// `parent_is_head` (`1-parse/state/element.js`): set entering a head's children, reset by a
    /// nested RegularElement/Component, transparent through other special elements and blocks.
    /// Gates `<title>` → `TitleElement`. Saved/restored around each element's children.
    pub(crate) in_svelte_head: bool,
    /// True while inside a `<template shadowrootmode>` — mirrors Svelte's
    /// `parent_is_shadowroot_template` (any ancestor RegularElement carrying a `shadowrootmode`
    /// attribute). Monotonic within a subtree (descendants inherit) but scoped to the template
    /// (restored for siblings). Suppresses `<slot>` → `SlotElement` (it stays a `RegularElement`).
    pub(crate) in_shadowroot_template: bool,
}

impl<'a, 'arena> SvelteParser<'a, 'arena> {
    pub(crate) fn new(source: &'a str, arena: &'arena Bump) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(source);
        // Extract token data immediately to avoid keeping token alive
        let (kind, start, end) = {
            let token = lexer.next_token()?;
            (token.kind, token.start as usize, token.end as usize)
        };
        // The Svelte parser owns the single interner shared with every embedded
        // `<script>`/`{expr}`. Its tenants are element/attribute names plus the
        // rare escaped TS identifier — tens of short strings, not the
        // per-identifier population the retired source-proportional pre-size was
        // tuned for. A small fixed capacity covers a typical component's distinct
        // names in one up-front allocation instead of ~9 from-empty growth steps.
        let interner = Rc::new(RefCell::new(DefaultStringInterner::with_capacity(32)));
        Ok(Self {
            arena,
            source,
            lexer,
            current_kind: kind,
            current_start: start,
            current_end: end,
            peek: None,
            interner,
            base_offset: 0,
            expression_comments: Vec::new(),
            in_svelte_head: false,
            in_shadowroot_template: false,
        })
    }

    /// Allocate a single AST node in the arena, returning a shared `&'arena`
    /// reference (replaces `Box::new`). Zero-copy: `Bump::alloc` moves the value
    /// into arena memory.
    #[inline]
    pub(crate) fn alloc<T>(&self, val: T) -> &'arena T {
        self.arena.alloc(val)
    }

    /// A growable vector that builds AST-node collections **directly in the
    /// arena**. Build it in the parse loop, then `.into_bump_slice()` to store
    /// the field (zero-copy). Carries its own `Copy` `&'arena Bump`, so pushing
    /// into it inside a `&mut self` method does NOT borrow `self`.
    #[inline]
    pub(crate) fn bvec<T>(&self) -> BumpVec<'arena, T> {
        BumpVec::new_in(self.arena)
    }

    /// Allocate a string (raw or decoded) in the arena as `&'arena str` — used
    /// for the Svelte directive names / modifiers / raw parameter text that were
    /// owned `String`s pre-arena. One copy into the arena.
    #[inline]
    pub(crate) fn alloc_str_in(&self, s: &str) -> &'arena str {
        self.arena.alloc_str(s)
    }

    /// Returns the lexer's initial position (after BOM skip).
    /// Used by parser to initialize gap tracking.
    pub(crate) fn initial_position(&self) -> usize {
        self.lexer.initial_position()
    }

    pub(crate) fn advance(&mut self) -> Result<(), ParseError> {
        let token = match self.peek.take() {
            Some(token) => token,
            None => self.lexer.next_token()?,
        };
        self.current_kind = token.kind;
        self.current_start = self.base_offset + token.start as usize;
        self.current_end = self.base_offset + token.end as usize;
        Ok(())
    }

    pub(crate) fn intern(&self, s: &str) -> DefaultSymbol {
        self.interner.borrow_mut().get_or_intern(s)
    }

    pub(crate) fn current_pos(&self) -> (usize, usize) {
        (self.current_start, self.current_end)
    }

    /// The current token's verbatim source text. Returns `&'a str` (borrowing the
    /// immutable source), not `&self` — so callers can hold it across `advance()`
    /// (and other `&mut self` calls) without a borrow-escape `.to_string()`.
    pub(crate) fn current_value(&self) -> &'a str {
        // current_start/end are global, so use them directly
        &self.source[self.current_start..self.current_end]
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

    /// Check if the next tag matches the given name (e.g., "script", "style")
    /// Returns true if we're at `<tagname`, false otherwise
    /// Does not allocate - compares directly against source
    pub(crate) fn is_next_tag(&mut self, tag_name: &str) -> Result<bool, ParseError> {
        if !self.check(TokenKind::LeftAngle) {
            return Ok(false);
        }

        // Peek at next token
        if self.peek.is_none() {
            self.peek = Some(self.lexer.next_token()?);
        }

        if let Some(peek) = &self.peek
            && peek.kind == TokenKind::Identifier
        {
            // Compare directly without allocating (peek positions are
            // slice-relative, so shift by base_offset to index the full source).
            let value = &self.source
                [self.base_offset + peek.start as usize..self.base_offset + peek.end as usize];
            return Ok(value == tag_name);
        }

        Ok(false)
    }

    /// Peek at the next token to check if it matches the given kind
    /// Does not consume current token or advance parser
    /// Returns true if next token matches kind, false otherwise
    pub(crate) fn is_next_token(&mut self, kind: TokenKind) -> Result<bool, ParseError> {
        // Populate peek cache if not already cached
        if self.peek.is_none() {
            self.peek = Some(self.lexer.next_token()?);
        }

        Ok(self.peek.as_ref().is_some_and(|p| p.kind == kind))
    }

    /// Parse a text node if there's a gap between the last position and current position.
    /// The Svelte lexer skips whitespace, so gaps represent text/whitespace content.
    pub(crate) fn capture_text_if_gap(
        &self,
        last_end: usize,
        nodes: &mut BumpVec<'arena, FragmentNode<'arena>>,
    ) -> Result<(), ParseError> {
        if self.current_start > last_end {
            let text = self.parse_text(last_end, self.current_start)?;
            nodes.push(FragmentNode::Text(text));
        }
        Ok(())
    }

    /// Advance the lexer to a specific position in the source.
    /// Used when we've manually scanned ahead (e.g., for {@attach} parsing).
    /// Preserves the current `inside_tag` state for correct tokenization.
    pub(crate) fn advance_to_position(&mut self, pos: usize) -> Result<(), ParseError> {
        // Save the inside_tag state before creating new lexer
        let was_inside_tag = self.lexer.inside_tag;

        // Reset the lexer to start from the new position. Positions are reported
        // relative to the slice; the parser shifts them by base_offset.
        self.lexer = Lexer::new(&self.source[pos..]);
        self.base_offset = pos;
        self.peek = None;

        // Restore inside_tag state
        self.lexer.inside_tag = was_inside_tag;

        // Get the next token at the new position
        let token = self.lexer.next_token()?;
        self.current_kind = token.kind;
        self.current_start = self.base_offset + token.start as usize;
        self.current_end = self.base_offset + token.end as usize;

        Ok(())
    }

    /// Try to read a JS-style comment (`//` or `/* */`) at the current position.
    ///
    /// Called when the current token is `Slash`, to check whether the slash begins
    /// a comment rather than a self-closing `/>`. If a comment is found, it is pushed
    /// to `expression_comments` and the lexer is advanced past the comment.
    ///
    /// Returns `true` if a comment was consumed, `false` if it's a regular slash.
    pub(crate) fn try_read_js_comment(&mut self) -> Result<bool, ParseError> {
        let pos = self.current_start;
        let bytes = self.source.as_bytes();

        if pos + 1 >= bytes.len() {
            return Ok(false);
        }

        match bytes[pos + 1] {
            b'/' => {
                // Line comment: // ... up to \n
                let content_start = pos + 2;
                let mut end = content_start;
                while end < bytes.len() && bytes[end] != b'\n' {
                    end += 1;
                }

                let content = &self.source[content_start..end];
                self.expression_comments.push(expression_comment(
                    Span::new(pos as u32, end as u32),
                    Span::new(content_start as u32, end as u32),
                    false,
                    content,
                    true,
                ));

                self.advance_to_position(end)?;
                Ok(true)
            }
            b'*' => {
                // Block comment: /* ... */
                let content_start = pos + 2;
                let mut end = content_start;
                while end + 1 < bytes.len() {
                    if bytes[end] == b'*' && bytes[end + 1] == b'/' {
                        break;
                    }
                    end += 1;
                }

                if end + 1 >= bytes.len() {
                    return Err(self.error_unclosed_at("block comment", pos));
                }

                let content = &self.source[content_start..end];
                let comment_end = end + 2; // past */
                self.expression_comments.push(expression_comment(
                    Span::new(pos as u32, comment_end as u32),
                    Span::new(content_start as u32, end as u32),
                    true,
                    content,
                    true,
                ));

                self.advance_to_position(comment_end)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    // Error Construction Helpers
    // Note: No #[inline] - error paths are cold paths, inlining would just bloat code size

    /// Create error with custom message at current position
    pub(crate) fn error_msg(&self, message: &str) -> ParseError {
        ParseError::InvalidSyntax {
            message: message.to_string(),
            position: self.current_start,
            context: None,
        }
    }

    /// Create error with custom message at specified position
    pub(crate) fn error_msg_at(&self, message: &str, position: usize) -> ParseError {
        ParseError::InvalidSyntax {
            message: message.to_string(),
            position,
            context: None,
        }
    }

    /// Create "Expected X" error at current position
    pub(crate) fn error_expected(&self, what: &str) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Expected {what}"),
            position: self.current_start,
            context: None,
        }
    }

    /// Create "Expected X" error at specified position
    pub(crate) fn error_expected_at(&self, what: &str, position: usize) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Expected {what}"),
            position,
            context: None,
        }
    }

    /// Create "Expected X, found Y" error at current position
    pub(crate) fn error_expected_found(&self, what: &str) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Expected {what}, found {}", self.current_kind),
            position: self.current_start,
            context: None,
        }
    }

    /// Create "Unclosed X" error at specified position
    pub(crate) fn error_unclosed_at(&self, what: &str, position: usize) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Unclosed {what}"),
            position,
            context: None,
        }
    }

    /// Create "Duplicate X found" error at current position
    pub(crate) fn error_duplicate(&self, what: &str) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Duplicate {what} found"),
            position: self.current_start,
            context: None,
        }
    }

    /// Create "Unknown X: Y" error at specified position
    pub(crate) fn error_unknown_at(&self, kind: &str, value: &str, position: usize) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Unknown {kind}: {value}"),
            position,
            context: None,
        }
    }

    // TypeScript Expression Parsing Helpers
    // These helpers wrap tsv_ts parsing functions and automatically collect comments.

    /// Parse a TypeScript expression and collect any comments.
    ///
    /// Comments are added to `self.expression_comments` for later inclusion in `Root.comments`.
    pub(crate) fn parse_ts_expression(
        &mut self,
        source: &str,
        base_offset: usize,
    ) -> Result<Expression<'arena>, ParseError> {
        let (expr, comments) = tsv_ts::parse_expression_with_comments(
            source,
            base_offset,
            Rc::clone(&self.interner),
            self.arena,
        )?;
        self.expression_comments.extend(comments);
        Ok(expr)
    }

    /// Parse a partial TypeScript expression (stops at top-level identifiers like `as`).
    ///
    /// Comments are collected.
    pub(crate) fn parse_ts_expression_partial(
        &mut self,
        source: &str,
        base_offset: usize,
    ) -> Result<(Expression<'arena>, usize), ParseError> {
        let (expr, end_pos, comments) = tsv_ts::parse_expression_partial_with_comments(
            source,
            base_offset,
            Rc::clone(&self.interner),
            self.arena,
        )?;
        self.expression_comments.extend(comments);
        Ok((expr, end_pos))
    }

    /// Parse a TypeScript pattern (destructuring) and collect any comments.
    /// Also handles optional type annotations (`: Type`) after the pattern.
    pub(crate) fn parse_ts_pattern(
        &mut self,
        source: &str,
        base_offset: usize,
    ) -> Result<Expression<'arena>, ParseError> {
        let (pattern, comments) = tsv_ts::parse_pattern_with_comments(
            source,
            base_offset,
            Rc::clone(&self.interner),
            self.arena,
        )?;
        // Canonical reads a destructure via a synthetic `(pattern = 1)` acorn
        // parse whose inserted `(` shifts the pattern's start line one column
        // right when that line is `> 1` — the same quirk the pattern nodes get
        // (`adjust_read_pattern_columns`) also lands on comments collected on
        // that line, and the wire serializes them with the shifted columns.
        let pattern_on_first_line = !self.source[..base_offset].contains('\n');
        self.expression_comments
            .extend(comments.into_iter().map(|mut c| {
                if !pattern_on_first_line
                    && !self.source[base_offset..c.span.start as usize].contains('\n')
                {
                    c.bump_pattern_columns = true;
                }
                c
            }));
        Ok(pattern)
    }

    /// Parse a TypeScript statement (the body of a `{const}`/`{let}` tag is a
    /// `VariableDeclaration`) and collect any comments.
    pub(crate) fn parse_ts_statement(
        &mut self,
        source: &str,
        base_offset: usize,
    ) -> Result<tsv_ts::Statement<'arena>, ParseError> {
        let (stmt, comments) = tsv_ts::parse_statement_with_comments(
            source,
            base_offset,
            Rc::clone(&self.interner),
            self.arena,
        )?;
        self.expression_comments.extend(comments);
        Ok(stmt)
    }
}
