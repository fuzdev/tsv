// TypeScript parser - main entry point and coordination
//
// TODO(parser): Expand TypeScript support to unblock comment tests
// Currently blocking comment tests:
// - around_braces, mixed_positions: Need object literal support `{ prop: value }`
// - More generally: Need full expression parsing (arrays, objects, etc.)
// See TODO_COMMENTS.md "Issue 3" for impact on comment tests.

use crate::ast::internal::*;
use crate::lexer::{Lexer, TokenKind};
use std::cell::RefCell;
use std::rc::Rc;
use string_interner::{DefaultStringInterner, DefaultSymbol};
use tsv_lang::{ParseError, PeekData, Span};

// Import parsing implementations
mod expression;
mod statement;

pub struct Parser<'a> {
    source: &'a str,
    lexer: Lexer<'a>,
    current_kind: TokenKind,
    current_start: usize,
    current_end: usize,
    current_decoded: Option<String>, // Decoded string value (for strings with escapes)
    peek_cache: Option<PeekData<TokenKind>>,
    interner: Rc<RefCell<DefaultStringInterner>>,
    base_offset: usize,     // Offset in full source (for embedded expressions)
    comments: Vec<Comment>, // Collected comments during parsing
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Result<Self, ParseError> {
        Self::with_interner(
            source,
            0,
            Rc::new(RefCell::new(DefaultStringInterner::new())),
        )
    }

    /// Create a parser with shared interner and base offset.
    ///
    /// Used when parsing embedded expressions/scripts in Svelte templates.
    /// base_offset is added to all span positions to get correct positions in full source.
    pub fn with_interner(
        source: &'a str,
        base_offset: usize,
        interner: Rc<RefCell<DefaultStringInterner>>,
    ) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(source);
        // Extract token data immediately to avoid keeping token alive
        let (mut kind, mut start, mut end, mut decoded) = {
            let token = lexer.next_token()?;
            (token.kind, token.start, token.end, token.decoded)
        };

        // Collect leading comment tokens
        let mut comments = Vec::new();
        while let TokenKind::Comment { content, is_block } = &kind {
            comments.push(Comment {
                content: content.clone(),
                is_block: *is_block,
                span: Span::new((start + base_offset) as u32, (end + base_offset) as u32),
            });
            let token = lexer.next_token()?;
            kind = token.kind;
            start = token.start;
            end = token.end;
            decoded = token.decoded;
        }

        Ok(Self {
            source,
            lexer,
            current_kind: kind,
            current_start: start,
            current_end: end,
            current_decoded: decoded,
            peek_cache: None,
            interner,
            base_offset,
            comments,
        })
    }

    pub(super) fn advance(&mut self) -> Result<(), ParseError> {
        // Get next token (from peek cache or lexer)
        if let Some(peek) = self.peek_cache.take() {
            self.current_kind = peek.kind;
            self.current_start = peek.start;
            self.current_end = peek.end;
            self.current_decoded = peek.decoded;
        } else {
            let token = self.lexer.next_token()?;
            self.current_kind = token.kind;
            self.current_start = token.start;
            self.current_end = token.end;
            self.current_decoded = token.decoded;
        }

        // Collect comment tokens into comments Vec
        while let TokenKind::Comment { content, is_block } = &self.current_kind {
            self.comments.push(Comment {
                content: content.clone(),
                is_block: *is_block,
                span: Span::new(
                    (self.current_start + self.base_offset) as u32,
                    (self.current_end + self.base_offset) as u32,
                ),
            });
            let token = self.lexer.next_token()?;
            self.current_kind = token.kind;
            self.current_start = token.start;
            self.current_end = token.end;
            self.current_decoded = token.decoded;
        }

        Ok(())
    }

    pub(super) fn intern(&self, s: &str) -> DefaultSymbol {
        self.interner.borrow_mut().get_or_intern(s)
    }

    // Helper methods for extract-then-advance pattern

    pub(super) fn current_kind(&self) -> TokenKind {
        self.current_kind.clone()
    }

    pub(super) fn current_pos(&self) -> (usize, usize) {
        (
            self.current_start + self.base_offset,
            self.current_end + self.base_offset,
        )
    }

    pub(super) fn current_value(&self) -> &str {
        &self.source[self.current_start..self.current_end]
    }

    /// Get the decoded string value for the current token (for strings with escapes)
    ///
    /// Currently unused (printer preserves escapes as-is instead of decoding).
    /// This will be needed in the future for:
    /// - Expression evaluation (computing const values)
    /// - Type analysis (analyzing string literal types)
    /// - Linting (analyzing string content for patterns)
    ///
    /// Keep this method for future tooling that needs runtime string values.
    #[allow(dead_code)]
    pub(super) fn current_decoded(&self) -> Option<&str> {
        self.current_decoded.as_deref()
    }

    pub(super) fn check(&self, kind: &TokenKind) -> bool {
        &self.current_kind == kind
    }

    // Peek helpers for lookahead (needed for type annotations, operators, etc.)
    // Lazily computes peek token on first access
    pub(super) fn peek_kind(&mut self) -> TokenKind {
        if self.peek_cache.is_none()
            && let Ok(token) = self.lexer.next_token()
        {
            self.peek_cache = Some(PeekData::with_decoded(
                token.kind,
                token.start,
                token.end,
                token.decoded,
            ));
        }
        self.peek_cache
            .as_ref()
            .map(|p| p.kind.clone())
            .unwrap_or(TokenKind::Eof)
    }

    #[expect(dead_code, reason = "Convenience wrapper for peek_kind() == kind")]
    pub(super) fn peek_check(&mut self, kind: &TokenKind) -> bool {
        &self.peek_kind() == kind
    }

    pub(super) fn expect(&mut self, kind: &TokenKind) -> Result<(), ParseError> {
        if self.check(kind) {
            self.advance()
        } else {
            Err(ParseError::UnexpectedToken {
                expected: kind.to_string(),
                found: self.current_kind.to_string(),
                position: self.current_start,
                context: None,
            })
        }
    }

    /// Consume a token if it matches the given kind (optional token consumption)
    ///
    /// Returns `true` if the token was consumed, `false` otherwise.
    ///
    /// Useful for optional syntax elements like:
    /// - Trailing commas: `[1, 2, 3,]` - eat(Comma) at end
    /// - Optional semicolons in some contexts
    /// - Optional type annotations: eat(Colon) to check presence
    ///
    /// # Example
    /// ```ignore
    /// let has_init = if self.eat(TokenKind::Equals) {
    ///     Some(self.parse_expression()?)
    /// } else {
    ///     None
    /// };
    /// ```
    pub(super) fn eat(&mut self, kind: TokenKind) -> bool {
        if self.check(&kind) {
            self.advance().is_ok()
        } else {
            false
        }
    }

    /// Handle list separator (comma) and terminator in list parsing
    ///
    /// Consolidates comma/terminator handling across:
    /// - Object properties: `{ a: 1, b: 2 }`
    /// - Array elements: `[1, 2, 3]`
    /// - Function parameters: `fn(a, b, c)`
    /// - Type parameters: `Array<T, U>`
    ///
    /// Returns:
    /// - `Ok(true)` if more elements expected (found separator, not at terminator)
    /// - `Ok(false)` if list ended (found terminator or trailing separator)
    /// - `Err(ParseError)` if neither separator nor terminator found
    ///
    /// Handles trailing separators uniformly: `[1, 2,]` is valid
    ///
    /// # Example
    /// ```ignore
    /// loop {
    ///     properties.push(self.parse_property()?);
    ///     if !self.expect_list_separator(&TokenKind::Comma, &TokenKind::BraceClose)? {
    ///         break;
    ///     }
    /// }
    /// ```
    pub(super) fn expect_list_separator(
        &mut self,
        separator: &TokenKind,
        terminator: &TokenKind,
    ) -> Result<bool, ParseError> {
        if self.check(separator) {
            self.advance()?;
            if self.check(terminator) {
                Ok(false) // Trailing separator, end of list
            } else {
                Ok(true) // More elements expected
            }
        } else if self.check(terminator) {
            Ok(false) // End of list
        } else {
            Err(ParseError::InvalidSyntax {
                message: format!(
                    "Expected '{}' or '{}' after list element, found {}",
                    separator, terminator, self.current_kind
                ),
                position: self.current_pos().0,
                context: None,
            })
        }
    }

    pub fn parse(&mut self) -> Result<Program, ParseError> {
        let start = self.base_offset; // Start at base_offset for embedded contexts
        let mut body = Vec::new();

        while self.current_kind != TokenKind::Eof {
            body.push(self.parse_statement()?);
        }

        // Use current_pos() to get global position (includes base_offset)
        let (_, end) = self.current_pos();

        Ok(Program {
            body,
            comments: std::mem::take(&mut self.comments),
            span: Span::new(start as u32, end as u32),
            interner: Rc::clone(&self.interner),
        })
    }

    /// Parse a single expression (used by Svelte for expression tags)
    pub fn parse_expression_public(&mut self) -> Result<Expression, ParseError> {
        self.parse_expression()
    }
}

/// Parse TypeScript source code into an AST.
pub fn parse_typescript(source: &str) -> Result<Program, ParseError> {
    let mut parser = Parser::new(source)?;
    parser.parse()
}
