// TypeScript parser - main entry point and coordination
//
// TODO(parser): Expand TypeScript support to unblock comment tests
// Currently blocking comment tests:
// - around_braces, mixed_positions: Need object literal support `{ prop: value }`
// - More generally: Need full expression parsing (arrays, objects, etc.)
// See TODO_COMMENTS.md "Issue 3" for impact on comment tests.

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, Lexer, TokenKind};
use std::cell::RefCell;
use std::rc::Rc;
use string_interner::{DefaultStringInterner, DefaultSymbol};
use tsv_lang::{ParseError, PeekData, Span};

// Import parsing implementations
mod expression;
mod statement; // Statement parsing (refactored into submodules)

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
    /// Whether to parse TypeScript `as`/`satisfies` operators.
    /// Disabled in partial expression parsing for Svelte template contexts
    /// where `as` has different meaning (e.g., `{#each items as pattern}`).
    allow_ts_type_assertions: bool,
    /// True when parsing inside `declare namespace` or `declare module`.
    /// Functions inside ambient contexts don't have bodies (end with `;`).
    in_ambient_context: bool,
    /// Stored lexer error from peek_kind(). Returned on next advance() call.
    /// This ensures lexer errors propagate even when peek swallows them.
    lexer_error: Option<ParseError>,
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
            allow_ts_type_assertions: true, // Enable by default (TypeScript context)
            in_ambient_context: false,      // Not in declare namespace/module
            lexer_error: None,              // No stored lexer error
        })
    }

    pub(super) fn advance(&mut self) -> Result<(), ParseError> {
        // Check for stored lexer error from peek_kind() - propagate it now
        if let Some(err) = self.lexer_error.take() {
            return Err(err);
        }
        self.advance_inner()
    }

    /// Advance without checking stored error first. Used by try_advance().
    fn advance_inner(&mut self) -> Result<(), ParseError> {
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

    /// Try to advance, storing any error for later instead of returning it.
    /// Returns true on success, false on error (with error stored in lexer_error).
    /// Used by eat() and eat_contextual_keyword() which return bool.
    fn try_advance(&mut self) -> bool {
        match self.advance_inner() {
            Ok(()) => true,
            Err(err) => {
                self.lexer_error = Some(err);
                false
            }
        }
    }

    pub(super) fn intern(&self, s: &str) -> DefaultSymbol {
        self.interner.borrow_mut().get_or_intern(s)
    }

    // Helper methods for extract-then-advance pattern

    #[inline]
    pub(super) fn current_kind(&self) -> &TokenKind {
        &self.current_kind
    }

    /// Update current token state from a new token (for template continuation)
    pub(super) fn update_current(&mut self, token: crate::lexer::Token) {
        self.current_kind = token.kind;
        self.current_start = token.start;
        self.current_end = token.end;
        self.current_decoded = token.decoded;
    }

    #[inline]
    pub(super) fn current_pos(&self) -> (usize, usize) {
        (
            self.current_start + self.base_offset,
            self.current_end + self.base_offset,
        )
    }

    /// Get the end position of the previously consumed token (with base_offset).
    ///
    /// Useful for determining where statements end after consuming optional tokens
    /// like semicolons (via ASI or explicit).
    #[inline]
    pub(super) fn prev_token_end(&self) -> usize {
        self.prev_end + self.base_offset
    }

    /// Get the raw end position (without base_offset) for lexer operations
    pub(super) fn current_raw_end(&self) -> usize {
        self.current_end
    }

    #[inline]
    pub(super) fn current_value(&self) -> &str {
        &self.source[self.current_start..self.current_end]
    }

    /// Get the decoded string value for the current token (for strings with escapes)
    ///
    /// Used for:
    /// - Identifiers with unicode escapes (\u0066oo → "foo")
    /// - Expression evaluation (computing const values)
    /// - Type analysis (analyzing string literal types)
    /// - Linting (analyzing string content for patterns)
    pub(super) fn current_decoded(&self) -> Option<&str> {
        self.current_decoded.as_deref()
    }

    /// Get the identifier name from the current token.
    ///
    /// For identifiers with unicode escapes, returns the decoded name.
    /// For regular identifiers, returns the raw source text.
    ///
    /// Example: `\u0066oo` returns "foo", `bar` returns "bar"
    pub(super) fn current_identifier_name(&self) -> &str {
        self.current_decoded
            .as_deref()
            .unwrap_or_else(|| self.current_value())
    }

    /// Intern the current identifier, using decoded name if available.
    ///
    /// This is the canonical way to intern identifiers. For identifiers with
    /// unicode escapes (e.g., `\u0066oo`), returns the decoded symbol (`foo`).
    /// For regular identifiers, returns the raw source text.
    ///
    /// Use this instead of `self.intern(self.current_value())` for all identifier
    /// interning to ensure escaped identifiers are handled correctly.
    pub(super) fn intern_identifier(&self) -> DefaultSymbol {
        self.intern(self.current_identifier_name())
    }

    // Error construction helpers - reduce boilerplate for common error patterns

    /// Create an error with custom message at current position
    pub(super) fn error_msg(&self, message: &str) -> ParseError {
        ParseError::InvalidSyntax {
            message: message.to_string(),
            position: self.current_pos().0,
            context: None,
        }
    }

    /// Create an error with custom message at custom position
    pub(super) fn error_msg_at(&self, message: &str, position: usize) -> ParseError {
        ParseError::InvalidSyntax {
            message: message.to_string(),
            position,
            context: None,
        }
    }

    /// Create an error: "Expected X"
    pub(super) fn error_expected(&self, what: &str) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Expected {what}"),
            position: self.current_pos().0,
            context: None,
        }
    }

    /// Create an error: "Expected X" at custom position
    pub(super) fn error_expected_at(&self, what: &str, position: usize) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Expected {what}"),
            position,
            context: None,
        }
    }

    /// Create an error: "Expected X, found Y"
    pub(super) fn error_expected_found(&self, what: &str) -> ParseError {
        let kind = &self.current_kind;
        ParseError::InvalidSyntax {
            message: format!("Expected {what}, found {kind}"),
            position: self.current_pos().0,
            context: None,
        }
    }

    /// Create an error: "Expected X, found Y" at custom position
    pub(super) fn error_expected_found_at(&self, what: &str, position: usize) -> ParseError {
        let kind = &self.current_kind;
        ParseError::InvalidSyntax {
            message: format!("Expected {what}, found {kind}"),
            position,
            context: None,
        }
    }

    /// Create an error: "Expected X after Y, found Z"
    pub(super) fn error_expected_after(&self, what: &str, after: &str) -> ParseError {
        let kind = &self.current_kind;
        ParseError::InvalidSyntax {
            message: format!("Expected {what} after '{after}', found {kind}"),
            position: self.current_pos().0,
            context: None,
        }
    }

    /// Create an error: "Unexpected keyword 'X'"
    pub(super) fn error_unexpected_keyword(&self, kw: KeywordKind) -> ParseError {
        ParseError::InvalidSyntax {
            message: format!("Unexpected keyword '{kw}'"),
            position: self.current_pos().0,
            context: None,
        }
    }

    /// Create an error: "Expected 'X' or 'Y' after list element, found Z"
    pub(super) fn error_list_separator(
        &self,
        separator: &TokenKind,
        terminator: &TokenKind,
    ) -> ParseError {
        let kind = &self.current_kind;
        ParseError::InvalidSyntax {
            message: format!(
                "Expected '{separator}' or '{terminator}' after list element, found {kind}"
            ),
            position: self.current_pos().0,
            context: None,
        }
    }

    pub(super) fn check(&self, kind: &TokenKind) -> bool {
        &self.current_kind == kind
    }

    /// Check if current token is an assignment operator and return it.
    ///
    /// Returns `Some(operator)` for: `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `**=`,
    /// `<<=`, `>>=`, `>>>=`, `&=`, `|=`, `^=`, `&&=`, `||=`, `??=`
    pub(super) fn try_assignment_operator(&self) -> Option<AssignmentOperator> {
        match &self.current_kind {
            TokenKind::Equals => Some(AssignmentOperator::Assign),
            TokenKind::PlusEquals => Some(AssignmentOperator::AddAssign),
            TokenKind::MinusEquals => Some(AssignmentOperator::SubtractAssign),
            TokenKind::StarEquals => Some(AssignmentOperator::MultiplyAssign),
            TokenKind::SlashEquals => Some(AssignmentOperator::DivideAssign),
            TokenKind::PercentEquals => Some(AssignmentOperator::RemainderAssign),
            TokenKind::StarStarEquals => Some(AssignmentOperator::ExponentiateAssign),
            TokenKind::LeftShiftEquals => Some(AssignmentOperator::LeftShiftAssign),
            TokenKind::RightShiftEquals => Some(AssignmentOperator::RightShiftAssign),
            TokenKind::UnsignedRightShiftEquals => {
                Some(AssignmentOperator::UnsignedRightShiftAssign)
            }
            TokenKind::AmpersandEquals => Some(AssignmentOperator::BitwiseAndAssign),
            TokenKind::PipeEquals => Some(AssignmentOperator::BitwiseOrAssign),
            TokenKind::CaretEquals => Some(AssignmentOperator::BitwiseXorAssign),
            TokenKind::AmpersandAmpersandEquals => Some(AssignmentOperator::LogicalAndAssign),
            TokenKind::PipePipeEquals => Some(AssignmentOperator::LogicalOrAssign),
            TokenKind::QuestionQuestionEquals => Some(AssignmentOperator::NullishAssign),
            _ => None,
        }
    }

    // Peek helpers for lookahead (needed for type annotations, operators, etc.)
    // Lazily computes peek token on first access.
    // Stores lexer errors to be returned on next advance() call.
    pub(super) fn peek_kind(&mut self) -> TokenKind {
        if self.peek_cache.is_none() && self.lexer_error.is_none() {
            match self.lexer.next_token() {
                Ok(token) => {
                    self.peek_cache = Some(PeekData::with_decoded(
                        token.kind,
                        token.start,
                        token.end,
                        token.decoded,
                    ));
                }
                Err(err) => {
                    // Store error to be returned on next advance()
                    self.lexer_error = Some(err);
                }
            }
        }
        self.peek_cache
            .as_ref()
            .map_or(TokenKind::Eof, |p| p.kind.clone())
    }

    #[expect(dead_code, reason = "Convenience wrapper for peek_kind() == kind")]
    pub(super) fn peek_check(&mut self, kind: &TokenKind) -> bool {
        &self.peek_kind() == kind
    }

    /// Get the value of the peek token as a string slice
    pub(super) fn peek_value(&self) -> &str {
        self.peek_cache
            .as_ref()
            .map_or("", |p| &self.source[p.start..p.end])
    }

    /// Check if peek token is an identifier (used for contextual keyword disambiguation)
    pub(super) fn peek_is_identifier(&mut self) -> bool {
        matches!(self.peek_kind(), TokenKind::Identifier)
    }

    /// Check if peek token is a specific kind
    pub(super) fn peek_is(&mut self, kind: &TokenKind) -> bool {
        self.peek_kind() == *kind
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

    /// Check if current token is an identifier or keyword.
    ///
    /// In JS/TypeScript, reserved words (keywords) can be used as property names
    /// in member expressions: `obj.class`, `obj.if`, `obj.default()`.
    ///
    /// This is distinct from `peek_is_property_name` which also allows `[` and strings.
    /// After `.` or `?.`, we expect just an identifier or keyword (not computed/string).
    pub(super) fn current_is_identifier_or_keyword(&self) -> bool {
        matches!(
            self.current_kind,
            TokenKind::Identifier | TokenKind::Keyword(_)
        )
    }

    /// Get the property name string from current token (identifier or keyword).
    ///
    /// Returns the string representation for property name contexts where both
    /// identifiers and keywords are valid (e.g., after `.` in member access).
    ///
    /// # Precondition
    /// Current token must be an identifier or keyword. Call `current_is_identifier_or_keyword()`
    /// to verify before calling this method.
    pub(super) fn current_property_name(&self) -> &str {
        match &self.current_kind {
            TokenKind::Identifier => self.current_value(),
            TokenKind::Keyword(kw) => kw.as_str(),
            _ => {
                debug_assert!(
                    false,
                    "current_property_name called on non-identifier/keyword token"
                );
                // Return empty string as fallback in release builds
                ""
            }
        }
    }

    /// Check if peek token could be a class member name (identifier, keyword, computed key, or private identifier)
    ///
    /// Used to detect accessor syntax in class bodies:
    /// - `get x() {}` - getter (peek is `x` = identifier)
    /// - `get #x() {}` - private getter (peek is `#`)
    /// - `get [expr]() {}` - computed getter (peek is `[`)
    pub(super) fn peek_is_class_member_name(&mut self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Identifier
                | TokenKind::BracketOpen
                | TokenKind::String
                | TokenKind::Keyword(_)
                | TokenKind::Hash
        )
    }

    /// Parse a private identifier: `#name`
    ///
    /// Current token must be `#`, followed by an identifier.
    /// Returns the PrivateIdentifier with span including the `#`.
    pub(super) fn parse_private_identifier(&mut self) -> Result<PrivateIdentifier, ParseError> {
        debug_assert!(matches!(self.current_kind(), TokenKind::Hash));
        let start = self.current_pos().0;
        self.advance()?; // consume '#'

        // Must be followed by an identifier
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(self.error_expected_after("identifier", "#"));
        }

        let (_, end) = self.current_pos();
        let name = self.intern_identifier();
        self.advance()?;

        Ok(PrivateIdentifier {
            name,
            span: Span::new(start as u32, end as u32),
        })
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

    /// Expect `>` in type context, handling compound token splitting
    ///
    /// In TypeScript, compound tokens starting with `>` can appear in type contexts where
    /// they need to be split (e.g., `Array<Map<K, V>>`, `const k: <T>() => T = ...`).
    ///
    /// This method:
    /// - Consumes `>` normally if current token is `>`
    /// - Splits `>>` into `>` + `>`, consuming the first
    /// - Splits `>>>` into `>` + `>>`, consuming the first
    /// - Splits `>=` into `>` + re-lex (may become `=>`)
    /// - Splits `>>=` into `>` + re-lex (may become `>=` or `>` + `=`)
    /// - Splits `>>>=` into `>` + re-lex (may become `>>=`)
    pub(super) fn expect_greater_than_in_type(&mut self) -> Result<(), ParseError> {
        match self.current_kind {
            TokenKind::GreaterThan => {
                // Normal case: single `>`
                self.advance()
            }
            TokenKind::RightShift => {
                // `>>` - split into `>` + `>`
                // Consume first `>` by advancing start position
                self.current_start += 1;
                self.current_kind = TokenKind::GreaterThan;
                // Clear peek cache since token boundaries changed
                self.peek_cache = None;
                Ok(())
            }
            TokenKind::UnsignedRightShift => {
                // `>>>` - split into `>` + `>>`
                // Consume first `>` by advancing start position
                self.current_start += 1;
                self.current_kind = TokenKind::RightShift;
                // Clear peek cache since token boundaries changed
                self.peek_cache = None;
                Ok(())
            }
            TokenKind::GreaterThanEquals
            | TokenKind::RightShiftEquals
            | TokenKind::UnsignedRightShiftEquals => {
                // `>=`, `>>=`, `>>>=` - consume `>`, re-lex from next position
                // The remainder might combine with subsequent chars (e.g., `>=` -> `=>`)
                let new_start = self.current_start + 1;
                let token = self.lexer.seek_and_next_token(new_start)?;
                self.current_kind = token.kind;
                self.current_start = token.start;
                self.current_end = token.end;
                self.current_decoded = token.decoded;
                // Clear peek cache since token changed
                self.peek_cache = None;
                Ok(())
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: "'>'".to_string(),
                found: format!("'{}'", self.current_kind),
                position: self.current_start,
                context: None,
            }),
        }
    }

    /// Check if current token is `>` or can be split to produce `>` (for type contexts)
    pub(super) fn check_greater_than_in_type(&self) -> bool {
        matches!(
            self.current_kind,
            TokenKind::GreaterThan
                | TokenKind::RightShift
                | TokenKind::UnsignedRightShift
                | TokenKind::GreaterThanEquals
                | TokenKind::RightShiftEquals
                | TokenKind::UnsignedRightShiftEquals
        )
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
        self.check(&kind) && self.try_advance()
    }

    /// Consume a contextual keyword if present (identifier with specific value).
    /// Returns true if consumed, false otherwise.
    #[inline]
    pub(super) fn eat_contextual_keyword(&mut self, keyword: &str) -> bool {
        matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == keyword
            && self.try_advance()
    }

    /// Check if the next (peek) token is a contextual keyword.
    /// Does not consume any tokens (only peeks).
    #[inline]
    pub(super) fn peek_is_contextual_keyword(&mut self, keyword: &str) -> bool {
        matches!(self.peek_kind(), TokenKind::Identifier) && self.peek_value() == keyword
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
        // Check for stored lexer error first (from failed eat/peek operations)
        if let Some(err) = self.lexer_error.take() {
            return Err(err);
        }
        if self.eat(TokenKind::Semicolon) {
            return Ok(());
        }
        // Check again after eat() in case it stored an error
        if let Some(err) = self.lexer_error.take() {
            return Err(err);
        }
        if self.can_insert_semicolon() {
            return Ok(());
        }
        Err(self.error_expected("';'"))
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
            Err(self.error_list_separator(separator, terminator))
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
                        let param_start = self.current_pos().0;

                        // Check for parameter property modifiers: public, private, protected, readonly
                        let accessibility = if self.eat_contextual_keyword("public") {
                            Some(Accessibility::Public)
                        } else if self.eat_contextual_keyword("private") {
                            Some(Accessibility::Private)
                        } else if self.eat_contextual_keyword("protected") {
                            Some(Accessibility::Protected)
                        } else {
                            None
                        };

                        // Check for readonly modifier (can appear alone or after accessibility)
                        let readonly = self.eat_contextual_keyword("readonly");

                        // If we have modifiers, this is a parameter property
                        if accessibility.is_some() || readonly {
                            // Parse the parameter name
                            let (id_start, id_end) = self.current_pos();
                            if !matches!(self.current_kind(), TokenKind::Identifier) {
                                return Err(self
                                    .error_expected_at("parameter name after modifier", id_start));
                            }
                            let symbol = self.intern_identifier();
                            self.advance()?;

                            // Check for optional marker: param?
                            let optional = self.eat(TokenKind::Question);

                            // Check for type annotation: param: type
                            let (type_annotation, end_pos) = if self.check(&TokenKind::Colon) {
                                let ta = self.parse_type_annotation()?;
                                let end = ta.span.end;
                                (Some(ta), end as usize)
                            } else {
                                (None, id_end)
                            };

                            let identifier = Identifier {
                                name: symbol,
                                optional,
                                type_annotation,
                                span: Span::new(id_start as u32, end_pos as u32),
                            };

                            // Check for default value: param = default
                            let (parameter, param_end): (Expression, u32) =
                                if self.check(&TokenKind::Equals) {
                                    self.advance()?;
                                    let default_value = self.parse_assignment_expression()?;
                                    let assign_end = default_value.span().end;
                                    (
                                        Expression::AssignmentPattern(AssignmentPattern {
                                            left: Box::new(Expression::Identifier(identifier)),
                                            right: Box::new(default_value),
                                            span: Span::new(id_start as u32, assign_end),
                                        }),
                                        assign_end,
                                    )
                                } else {
                                    (Expression::Identifier(identifier), end_pos as u32)
                                };

                            Expression::TSParameterProperty(TSParameterProperty {
                                accessibility,
                                readonly,
                                parameter: Box::new(parameter),
                                span: Span::new(param_start as u32, param_end),
                            })
                        } else {
                            // Simple identifier parameter (no modifiers)
                            let (param_start, param_end) = self.current_pos();
                            let symbol = self.intern_identifier();
                            self.advance()?;

                            // Check for optional marker: param?
                            let optional = self.eat(TokenKind::Question);

                            // Check for type annotation: param: type
                            let (type_annotation, id_end) = if self.check(&TokenKind::Colon) {
                                let ta = self.parse_type_annotation()?;
                                let end = ta.span.end;
                                (Some(ta), end as usize)
                            } else {
                                (None, param_end)
                            };

                            let mut param = Expression::Identifier(Identifier {
                                name: symbol,
                                optional,
                                type_annotation,
                                span: Span::new(param_start as u32, id_end as u32),
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
                    TokenKind::DotDotDot => {
                        // Rest parameter: ...args or ...args: type
                        let rest_start = self.current_pos().0;
                        self.advance()?; // consume '...'

                        // Parse the identifier
                        let (id_start, id_end) = self.current_pos();
                        let symbol = self.intern_identifier();
                        self.expect(&TokenKind::Identifier)?;

                        // Check for type annotation: ...args: type
                        let (type_annotation, arg_end) = if self.check(&TokenKind::Colon) {
                            let ta = self.parse_type_annotation()?;
                            let end = ta.span.end;
                            (Some(ta), end as usize)
                        } else {
                            (None, id_end)
                        };

                        let argument = Expression::Identifier(Identifier {
                            name: symbol,
                            optional: false,
                            type_annotation,
                            span: Span::new(id_start as u32, arg_end as u32),
                        });

                        Expression::RestElement(RestElement {
                            argument: Box::new(argument),
                            span: Span::new(rest_start as u32, arg_end as u32),
                        })
                    }
                    _ => {
                        return Err(
                            self.error_expected_found("parameter name or destructuring pattern")
                        );
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
    /// TypeScript `as`/`satisfies` parsing is disabled in this mode because `as`
    /// has special meaning in Svelte template contexts (e.g., `{#each items as pattern}`).
    ///
    /// Returns (expression, end_position) where end_position is where the next
    /// unparsed content begins (in absolute source coordinates with base_offset).
    pub fn parse_assignment_expression_partial(
        &mut self,
    ) -> Result<(Expression, usize), ParseError> {
        // Disable TypeScript type assertion parsing in partial mode
        // to avoid consuming `as` which has different meaning in Svelte templates
        let saved = self.allow_ts_type_assertions;
        self.allow_ts_type_assertions = false;
        let result = self.parse_assignment_expression();
        self.allow_ts_type_assertions = saved;

        let expr = result?;
        // Return the start of the current (unconsumed) token
        let next_pos = self.current_start + self.base_offset;
        Ok((expr, next_pos))
    }

    /// Convert an expression to a binding pattern.
    ///
    /// This converts ObjectExpression to ObjectPattern, ArrayExpression to ArrayPattern,
    /// etc. Used when parsing destructuring patterns in variable declarations and
    /// similar contexts.
    ///
    /// # Arguments
    ///
    /// * `expr` - The expression to convert (typically an ObjectExpression or ArrayExpression)
    ///
    /// # Returns
    ///
    /// * `Ok(Expression)` - The converted pattern (ObjectPattern, ArrayPattern, etc.)
    /// * `Err(ParseError)` - If the expression cannot be converted to a valid pattern
    pub fn expression_to_pattern(&self, expr: Expression) -> Result<Expression, ParseError> {
        self.to_assignable(expr)
    }

    /// Parse a string literal into a Literal node.
    ///
    /// Expects the current token to be a String token.
    pub(super) fn parse_string_literal(&mut self) -> Result<Literal, ParseError> {
        debug_assert!(matches!(self.current_kind(), TokenKind::String));

        let (start, end) = self.current_pos();
        let raw = self.current_value().to_string();
        let quote = raw.chars().next().unwrap_or('"');

        let content = if let Some(decoded) = self.current_decoded() {
            decoded.to_string()
        } else if raw.len() >= 2 {
            raw[1..raw.len() - 1].to_string()
        } else {
            String::new()
        };

        self.advance()?;

        Ok(Literal {
            value: LiteralValue::String { content, quote },
            span: Span::new(start as u32, end as u32),
        })
    }
}

/// Parse TypeScript source code into an AST.
pub fn parse_typescript(source: &str) -> Result<Program, ParseError> {
    let mut parser = Parser::new(source)?;
    parser.parse()
}
