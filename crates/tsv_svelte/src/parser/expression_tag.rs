// Expression tag parsing

use std::rc::Rc;

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::source_scan::{TriviaProfile, is_regex_start, skip_regex_literal, skip_trivia};
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
///
/// Strings and line/block comments are skipped by the shared trivia cursor
/// (`skip_trivia`, JS profile) — so escape handling is correct in exactly one
/// place. Regex literals are the one thing the cursor deliberately leaves
/// significant (disambiguating `/` needs previous-token lookback); this matcher
/// handles them via the shared `source_scan::is_regex_start` /
/// `skip_regex_literal` helpers (the same ones the TS arrow-vs-paren lookahead
/// uses).
pub(crate) fn scan_to_matching_brace(bytes: &[u8], scan_start: usize) -> Option<usize> {
    let end = bytes.len();
    let mut brace_depth: u32 = 1; // the opening `{` before scan_start

    let mut i = scan_start;
    while i < end {
        // Strings / line / block comments: a brace inside them isn't significant.
        if let Some(past) = skip_trivia(bytes, i, end, TriviaProfile::JS) {
            i = past;
            continue;
        }

        // A `/` the cursor left significant is division or a regex literal. The
        // `i + 1 < end` guard mirrors the historical scanner (a trailing `/` is
        // never a regex start). `is_regex_start` only fires here because
        // `skip_trivia` already consumed `//` and `/*`.
        if bytes[i] == b'/' && i + 1 < end && is_regex_start(bytes, i, scan_start) {
            i = skip_regex_literal(bytes, i, end);
            continue;
        }

        match bytes[i] {
            b'{' => brace_depth += 1,
            b'}' => {
                brace_depth -= 1;
                if brace_depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }

        i += 1;
    }

    None
}
