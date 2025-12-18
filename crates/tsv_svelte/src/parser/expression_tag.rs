// Expression tag parsing

use std::rc::Rc;

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::parser_impl::SvelteParser;

impl<'a> SvelteParser<'a> {
    /// Parse an expression tag: {expression}
    pub(crate) fn parse_expression_tag(&mut self) -> Result<ExpressionTag, ParseError> {
        let start = self.current_start;

        // Verify we're at opening brace
        if !self.check(TokenKind::LeftBrace) {
            return Err(self.error_expected_found("'{'"));
        }

        // Calculate expression start (after the '{')
        let expr_start = self.current_end;

        // Find matching closing brace with proper handling of nested braces, strings, and comments
        let source_bytes = self.source.as_bytes();
        let mut expr_end = expr_start;
        let mut found_close = false;
        let mut brace_depth = 1; // Already saw opening {
        let mut in_string = false;
        let mut string_char = '\0';
        let mut in_block_comment = false;
        let mut in_line_comment = false;
        let mut escape_next = false;

        // Scan raw source for matching closing brace
        let mut i = expr_start;
        while i < source_bytes.len() {
            let ch = source_bytes[i] as char;

            // Handle escape sequences in strings
            if in_string && escape_next {
                escape_next = false;
                i += 1;
                continue;
            }

            if in_string && ch == '\\' {
                escape_next = true;
                i += 1;
                continue;
            }

            // Handle line comment end (newline ends line comment)
            if in_line_comment {
                if ch == '\n' {
                    in_line_comment = false;
                }
                i += 1;
                continue;
            }

            // Handle strings (skip braces when inside)
            if !in_block_comment {
                if in_string {
                    if ch == string_char {
                        in_string = false;
                    }
                    i += 1;
                    continue;
                } else if ch == '"' || ch == '\'' || ch == '`' {
                    in_string = true;
                    string_char = ch;
                    i += 1;
                    continue;
                }
            }

            // Handle comments and regex (skip braces when inside)
            if !in_string {
                if in_block_comment {
                    // Block comment: /* ... */
                    if ch == '*' && i + 1 < source_bytes.len() && source_bytes[i + 1] as char == '/'
                    {
                        in_block_comment = false;
                        i += 2; // Skip */
                        continue;
                    }
                    i += 1;
                    continue;
                } else if ch == '/' && i + 1 < source_bytes.len() {
                    let next_char = source_bytes[i + 1] as char;
                    if next_char == '*' {
                        in_block_comment = true;
                        i += 2; // Skip /*
                        continue;
                    } else if next_char == '/' {
                        in_line_comment = true;
                        i += 2; // Skip //
                        continue;
                    } else if is_regex_start(source_bytes, i, expr_start) {
                        // Regex literal: /pattern/flags - skip to end
                        i = skip_regex_literal(source_bytes, i);
                        continue;
                    }
                    // Otherwise, treat / as division operator - continue normal processing
                }
            }

            // Count braces (only outside strings, comments, and regex)
            if !in_string && !in_block_comment {
                if ch == '{' {
                    brace_depth += 1;
                } else if ch == '}' {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        expr_end = i;
                        found_close = true;
                        break;
                    }
                }
            }

            i += 1;
        }

        if !found_close {
            return Err(self.error_unclosed_at("expression tag", start));
        }

        // Extract expression content
        let expr_content = &self.source[expr_start..expr_end];

        // Parse expression using TypeScript parser (with comments)
        let (expression, comments) = tsv_ts::parse_expression_with_comments(
            expr_content,
            expr_start,
            Rc::clone(&self.interner),
        )?;

        // Add expression comments to the parser's collection for later inclusion in Root.comments
        self.expression_comments.extend(comments);

        // The span end is right after the closing brace
        let end = expr_end + 1;

        // Recreate lexer AFTER the closing brace (not at it)
        // This way we don't need the lexer to produce a RightBrace token,
        // which allows '}' in template text to be treated as plain text.
        // This matches Svelte's parser behavior where '}' is consumed directly
        // after expression parsing, not tokenized.
        let remaining_source = &self.source[end..];

        // Save the lexer state before creating new lexer
        // This preserves the context (tag vs template) after expression parsing
        // Example: class={expr}> - we're still in tag mode after the }
        // Example: {expr}</div> - we're in template mode after the }
        let saved_inside_tag = self.lexer.inside_tag;
        let mut new_lexer = crate::lexer::Lexer::new(remaining_source);
        new_lexer.inside_tag = saved_inside_tag;

        // Get the first token at the new position (after the '}')
        let (token_kind, token_start, token_end) = {
            let token = new_lexer.next_token()?;
            (token.kind, token.start, token.end)
        };

        // Update parser state
        self.lexer = new_lexer;
        self.base_offset = end;
        self.current_kind = token_kind;
        self.current_start = end + token_start;
        self.current_end = end + token_end;
        self.peek_cache = None;

        Ok(ExpressionTag {
            expression,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }
}

/// Determine if `/` at position `slash_pos` is starting a regex literal.
///
/// Uses context: if the previous non-whitespace character could end an expression
/// (identifier, number, `)`, `]`), then `/` is likely division.
/// Otherwise, it's likely a regex start.
fn is_regex_start(source: &[u8], slash_pos: usize, expr_start: usize) -> bool {
    // Find the previous non-whitespace character
    let mut j = slash_pos;
    while j > expr_start {
        j -= 1;
        let ch = source[j] as char;
        if !ch.is_ascii_whitespace() {
            // Characters that END an expression - / after these is DIVISION
            // Identifier chars (a-z, A-Z, 0-9, _), ), ], numbers
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == ')' || ch == ']' {
                return false;
            }
            // Characters that could PRECEDE a regex - / after these is REGEX
            // (, [, {, ,, ;, :, =, !, ~, +, -, *, /, %, <, >, &, |, ^, ?, arrow (>)
            return true;
        }
    }
    // At start of expression, / is likely regex (e.g., {/pattern/})
    true
}

/// Skip past a regex literal starting at `start_pos`, returning position after the regex.
///
/// Handles escape sequences, character classes `[...]`, and regex flags.
fn skip_regex_literal(source: &[u8], start_pos: usize) -> usize {
    let mut i = start_pos + 1; // Move past opening /

    while i < source.len() {
        let ch = source[i] as char;

        if ch == '\\' && i + 1 < source.len() {
            // Escape sequence - skip next char
            i += 2;
        } else if ch == '/' {
            // Found closing / - skip it and any flags
            i += 1;
            while i < source.len() && (source[i] as char).is_ascii_lowercase() {
                i += 1;
            }
            return i;
        } else if ch == '[' {
            // Character class - skip to closing ]
            i += 1;
            while i < source.len() {
                let class_ch = source[i] as char;
                if class_ch == '\\' && i + 1 < source.len() {
                    i += 2;
                } else if class_ch == ']' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
        } else {
            i += 1;
        }
    }

    i
}
