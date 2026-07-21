use super::CssParser;
use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

/// Check if we're looking at the start of a nested rule (selector) rather than a declaration.
///
/// Nested rules can start with:
/// - `&` (nesting selector)
/// - `.` (class selector)
/// - `#` (ID selector)
/// - `*` (universal selector)
/// - `:` (pseudo-class/element)
/// - `[` (attribute selector)
/// - `>`, `+`, `~` (leading combinator - CSS Nesting relative selectors)
/// - Identifier (type selector - but could also be a property name)
///
/// For identifiers, we need to look ahead: if next non-whitespace/comment token is `:`, it's a declaration.
/// Custom-property identifiers (`--*`) are always declarations and bypass the lookahead.
pub(crate) fn is_nested_rule_start(parser: &CssParser<'_, '_>) -> Result<bool, ParseError> {
    match &parser.current_kind {
        // Unambiguous selector start tokens
        TokenKind::Ampersand
        | TokenKind::Dot
        | TokenKind::Hash
        | TokenKind::Asterisk
        | TokenKind::Colon
        | TokenKind::LeftBracket
        // Keyframe selector stop (`0%`, `50%`): a `<percentage>`-led block child is
        // always a keyframe rule, never a declaration (a property name is an
        // identifier or `--custom`), so it starts a rule wherever this is consulted.
        | TokenKind::Percentage
        // Leading combinators (CSS Nesting relative selectors: `> .child {}`, `+ .sibling {}`)
        | TokenKind::GreaterThan
        | TokenKind::Plus
        | TokenKind::Tilde => Ok(true),

        // Ambiguous: identifier could be type selector (nested rule) or property name (declaration)
        // Look ahead to check if next non-whitespace/comment token is `:` (declaration) or not (nested rule)
        TokenKind::Identifier => {
            // CSS Custom Properties (`--foo`) are always declarations. CSS Variables
            // Module Level 1 §2.1 defines their value as `<declaration-value>`, which
            // permits any token sequence with balanced `()` / `[]` / `{}` — including
            // a top-level `{...}` block. Without this short-circuit, `--foo: { ... }`
            // would misclassify as a type-selector + pseudo-class.
            if parser.current_identifier().starts_with("--") {
                return Ok(false);
            }
            // Peek past whitespace and comments to find the significant next token. A `:`
            // is settled from the bytes — a property name is followed by one, so this is
            // the path nearly every block child takes.
            let next_kind = super::decl_scan::peek_significant_kind(parser)?;
            match next_kind {
                // Colon after identifier - ambiguous: could be declaration (`color: red`)
                // or nested rule with pseudo-class (`span:hover { }`)
                // Need deeper lookahead to disambiguate
                TokenKind::Colon => is_type_selector_with_pseudo(parser),
                // Left brace after identifier = nested rule (e.g., "div {")
                TokenKind::LeftBrace => Ok(true),
                // Selector tokens after identifier = nested rule
                TokenKind::Dot | TokenKind::Hash | TokenKind::LeftBracket => Ok(true),
                // Other tokens - likely nested rule
                _ => Ok(true),
            }
        }

        _ => Ok(false),
    }
}

/// Disambiguate `Identifier` + `Colon` between declaration and nested rule.
///
/// Examples:
/// - `color: red;` or `color:red;` → declaration (ends with `;`)
/// - `span:hover { }` → nested rule (ends with `{`)
/// - `filter:blur(5px);` → declaration (function value, ends with `;`)
/// - `span:not(:last-child)::after { }` → nested rule (ends with `{`)
///
/// Scans forward from after the identifier, skipping parenthesized groups, until it finds
/// `{` (nested rule) or `;`/`}` (declaration). For a declaration that means walking the whole
/// value — so `decl_scan` walks it as bytes (keeping an equivalent token walk as its fallback
/// and its debug-time oracle) and, in that one pass, also collects the value facts, which it
/// stashes on the parser for `parse_declaration` to reuse rather than re-scan.
fn is_type_selector_with_pseudo(parser: &CssParser<'_, '_>) -> Result<bool, ParseError> {
    super::decl_scan::scan_rule_or_declaration(parser, parser.current_end)
}

/// Parse a CSS rule: `selector { property: value; }`
///
/// When `nested` is true, the selector list allows leading combinators (CSS Nesting relative selectors).
/// For example: `> .child {}`, `+ .sibling {}`, `~ .general {}`
pub(crate) fn parse_rule<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    nested: bool,
) -> Result<CssRule<'arena>, ParseError> {
    let start = parser.span_pos(parser.current_start);

    // Nested rules use relative selectors (can start with combinators like `> .child`)
    // Top-level rules use complex selectors (cannot start with combinators)
    let selector = if nested {
        super::selectors::parse_relative_selector_list(parser)?
    } else {
        super::selectors::parse_complex_selector_list(parser)?
    };

    // Capture any comments after selector (before {)
    let mut declarations = parser.bvec();
    parser.skip_whitespace()?;
    while matches!(&parser.current_kind, TokenKind::Comment) {
        let comment = parser.parse_block_comment()?;
        declarations.push(CssBlockChild::Comment(comment));
        parser.skip_whitespace()?;
    }

    // Expect { and capture its start
    let block_start = parser.span_pos(parser.current_start);
    parser.expect(TokenKind::LeftBrace)?;
    parser.skip_whitespace()?;

    // Parse declarations, comments, and nested rules
    while !parser.check(TokenKind::RightBrace) && !parser.check(TokenKind::Eof) {
        // `read_block` boundary: discard legacy `<!-- ... -->` markers between items
        // (matches parseCss's `allow_comment_or_whitespace`); a marker may sit right
        // before the closing `}`, so re-check the terminators after skipping.
        parser.skip_html_comment_markers()?;
        if parser.check(TokenKind::RightBrace) || parser.check(TokenKind::Eof) {
            break;
        }

        if matches!(&parser.current_kind, TokenKind::Comment) {
            let comment = parser.parse_block_comment()?;
            declarations.push(CssBlockChild::Comment(comment));
            continue;
        }

        // Check for nested at-rule (CSS Nesting Module)
        if parser.check(TokenKind::AtSign) {
            // Parse nested at-rule (e.g., @media inside a rule)
            // Pass true for nested_in_rule since we're inside a regular rule's declaration block
            let nested_atrule = super::atrules::parse_atrule(parser, true)?;
            declarations.push(CssBlockChild::Atrule(nested_atrule));
            parser.skip_whitespace()?;
            continue;
        }

        // Check if we're looking at a nested rule (CSS Nesting Module)
        if is_nested_rule_start(parser)? {
            // Parse nested rule recursively (nested rules allow leading combinators)
            let nested_rule = parse_rule(parser, true)?;
            declarations.push(CssBlockChild::Rule(nested_rule));
            parser.skip_whitespace()?;
            continue;
        }

        // Otherwise, parse as declaration
        if parser.check(TokenKind::Identifier) {
            let decl = parse_declaration(parser)?;
            declarations.push(CssBlockChild::Declaration(decl));
        } else {
            // Skip unexpected tokens
            parser.advance()?;
        }
        parser.skip_whitespace()?;
    }

    // Expect } and capture its end position
    if !parser.check(TokenKind::RightBrace) {
        return Err(parser.error_expected("'}'"));
    }
    let block_end = parser.span_pos(parser.current_end);
    parser.advance()?; // consume }

    Ok(CssRule {
        selector,
        block_span: Span {
            start: block_start,
            end: block_end,
        },
        declarations: declarations.into_bump_slice(),
        span: Span {
            start,
            end: block_end,
        },
    })
}

/// Parse a CSS declaration: `property: value;`
pub(crate) fn parse_declaration<'arena>(
    parser: &mut CssParser<'_, 'arena>,
) -> Result<CssDeclaration<'arena>, ParseError> {
    let start = parser.base_offset() + parser.current_start;

    // Parse property
    if !parser.check(TokenKind::Identifier) {
        return Err(parser.error_expected_at("property name", start));
    }
    // Internal AST: use decoded value (spec-compliant)
    // Svelte quirk (raw value) will be applied in conversion layer
    let property = parser.current_identifier_in_arena();
    parser.advance()?;

    let property_gap_comment = parser.skip_whitespace_and_comments()?;

    // Record the real `property : value` colon offset (host coordinates, like the
    // declaration span) so the writer splits property/value without a re-scan. The
    // parser sits on the colon here — whitespace and comments already skipped — and
    // `expect` below guarantees it is one.
    let colon_offset = (parser.base_offset() + parser.current_start) as u32;
    // Expect :
    parser.expect(TokenKind::Colon)?;
    // Only skip whitespace, NOT comments - comments in values need to be preserved
    parser.skip_whitespace()?;

    // Scan the value to its terminator (`;` / `}` at depth zero, or EOF), collecting the
    // few facts the declaration node needs: where its span ends, whether it holds a
    // comment, whether it is empty, and where a trailing `!important` sits. The scan walks
    // bytes rather than tokens — a value's text is re-parsed from source below
    // (`parse_value_from_source`) anyway, so tokenizing it here only to find a `;` was
    // paying the lexer twice for the same bytes. See `decl_scan`.
    //
    // For a non-custom declaration the rule/declaration disambiguation already walked this
    // value (to decide it was a declaration, not a nested rule) and stashed these facts, so
    // reuse them rather than walk a second time. The stash is absent for a custom property
    // (which bypasses the disambiguation) or when the byte scan declined; then scan now.
    let value_start_raw = parser.current_start;
    let facts = match parser.take_value_facts(value_start_raw) {
        Some(facts) => {
            #[cfg(debug_assertions)]
            {
                // The reused facts must equal a fresh scan at the parser's own value start —
                // proving the disambiguation located the value identically to the parser's
                // positioning here.
                let fresh = super::decl_scan::scan_value(parser, value_start_raw);
                debug_assert!(
                    fresh.as_ref().is_ok_and(|fresh| *fresh == facts),
                    "reused value facts disagreed with a fresh scan at {value_start_raw}: \
                     reused {facts:?}, fresh {fresh:?}"
                );
            }
            facts
        }
        None => super::decl_scan::scan_value(parser, value_start_raw)?,
    };

    // Land on the terminator the scan found — the lexer never tokenized the value, and it
    // does not tokenize the terminator either: the scan stopped *on* it, so it knows which
    // token it is.
    parser.seat_at_terminator(facts.terminator, facts.terminator_kind);

    let has_value_comment = facts.has_comment;
    let important_end = facts.important_end.map(|end| parser.span_pos(end));

    // Allow empty value if we have comments (e.g., `color: /* comment */;`)
    // Svelte treats the comment as the value in this case.
    // Also allow an empty custom-property value (`--a:;`): css-variables-1 makes the
    // value optional (`<declaration-value>?`), and css-syntax-3 trims leading/trailing
    // whitespace, so the value is empty regardless of spacing. The empty value parses to
    // an empty identifier and prints as a single space (`--a: ;`), the form
    // css-variables-1 mandates for serialization. Non-custom empty values stay an error.
    if facts.is_empty && !has_value_comment && !property.starts_with("--") {
        return Err(parser.error_msg_at("Empty CSS value", start));
    }

    // Create span for the value (from first token to last token, excluding comments)
    let value_span = Span {
        start: parser.span_pos(value_start_raw),
        end: parser.span_pos(facts.value_end),
    };

    // Parse the value directly from source for accurate span tracking
    // Use parse_value_from_source instead of parse_value_string to avoid
    // span drift from whitespace differences between source and reconstructed tokens
    //
    // The scan's offsets are already source-relative; the span above is the
    // `base_offset`-shifted (host) form of the same range.
    let base = parser.base_offset() as u32;
    let source_relative_span = Span {
        start: value_start_raw as u32,
        end: facts.value_end as u32,
    };

    // Custom properties (--*) with unusual values (e.g., leading comma) preserve raw value
    // Normal custom property values are still parsed for proper formatting
    let raw_value = source_relative_span.extract(parser.source());
    let trimmed_value = raw_value.trim();

    let value = if property.starts_with("--") && trimmed_value.starts_with(',') {
        // Leading comma is unusual syntax - preserve as raw identifier
        // (text recovered verbatim from `span` at print time)
        CssValue::Identifier { span: value_span }
    } else {
        super::value::parse_value_from_source(
            parser.source(),
            source_relative_span,
            base,
            parser.arena,
        )
    };

    // Optionally consume semicolon (but don't include it in the declaration span)
    if parser.check(TokenKind::Semicolon) {
        parser.advance()?;
    }

    // Declaration ends after the value, NOT including the semicolon
    let decl_span = Span {
        start: start as u32,
        end: value_span.end,
    };

    // A block comment anywhere in the declaration extent (property→colon gap or the
    // value/`!important`/trailing region, tracked by `has_value_comment`) routes the
    // writer to the comment-aware split/strip; false takes its zero-scan fast path.
    let has_block_comment = property_gap_comment || has_value_comment;

    // Span covers the entire declaration (property + value, not including semicolon)
    // The source value will be extracted on-demand during conversion using this span
    Ok(CssDeclaration {
        property,
        value,
        important_end,
        span: decl_span,
        colon_offset,
        has_block_comment,
    })
}
