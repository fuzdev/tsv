// Control flow block parsing
//
// Handles: {#if}, {#each}, {#await}, {#key} blocks

use std::rc::Rc;

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use crate::parser::element::ParsedElement;
use tsv_lang::{ParseError, Span};

use super::expression_tag::scan_to_matching_brace;
use super::parser_impl::SvelteParser;

/// Find the position of the LAST top-level ` as ` keyword in a string.
///
/// "Top-level" means not inside `()`, `[]`, `{}`, or `<>` brackets, and not inside string
/// literals. Returns the byte offset of the space before `as`, or None if not found.
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
        match bytes[i] {
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b')' | b']' | b'}' | b'>' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            b'\'' | b'"' | b'`' => {
                let quote = bytes[i];
                i += 1;
                while i < len && bytes[i] != quote {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
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

/// Return type for parse_each_binding: (context, index, key_expr, key_span)
type EachBindingResult = (
    tsv_ts::Expression,
    Option<String>,
    Option<tsv_ts::Expression>,
    Option<Span>,
);

/// Return type for parse_index_and_key_after_context: (index, key_expr, key_span)
type IndexAndKeyResult = (Option<String>, Option<tsv_ts::Expression>, Option<Span>);

impl<'a> SvelteParser<'a> {
    /// Parse a control flow block starting with {#
    ///
    /// Dispatches to specific block parsers based on the keyword.
    pub(crate) fn parse_block(&mut self) -> Result<FragmentNode, ParseError> {
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
    fn parse_if_block(&mut self, start: usize) -> Result<FragmentNode, ParseError> {
        self.parse_if_block_inner(start, false)
    }

    /// Inner parser for if blocks (handles both {#if} and {:else if})
    fn parse_if_block_inner(
        &mut self,
        start: usize,
        is_elseif: bool,
    ) -> Result<FragmentNode, ParseError> {
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
        let expr_offset = tag_content_start + super::subslice_offset(expr_content, expr_str);

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
            // Peek at what follows {:
            let keyword = self.continuation_keyword_at(self.current_end);
            // Normalize whitespace: "else  if" -> "else if"
            let keyword_normalized: String = keyword
                .split_whitespace()
                .take(2)
                .collect::<Vec<_>>()
                .join(" ");

            if keyword_normalized == "else if" {
                // {:else if} - parse as nested if block
                let elseif_start = self.current_start;
                let elseif_block = self.parse_if_block_inner(elseif_start, true)?;
                Some(Fragment {
                    nodes: vec![elseif_block],
                })
            } else if keyword_normalized == "else" {
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
    fn parse_each_block(&mut self, start: usize) -> Result<FragmentNode, ParseError> {
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
        let (expression, context, index, key, key_span) = if let Some(binding_str) = after_expr
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
                let (ctx, idx, k, k_span) =
                    self.parse_each_binding(real_binding, real_binding_offset)?;
                (expression, Some(ctx), idx, k, k_span)
            } else {
                // Normal case: first `as` is the Svelte binding separator
                let (ctx, idx, k, k_span) = self.parse_each_binding(binding_str, binding_offset)?;
                (expression, Some(ctx), idx, k, k_span)
            }
        } else {
            // No `as` clause: {#each expr} or {#each expr, index}
            // Check for ", index" syntax
            let trimmed = after_expr.trim();
            if let Some(rest) = trimmed.strip_prefix(',') {
                // {#each expr, index} - just index, no context
                let index_str = rest.trim().to_string();
                (expression, None, Some(index_str), None, None)
            } else {
                // {#each expr} - no context, no index
                (expression, None, None, None, None)
            }
        };

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
    ) -> Result<EachBindingResult, ParseError> {
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
        let (index, key, key_span) =
            self.parse_index_and_key_after_context(remaining, pattern_end)?;

        Ok((context, index, key, key_span))
    }

    /// Parse a context pattern: identifier or destructuring pattern.
    /// Like Svelte's read_pattern, this stops at whitespace/comma/paren for identifiers.
    fn parse_context_pattern(
        &mut self,
        input: &str,
        offset: usize,
    ) -> Result<(tsv_ts::Expression, usize), ParseError> {
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
                .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')
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
                )?;
                if let tsv_ts::Expression::Identifier(id) = &mut expr {
                    id.type_annotation = Some(ta);
                }
                Ok((expr, type_end))
            } else {
                Ok((expr, adjusted + end))
            }
        }
    }

    /// Find the matching closing bracket for a string starting with { or [
    fn find_matching_bracket(&self, input: &str) -> Result<usize, ParseError> {
        let (open, close) = match input.chars().next() {
            Some('{') => ('{', '}'),
            Some('[') => ('[', ']'),
            _ => {
                return Err(ParseError::InvalidSyntax {
                    message: "Expected { or [".to_string(),
                    position: 0,
                    context: None,
                });
            }
        };

        let mut depth = 0;
        let mut in_string = false;
        let mut string_char = '"';

        for (i, c) in input.char_indices() {
            if in_string {
                if c == string_char && !input[..i].ends_with('\\') {
                    in_string = false;
                }
            } else {
                match c {
                    '"' | '\'' | '`' => {
                        in_string = true;
                        string_char = c;
                    }
                    c if c == open => depth += 1,
                    c if c == close => {
                        depth -= 1;
                        if depth == 0 {
                            return Ok(i + 1); // Include closing bracket
                        }
                    }
                    _ => {}
                }
            }
        }

        Err(ParseError::InvalidSyntax {
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
    ) -> Result<IndexAndKeyResult, ParseError> {
        let trimmed = remaining.trim_start();
        let ws_len = remaining.len() - trimmed.len();
        let offset = remaining_offset + ws_len;

        let mut rest = trimmed;
        let mut rest_offset = offset;
        let mut index = None;

        // Check for ", index"
        if let Some(after_comma) = rest.strip_prefix(',') {
            let after_comma_trimmed = after_comma.trim_start();
            let comma_ws = after_comma.len() - after_comma_trimmed.len();

            // Read index identifier (until whitespace or '(')
            let idx_end = after_comma_trimmed
                .find(|c: char| c.is_whitespace() || c == '(')
                .unwrap_or(after_comma_trimmed.len());

            if idx_end > 0 {
                index = Some(after_comma_trimmed[..idx_end].to_string());
                rest = &after_comma_trimmed[idx_end..];
                rest_offset = offset + 1 + comma_ws + idx_end;
            }
        }

        // Check for "(key)"
        let rest_trimmed = rest.trim_start();
        let (key, key_span) = if rest_trimmed.starts_with('(') && rest_trimmed.ends_with(')') {
            let key_str = &rest_trimmed[1..rest_trimmed.len() - 1];
            let key_ws = rest.len() - rest_trimmed.len();
            let key_offset = rest_offset + key_ws + 1; // +1 for '('
            let key_expr = self.parse_ts_expression(
                key_str.trim(),
                key_offset + (key_str.len() - key_str.trim_start().len()),
            )?;
            // Span includes the parentheses: from '(' to after ')'
            let span_start = (rest_offset + key_ws) as u32;
            let span_end = (rest_offset + key_ws + rest_trimmed.len()) as u32;
            (
                Some(key_expr),
                Some(tsv_lang::Span::new(span_start, span_end)),
            )
        } else {
            (None, None)
        };

        Ok((index, key, key_span))
    }

    /// Parse an await block: {#await expression}...{:then value}...{:catch error}...{/await}
    fn parse_await_block(&mut self, start: usize) -> Result<FragmentNode, ParseError> {
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

        let (pending, then_fragment, catch_fragment, value, error, end) = if let Some(value_str) =
            shorthand_then
        {
            // Shorthand then syntax: {#await promise then value}...{/await}
            let value = if !value_str.is_empty() {
                // Calculate offset: we know value_str comes after "expression then "
                let then_keyword_end = expr_end_pos + (after_expr.len() - value_str.len());
                let value_trimmed = value_str.trim_start();
                let value_offset = then_keyword_end + (value_str.len() - value_trimmed.len());
                Some(self.parse_ts_pattern(value_trimmed, value_offset)?)
            } else {
                None
            };

            let then_content = self.parse_block_children(&["catch", "await"], content_start)?;

            // Check for optional {:catch} continuation after then-shorthand
            let (catch_fragment, error) = if self.check_await_continuation("catch") {
                self.parse_await_catch_continuation()?
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
                // Calculate offset: we know error_str comes after "expression catch "
                let catch_keyword_end = expr_end_pos + (after_expr.len() - error_str.len());
                let error_trimmed = error_str.trim_start();
                let error_offset = catch_keyword_end + (error_str.len() - error_trimmed.len());
                Some(self.parse_ts_pattern(error_trimmed, error_offset)?)
            } else {
                None
            };

            let catch_content = self.parse_block_children(&["then", "await"], content_start)?;

            // Check for optional {:then} continuation after catch-shorthand
            let (then_fragment, value) = if self.check_await_continuation("then") {
                self.parse_await_then_continuation(&["await"])?
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
            let trailing = after_expr.trim_start();
            if !trailing.is_empty() {
                let trailing_start = expr_end_pos + (after_expr.len() - trailing.len());
                return Err(self.error_expected_at("'}'", trailing_start));
            }

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
                    let (frag, val) = self.parse_await_then_continuation(&["catch", "await"])?;
                    then_fragment = frag;
                    value = val;
                } else if self.check_await_continuation("catch") {
                    let (frag, err) = self.parse_await_catch_continuation()?;
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

    /// Parse a {:catch error} continuation block within an await block.
    /// Returns (catch_fragment, error_pattern) if a catch continuation is found.
    fn parse_await_catch_continuation(
        &mut self,
    ) -> Result<(Option<Fragment>, Option<tsv_ts::Expression>), ParseError> {
        let catch_tag_start = self.current_end;
        let (catch_tag_content, catch_content_start) =
            self.scan_block_tag_content(catch_tag_start)?;
        let error_str = self
            .strip_keyword_value(catch_tag_content, "catch", catch_tag_start)?
            .trim();

        let error = if !error_str.is_empty() {
            let error_offset =
                catch_tag_start + super::subslice_offset(catch_tag_content, error_str);
            Some(self.parse_ts_pattern(error_str, error_offset)?)
        } else {
            None
        };

        let catch_fragment = self.parse_block_children(&["await"], catch_content_start)?;
        Ok((Some(catch_fragment), error))
    }

    /// Parse a {:then value} continuation block within an await block.
    /// Returns (then_fragment, value_pattern) if a then continuation is found.
    fn parse_await_then_continuation(
        &mut self,
        stop_keywords: &[&str],
    ) -> Result<(Option<Fragment>, Option<tsv_ts::Expression>), ParseError> {
        let then_tag_start = self.current_end;
        let (then_tag_content, then_content_start) = self.scan_block_tag_content(then_tag_start)?;
        let value_str = self
            .strip_keyword_value(then_tag_content, "then", then_tag_start)?
            .trim();

        let value = if !value_str.is_empty() {
            let value_offset = then_tag_start + super::subslice_offset(then_tag_content, value_str);
            Some(self.parse_ts_pattern(value_str, value_offset)?)
        } else {
            None
        };

        let then_fragment = self.parse_block_children(stop_keywords, then_content_start)?;
        Ok((Some(then_fragment), value))
    }

    /// Read the leading alphabetic keyword at `pos` in the source — the `if` in
    /// `{#if}`, the `each` in `{/each}`, the `html` in `{@html}`. Stops at the
    /// first non-alphabetic byte (space, `}`, …); returns `""` when there is none.
    fn keyword_at(&self, pos: usize) -> &'a str {
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
    fn strip_block_keyword(
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
        let trailing = close_content[expected.len()..].trim_start();
        if !trailing.is_empty() {
            let trailing_start = close_tag_start + (close_content.len() - trailing.len());
            return Err(self.error_expected_at("'}'", trailing_start));
        }

        Ok(after_close)
    }

    /// Parse a key block: {#key expression}...{/key}
    fn parse_key_block(&mut self, start: usize) -> Result<FragmentNode, ParseError> {
        // Get the content start position (after {#)
        let tag_content_start = self.current_end;

        // Scan to find closing } and extract content
        let (tag_content, content_start) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "key expression" — Svelte requires whitespace after the keyword.
        let expr_str = self
            .strip_block_keyword(tag_content, "key", tag_content_start)?
            .trim();

        let expr_offset = tag_content_start + super::subslice_offset(tag_content, expr_str);
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
    fn parse_snippet_block(&mut self, start: usize) -> Result<FragmentNode, ParseError> {
        // Get the content start position (after {#)
        let tag_content_start = self.current_end;

        // Scan to find closing } and extract content
        let (tag_content, content_start) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "snippet name(params)" or "snippet name<T>(params)" — Svelte
        // requires whitespace after the keyword.
        let content = self
            .strip_block_keyword(tag_content, "snippet", tag_content_start)?
            .trim();

        // Find the name (identifier before < or ()
        // Need to handle: fn, fn<T>, fn<T, U>
        let generic_pos = content.find('<');
        let paren_pos = content.find('(').unwrap_or(content.len());

        // Extract type parameters if present (like Svelte's parser)
        let (name_end, type_parameters) = if let Some(gpos) = generic_pos {
            if gpos < paren_pos {
                // Found '<' before '(' - this is a generic type parameter
                // Find the matching '>'
                let close_pos = self.find_matching_angle_bracket(content, gpos)?;
                let type_params = content[gpos + 1..close_pos].to_string();
                (gpos, Some(type_params))
            } else {
                // '<' is after '(' - not a generic, probably a comparison in default value
                (paren_pos, None)
            }
        } else {
            (paren_pos, None)
        };

        let name_str = content[..name_end].trim();
        let name_offset = tag_content_start + super::subslice_offset(tag_content, name_str);
        let expression = self.parse_ts_expression(name_str, name_offset)?;

        // Opening tag span is from start to content_start (includes the closing })
        let opening_tag_span = Span {
            start: start as u32,
            end: content_start as u32,
        };

        // Parse parameters (between parentheses)
        let mut parameters = Vec::new();
        let mut raw_parameters = None;
        if paren_pos < content.len() {
            let close_paren = content.rfind(')').unwrap_or(content.len());
            let params_str = &content[paren_pos + 1..close_paren];
            if !params_str.trim().is_empty() {
                // Compute params_offset (shared by both branches)
                let params_offset =
                    tag_content_start + super::subslice_offset(tag_content, params_str);

                if params_str.contains(':') {
                    // Parse typed parameters by wrapping as a function signature
                    const WRAPPER_PREFIX: &str = "function f(";
                    let wrapper = format!("{WRAPPER_PREFIX}{params_str}) {{}}");
                    let base = params_offset.saturating_sub(WRAPPER_PREFIX.len());
                    match tsv_ts::parse_with_interner(&wrapper, base, Rc::clone(&self.interner)) {
                        Ok(program) => {
                            if let Some(tsv_ts::Statement::FunctionDeclaration(func)) =
                                program.body.into_iter().next()
                            {
                                parameters = func.params;
                            }
                        }
                        Err(_) => {
                            // Fall back to raw string if parsing fails
                            raw_parameters = Some(params_str.trim().to_string());
                        }
                    }
                } else {
                    parameters = self.parse_snippet_parameters(params_str, params_offset)?;
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
            parameters,
            raw_parameters,
            body,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
            opening_tag_span,
        }))
    }

    /// Parse snippet parameters (comma-separated patterns with optional defaults)
    fn parse_snippet_parameters(
        &mut self,
        params: &str,
        base_offset: usize,
    ) -> Result<Vec<tsv_ts::Expression>, ParseError> {
        let mut parameters = Vec::new();
        let mut current_pos = 0;

        // Work with original params string to keep positions correct
        while current_pos < params.len() {
            let remaining = &params[current_pos..];
            let ws_len = remaining.len() - remaining.trim_start().len();
            let offset = base_offset + current_pos + ws_len;
            let remaining_trimmed = remaining.trim_start();

            if remaining_trimmed.is_empty() {
                break;
            }

            // Parse one parameter (pattern potentially with default value)
            let (param, param_end) = self.parse_context_pattern(remaining_trimmed, offset)?;

            // Check for default value: = expr
            let after_param = &params[param_end - base_offset..];
            let after_param_trimmed = after_param.trim_start();

            if after_param_trimmed.starts_with('=') {
                // Has default value - parse full parameter as a pattern
                // (e.g., `{a, b} = defaultObj` becomes AssignmentPattern)
                let next_comma = after_param_trimmed
                    .find(',')
                    .unwrap_or(after_param_trimmed.len());
                let full_param = &params[current_pos
                    ..param_end - base_offset + after_param.len() - after_param_trimmed.len()
                        + next_comma];
                let full_param_expr =
                    self.parse_ts_pattern(full_param.trim(), base_offset + current_pos + ws_len)?;
                parameters.push(full_param_expr);
                current_pos = param_end - base_offset + after_param.len()
                    - after_param_trimmed.len()
                    + next_comma;
            } else {
                parameters.push(param);
                current_pos = param_end - base_offset;
            }

            // Skip comma if present
            let after = &params[current_pos..];
            let after_trimmed = after.trim_start();
            if after_trimmed.starts_with(',') {
                current_pos += after.len() - after_trimmed.len() + 1;
            }
        }

        Ok(parameters)
    }

    /// Parse a template tag starting with {@
    ///
    /// Dispatches to specific tag parsers based on the keyword.
    pub(crate) fn parse_template_tag(&mut self) -> Result<FragmentNode, ParseError> {
        let start = self.current_start;

        // We're at {@, consume it
        if !self.check(TokenKind::TagOpen) {
            return Err(self.error_expected_found("'{@'"));
        }

        // After {@ we expect a tag keyword: html, const, debug, render
        let keyword = self.keyword_at(self.current_end);

        match keyword {
            "html" => self.parse_html_tag(start),
            "const" => self.parse_const_tag(start),
            "debug" => self.parse_debug_tag(start),
            "render" => self.parse_render_tag(start),
            _ => Err(self.error_unknown_at("template tag", &format!("{{@{keyword}}}"), start)),
        }
    }

    /// Parse an html tag: {@html expression}
    fn parse_html_tag(&mut self, start: usize) -> Result<FragmentNode, ParseError> {
        let tag_content_start = self.current_end;
        let (tag_content, after_close) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "html expression" — Svelte requires whitespace after the keyword.
        let expr_str = self
            .strip_block_keyword(tag_content, "html", tag_content_start)?
            .trim();

        let expr_offset = tag_content_start + super::subslice_offset(tag_content, expr_str);
        let expression = self.parse_ts_expression(expr_str, expr_offset)?;

        // End is right after the closing }
        let end = after_close;

        Ok(FragmentNode::HtmlTag(HtmlTag {
            expression,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        }))
    }

    /// Parse a const tag: {@const name = expression}
    fn parse_const_tag(&mut self, start: usize) -> Result<FragmentNode, ParseError> {
        let tag_content_start = self.current_end;
        let (tag_content, after_close) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "const name = expression" — Svelte requires whitespace after the keyword.
        let decl_str = self
            .strip_block_keyword(tag_content, "const", tag_content_start)?
            .trim();

        let decl_offset = tag_content_start + super::subslice_offset(tag_content, decl_str);

        // Find the = sign (accounting for destructuring patterns with nested =)
        // We need to find the top-level = that separates id from init
        let eq_pos = self.find_const_equals(decl_str)?;

        let id_str = decl_str[..eq_pos].trim();
        let init_str = decl_str[eq_pos + 1..].trim();

        let id_offset = decl_offset + super::subslice_offset(decl_str, id_str);
        let init_offset =
            decl_offset + eq_pos + 1 + (decl_str[eq_pos + 1..].len() - init_str.len());

        // Parse id as a pattern (identifier or destructuring)
        // Use parse_pattern to convert ObjectExpression/ArrayExpression to patterns
        let id = self.parse_ts_pattern(id_str, id_offset)?;

        // Parse init as an expression
        let init = self.parse_ts_expression(init_str, init_offset)?;

        let end = after_close;

        Ok(FragmentNode::ConstTag(ConstTag {
            id,
            init,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        }))
    }

    /// Find the top-level = in a const declaration (not inside brackets/braces)
    ///
    /// NOTE: The escape handling is simplified - it doesn't correctly handle
    /// escaped backslashes (e.g., `"test\\"` would be parsed incorrectly).
    /// This is unlikely to occur in real @const declarations.
    fn find_const_equals(&self, s: &str) -> Result<usize, ParseError> {
        let mut depth = 0;
        let mut in_string = false;
        let mut string_char = '"';

        for (i, c) in s.char_indices() {
            if in_string {
                if c == string_char && !s[..i].ends_with('\\') {
                    in_string = false;
                }
            } else {
                match c {
                    '"' | '\'' | '`' => {
                        in_string = true;
                        string_char = c;
                    }
                    '{' | '[' | '(' => depth += 1,
                    '}' | ']' | ')' => depth -= 1,
                    '=' if depth == 0 => return Ok(i),
                    _ => {}
                }
            }
        }

        Err(ParseError::InvalidSyntax {
            message: "Expected '=' in const declaration".to_string(),
            position: 0,
            context: None,
        })
    }

    /// Parse a debug tag: {@debug} or {@debug x, y, z}
    ///
    /// Unlike Prettier (which strips comments), we preserve TS comments in debug tags.
    /// Comments are extracted and stored in Root.comments for lookup by span.
    fn parse_debug_tag(&mut self, start: usize) -> Result<FragmentNode, ParseError> {
        let tag_content_start = self.current_end;
        let (tag_content, after_close) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "debug" or "debug x, y, z"
        // First get the part after "debug" keyword
        let (idents_str, idents_offset) = if let Some(stripped) = tag_content.strip_prefix("debug ")
        {
            let offset = tag_content_start + "debug ".len();
            (stripped, offset)
        } else if let Some(stripped) = tag_content.strip_prefix("debug") {
            let offset = tag_content_start + "debug".len();
            (stripped, offset)
        } else {
            ("", tag_content_start)
        };

        // Extract TS comments from the identifiers portion (preserves in Root.comments)
        // Returns content with comments replaced by spaces (positions preserved)
        let cleaned_idents = self.extract_ts_comments(idents_str, idents_offset);

        let mut identifiers = Vec::new();
        if !cleaned_idents.trim().is_empty() {
            // Parse comma-separated identifiers from the cleaned content
            // Since comments are replaced with equal-length spaces, positions are preserved
            let mut pos = 0;
            for chunk in cleaned_idents.split(',') {
                let trimmed = chunk.trim();
                if !trimmed.is_empty() {
                    // Find where trimmed content starts within this chunk
                    let trim_offset = super::subslice_offset(chunk, trimmed);
                    let ident_offset = idents_offset + pos + trim_offset;
                    let expr = self.parse_ts_expression(trimmed, ident_offset)?;
                    identifiers.push(expr);
                }
                pos += chunk.len() + 1; // +1 for the comma
            }
        }

        let end = after_close;

        Ok(FragmentNode::DebugTag(DebugTag {
            identifiers,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        }))
    }

    /// Parse a render tag: {@render fn()} or {@render fn?.()}
    fn parse_render_tag(&mut self, start: usize) -> Result<FragmentNode, ParseError> {
        let tag_content_start = self.current_end;
        let (tag_content, after_close) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "render expression" where expression must be a call — Svelte
        // requires whitespace after the keyword.
        let expr_str = self
            .strip_block_keyword(tag_content, "render", tag_content_start)?
            .trim();

        let expr_offset = tag_content_start + super::subslice_offset(tag_content, expr_str);
        let expression = self.parse_ts_expression(expr_str, expr_offset)?;

        let end = after_close;

        Ok(FragmentNode::RenderTag(RenderTag {
            expression,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        }))
    }

    /// Find the matching closing angle bracket for generics like `<T>`.
    /// Used for TypeScript generics in snippet declarations.
    /// Similar to Svelte's match_bracket utility.
    fn find_matching_angle_bracket(
        &self,
        content: &str,
        open_pos: usize,
    ) -> Result<usize, ParseError> {
        let bytes = content.as_bytes();
        let mut depth = 1;
        let mut i = open_pos + 1;
        let mut in_string = false;
        let mut string_char = 0u8;
        let mut escape_next = false;

        while i < bytes.len() && depth > 0 {
            let ch = bytes[i];

            if escape_next {
                escape_next = false;
                i += 1;
                continue;
            }

            if in_string {
                if ch == b'\\' {
                    escape_next = true;
                } else if ch == string_char {
                    in_string = false;
                }
                i += 1;
                continue;
            }

            match ch {
                b'"' | b'\'' | b'`' => {
                    in_string = true;
                    string_char = ch;
                }
                b'<' => depth += 1,
                b'>' => depth -= 1,
                _ => {}
            }
            i += 1;
        }

        if depth == 0 {
            Ok(i - 1) // Position of closing bracket
        } else {
            Err(ParseError::UnexpectedEof {
                position: content.len(),
                context: None,
            })
        }
    }

    /// Scan source from a position until we find the closing } of a block tag
    /// Returns (content between start and }, position after })
    fn scan_block_tag_content(&mut self, start: usize) -> Result<(&'a str, usize), ParseError> {
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
    ) -> Result<Fragment, ParseError> {
        let mut nodes = Vec::new();
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
                match self.parse_element_or_special(false)? {
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
                let expr = self.parse_expression_tag()?;
                last_end = expr.span.end_usize();
                nodes.push(FragmentNode::ExpressionTag(expr));
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

        Ok(Fragment { nodes })
    }
}
