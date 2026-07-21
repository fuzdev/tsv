// Expression tag parsing

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::parser_impl::SvelteParser;

impl<'a, 'arena> SvelteParser<'a, 'arena> {
    /// Parse an expression tag `{expression}` at the current lexer position, then
    /// advance the lexer past the closing `}`.
    ///
    /// Used by callers that drive the token stream (template `{expr}` tags,
    /// directive values). Position-based callers that own their own cursor — the
    /// attribute-value sequence readers — use `parse_expression_tag_at`, which runs
    /// the same scan + parse without touching the lexer.
    pub(crate) fn parse_expression_tag(&mut self) -> Result<ExpressionTag<'arena>, ParseError> {
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
    ) -> Result<ExpressionTag<'arena>, ParseError> {
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
            &mut self.interner,
            self.arena,
        )?;

        // Add expression comments to the parser's collection for later inclusion in Root.comments
        self.expression_comments.extend_from_slice(comments);

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
/// `scan_start`, skipping nested braces, strings, line/block comments, regex
/// literals, and (interpolation-aware) template literals. `scan_start` is the
/// first byte to scan (the opening `{` is counted as depth 1). Returns the byte
/// offset of the matching `}`, or `None` if the braces never balance.
///
/// The single robust brace matcher shared by every `{…}` construct — expression
/// tags, `{@…}` tags, `{...spread}`, and block tags — so none reimplements it (and
/// weaker copies can't desync on a `}` inside a regex/comment/string/template).
///
/// A thin wrapper over `tsv_lang::source_scan::scan_to_matching_brace` (the shared
/// expression-context balanced-brace scanner, which the `${…}` template-interpolation
/// skip also uses) with `end = bytes.len()`.
#[inline]
pub(crate) fn scan_to_matching_brace(bytes: &[u8], scan_start: usize) -> Option<usize> {
    tsv_lang::source_scan::scan_to_matching_brace(bytes, scan_start, bytes.len())
}
