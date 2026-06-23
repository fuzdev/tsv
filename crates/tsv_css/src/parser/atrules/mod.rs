// CSS at-rule parsing
//
// Dispatch spine for `@`-prefixed at-rules: parse the name, route the prelude to a
// structured parser (`preludes`) or the verbatim/normalized raw parser (`raw`), then
// parse the optional block. Also holds the shared at-rule predicates.

mod preludes;
mod raw;

pub(super) use super::CssParser;

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use self::preludes::{
    parse_container_prelude, parse_import_prelude, parse_scope_prelude, parse_supports_prelude,
};
use self::raw::parse_raw_prelude_content;

/// Check if an at-rule name is a keyframes rule (including vendor-prefixed versions)
/// e.g., "keyframes", "-webkit-keyframes", "-moz-keyframes"
fn is_keyframes_atrule(name: &str) -> bool {
    name == "keyframes"
        || name == "-webkit-keyframes"
        || name == "-moz-keyframes"
        || name == "-o-keyframes"
        || name == "-ms-keyframes"
}

/// Check if current token is a CSS boolean operator keyword (and, or, not)
pub(super) fn is_boolean_operator(parser: &CssParser<'_>) -> bool {
    if let TokenKind::Identifier = &parser.current_kind {
        let identifier = parser
            .current_identifier()
            .unwrap_or_else(|| parser.current_value());
        matches!(identifier, "and" | "or" | "not")
    } else {
        false
    }
}

/// Parse a CSS at-rule: `@media (...) { ... }` or `@import "...";`
///
/// `nested_in_rule`: true if this at-rule is nested inside a regular rule's declaration block
pub(crate) fn parse_atrule(
    parser: &mut CssParser<'_>,
    nested_in_rule: bool,
) -> Result<CssAtrule, ParseError> {
    let start = parser.base_offset() + parser.current_start;

    // Expect @ symbol
    parser.expect(TokenKind::AtSign)?;

    // Parse at-rule name (identifier after @)
    if !parser.check(TokenKind::Identifier) {
        return Err(parser.error_expected_after("at-rule name", "@"));
    }

    // Internal AST: use decoded value (spec-compliant)
    let name = parser
        .current_identifier()
        .unwrap_or_else(|| parser.current_value())
        .to_string();
    parser.advance()?;

    parser.skip_whitespace()?;

    // Parse prelude based on at-rule type
    let prelude = if name == "import" {
        // Parse @import prelude structurally (url/string + layer/supports/media)
        let (values, span) = parse_import_prelude(parser)?;
        PreludeValue::Values { values, span }
    } else if name == "scope" {
        // Parse @scope prelude as structured selector lists
        let (root, limit, span) = parse_scope_prelude(parser)?;
        PreludeValue::Selectors { root, limit, span }
    } else if name == "supports" {
        // Parse @supports prelude as structured conditions (for line-width wrapping)
        let (condition, span) = parse_supports_prelude(parser)?;
        PreludeValue::Supports { condition, span }
    } else if name == "container" {
        // Parse @container prelude as structured conditions (for line-width wrapping)
        let (name, condition, span) = parse_container_prelude(parser)?;
        PreludeValue::Container {
            name,
            condition,
            span,
        }
    } else if name == "media" {
        // Parse @media as raw string to preserve comments
        // Wrapping is handled in the printer by finding and/or boundaries
        // Fully structuring preludes is a deferred design option — see
        // docs/architecture.md § "Red-Green Trees (Deferred)"
        let (content, span) = parse_raw_prelude_content(parser, false, true)?;
        PreludeValue::Media { content, span }
    } else {
        // Parse as raw string for other at-rules (@keyframes, @layer, @page, etc.).
        // Most have no `property: value` / media-query grammar, so prettier keeps the
        // prelude verbatim (outer-trimmed; only `url()` inner whitespace is trimmed) —
        // preserve internal whitespace (`@layer a , b` must not become `a, b`).
        // `@namespace` is the exception: prettier re-parses its prelude as a value (see
        // postcss `parser-postcss.js`), normalizing whitespace to single spaces, so it
        // takes the normalizing path. The public AST stays source-verbatim either way
        // (the printer-facing `content` is what differs); see `convert.rs`.
        let normalize_whitespace = name == "namespace";
        let (content, span) = parse_raw_prelude_content(parser, false, normalize_whitespace)?;
        PreludeValue::Raw { content, span }
    };

    // Parse block (if present)
    let (block, end) = if parser.check(TokenKind::LeftBrace) {
        let block = parse_atrule_block(parser, &name, nested_in_rule)?;
        let end = block.span.end;
        (Some(block), end)
    } else if parser.check(TokenKind::Semicolon) {
        // Statement at-rule (no block)
        let end = parser.base_offset() + parser.current_end;
        parser.advance()?;
        return Ok(CssAtrule {
            name,
            prelude,
            block: None,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        });
    } else {
        return Err(parser.error_expected_after("'{' or ';'", "at-rule prelude"));
    };

    Ok(CssAtrule {
        name,
        prelude,
        block,
        span: Span {
            start: start as u32,
            end,
        },
    })
}

/// Parse an at-rule block: `{ ... }`
/// Block contents depend on at-rule type and nesting context:
/// - @media, @supports, @layer: contain rules (top-level) or declarations (nested)
/// - @keyframes: contains keyframe blocks (rules with percentage/from/to selectors)
/// - @font-face, @page: contain declarations
///
/// `nested_in_rule`: true if this at-rule is nested inside a regular rule's declaration block
fn parse_atrule_block(
    parser: &mut CssParser<'_>,
    atrule_name: &str,
    nested_in_rule: bool,
) -> Result<CssAtruleBlock, ParseError> {
    let start = parser.base_offset() + parser.current_start;

    // Expect {
    parser.expect(TokenKind::LeftBrace)?;
    parser.skip_whitespace()?;

    let mut children = Vec::new();

    // Determine what content to expect based on at-rule type and nesting context
    // When nested inside a rule, at-rules that normally contain rules should contain declarations instead
    let expect_rules = (matches!(
        atrule_name,
        "media"
            | "supports"
            | "layer"
            | "container"
            | "starting-style"
            | "scope"
            | "font-feature-values"
    ) || is_keyframes_atrule(atrule_name))
        && !nested_in_rule;
    let expect_declarations = matches!(
        atrule_name,
        "font-face" | "page" | "property" | "counter-style" | "color-profile" | "position-try" | "font-palette-values"
        // @font-feature-values nested at-rules (all contain declarations)
        | "stylistic" | "styleset" | "character-variant" | "swash" | "ornaments" | "annotation"
        // Page margin boxes (nested within @page, all contain declarations)
        | "top-left-corner" | "top-left" | "top-center" | "top-right" | "top-right-corner"
        | "left-top" | "left-middle" | "left-bottom"
        | "right-top" | "right-middle" | "right-bottom"
        | "bottom-left-corner" | "bottom-left" | "bottom-center" | "bottom-right" | "bottom-right-corner"
    );
    // Conditional group at-rules (@media, @supports, etc.) nested inside a rule
    // can contain BOTH declarations and nested rules — they fall through to the
    // generic fallback which uses is_nested_rule_start() to disambiguate.

    while !parser.check(TokenKind::RightBrace) && !parser.check(TokenKind::Eof) {
        if matches!(&parser.current_kind, TokenKind::Comment) {
            let comment = parser.parse_block_comment()?;
            children.push(CssBlockChild::Comment(comment));
            continue;
        }

        // Handle nested at-rules
        if parser.check(TokenKind::AtSign) {
            // Propagate the nesting context: an at-rule nested inside a conditional
            // group at-rule that is itself inside a style rule is still in a nesting
            // context, so its block holds declarations (per CSS Nesting). Outside a
            // rule, `nested_in_rule` is false and this stays a normal at-rule block.
            let atrule = parse_atrule(parser, nested_in_rule)?;
            children.push(CssBlockChild::Atrule(atrule));
            parser.skip_whitespace()?;
            continue;
        }

        // For @font-face and @page, parse declarations
        if expect_declarations && parser.check(TokenKind::Identifier) {
            let decl = super::declarations::parse_declaration(parser)?;
            children.push(CssBlockChild::Declaration(decl));
            parser.skip_whitespace()?;
            continue;
        }

        // For @media, @supports, @layer, @keyframes, parse rules
        if expect_rules {
            let rule = super::declarations::parse_rule(parser, false)?;
            children.push(CssBlockChild::Rule(rule));
            parser.skip_whitespace()?;
            continue;
        }

        // Generic fallback for unknown at-rules: try to detect whether this is a declaration or rule
        // by checking if the current position looks like a nested rule start
        let looks_like_rule = super::declarations::is_nested_rule_start(parser)?;
        if looks_like_rule {
            // Parse as rule (selector + block) — use nested=true to allow leading combinators
            let rule = super::declarations::parse_rule(parser, true)?;
            children.push(CssBlockChild::Rule(rule));
            parser.skip_whitespace()?;
            continue;
        } else if parser.check(TokenKind::Identifier) {
            // Parse as declaration (property: value)
            let decl = super::declarations::parse_declaration(parser)?;
            children.push(CssBlockChild::Declaration(decl));
            parser.skip_whitespace()?;
            continue;
        }

        // Fallback: unexpected token
        return Err(parser.error_unexpected(&format!("token in @{atrule_name} block")));
    }

    // Expect }
    if !parser.check(TokenKind::RightBrace) {
        return Err(parser.error_expected("'}'"));
    }
    let end = parser.base_offset() + parser.current_end;
    parser.advance()?; // consume }

    Ok(CssAtruleBlock {
        children,
        span: Span {
            start: start as u32,
            end: end as u32,
        },
    })
}
