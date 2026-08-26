// Expression tag parsing

use crate::ast::internal::*;
use crate::lexer::{BlockOrTagMarker, TokenKind};
use tsv_lang::{ParseError, Span};

use super::parser_impl::SvelteParser;

/// The two contexts Svelte's `read_sequence` runs in — a run of text and `{expr}` chunks,
/// where a `{#…}` block or `{@…}` tag is invalid. The strings are Svelte's own `location`
/// argument to `block_invalid_placement` / `tag_invalid_placement`, so the messages
/// [`SvelteParser::check_sequence_placement`] builds read exactly as the canonical parser's.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SequenceLocation {
    /// `<textarea>` RCDATA content — Svelte's sole RCDATA element.
    InsideTextarea,
    /// An attribute value, quoted or not, a directive's and a style directive's included.
    AttributeValue,
}

impl SequenceLocation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::InsideTextarea => "inside <textarea>",
            Self::AttributeValue => "in attribute value",
        }
    }
}

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

    /// Parse an expression tag inside a **sequence** — a run of text and `{expr}` chunks —
    /// rejecting a `{#…}` block or `{@…}` tag first, as Svelte's `read_sequence` does.
    ///
    /// This is the entry point every sequence reader takes, rather than
    /// [`Self::parse_expression_tag_at`] plus a guard the next reader can forget: tsv reaches
    /// by five routes what Svelte reaches through one `read_sequence`, and a guard missing
    /// from one of them is the whole bug. The two routes this cannot serve — the directive
    /// arms, which take their `{…}` off the token stream — ask
    /// [`Self::check_sequence_placement`] directly.
    pub(crate) fn parse_sequence_expression_tag_at(
        &mut self,
        brace_pos: usize,
        location: SequenceLocation,
    ) -> Result<ExpressionTag<'arena>, ParseError> {
        self.check_sequence_placement(brace_pos, location)?;
        self.parse_expression_tag_at(brace_pos)
    }

    /// Reject a `{#…}` block or `{@…}` tag written where only a text/`{expr}` sequence is
    /// allowed — Svelte's `read_sequence` guard (`1-parse/state/element.js`), which runs
    /// *before* the expression is read.
    ///
    /// Without it the brace contents reach the TypeScript expression parser, which answers a
    /// question nobody asked: `{@debug e}` becomes a decorator (`Expected 'class' after
    /// 'decorator'`) and `{#x in y}` is the one production where a private name is an operand
    /// (the ergonomic brand check), so it *parses* — an over-acceptance in every sequence
    /// context, attribute values included.
    ///
    /// The marker need not be glued to the `{`: [`BlockOrTagMarker::in_sequence_at`] skips the
    /// gap and owns the question of why tsv is wider here than `read_sequence` is. The error is
    /// still reported **at the brace**, matching Svelte's own `block_invalid_placement` index.
    pub(crate) fn check_sequence_placement(
        &self,
        brace_pos: usize,
        location: SequenceLocation,
    ) -> Result<(), ParseError> {
        let Some((marker, marker_pos)) = BlockOrTagMarker::in_sequence_at(self.source, brace_pos)
        else {
            return Ok(());
        };
        let bytes = self.source.as_bytes();
        // Svelte names the construct with `read_until(/[^a-z]/)` — lowercase ASCII only, so
        // `{@html expr}` names `html` and `{#}` names nothing.
        // `name_start <= bytes.len()`: the marker byte at `marker_pos` was found, so the slice
        // below is in range even when the document ends right after it (`{#`).
        let name_start = marker_pos + 1;
        let rest = &bytes[name_start..];
        let name_len = rest
            .iter()
            .position(|b| !b.is_ascii_lowercase())
            .unwrap_or(rest.len());
        let name = &self.source[name_start..name_start + name_len];
        let sigil = marker.sigil();
        let construct = marker.construct();
        let location = location.as_str();
        Err(self.error_msg_at(
            &format!("{{{sigil}{name} ...}} {construct} cannot be {location}"),
            brace_pos,
        ))
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

        // Parse expression using TypeScript parser — the shared helper, which
        // collects the comments into `Root.comments` and records the acorn
        // region the wire writer seeds this island's `loc` from.
        let expression = self.parse_ts_expression(expr_content, expr_start)?;

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
