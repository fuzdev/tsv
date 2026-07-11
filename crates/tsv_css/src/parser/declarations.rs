use super::CssParser;
use crate::ast::internal::*;
use crate::lexer::{Lexer, TokenKind};
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
            // Peek past whitespace and comments to find the significant next token
            let next_kind = parser.peek_past_whitespace()?;
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
/// Approach: create a temporary lexer from after the identifier and scan forward,
/// skipping parenthesized groups, until we find `{` (nested rule) or `;`/`}` (declaration).
fn is_type_selector_with_pseudo(parser: &CssParser<'_, '_>) -> Result<bool, ParseError> {
    let remaining = &parser.source()[parser.current_end..];
    let mut temp = Lexer::new(remaining);
    // `u32` so an unbalanced close saturates at 0 (never negative) — a signed depth
    // would floor `saturating_sub` at `i32::MIN` and disable the `== 0` terminator
    // checks below. Matches every other depth counter in the crate.
    let mut paren_depth: u32 = 0;
    loop {
        let tok = temp.next_token()?;
        match &tok.kind {
            TokenKind::LeftParen => paren_depth += 1,
            TokenKind::RightParen => paren_depth = paren_depth.saturating_sub(1),
            TokenKind::LeftBrace if paren_depth == 0 => return Ok(true),
            TokenKind::Semicolon | TokenKind::RightBrace if paren_depth == 0 => return Ok(false),
            TokenKind::Eof => return Ok(false),
            _ => {}
        }
    }
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
    let property_ident = parser.current_identifier();
    let property = parser.alloc_str_in(property_ident);
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

    // Track value start and end for span calculation
    let value_start = parser.base_offset() + parser.current_start;

    // Parse value (collect tokens until ; or })
    // IMPORTANT: Track bracket depth so balanced groups don't terminate the value:
    // - parens for functions like `url(data:image/png;base64,...)`
    // - braces and brackets for custom properties (`<declaration-value>` per CSS Syntax
    //   Level 3 §4.3.7 permits balanced `()` / `[]` / `{}` blocks)
    let mut has_value_comment = false;
    let mut value_end = value_start;
    // The value text is re-extracted verbatim from source below (`parse_value_from_source`),
    // so the token scan never materializes the parts — it only needs (a) whether any part
    // exists (emptiness) and (b) enough about the last two parts to strip a trailing
    // `!important`. Track a rolling two-part window instead of a `Vec<String>`:
    //   - `*_is_bang` / `*_is_important`: is the part the `!` marker / the `important` keyword
    //   - `*_ends`: (value_end before this part, this part's token end) — for the span rollback
    // Comments are not parts but do advance `value_end`, so the window pair correctly rolls
    // back past a comment that sits next to `!important`.
    let mut part_count: usize = 0;
    let mut last_is_bang = false;
    let mut last_is_important = false;
    let mut last_ends: (usize, usize) = (0, 0);
    let mut prev_is_bang = false;
    let mut prev_ends: (usize, usize) = (0, 0);
    // `u32` (not `i32`): an *unbalanced* close — e.g. the stray `)` the lexer leaves after
    // truncating an unquoted `url()` with nested parens like `url(a(b))` at the first `)` —
    // must saturate at depth 0 so a following `;`/`}` still terminates the value. A signed
    // depth floors `saturating_sub` at `i32::MIN`, driving depth negative and disabling the
    // `== 0` terminator (it then swallows the `;`, the `}`, and on to EOF). Matches the other
    // depth counters in the crate (`is_type_selector_with_pseudo`, `atrules/raw`, `value/*`).
    let mut paren_depth: u32 = 0;
    let mut brace_depth: u32 = 0;
    let mut bracket_depth: u32 = 0;
    while !parser.check(TokenKind::Eof)
        && !(paren_depth == 0
            && brace_depth == 0
            && bracket_depth == 0
            && (parser.check(TokenKind::Semicolon) || parser.check(TokenKind::RightBrace)))
    {
        // Track balanced-group depth (each close saturates at 0 — see the `u32` note at
        // the counters' declaration). An outer `}` at depth 0 would have terminated the
        // loop above, so reaching the RightBrace arm here means we're inside a value-level
        // block (custom-property block values).
        match &parser.current_kind {
            TokenKind::LeftParen => paren_depth += 1,
            TokenKind::RightParen => paren_depth = paren_depth.saturating_sub(1),
            TokenKind::LeftBrace => brace_depth += 1,
            TokenKind::RightBrace => brace_depth = brace_depth.saturating_sub(1),
            TokenKind::LeftBracket => bracket_depth += 1,
            TokenKind::RightBracket => bracket_depth = bracket_depth.saturating_sub(1),
            _ => {}
        }

        // Classify the token only for `!important` stripping (the value text itself is
        // re-extracted from source below, never reconstructed here).
        let (is_bang, is_important) = match &parser.current_kind {
            // An identifier can't be `!`; it can be `important` (case-insensitive).
            TokenKind::Identifier => (
                false,
                parser
                    .current_identifier()
                    .eq_ignore_ascii_case("important"),
            ),
            // A quoted string / number / percentage / dimension is never `!` or `important`.
            TokenKind::String { .. }
            | TokenKind::Number
            | TokenKind::Percentage
            | TokenKind::Dimension { .. } => (false, false),
            TokenKind::Whitespace => {
                parser.advance()?;
                continue;
            }
            TokenKind::Comment => {
                // Track that we have a comment (for allowing comment-only values)
                has_value_comment = true;
                // Update value_end to include the comment in the declaration span
                value_end = parser.base_offset() + parser.current_end;
                parser.advance()?;
                continue;
            }
            TokenKind::Bang => (true, false),
            _ => {
                // Other tokens (brackets/braces/parens/etc.) - classify from source.
                let text = parser.current_value();
                (text == "!", text.eq_ignore_ascii_case("important"))
            }
        };

        let token_end = parser.base_offset() + parser.current_end;
        // Roll the two-part window (previous <- last <- this).
        prev_is_bang = last_is_bang;
        prev_ends = last_ends;
        last_is_bang = is_bang;
        last_is_important = is_important;
        last_ends = (value_end, token_end);
        part_count += 1;
        value_end = token_end;
        parser.advance()?;
    }

    // Check for !important at the end of value: the second-to-last part is `!` and the
    // last is `important` (case-insensitive).
    let important_matched = part_count >= 2 && prev_is_bang && last_is_important;
    let important_end = if important_matched {
        // End of the `important` token itself (a trailing comment may have advanced
        // value_end past it); roll the value span back to just before the `!` was
        // scanned, which keeps any comments between the value and the `!`.
        let end_with_important = last_ends.1;
        value_end = prev_ends.0;
        Some(end_with_important as u32)
    } else {
        None
    };

    // The value is empty when no parts remain after the optional `!important` strip.
    // (Every part is non-empty, so the old "join all parts and check is_empty" — whose
    // joined string was used for nothing else, since the value is parsed from source —
    // reduces to this count check.)
    let value_is_empty = if important_matched {
        part_count - 2 == 0
    } else {
        part_count == 0
    };

    // Allow empty value if we have comments (e.g., `color: /* comment */;`)
    // Svelte treats the comment as the value in this case.
    // Also allow an empty custom-property value (`--a:;`): css-variables-1 makes the
    // value optional (`<declaration-value>?`), and css-syntax-3 trims leading/trailing
    // whitespace, so the value is empty regardless of spacing. The empty value parses to
    // an empty identifier and prints as a single space (`--a: ;`), the form
    // css-variables-1 mandates for serialization. Non-custom empty values stay an error.
    if value_is_empty && !has_value_comment && !property.starts_with("--") {
        return Err(parser.error_msg_at("Empty CSS value", start));
    }

    // Create span for the value (from first token to last token, excluding comments)
    let value_span = Span {
        start: value_start as u32,
        end: value_end as u32,
    };

    // Parse the value directly from source for accurate span tracking
    // Use parse_value_from_source instead of parse_value_string to avoid
    // span drift from whitespace differences between source and reconstructed tokens
    //
    // Convert absolute span to source-relative span (subtract base_offset)
    let base = parser.base_offset() as u32;
    let source_relative_span = Span {
        start: value_span.start - base,
        end: value_span.end - base,
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

    // Declaration ends after the value, NOT including the semicolon
    let end = value_end;

    // Optionally consume semicolon (but don't include it in the declaration span)
    if parser.check(TokenKind::Semicolon) {
        parser.advance()?;
    }

    let decl_span = Span {
        start: start as u32,
        end: end as u32,
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
