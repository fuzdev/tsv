// Control flow block parsing
//
// Handles: {#if}, {#each}, {#await}, {#key} blocks

use std::rc::Rc;

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::parser_impl::SvelteParser;

impl<'a> SvelteParser<'a> {
    /// Parse a control flow block starting with {#
    ///
    /// Dispatches to specific block parsers based on the keyword.
    pub(crate) fn parse_block(&mut self) -> Result<FragmentNode, ParseError> {
        let start = self.current_start;

        // We're at {#, consume it
        if !self.check(TokenKind::BlockOpen) {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected '{{#', found {}", self.current_kind),
                position: self.current_start,
                context: None,
            });
        }

        // Look at the source to determine the block type
        // After {# we expect a keyword: if, each, await, key
        let after_open = self.current_end;
        let remaining = &self.source[after_open..];

        // Find the keyword
        let keyword_end = remaining
            .find(|c: char| !c.is_alphabetic())
            .unwrap_or(remaining.len());
        let keyword = &remaining[..keyword_end];

        match keyword {
            "if" => self.parse_if_block(start),
            "each" => self.parse_each_block(start),
            "await" => self.parse_await_block(start),
            "key" => self.parse_key_block(start),
            _ => Err(ParseError::InvalidSyntax {
                message: format!("Unknown block type: {{#{keyword}}}"),
                position: start,
                context: None,
            }),
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

        // Extract the expression (skip "if " or "else if " prefix)
        let expr_str = if is_elseif {
            expr_content
                .strip_prefix("else if ")
                .or_else(|| expr_content.strip_prefix("else if"))
                .unwrap_or(expr_content)
                .trim()
        } else {
            expr_content
                .strip_prefix("if ")
                .or_else(|| expr_content.strip_prefix("if"))
                .unwrap_or(expr_content)
                .trim()
        };

        // Parse the test expression
        let expr_offset = tag_content_start + expr_content.find(expr_str).unwrap_or(0);

        let test = tsv_ts::parse_expression(expr_str, expr_offset, Rc::clone(&self.interner))?;

        // Parse consequent (content until {:else}, {:else if}, or {/if})
        let consequent = self.parse_block_children(&["else", "if"], content_start)?;

        // Check for alternate branch
        let alternate = if self.check(TokenKind::BlockContinue) {
            // Peek at what follows {:
            let after_continue = self.current_end;
            let remaining = &self.source[after_continue..];
            let keyword_end = remaining
                .find(|c: char| !c.is_alphabetic() && c != ' ')
                .unwrap_or(remaining.len());
            let keyword = remaining[..keyword_end].trim();

            if keyword.starts_with("else if") {
                // {:else if} - parse as nested if block
                let elseif_start = self.current_start;
                let elseif_block = self.parse_if_block_inner(elseif_start, true)?;
                Some(Fragment {
                    nodes: vec![elseif_block],
                })
            } else if keyword == "else" {
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

        // Expect closing {/if}
        let end = if self.check(TokenKind::BlockClose) {
            let close_tag_start = self.current_end;
            let (_, after_close) = self.scan_block_tag_content(close_tag_start)?; // consume "if}"
            after_close
        } else {
            self.current_start
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
        }))
    }

    /// Parse an each block: {#each expression as context, index (key)}...{:else}...{/each}
    fn parse_each_block(&mut self, start: usize) -> Result<FragmentNode, ParseError> {
        // Get the content start position (after {#)
        let tag_content_start = self.current_end;

        // Scan to find closing } and extract content
        let (tag_content, content_start) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "each expression as context, index (key)"
        // Strip "each " prefix
        let each_prefix_len = if tag_content.starts_with("each ") { 5 } else { 0 };
        let content = &tag_content[each_prefix_len..];
        let content_offset = tag_content_start + each_prefix_len;

        // Use partial parsing for the iterable expression - stops at identifiers like "as"
        // This correctly handles cases like `getItems(" as ")` where " as " is inside a string
        let (expression, expr_end_pos) = tsv_ts::parse_expression_partial(
            content.trim_start(),
            content_offset + (content.len() - content.trim_start().len()),
            Rc::clone(&self.interner),
        )?;

        // After the expression, check for " as " or ", index" or just "}"
        let expr_consumed = expr_end_pos - content_offset;
        let after_expr = &content[expr_consumed..];

        // Try to strip " as " to get binding (with context pattern)
        let (context, index, key) = if let Some(binding_str) = after_expr
            .strip_prefix(" as ")
            .or_else(|| after_expr.trim_start().strip_prefix("as "))
        {
            // Has `as` clause: parse context pattern, index, and key
            let as_len = after_expr.len() - binding_str.len();
            let binding_offset = content_offset + expr_consumed + as_len;
            let (ctx, idx, k) = self.parse_each_binding(binding_str, binding_offset)?;
            (Some(ctx), idx, k)
        } else {
            // No `as` clause: {#each expr} or {#each expr, index}
            // Check for ", index" syntax
            let trimmed = after_expr.trim();
            if let Some(rest) = trimmed.strip_prefix(',') {
                // {#each expr, index} - just index, no context
                let index_str = rest.trim().to_string();
                (None, Some(index_str), None)
            } else {
                // {#each expr} - no context, no index
                (None, None, None)
            }
        };

        // Parse body
        let body = self.parse_block_children(&["else", "each"], content_start)?;

        // Check for fallback
        let fallback = if self.check(TokenKind::BlockContinue) {
            let else_tag_start = self.current_end;
            let (_, else_content_start) = self.scan_block_tag_content(else_tag_start)?; // consume "else}"
            Some(self.parse_block_children(&["each"], else_content_start)?)
        } else {
            None
        };

        // Expect closing {/each}
        let end = if self.check(TokenKind::BlockClose) {
            let close_tag_start = self.current_end;
            let (_, after_close) = self.scan_block_tag_content(close_tag_start)?; // consume "each}"
            after_close
        } else {
            self.current_start
        };

        Ok(FragmentNode::EachBlock(EachBlock {
            expression,
            context,
            index,
            key,
            body,
            fallback,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
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
    fn parse_each_binding(
        &self,
        binding: &str,
        binding_offset: usize,
    ) -> Result<(tsv_ts::Expression, Option<String>, Option<tsv_ts::Expression>), ParseError> {
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
        let (index, key) = self.parse_index_and_key_after_context(remaining, pattern_end)?;

        Ok((context, index, key))
    }

    /// Parse a context pattern: identifier or destructuring pattern.
    /// Like Svelte's read_pattern, this stops at whitespace/comma/paren for identifiers.
    fn parse_context_pattern(
        &self,
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
            let expr = tsv_ts::parse_expression(
                pattern_str,
                adjusted,
                Rc::clone(&self.interner),
            )?;
            Ok((expr, adjusted + end))
        } else {
            // Simple identifier - read until non-identifier char
            let end = trimmed
                .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')
                .unwrap_or(trimmed.len());
            if end == 0 {
                return Err(ParseError::InvalidSyntax {
                    message: "Expected identifier or pattern".to_string(),
                    position: offset,
                    context: None,
                });
            }
            let ident_str = &trimmed[..end];
            let expr = tsv_ts::parse_expression(
                ident_str,
                adjusted,
                Rc::clone(&self.interner),
            )?;
            Ok((expr, adjusted + end))
        }
    }

    /// Find the matching closing bracket for a string starting with { or [
    fn find_matching_bracket(&self, input: &str) -> Result<usize, ParseError> {
        let open = input.chars().next().unwrap();
        let close = match open {
            '{' => '}',
            '[' => ']',
            _ => {
                return Err(ParseError::InvalidSyntax {
                    message: "Expected { or [".to_string(),
                    position: 0,
                    context: None,
                })
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
    fn parse_index_and_key_after_context(
        &self,
        remaining: &str,
        remaining_offset: usize,
    ) -> Result<(Option<String>, Option<tsv_ts::Expression>), ParseError> {
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
        let key = if rest_trimmed.starts_with('(') && rest_trimmed.ends_with(')') {
            let key_str = &rest_trimmed[1..rest_trimmed.len() - 1];
            let key_ws = rest.len() - rest_trimmed.len();
            let key_offset = rest_offset + key_ws + 1; // +1 for '('
            Some(tsv_ts::parse_expression(
                key_str.trim(),
                key_offset + (key_str.len() - key_str.trim_start().len()),
                Rc::clone(&self.interner),
            )?)
        } else {
            None
        };

        Ok((index, key))
    }

    /// Parse an await block: {#await expression}...{:then value}...{:catch error}...{/await}
    fn parse_await_block(&mut self, start: usize) -> Result<FragmentNode, ParseError> {
        // Get the content start position (after {#)
        let tag_content_start = self.current_end;

        // Scan to find closing } and extract content
        let (tag_content, content_start) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "await expression" or "await expression then value"
        // Strip "await " prefix
        let await_prefix_len = if tag_content.starts_with("await ") { 6 } else { 0 };
        let content = &tag_content[await_prefix_len..];
        let content_offset = tag_content_start + await_prefix_len;

        // Use partial parsing for the promise expression
        // This correctly handles cases like `fetch(" then ")` where " then " is inside a string
        let (expression, expr_end_pos) = tsv_ts::parse_expression_partial(
            content.trim_start(),
            content_offset + (content.len() - content.trim_start().len()),
            Rc::clone(&self.interner),
        )?;

        // Check what follows the expression
        let expr_consumed = expr_end_pos - content_offset;
        let after_expr = &content[expr_consumed..];

        // Check for shorthand: {#await promise then value}
        let shorthand_value = if let Some(rest) = after_expr.strip_prefix(" then ") {
            Some(rest)
        } else if let Some(rest) = after_expr.trim_start().strip_prefix("then ") {
            Some(rest)
        } else if after_expr.trim() == "then" || after_expr.trim_start().starts_with("then}") {
            Some("")
        } else {
            None
        };

        let (pending, then_fragment, catch_fragment, value, error, end) = if let Some(value_str) =
            shorthand_value
        {
            // Shorthand syntax: no pending block
            let value = if !value_str.is_empty() {
                // Calculate offset: we know value_str comes after "expression then "
                let then_keyword_end = expr_end_pos + (after_expr.len() - value_str.len());
                let value_trimmed = value_str.trim_start();
                let value_offset = then_keyword_end + (value_str.len() - value_trimmed.len());
                Some(tsv_ts::parse_expression(
                    value_trimmed,
                    value_offset,
                    Rc::clone(&self.interner),
                )?)
            } else {
                None
            };

            let then_content = self.parse_block_children(&["await"], content_start)?;

            // Expect closing {/await}
            let block_end = if self.check(TokenKind::BlockClose) {
                let close_tag_start = self.current_end;
                let (_, after_close) = self.scan_block_tag_content(close_tag_start)?;
                after_close
            } else {
                self.current_start
            };

            (None, Some(then_content), None, value, None, block_end)
        } else {
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
            while self.check(TokenKind::BlockContinue) {
                let continue_start = self.current_end;
                let remaining = &self.source[continue_start..];
                let keyword_end = remaining
                    .find(|c: char| !c.is_alphabetic() && c != ' ')
                    .unwrap_or(remaining.len());
                let keyword = remaining[..keyword_end].trim();

                if keyword.starts_with("then") {
                    let then_tag_start = self.current_end;
                    let (then_tag_content, then_content_start) =
                        self.scan_block_tag_content(then_tag_start)?;
                    let value_str = then_tag_content
                        .strip_prefix("then ")
                        .or_else(|| then_tag_content.strip_prefix("then"))
                        .unwrap_or("")
                        .trim();

                    if !value_str.is_empty() {
                        let value_offset =
                            then_tag_start + then_tag_content.find(value_str).unwrap_or(0);
                        value = Some(tsv_ts::parse_expression(
                            value_str,
                            value_offset,
                            Rc::clone(&self.interner),
                        )?);
                    }

                    then_fragment =
                        Some(self.parse_block_children(&["catch", "await"], then_content_start)?);
                } else if keyword.starts_with("catch") {
                    let catch_tag_start = self.current_end;
                    let (catch_tag_content, catch_content_start) =
                        self.scan_block_tag_content(catch_tag_start)?;
                    let error_str = catch_tag_content
                        .strip_prefix("catch ")
                        .or_else(|| catch_tag_content.strip_prefix("catch"))
                        .unwrap_or("")
                        .trim();

                    if !error_str.is_empty() {
                        let error_offset =
                            catch_tag_start + catch_tag_content.find(error_str).unwrap_or(0);
                        error = Some(tsv_ts::parse_expression(
                            error_str,
                            error_offset,
                            Rc::clone(&self.interner),
                        )?);
                    }

                    catch_fragment =
                        Some(self.parse_block_children(&["await"], catch_content_start)?);
                } else {
                    break;
                }
            }

            // Expect closing {/await}
            let block_end = if self.check(TokenKind::BlockClose) {
                let close_tag_start = self.current_end;
                let (_, after_close) = self.scan_block_tag_content(close_tag_start)?;
                after_close
            } else {
                self.current_start
            };

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
        }))
    }

    /// Parse a key block: {#key expression}...{/key}
    fn parse_key_block(&mut self, start: usize) -> Result<FragmentNode, ParseError> {
        // Get the content start position (after {#)
        let tag_content_start = self.current_end;

        // Scan to find closing } and extract content
        let (tag_content, content_start) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "key expression"
        let expr_str = tag_content
            .strip_prefix("key ")
            .unwrap_or(tag_content)
            .trim();

        let expr_offset = tag_content_start + tag_content.find(expr_str).unwrap_or(0);
        let expression =
            tsv_ts::parse_expression(expr_str, expr_offset, Rc::clone(&self.interner))?;

        // Parse fragment
        let fragment = self.parse_block_children(&["key"], content_start)?;

        // Expect closing {/key}
        let end = if self.check(TokenKind::BlockClose) {
            let close_tag_start = self.current_end;
            let (_, after_close) = self.scan_block_tag_content(close_tag_start)?;
            after_close
        } else {
            self.current_start
        };

        Ok(FragmentNode::KeyBlock(KeyBlock {
            expression,
            fragment,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        }))
    }

    /// Scan source from a position until we find the closing } of a block tag
    /// Returns (content between start and }, position after })
    fn scan_block_tag_content(&mut self, start: usize) -> Result<(&'a str, usize), ParseError> {
        let source_bytes = self.source.as_bytes();
        let mut end = start;
        let mut brace_depth = 0;
        let mut in_string = false;
        let mut string_char = '\0';
        let mut escape_next = false;

        for (i, &byte) in source_bytes.iter().enumerate().skip(start) {
            let ch = byte as char;

            if in_string && escape_next {
                escape_next = false;
                continue;
            }

            if in_string && ch == '\\' {
                escape_next = true;
                continue;
            }

            if in_string {
                if ch == string_char {
                    in_string = false;
                }
            } else if ch == '"' || ch == '\'' || ch == '`' {
                in_string = true;
                string_char = ch;
            } else if ch == '{' {
                brace_depth += 1;
            } else if ch == '}' {
                if brace_depth == 0 {
                    end = i;
                    break;
                }
                brace_depth -= 1;
            }
        }

        let content = &self.source[start..end];

        // Recreate lexer at the position after }
        let after_close = end + 1; // Skip past the }
        let remaining_source = &self.source[after_close..];
        let mut new_lexer = crate::lexer::Lexer::new(remaining_source);
        new_lexer.inside_tag = false; // Back to template mode

        let (token_kind, token_start, token_end) = {
            let token = new_lexer.next_token()?;
            (token.kind, token.start, token.end)
        };

        self.lexer = new_lexer;
        self.base_offset = after_close;
        self.current_kind = token_kind;
        self.current_start = after_close + token_start;
        self.current_end = after_close + token_end;
        self.peek_cache = None;

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
            if self.check(TokenKind::BlockClose) {
                let after_close = self.current_end;
                let remaining = &self.source[after_close..];
                let keyword_end = remaining
                    .find(|c: char| !c.is_alphabetic())
                    .unwrap_or(remaining.len());
                let keyword = &remaining[..keyword_end];

                if stop_keywords.contains(&keyword) {
                    break;
                }
            }

            // Check for block continue {:keyword}
            if self.check(TokenKind::BlockContinue) {
                let after_continue = self.current_end;
                let remaining = &self.source[after_continue..];
                let keyword_end = remaining
                    .find(|c: char| !c.is_alphabetic() && c != ' ')
                    .unwrap_or(remaining.len());
                let keyword = remaining[..keyword_end].trim();

                // Check if any stop keyword starts with or matches this keyword
                let should_stop = stop_keywords
                    .iter()
                    .any(|sk| keyword.starts_with(sk) || keyword == *sk);

                if should_stop {
                    break;
                }
            }

            // Parse child nodes
            if self.check(TokenKind::Comment) {
                let comment = self.parse_comment()?;
                last_end = comment.span.end as usize;
                nodes.push(FragmentNode::Comment(comment));
            } else if self.check(TokenKind::LeftAngle) {
                // Check if closing tag
                if self.is_next_token(TokenKind::Slash)? {
                    break;
                }
                let element = self.parse_element()?;
                last_end = element.span.end as usize;
                nodes.push(FragmentNode::Element(element));
            } else if self.check(TokenKind::LeftBrace) {
                let expr = self.parse_expression_tag()?;
                last_end = expr.span.end as usize;
                nodes.push(FragmentNode::ExpressionTag(expr));
            } else if self.check(TokenKind::BlockOpen) {
                let block = self.parse_block()?;
                last_end = block.span().end as usize;
                nodes.push(block);
            } else {
                // Unknown token - might be text content that wasn't captured
                break;
            }
        }

        Ok(Fragment { nodes })
    }
}
