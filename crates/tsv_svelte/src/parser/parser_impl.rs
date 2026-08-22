// SvelteParser struct and helper methods

use crate::ast::internal::{self, FragmentNode};
use crate::lexer::{Lexer, Token, TokenKind};
use crate::parser::element::tag_name_end;
use crate::whitespace::is_svelte_ws;
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use tsv_lang::{Comment, ParseError, PrefixLines, Span};
use tsv_ts::Expression;
use tsv_ts::TSTypeAnnotation;
use tsv_ts::TopLevelAs;
use tsv_ts::{is_id_continue, is_id_start};

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
        multiline: Comment::content_is_multiline(is_block, content),
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
    pub(crate) base_offset: usize, // Offset of lexer's source in full source
    /// TS comments collected from template expressions (e.g., {@debug /* comment */ a})
    pub(crate) expression_comments: Vec<Comment>,
    /// Every embedded acorn parse, in the order the reads happen — which is
    /// source order, so `Root.acorn_regions` needs no sort. See
    /// [`record_acorn_region`](Self::record_acorn_region).
    ///
    /// Bump-allocated because `Root` wants an `&'arena [_]`: growing in the
    /// arena hands the finished run straight over (`into_bump_slice`), where a
    /// heap `Vec` owes a malloc, a free, and a copy into the arena at the end.
    pub(crate) acorn_regions: BumpVec<'arena, internal::AcornRegion>,
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

/// A point in the parser's embedded-parse ledgers to rewind to — see
/// [`SvelteParser::embedded_parse_mark`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct EmbeddedParseMark {
    comments: usize,
    acorn_regions: usize,
}

/// Svelte's reserved-word list (`RESERVED_WORDS`, `svelte/src/utils.js`): the JS
/// keywords, the strict-mode future-reserved set, and `eval` / `arguments`.
///
/// `read_identifier` rejects every one (`e.unexpected_reserved_word`), at PARSE time and
/// regardless of mode — so this is a rule of Svelte's own template grammar, not a JS early
/// error, which is why it is enforced here rather than deferred the way tsv's TypeScript
/// parser defers "reserved word as identifier" (root `CLAUDE.md` §Strict Mode Only).
///
/// Canonical calls `read_identifier` from **six** positions and every one takes this rule:
/// a `{#snippet}` name and an `{#each}` index (`1-parse/state/tag.js`), the
/// plain-identifier binding of `{#each … as p}` / `{:then p}` / `{:catch p}` — `read_pattern`
/// opens with `parser.read_identifier()` (`1-parse/read/context.js:16`) — and a shorthand
/// attribute `{name}` (`1-parse/state/element.js:575`).
///
/// ⚠️ Only `read_pattern`'s **destructuring** branch (`{`/`[`) falls through to acorn, and
/// that is the sole position where the deferral is right. Reading the split the other way
/// round — "a `read_pattern` position goes to acorn" — is wrong for the binding shape
/// people actually write, and left four of the six positions unguarded.
/// [`SvelteParser::read_identifier`] is tsv's single reader for the six.
fn is_reserved_word(name: &str) -> bool {
    matches!(
        name,
        "arguments"
            | "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "eval"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "implements"
            | "import"
            | "in"
            | "instanceof"
            | "interface"
            | "let"
            | "new"
            | "null"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "return"
            | "static"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
    )
}

impl<'a, 'arena> SvelteParser<'a, 'arena> {
    pub(crate) fn new(source: &'a str, arena: &'arena Bump) -> Result<Self, ParseError> {
        let mut lexer = Lexer::at_offset(source, 0);
        // Extract token data immediately to avoid keeping token alive
        let (kind, start, end) = {
            let token = lexer.next_token()?;
            (token.kind, token.start as usize, token.end as usize)
        };
        Ok(Self {
            arena,
            source,
            lexer,
            current_kind: kind,
            current_start: start,
            current_end: end,
            peek: None,
            base_offset: 0,
            expression_comments: Vec::new(),
            acorn_regions: BumpVec::new_in(arena),
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
    ///
    /// ⚠️ Compares the whole tag-name **run** ([`tag_name_end`]), not the peeked identifier
    /// token: the lexer's identifier scan is narrower than the run, so a token-only compare
    /// answers yes for `<script%x>` / `<style%x>` / `<svelte:options%x>` and routes a name
    /// Svelte rejects into the raw-text or options parser, which never grades it again.
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
            let name_start = self.base_offset + peek.start as usize;
            let name_end = tag_name_end(self.source, self.base_offset + peek.end as usize);
            return Ok(&self.source[name_start..name_end] == tag_name);
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

    /// Advance the lexer to a specific position in the source, used after a manual byte
    /// scan (e.g. `{@attach}` / RCDATA / raw-text parsing).
    ///
    /// Preserves the current `inside_tag` state. ⚠️ After a scan that jumped the cursor
    /// forward, that state is a **stale** artifact of wherever the lexer's last token
    /// happened to land (the scan ignored it), so "preserve" gives the right mode only
    /// when `pos` resumes adjacent to that token, or when the mode genuinely carries (an
    /// `{expr}` tag, a JS comment, a mid-tag attribute resync). A caller resuming into a
    /// KNOWN mode past an unrelated token must set `self.lexer.inside_tag` explicitly
    /// first — see `parse_rcdata_content`, which forces template mode after `</textarea>`.
    pub(crate) fn advance_to_position(&mut self, pos: usize) -> Result<(), ParseError> {
        // Save the inside_tag state before creating new lexer
        let was_inside_tag = self.lexer.inside_tag;

        // Reset the lexer to start from the new position. Token positions are reported
        // relative to the slice; the parser shifts them by base_offset, and the lexer
        // carries the same offset so its ERRORS are reported against the whole document.
        self.lexer = Lexer::at_offset(&self.source[pos..], pos);
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

    /// Resync the lexer past a name run ending at `name_end`. Fast path (`name_end` == the
    /// current Identifier token's end) is a plain `advance()`; when the name was extended
    /// past the token, re-lex at `name_end`.
    ///
    /// Both name readers that mirror Svelte's `read_tag` need this — the tag name
    /// (`parser/element.rs`) and the attribute/directive name (`parser/attribute.rs`) — since
    /// each reads a raw run the lexer's narrower identifier scan can stop short of.
    pub(crate) fn advance_past_name(&mut self, name_end: usize) -> Result<(), ParseError> {
        if name_end == self.current_end {
            self.advance()
        } else {
            self.advance_to_position(name_end)
        }
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
        ParseError::invalid_syntax(message.to_string(), self.current_start)
    }

    /// Create error with custom message at specified position
    pub(crate) fn error_msg_at(&self, message: &str, position: usize) -> ParseError {
        ParseError::invalid_syntax(message.to_string(), position)
    }

    /// Create "Expected X" error at specified position
    pub(crate) fn error_expected_at(&self, what: &str, position: usize) -> ParseError {
        ParseError::invalid_syntax(format!("Expected {what}"), position)
    }

    /// Create "Expected X, found Y" error at current position
    pub(crate) fn error_expected_found(&self, what: &str) -> ParseError {
        ParseError::invalid_syntax(
            format!("Expected {what}, found {}", self.current_kind),
            self.current_start,
        )
    }

    /// Create "Unclosed X" error at specified position
    pub(crate) fn error_unclosed_at(&self, what: &str, position: usize) -> ParseError {
        ParseError::invalid_syntax(format!("Unclosed {what}"), position)
    }

    /// Create "Duplicate X found" error at current position
    pub(crate) fn error_duplicate(&self, what: &str) -> ParseError {
        ParseError::invalid_syntax(format!("Duplicate {what} found"), self.current_start)
    }

    /// Create "Duplicate {:kw} clause found" error at current position — a block
    /// continuation REPEATING one the block already has.
    ///
    /// The `{:…}` braces are spelled here rather than by the caller so the continuation
    /// guards (`{:then}`, `{:catch}`, `{:else}`) share one wording, and so no call site
    /// carries a literal that reads as a format argument.
    pub(crate) fn error_duplicate_clause(&self, keyword: &str) -> ParseError {
        self.error_duplicate(&format!("{{:{keyword}}} clause"))
    }

    /// Create "{:a} cannot follow {:b}" error at current position — the sibling of
    /// [`Self::error_duplicate_clause`] for a continuation that is **not** a repeat but
    /// still lands on a slot its predecessor filled.
    ///
    /// The distinction is the whole point: `{#if a}1{:else}2{:else if b}3{/if}` holds one
    /// `{:else}` and one `{:else if}`, so calling either a duplicate names a thing the
    /// author did not write. What is taken is the block's single alternate, and the
    /// clearest way to say so is to name the pair.
    pub(crate) fn error_clause_after(&self, clause: &str, predecessor: &str) -> ParseError {
        ParseError::invalid_syntax(
            format!("{{:{clause}}} cannot follow {{:{predecessor}}}"),
            self.current_start,
        )
    }

    /// Read a JS identifier off the front of `s`, exactly as Svelte's `read_identifier`
    /// does (`1-parse/index.js:243`): an `ID_Start` char, then an `ID_Continue` run, then
    /// a rejection if the result is a reserved word. `offset` is the absolute source
    /// offset of `s[0]`.
    ///
    /// `Ok(None)` means *no identifier starts here* — the caller decides whether that is
    /// an error (a `{#snippet}` name is mandatory, a shorthand attribute's is too) or
    /// simply the absence of an optional piece (an `{#each}` index, where a non-start
    /// leaves the comma unconsumed so the trailing check reports it). Only the reserved
    /// word is an error the reader itself can raise, because it is the one case where an
    /// identifier WAS read.
    ///
    /// ⚠️ The character class is `tsv_ts`'s [`is_id_start`] / [`is_id_continue`], the
    /// ECMAScript one canonical reaches through acorn (`isIdentifierStart(code, true)` /
    /// `isIdentifierChar(code, true)`), never a local approximation. Rust's
    /// `char::is_alphabetic` / `is_alphanumeric` is NOT this class and misses in **both**
    /// directions: U+2118 is `ID_Start` but not alphabetic, and U+00B2 is alphanumeric but
    /// not `ID_Continue`. This function is their only reader in the crate, which is what
    /// keeps that answer in one place.
    ///
    /// The single reader for all six positions canonical reads this way, each of which had
    /// an inline copy and had drifted: the snippet's ran `is_id_continue` from the FIRST
    /// character (a leading digit joined the name), the pattern and shorthand copies never
    /// asked the reserved question, and the shorthand's spelled the class as
    /// `is_alphanumeric() || '_' || '$'` — diverging in both of the directions above
    /// (`{℘}` rejected though canonical accepts it, `{a²}` accepted though canonical stops
    /// the identifier at the `²` and then fails to eat `}`).
    pub(crate) fn read_identifier(
        &self,
        s: &'a str,
        offset: usize,
    ) -> Result<Option<&'a str>, ParseError> {
        let Some(first) = s.chars().next().filter(|c| is_id_start(*c)) else {
            return Ok(None);
        };
        debug_assert!(is_id_continue(first), "ID_Start is a subset of ID_Continue");
        let end = s.find(|c: char| !is_id_continue(c)).unwrap_or(s.len());
        let name = &s[..end];
        if is_reserved_word(name) {
            return Err(self.error_msg_at(&format!("Unexpected reserved word '{name}'"), offset));
        }
        Ok(Some(name))
    }

    /// Create "Unknown X: Y" error at specified position
    pub(crate) fn error_unknown_at(&self, kind: &str, value: &str, position: usize) -> ParseError {
        ParseError::invalid_syntax(format!("Unknown {kind}: {value}"), position)
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
        self.record_acorn_region(base_offset, PrefixLines::Ecmascript);
        let (expr, comments) =
            tsv_ts::parse_expression_with_comments(source, base_offset, self.arena)?;
        self.expression_comments.extend_from_slice(comments);
        Ok(expr)
    }

    /// Parse a partial TypeScript expression — one assignment expression, stopping at the
    /// first top-level comma.
    ///
    /// `top_level_as` is the block head's answer to "is a top-level `as` mine or
    /// TypeScript's" ([`tsv_ts::TopLevelAs`]). There is no default to fall back on —
    /// `{#each}` and `{#await}` answer it oppositely, and each answer carries an
    /// obligation the other does not. Neither head is ever asked about `satisfies`: no
    /// Svelte separator is spelled that way, so it stays TypeScript's in both.
    ///
    /// Comments are collected.
    pub(crate) fn parse_ts_expression_partial(
        &mut self,
        source: &str,
        base_offset: usize,
        top_level_as: TopLevelAs,
    ) -> Result<(Expression<'arena>, usize), ParseError> {
        self.record_acorn_region(base_offset, PrefixLines::Ecmascript);
        let (expr, end_pos, comments) = tsv_ts::parse_expression_partial_with_comments(
            source,
            base_offset,
            self.arena,
            top_level_as,
        )?;
        self.expression_comments.extend_from_slice(comments);
        Ok((expr, end_pos))
    }

    /// Parse a standalone type annotation (`: Type`) and collect any comments.
    ///
    /// The `{#each}` head is the one block reader that parses its binding's
    /// annotation separately — its pattern slice must stop before the
    /// `, index` / `(key)` tail — so it cannot ride `parse_ts_pattern`'s
    /// single sub-parse the way `{:then}` / `{:catch}` do.
    ///
    /// Returns only the annotation: its `span.end` is the consumed extent, and
    /// the sub-parser's own stop position is not offered because the lexer's
    /// lookahead has already swallowed the trailing trivia by then (see
    /// `tsv_ts::parse_type_annotation_partial`).
    pub(crate) fn parse_ts_type_annotation(
        &mut self,
        source: &str,
        base_offset: usize,
    ) -> Result<TSTypeAnnotation<'arena>, ParseError> {
        self.record_annotation_acorn_region(base_offset);
        let (ta, comments) =
            tsv_ts::parse_type_annotation_partial(source, base_offset, self.arena)?;
        self.expression_comments.extend_from_slice(comments);
        Ok(ta)
    }

    /// Parse a TypeScript pattern (destructuring) and collect any comments.
    /// Also handles optional type annotations (`: Type`) after the pattern.
    pub(crate) fn parse_ts_pattern(
        &mut self,
        source: &str,
        base_offset: usize,
    ) -> Result<Expression<'arena>, ParseError> {
        self.record_acorn_region(base_offset, PrefixLines::Lf);
        let (pattern, comments) =
            tsv_ts::parse_pattern_with_comments(source, base_offset, self.arena)?;
        // A trailing `: T` this sub-parse swallowed is a SECOND acorn parse for
        // canonical (`read_pattern` calls `read_type_annotation` after it), so it
        // gets its own region. `{#each}` splits the two reads itself and hands
        // the annotation to `parse_ts_type_annotation`, which records it there —
        // this arm is the one-sub-parse readers, `{:then}` / `{:catch}` /
        // `{@const}`.
        if let Some(annotation) = tsv_ts::ast::convert::block_pattern_annotation_span(&pattern) {
            self.record_annotation_acorn_region(annotation.start as usize);
        }
        // Canonical reads a destructure via a synthetic `(pattern = 1)` acorn
        // parse whose inserted `(` shifts the pattern's start line one column
        // right when that line is `> 1` — the same quirk the pattern nodes get
        // (`adjust_read_pattern_columns`) also lands on comments collected on
        // that line, and the wire serializes them with the shifted columns.
        //
        // The shift stops at the **bare** pattern's end. Canonical's `read_pattern`
        // hands the trailing `: T` to `read_type_annotation`, a separate parse that
        // prefixes `_ as ` and so preserves every column — and a plain identifier
        // binding never runs the synthetic parse at all, its only comment-bearing
        // region being that annotation. So a comment at or past the bare end keeps
        // its true column.
        let pattern_on_first_line = !self.source[..base_offset].contains('\n');
        let bare_pattern_end = pattern.span().end;
        self.expression_comments
            .extend(comments.iter().copied().map(|mut c| {
                if !pattern_on_first_line
                    && c.span.start < bare_pattern_end
                    && !self.source[base_offset..c.span.start as usize].contains('\n')
                {
                    c.bump_pattern_columns = true;
                }
                c
            }));
        Ok(pattern)
    }

    /// Record the embedded **acorn parse** Svelte runs over this component
    /// starting around `origin` — the fact `Root.acorn_regions` carries to the
    /// wire writer, which rebuilds acorn's line/column seed from it
    /// ([`tsv_lang::AcornSeed`]).
    ///
    /// Svelte calls `allow_whitespace()` before every one of these reads, so
    /// acorn's `startPos` is the first non-whitespace byte — while tsv's own
    /// sub-parse offsets are sometimes the delimiter still behind it. The skip
    /// lives here rather than at each call site because it is one rule, and
    /// getting it wrong is invisible until it isn't: a terminator tsv is still
    /// standing behind belongs to the prefix acorn **skipped**, and counting it
    /// moves every position in the island a line.
    pub(crate) fn record_acorn_region(&mut self, origin: usize, prefix: PrefixLines) {
        let rest = &self.source[origin..];
        let at = origin + (rest.len() - rest.trim_start_matches(is_svelte_ws).len());
        self.record_acorn_region_at(at, at, prefix);
    }

    /// Record the acorn parse of a block pattern's trailing `: T`, whose `:` is
    /// at `colon`.
    ///
    /// Svelte reads it with a second parse over `blanked_prefix + "_ as " +
    /// rest`, entered at `a = parser.index - "_ as ".len()` with `parser.index`
    /// just past the `:` — so acorn seeds five bytes behind the colon and starts
    /// lexing real source again one past it.
    pub(crate) fn record_annotation_acorn_region(&mut self, colon: usize) {
        const AS_INSERT_LEN: usize = "_ as ".len();
        let colon_end = colon + 1;
        self.record_acorn_region_at(
            colon_end,
            colon_end.saturating_sub(AS_INSERT_LEN),
            PrefixLines::Lf,
        );
    }

    /// The explicit form, for the two regions whose parse does **not** begin at
    /// the first non-whitespace byte: `<script>` content, which `read_script`
    /// reaches by lexing from offset 0 (so its leading whitespace is acorn's to
    /// count), and a block pattern's trailing `: T`, whose parse starts on
    /// Svelte's synthetic `_ as ` five bytes behind the real ones.
    ///
    /// `lex_start` is the first byte of the component acorn lexes for real;
    /// `origin` is acorn's `startPos`.
    pub(crate) fn record_acorn_region_at(
        &mut self,
        lex_start: usize,
        origin: usize,
        prefix: PrefixLines,
    ) {
        debug_assert!(
            self.acorn_regions
                .last()
                .is_none_or(|r| (r.lex_start as usize) < lex_start),
            "acorn regions are recorded in strict source order and never repeat \
             (binary-searched by the wire writer); a re-parse over the same region \
             must rewind through `EmbeddedParseMark`"
        );
        self.acorn_regions.push(internal::AcornRegion {
            lex_start: lex_start as u32,
            origin: origin as u32,
            prefix,
        });
    }

    /// How far both of the ledgers an embedded parse appends to had filled before
    /// it ran, so a **re-parse over the same region** can rewind them together.
    ///
    /// One value rather than two marks because the two ledgers fail differently
    /// and only one of the failures is loud: a comment registered twice is
    /// printed twice, while a repeated [`internal::AcornRegion`] is (today) an
    /// exact duplicate that `partition_point` resolves to the same seed — so
    /// rewinding one and forgetting the other reads as correct until the regions
    /// stop being identical.
    pub(crate) fn embedded_parse_mark(&self) -> EmbeddedParseMark {
        EmbeddedParseMark {
            comments: self.expression_comments.len(),
            acorn_regions: self.acorn_regions.len(),
        }
    }

    /// Drop everything the embedded parses since `mark` registered, leaving the
    /// parser as if they had not run. See [`embedded_parse_mark`](Self::embedded_parse_mark).
    pub(crate) fn rewind_embedded_parses(&mut self, mark: EmbeddedParseMark) {
        self.expression_comments.truncate(mark.comments);
        self.acorn_regions.truncate(mark.acorn_regions);
    }

    /// Parse a TypeScript statement (the body of a `{const}`/`{let}` tag is a
    /// `VariableDeclaration`) and collect any comments.
    pub(crate) fn parse_ts_statement(
        &mut self,
        source: &str,
        base_offset: usize,
    ) -> Result<tsv_ts::Statement<'arena>, ParseError> {
        self.record_acorn_region(base_offset, PrefixLines::Ecmascript);
        let (stmt, comments) =
            tsv_ts::parse_statement_with_comments(source, base_offset, self.arena)?;
        self.expression_comments.extend_from_slice(comments);
        Ok(stmt)
    }
}
