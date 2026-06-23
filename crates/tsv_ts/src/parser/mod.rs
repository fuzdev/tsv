// TypeScript parser - main entry point and coordination

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, Lexer, TokenKind};
use std::cell::RefCell;
use std::rc::Rc;
use string_interner::{DefaultStringInterner, DefaultSymbol};
use tsv_lang::{ParseError, PeekData, SharedInterner, Span};

// Import parsing implementations
mod expression;
mod expression_arrow; // Arrow function predicate scans and builders
mod expression_assignable; // Cover-grammar expression→pattern conversion (`to_assignable`)
mod expression_literals; // Object and array literal parsing
mod expression_lookahead; // Arrow function and type argument disambiguation
mod expression_template; // Template literal parsing (`\`...${expr}...\``)
mod expression_type_args; // Type-argument byte-scan lookahead (`<Type, ...>` vs `<`)
mod parameters; // Function/method parameter and destructuring-pattern parsing
mod scan; // Low-level byte scanning utilities
mod statement; // Statement parsing (refactored into submodules)
mod type_members; // Type-literal / interface-body member grammar (property/method/signature elements)
mod types; // TypeScript type-syntax parsing (annotations, type expressions, type parameters)

#[allow(clippy::struct_excessive_bools)]
pub struct Parser<'a> {
    source: &'a str,
    lexer: Lexer<'a>,
    current_kind: TokenKind,
    current_start: usize,
    current_end: usize,
    current_decoded: Option<String>, // Decoded string value (for strings with escapes)
    peek_cache: Option<PeekData<TokenKind>>,
    interner: SharedInterner,
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
    /// Nesting depth inside grouping delimiters (`(...)`, `[...]`, `{...}`, `${...}`).
    /// Used to disambiguate context-sensitive keywords inside nested expressions:
    /// - `as`/`satisfies`: always type assertions when depth > 0, even when
    ///   `allow_ts_type_assertions` is false (Svelte `#each` partial parsing)
    /// - `in`: always a binary operator when depth > 0, even when `allow_in` is
    ///   false (for-loop header parsing)
    grouping_depth: u32,
    /// True when parsing inside `declare namespace` or `declare module`.
    /// Functions inside ambient contexts don't have bodies (end with `;`).
    in_ambient_context: bool,
    /// Stored lexer error from peek_kind(). Returned on next advance() call.
    /// This ensures lexer errors propagate even when peek swallows them.
    lexer_error: Option<ParseError>,
    /// Whether a line terminator (including before/inside comments drained
    /// during the peek) precedes the cached peek token. Only meaningful while
    /// `peek_cache` is `Some`; consumed by `advance_inner()`.
    peek_had_line_terminator: bool,
    /// Whether to allow `in` as a binary operator.
    /// Set to false when parsing for-loop headers to distinguish `for (x in y)` from expressions.
    allow_in: bool,
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
        interner: SharedInterner,
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
                emit_character_field: false,
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
            grouping_depth: 0,              // Not inside any grouping delimiters
            in_ambient_context: false,      // Not in declare namespace/module
            lexer_error: None,              // No stored lexer error
            peek_had_line_terminator: false, // No peek cached yet
            allow_in: true,                 // Allow `in` binary operator by default
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
            // Recorded while populating the peek cache — includes line
            // terminators before/inside comments drained during the peek.
            self.had_line_terminator = self.peek_had_line_terminator;
        } else {
            let token = self.lexer.next_token()?;
            self.current_kind = token.kind;
            self.current_start = token.start;
            self.current_end = token.end;
            self.current_decoded = token.decoded;
            self.had_line_terminator = self.lexer.had_line_terminator();
        }

        self.collect_comments()
    }

    /// Drain consecutive `Comment` tokens starting at the current token into `self.comments`,
    /// leaving the current token at the first non-comment token. Shared by `advance_inner` and
    /// the regex relex path (`parse_primary_expression`), both of which land on a fresh token and
    /// must absorb any comments before the next consumer reads the current token.
    pub(super) fn collect_comments(&mut self) -> Result<(), ParseError> {
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
                emit_character_field: false,
            });
            let token = self.lexer.next_token()?;
            self.update_current(token);
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

    /// Overwrite the current token's kind/start/end/decoded from a freshly lexed token, without
    /// the surrounding bookkeeping (`prev_end`, the line-terminator flag, comment collection).
    /// Used by `collect_comments` and by callers that resync the lexer themselves before reading —
    /// template continuation and the regex relex.
    #[inline]
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

    /// Get the string value of the current identifier or contextual keyword.
    ///
    /// Returns the decoded name for identifiers with unicode escapes,
    /// or the keyword string for contextual keywords. Returns `None`
    /// if the current token is not identifier-like.
    pub(super) fn current_identifier_or_keyword_name(&self) -> Option<&str> {
        match self.current_kind() {
            TokenKind::Identifier => Some(self.current_identifier_name()),
            TokenKind::Keyword(kw) if kw.can_be_identifier() => Some(kw.as_str()),
            _ => None,
        }
    }

    /// Intern the current token as an identifier, accepting contextual keywords.
    ///
    /// Handles `TokenKind::Identifier` (with unicode escape decoding) and
    /// contextual keywords like `from`, `as`, `satisfies`. Returns `None`
    /// if the current token is not identifier-like.
    pub(super) fn try_intern_identifier_or_keyword(&self) -> Option<DefaultSymbol> {
        match self.current_kind() {
            TokenKind::Identifier => Some(self.intern_identifier()),
            TokenKind::Keyword(kw) if kw.can_be_identifier() => Some(self.intern(kw.as_str())),
            _ => None,
        }
    }

    /// Intern the current token as a binding name, accepting contextual keywords.
    ///
    /// Like `try_intern_identifier_or_keyword` but uses `can_be_binding_name()`,
    /// which excludes `await`, `yield`, and `let` (not valid as parameter/variable names).
    pub(super) fn try_intern_binding_name(&self) -> Option<DefaultSymbol> {
        match self.current_kind() {
            TokenKind::Identifier => Some(self.intern_identifier()),
            TokenKind::Keyword(kw) if kw.can_be_binding_name() => Some(self.intern(kw.as_str())),
            _ => None,
        }
    }

    /// Like `try_intern_binding_name`, but also accepts the `this` keyword as the
    /// TypeScript `this` parameter (`function f(this: T)`, `(this: T) => U`).
    pub(super) fn try_intern_param_name(&self) -> Option<DefaultSymbol> {
        self.try_intern_binding_name().or_else(|| {
            if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::This)) {
                Some(self.intern("this"))
            } else {
                None
            }
        })
    }

    /// Intern the current token as a module export name, accepting ANY keyword.
    ///
    /// ES spec: ModuleExportName is IdentifierName | StringLiteral.
    /// IdentifierName includes all reserved words (e.g., `export { x as if }`).
    pub(super) fn try_intern_identifier_name(&self) -> Option<DefaultSymbol> {
        match self.current_kind() {
            TokenKind::Identifier => Some(self.intern_identifier()),
            TokenKind::Keyword(kw) => Some(self.intern(kw.as_str())),
            _ => None,
        }
    }

    /// Extract string literal content and quote character from current token.
    ///
    /// Assumes current token is `TokenKind::String`. Returns `(content, quote)` where:
    /// - `content` is the decoded string value (escapes processed)
    /// - `quote` is the quote character used (`'` or `"`)
    ///
    /// Uses decoded value from lexer if available (escapes present),
    /// otherwise extracts content by stripping quotes from raw value.
    pub(super) fn extract_string_literal(&self) -> (String, char) {
        let raw = self.current_value();
        let quote = raw.chars().next().unwrap_or('"');
        let content = if let Some(decoded) = self.current_decoded() {
            decoded.to_string()
        } else if raw.len() >= 2 {
            raw[1..raw.len() - 1].to_string()
        } else {
            String::new()
        };
        (content, quote)
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
    //
    // Comment tokens are drained into `self.comments` (mirroring
    // `collect_comments()`) so the cached token — and every peek-based
    // decision — is the next CODE token. Line terminators seen while
    // draining are recorded in `peek_had_line_terminator` for the
    // advance() that later consumes the cached token.
    pub(super) fn peek_kind(&mut self) -> TokenKind {
        if self.peek_cache.is_none() && self.lexer_error.is_none() {
            self.peek_had_line_terminator = false;
            loop {
                match self.lexer.next_token() {
                    Ok(token) => {
                        if self.lexer.had_line_terminator() {
                            self.peek_had_line_terminator = true;
                        }
                        if let TokenKind::Comment { content, is_block } = &token.kind {
                            // ECMAScript spec: a MultiLineComment containing a line
                            // terminator counts as one for ASI purposes.
                            if *is_block && content.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
                                self.peek_had_line_terminator = true;
                            }
                            self.comments.push(Comment {
                                content: content.clone(),
                                is_block: *is_block,
                                span: Span::new(
                                    (token.start + self.base_offset) as u32,
                                    (token.end + self.base_offset) as u32,
                                ),
                                emit_character_field: false,
                            });
                            continue;
                        }
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
                break;
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

    /// Whether the peek token can begin a function parameter binding: an
    /// identifier, a destructuring pattern (`[`/`{`), a rest element (`...`),
    /// `this`, or a contextual keyword usable as a binding name (e.g. another
    /// modifier like `readonly`). Used to disambiguate a contextual modifier
    /// keyword (`override`) from a parameter that happens to be named the same.
    pub(super) fn peek_starts_parameter_binding(&mut self) -> bool {
        match self.peek_kind() {
            TokenKind::Identifier
            | TokenKind::BracketOpen
            | TokenKind::BraceOpen
            | TokenKind::DotDotDot
            | TokenKind::Keyword(KeywordKind::This) => true,
            TokenKind::Keyword(kw) => kw.can_be_binding_name(),
            _ => false,
        }
    }

    /// Get the start position of the peek token (cache must be populated via peek_kind() first)
    pub(super) fn peek_start(&self) -> usize {
        self.peek_cache.as_ref().map_or(0, |p| p.start)
    }

    /// Whether a line terminator separates the current token from the peeked one.
    ///
    /// Scans the raw inter-token slice, so a comment containing a newline counts
    /// as a line terminator (per ASI rules). Used for `[no LineTerminator here]`
    /// restrictions like `using [no LineTerminator here] BindingIdentifier`.
    pub(super) fn peek_preceded_by_line_terminator(&mut self) -> bool {
        self.peek_kind(); // populate the cache
        let to = self.peek_start();
        let from = self.current_end.min(to);
        self.source[from..to].contains(['\n', '\r', '\u{2028}', '\u{2029}'])
    }

    /// Whether the peeked token is an identifier on the same line as the current
    /// token (tsc's `nextTokenIsIdentifierOnSameLine`).
    ///
    /// The shared shape behind the contextual-keyword declaration starters
    /// (`using`/`type`/`interface`/`namespace`/`module`): a line break before the
    /// name demotes the keyword to a plain identifier and ASI splits the statement.
    pub(super) fn peek_is_same_line_identifier(&mut self) -> bool {
        self.peek_is_identifier() && !self.peek_preceded_by_line_terminator()
    }

    /// Whether the token after a `declare` modifier begins an ambient declaration
    /// on the same line.
    ///
    /// `declare` is a contextual keyword: a following line terminator (ASI) or a
    /// non-declaration token demotes it to a plain identifier (`declare;`,
    /// `declare = x`). Mirrors tsc's `isDeclaration` modifier handling
    /// (`nextToken(); if (hasPrecedingLineBreak()) return false; continue;`): the
    /// next token must be a declaration starter on the same line. The contextual
    /// starters (`abstract`/`namespace`/`module`/`interface`/`type`/`global`) are
    /// matched by source value since they lex as plain identifiers.
    pub(super) fn peek_starts_ambient_declaration(&mut self) -> bool {
        if self.peek_preceded_by_line_terminator() {
            return false;
        }
        match self.peek_kind() {
            TokenKind::Keyword(
                KeywordKind::Const
                | KeywordKind::Let
                | KeywordKind::Var
                | KeywordKind::Function
                | KeywordKind::Class
                | KeywordKind::Enum,
            ) => true,
            TokenKind::Identifier => matches!(
                self.peek_value(),
                "abstract" | "namespace" | "module" | "interface" | "type" | "global"
            ),
            _ => false,
        }
    }

    /// Whether the peeked token is followed on the same line by an identifier.
    ///
    /// Used for `await using [no LineTerminator here] BindingIdentifier`, where
    /// the binding sits one token past the peek horizon.
    pub(super) fn peek_followed_by_same_line_identifier(&mut self) -> bool {
        self.peek_kind(); // populate the cache
        let after_peek = self.peek_cache.as_ref().map_or(0, |p| p.end);
        let bytes = self.source.as_bytes();
        let pos = scan::skip_whitespace_and_comments(bytes, after_peek);
        pos < bytes.len()
            && scan::is_identifier_start(bytes[pos])
            && !self.source[after_peek..pos].contains(['\n', '\r', '\u{2028}', '\u{2029}'])
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
                | TokenKind::Number
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
                | TokenKind::Number
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

        // Must be followed by an identifier (keywords like `async` are valid: `#async`)
        let (_, end) = self.current_pos();
        let Some(name) = self.try_intern_identifier_or_keyword() else {
            return Err(self.error_expected_after("identifier", "#"));
        };
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
                position: self.current_pos().0,
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
    ///   Consume a `>` in type context and return the end position of the consumed `>`.
    ///   Handles `>>`, `>>>`, `>=`, etc. by splitting the token.
    pub(super) fn greater_than_end_in_type(&mut self) -> Result<u32, ParseError> {
        let end = (self.current_pos().0 + 1) as u32;
        self.expect_greater_than_in_type()?;
        Ok(end)
    }

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
                // Drop comments drained by a discarded peek — the seek below
                // re-lexes that region, and they'd be collected twice.
                let relex_from = (new_start + self.base_offset) as u32;
                while self
                    .comments
                    .last()
                    .is_some_and(|c| c.span.start >= relex_from)
                {
                    self.comments.pop();
                }
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
                position: self.current_pos().0,
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
    /// Consume a semicolon and return the end position (including the semicolon).
    /// Use this for statement spans that include the trailing semicolon.
    pub(super) fn semicolon_end(&mut self) -> Result<u32, ParseError> {
        self.semicolon()?;
        Ok(self.prev_token_end() as u32)
    }

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

    pub fn parse(&mut self) -> Result<Program, ParseError> {
        let start = self.base_offset; // Start at base_offset for embedded contexts
        let mut body = Vec::new();

        while self.current_kind != TokenKind::Eof {
            body.push(self.parse_statement()?);
        }
        self.adapt_directive_prologue(&mut body);

        // Use current_pos() to get global position (includes base_offset)
        let (_, end) = self.current_pos();

        // Build line breaks table for O(log n) line boundary lookups
        // Must add base_offset to each position since AST spans use global positions
        let base_offset_u32 = self.base_offset as u32;
        let line_breaks: Vec<u32> = tsv_lang::printing::build_line_breaks(self.source)
            .into_iter()
            .map(|pos| pos + base_offset_u32)
            .collect();

        Ok(Program {
            body,
            comments: std::mem::take(&mut self.comments),
            line_breaks,
            span: Span::new(start as u32, end as u32),
            interner: Rc::clone(&self.interner),
        })
    }

    /// Parse a single expression (used by Svelte for expression tags)
    pub fn parse_expression_public(&mut self) -> Result<Expression, ParseError> {
        self.parse_expression()
    }

    /// Parse a single expression and return it with any collected comments.
    /// Used for expressions in Svelte templates where comments need to be preserved.
    pub fn parse_expression_with_comments(
        &mut self,
    ) -> Result<(Expression, Vec<Comment>), ParseError> {
        let expr = self.parse_expression()?;
        let comments = self.take_comments();
        Ok((expr, comments))
    }

    /// Take ownership of collected comments.
    /// Used when parsing expressions that need to return comments to the caller.
    pub fn take_comments(&mut self) -> Vec<Comment> {
        std::mem::take(&mut self.comments)
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

    /// Check if the current token is a colon.
    pub fn at_colon(&self) -> bool {
        matches!(self.current_kind, TokenKind::Colon)
    }

    /// Parse a type annotation (`: Type`) at the current position.
    /// Public wrapper for use from lib.rs.
    pub fn parse_type_annotation_public(&mut self) -> Result<TSTypeAnnotation, ParseError> {
        self.parse_type_annotation()
    }

    /// Get the current token's start position (absolute, with base_offset).
    pub fn current_absolute_position(&self) -> usize {
        self.current_start + self.base_offset
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
        let (content, quote) = self.extract_string_literal();
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
