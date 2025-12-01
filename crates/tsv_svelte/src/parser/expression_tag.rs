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
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected '{{', found {}", self.current_kind),
                position: self.current_start,
                context: None,
            });
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
        let mut in_comment = false;
        let mut escape_next = false;

        // Scan raw source for matching closing brace
        for (i, &byte) in source_bytes.iter().enumerate().skip(expr_start) {
            let ch = byte as char;

            // Handle escape sequences in strings
            if in_string && escape_next {
                escape_next = false;
                continue;
            }

            if in_string && ch == '\\' {
                escape_next = true;
                continue;
            }

            // Handle strings (skip braces when inside)
            if !in_comment {
                if in_string {
                    if ch == string_char {
                        in_string = false;
                    }
                } else if ch == '"' || ch == '\'' || ch == '`' {
                    in_string = true;
                    string_char = ch;
                }
            }

            // Handle comments (skip braces when inside)
            if !in_string {
                if in_comment {
                    // Block comment: /* ... */
                    if ch == '*' && i + 1 < source_bytes.len() && source_bytes[i + 1] as char == '/'
                    {
                        in_comment = false;
                    }
                } else if ch == '/' && i + 1 < source_bytes.len() {
                    let next_char = source_bytes[i + 1] as char;
                    if next_char == '*' {
                        in_comment = true;
                    }
                    // Note: Line comments (//) are not relevant in expressions inside {}
                    // since newlines break the expression anyway
                }
            }

            // Count braces (only outside strings and comments)
            if !in_string && !in_comment {
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
        }

        if !found_close {
            return Err(ParseError::InvalidSyntax {
                message: "Unterminated expression tag".to_string(),
                position: start,
                context: None,
            });
        }

        // Extract expression content
        let expr_content = &self.source[expr_start..expr_end];

        // Parse expression using TypeScript parser
        let expression =
            tsv_ts::parse_expression(expr_content, expr_start, Rc::clone(&self.interner))?;

        // Recreate lexer starting from the closing brace position
        // TODO(refactor): This lexer reconstruction pattern works but is somewhat unusual.
        // Alternative approaches to consider:
        // 1. "Skip mode" - lexer that skips unknown content until sync point
        // 2. Explicit position tracking without lexer replacement
        // 3. Unified lexer that understands both template and expression contexts
        // Current approach chosen for:
        // - Simplicity: Clean lexer state after expression
        // - Safety: No stale peek cache or position drift
        // - Performance: ~170ns overhead is negligible
        // Keep this pattern for now, but document for future review.
        let remaining_source = &self.source[expr_end..];

        // Save the lexer state before creating new lexer
        // This preserves the context (tag vs template) after expression parsing
        // Example: class={expr}> - we're still in tag mode after the }
        // Example: {expr}</div> - we're in template mode after the }
        // TODO(future optimization/redesign):
        // Could track ParsingContext explicitly in SvelteParser (Template vs TagAttributes enum)
        // and set lexer.inside_tag from parser context instead of saving/restoring.
        // Pros: More explicit parsing context available for error messages/validation.
        // Cons: ~3ns slower (branch instead of direct copy), parser state to maintain.
        // Current approach (save/restore) is simpler and slightly faster - YAGNI principle.
        let saved_inside_tag = self.lexer.inside_tag;
        let mut new_lexer = crate::lexer::Lexer::new(remaining_source);
        new_lexer.inside_tag = saved_inside_tag;

        // Get the first token (should be }) and extract its data
        let (token_kind, token_start, token_end) = {
            let token = new_lexer.next_token()?;
            (token.kind, token.start, token.end)
        };

        // Now we can move the lexer and set base_offset
        self.lexer = new_lexer;
        self.base_offset = expr_end; // Lexer's source starts at expr_end in full source
        self.current_kind = token_kind;
        self.current_start = expr_end + token_start;
        self.current_end = expr_end + token_end;
        self.peek_cache = None;

        // Verify it's the closing brace
        if !self.check(TokenKind::RightBrace) {
            return Err(ParseError::InvalidSyntax {
                message: format!("Expected '}}', found {}", self.current_kind),
                position: self.current_start,
                context: None,
            });
        }

        // Save the end position (right after the '}') before advancing
        // The lexer skips whitespace on advance, so we must capture end first
        let end = self.current_end;

        // Consume the closing brace
        self.advance()?;

        Ok(ExpressionTag {
            expression,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }
}
