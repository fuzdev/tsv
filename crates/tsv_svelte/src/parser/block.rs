// Control flow block parsing
//
// Handles: {#if}, {#each}, {#await}, {#key} blocks

use std::rc::Rc;

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use crate::parser::element::ParsedElement;
use tsv_lang::source_scan::{TriviaProfile, skip_trivia};
use tsv_lang::{ParseError, Span};
use tsv_ts::Expression;

use super::expression_tag::scan_to_matching_brace;
use super::parser_impl::SvelteParser;
use super::{match_bracket, subslice_offset};

/// Whether `c` may START a JS identifier (letter, `_`, or `$` — never a digit).
/// Mirrors the leading-char rule of Svelte's `read_identifier`, used to validate the
/// `{#each}` index so a comment glyph or numeric literal isn't taken as the index.
fn is_identifier_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '$'
}

/// Whether `c` may CONTINUE a JS identifier (`is_identifier_start` plus digits).
fn is_identifier_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// Find the position of the LAST top-level ` as ` keyword in a string.
///
/// "Top-level" means not inside `()`, `[]`, `{}`, or `<>` brackets, and not inside string
/// literals or comments (skipped via the shared cursor). Returns the byte offset of the
/// space before `as`, or None if not found.
///
/// Used to detect TypeScript type assertions in `{#each}` expressions:
/// `{#each items as A[] as item}` → binding_str is `A[] as item`, this finds ` as ` after `A[]`.
fn find_last_top_level_as(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut depth: i32 = 0;
    let mut last_pos = None;
    let mut i = 0;

    while i < len {
        // Skip comments + strings via the shared cursor, so a ` as ` (or a bracket)
        // inside trivia can't mis-anchor the split — and string escapes are handled
        // in one escape-correct place.
        if let Some(past) = skip_trivia(bytes, i, len, TriviaProfile::JS) {
            i = past;
            continue;
        }
        match bytes[i] {
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b')' | b']' | b'}' | b'>' => depth = depth.saturating_sub(1),
            b' ' if depth == 0 && i + 3 <= len => {
                // Check for " as " or " as" at end of string
                if bytes[i + 1] == b'a'
                    && bytes[i + 2] == b's'
                    && (i + 3 == len || bytes[i + 3] == b' ')
                {
                    last_pos = Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    last_pos
}

/// Return type for parse_each_binding: (context, index, key_expr, key_span, consumed_end).
/// `consumed_end` is the absolute source offset just past the last token the binding
/// consumed — the caller rejects any non-whitespace between it and the closing `}`.
type EachBindingResult<'arena> = (
    Expression<'arena>,
    Option<&'arena str>,
    Option<Expression<'arena>>,
    Option<Span>,
    usize,
);

/// Return type for parse_index_and_key_after_context: (index, key_expr, key_span, consumed_end).
type IndexAndKeyResult<'arena> = (
    Option<&'arena str>,
    Option<Expression<'arena>>,
    Option<Span>,
    usize,
);

impl<'a, 'arena> SvelteParser<'a, 'arena> {
    /// Parse a control flow block starting with {#
    ///
    /// Dispatches to specific block parsers based on the keyword.
    pub(crate) fn parse_block(&mut self) -> Result<FragmentNode<'arena>, ParseError> {
        let start = self.current_start;

        // We're at {#, consume it
        if !self.check(TokenKind::BlockOpen) {
            return Err(self.error_expected_found("'{#'"));
        }

        // After {# we expect a block keyword: if, each, await, key, snippet
        let keyword = self.keyword_at(self.current_end);

        match keyword {
            "if" => self.parse_if_block(start),
            "each" => self.parse_each_block(start),
            "await" => self.parse_await_block(start),
            "key" => self.parse_key_block(start),
            "snippet" => self.parse_snippet_block(start),
            _ => Err(self.error_unknown_at("block type", &format!("{{#{keyword}}}"), start)),
        }
    }

    /// Parse an if block: {#if test}...{:else if test}...{:else}...{/if}
    fn parse_if_block(&mut self, start: usize) -> Result<FragmentNode<'arena>, ParseError> {
        self.parse_if_block_inner(start, false)
    }

    /// Inner parser for if blocks (handles both {#if} and {:else if})
    fn parse_if_block_inner(
        &mut self,
        start: usize,
        is_elseif: bool,
    ) -> Result<FragmentNode<'arena>, ParseError> {
        // Get the content start position (after {# or {:)
        let tag_content_start = self.current_end;

        // Scan to find closing } and extract content
        let (expr_content, content_start) = self.scan_block_tag_content(tag_content_start)?;

        // Extract the expression (skip "if " or "else if " prefix, handling
        // variable whitespace). Svelte requires whitespace after the `if`
        // keyword, so `{#if(x)}` / `{:else if(x)}` are rejected.
        let expr_str = if is_elseif {
            // {:else if expr} - skip "else", whitespace, "if", whitespace
            let after_else = expr_content
                .strip_prefix("else")
                .unwrap_or(expr_content)
                .trim_start();
            self.strip_block_keyword(after_else, "if", tag_content_start)?
                .trim_start()
        } else {
            // {#if expr} - skip "if", whitespace
            self.strip_block_keyword(expr_content, "if", tag_content_start)?
                .trim_start()
        };

        // Parse the test expression (with comments)
        let expr_offset = tag_content_start + subslice_offset(expr_content, expr_str);

        let test = self.parse_ts_expression(expr_str, expr_offset)?;

        // Opening tag span is from start to content_start (includes the closing })
        let opening_tag_span = Span {
            start: start as u32,
            end: content_start as u32,
        };

        // Parse consequent (content until {:else}, {:else if}, or {/if})
        let consequent = self.parse_block_children(&["else", "if"], content_start)?;

        // Check for alternate branch
        let alternate = if self.check(TokenKind::BlockContinue) {
            // Peek at what follows {:. Match the first two whitespace-delimited
            // words allocation-free (the old `.take(2).join(" ")` normalized
            // "else  if" -> "else if" only to compare against these two forms).
            let keyword = self.continuation_keyword_at(self.current_end);
            let mut words = keyword.split_whitespace();
            let first = words.next();
            let second = words.next();
            let is_else_if = first == Some("else") && second == Some("if");
            let is_else = first == Some("else") && second.is_none();

            if is_else_if {
                // {:else if} - parse as nested if block
                let elseif_start = self.current_start;
                let elseif_block = self.parse_if_block_inner(elseif_start, true)?;
                let mut nodes = self.bvec();
                nodes.push(elseif_block);
                Some(Fragment {
                    nodes: nodes.into_bump_slice(),
                })
            } else if is_else {
                // {:else} - parse else branch
                let else_tag_start = self.current_end;
                let (_, else_content_start) = self.scan_block_tag_content(else_tag_start)?; // consume "else}"
                let else_content = self.parse_block_children(&["if"], else_content_start)?;
                Some(else_content)
            } else {
                None
            }
        } else {
            None
        };

        // Determine end position
        // For {:else if}, the nested IfBlock already consumed {/if}, so use its end position
        // For all other cases (no alternate, {:else}, {:else}{#if}), consume {/if} ourselves
        let elseif_end = alternate.as_ref().and_then(|alt| {
            if let Some(FragmentNode::IfBlock(inner)) = alt.nodes.first()
                && inner.elseif
            {
                return Some(inner.span.end as usize);
            }
            None
        });

        let end = if let Some(end_pos) = elseif_end {
            end_pos
        } else {
            self.expect_block_close_keyword("if", start)?
        };

        Ok(FragmentNode::IfBlock(IfBlock {
            elseif: is_elseif,
            test,
            consequent,
            alternate,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
            opening_tag_span,
        }))
    }

    /// Parse an each block: {#each expression as context, index (key)}...{:else}...{/each}
    fn parse_each_block(&mut self, start: usize) -> Result<FragmentNode<'arena>, ParseError> {
        // Get the content start position (after {#)
        let tag_content_start = self.current_end;

        // Scan to find closing } and extract content
        let (tag_content, content_start) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "each expression as context, index (key)" — Svelte requires
        // whitespace after the keyword. The remainder keeps its leading
        // whitespace; `content_offset` points just past the keyword and the
        // `trim_start()` below recovers the expression's exact offset.
        let content = self.strip_block_keyword(tag_content, "each", tag_content_start)?;
        let content_offset = tag_content_start + (tag_content.len() - content.len());

        // Use partial parsing for the iterable expression - stops at identifiers like "as"
        // This correctly handles cases like `getItems(" as ")` where " as " is inside a string
        let (expression, expr_end_pos) = self.parse_ts_expression_partial(
            content.trim_start(),
            content_offset + (content.len() - content.trim_start().len()),
        )?;

        // Opening tag span is from start to content_start (includes the closing })
        let opening_tag_span = Span {
            start: start as u32,
            end: content_start as u32,
        };

        // After the expression, check for " as " or ", index" or just "}"
        let expr_consumed = expr_end_pos - content_offset;
        let after_expr = &content[expr_consumed..];

        // Try to strip " as " to get binding (with context pattern)
        let (expression, context, index, key, key_span, binding_end) = if let Some(binding_str) =
            after_expr
                .strip_prefix(" as ")
                .or_else(|| after_expr.trim_start().strip_prefix("as "))
        {
            let as_len = after_expr.len() - binding_str.len();
            let binding_offset = content_offset + expr_consumed + as_len;

            // Check if binding_str contains another top-level `as` keyword.
            // If so, the first `as` was a TypeScript type assertion (e.g., `items as A[] as item`),
            // not the Svelte binding separator. Find the LAST top-level `as` to split correctly,
            // handling chained assertions like `items as A as B[] as item`.
            if let Some(last_as_pos) = find_last_top_level_as(binding_str) {
                // Re-parse: expression extends through all type assertions
                let full_expr_end = expr_consumed + as_len + last_as_pos;
                let full_expr_str = &content[..full_expr_end];
                let expr_offset = content_offset + (content.len() - content.trim_start().len());
                let expression = self.parse_ts_expression(full_expr_str.trim(), expr_offset)?;

                // Real binding starts after the last " as " (4 bytes: space-a-s-space)
                let real_binding_start = last_as_pos + " as ".len();
                let real_binding = &binding_str[real_binding_start..];
                let real_binding_offset = binding_offset + real_binding_start;
                let (ctx, idx, k, k_span, b_end) =
                    self.parse_each_binding(real_binding, real_binding_offset)?;
                (expression, Some(ctx), idx, k, k_span, b_end)
            } else {
                // Normal case: first `as` is the Svelte binding separator
                let (ctx, idx, k, k_span, b_end) =
                    self.parse_each_binding(binding_str, binding_offset)?;
                (expression, Some(ctx), idx, k, k_span, b_end)
            }
        } else {
            // No `as` clause: the remainder is the optional `, index` and/or `(key)` —
            // the same grammar as after a context, just without one — so route it through
            // the shared parser (context stays `None`). Svelte allows index/key without
            // `as` (`{#each items, i}`, `{#each items, i (key)}`); the shared parser also
            // bounds the key with the trivia-aware bracket scanner and reports a precise
            // `consumed_end`, so trailing junk is rejected below instead of swallowed.
            //
            // Read from the expression's SEMANTIC end, not `expr_end_pos` (the partial
            // parser's stop, which swallows a trailing comment as trivia) — mirroring
            // Svelte's `parser.index = expression.end`. This way a comment after the
            // iterable with no binding (`{#each items /* c */}`) becomes trailing junk
            // rejected below, not a silently kept comment. (The `as` branch keeps using
            // `after_expr` so a comment *before* `as` — `{#each items /* c */ as item}`,
            // which Svelte accepts — still resolves to the binding.)
            let semantic_end = expression.span().end as usize;
            let after_semantic = &content[semantic_end - content_offset..];
            let (idx, k, k_span, b_end) =
                self.parse_index_and_key_after_context(after_semantic, semantic_end)?;
            (expression, None, idx, k, k_span, b_end)
        };

        // The opening tag must end at `}` immediately after the binding (Svelte's final
        // `eat('}')`): only whitespace may remain. A stray comment, leftover index/key
        // fragment, or junk here is rejected — not silently dropped (content loss).
        let brace_pos = content_start - 1;
        self.reject_trailing_tag_content(&self.source[binding_end..brace_pos], binding_end)?;

        // Parse body
        let body = self.parse_block_children(&["else", "each"], content_start)?;

        // Check for fallback. Only `{:else}` is an each continuation — any other
        // `{:keyword}` (e.g. `{:catch}`, `{:then}`) is left unconsumed so it
        // surfaces as an orphan-continuation error, matching the canonical parser.
        let fallback = if self.check(TokenKind::BlockContinue)
            && self.continuation_keyword_at(self.current_end) == "else"
        {
            let else_tag_start = self.current_end;
            let (_, else_content_start) = self.scan_block_tag_content(else_tag_start)?; // consume "else}"
            Some(self.parse_block_children(&["each"], else_content_start)?)
        } else {
            None
        };

        // Expect closing {/each}
        let end = self.expect_block_close_keyword("each", start)?;

        Ok(FragmentNode::EachBlock(EachBlock {
            expression,
            context,
            index,
            key,
            key_span,
            body,
            fallback,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
            opening_tag_span,
        }))
    }

    /// Parse each binding: "context, index (key)" using the TypeScript expression parser.
    ///
    /// Uses partial expression parsing which correctly handles:
    /// - Simple identifiers: `item`
    /// - Object destructuring: `{ a, b }` (commas inside braces don't split)
    /// - Array destructuring: `[a, b]` (commas inside brackets don't split)
    /// - Strings with brackets: `{ a: "}" }` (braces inside strings don't count)
    /// - Template literals: `` { a: `${x}` } ``
    ///
    /// The TS parser stops at top-level commas, so `{ a, b }, i` parses `{ a, b }` and leaves `, i`.
    ///
    /// Returns (context, index, key, key_span) where key_span includes the parentheses.
    fn parse_each_binding(
        &mut self,
        binding: &str,
        binding_offset: usize,
    ) -> Result<EachBindingResult<'arena>, ParseError> {
        // Calculate leading whitespace and adjust offset accordingly
        let leading_ws = binding.len() - binding.trim_start().len();
        let trimmed = binding.trim();
        let adjusted_offset = binding_offset + leading_ws;

        // Parse context as a PATTERN (like Svelte does), not as expression
        // Patterns are: identifiers OR destructuring {..}/[..]
        // This naturally stops at whitespace/comma/paren, avoiding the
        // `item (key)` being parsed as a function call
        let (context, pattern_end) = self.parse_context_pattern(trimmed, adjusted_offset)?;

        // Parse remaining: ", index" and/or "(key)"
        let consumed = pattern_end - adjusted_offset;
        let remaining = &trimmed[consumed..];
        let (index, key, key_span, consumed_end) =
            self.parse_index_and_key_after_context(remaining, pattern_end)?;

        Ok((context, index, key, key_span, consumed_end))
    }

    /// Parse a context pattern: identifier or destructuring pattern.
    /// Like Svelte's read_pattern, this stops at whitespace/comma/paren for identifiers.
    fn parse_context_pattern(
        &mut self,
        input: &str,
        offset: usize,
    ) -> Result<(Expression<'arena>, usize), ParseError> {
        let trimmed = input.trim_start();
        let ws_len = input.len() - trimmed.len();
        let adjusted = offset + ws_len;

        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            // Destructuring pattern - find matching bracket
            let end = self.find_matching_bracket(trimmed)?;
            let pattern_str = &trimmed[..end];
            // Use parse_pattern to get ObjectPattern/ArrayPattern instead of ObjectExpression/ArrayExpression
            let expr = self.parse_ts_pattern(pattern_str, adjusted)?;
            Ok((expr, adjusted + end))
        } else {
            // Simple identifier - read until non-identifier char
            let end = trimmed
                .find(|c: char| !is_identifier_continue(c))
                .unwrap_or(trimmed.len());
            if end == 0 {
                return Err(self.error_expected_at("identifier or pattern", offset));
            }
            let ident_str = &trimmed[..end];
            let mut expr = self.parse_ts_expression(ident_str, adjusted)?;

            // Check for type annotation (`: Type`) after identifier
            let after_ident = trimmed[end..].trim_start();
            if after_ident.starts_with(':') {
                let ws_before_colon = trimmed.len() - end - after_ident.len();
                let colon_offset = adjusted + end + ws_before_colon;
                let (ta, type_end) = tsv_ts::parse_type_annotation_partial(
                    after_ident,
                    colon_offset,
                    Rc::clone(&self.interner),
                    self.arena,
                )?;
                if let Expression::Identifier(id) = &mut expr {
                    // Re-bind the identifier's binding extra with the parsed type
                    // annotation, preserving any decorators already present.
                    let decorators = id.decorators();
                    id.extra = Some(self.alloc(tsv_ts::ast::internal::IdentifierParamExtra {
                        type_annotation: Some(ta),
                        decorators,
                    }));
                }
                Ok((expr, type_end))
            } else {
                Ok((expr, adjusted + end))
            }
        }
    }

    /// Find the matching closing bracket for a string starting with `{` or `[`,
    /// returning the byte offset just past the close (so `&input[..end]` is the whole
    /// bracketed run). Comment- and string-aware via the shared cursor.
    fn find_matching_bracket(&self, input: &str) -> Result<usize, ParseError> {
        let bytes = input.as_bytes();
        let (open, close) = match bytes.first() {
            Some(b'{') => (b'{', b'}'),
            Some(b'[') => (b'[', b']'),
            _ => {
                return Err(ParseError::InvalidSyntax {
                    message: "Expected { or [".to_string(),
                    position: 0,
                    context: None,
                });
            }
        };

        match_bracket(bytes, 0, bytes.len(), open, close, TriviaProfile::JS)
            .map(|close_pos| close_pos + 1) // include the closing bracket
            .ok_or_else(|| ParseError::InvalidSyntax {
                message: "Unmatched bracket".to_string(),
                position: 0,
                context: None,
            })
    }

    /// Parse ", index" and/or "(key)" after the context pattern
    ///
    /// Returns (index, key_expression, key_span) where key_span includes the parentheses.
    fn parse_index_and_key_after_context(
        &mut self,
        remaining: &str,
        remaining_offset: usize,
    ) -> Result<IndexAndKeyResult<'arena>, ParseError> {
        let trimmed = remaining.trim_start();
        let ws_len = remaining.len() - trimmed.len();
        let offset = remaining_offset + ws_len;

        let mut rest = trimmed;
        let mut rest_offset = offset;
        let mut index = None;
        // Absolute offset just past the last token the binding consumed. Starts at the
        // context end (`remaining_offset`): with no index/key the binding ends there, so
        // everything in `remaining` is trailing and the caller rejects it.
        let mut consumed_end = remaining_offset;

        // Check for ", index" — the index is a bare identifier (Svelte's `read_identifier`).
        if let Some(after_comma) = rest.strip_prefix(',') {
            let after_comma_trimmed = after_comma.trim_start();
            let comma_ws = after_comma.len() - after_comma_trimmed.len();

            // An identifier must start with a letter / `_` / `$` (never a digit, comment
            // glyph, or other char). A non-start leaves the index unread and the comma
            // unconsumed, so the caller's trailing check rejects — matching Svelte, which
            // emits "Expected an identifier" for `{#each x as y, /* c */ i}` and `, 5`.
            let idx_end = if after_comma_trimmed.starts_with(|c: char| is_identifier_start(c)) {
                after_comma_trimmed
                    .find(|c: char| !is_identifier_continue(c))
                    .unwrap_or(after_comma_trimmed.len())
            } else {
                0
            };

            if idx_end > 0 {
                index = Some(self.alloc_str_in(&after_comma_trimmed[..idx_end]));
                rest = &after_comma_trimmed[idx_end..];
                rest_offset = offset + 1 + comma_ws + idx_end;
                consumed_end = rest_offset;
            }
        }

        // Check for "(key)" — match the `)` with the trivia-aware bracket scanner so a
        // `)` inside a string/comment in the key can't end it early, and any trailing
        // junk after the real `)` is left for the caller's trailing check (not swallowed).
        let rest_trimmed = rest.trim_start();
        let (key, key_span) = if rest_trimmed.starts_with('(') {
            let key_ws = rest.len() - rest_trimmed.len();
            let paren_start = rest_offset + key_ws; // absolute offset of '('
            let close = match_bracket(
                rest_trimmed.as_bytes(),
                0,
                rest_trimmed.len(),
                b'(',
                b')',
                TriviaProfile::JS,
            )
            .ok_or_else(|| self.error_expected_at("')'", paren_start + rest_trimmed.len()))?;
            let key_str = &rest_trimmed[1..close];
            let key_offset = paren_start + 1; // after '('
            let key_expr = self.parse_ts_expression(
                key_str.trim(),
                key_offset + (key_str.len() - key_str.trim_start().len()),
            )?;
            // Span includes the parentheses: from '(' to after ')'.
            let span_start = paren_start as u32;
            let span_end = (paren_start + close + 1) as u32;
            consumed_end = span_end as usize;
            (Some(key_expr), Some(Span::new(span_start, span_end)))
        } else {
            (None, None)
        };

        Ok((index, key, key_span, consumed_end))
    }

    /// Parse an await block: {#await expression}...{:then value}...{:catch error}...{/await}
    fn parse_await_block(&mut self, start: usize) -> Result<FragmentNode<'arena>, ParseError> {
        // Get the content start position (after {#)
        let tag_content_start = self.current_end;

        // Scan to find closing } and extract content
        let (tag_content, content_start) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "await expression" or "await expression then value" — Svelte
        // requires whitespace after the keyword. The remainder keeps its leading
        // whitespace; `content_offset` points just past the keyword and the
        // `trim_start()` below recovers the expression's exact offset.
        let content = self.strip_block_keyword(tag_content, "await", tag_content_start)?;
        let content_offset = tag_content_start + (tag_content.len() - content.len());

        // Use partial parsing for the promise expression
        // This correctly handles cases like `fetch(" then ")` where " then " is inside a string
        let (expression, expr_end_pos) = self.parse_ts_expression_partial(
            content.trim_start(),
            content_offset + (content.len() - content.trim_start().len()),
        )?;

        // Opening tag span is from start to content_start (includes the closing })
        let opening_tag_span = Span {
            start: start as u32,
            end: content_start as u32,
        };

        // Check what follows the expression
        let expr_consumed = expr_end_pos - content_offset;
        let after_expr = &content[expr_consumed..];

        // Check for shorthand: {#await promise then value}
        let shorthand_then = if let Some(rest) = after_expr.strip_prefix(" then ") {
            Some(rest)
        } else if let Some(rest) = after_expr.trim_start().strip_prefix("then ") {
            Some(rest)
        } else if after_expr.trim() == "then" || after_expr.trim_start().starts_with("then}") {
            Some("")
        } else {
            None
        };

        // Check for shorthand: {#await promise catch error}
        let shorthand_catch = if let Some(rest) = after_expr.strip_prefix(" catch ") {
            Some(rest)
        } else if let Some(rest) = after_expr.trim_start().strip_prefix("catch ") {
            Some(rest)
        } else if after_expr.trim() == "catch" || after_expr.trim_start().starts_with("catch}") {
            Some("")
        } else {
            None
        };

        // The block form (`{#await x}…{/await}`) always carries a pending Fragment —
        // empty or not — unlike the inline `then`/`catch` shorthand (no pending
        // phase); the writer emits `{Fragment, []}` vs `null` from this.
        let pending_block = shorthand_then.is_none() && shorthand_catch.is_none();
        let (pending, then_fragment, catch_fragment, value, error, end) = if let Some(value_str) =
            shorthand_then
        {
            // Shorthand then syntax: {#await promise then value}...{/await}
            let value = if !value_str.is_empty() {
                // `value_str` starts right after "expression then "; the pattern parser
                // trims its own leading whitespace, so pass the raw slice + that offset.
                let then_keyword_end = expr_end_pos + (after_expr.len() - value_str.len());
                Some(self.parse_await_value_pattern(value_str, then_keyword_end)?)
            } else {
                None
            };

            let then_content = self.parse_block_children(&["catch", "await"], content_start)?;

            // Check for optional {:catch} continuation after then-shorthand
            let (catch_fragment, error) = if self.check_await_continuation("catch") {
                self.parse_await_continuation("catch", &["await"])?
            } else {
                (None, None)
            };

            let block_end = self.expect_block_close_keyword("await", start)?;

            (
                None,
                Some(then_content),
                catch_fragment,
                value,
                error,
                block_end,
            )
        } else if let Some(error_str) = shorthand_catch {
            // Shorthand catch syntax: {#await promise catch error}...{/await}
            let error = if !error_str.is_empty() {
                // `error_str` starts right after "expression catch "; the pattern parser
                // trims its own leading whitespace, so pass the raw slice + that offset.
                let catch_keyword_end = expr_end_pos + (after_expr.len() - error_str.len());
                Some(self.parse_await_value_pattern(error_str, catch_keyword_end)?)
            } else {
                None
            };

            let catch_content = self.parse_block_children(&["then", "await"], content_start)?;

            // Check for optional {:then} continuation after catch-shorthand
            let (then_fragment, value) = if self.check_await_continuation("then") {
                self.parse_await_continuation("then", &["await"])?
            } else {
                (None, None)
            };

            let block_end = self.expect_block_close_keyword("await", start)?;

            (
                None,
                then_fragment,
                Some(catch_content),
                value,
                error,
                block_end,
            )
        } else {
            // No `then`/`catch` shorthand matched, so the opening tag must end
            // right after the promise expression. Reject trailing content like
            // `{#await p garbage}` or a shorthand jammed against the expression
            // (`{#await p then(v)}`) — the canonical parser rejects both.
            self.reject_trailing_tag_content(after_expr, expr_end_pos)?;

            // Full syntax with pending block
            let pending_content =
                self.parse_block_children(&["then", "catch", "await"], content_start)?;
            let pending = if !pending_content.nodes.is_empty() {
                Some(pending_content)
            } else {
                None
            };

            let mut then_fragment = None;
            let mut catch_fragment = None;
            let mut value = None;
            let mut error = None;

            // Parse :then and :catch blocks
            loop {
                if self.check_await_continuation("then") {
                    let (frag, val) = self.parse_await_continuation("then", &["catch", "await"])?;
                    then_fragment = frag;
                    value = val;
                } else if self.check_await_continuation("catch") {
                    let (frag, err) = self.parse_await_continuation("catch", &["await"])?;
                    catch_fragment = frag;
                    error = err;
                } else {
                    break;
                }
            }

            let block_end = self.expect_block_close_keyword("await", start)?;

            (
                pending,
                then_fragment,
                catch_fragment,
                value,
                error,
                block_end,
            )
        };

        Ok(FragmentNode::AwaitBlock(AwaitBlock {
            expression,
            value,
            error,
            pending,
            pending_block,
            then: then_fragment,
            catch: catch_fragment,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
            opening_tag_span,
        }))
    }

    /// Check if the next token is a BlockContinue with the given keyword (e.g., "catch", "then").
    fn check_await_continuation(&self, keyword: &str) -> bool {
        self.check(TokenKind::BlockContinue)
            && self
                .continuation_keyword_at(self.current_end)
                .starts_with(keyword)
    }

    /// Parse a `{:then value}` / `{:catch error}` continuation block within an await block.
    /// `keyword` is `"then"` or `"catch"`; `stop_keywords` are the continuations that end this
    /// block's body (`{:catch}` always stops only at `{/await}`; `{:then}` also stops at a
    /// following `{:catch}` in the full form). Returns `(fragment, binding_pattern)`.
    fn parse_await_continuation(
        &mut self,
        keyword: &str,
        stop_keywords: &[&str],
    ) -> Result<(Option<Fragment<'arena>>, Option<Expression<'arena>>), ParseError> {
        let tag_start = self.current_end;
        let (tag_content, content_start) = self.scan_block_tag_content(tag_start)?;
        let binding_str = self
            .strip_keyword_value(tag_content, keyword, tag_start)?
            .trim();

        let binding = if !binding_str.is_empty() {
            let offset = tag_start + subslice_offset(tag_content, binding_str);
            Some(self.parse_await_value_pattern(binding_str, offset)?)
        } else {
            None
        };

        let fragment = self.parse_block_children(stop_keywords, content_start)?;
        Ok((Some(fragment), binding))
    }

    /// Read the leading alphabetic keyword at `pos` in the source — the `if` in
    /// `{#if}`, the `each` in `{/each}`, the `html` in `{@html}`. Stops at the
    /// first non-alphabetic byte (space, `}`, …); returns `""` when there is none.
    pub(super) fn keyword_at(&self, pos: usize) -> &'a str {
        let remaining = &self.source[pos..];
        let end = remaining
            .find(|c: char| !c.is_alphabetic())
            .unwrap_or(remaining.len());
        &remaining[..end]
    }

    /// Read the continuation keyword-run at `pos` — the alphabetic-and-space run
    /// after `{:`, trimmed. Unlike `keyword_at` this keeps internal spaces so the
    /// two-word `else if` survives; callers compare against `"else"`, `"else if"`,
    /// `"catch"`, etc. Trailing content makes the run miss every keyword (e.g.
    /// `{:else garbage}` yields `"else garbage"`, which is neither `else` nor
    /// `else if`), so it is left unconsumed and surfaces as an error.
    fn continuation_keyword_at(&self, pos: usize) -> &'a str {
        let remaining = &self.source[pos..];
        let end = remaining
            .find(|c: char| !c.is_alphabetic() && c != ' ')
            .unwrap_or(remaining.len());
        remaining[..end].trim()
    }

    /// Strip a leading block/tag keyword, enforcing the whitespace Svelte
    /// requires between the keyword and any value that follows. The value may be
    /// absent (`{:then}` → `Ok("")`), but a value jammed against the keyword
    /// (`{:then(v)}`, `{:thenx}`) is rejected — matching the canonical parser,
    /// which emits `expected_whitespace`. Any whitespace counts (space, tab,
    /// newline), so the returned remainder is left untrimmed; callers trim it and
    /// recover span offsets with `subslice_offset`.
    fn strip_keyword_value(
        &self,
        content: &'a str,
        keyword: &str,
        keyword_start: usize,
    ) -> Result<&'a str, ParseError> {
        let rest = content.strip_prefix(keyword).unwrap_or(content);
        if rest.is_empty() || rest.starts_with(|c: char| c.is_whitespace()) {
            Ok(rest)
        } else {
            Err(self.error_expected_at(&format!("whitespace after `{keyword}`"), keyword_start))
        }
    }

    /// Like `strip_keyword_value`, but the value is mandatory: the keyword
    /// standing alone (`{#each}`, `{@html}`) is also rejected. Used by the blocks
    /// and tags whose expression or name is required.
    pub(super) fn strip_block_keyword(
        &self,
        content: &'a str,
        keyword: &str,
        keyword_start: usize,
    ) -> Result<&'a str, ParseError> {
        let rest = self.strip_keyword_value(content, keyword, keyword_start)?;
        if rest.is_empty() {
            return Err(
                self.error_expected_at(&format!("whitespace after `{keyword}`"), keyword_start)
            );
        }
        Ok(rest)
    }

    /// Require that `region` — whose first byte is at absolute source offset `region_start` —
    /// holds only whitespace before the tag's closing `}`. This is Svelte's `allow_whitespace`
    /// then `eat('}')` after a tag's payload: a stray comment, leftover binding fragment, or
    /// junk is rejected (erroring at the first non-whitespace byte), never silently dropped.
    /// Shared by every block whose tag ends right after its payload — `{#each}`'s binding,
    /// `{#await}`'s promise, `{#snippet}`'s `)`, and every `{/block}` close.
    fn reject_trailing_tag_content(
        &self,
        region: &str,
        region_start: usize,
    ) -> Result<(), ParseError> {
        let trailing = region.trim_start();
        if !trailing.is_empty() {
            let trailing_start = region_start + (region.len() - trailing.len());
            return Err(self.error_expected_at("'}'", trailing_start));
        }
        Ok(())
    }

    /// Parse a `{#await}` `then`/`catch` binding pattern (the value/error), rejecting a
    /// comment immediately BEFORE the pattern or BETWEEN the pattern and its `:`/`}`.
    /// Svelte reads these with `read_pattern` — acorn at the current index, having skipped
    /// only whitespace — so a comment before the pattern fails ("Expected identifier or
    /// destructure pattern") and the following `eat()` rejects one between the pattern and
    /// the next token. A comment INSIDE a destructure (`{ a /* c */ }`) or INSIDE the type
    /// annotation (`value: /* c */ number`) stays valid — it's acorn trivia within the
    /// pattern/type. tsv's `parse_ts_pattern` is comment-tolerant (it would relocate or drop
    /// a surrounding comment), so this gate restores Svelte's strictness. `region_offset` is
    /// the absolute source offset of `region[0]`.
    fn parse_await_value_pattern(
        &mut self,
        region: &str,
        region_offset: usize,
    ) -> Result<Expression<'arena>, ParseError> {
        let lead = region.len() - region.trim_start().len();
        let value_start = region_offset + lead;
        let trimmed = region.trim();
        let pattern = self.parse_ts_pattern(trimmed, value_start)?;
        let span = pattern.span();
        // Leading comment: the pattern would start past `value_start`.
        if span.start as usize != value_start {
            return Err(self.error_expected_at("identifier or destructure pattern", value_start));
        }
        // Comment right after the pattern, before its `:`/`}`. The leftover may legitimately
        // be a `: type` annotation (kept), so we reject only when it *starts* with a comment —
        // a comment INSIDE the type (`value: /* c */ number`) leaves `:` first and is allowed.
        let after = span.end as usize - value_start; // index into `trimmed`
        let tail = trimmed[after..].trim_start();
        if tail.starts_with("/*") || tail.starts_with("//") {
            return Err(
                self.error_expected_at("identifier or destructure pattern", span.end as usize)
            );
        }
        Ok(pattern)
    }

    /// Consume the closing `{/expected}` tag and return the position after it.
    /// `block_start` is the byte offset of the opening `{#expected`, used to
    /// locate the unclosed-block error.
    ///
    /// Three failure modes, all rejected by the canonical parser:
    /// - the block is left unclosed (`{#if x}a`) — reported at `block_start`;
    /// - the close names a different block — a mismatch like `{#if x}…{/each}`;
    /// - the close carries trailing junk (`{/each foo}`) — only whitespace may
    ///   follow the keyword before `}`.
    fn expect_block_close_keyword(
        &mut self,
        expected: &str,
        block_start: usize,
    ) -> Result<usize, ParseError> {
        // Unclosed block: Svelte requires a matching `{/expected}`.
        if !self.check(TokenKind::BlockClose) {
            return Err(self.error_unclosed_at(&format!("{{#{expected}}} block"), block_start));
        }

        // The keyword after `{/` must match the open block.
        if self.keyword_at(self.current_end) != expected {
            return Err(self.error_expected_at(&format!("{{/{expected}}}"), self.current_start));
        }

        let close_tag_start = self.current_end;
        let (close_content, after_close) = self.scan_block_tag_content(close_tag_start)?;

        // Only whitespace may follow the keyword: `{/each foo}` is rejected.
        // The keyword matched above, so `close_content` starts with `expected`.
        self.reject_trailing_tag_content(
            &close_content[expected.len()..],
            close_tag_start + expected.len(),
        )?;

        Ok(after_close)
    }

    /// Parse a key block: {#key expression}...{/key}
    fn parse_key_block(&mut self, start: usize) -> Result<FragmentNode<'arena>, ParseError> {
        // Get the content start position (after {#)
        let tag_content_start = self.current_end;

        // Scan to find closing } and extract content
        let (tag_content, content_start) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "key expression" — Svelte requires whitespace after the keyword.
        let expr_str = self
            .strip_block_keyword(tag_content, "key", tag_content_start)?
            .trim();

        let expr_offset = tag_content_start + subslice_offset(tag_content, expr_str);
        let expression = self.parse_ts_expression(expr_str, expr_offset)?;

        // Opening tag span is from start to content_start (includes the closing })
        let opening_tag_span = Span {
            start: start as u32,
            end: content_start as u32,
        };

        // Parse fragment
        let fragment = self.parse_block_children(&["key"], content_start)?;

        // Expect closing {/key}
        let end = self.expect_block_close_keyword("key", start)?;

        Ok(FragmentNode::KeyBlock(KeyBlock {
            expression,
            fragment,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
            opening_tag_span,
        }))
    }

    /// Parse a snippet block: {#snippet name(params)}...{/snippet}
    /// Also handles TypeScript generics: {#snippet name<T>(params)}
    fn parse_snippet_block(&mut self, start: usize) -> Result<FragmentNode<'arena>, ParseError> {
        // Get the content start position (after {#)
        let tag_content_start = self.current_end;

        // Scan to find closing } and extract content
        let (tag_content, content_start) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "snippet name(params)" or "snippet name<T>(params)" — Svelte
        // requires whitespace after the keyword.
        let content = self
            .strip_block_keyword(tag_content, "snippet", tag_content_start)?
            .trim();
        let content_bytes = content.as_bytes();
        // Absolute offset of `content[0]` (the name's first byte) in the source, the
        // base for every span and error position below.
        let content_offset = tag_content_start + subslice_offset(tag_content, content);

        // Mirror Svelte's snippet-head grammar (`1-parse/state/tag.js`): read the name,
        // then an optional `<…>` generic via the naive `<`/`>` matcher, then REQUIRE a
        // `(`. Svelte's generic matcher tracks only angle depth (never parens), so a `>`
        // from a `=>` / `>=` / `>>` closes the generic early and the required `(` can't be
        // found — Svelte rejects, and we reject in lockstep. A function type (or any stray
        // `>`) in a snippet generic is invalid Svelte, so corrupting it on format would be
        // worse than a parse error. See `find_matching_angle_bracket`.

        // Name: the leading identifier run, like Svelte's `read_identifier`. `content` is
        // trimmed, so it starts at the name.
        let name_len = content
            .find(|c: char| !is_identifier_continue(c))
            .unwrap_or(content.len());
        if name_len == 0 {
            return Err(self.error_expected_at("snippet name", content_offset));
        }
        let name_str = &content[..name_len];
        let expression = self.parse_ts_expression(name_str, content_offset)?;

        // Optional `<…>` generic. `head_start` is the `<` (or, with no generic, the `(`)
        // where the parseable signature head begins — the wrapper slice below spans from
        // there through the matching `)`.
        let after_name = content[name_len..].trim_start();
        let head_start = content.len() - after_name.len();
        let (after_generic, type_params_raw): (usize, Option<&'arena str>) =
            if after_name.starts_with('<') {
                // `type_params_raw` is the raw inner text — feeds the public AST's `typeParams`
                // string (Svelte stores it raw too) and the parse-failure fallback.
                let close_pos = self.find_matching_angle_bracket(content, head_start)?;
                (
                    close_pos + 1,
                    Some(self.alloc_str_in(&content[head_start + 1..close_pos])),
                )
            } else {
                (head_start, None)
            };

        // Require `(` after only whitespace — Svelte's `allow_whitespace` then
        // `eat('(', true)`. Crucially this skips whitespace but NOT comments, so
        // `<T> /* c */ (…)` is rejected exactly as Svelte rejects it.
        let after_generic_str = &content[after_generic..];
        let paren_pos =
            after_generic + (after_generic_str.len() - after_generic_str.trim_start().len());
        if !content[paren_pos..].starts_with('(') {
            return Err(self.error_expected_at("'('", content_offset + paren_pos));
        }

        // Opening tag span is from start to content_start (includes the closing })
        let opening_tag_span = Span {
            start: start as u32,
            end: content_start as u32,
        };

        // The `)` matching the opening `(` — depth- and trivia-aware, so a `)` inside a
        // string/comment in a param default can't end the list early. Svelte requires the
        // close (`eat(')', true)`); an unmatched `(` is rejected.
        let close_paren = match_bracket(
            content_bytes,
            paren_pos,
            content.len(),
            b'(',
            b')',
            TriviaProfile::JS,
        )
        .ok_or_else(|| self.error_expected_at("')'", content_offset + content.len()))?;

        // Only whitespace may follow `)` before the closing `}` — Svelte's
        // `allow_whitespace` then `eat('}', true)`. `{#snippet fn() junk}` is rejected.
        self.reject_trailing_tag_content(
            &content[close_paren + 1..],
            content_offset + close_paren + 1,
        )?;

        // Absolute source span of the parens (`start` = `(`, `end` = `)`), for comment
        // lookup when printing the parameter list.
        let params_paren = Some(Span {
            start: (content_offset + paren_pos) as u32,
            end: (content_offset + close_paren) as u32,
        });
        let params_str = &content[paren_pos + 1..close_paren];

        // Parse the signature head `<TP>(PARAMS)` as `function f<TP>(PARAMS) {}` so every
        // position — type parameters (constraints/defaults/modifiers/comments),
        // typed/destructured params, comments anywhere — goes through the canonical
        // comment-collecting parser. Wrapping a *contiguous* source slice (from the `<` or
        // `(` through the matching `)`) keeps the single `base` offset valid across both
        // `<…>` and `(…)`. Collected comments merge into the root buffer (the printer
        // locates them by position). Falls back to raw text on parse failure (e.g. a form
        // acorn-typescript rejects); the generics are already captured in `type_params_raw`.
        let mut type_parameters: Option<tsv_ts::TSTypeParameterDeclaration<'arena>> = None;
        let mut parameters: &'arena [Expression<'arena>] = &[];
        let mut raw_parameters: Option<&'arena str> = None;
        if type_params_raw.is_some() || !params_str.trim().is_empty() {
            // The head runs from where the signature begins (`<` or `(`) through the `)`.
            let head_slice = &content[head_start..=close_paren];
            const WRAPPER_PREFIX: &str = "function f";
            let wrapper = format!("{WRAPPER_PREFIX}{head_slice} {{}}");
            let base = (content_offset + head_start).saturating_sub(WRAPPER_PREFIX.len());
            match tsv_ts::parse_with_interner(&wrapper, base, Rc::clone(&self.interner), self.arena)
            {
                Ok(mut program) => {
                    self.expression_comments.append(&mut program.comments);
                    if let Some(tsv_ts::Statement::FunctionDeclaration(func)) = program.body.first()
                    {
                        type_parameters.clone_from(&func.type_parameters);
                        parameters = func.params;
                    }
                }
                // Keep the raw parameter text so nothing is dropped.
                Err(_) => {
                    if !params_str.trim().is_empty() {
                        raw_parameters = Some(self.alloc_str_in(params_str.trim()));
                    }
                }
            }
        }

        // Parse body
        let body = self.parse_block_children(&["snippet"], content_start)?;

        // Expect closing {/snippet}
        let end = self.expect_block_close_keyword("snippet", start)?;

        Ok(FragmentNode::SnippetBlock(SnippetBlock {
            expression,
            type_parameters,
            type_params_raw,
            parameters,
            raw_parameters,
            params_paren,
            body,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
            opening_tag_span,
        }))
    }

    /// Find the matching closing angle bracket for generics like `<T>` (the byte
    /// offset of the `>`). Used for TypeScript generics in snippet declarations.
    /// Comment- and string-aware via the shared cursor.
    ///
    /// Deliberately a naive `<`/`>` depth count, mirroring Svelte's own snippet-generic
    /// scanner (`match_bracket` with `pointy_bois`): a `>` from a `=>` / `>=` / `>>`
    /// decrements depth and closes the generic early. `parse_snippet_block` then requires
    /// a `(` immediately after, so such a head (a function type — `<T extends () => void>`,
    /// `<T = () => void>` — or any stray `>`) is rejected exactly as Svelte rejects it,
    /// rather than mis-sliced and corrupted on format.
    fn find_matching_angle_bracket(
        &self,
        content: &str,
        open_pos: usize,
    ) -> Result<usize, ParseError> {
        match_bracket(
            content.as_bytes(),
            open_pos,
            content.len(),
            b'<',
            b'>',
            TriviaProfile::JS,
        )
        .ok_or(ParseError::UnexpectedEof {
            position: content.len(),
            context: None,
        })
    }

    /// Scan source from a position until we find the closing } of a block tag
    /// Returns (content between start and }, position after })
    pub(super) fn scan_block_tag_content(
        &mut self,
        start: usize,
    ) -> Result<(&'a str, usize), ParseError> {
        // Find the block tag's closing `}` (skips strings/comments/regex). `start`
        // is just after the `{#…`/`{@…` keyword, so the opening `{` is the depth-1
        // brace that `scan_to_matching_brace` matches.
        let Some(end) = scan_to_matching_brace(self.source.as_bytes(), start) else {
            return Err(self.error_unclosed_at("block tag", start));
        };

        let content = &self.source[start..end];

        // Reposition the lexer past `}`. Block tags only occur in template content,
        // so `inside_tag` is already `false` (template mode) and stays that way for
        // the block body, which `advance_to_position` preserves.
        let after_close = end + 1; // Skip past the }
        self.advance_to_position(after_close)?;

        Ok((content, after_close))
    }

    /// Parse children of a block until we hit a closing or intermediate tag
    /// stop_keywords: keywords that should stop parsing (e.g., ["else", "if"] for if blocks)
    /// content_start: position to start capturing text from (position after opening tag's `}`)
    fn parse_block_children(
        &mut self,
        stop_keywords: &[&str],
        content_start: usize,
    ) -> Result<Fragment<'arena>, ParseError> {
        let mut nodes = self.bvec();
        let mut last_end = content_start;

        loop {
            // Capture text gaps
            self.capture_text_if_gap(last_end, &mut nodes)?;

            if self.check(TokenKind::Eof) {
                break;
            }

            // Check for block close {/keyword}
            if self.check(TokenKind::BlockClose)
                && stop_keywords.contains(&self.keyword_at(self.current_end))
            {
                break;
            }

            // Check for block continue {:keyword}
            if self.check(TokenKind::BlockContinue) {
                let keyword = self.continuation_keyword_at(self.current_end);

                // Stop when the continuation keyword begins with a stop keyword,
                // so the two-word `{:else if}` matches the `else` stop.
                let should_stop = stop_keywords.iter().any(|sk| keyword.starts_with(sk));

                if should_stop {
                    break;
                }
            }

            // Parse child nodes
            if self.check(TokenKind::Comment) {
                let comment = self.parse_comment()?;
                last_end = comment.span.end_usize();
                nodes.push(FragmentNode::Comment(comment));
            } else if self.check(TokenKind::LeftAngle) {
                // Check if closing tag
                if self.is_next_token(TokenKind::Slash)? {
                    break;
                }
                match self.parse_element_or_special()? {
                    ParsedElement::Element(elem) => {
                        last_end = elem.span.end_usize();
                        nodes.push(FragmentNode::Element(elem));
                    }
                    ParsedElement::SpecialElement(elem) => {
                        last_end = elem.span.end_usize();
                        nodes.push(FragmentNode::SpecialElement(elem));
                    }
                }
            } else if self.check(TokenKind::LeftBrace) {
                let tag = self.parse_brace_tag()?;
                last_end = tag.span().end_usize();
                nodes.push(tag);
            } else if self.check(TokenKind::BlockOpen) {
                let block = self.parse_block()?;
                last_end = block.span().end_usize();
                nodes.push(block);
            } else if self.check(TokenKind::TagOpen) {
                let tag = self.parse_template_tag()?;
                last_end = tag.span().end_usize();
                nodes.push(tag);
            } else {
                // Unknown token - might be text content that wasn't captured
                break;
            }
        }

        Ok(Fragment {
            nodes: nodes.into_bump_slice(),
        })
    }
}
