// TypeScript parser - main entry point and coordination

use crate::Goal;
use crate::ast::internal::*;
use crate::lexer::{KeywordKind, Lexer, Token, TokenKind};
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use std::cell::RefCell;
use std::rc::Rc;
use string_interner::{DefaultStringInterner, DefaultSymbol};
use tsv_lang::{ParseError, SharedInterner, Span};

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

/// Build a detached [`Comment`] from a lexed comment token's positions.
///
/// `content_start` / `token_*` are local (pre-`base_offset`) byte offsets; the
/// stored spans are shifted into host coordinates by `base_offset` so embedded
/// `<script>` / `{expr}` comments slice the host source. The content end is the
/// token end minus the closing `*/` for block comments (`//` and `#!` run to the
/// token end). Returns the comment plus whether its content holds a line
/// terminator — block comments only — which callers fold into
/// `had_line_terminator` for ASI (a multi-line comment counts as one terminator).
fn comment_from_token(
    source: &str,
    token_start: usize,
    token_end: usize,
    content_start: usize,
    is_block: bool,
    base_offset: usize,
) -> (Comment, bool) {
    let content_end = if is_block { token_end - 2 } else { token_end };
    let content = &source[content_start..content_end];
    let has_line_terminator = is_block && content.contains(['\n', '\r', '\u{2028}', '\u{2029}']);
    let comment = Comment {
        content_span: Span::new(
            (content_start + base_offset) as u32,
            (content_end + base_offset) as u32,
        ),
        is_block,
        multiline: content.contains('\n'),
        span: Span::new(
            (token_start + base_offset) as u32,
            (token_end + base_offset) as u32,
        ),
        emit_character_field: false,
    };
    (comment, has_line_terminator)
}

#[allow(clippy::struct_excessive_bools)]
pub struct Parser<'a, 'arena> {
    /// Bump arena that owns every AST node this parser allocates. Supplied by
    /// the caller (caller-owns-`Bump`); the returned `Program<'arena>` borrows
    /// from it. `&'arena Bump` is `Copy`, so `self.alloc(owned)` and
    /// `self.arena.alloc(self.parse_x()?)` (even while `&mut self` is held — the
    /// field read borrows the `Bump`, not `self`) both work directly; lift it into a
    /// local (`let arena = self.arena;`) only when several allocations in one method
    /// share it.
    arena: &'arena Bump,
    source: &'a str,
    lexer: Lexer<'a>,
    /// The current token's classification + span as the lexer's 16-byte POD,
    /// stored in place so `advance()` overwrites it directly (`self.current =
    /// self.lexer.next_token()?`) with no intermediate `Token` scattered into
    /// separate scalar fields. The rare decoded value rides out-of-band in
    /// `current_decoded` (escape paths only), mirroring the lexer's split.
    current: Token,
    current_decoded: Option<String>, // Decoded string value (for strings with escapes)
    /// Single-token lookahead slot, stored as the 16-byte `Token` POD with its
    /// decoded value out-of-band in `peek_decoded` (mirroring the `current` /
    /// `current_decoded` split). Consuming the peek is then a direct `Token` copy
    /// into `current` with no intermediate lookahead struct / `Option<String>` to
    /// reassemble.
    peek: Option<Token>,
    peek_decoded: Option<String>,
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
    /// `peek` is `Some`; consumed by `advance_inner()`.
    peek_had_line_terminator: bool,
    /// Whether to allow `in` as a binary operator.
    /// Set to false when parsing for-loop headers to distinguish `for (x in y)` from expressions.
    allow_in: bool,
    /// The syntactic goal symbol (`Script` vs `Module`) this parse runs against.
    /// Fixed for the whole parse — embedders (Svelte) and the standalone
    /// `parse`/`format` default to `Module`; `parse_with_goal` overrides it.
    goal: Goal,
    /// The `[Await]` grammar context. `true` (`[+Await]`) inside an async
    /// function/arrow/method/generator's params or body, a class static
    /// initialization block, a `for await` head, and — by default — module top
    /// level; reset to `false` (`[~Await]`) on entering a non-async
    /// function-like scope, and at Script top level. When `false`, `await` is
    /// not an await-expression: under `Goal::Script` it is an ordinary
    /// identifier (`await_is_identifier`), under `Goal::Module` it is reserved.
    in_await: bool,
    /// Whether the type grammar disallows function/constructor types at the
    /// current position: a union/intersection constituent (after `|`/`&`,
    /// including the leading-operator forms) or a type-operator operand
    /// (`keyof`/`unique`/`readonly`). TS (and acorn-typescript) admit
    /// `FunctionType`/`ConstructorType` only at full-type positions, so at these
    /// operand positions a `(` is always a parenthesized type — a following `=>`
    /// belongs to an enclosing construct (e.g. the enclosing arrow function's own
    /// `=>` in `(): A & (B) => x`) — and `new () => T` / `<T>() => U` are syntax
    /// errors (`A & () => x` must be written `A & (() => x)`). Set by the
    /// constituent/operand parses in `types.rs`; cleared at every full-type
    /// descent (`parse_type`), so nested positions (type arguments, tuple
    /// members, object-type members, conditional branches, parenthesized inners)
    /// parse function types greedily again.
    fn_type_disallowed: bool,
    /// Whether the type grammar disallows conditional types at the current
    /// position: the extends clause of a conditional type and the constraint of
    /// a constrained `infer` (acorn-typescript's
    /// `inDisallowConditionalTypesContext`). Read by the constrained-infer
    /// parse: at a disallow position `infer U extends C ? …` keeps `C` as the
    /// constraint (the `?` belongs to the enclosing conditional); at an allow
    /// position the same tokens are a conditional whose check is the bare
    /// `infer U` (see `pending_conditional_extends`). Cleared at every
    /// full-type descent (`parse_type`), matching acorn's allow-context resets
    /// (parenthesized inners, tuple/object members, type arguments, and — via
    /// its explicit signature wrapper — function/constructor-type params and
    /// returns).
    conditional_type_disallowed: bool,
    /// Hand-off from the constrained-infer parse to `parse_type_inner`: when
    /// `infer U extends C` at an allow-conditional position is directly
    /// followed by `?`, the already-parsed `C` re-binds as the extends clause
    /// of a conditional whose check is the bare `infer U` (acorn rolls back
    /// its constraint tryParse and re-parses `extends C` at the conditional
    /// level; this hand-off reproduces that without re-lexing). Set only when
    /// the current token is `?`; consumed by the innermost enclosing
    /// `parse_type_inner`, which nothing can precede (every intermediate
    /// union/intersection/array/operand loop breaks on `?`).
    pending_conditional_extends: Option<TSType<'arena>>,
}

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Create a parser with a fresh interner against an explicit goal symbol.
    /// The standalone `parse`/`parse_with_goal` paths use this; embedders go
    /// through [`Parser::with_interner`] (always `Module`).
    fn new_with_goal(source: &'a str, goal: Goal, arena: &'arena Bump) -> Result<Self, ParseError> {
        Self::with_interner_and_goal(
            source,
            0,
            Rc::new(RefCell::new(DefaultStringInterner::with_capacity(
                tsv_lang::estimated_interner_capacity(source.len()),
            ))),
            goal,
            arena,
        )
    }

    /// Allocate a single AST node in the arena, returning a shared `&'arena`
    /// reference (replaces `Box::new`). Zero-copy: `Bump::alloc` moves the value
    /// into arena memory; the mut→shared reborrow is implicit.
    #[inline]
    fn alloc<T>(&self, val: T) -> &'arena T {
        self.arena.alloc(val)
    }

    /// A growable vector that builds AST-node collections **directly in the
    /// arena** — the preferred way to gather children. Build it in the parse
    /// loop, then `.into_bump_slice()` to store the field (zero-copy: the buffer
    /// is already arena-owned; `into_bump_slice` just hands it back). Carries its
    /// own `Copy` `&'arena Bump`, so pushing `self.parse_x()?` inside the loop
    /// does NOT borrow `self` — no `&mut self` conflict.
    #[inline]
    fn bvec<T>(&self) -> BumpVec<'arena, T> {
        BumpVec::new_in(self.arena)
    }

    /// Allocate a decoded string (escapes processed — not a verbatim source
    /// slice) in the arena. One copy into the arena. (No-escape string literals
    /// are a verbatim source slice and could instead carry a `Span`, avoiding
    /// even this one copy.)
    #[inline]
    fn alloc_str_in(&self, s: &str) -> &'arena str {
        self.arena.alloc_str(s)
    }

    /// Allocate the binding `extra` for a typed identifier carrying `ta`: a type
    /// annotation and no decorators. Callers thread the optionality
    /// (`type_annotation.map(|ta| self.typed_extra(ta))`); decorators, when
    /// present, are folded in separately by the parameter-list caller
    /// (`attach_param_decorators`).
    #[inline]
    fn typed_extra(&self, ta: TSTypeAnnotation<'arena>) -> &'arena IdentifierParamExtra<'arena> {
        self.alloc(IdentifierParamExtra {
            type_annotation: Some(ta),
            decorators: None,
        })
    }

    /// Create a parser with shared interner and base offset.
    ///
    /// Used when parsing embedded expressions/scripts in Svelte templates.
    /// base_offset is added to all span positions to get correct positions in full source.
    /// Embedded contexts are always modules (Svelte `<script>` is a module), so
    /// this defaults the goal; the goal-aware [`Parser::new_with_goal`] is the
    /// only `Script` entry.
    pub fn with_interner(
        source: &'a str,
        base_offset: usize,
        interner: SharedInterner,
        arena: &'arena Bump,
    ) -> Result<Self, ParseError> {
        Self::with_interner_and_goal(source, base_offset, interner, Goal::Module, arena)
    }

    /// [`Parser::with_interner`] with an explicit goal symbol — the single
    /// constructor that actually builds the parser state.
    fn with_interner_and_goal(
        source: &'a str,
        base_offset: usize,
        interner: SharedInterner,
        goal: Goal,
        arena: &'arena Bump,
    ) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(source);
        let mut current = lexer.next_token()?;
        let mut decoded = lexer.take_decoded().map(|b| *b);

        // Collect leading comment tokens
        let mut comments = Vec::new();
        while let TokenKind::Comment {
            is_block,
            content_start,
        } = &current.kind
        {
            let (comment, _) = comment_from_token(
                source,
                current.start as usize,
                current.end as usize,
                *content_start as usize,
                *is_block,
                base_offset,
            );
            comments.push(comment);
            current = lexer.next_token()?;
            decoded = lexer.take_decoded().map(|b| *b);
        }

        Ok(Self {
            arena,
            source,
            lexer,
            current,
            current_decoded: decoded,
            peek: None,
            peek_decoded: None,
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
            goal,
            // Module top level is `[+Await]` (`ModuleItem[+Await]`); Script top
            // level is `[~Await]` (`ScriptBody[~Await]`).
            in_await: matches!(goal, Goal::Module),
            fn_type_disallowed: false, // Top level is a full-type position
            conditional_type_disallowed: false, // Top level allows conditional types
            pending_conditional_extends: None,
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
        self.prev_end = self.current.end as usize;

        // Get next token (from peek cache or lexer)
        if let Some(peek) = self.peek.take() {
            // Direct 16-byte copy of the cached token POD — no field-by-field
            // reassembly or `usize`→`u32` conversion.
            self.current = peek;
            self.current_decoded = self.peek_decoded.take();
            // Recorded while populating the peek cache — includes line
            // terminators before/inside comments drained during the peek.
            self.had_line_terminator = self.peek_had_line_terminator;
        } else {
            // Write the lexed token straight into the current slot — `next_token_into`
            // writes through `&mut self.current` (disjoint from `&mut self.lexer`), so
            // no intermediate `Token` is built/returned/scattered (no sret round-trip).
            self.lexer.next_token_into(&mut self.current)?;
            self.current_decoded = self.lexer.take_decoded().map(|b| *b);
            self.had_line_terminator = self.lexer.had_line_terminator();
        }

        self.collect_comments()
    }

    /// Drain any `Comment` tokens at the current position into `self.comments`, leaving the current
    /// token at the first non-comment token. Shared by `advance_inner` and the regex relex path
    /// (`parse_primary_expression`), both of which land on a fresh token and must absorb any
    /// comments before the next consumer reads the current token.
    ///
    /// The common case — the current token is *not* a comment — is a single discriminant check that
    /// inlines into the hot `advance` pump; the drain loop itself is cold-outlined into
    /// `drain_comments` so it never bloats the inlined fast path.
    #[inline]
    pub(super) fn collect_comments(&mut self) -> Result<(), ParseError> {
        if matches!(self.current.kind, TokenKind::Comment { .. }) {
            self.drain_comments()
        } else {
            Ok(())
        }
    }

    /// The cold half of `collect_comments`: the current token is known to be a `Comment` on entry;
    /// drain it and any consecutive comments. `#[cold]` + `#[inline(never)]` keep it off the hot pump.
    #[cold]
    #[inline(never)]
    fn drain_comments(&mut self) -> Result<(), ParseError> {
        while let TokenKind::Comment {
            is_block,
            content_start,
        } = &self.current.kind
        {
            // ECMAScript spec: if a MultiLineComment contains one or more line terminators,
            // then it is replaced by a single line terminator for ASI purposes.
            // So block comments with newlines should set had_line_terminator.
            let (comment, has_line_terminator) = comment_from_token(
                self.source,
                self.current.start as usize,
                self.current.end as usize,
                *content_start as usize,
                *is_block,
                self.base_offset,
            );
            if has_line_terminator {
                self.had_line_terminator = true;
            }
            self.comments.push(comment);
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
        &self.current.kind
    }

    /// Overwrite the current token's kind/start/end/decoded from a freshly lexed token, without
    /// the surrounding bookkeeping (`prev_end`, the line-terminator flag, comment collection).
    /// Used by `collect_comments` and by callers that resync the lexer themselves before reading —
    /// template continuation and the regex relex.
    #[inline]
    pub(super) fn update_current(&mut self, token: Token) {
        self.current = token;
        // `decoded` rides out-of-band on the lexer; the caller lexed `token` from
        // `self.lexer` immediately before this call, so drain it here.
        self.current_decoded = self.lexer.take_decoded().map(|b| *b);
    }

    #[inline]
    pub(super) fn current_pos(&self) -> (usize, usize) {
        (
            self.current.start as usize + self.base_offset,
            self.current.end as usize + self.base_offset,
        )
    }

    /// Convert a raw `source`-relative offset into an absolute `Span` coordinate:
    /// `base_offset`-shifted and narrowed to `u32`. Raw offsets index `self.source`
    /// (e.g. `self.current.start as usize`, a captured scan position); `Span` fields
    /// store the shifted `u32`. Centralizes the `(base_offset + pos) as u32` boundary
    /// cast — the `u32` sibling of `current_pos` (which stays `usize` for indexing).
    #[inline]
    pub(super) fn span_pos(&self, raw: usize) -> u32 {
        (self.base_offset + raw) as u32
    }

    /// Get the end position of the previously consumed token (with base_offset).
    ///
    /// Useful for determining where statements end after consuming optional tokens
    /// like semicolons (via ASI or explicit).
    #[inline]
    pub(super) fn prev_token_end(&self) -> usize {
        self.prev_end + self.base_offset
    }

    /// Consume an optional `?` marker, extending a binding's end to cover it.
    /// Returns `(present, end)`: when the `?` is eaten, `end` advances to
    /// `prev_token_end` (so a `?` with no following type annotation still
    /// extends the identifier span); otherwise `end` passes through unchanged.
    #[inline]
    pub(super) fn eat_optional_marker(&mut self, end: usize) -> (bool, usize) {
        if self.eat(TokenKind::Question) {
            (true, self.prev_token_end())
        } else {
            (false, end)
        }
    }

    /// Get the raw end position (without base_offset) for lexer operations
    pub(super) fn current_raw_end(&self) -> usize {
        self.current.end as usize
    }

    /// Resolve a `StringCooked` during parse. Stored spans are in **host**
    /// coordinates (`base_offset` added), but `self.source` is the local
    /// (possibly embedded) slice — so the span shifts back before slicing.
    /// Resolving a host span directly against `self.source` reads the wrong
    /// bytes under Svelte embedding, or panics past the slice end.
    pub(super) fn resolve_cooked<'s>(
        &'s self,
        cooked: &'s StringCooked<'arena>,
        span: Span,
    ) -> &'s str {
        let local = Span::new(
            span.start - self.base_offset as u32,
            span.end - self.base_offset as u32,
        );
        cooked.resolve(local, self.source)
    }

    /// The current token's verbatim source text. Returns `&'a str` (borrowing the
    /// immutable source), not `&self` — so callers can hold it across `advance()`
    /// without a borrow-escape `.to_string()`.
    #[inline]
    pub(super) fn current_value(&self) -> &'a str {
        &self.source[self.current.start as usize..self.current.end as usize]
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

    /// Intern the current token, which the caller has already verified is either
    /// a plain `Identifier` or `await` used as an identifier (`at_await_identifier`
    /// — e.g. a class name, single-param arrow param, or `break`/`continue` label
    /// at Script `[~Await]`). A plain identifier decodes unicode escapes; `await`
    /// interns verbatim.
    pub(super) fn intern_identifier_or_await(&self) -> DefaultSymbol {
        if matches!(self.current_kind(), TokenKind::Identifier) {
            self.intern_identifier()
        } else {
            self.intern("await")
        }
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

    /// Intern the current token as a function declaration name. Like
    /// [`Parser::try_intern_identifier_or_keyword`], but `await` is accepted only
    /// where it is a valid identifier (Script `[~Await]`): at `Module` / `[+Await]`
    /// it is a reserved `BindingIdentifier` (the goal-level and `[Await]` early
    /// errors), so `function await(){}` / `export function await(){}` reject there,
    /// matching acorn-as-module and the function-expression name path. Other
    /// contextual keywords (`async`, `from`, type keywords) stay valid names.
    pub(super) fn try_intern_function_name(&self) -> Option<DefaultSymbol> {
        if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Await))
            && !self.await_is_identifier()
        {
            return None;
        }
        self.try_intern_identifier_or_keyword()
    }

    /// Intern the current token as a binding name, accepting contextual keywords.
    ///
    /// Like `try_intern_identifier_or_keyword` but uses `can_be_binding_name()`,
    /// which excludes `await`, `yield`, and `let` (not valid as parameter/variable names).
    pub(super) fn try_intern_binding_name(&self) -> Option<DefaultSymbol> {
        match self.current_kind() {
            TokenKind::Identifier => Some(self.intern_identifier()),
            TokenKind::Keyword(kw) if kw.can_be_binding_name() => Some(self.intern(kw.as_str())),
            // `await` is a valid `BindingIdentifier` only at Script goal in a
            // `[~Await]` context (the two independent goal/`[Await]` early errors).
            TokenKind::Keyword(KeywordKind::Await) if self.await_is_identifier() => {
                Some(self.intern("await"))
            }
            _ => None,
        }
    }

    /// Whether `await` may be used as an ordinary identifier in the current
    /// context: only at `Goal::Script` and outside any `[+Await]` context. This
    /// is the conjunction of the two independent ECMAScript early errors —
    /// `await`-as-identifier is a Syntax Error if the goal is `Module`, OR if the
    /// production carries the `[Await]` parameter.
    pub(super) fn await_is_identifier(&self) -> bool {
        self.goal == Goal::Script && !self.in_await
    }

    /// Whether the current token is `await` *and* it may be an ordinary
    /// identifier here (`await_is_identifier`) — i.e. it should be treated as a
    /// `BindingIdentifier`/`IdentifierReference`/`LabelIdentifier` rather than an
    /// await-expression or a reserved word.
    pub(super) fn at_await_identifier(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Await))
            && self.await_is_identifier()
    }

    /// Shared body of the `with_*` context combinators: run `f` with the
    /// boolean context flag selected by `flag` set to `value`, restoring the
    /// prior value afterward (on success and error alike).
    pub(super) fn with_context_flag<T>(
        &mut self,
        flag: fn(&mut Self) -> &mut bool,
        value: bool,
        f: impl FnOnce(&mut Self) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        let saved = std::mem::replace(flag(self), value);
        let result = f(self);
        *flag(self) = saved;
        result
    }

    /// Run `f` with the `[Await]` context set to `value`, restoring it
    /// afterward (on success and error alike). Mirrors `with_allow_in`; wrap a
    /// function-like scope's params+body so a nested async/non-async scope sets
    /// its own `await`-context without leaking to the enclosing one.
    pub(super) fn with_in_await<T>(
        &mut self,
        value: bool,
        f: impl FnOnce(&mut Self) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        self.with_context_flag(|p| &mut p.in_await, value, f)
    }

    /// Run `f` with the function-type-disallowed context set to `value`,
    /// restoring it afterward (on success and error alike). Mirrors
    /// `with_in_await`. Set `true` around union/intersection constituent and
    /// type-operator operand parses; set `false` at full-type entry
    /// (`parse_type`) so nested positions parse function types greedily again.
    pub(super) fn with_fn_type_disallowed<T>(
        &mut self,
        value: bool,
        f: impl FnOnce(&mut Self) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        self.with_context_flag(|p| &mut p.fn_type_disallowed, value, f)
    }

    /// Run `f` with the conditional-type-disallowed context set to `value`,
    /// restoring it afterward (on success and error alike). Mirrors
    /// `with_fn_type_disallowed`. Set `true` around a conditional's extends
    /// clause and a constrained infer's constraint; set `false` at full-type
    /// entry (`parse_type`) so nested positions parse conditionals greedily
    /// again.
    pub(super) fn with_conditional_type_disallowed<T>(
        &mut self,
        value: bool,
        f: impl FnOnce(&mut Self) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        self.with_context_flag(|p| &mut p.conditional_type_disallowed, value, f)
    }

    /// Run `f` at a full-type position — both type-context restrictions
    /// (`fn_type_disallowed`, `conditional_type_disallowed`) cleared, each
    /// restored afterward — so nested full-type positions parse function and
    /// conditional types greedily even when reached from a constituent/operand
    /// parse. Wraps the full-type entry (`parse_type_inner`).
    pub(super) fn with_full_type_context<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        self.with_fn_type_disallowed(false, |p| p.with_conditional_type_disallowed(false, f))
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

    /// Intern the current token as the `IdentifierName` half of a module export
    /// name, accepting ANY keyword (e.g. `export { x as if }`).
    ///
    /// ES spec: `ModuleExportName : IdentifierName | StringLiteral`. This handles
    /// only the `IdentifierName` arm; callers test for `TokenKind::String` first
    /// and build a `ModuleExportName::Literal` for the `StringLiteral` arm.
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
    /// Decoded form of the current string-literal token, as a [`StringCooked`].
    ///
    /// `Verbatim` (no escapes) carries no allocation — the decoded value equals
    /// the inner source slice (recovered later via `StringCooked::resolve(span,
    /// source)`). `Decoded` (escapes present) arena-copies the lexer's decoded
    /// value (one copy). The quote char is no longer stored — recover it via
    /// `Literal::string_quote(source)`.
    pub(super) fn extract_string_cooked(&self) -> StringCooked<'arena> {
        match self.current_decoded() {
            Some(decoded) => StringCooked::Decoded(self.alloc_str_in(decoded)),
            None => StringCooked::Verbatim,
        }
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
        let kind = &self.current.kind;
        ParseError::InvalidSyntax {
            message: format!("Expected {what}, found {kind}"),
            position: self.current_pos().0,
            context: None,
        }
    }

    /// Create an error: "Expected X, found Y" at custom position
    pub(super) fn error_expected_found_at(&self, what: &str, position: usize) -> ParseError {
        let kind = &self.current.kind;
        ParseError::InvalidSyntax {
            message: format!("Expected {what}, found {kind}"),
            position,
            context: None,
        }
    }

    /// Create an error: "Expected X after Y, found Z"
    pub(super) fn error_expected_after(&self, what: &str, after: &str) -> ParseError {
        let kind = &self.current.kind;
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
        let kind = &self.current.kind;
        ParseError::InvalidSyntax {
            message: format!(
                "Expected '{separator}' or '{terminator}' after list element, found {kind}"
            ),
            position: self.current_pos().0,
            context: None,
        }
    }

    pub(super) fn check(&self, kind: &TokenKind) -> bool {
        &self.current.kind == kind
    }

    /// Check if current token is an assignment operator and return it.
    ///
    /// Returns `Some(operator)` for: `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `**=`,
    /// `<<=`, `>>=`, `>>>=`, `&=`, `|=`, `^=`, `&&=`, `||=`, `??=`
    pub(super) fn try_assignment_operator(&self) -> Option<AssignmentOperator> {
        match &self.current.kind {
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
        if self.peek.is_none() && self.lexer_error.is_none() {
            self.peek_had_line_terminator = false;
            loop {
                match self.lexer.next_token() {
                    Ok(token) => {
                        if self.lexer.had_line_terminator() {
                            self.peek_had_line_terminator = true;
                        }
                        if let TokenKind::Comment {
                            is_block,
                            content_start,
                        } = &token.kind
                        {
                            // ECMAScript spec: a MultiLineComment containing a line
                            // terminator counts as one for ASI purposes.
                            let (comment, has_line_terminator) = comment_from_token(
                                self.source,
                                token.start as usize,
                                token.end as usize,
                                *content_start as usize,
                                *is_block,
                                self.base_offset,
                            );
                            if has_line_terminator {
                                self.peek_had_line_terminator = true;
                            }
                            self.comments.push(comment);
                            continue;
                        }
                        self.peek = Some(token);
                        self.peek_decoded = self.lexer.take_decoded().map(|b| *b);
                    }
                    Err(err) => {
                        // Store error to be returned on next advance() (unbox: the
                        // lexer returns Box<ParseError>, `lexer_error` is ParseError).
                        self.lexer_error = Some(*err);
                    }
                }
                break;
            }
        }
        self.peek
            .as_ref()
            .map_or(TokenKind::Eof, |t| t.kind.clone())
    }

    #[expect(dead_code, reason = "Convenience wrapper for peek_kind() == kind")]
    pub(super) fn peek_check(&mut self, kind: &TokenKind) -> bool {
        &self.peek_kind() == kind
    }

    /// Get the value of the peek token as a string slice
    pub(super) fn peek_value(&self) -> &str {
        self.peek
            .as_ref()
            .map_or("", |t| &self.source[t.start as usize..t.end as usize])
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
    /// keyword (`override`, `readonly`) from a parameter that happens to be
    /// named the same.
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
        self.peek.as_ref().map_or(0, |t| t.start as usize)
    }

    /// Whether a line terminator separates the current token from the peeked one.
    ///
    /// Scans the raw inter-token slice, so a comment containing a newline counts
    /// as a line terminator (per ASI rules). Used for `[no LineTerminator here]`
    /// restrictions like `using [no LineTerminator here] BindingIdentifier`.
    pub(super) fn peek_preceded_by_line_terminator(&mut self) -> bool {
        self.peek_kind(); // populate the cache
        let to = self.peek_start();
        let from = (self.current.end as usize).min(to);
        self.source[from..to].contains(['\n', '\r', '\u{2028}', '\u{2029}'])
    }

    /// Whether the peeked token is an identifier on the same line as the current
    /// token (tsc's `nextTokenIsIdentifierOnSameLine`).
    ///
    /// The shared shape behind the contextual-keyword declaration starters
    /// (`type`/`interface`/`namespace`/`module`): a line break before the
    /// name demotes the keyword to a plain identifier and ASI splits the statement.
    pub(super) fn peek_is_same_line_identifier(&mut self) -> bool {
        self.peek_is_identifier() && !self.peek_preceded_by_line_terminator()
    }

    /// Whether the peeked token is a same-line *binding word* for a `using`
    /// declaration: any identifier-shaped word — a plain identifier or a
    /// keyword-lexed contextual name (`async`, `undefined`, …) — except the
    /// words that continue the *expression* reading of `using` instead:
    /// the word-shaped binary operators (`using in b`, `using instanceof C`)
    /// and the cast keywords (`using as T`, `using satisfies T` — acorn reads
    /// these as casts of the identifier `using`; tsc commits to a declaration
    /// with a binding named `as`/`satisfies`, but the drop-in oracle wins).
    /// Reserved words (`function`, `let`, …) pass the gate and are rejected by
    /// the binding parser, matching acorn's rejection of both readings. The
    /// one-past-peek sibling is `peek_followed_by_same_line_binding_word`.
    pub(super) fn peek_is_same_line_binding_word(&mut self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Identifier | TokenKind::Keyword(_)
        ) && !self.peek_preceded_by_line_terminator()
            && !matches!(self.peek_value(), "in" | "instanceof" | "as" | "satisfies")
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
    pub(super) fn peek_followed_by_same_line_binding_word(&mut self) -> bool {
        self.peek_kind(); // populate the cache
        let after_peek = self.peek.as_ref().map_or(0, |t| t.end as usize);
        let bytes = self.source.as_bytes();
        let pos = scan::skip_whitespace_and_comments(bytes, after_peek);
        pos < bytes.len()
            && scan::is_identifier_start(bytes[pos])
            && !self.source[after_peek..pos].contains(['\n', '\r', '\u{2028}', '\u{2029}'])
            && {
                // A word continuing the *expression* reading instead of binding:
                // `await using in b` / `await using instanceof C` are await
                // expressions (`in`/`instanceof` are the word-shaped binary
                // operators), and `await using as T` / `await using satisfies T`
                // are casts of `await using` (acorn's reading; tsc would commit
                // to a declaration binding `as`/`satisfies`, but the drop-in
                // oracle wins). Every other word is a binding attempt — including
                // contextual keywords that are valid binding names (`async`,
                // `undefined`, `of`). Mirrors `peek_is_same_line_binding_word`.
                let end = scan::skip_identifier(bytes, pos);
                let word = &bytes[pos..end];
                word != b"in" && word != b"instanceof" && word != b"as" && word != b"satisfies"
            }
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
            self.current.kind,
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
    pub(super) fn current_property_name(&self) -> &'a str {
        match &self.current.kind {
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
                found: self.current.kind.to_string(),
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
        match self.current.kind {
            TokenKind::GreaterThan => {
                // Normal case: single `>`
                self.advance()
            }
            TokenKind::RightShift => {
                // `>>` - split into `>` + `>`
                // Consume first `>` by advancing start position. The split
                // only narrows the current token — `current.end` and every
                // later token boundary are unchanged — so a cached peek (lexed
                // from `current.end`) stays valid and MUST be kept: clearing
                // it would desync the cache from the lexer's cursor (the next
                // fill would silently skip the peeked token).
                self.current.start += 1;
                self.current.kind = TokenKind::GreaterThan;
                Ok(())
            }
            TokenKind::UnsignedRightShift => {
                // `>>>` - split into `>` + `>>`
                // Consume first `>` by advancing start position; the cached
                // peek stays valid (see the `>>` arm).
                self.current.start += 1;
                self.current.kind = TokenKind::RightShift;
                Ok(())
            }
            TokenKind::GreaterThanEquals
            | TokenKind::RightShiftEquals
            | TokenKind::UnsignedRightShiftEquals => {
                // `>=`, `>>=`, `>>>=` - consume `>`, re-lex from next position
                // The remainder might combine with subsequent chars (e.g., `>=` -> `=>`)
                let new_start = self.current.start as usize + 1;
                // Drop comments drained by a discarded peek — the seek below
                // re-lexes that region, and they'd be collected twice.
                let relex_from = self.span_pos(new_start);
                while self
                    .comments
                    .last()
                    .is_some_and(|c| c.span.start >= relex_from)
                {
                    self.comments.pop();
                }
                let token = self.lexer.seek_and_next_token(new_start)?;
                self.current = token;
                self.current_decoded = self.lexer.take_decoded().map(|b| *b);
                // Clear peek cache since token changed
                self.peek = None;
                self.peek_decoded = None;
                Ok(())
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: "'>'".to_string(),
                found: format!("'{}'", self.current.kind),
                position: self.current_pos().0,
                context: None,
            }),
        }
    }

    /// Whether the current token is `<` or the `<<` shift token, whose first
    /// `<` can open a type-argument list (`f<<T>(v: T) => void>()`) — the
    /// opening mirror of `check_greater_than_in_type`. `<<=` never splits: the
    /// `<=` remainder cannot continue a type-argument list (matches acorn).
    pub(super) fn check_less_than_in_type(&self) -> bool {
        matches!(
            self.current.kind,
            TokenKind::LessThan | TokenKind::LeftShift
        )
    }

    /// Expect `<` opening a type-argument list, splitting `<<` into `<` + `<`
    /// — the opening mirror of `expect_greater_than_in_type`.
    pub(super) fn expect_less_than_in_type(&mut self) -> Result<(), ParseError> {
        match self.current.kind {
            TokenKind::LessThan => self.advance(),
            TokenKind::LeftShift => {
                // Consume the first `<` by advancing the token start; the
                // remainder is the inner `<`. The cached peek stays valid
                // (see `expect_greater_than_in_type`'s `>>` arm).
                self.current.start += 1;
                self.current.kind = TokenKind::LessThan;
                Ok(())
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: "'<'".to_string(),
                found: format!("'{}'", self.current.kind),
                position: self.current_pos().0,
                context: None,
            }),
        }
    }

    /// Check if current token is `>` or can be split to produce `>` (for type contexts)
    pub(super) fn check_greater_than_in_type(&self) -> bool {
        matches!(
            self.current.kind,
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
        matches!(self.current.kind, TokenKind::Eof | TokenKind::BraceClose)
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

    pub fn parse(&mut self) -> Result<Program<'arena>, ParseError> {
        let start = self.base_offset; // Start at base_offset for embedded contexts
        let mut body = self.bvec();

        while self.current.kind != TokenKind::Eof {
            body.push(self.parse_module_item()?);
        }
        self.adapt_directive_prologue(&mut body);

        // Use current_pos() to get global position (includes base_offset)
        let (_, end) = self.current_pos();

        // Build line breaks table for O(log n) line boundary lookups
        // Must add base_offset to each position since AST spans use global positions
        let base_offset_u32 = self.base_offset as u32;
        // Standalone (`.ts`/`.svelte.ts`) parses have `base_offset == 0`, so the per-position
        // shift is an identity — return the table directly instead of cloning it through
        // `.map(|pos| pos + 0).collect()`. Only embedded `<script>` (base_offset > 0) needs the shift.
        let line_breaks: Vec<u32> = if base_offset_u32 == 0 {
            tsv_lang::printing::build_line_breaks(self.source)
        } else {
            tsv_lang::printing::build_line_breaks(self.source)
                .into_iter()
                .map(|pos| pos + base_offset_u32)
                .collect()
        };

        Ok(Program {
            body: body.into_bump_slice(),
            comments: std::mem::take(&mut self.comments),
            line_breaks,
            span: Span::new(start as u32, end as u32),
            interner: Rc::clone(&self.interner),
            goal: self.goal,
        })
    }

    /// Parse a single expression (used by Svelte for expression tags)
    pub fn parse_expression_public(&mut self) -> Result<Expression<'arena>, ParseError> {
        self.parse_expression()
    }

    /// Parse a single expression and return it with any collected comments.
    /// Used for expressions in Svelte templates where comments need to be preserved.
    pub fn parse_expression_with_comments(
        &mut self,
    ) -> Result<(Expression<'arena>, Vec<Comment>), ParseError> {
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
    ) -> Result<(Expression<'arena>, usize), ParseError> {
        // Disable TypeScript type assertion parsing in partial mode
        // to avoid consuming `as` which has different meaning in Svelte templates
        let saved = self.allow_ts_type_assertions;
        self.allow_ts_type_assertions = false;
        let result = self.parse_assignment_expression();
        self.allow_ts_type_assertions = saved;

        let expr = result?;
        // Return the start of the current (unconsumed) token
        let next_pos = self.current.start as usize + self.base_offset;
        Ok((expr, next_pos))
    }

    /// Check if the current token is a colon.
    pub fn at_colon(&self) -> bool {
        matches!(self.current.kind, TokenKind::Colon)
    }

    /// Parse a type annotation (`: Type`) at the current position.
    /// Public wrapper for use from lib.rs.
    pub fn parse_type_annotation_public(&mut self) -> Result<TSTypeAnnotation<'arena>, ParseError> {
        self.parse_type_annotation()
    }

    /// Get the current token's start position (absolute, with base_offset).
    pub fn current_absolute_position(&self) -> usize {
        self.current.start as usize + self.base_offset
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
    pub fn expression_to_pattern(
        &self,
        expr: Expression<'arena>,
    ) -> Result<Expression<'arena>, ParseError> {
        // Svelte `{:then}` / `{:catch}` binding patterns — a binding context, so a
        // type-assertion target is rejected (same as for-heads / function params).
        self.to_assignable(expr, expression_assignable::AssignableContext::Binding)
    }

    /// Parse a string literal into a Literal node.
    ///
    /// Expects the current token to be a String token.
    pub(super) fn parse_string_literal(&mut self) -> Result<Literal<'arena>, ParseError> {
        debug_assert!(matches!(self.current_kind(), TokenKind::String));

        let (start, end) = self.current_pos();
        let cooked = self.extract_string_cooked();
        self.advance()?;

        Ok(Literal {
            value: LiteralValue::String(cooked),
            span: Span::new(start as u32, end as u32),
        })
    }
}

/// Parse TypeScript source code into an AST allocated in `arena`.
pub fn parse_typescript<'arena>(
    source: &str,
    arena: &'arena Bump,
) -> Result<Program<'arena>, ParseError> {
    parse_typescript_with_goal(source, Goal::Module, arena)
}

/// [`parse_typescript`] against an explicit goal symbol. `parse_typescript` is
/// the `Goal::Module` form.
pub fn parse_typescript_with_goal<'arena>(
    source: &str,
    goal: Goal,
    arena: &'arena Bump,
) -> Result<Program<'arena>, ParseError> {
    let mut parser = Parser::new_with_goal(source, goal, arena)?;
    parser.parse()
}
