// Expression tag parsing

use std::rc::Rc;

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::parser_impl::SvelteParser;

impl<'a> SvelteParser<'a> {
    /// Parse an expression tag `{expression}` at the current lexer position, then
    /// advance the lexer past the closing `}`.
    ///
    /// Used by callers that drive the token stream (template `{expr}` tags,
    /// directive values). Position-based callers that own their own cursor — the
    /// attribute-value sequence readers — use `parse_expression_tag_at`, which runs
    /// the same scan + parse without touching the lexer.
    pub(crate) fn parse_expression_tag(&mut self) -> Result<ExpressionTag, ParseError> {
        // Verify we're at opening brace
        if !self.check(TokenKind::LeftBrace) {
            return Err(self.error_expected_found("'{'"));
        }

        let tag = self.parse_expression_tag_at(self.current_start)?;

        // Resume lexing AFTER the closing brace (not at it), preserving tag-vs-template
        // context. Repositioning past `}` means the lexer never tokenizes it, so a `}`
        // in template text stays plain text — matching Svelte, which consumes `}`
        // directly after expression parsing (e.g. `class={expr}>` stays in tag mode,
        // `{expr}</div>` returns to template mode).
        self.advance_to_position(tag.span.end as usize)?;

        Ok(tag)
    }

    /// Scan and parse an expression tag `{expression}` starting at byte `brace_pos`
    /// (which must be `{`). The returned tag's span runs from `brace_pos` through the
    /// byte just past the matching `}` (`tag.span.end`).
    ///
    /// Unlike `parse_expression_tag`, this does **not** touch the lexer — the caller
    /// owns the cursor (the raw-byte attribute-value sequence readers reposition once
    /// when the whole value is done). The matching `}` is found by a raw scan that
    /// skips nested braces, string literals, line/block comments, and regex literals.
    pub(crate) fn parse_expression_tag_at(
        &mut self,
        brace_pos: usize,
    ) -> Result<ExpressionTag, ParseError> {
        debug_assert_eq!(
            self.source.as_bytes().get(brace_pos),
            Some(&b'{'),
            "parse_expression_tag_at must start at `{{`"
        );
        let start = brace_pos;
        let expr_start = brace_pos + 1; // after the '{'

        // Find the matching closing `}` — the one robust brace matcher.
        let Some(expr_end) = scan_to_matching_brace(self.source.as_bytes(), expr_start) else {
            return Err(self.error_unclosed_at("expression tag", start));
        };

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

        Ok(ExpressionTag {
            expression,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }
}

/// Find the `}` that closes the construct opened by a `{` just before
/// `scan_start`, skipping nested braces, string literals, line/block comments,
/// and regex literals. `scan_start` is the first byte to scan (the opening `{` is
/// counted as depth 1). Returns the byte offset of the matching `}`, or `None`
/// if the braces never balance.
///
/// The single robust brace matcher shared by every `{…}` construct — expression
/// tags, `{@…}` tags, `{...spread}`, and block tags — so none reimplements it
/// (and weaker copies can't desync on a `}` inside a regex/comment/string).
pub(crate) fn scan_to_matching_brace(bytes: &[u8], scan_start: usize) -> Option<usize> {
    let mut brace_depth = 1; // the opening `{` before scan_start
    let mut in_string = false;
    let mut string_char = '\0';
    let mut in_block_comment = false;
    let mut in_line_comment = false;
    let mut escape_next = false;

    let mut i = scan_start;
    while i < bytes.len() {
        let ch = bytes[i] as char;

        // Escape sequences inside strings
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

        // A newline ends a line comment
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        // Strings (braces inside are ignored)
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

        // Comments and regex (braces inside are ignored)
        if !in_string {
            if in_block_comment {
                if ch == '*' && i + 1 < bytes.len() && bytes[i + 1] as char == '/' {
                    in_block_comment = false;
                    i += 2; // skip */
                    continue;
                }
                i += 1;
                continue;
            } else if ch == '/' && i + 1 < bytes.len() {
                match bytes[i + 1] as char {
                    '*' => {
                        in_block_comment = true;
                        i += 2; // skip /*
                        continue;
                    }
                    '/' => {
                        in_line_comment = true;
                        i += 2; // skip //
                        continue;
                    }
                    _ if is_regex_start(bytes, i, scan_start) => {
                        i = skip_regex_literal(bytes, i);
                        continue;
                    }
                    // otherwise `/` is division — fall through
                    _ => {}
                }
            }
        }

        // Count braces only outside strings/comments/regex
        if !in_string && !in_block_comment {
            if ch == '{' {
                brace_depth += 1;
            } else if ch == '}' {
                brace_depth -= 1;
                if brace_depth == 0 {
                    return Some(i);
                }
            }
        }

        i += 1;
    }

    None
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
