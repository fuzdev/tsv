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
    parse_condition_query, parse_container_prelude, parse_import_prelude, parse_scope_prelude,
};
use self::raw::parse_raw_prelude_content;

/// Check if an at-rule name is a keyframes rule (including vendor-prefixed versions)
/// e.g., "keyframes", "-webkit-keyframes", "-moz-keyframes"
pub(crate) fn is_keyframes_atrule(name: &str) -> bool {
    // At-rule names are ASCII case-insensitive (CSS Syntax 3), so `@KEYFRAMES` and
    // `@-WEBKIT-Keyframes` are keyframes too. Callers pass either the raw name (the
    // helper folds case) or an already-lowercased dispatch name.
    name.eq_ignore_ascii_case("keyframes")
        || name.eq_ignore_ascii_case("-webkit-keyframes")
        || name.eq_ignore_ascii_case("-moz-keyframes")
        || name.eq_ignore_ascii_case("-o-keyframes")
        || name.eq_ignore_ascii_case("-ms-keyframes")
}

/// Whether `ident` is a CSS boolean-operator keyword (`and`/`or`/`not`).
///
/// CSS grammar keywords are ASCII case-insensitive (CSS Syntax 3 §"tokenizing"),
/// so `AND`/`Or`/`NOT` are the same keyword as `and`/`or`/`not`. This drives
/// argument/connector *recognition* only; the printer preserves the keyword's
/// source case (matching prettier — see `connector_raw` and the boolean-operator
/// case note in `docs/conformance_prettier.md`).
pub(super) fn is_boolean_operator_keyword(ident: &str) -> bool {
    ident.eq_ignore_ascii_case("and")
        || ident.eq_ignore_ascii_case("or")
        || ident.eq_ignore_ascii_case("not")
}

/// Check if current token is a CSS boolean operator keyword (and, or, not)
pub(super) fn is_boolean_operator(parser: &CssParser<'_, '_>) -> bool {
    if let TokenKind::Identifier = &parser.current_kind {
        is_boolean_operator_keyword(parser.current_identifier())
    } else {
        false
    }
}

/// Parse a CSS at-rule: `@media (...) { ... }` or `@import "...";`
///
/// `nested_in_rule`: true if this at-rule is nested inside a regular rule's declaration block
pub(crate) fn parse_atrule<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    nested_in_rule: bool,
) -> Result<CssAtrule<'arena>, ParseError> {
    let start = parser.span_pos(parser.current_start);

    // Expect @ symbol
    parser.expect(TokenKind::AtSign)?;

    // Parse at-rule name (identifier after @)
    if !parser.check(TokenKind::Identifier) {
        return Err(parser.error_expected_after("at-rule name", "@"));
    }

    // Internal AST: use decoded value (spec-compliant)
    let name_ident = parser.current_identifier();
    let name = parser.alloc_str_in(name_ident);
    parser.advance()?;

    parser.skip_whitespace()?;

    // At-rule names are ASCII case-insensitive (CSS Syntax 3); dispatch on the
    // lowercased name so `@MEDIA`/`@Font-Face` route to the right prelude/block
    // grammar instead of the raw fall-through. The stored `name` keeps its source
    // case (the public AST matches Svelte, which preserves it — `@SUPPORTS` →
    // `"name": "SUPPORTS"`); the printer lowercases it for output.
    let name_lc_owned;
    let name_lc: &str = if name.bytes().any(|b| b.is_ascii_uppercase()) {
        name_lc_owned = name.to_ascii_lowercase();
        &name_lc_owned
    } else {
        name
    };

    // Raw source offset of the first prelude token — for the conditional-at-rule raw
    // fallback after the dispatch. Captured before any prelude parsing consumes tokens.
    let prelude_start_raw = parser.current_start;

    // Parse prelude based on at-rule type
    let prelude = if name_lc == "import" {
        // Parse @import prelude structurally (url/string + layer/supports/media). A
        // prelude that doesn't lead with a `<url>`/`<string>` isn't a structurable
        // @import; it falls back to a raw verbatim prelude internally (same CSS-Syntax
        // rationale as the `@supports`/`@container` fallback below).
        parse_import_prelude(parser)?
    } else if name_lc == "scope" {
        // Parse @scope prelude as structured selector lists
        let scope = parse_scope_prelude(parser)?;
        PreludeValue::Selectors {
            root: scope.root,
            limit: scope.limit,
            span: scope.span,
        }
    } else if name_lc == "supports" {
        // Parse @supports prelude as structured conditions (for line-width wrapping)
        let (condition, span) = parse_condition_query(parser)?;
        PreludeValue::Supports { condition, span }
    } else if name_lc == "container" {
        // Parse @container prelude as structured conditions (for line-width wrapping)
        let (name, condition, span) = parse_container_prelude(parser)?;
        PreludeValue::Container {
            name,
            condition,
            span,
        }
    } else if name_lc == "media" {
        // Parse @media as raw string to preserve comments
        // Wrapping is handled in the printer by finding and/or boundaries
        // Fully structuring preludes is a deferred design option — see
        // docs/architecture.md § "Red-Green Trees (Deferred)"
        let (content, span) = parse_raw_prelude_content(parser, false, true, false)?;
        PreludeValue::Media { content, span }
    } else {
        // Parse as raw string for other at-rules (@keyframes, @layer, @page, etc.).
        // Most have no `property: value` / media-query grammar, so prettier keeps the
        // prelude verbatim (outer-trimmed; only `url()` inner whitespace is trimmed) —
        // preserve internal whitespace (`@layer a , b` must not become `a, b`).
        // `@namespace` is the exception: prettier re-parses its prelude as a value (see
        // postcss `parser-postcss.js`), normalizing whitespace to single spaces AND string /
        // `url()` quotes to single quotes, so it takes the normalizing path. The public AST
        // stays source-verbatim either way (the printer-facing `content` is what differs);
        // see `convert/mod.rs`.
        let is_namespace = name_lc == "namespace";
        let (content, span) = parse_raw_prelude_content(parser, false, is_namespace, is_namespace)?;
        PreludeValue::Raw { content, span }
    };

    // Raw fallback for conditional at-rules (`@supports`/`@container`) whose prelude
    // isn't a structurable condition. Per CSS Syntax 3 an at-rule prelude is always
    // consumed as component values; a prelude that isn't a valid
    // `<supports-condition>`/`<container-condition>` makes the rule evaluate false — it
    // is NOT a parse error (parseCss stores it raw, prettier prints it verbatim).
    // `parse_condition_query` breaks off at the first token it can't fold in, leaving
    // those tokens unconsumed; re-emit the whole prelude verbatim instead of erroring at
    // the block boundary below. (`@import` does the equivalent fallback internally, since
    // its structured parse requires a leading `<url>`/`<string>` rather than trailing off.)
    let prelude = if matches!(name_lc, "supports" | "container") && !parser.at_prelude_end() {
        reconsume_prelude_as_raw(parser, prelude_start_raw)?
    } else {
        prelude
    };

    // Parse block (if present)
    let (block, end) = if parser.check(TokenKind::LeftBrace) {
        let block = parse_atrule_block(parser, name_lc, nested_in_rule)?;
        let end = block.span.end;
        (Some(block), end)
    } else if parser.check(TokenKind::Semicolon) {
        // Statement at-rule (no block)
        let end = parser.span_pos(parser.current_end);
        parser.advance()?;
        return Ok(CssAtrule {
            name,
            prelude,
            block: None,
            span: Span { start, end },
        });
    } else {
        return Err(parser.error_expected_after("'{' or ';'", "at-rule prelude"));
    };

    Ok(CssAtrule {
        name,
        prelude,
        block,
        span: Span { start, end },
    })
}

/// Re-emit an at-rule prelude that couldn't be structured (a non-condition
/// `@supports`/`@container`, or a non-`<url>`/`<string>` `@import`) as a raw verbatim
/// prelude.
///
/// Consumes the remaining prelude tokens up to the block boundary and rebuilds the
/// **whole** prelude — from `prelude_start_raw`, the raw (base-offset-free) source
/// offset of its first token — verbatim and outer-trimmed, as `PreludeValue::Raw`
/// (whose printer emits it as-is and whose convert extracts it from `span`). This
/// matches parseCss (which stores an unstructurable prelude raw) and prettier (which
/// prints it verbatim); see the call sites for the CSS-Syntax rationale.
pub(super) fn reconsume_prelude_as_raw<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    prelude_start_raw: usize,
) -> Result<PreludeValue<'arena>, ParseError> {
    while !parser.at_prelude_end() {
        parser.advance()?;
    }

    // Verbatim prelude text runs from its first token to the boundary token; outer
    // whitespace is trimmed so the public AST (from `span`) and printer `content` agree.
    let raw = &parser.source()[prelude_start_raw..parser.current_start];
    let content = raw.trim();
    let content_start_raw = prelude_start_raw + (raw.len() - raw.trim_start().len());
    let content_end_raw = content_start_raw + content.len();

    Ok(PreludeValue::Raw {
        content: parser.alloc_str_in(content),
        span: Span {
            start: parser.span_pos(content_start_raw),
            end: parser.span_pos(content_end_raw),
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
fn parse_atrule_block<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    atrule_name: &str,
    nested_in_rule: bool,
) -> Result<CssAtruleBlock<'arena>, ParseError> {
    let start = parser.span_pos(parser.current_start);

    // Expect {
    parser.expect(TokenKind::LeftBrace)?;
    parser.skip_whitespace()?;

    let mut children = parser.bvec();

    // Determine what content to expect based on at-rule type and nesting context
    // When nested inside a rule, at-rules that normally contain rules should contain declarations instead
    // NB: `@scope` is deliberately absent — its body is a nested-declarations
    // context (CSS Cascade 6 §"Scoped Style Rules"): the selectors are a
    // `<relative-selector-list>` scoped to `:scope` (a leading combinator like
    // `> p` is valid) and bare declarations apply to `:scope`. It therefore flows
    // through the generic fallback below (relative rules via `parse_rule(nested)`
    // + declarations), not this non-relative rule-list path.
    let expect_rules = (matches!(
        atrule_name,
        "media" | "supports" | "layer" | "container" | "starting-style" | "font-feature-values"
    ) || is_keyframes_atrule(atrule_name))
        && !nested_in_rule;
    // Everything that is NOT a top-level rule-list block flows through the generic
    // block-agnostic fallback below (`parse_block_child`), which disambiguates a bare
    // declaration from a nested rule via `is_nested_rule_start()` per CSS Syntax 3
    // §"consume a block's contents". This covers the declaration-context at-rules
    // (`@font-face`, `@page`, `@property`, `@counter-style`, `@color-profile`,
    // `@position-try`, `@font-palette-values`, the `@font-feature-values` sub-rules, and
    // the `@page` margin boxes like `@top-center`) as well as unknown at-rules and
    // `@scope`. Making them agnostic matches parseCss, which is a pure scan-to-terminator
    // classifier: `@page :first { h1 {} }` parses the `h1 {}` as a nested style rule
    // (declarations end at `;`/`}`, a nested rule at `{`), while `margin: 1cm;` stays a
    // declaration. "Invalid in this context" (a style rule inside `@font-face`) is a later
    // validity concern, deferred to diagnostics — the same permissive-parser posture as
    // elsewhere. Conditional group at-rules (@media/@supports/@layer/@container) nested
    // inside a rule likewise fall through here.

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

        // @media/@supports/@layer/@container/keyframes/… block — a rule-list context.
        // But CSS block parsing is agnostic (CSS Syntax 3 §"consume a block's contents"):
        // a bare declaration still parses here (invalid-in-context, dropped by a later
        // validity step, but parseCss keeps it — e.g. `@media screen { color: red }`, the
        // shape @function/@mixin bodies use). `parse_block_child` disambiguates; `false`
        // keeps the non-relative selector grammar so keyframe stops (`0%`, `from`) and
        // top-level complex selectors parse unchanged.
        if expect_rules {
            children.push(parse_block_child(parser, false, atrule_name)?);
            parser.skip_whitespace()?;
            continue;
        }

        // Nested-declarations fallback (declaration-context at-rules like `@page`/
        // `@font-face`, unknown at-rules, `@scope`): declaration-vs-rule disambiguation
        // via `is_nested_rule_start`. `true` allows a leading combinator (a `> .child {}`
        // relative selector in a `@scope` body).
        children.push(parse_block_child(parser, true, atrule_name)?);
        parser.skip_whitespace()?;
    }

    // Expect }
    if !parser.check(TokenKind::RightBrace) {
        return Err(parser.error_expected("'}'"));
    }
    let end = parser.span_pos(parser.current_end);
    parser.advance()?; // consume }

    Ok(CssAtruleBlock {
        children: children.into_bump_slice(),
        span: Span { start, end },
    })
}

/// Parse one at-rule block child, disambiguating a nested rule from a bare
/// declaration via `is_nested_rule_start` (CSS block parsing is agnostic — CSS
/// Syntax 3 §"consume a block's contents"). `allow_relative_selectors` picks the
/// selector grammar for the rule case: `false` = non-relative (top-level rule-list
/// blocks — conditional groups, keyframes), `true` = relative (nested-declarations
/// contexts — unknown at-rules, `@scope` — where a leading combinator is valid).
fn parse_block_child<'arena>(
    parser: &mut CssParser<'_, 'arena>,
    allow_relative_selectors: bool,
    atrule_name: &str,
) -> Result<CssBlockChild<'arena>, ParseError> {
    if super::declarations::is_nested_rule_start(parser)? {
        Ok(CssBlockChild::Rule(super::declarations::parse_rule(
            parser,
            allow_relative_selectors,
        )?))
    } else if parser.check(TokenKind::Identifier) {
        Ok(CssBlockChild::Declaration(
            super::declarations::parse_declaration(parser)?,
        ))
    } else {
        Err(parser.error_unexpected(&format!("token in @{atrule_name} block")))
    }
}
