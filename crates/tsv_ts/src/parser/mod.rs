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
    /// True if a line terminator occurred between the previous token and current token.
    /// Used for ASI (Automatic Semicolon Insertion).
    had_line_terminator: bool,
    /// End position of the previous token (before current). Used for span calculation
    /// when ASI inserts a semicolon.
    prev_end: usize,
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
            had_line_terminator: false, // No line terminator before first token
            prev_end: 0,
        })
    }

    pub(super) fn advance(&mut self) -> Result<(), ParseError> {
        // Save previous token's end position for ASI span calculation
        self.prev_end = self.current_end;

        // Get next token (from peek cache or lexer)
        if let Some(peek) = self.peek_cache.take() {
            self.current_kind = peek.kind;
            self.current_start = peek.start;
            self.current_end = peek.end;
            self.current_decoded = peek.decoded;
            // Peek doesn't preserve line terminator info, so check lexer state
            // Note: This is slightly imprecise for peek, but peek is rare
            self.had_line_terminator = self.lexer.had_line_terminator();
        } else {
            let token = self.lexer.next_token()?;
            self.current_kind = token.kind;
            self.current_start = token.start;
            self.current_end = token.end;
            self.current_decoded = token.decoded;
            self.had_line_terminator = self.lexer.had_line_terminator();
        }

        // Collect comment tokens into comments Vec
        while let TokenKind::Comment { content, is_block } = &self.current_kind {
            // ECMAScript spec: if a MultiLineComment contains one or more line terminators,
            // then it is replaced by a single line terminator for ASI purposes.
            // So block comments with newlines should set had_line_terminator.
            if *is_block && content.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
                self.had_line_terminator = true;
            }

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
            // Also check line terminator in whitespace after comment
            if self.lexer.had_line_terminator() {
                self.had_line_terminator = true;
            }
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

    /// Update current token state from a new token (for template continuation)
    pub(super) fn update_current(&mut self, token: crate::lexer::Token) {
        self.current_kind = token.kind;
        self.current_start = token.start;
        self.current_end = token.end;
        self.current_decoded = token.decoded;
    }

    pub(super) fn current_pos(&self) -> (usize, usize) {
        (
            self.current_start + self.base_offset,
            self.current_end + self.base_offset,
        )
    }

    /// Get the raw end position (without base_offset) for lexer operations
    pub(super) fn current_raw_end(&self) -> usize {
        self.current_end
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
            .map_or(TokenKind::Eof, |p| p.kind.clone())
    }

    #[expect(dead_code, reason = "Convenience wrapper for peek_kind() == kind")]
    pub(super) fn peek_check(&mut self, kind: &TokenKind) -> bool {
        &self.peek_kind() == kind
    }

    /// Check if peek token is an identifier (used for contextual keyword disambiguation)
    pub(super) fn peek_is_identifier(&mut self) -> bool {
        matches!(self.peek_kind(), TokenKind::Identifier)
    }

    /// Check if peek token could be a property name (identifier, keyword, string, or computed key)
    ///
    /// Used to detect getter/setter syntax where `get` and `set` are contextual keywords:
    /// - `{ get x() {} }` - getter (peek is `x` = identifier)
    /// - `{ get [expr]() {} }` - computed getter (peek is `[`)
    /// - `{ get }` - shorthand property (peek is `}`, not a property name, so NOT a getter)
    pub(super) fn peek_is_property_name(&mut self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Identifier
                | TokenKind::BracketOpen
                | TokenKind::String
                | TokenKind::Keyword(_)
        )
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

    /// Check if a semicolon can be inserted at the current position (ASI).
    ///
    /// Returns true if:
    /// - Current token is EOF, OR
    /// - Current token is `}`, OR
    /// - A line terminator occurred between the previous token and current token
    ///
    /// This is the core ASI detection per ECMAScript spec section 12.9.
    pub(super) fn can_insert_semicolon(&self) -> bool {
        matches!(self.current_kind, TokenKind::Eof | TokenKind::BraceClose)
            || self.had_line_terminator
    }

    /// Consume a semicolon, or accept if ASI allows one.
    ///
    /// This is the main ASI entry point for statement termination.
    /// Use this instead of `expect(&TokenKind::Semicolon)` for statement-ending semicolons.
    ///
    /// Returns Ok(()) if:
    /// - A semicolon token was consumed, OR
    /// - ASI conditions allow implicit semicolon insertion
    ///
    /// Returns Err if neither explicit semicolon nor ASI conditions are met.
    pub(super) fn semicolon(&mut self) -> Result<(), ParseError> {
        if self.eat(TokenKind::Semicolon) {
            return Ok(());
        }
        if self.can_insert_semicolon() {
            return Ok(());
        }
        Err(ParseError::InvalidSyntax {
            message: "Expected ';'".to_string(),
            position: self.current_pos().0,
            context: None,
        })
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

    /// Parse a parenthesized parameter list: `(a, b, c)`
    ///
    /// Used by function declarations, method shorthand, and arrow functions.
    /// Consumes both opening and closing parentheses.
    pub(super) fn parse_parameter_list(&mut self) -> Result<Vec<Expression>, ParseError> {
        self.expect(&TokenKind::ParenOpen)?;

        let mut params = Vec::new();
        if !self.check(&TokenKind::ParenClose) {
            loop {
                // Parse parameter: identifier, array pattern, or object pattern
                let param = match self.current_kind() {
                    TokenKind::Identifier => {
                        // Simple identifier parameter
                        let (param_start, param_end) = self.current_pos();
                        let symbol = self.intern(self.current_value());
                        self.advance()?;
                        let mut param = Expression::Identifier(Identifier {
                            name: symbol,
                            type_annotation: None,
                            span: Span::new(param_start as u32, param_end as u32),
                        });
                        // Check for default value: param = default
                        if self.check(&TokenKind::Equals) {
                            self.advance()?; // consume '='
                            let default_value = self.parse_assignment_expression()?;
                            let assign_end = default_value.span().end;
                            param = Expression::AssignmentPattern(AssignmentPattern {
                                left: Box::new(param),
                                right: Box::new(default_value),
                                span: Span::new(param_start as u32, assign_end),
                            });
                        }
                        param
                    }
                    TokenKind::BracketOpen => {
                        // Array destructuring pattern: [a, b]
                        let expr = self.parse_array_expression()?;
                        let pattern = self.to_assignable(expr)?;
                        // Check for default value
                        if self.check(&TokenKind::Equals) {
                            let pattern_start = pattern.span().start;
                            self.advance()?; // consume '='
                            let default_value = self.parse_assignment_expression()?;
                            let assign_end = default_value.span().end;
                            Expression::AssignmentPattern(AssignmentPattern {
                                left: Box::new(pattern),
                                right: Box::new(default_value),
                                span: Span::new(pattern_start, assign_end),
                            })
                        } else {
                            pattern
                        }
                    }
                    TokenKind::BraceOpen => {
                        // Object destructuring pattern: {a, b}
                        let expr = self.parse_object_expression()?;
                        let pattern = self.to_assignable(expr)?;
                        // Check for default value
                        if self.check(&TokenKind::Equals) {
                            let pattern_start = pattern.span().start;
                            self.advance()?; // consume '='
                            let default_value = self.parse_assignment_expression()?;
                            let assign_end = default_value.span().end;
                            Expression::AssignmentPattern(AssignmentPattern {
                                left: Box::new(pattern),
                                right: Box::new(default_value),
                                span: Span::new(pattern_start, assign_end),
                            })
                        } else {
                            pattern
                        }
                    }
                    _ => {
                        return Err(ParseError::InvalidSyntax {
                            message: format!(
                                "Expected parameter name or destructuring pattern, found {}",
                                self.current_kind()
                            ),
                            position: self.current_pos().0,
                            context: None,
                        });
                    }
                };

                params.push(param);

                // Check for comma or closing paren
                if !self.expect_list_separator(&TokenKind::Comma, &TokenKind::ParenClose)? {
                    break;
                }
            }
        }

        self.expect(&TokenKind::ParenClose)?;
        Ok(params)
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

    /// Parse a single assignment expression and return position where parsing stopped.
    ///
    /// Unlike `parse_expression_public()`, this stops at top-level commas.
    /// This is useful for parsing expressions embedded in contexts where commas
    /// have other meanings (like `{#each items as pattern, index}`).
    ///
    /// Returns (expression, end_position) where end_position is where the next
    /// unparsed content begins (in absolute source coordinates with base_offset).
    pub fn parse_assignment_expression_partial(&mut self) -> Result<(Expression, usize), ParseError> {
        let expr = self.parse_assignment_expression()?;
        // Return the start of the current (unconsumed) token
        let next_pos = self.current_start + self.base_offset;
        Ok((expr, next_pos))
    }
}

/// Parse TypeScript source code into an AST.
pub fn parse_typescript(source: &str) -> Result<Program, ParseError> {
    let mut parser = Parser::new(source)?;
    parser.parse()
}
