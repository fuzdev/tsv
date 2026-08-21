// TypeScript parser - main entry point and coordination

use crate::Goal;
use crate::ast::internal::*;
use crate::lexer::{KeywordKind, Lexer, Token, TokenKind, is_es_line_terminator};
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use tsv_lang::{ParseError, Span};

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

pub(crate) use expression::is_jsdoc_type_cast_comment;

/// Who owns a top-level `as` in a partial expression parse — TypeScript, or the host
/// grammar the expression is embedded in.
///
/// Svelte's block heads are the only callers, and they split on whether the head's own
/// binding separator is spelled `as`:
///
/// - [`TopLevelAs::Assertion`] — `{#await p as T then v}`. `then` / `catch` are not type
///   syntax, so an `as` in an await head is always TypeScript's; the parse ends on its
///   own at the clause keyword.
/// - [`TopLevelAs::HostSeparator`] — `{#each xs as item}`, where the separator *is* `as`.
///   The parse must leave the keyword for the block reader, which then walks the
///   assertion run itself (`tsv_svelte`'s `each_binding_separator`).
///
/// ⚠️ The axis is `as` **alone**, which is why the name says so. `satisfies` is a type
/// assertion too, but no host separator is spelled `satisfies`, so it is never in
/// question and stays consumable under both variants — the wider "type assertions"
/// framing this replaced denied it in `{#each}` for no reason, rejecting
/// `{#each xs satisfies T as item}`, which canonical Svelte accepts.
///
/// The policy applies only at [`Parser::grouping_depth`] `0` — inside grouping,
/// `(x as T)` is unambiguously an assertion under both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopLevelAs {
    /// Consume `as` wherever it is grammatical.
    Assertion,
    /// Stop at a top-level `as`, leaving the keyword to the host grammar.
    HostSeparator,
}

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
    // Line comments end at the first line terminator, so their content never
    // contains one — gate both scans below on `is_block` (value unchanged, scan
    // skipped for every `//` comment).
    //
    // Ask the `\n` question first: it is a single-`char` pattern, so it lowers to
    // a `memchr`, whereas the full `LineTerminator` predicate is a char-at-a-time
    // searcher over the whole comment body (a long JSDoc block pays that in full).
    // And a `\n` *is* a line terminator, so on the common multi-line block comment
    // it answers the wider question for free; only a block comment with no `\n` at
    // all — a one-liner like `/* x */` — reaches the rare-terminator scan, which is
    // spelled as the shared production minus the character already ruled out rather
    // than as its own list, so widening the class cannot leave this behind.
    let multiline = is_block && content.contains('\n');
    let has_line_terminator =
        is_block && (multiline || content.contains(|c| is_es_line_terminator(c) && c != '\n'));
    let comment = Comment {
        content_span: Span::new(
            (content_start + base_offset) as u32,
            (content_end + base_offset) as u32,
        ),
        is_block,
        multiline,
        span: Span::new(
            (token_start + base_offset) as u32,
            (token_end + base_offset) as u32,
        ),
        emit_character_field: false,
        bump_pattern_columns: false,
        // Set later by the parser: a `(` glued to this comment makes it a cast's.
        owned_by_node: false,
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
    /// Decoded string/identifier value for the current token (escape paths only),
    /// copied into the AST arena at receipt so it is a `Copy` `&'arena str` rather
    /// than an owned `String` — the lexer decodes into a reused scratch buffer and
    /// the parser arena-copies it immediately, so no per-literal `String` survives.
    current_decoded: Option<&'arena str>,
    /// Single-token lookahead slot, stored as the 16-byte `Token` POD with its
    /// decoded value out-of-band in `peek_decoded` (mirroring the `current` /
    /// `current_decoded` split). Consuming the peek is then a direct `Token` copy
    /// into `current` with no intermediate lookahead struct to reassemble.
    peek: Option<Token>,
    peek_decoded: Option<&'arena str>,
    base_offset: usize, // Offset in full source (for embedded expressions)
    /// Comments collected during parsing, gathered directly in the AST arena
    /// (`Comment` is a `Copy` POD, so bumpalo's no-`Drop` rule holds). Handed to
    /// consumers as an `&'arena [Comment]` slice via `take_comments`, so the
    /// warm binding loops (`reset()`-reused arenas) never malloc for comments.
    comments: BumpVec<'arena, Comment>,
    /// True if a line terminator occurred between the previous token and current token.
    /// Used for ASI (Automatic Semicolon Insertion).
    had_line_terminator: bool,
    /// End position of the previous token (before current). Used for span calculation
    /// when ASI inserts a semicolon.
    prev_end: usize,
    /// Whether a top-level `as` is TypeScript's assertion operator. Set per partial
    /// parse from a [`TopLevelAs`] policy — false only where the host grammar spells its
    /// own separator `as` (`{#each items as pattern}`), never as a blanket
    /// "Svelte template" rule (`{#await p as T then v}` keeps it).
    ///
    /// `satisfies` is deliberately NOT gated by this: no host separator collides with it,
    /// so it is an assertion in every context.
    top_level_as_is_assertion: bool,
    /// Nesting depth inside grouping delimiters (`(...)`, `[...]`, `{...}`, `${...}`),
    /// maintained by [`Parser::enter_grouping`] / [`Parser::exit_grouping`].
    /// Used to disambiguate context-sensitive keywords inside nested expressions:
    /// - `as`: always a type assertion when depth > 0, even when
    ///   `top_level_as_is_assertion` is false (Svelte `#each` partial parsing)
    /// - `in`: a binary operator once the depth rises above [`Parser::no_in_depth`],
    ///   even when `allow_in` is false (for-loop header parsing)
    ///
    /// ⚠️ A reading of this counter is meaningless without a **baseline**, and the
    /// two consumers above use different ones — a literal `0` for `as` (that region
    /// opens at the parse root, where a fresh `Parser` starts at 0) and
    /// `no_in_depth` for `in` (that region opens mid-parse, at whatever depth the
    /// enclosing expression had reached). Copying one gate's baseline to the other
    /// is the bug this split exists to prevent.
    grouping_depth: u32,
    /// When `true`, grouping parens `(expr)` are preserved as an internal
    /// `ParenthesizedExpression` node instead of being discarded. Off by default
    /// (matching acorn/Svelte, whose public AST is paren-free); enabled only for
    /// the `{#snippet}`-parameter sub-parse, where Svelte parses with acorn's
    /// `preserveParens: true` and skips `remove_parens`. Set via
    /// [`crate::parse_embedded_preserve_parens`].
    pub(crate) preserve_parens: bool,
    /// True when parsing inside `declare namespace`/`declare module` (acorn/babel
    /// `inAmbientContext`). Relaxes a few ambient-only grammar rules — notably a
    /// single trailing comma after a rest parameter is tolerated throughout the
    /// subtree (see the rest-comma checks in `parameters.rs`/`types.rs`). It does NOT
    /// force functions bodiless — a plain `function f() {}` inside a `declare
    /// namespace` is an ordinary function declaration (its ambient implementation
    /// error is deferred to diagnostics).
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
    /// The [`Parser::grouping_depth`] at which the current `[~In]` region began —
    /// the baseline the `in`-is-a-binary-operator gate compares against, so the
    /// question it asks is "has a grouping opened **since this for-header
    /// started**?" rather than "is any grouping open anywhere?". Only meaningful
    /// while `allow_in` is false; set (save/restore) by
    /// [`Parser::parse_expression_no_in`]. A plain `== 0` test would read the
    /// enclosing expression's delimiters — `fn(function () { for (k in o) {} })`
    /// parses its header at depth 1 — and take the for-in separator for a
    /// relational `in`.
    no_in_depth: u32,
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
    /// The `[Yield]` grammar context. `true` (`[+Yield]`) inside a generator
    /// function's params **and** body; reset to `false` (`[~Yield]`) on entering
    /// any non-generator function-like scope (a plain function, an arrow, a class
    /// static block or field initializer) and at top level. Unlike `[Await]` it
    /// is never goal-driven — a generator is marked by `*`, not by module vs
    /// script. It gates the bare-`yield`-operand guard (`is_bare_assignment_head`):
    /// a `yield` expression can only be a complete `AssignmentExpression` that no
    /// operator extends where `yield` is actually a `YieldExpression`, i.e. in
    /// `[+Yield]`. Outside a generator `yield` is a (deferred) reserved-word
    /// identifier, so the guard must not fire — see the guard in `expression.rs`.
    in_yield: bool,
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
    /// Hand-off from the constrained-infer parse to `parse_type`: when
    /// `infer U extends C` at an allow-conditional position is directly
    /// followed by `?`, the already-parsed `C` re-binds as the extends clause
    /// of a conditional whose check is the bare `infer U` (acorn rolls back
    /// its constraint tryParse and re-parses `extends C` at the conditional
    /// level; this hand-off reproduces that without re-lexing). Set only when
    /// the current token is `?`; consumed by the innermost enclosing
    /// `parse_type`, which nothing can precede (every intermediate
    /// union/intersection/array/operand loop breaks on `?`).
    pending_conditional_extends: Option<TSType<'arena>>,
}

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Create a parser against an explicit goal symbol. The standalone
    /// `parse`/`parse_with_goal` paths use this; embedders go through
    /// [`Parser::with_base_offset`] (always `Module`).
    fn new_with_goal(source: &'a str, goal: Goal, arena: &'arena Bump) -> Result<Self, ParseError> {
        Self::with_base_offset_and_goal(source, 0, goal, arena)
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

    /// Copy the lexer's just-produced decoded value (escape paths only) into the
    /// AST arena, yielding a `Copy` `&'arena str`. The lexer decodes into a reused
    /// scratch that the next lex overwrites, so this runs immediately after each
    /// lex; `None` for the common escape-free token. One copy into the arena — the
    /// same copy the old owned-`String` path made when it stored the value, now the
    /// only allocation on the escape path.
    #[inline]
    fn decoded_to_arena(&self) -> Option<&'arena str> {
        self.lexer
            .decoded_str()
            .map(|s| -> &'arena str { self.arena.alloc_str(s) })
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

    /// Create a parser at a base offset (for embedded expressions).
    /// Embedded contexts are always modules (Svelte `<script>` is a module), so this
    /// defaults the goal.
    ///
    /// Used when parsing embedded expressions/scripts in Svelte templates.
    /// base_offset is added to all span positions to get correct positions in full source.
    /// Embedded contexts are always modules (Svelte `<script>` is a module), so
    /// this defaults the goal; the goal-aware [`Parser::new_with_goal`] is the
    /// only `Script` entry.
    pub fn with_base_offset(
        source: &'a str,
        base_offset: usize,
        arena: &'arena Bump,
    ) -> Result<Self, ParseError> {
        Self::with_base_offset_and_goal(source, base_offset, Goal::Module, arena)
    }

    /// [`Parser::with_base_offset`] with an explicit goal symbol — the single
    /// constructor that actually builds the parser state.
    fn with_base_offset_and_goal(
        source: &'a str,
        base_offset: usize,
        goal: Goal,
        arena: &'arena Bump,
    ) -> Result<Self, ParseError> {
        // The lexer scans `source` (the island) but reports errors against the document
        // it sits in, so it carries the same `base_offset` the spans below are shifted by.
        let mut lexer = Lexer::at_offset(source, base_offset);
        let mut current = lexer.next_token()?;
        let mut decoded = lexer
            .decoded_str()
            .map(|s| -> &'arena str { arena.alloc_str(s) });

        // Collect leading comment tokens
        let mut comments = BumpVec::new_in(arena);
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
            decoded = lexer
                .decoded_str()
                .map(|s| -> &'arena str { arena.alloc_str(s) });
        }

        Ok(Self {
            arena,
            source,
            lexer,
            current,
            current_decoded: decoded,
            peek: None,
            peek_decoded: None,
            base_offset,
            comments,
            had_line_terminator: false, // No line terminator before first token
            prev_end: 0,
            top_level_as_is_assertion: true, // Enable by default (TypeScript context)
            grouping_depth: 0,               // Not inside any grouping delimiters
            preserve_parens: false,          // Discard grouping parens (paren-free public AST)
            in_ambient_context: false,       // Not in declare namespace/module
            lexer_error: None,               // No stored lexer error
            peek_had_line_terminator: false, // No peek cached yet
            allow_in: true,                  // Allow `in` binary operator by default
            no_in_depth: 0,                  // Only read while `allow_in` is false
            goal,
            // Module top level is `[+Await]` (`ModuleItem[+Await]`); Script top
            // level is `[~Await]` (`ScriptBody[~Await]`).
            in_await: matches!(goal, Goal::Module),
            // Top level is `[~Yield]` in both goals — a generator scope is entered
            // only via `*` on a function/method (see `with_fn_context`).
            in_yield: false,
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
            self.current_decoded = self.decoded_to_arena();
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
        self.current_decoded = self.decoded_to_arena();
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
        self.current_decoded
    }

    /// The current identifier token's name channel — the canonical identifier
    /// name constructor. Span-identity (`escaped: None`, name = the raw token
    /// bytes) unless the token carries a decoded unicode escape
    /// (`\u0066oo` → `foo`) or is too long for `raw_len` — only those rare
    /// cases carry the arena-`&str` escape hatch.
    pub(super) fn current_ident_name(&self) -> IdentName<'arena> {
        if let Some(decoded) = self.current_decoded {
            IdentName {
                escaped: Some(decoded),
                raw_len: 0,
            }
        } else {
            self.current_raw_ident_name()
        }
    }

    /// The current token's name channel from its RAW source text, ignoring any
    /// decoded escape value. For keyword tokens, which are never escaped — the
    /// lexer re-classifies an escaped keyword as an `Identifier`, so its decoded
    /// value flows through `current_ident_name` instead (property/member and
    /// class/interface/type-member keys decode via that path — acorn parity).
    pub(super) fn current_raw_ident_name(&self) -> IdentName<'arena> {
        let len = self.current.end - self.current.start;
        if len > u16::MAX as u32 {
            // Absurdly long name (> 64 KiB): `raw_len` can't hold it, so store
            // the raw source slice arena-copied as the `&'arena str` escape
            // hatch (essentially unreachable — no real identifier is this long).
            let value = self.current_value();
            IdentName {
                escaped: Some(self.arena.alloc_str(value)),
                raw_len: 0,
            }
        } else {
            IdentName {
                escaped: None,
                raw_len: len as u16,
            }
        }
    }

    /// Name channel for the current token, which the caller has already verified
    /// is a plain `Identifier` or a keyword-lexed word valid as a name here — the
    /// [`Parser::at_binding_name`] set, so a contextual keyword or `await` at Script
    /// `[~Await]` (e.g. a class name, single-param arrow param, or `break`/`continue`
    /// label). A plain identifier decodes unicode escapes; a keyword token is never
    /// escaped (the lexer re-classifies an escaped keyword as an `Identifier`), so it
    /// is taken verbatim.
    pub(super) fn current_ident_name_or_await(&self) -> IdentName<'arena> {
        if matches!(self.current_kind(), TokenKind::Identifier) {
            self.current_ident_name()
        } else {
            self.current_raw_ident_name()
        }
    }

    /// Whether a name equals `expected` (an ASCII name like `"this"`) — the shared
    /// core of [`Parser::ident_name_is`] / [`Parser::private_name_is`]. An escaped
    /// an escaped name compares its arena string (so an escaped `this` still matches); a
    /// span-identity name compares the `name_len` raw source bytes at `name_start`
    /// (host coordinates, shifted back to the local slice).
    fn name_bytes_are(
        &self,
        escaped: Option<&str>,
        name_start: usize,
        name_len: usize,
        expected: &str,
    ) -> bool {
        match escaped {
            Some(s) => s == expected,
            None => {
                let start = name_start - self.base_offset;
                self.source.as_bytes().get(start..start + name_len) == Some(expected.as_bytes())
            }
        }
    }

    /// Whether `id`'s name equals `expected` (an ASCII name like `"this"`).
    pub(super) fn ident_name_is(&self, id: &Identifier<'_>, expected: &str) -> bool {
        self.name_bytes_are(
            id.escaped_name,
            id.span.start as usize,
            id.name_len as usize,
            expected,
        )
    }

    /// Whether a private identifier's name (the part after `#`) equals `expected`.
    /// The name begins one byte past the node span's start — the `#` — so it passes
    /// `span.start + 1`. Used to reject the reserved `#constructor` class-element name.
    pub(super) fn private_name_is(&self, pid: &PrivateIdentifier<'_>, expected: &str) -> bool {
        self.name_bytes_are(
            pid.name.escaped,
            pid.span.start as usize + 1,
            pid.name.raw_len as usize,
            expected,
        )
    }

    /// Name channel for the current token as an identifier — the set
    /// `KeywordKind::can_be_identifier` admits (`from`, `as`, `satisfies`, the type
    /// keywords, … plus `let` / `yield` / `await`). Used at positions where an
    /// unconditionally-reserved word would be read as a keyword instead (a
    /// specifier local name, a label, a qualified type name's right side).
    ///
    /// ⚠️ This is a **name-building** channel, not a gate: it is deliberately wider
    /// than [`Parser::at_binding_name`] / [`Parser::at_reference_name`] and applies
    /// neither the goal axis to `await` nor the `[~Yield]` guard to `yield`. Every
    /// caller must already have decided the word is legal here — a label site, for
    /// instance, is admitted by `at_reference_name` at the statement dispatcher, and
    /// calling this without that gate would accept `yield:` inside a generator.
    ///
    /// Handles `TokenKind::Identifier` with unicode escape decoding. Returns `None`
    /// if the current token is not identifier-like.
    ///
    /// ⚠️ For a position the grammar spells `IdentifierName` — where every reserved
    /// word is valid — use [`Parser::try_identifier_name`] instead.
    pub(super) fn try_ident_or_contextual_name(&self) -> Option<IdentName<'arena>> {
        match self.current_kind() {
            TokenKind::Identifier => Some(self.current_ident_name()),
            TokenKind::Keyword(kw) if kw.can_be_identifier() => Some(self.current_raw_ident_name()),
            _ => None,
        }
    }

    /// Name channel for the current token as a function declaration name — a
    /// `BindingIdentifier`, so it asks [`Parser::await_is_binding_name`]: `await`
    /// names a function at `Goal::Script` (whatever the `[Await]` context, that bar
    /// being a deferred early error) and is reserved at `Module`, so `function
    /// await(){}` / `export function await(){}` reject there, matching
    /// acorn-as-module. Other keyword-lexed names (`async`, `from`, `let`, `yield`,
    /// the type keywords) stay valid.
    pub(super) fn try_function_name(&self) -> Option<IdentName<'arena>> {
        if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Await))
            && !self.await_is_binding_name()
        {
            return None;
        }
        self.try_ident_or_contextual_name()
    }

    /// Whether the current token is binding-name-eligible — the allocation-free
    /// classification behind `try_binding_name().is_some()`, without building the
    /// name. Pure `&self` (no allocation), so it is safe inside a `debug_assert!`
    /// (the name-building path `try_binding_name` takes may arena-allocate an
    /// escaped name).
    ///
    /// The set is [`KeywordKind::can_be_binding_name`] plus `await` where the goal
    /// axis makes it an identifier — so `let` and `yield` are in, their only bar
    /// being a strict-mode early error tsv defers.
    ///
    /// This also guards the two **`IdentifierReference` heads** that tsv checks
    /// before committing — a heritage element's `TypeName` (`interface A extends X`,
    /// `class C implements X`) and an import-equals module reference (`import x =
    /// A.B`) — which want the same set for the same reason: in strict mode the bar
    /// on `let`/`yield` is an early error in the reference and binding spellings
    /// alike, while a genuine `ReservedWord` (`void`) is excluded by the
    /// `Identifier` production in both. tsc and prettier accept `let`/`yield` at
    /// both heads; acorn accepts them at the heritage head and rejects them at the
    /// module reference, but it is the shape oracle, not the validity one. The
    /// heritage head once needed a wider predicate of its own purely because this
    /// set was missing `let`.
    ///
    /// ⚠️ An *expression*-position reader wants
    /// [`Parser::keyword_is_expression_identifier`] instead — inside a generator
    /// `yield` is the operator there, which is a production guard rather than a
    /// deferrable early error.
    pub(super) fn at_binding_name(&self) -> bool {
        match self.current_kind() {
            TokenKind::Identifier => true,
            TokenKind::Keyword(kw) if kw.can_be_binding_name() => true,
            TokenKind::Keyword(KeywordKind::Await) => self.await_is_binding_name(),
            _ => false,
        }
    }

    pub(super) fn try_binding_name(&self) -> Option<IdentName<'arena>> {
        match self.current_kind() {
            TokenKind::Identifier => Some(self.current_ident_name()),
            TokenKind::Keyword(kw) if kw.can_be_binding_name() => {
                Some(self.current_raw_ident_name())
            }
            // `await` is a valid `BindingIdentifier` at Script goal, whatever the
            // `[Await]` context: the goal bullet is enforced, the `[Await]` one is a
            // deferred early error like the `[Yield]` twin (`await_is_binding_name`).
            TokenKind::Keyword(KeywordKind::Await) if self.await_is_binding_name() => {
                Some(self.current_raw_ident_name())
            }
            _ => None,
        }
    }

    /// Take the current token as a declaration-name `BindingIdentifier` — an
    /// identifier or a contextual keyword valid as a binding name (`enum string {}`,
    /// `type any = …`, `namespace number {}`) — advancing past it. Returns `Ok(None)`
    /// *without advancing* when the current token can't be a binding name, so the
    /// caller emits its own position-specific error. The single home for the
    /// "a declaration name is a `BindingIdentifier`" policy shared by the interface /
    /// namespace-segment / declare-module / enum / type-alias name parsers. The class
    /// paths capture inline instead (they additionally exclude the heritage-starting
    /// `implements`), and the import path captures the name across a following branch.
    pub(super) fn take_binding_identifier(
        &mut self,
    ) -> Result<Option<Identifier<'arena>>, ParseError> {
        let Some(name) = self.try_binding_name() else {
            return Ok(None);
        };
        let (id_start, id_end) = self.current_pos();
        self.advance()?;
        Ok(Some(Identifier::simple(
            name,
            Span::new(id_start as u32, id_end as u32),
        )))
    }

    /// Whether `await` reads as a plain identifier at an **expression or label**
    /// position: only at `Goal::Script` and outside any `[+Await]` context.
    ///
    /// Both halves are real here, for different reasons. The goal half is the early
    /// error tsv enforces (`IdentifierReference : await` is a Syntax Error when the
    /// goal is `Module`). The `!in_await` half is not a deferral question at all —
    /// `IdentifierReference[Await] : [~Await] await` and `LabelIdentifier[Await] :
    /// [~Await] await` carry the guard **in their productions**, and inside a
    /// `[+Await]` context `await` is the operator, so the identifier reading is
    /// unreachable rather than merely invalid. Exactly parallel to
    /// [`Parser::yield_is_identifier`].
    ///
    /// ⚠️ A **binding** position wants [`Parser::await_is_binding_name`] instead —
    /// there the `[Await]` bar is a deferrable early error, not a production guard.
    pub(super) fn await_is_identifier(&self) -> bool {
        self.goal == Goal::Script && !self.in_await
    }

    /// Whether `await` may name a `BindingIdentifier` here: at `Goal::Script`,
    /// whatever the `[Await]` context.
    ///
    /// [`Parser::await_is_identifier`] minus the `!in_await` half, and the
    /// difference is the same one that lets `yield` be a binding name inside a
    /// generator. §sec-identifiers gives `BindingIdentifier[Yield, Await] :
    /// Identifier | `yield` | `await`` with **no** guard on either word, and writes
    /// both context bars as early errors instead (so ASI cannot split `let ⏎ await
    /// 0;`):
    ///
    /// ```text
    /// BindingIdentifier[Yield, Await] : `await`
    ///   It is a Syntax Error if this production has an [Await] parameter.
    /// ```
    ///
    /// tsv defers that one, exactly as it defers the `[Yield]` twin — so `async
    /// function h() { var await = 1; }` and `async function h(await) {}` parse at
    /// Script goal. Deferring is what the repo's rule prescribes: prettier formats
    /// both, and the bar needs non-local context (the enclosing function's
    /// async-ness), which is the mode-dependent / non-local class tsv defers rather
    /// than the unconditional-local class it rejects. tsc's parser agrees — its
    /// `isBindingIdentifier` deliberately admits `await`/`yield` and leaves the bar
    /// to the grammar checker (TS1359), the same bucket as TS1212 `let`. acorn
    /// rejects, but it is the shape oracle, not the validity one.
    ///
    /// The **goal** bullet is a different early error and stays enforced: `await` is
    /// a Syntax Error as a name when the goal is `Module`, which is what makes the
    /// goal axis observable at all.
    pub(super) fn await_is_binding_name(&self) -> bool {
        self.goal == Goal::Script
    }

    /// Whether `yield` reads as a plain identifier here rather than as the operator:
    /// outside a generator only.
    ///
    /// This is the `[~Yield]` guard that `IdentifierReference` and `LabelIdentifier`
    /// carry **in their productions** — and that `BindingIdentifier` deliberately does
    /// NOT (ecma262 §sec-identifiers gives it `Identifier | `yield` | `await`` with no
    /// guard and writes the `[Yield]` bar as an early error instead, so ASI cannot
    /// split `let ⏎ await 0;`). That asymmetry is the whole reason the three channels
    /// differ: `function* g() { var yield = 1; }` parses (binding, deferred early
    /// error) while `function* g() { o = { yield }; }` and `function* g() { yield: ; }`
    /// reject (reference / label, production guard). Real tsc and test262 both draw the
    /// line there. The `await` counterpart is [`Parser::await_is_identifier`].
    pub(super) fn yield_is_identifier(&self) -> bool {
        !self.in_yield
    }

    /// Whether `kw` reads as a plain identifier at an **expression** position —
    /// [`KeywordKind::can_be_binding_name`] narrowed by the one word whose
    /// expression reading differs from its binding reading (see
    /// [`Parser::yield_is_identifier`]).
    ///
    /// `await` is outside this set entirely: its callers ask
    /// [`Parser::await_is_identifier`] on arms of their own, ordered after the arm
    /// that calls this.
    pub(super) fn keyword_is_expression_identifier(&self, kw: KeywordKind) -> bool {
        kw.can_be_binding_name()
            && (!matches!(kw, KeywordKind::Yield) || self.yield_is_identifier())
    }

    /// Whether the current token may name an `IdentifierReference` **or** a
    /// `LabelIdentifier` — one predicate because ecma262 gives the two productions
    /// character-for-character identical guards:
    ///
    /// ```text
    /// IdentifierReference[Yield, Await] : Identifier | [~Yield] `yield` | [~Await] `await`
    /// LabelIdentifier[Yield, Await]     : Identifier | [~Yield] `yield` | [~Await] `await`
    /// ```
    ///
    /// [`Parser::at_binding_name`] with both context guards *applied* rather than
    /// deferred, and that is the whole difference between the two channels:
    /// `BindingIdentifier` carries no guard and puts the same bars in the early
    /// errors, which tsv defers. So inside a generator or async function the binding
    /// channel admits `yield` / `await` (`function* g() { var yield = 1; }`) while
    /// this one does not (`function* g() { yield: ; }`, `async function h() { await:
    /// ; }`). A guard is not a deferrable early error: in a `[+Yield]` / `[+Await]`
    /// context the word is the **operator**, so the name reading is unreachable, not
    /// merely invalid.
    ///
    /// The two non-label callers are the `IdentifierReference` heads tsv checks
    /// before committing — a heritage element's `TypeName` (`interface A extends X`,
    /// `class C implements X`) and an import-equals module reference (`import x =
    /// A.B`). tsc draws the same line by parsing heritage with its *expression*
    /// parser, so `function* g() { interface A extends yield {} }` and `async
    /// function h() { interface A extends await {} }` are TS1109 there — while a
    /// plain type annotation (`function* g() { let x: yield; }`) is fine, since that
    /// one never reaches an expression parser in either implementation.
    pub(super) fn at_reference_name(&self) -> bool {
        self.at_binding_name()
            && match self.current_kind() {
                TokenKind::Keyword(KeywordKind::Yield) => self.yield_is_identifier(),
                TokenKind::Keyword(KeywordKind::Await) => self.await_is_identifier(),
                _ => true,
            }
    }

    /// Enter a grouping delimiter — `(…)`, `[…]`, `{…}`, `${…}` — having just
    /// consumed its opener. Pairs with [`Parser::exit_grouping`] on the closer.
    ///
    /// The two calls are what maintain [`Parser::grouping_depth`], and every
    /// consumer of that counter is documented on the field: a *baseline* decides
    /// what a reading means, and the two questions it answers use different ones
    /// ([`Parser::no_in_depth`] vs the parse root's literal 0). Nine pairs across
    /// `expression.rs` / `expression_literals.rs` / `expression_template.rs` are
    /// hand-balanced across early returns and are deliberately NOT unwound on the
    /// error path — inert while the parser never backtracks, since a rejected
    /// parse propagates straight out. Wrapping them in a combinator (so the pair
    /// is balanced by construction) wants body extraction at several sites and is
    /// its own change.
    #[inline]
    pub(super) fn enter_grouping(&mut self) {
        self.grouping_depth += 1;
    }

    /// Leave a grouping delimiter, having just consumed its closer. See
    /// [`Parser::enter_grouping`].
    #[inline]
    pub(super) fn exit_grouping(&mut self) {
        debug_assert!(
            self.grouping_depth > 0,
            "exit_grouping without a matching enter_grouping"
        );
        self.grouping_depth -= 1;
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

    /// Run `f` with the function-like scope's `[Await]` and `[Yield]` contexts
    /// set to `is_async` / `is_generator`, restoring both afterward (on success
    /// and error alike). Wrap a function-like scope's params+body so a nested
    /// scope establishes its own `await`/`yield` context without leaking to (or
    /// inheriting from) the enclosing one: an async generator's body is
    /// `[+Await, +Yield]`, a plain function or an arrow inside a generator is
    /// `[~Yield]`, etc. Both flags reset together because they share the same
    /// boundary — every function-like scope — so threading them as one call makes
    /// it impossible to set one and forget the other.
    pub(super) fn with_fn_context<T>(
        &mut self,
        is_async: bool,
        is_generator: bool,
        f: impl FnOnce(&mut Self) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        let saved_await = std::mem::replace(&mut self.in_await, is_async);
        let saved_yield = std::mem::replace(&mut self.in_yield, is_generator);
        let result = f(self);
        self.in_await = saved_await;
        self.in_yield = saved_yield;
        result
    }

    /// Run `f` with the function-type-disallowed context set to `value`,
    /// restoring it afterward (on success and error alike). A single-flag
    /// `with_context_flag` wrapper. Set `true` around union/intersection constituent and
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
    /// parse. Wraps the full-type entry (`parse_type`).
    pub(super) fn with_full_type_context<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        self.with_fn_type_disallowed(false, |p| p.with_conditional_type_disallowed(false, f))
    }

    /// Like `try_binding_name`, but also accepts the `this` keyword as the
    /// TypeScript `this` parameter (`function f(this: T)`, `(this: T) => U`).
    pub(super) fn try_param_name(&self) -> Option<IdentName<'arena>> {
        match self.try_binding_name() {
            Some(name) => Some(name),
            None => self.this_as_name(),
        }
    }

    /// The `this` keyword used as a name channel — the TypeScript `this`
    /// parameter (`function f(this: T)`) and the subject of a `this` type
    /// predicate (`this is T`, `asserts this`). `None` when the current token is
    /// not `this`; its raw text is verbatim (`this` is never escaped).
    pub(super) fn this_as_name(&self) -> Option<IdentName<'arena>> {
        if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::This)) {
            Some(self.current_raw_ident_name())
        } else {
            None
        }
    }

    /// Name channel for the current token as an `IdentifierName`, accepting ANY
    /// keyword — `ReservedWord` is a subset of `IdentifierName`, so a position
    /// spelled `IdentifierName` in the grammar takes `if` / `default` / `class`
    /// as freely as `a`.
    ///
    /// Two such positions, both single-token productions where the reserved word
    /// cannot be misread as a keyword:
    ///
    /// - `ModuleExportName : IdentifierName | StringLiteral` (`export { x as if }`)
    ///   — this handles only the `IdentifierName` arm; callers test for
    ///   `TokenKind::String` first and build a `ModuleExportName::Literal`.
    /// - `PrivateIdentifier :: # IdentifierName` (`#default`).
    ///
    /// ⚠️ Not to be confused with [`Parser::try_ident_or_contextual_name`], which is
    /// *narrower* despite the broader-sounding name: it admits only the contextual
    /// keywords. Reaching for that one at an `IdentifierName` position is what made
    /// `#default` an over-rejection.
    pub(super) fn try_identifier_name(&self) -> Option<IdentName<'arena>> {
        match self.current_kind() {
            TokenKind::Identifier => Some(self.current_ident_name()),
            TokenKind::Keyword(_) => Some(self.current_raw_ident_name()),
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
        match self.current_decoded {
            Some(decoded) => StringCooked::Decoded(decoded),
            None => StringCooked::Verbatim,
        }
    }

    // Error construction helpers - reduce boilerplate for common error patterns

    /// Create an error with custom message at current position
    pub(super) fn error_msg(&self, message: &str) -> ParseError {
        ParseError::invalid_syntax(message.to_string(), self.current_pos().0)
    }

    /// Create an error with custom message at custom position
    pub(super) fn error_msg_at(&self, message: &str, position: usize) -> ParseError {
        ParseError::invalid_syntax(message.to_string(), position)
    }

    /// Create an error: "Expected X"
    pub(super) fn error_expected(&self, what: &str) -> ParseError {
        ParseError::invalid_syntax(format!("Expected {what}"), self.current_pos().0)
    }

    /// Create an error: "Expected X, found Y"
    pub(super) fn error_expected_found(&self, what: &str) -> ParseError {
        let kind = &self.current.kind;
        ParseError::invalid_syntax(
            format!("Expected {what}, found {kind}"),
            self.current_pos().0,
        )
    }

    /// Create an error: "Expected X, found Y" at custom position
    pub(super) fn error_expected_found_at(&self, what: &str, position: usize) -> ParseError {
        let kind = &self.current.kind;
        ParseError::invalid_syntax(format!("Expected {what}, found {kind}"), position)
    }

    /// Create an error: "Expected X after Y, found Z"
    pub(super) fn error_expected_after(&self, what: &str, after: &str) -> ParseError {
        let kind = &self.current.kind;
        ParseError::invalid_syntax(
            format!("Expected {what} after '{after}', found {kind}"),
            self.current_pos().0,
        )
    }

    /// Create an error: "Unexpected keyword 'X'"
    pub(super) fn error_unexpected_keyword(&self, kw: KeywordKind) -> ParseError {
        ParseError::invalid_syntax(format!("Unexpected keyword '{kw}'"), self.current_pos().0)
    }

    /// Create an error for the `with` statement — sloppy-mode only, and tsv parses
    /// strict mode only. Named rather than folded into `error_unexpected_keyword` so the
    /// message says *why* the word is refused; a bare "unexpected keyword" reads like a
    /// parser gap for a construct that is deliberately out of scope.
    pub(super) fn error_with_statement(&self) -> ParseError {
        ParseError::invalid_syntax(
            "The 'with' statement is not allowed in strict mode".to_owned(),
            self.current_pos().0,
        )
    }

    /// Create an error: "Expected 'X' or 'Y' after list element, found Z"
    pub(super) fn error_list_separator(
        &self,
        separator: &TokenKind,
        terminator: &TokenKind,
    ) -> ParseError {
        let kind = &self.current.kind;
        ParseError::invalid_syntax(
            format!("Expected '{separator}' or '{terminator}' after list element, found {kind}"),
            self.current_pos().0,
        )
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
                        self.peek_decoded = self.decoded_to_arena();
                    }
                    Err(err) => {
                        // Store error to be returned on next advance().
                        self.lexer_error = Some(err);
                    }
                }
                break;
            }
        }
        self.peek
            .as_ref()
            .map_or(TokenKind::Eof, |t| t.kind.clone())
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
        let kind = self.peek_kind();
        kind.is_binding_name_word()
            || matches!(
                kind,
                TokenKind::BracketOpen
                    | TokenKind::BraceOpen
                    | TokenKind::DotDotDot
                    | TokenKind::Keyword(KeywordKind::This)
            )
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
        self.source[from..to].contains(is_es_line_terminator)
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

    /// Whether the peeked token is a same-line *declaration name word* — a plain
    /// identifier or a keyword-lexed word valid as a binding name (`string`,
    /// `number`, `any`, … plus `let` / `yield`; the set
    /// `KeywordKind::can_be_binding_name` accepts). Used by the
    /// `interface`/`namespace`/`module` dispatch, which commits to a declaration
    /// only when a name follows on the same line (tsc
    /// `nextTokenIsIdentifier…OnSameLine`, whose `isBindingIdentifier` is likewise
    /// true for the contextual keywords *and* for the strict-mode-reserved words it
    /// defers). So `interface let {}` / `namespace let {}` commit, matching tsc; a
    /// word barred by a *production* rather than an early error (`void`, `enum`)
    /// fails the gate, leaving the contextual keyword an ordinary identifier.
    /// Mirrors the `try_binding_name` name-capture predicate the declaration
    /// parsers use for the name itself.
    pub(super) fn peek_is_same_line_name_word(&mut self) -> bool {
        self.peek_kind().is_binding_name_word() && !self.peek_preceded_by_line_terminator()
    }

    /// Demand a same-line name for a contextual declaration head already committed to
    /// — `interface` / `type` / `namespace` / `module` behind `declare` or `export`.
    ///
    /// The statement path asks [`Self::peek_is_same_line_name_word`] one token earlier,
    /// where failing it DEMOTES the head to a plain identifier and ASI splits the
    /// statement. Behind a modifier there is no demotion to fall back on — the modifier
    /// would be left with nothing to attach to — so the same question becomes an error
    /// rather than a fork, and every oracle agrees there is no reading: tsc rejects
    /// (TS1434, or TS1142 "Line break not permitted here" for `declare type`), as do
    /// acorn and prettier. Without it the two lines welded into ONE declaration and the
    /// line terminator simply vanished, tsv alone.
    ///
    /// `head` names the construct for the message ONLY — the string-name allowance is
    /// read off the token, never off this word, so a reworded message cannot change
    /// what parses. One helper for all four heads so they cannot drift apart — they
    /// had, which is how `declare interface` and `declare type` kept welding while
    /// `declare namespace` did not.
    pub(super) fn require_same_line_declaration_name(
        &mut self,
        head: &str,
    ) -> Result<(), ParseError> {
        if self.peek_is_same_line_declaration_name() {
            Ok(())
        } else {
            Err(self.same_line_declaration_name_error(head))
        }
    }

    /// The question [`Self::require_same_line_declaration_name`] asks, split out for the
    /// one caller that must ask it *before* consuming the head keyword: `export type`
    /// decides between the alias and the `{`/`*` re-export forms only after the advance,
    /// and those two forms take the break in every oracle (tsc's
    /// `canFollowExportModifier` exempts exactly `*` and `{`).
    ///
    /// Asked while `current` is still the head keyword, which is what lets it read the
    /// string-name allowance from the token: only `module` takes one
    /// (`declare module 'x' {}`).
    pub(super) fn peek_is_same_line_declaration_name(&mut self) -> bool {
        self.peek_is_same_line_name_word()
            || (self.current_value() == "module"
                && self.peek_kind() == TokenKind::String
                && !self.peek_preceded_by_line_terminator())
    }

    /// The error [`Self::require_same_line_declaration_name`] raises, so the split
    /// caller above reports it in the same words.
    pub(super) fn same_line_declaration_name_error(&self, head: &str) -> ParseError {
        self.error_msg(&format!("{head} name must be on the same line"))
    }

    /// Whether a statement-initial `let` heads a `LexicalDeclaration` rather than an
    /// `ExpressionStatement` — tsc's `isLetDeclaration`
    /// (`nextTokenIsBindingIdentifierOrStartOfDestructuring`): a binding name, `{`,
    /// or `[` follows.
    ///
    /// `let` is the one word that is neither a `ReservedWord` nor purely contextual:
    /// `Identifier : IdentifierName but not ReservedWord` admits it, so `let` is a
    /// perfectly good `IdentifierReference` (`let;`, `x = let`, `let.x = 1`,
    /// `typeof let`), barred only by the strict-mode early error tsv defers. What
    /// keeps the two readings apart is a *lookahead*, and ecma262 spells one half of
    /// it out — `ExpressionStatement` carries `[lookahead ∉ { …, `let` `[` }]`, so
    /// `let [` can never begin an expression statement. That is why `let[0] = 1` is
    /// **not** an indexed assignment but a declaration with an invalid array binding
    /// pattern, and stays a syntax error (tsc TS1181) even though `let.x = 1` parses.
    ///
    /// ⚠️ A **for-head** deliberately does NOT ask this: `for (let …)` commits to a
    /// declaration on the keyword alone, which is exactly what tsc does
    /// (`parseForOrForInOrForOfStatement` tests `token() === LetKeyword` with no
    /// lookahead), so `for (let[0] of a)` and `for (let.x of a)` both stay rejected.
    pub(super) fn at_let_declaration(&mut self) -> bool {
        let kind = self.peek_kind();
        kind.is_binding_name_word()
            || matches!(kind, TokenKind::BraceOpen | TokenKind::BracketOpen)
            || (matches!(kind, TokenKind::Keyword(KeywordKind::Await))
                && self.await_is_binding_name())
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
        self.peek_is_identifier_or_keyword()
            && !self.peek_preceded_by_line_terminator()
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
            // `declare async function` — the one starter carrying a `[no
            // LineTerminator here]` of its OWN, so the ambient reading is refused
            // here when `function` does not sit on the `async`'s line. Asking a
            // token later is what keeps this out of the `declare abstract⏎class`
            // trap (see `parse_declare_statement_kind`), where the ambient reading
            // is already committed and can no longer split into statements.
            TokenKind::Keyword(KeywordKind::Async) => {
                self.peek_followed_by_same_line_function_keyword()
            }
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
        scan::identifier_starts_at(bytes, pos)
            && !self.source[after_peek..pos].contains(is_es_line_terminator)
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

    /// Whether the peeked token is followed on the same line by the `function`
    /// keyword.
    ///
    /// Used for `declare async [no LineTerminator here] function`, where the
    /// keyword sits one token past the peek horizon. tsc's modifier lookahead
    /// bails on a break before `function`, so `declare async⏎function f(): void;`
    /// is not one ambient async signature there — and asking a token ahead is what
    /// lets that reading be declined *before* the `declare` dispatch commits to one
    /// it could no longer undo, the trap `declare abstract⏎class` sits in.
    ///
    /// What becomes of the residue is the ordinary expression-statement path's ASI
    /// question, which this predicate does not touch: tsc recovers into three
    /// statements, tsv rejects — as it did before `declare async` was accepted at all.
    pub(super) fn peek_followed_by_same_line_function_keyword(&mut self) -> bool {
        self.peek_kind(); // populate the cache
        let after_peek = self.peek.as_ref().map_or(0, |t| t.end as usize);
        let bytes = self.source.as_bytes();
        let pos = scan::skip_whitespace_and_comments(bytes, after_peek);
        pos < bytes.len()
            && !self.source[after_peek..pos].contains(is_es_line_terminator)
            && &bytes[pos..scan::skip_identifier(bytes, pos)] == b"function"
    }

    /// Whether the cursor sits at a `using` declaration head — the contextual
    /// keyword followed on the same line by a binding word (`using [no
    /// LineTerminator here] BindingIdentifier`; a break, or an
    /// expression-continuation word, demotes `using` to a plain identifier —
    /// `peek_is_same_line_binding_word` spells both).
    ///
    /// One question, one predicate: every dispatch site asks exactly this, and a
    /// for head then adds only its own `[lookahead ≠ of]`.
    pub(super) fn at_using_declaration(&mut self) -> bool {
        *self.current_kind() == TokenKind::Identifier
            && self.current_value() == "using"
            && self.peek_is_same_line_binding_word()
    }

    /// Whether the cursor sits at an `await using` declaration head. **Both** gaps
    /// carry `[no LineTerminator here]` — `await [no LT] using [no LT]
    /// BindingIdentifier` — so a break at either leaves an `await using`
    /// *expression* behind; a statement then splits under ASI, while a for head,
    /// having no ASI, is a syntax error. Splitting this question across the two
    /// dispatch sites is what let the for head lose both restrictions.
    ///
    /// Whether `await` may be an ordinary identifier here (`Goal::Script`) is the
    /// caller's question, not this one's — statement dispatch settles it first
    /// via `await_is_identifier`.
    pub(super) fn at_await_using_declaration(&mut self) -> bool {
        *self.current_kind() == TokenKind::Keyword(KeywordKind::Await)
            && self.peek_is_same_line_identifier()
            && self.peek_value() == "using"
            && self.peek_followed_by_same_line_binding_word()
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

    /// Peek sibling of `current_is_identifier_or_keyword`: whether the next token
    /// is an identifier or any keyword (acorn/tsc `tokenIsIdentifierOrKeyword`).
    pub(super) fn peek_is_identifier_or_keyword(&mut self) -> bool {
        matches!(
            self.peek_kind(),
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

    /// Consume the current `IdentifierName` token — an identifier or any keyword —
    /// as an [`Identifier`] node spanning exactly that token.
    ///
    /// The consuming counterpart of [`Parser::current_is_identifier_or_keyword`],
    /// shared by the *key* positions that take an `IdentifierName`: the property
    /// after `.` / `?.` (`obj.class`, including a `new` callee's chain), a class
    /// member key, and a type-member key. `\u` escapes decode (`x.a` → name
    /// `a`; ecma262 `IdentifierName` StringValue) — acorn parity.
    ///
    /// # Precondition
    /// Current token must satisfy [`Parser::current_is_identifier_or_keyword`], so
    /// callers own the "not a name here" error and its wording.
    pub(super) fn parse_identifier_name_node(&mut self) -> Result<Identifier<'arena>, ParseError> {
        debug_assert!(self.current_is_identifier_or_keyword());
        let (start, end) = self.current_pos();
        let name = self.current_ident_name();
        self.advance()?;
        Ok(Identifier::simple(
            name,
            Span::new(start as u32, end as u32),
        ))
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
    /// Current token must be `#`. ecma262 `PrivateIdentifier :: # IdentifierName`
    /// is a single *lexical* token, which pins both halves of what may follow:
    ///
    /// - the name is an `IdentifierName`, so **every** reserved word is a valid
    ///   private name (`#default`, `#class`, `#true`) — not just the contextual
    ///   keywords a `BindingIdentifier` admits. The one reserved private name,
    ///   `#constructor`, is rejected by the `ClassElementName` early error at the
    ///   class-member key site, on decoded StringValue.
    /// - the name is *glued* to the `#`: `# a`, `#/*c*/a` and `#⏎a` are two tokens,
    ///   not a private name (acorn rejects them in the lexer). Without the check
    ///   tsv accepted them and reprinted `#a` — rewriting invalid code as valid.
    ///
    /// Returns the PrivateIdentifier with span including the `#`.
    pub(super) fn parse_private_identifier(
        &mut self,
    ) -> Result<PrivateIdentifier<'arena>, ParseError> {
        debug_assert!(matches!(self.current_kind(), TokenKind::Hash));
        let (start, hash_end) = self.current_pos();
        self.advance()?; // consume '#'

        // The `IdentifierName` name channel decodes `\u` escapes, so an escaped
        // spelling names the same private name (`#\u0061` is `#a`) — acorn parity.
        let (name_start, end) = self.current_pos();
        let Some(name) = self.try_identifier_name() else {
            return Err(self.error_expected_after("identifier", "#"));
        };
        if name_start != hash_end {
            return Err(self.error_msg_at("A private name must follow '#' immediately", hash_end));
        }
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
            Err(self.unexpected_token_err(kind))
        }
    }

    /// Construct the `UnexpectedToken` error for a failed `expect`. Cold-outlined
    /// out of the hot `expect` body: valid input never takes this branch, so the
    /// two `to_string` allocations + struct build shouldn't inflate the caller.
    #[cold]
    #[inline(never)]
    fn unexpected_token_err(&self, kind: &TokenKind) -> ParseError {
        ParseError::unexpected_token(
            kind.to_string(),
            self.current.kind.to_string(),
            self.current_pos().0,
        )
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
                self.current_decoded = self.decoded_to_arena();
                // Clear peek cache since token changed
                self.peek = None;
                self.peek_decoded = None;
                Ok(())
            }
            _ => Err(ParseError::unexpected_token(
                "'>'".to_string(),
                format!("'{}'", self.current.kind),
                self.current_pos().0,
            )),
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
            _ => Err(ParseError::unexpected_token(
                "'<'".to_string(),
                format!("'{}'", self.current.kind),
                self.current_pos().0,
            )),
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

    /// Whether a type-predicate `is` follows the current subject token — the
    /// `parameterName [no LineTerminator here] is Type` rule (a newline before
    /// `is` makes it a stray token, matching acorn-typescript's
    /// `hasPrecedingLineBreak` guard, like the arrow `=>` / conditional
    /// `extends`). Assumes the current token is the predicate subject
    /// (`x` / `this`). Shared by the return-position predicate parser and the
    /// `this is T` primary-type sites. Does not consume any tokens (only peeks).
    #[inline]
    pub(super) fn peek_predicate_is_ahead(&mut self) -> bool {
        self.peek_is_contextual_keyword("is") && !self.peek_preceded_by_line_terminator()
    }

    /// Eat a leading `asserts` type-predicate keyword, committing only when an
    /// identifier or keyword follows it. Mirrors tsc's `parseNonArrayType`
    /// dispatch (`parser.ts`): `asserts` is a type-predicate prefix iff
    /// `nextTokenIsIdentifierOrKeyword` — otherwise (punctuation/literal: `{`,
    /// `[`, `<`, `.`, `|`, `&`, `;`, …) it stays an ordinary type name
    /// (`: asserts`, `: asserts[]`, `: asserts<T>`, `: asserts.Foo`), left
    /// unconsumed for the caller to parse as a regular type. When a *reserved*
    /// keyword follows (`asserts extends …`), the prefix commits and the caller
    /// then rejects it as a missing parameter name — matching tsc, which parses
    /// the keyword as the asserted identifier and errors.
    /// The asserted name must also be on the SAME line: tsc gates the whole reading on
    /// `lookAhead(nextTokenIsIdentifierOrKeywordOnSameLine)`, so across a break `asserts`
    /// is the ordinary type reference of that name and what follows begins nothing valid
    /// (TS1434; prettier rejects with it). acorn instead welds into the very
    /// `TSTypePredicate` it builds for the same-line spelling, discarding the line
    /// terminator — the shape target is not the validity oracle, so tsv follows tsc.
    /// Per ecma262 §sec-comments a block comment holding a line terminator IS one, so
    /// `asserts /*⏎*/ a` declines on the same rule.
    pub(super) fn eat_type_predicate_asserts(&mut self) -> bool {
        matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == "asserts"
            && self.peek_is_identifier_or_keyword()
            && !self.peek_preceded_by_line_terminator()
            && self.try_advance()
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

        Ok(Program {
            body: body.into_bump_slice(),
            comments: self.take_comments(),
            span: Span::new(start as u32, end as u32),
            goal: self.goal,
        })
    }

    /// Parse a single expression, WITHOUT requiring it to fill the input slice — the raw
    /// parse the pattern path builds on: `parse_pattern_with_comments` parses here, converts
    /// the result to a binding pattern, reads an optional `: Type`, then enforces
    /// end-of-input itself. (Expression tags use `parse_expression_with_comments`, which
    /// requires full consumption via `expect_end_of_input`.)
    pub fn parse_expression_unbounded(&mut self) -> Result<Expression<'arena>, ParseError> {
        self.parse_expression()
    }

    /// Require that parsing has consumed the whole input slice — the current token must
    /// be `Eof`. Trailing trivia (comments, whitespace) is consumed by the lexer, so only
    /// a stray *token* trips this.
    ///
    /// The Svelte embedders parse an expression or pattern from a slice bounded by `}`
    /// (`{@html expr}`, `{@const id = init}`, `{:then pattern}`, …) and require it to fill
    /// the slice exactly — Svelte's `eat('}', true)`. Without this check a trailing token
    /// is silently dropped (`{@html a b}` → `{@html a}`), which loses content rather than
    /// diverging visibly.
    pub fn expect_end_of_input(&self) -> Result<(), ParseError> {
        if self.current.kind == TokenKind::Eof {
            Ok(())
        } else {
            Err(self.error_expected_found("end of input"))
        }
    }

    /// Parse a single expression and return it with any collected comments.
    /// Used for expressions in Svelte templates where comments need to be preserved.
    /// The expression must fill the whole slice (see `expect_end_of_input`).
    pub fn parse_expression_with_comments(
        &mut self,
    ) -> Result<(Expression<'arena>, &'arena [Comment]), ParseError> {
        let expr = self.parse_expression()?;
        self.expect_end_of_input()?;
        let comments = self.take_comments();
        Ok((expr, comments))
    }

    /// Hand the collected comments to the caller as an arena slice.
    /// Used when parsing expressions that need to return comments to the caller.
    pub fn take_comments(&mut self) -> &'arena [Comment] {
        std::mem::replace(&mut self.comments, BumpVec::new_in(self.arena)).into_bump_slice()
    }

    /// Parse a single assignment expression and return position where parsing stopped.
    ///
    /// Unlike `parse_expression_unbounded()`, this stops at top-level commas.
    /// This is useful for parsing expressions embedded in contexts where commas
    /// have other meanings (like `{#each items as pattern, index}`).
    ///
    /// `top_level_as` decides whether a top-level `as` belongs to this parse or to the
    /// host's own grammar — see [`TopLevelAs`], which states the rule and names both
    /// callers. `satisfies` is unaffected either way.
    ///
    /// Returns (expression, end_position) where end_position is where the next
    /// unparsed content begins (in absolute source coordinates with base_offset).
    pub fn parse_assignment_expression_partial(
        &mut self,
        top_level_as: TopLevelAs,
    ) -> Result<(Expression<'arena>, usize), ParseError> {
        let saved = std::mem::replace(
            &mut self.top_level_as_is_assertion,
            top_level_as == TopLevelAs::Assertion,
        );
        let result = self.parse_assignment_expression();
        self.top_level_as_is_assertion = saved;

        let expr = result?;
        // Return the start of the current (unconsumed) token
        let next_pos = self.current.start as usize + self.base_offset;
        Ok((expr, next_pos))
    }

    /// Check if the current token is a colon.
    pub fn at_colon(&self) -> bool {
        matches!(self.current.kind, TokenKind::Colon)
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
    /// Most callers first confirm a `String` token, but the module `from`/source
    /// paths (`export * from …`, `export { … } from …`) reach here directly on
    /// arbitrary input, so a non-`String` token is returned as a clean parse error
    /// rather than panicking (debug) or mis-extracting a "string" from the wrong
    /// token (release, where a bare `debug_assert!` would be elided). Surfaced by
    /// the `fuzz` gate (`tsv_debug fuzz`).
    pub(super) fn parse_string_literal(&mut self) -> Result<Literal<'arena>, ParseError> {
        if !matches!(self.current_kind(), TokenKind::String) {
            return Err(self.error_expected("string literal"));
        }

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

/// Build a standalone [`Parser`] against `goal`, run `f`. What `f` returns
/// borrows only `arena`. Error context (`with_context`) is applied by the public
/// `crate::parse*` wrappers, not here.
fn parse_with<'arena, T>(
    source: &str,
    goal: Goal,
    arena: &'arena Bump,
    f: impl FnOnce(&mut Parser<'_, 'arena>) -> Result<T, ParseError>,
) -> Result<T, ParseError> {
    let mut parser = Parser::new_with_goal(source, goal, arena)?;
    f(&mut parser)
}

/// [`parse_typescript`] against an explicit goal symbol. `parse_typescript` is
/// the `Goal::Module` form.
// `Parser::parse` (clippy's method-path suggestion) fails the higher-ranked
// lifetime check on `parse_with`'s `f` bound; the closure infers it.
#[allow(clippy::redundant_closure_for_method_calls)]
pub fn parse_typescript_with_goal<'arena>(
    source: &str,
    goal: Goal,
    arena: &'arena Bump,
) -> Result<Program<'arena>, ParseError> {
    parse_with(source, goal, arena, |parser| parser.parse())
}

/// [`parse_typescript`] with grouping parens preserved as `ParenthesizedExpression`
/// nodes (acorn's `preserveParens: true`). Standalone analog of
/// [`crate::parse_embedded_preserve_parens`] — the binding audit
/// (`tsv_debug binding_audit`) reparses formatted output this way so the paren
/// structure a glued comment binds to is visible in the wire JSON.
pub fn parse_typescript_preserve_parens<'arena>(
    source: &str,
    arena: &'arena Bump,
) -> Result<Program<'arena>, ParseError> {
    parse_with(source, Goal::Module, arena, |parser| {
        parser.preserve_parens = true;
        parser.parse()
    })
}
