// CSS at-rule parsing
//
// Dispatch spine for `@`-prefixed at-rules: parse the name, route the prelude to a
// structured parser (`preludes`) or the verbatim/normalized raw parser (`raw`), then
// parse the optional block. Also holds the shared at-rule predicates.

mod preludes;
mod raw;

pub(super) use super::CssParser;

use crate::ast::*;
use crate::lexer::TokenKind;
use tsv_lang::store::{PoolText, flush_run_move};
use tsv_lang::{Error, Span};

use self::preludes::{
    ConditionReader, parse_condition_query, parse_container_prelude, parse_custom_selector_prelude,
    parse_import_prelude, parse_scope_prelude,
};
use self::raw::parse_raw_prelude_content;

/// Check if an at-rule name is a keyframes rule (including vendor-prefixed versions)
/// e.g., "keyframes", "-webkit-keyframes", "-moz-keyframes"
///
/// The set is spelled twice, here for the printer and in [`AtruleKind::of`]'s `Keyframes` arms
/// for the parser, so the two change together; the module's unit test grades both against one
/// table.
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
/// source case (matching prettier — see `ConnectorRun::text` and the boolean-operator
/// case note in `docs/conformance_prettier_css.md`).
pub(super) fn is_boolean_operator_keyword(ident: &str) -> bool {
    ident.eq_ignore_ascii_case("and")
        || ident.eq_ignore_ascii_case("or")
        || ident.eq_ignore_ascii_case("not")
}

/// What an at-rule's name routes to: the prelude reader that reads it, and whether its block
/// is a rule list. Read off the decoded name, ASCII case-insensitively (CSS Syntax 3), so
/// `@MEDIA` and `@Font-Face` route as their lowercase spellings do.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AtruleKind {
    Import,
    Scope,
    CustomSelector,
    Supports,
    Container,
    Media,
    CustomMedia,
    Namespace,
    Layer,
    StartingStyle,
    FontFeatureValues,
    /// `@keyframes` and its vendor-prefixed spellings ([`is_keyframes_atrule`]).
    Keyframes,
    /// Any other name: a raw prelude, and a block read child by child.
    Other,
}

impl AtruleKind {
    /// The kind `name` (decoded) names. The name is folded to ASCII lowercase once, into a
    /// buffer as long as the longest spelling (a longer name is none of them), and compared
    /// against each spelling of its own length. The `Keyframes` arms spell the set
    /// [`is_keyframes_atrule`] also spells, for the printer: the two change together.
    fn of(name: &str) -> Self {
        let name = name.as_bytes();
        let mut buf = [0u8; "font-feature-values".len()];
        let Some(folded) = buf.get_mut(..name.len()) else {
            return AtruleKind::Other;
        };
        for (dst, byte) in folded.iter_mut().zip(name) {
            *dst = byte.to_ascii_lowercase();
        }
        let is = |spelling: &[u8]| *folded == *spelling;
        match name.len() {
            5 if is(b"scope") => AtruleKind::Scope,
            5 if is(b"media") => AtruleKind::Media,
            5 if is(b"layer") => AtruleKind::Layer,
            6 if is(b"import") => AtruleKind::Import,
            8 if is(b"supports") => AtruleKind::Supports,
            9 if is(b"container") => AtruleKind::Container,
            9 if is(b"namespace") => AtruleKind::Namespace,
            9 if is(b"keyframes") => AtruleKind::Keyframes,
            12 if is(b"custom-media") => AtruleKind::CustomMedia,
            12 if is(b"-o-keyframes") => AtruleKind::Keyframes,
            13 if is(b"-ms-keyframes") => AtruleKind::Keyframes,
            14 if is(b"starting-style") => AtruleKind::StartingStyle,
            14 if is(b"-moz-keyframes") => AtruleKind::Keyframes,
            15 if is(b"custom-selector") => AtruleKind::CustomSelector,
            17 if is(b"-webkit-keyframes") => AtruleKind::Keyframes,
            19 if is(b"font-feature-values") => AtruleKind::FontFeatureValues,
            _ => AtruleKind::Other,
        }
    }

    /// Whether the at-rule's block is a rule list at the top level: the conditional group
    /// rules, `@layer`, `@starting-style`, `@font-feature-values` and the keyframes.
    fn holds_rules(self) -> bool {
        matches!(
            self,
            AtruleKind::Media
                | AtruleKind::Supports
                | AtruleKind::Layer
                | AtruleKind::Container
                | AtruleKind::StartingStyle
                | AtruleKind::FontFeatureValues
                | AtruleKind::Keyframes
        )
    }
}

/// The error for a token no block child starts with, inside the at-rule `name` names: the
/// name read back from the pool, lowercased as the dispatch reads it.
#[cold]
#[inline(never)]
fn unexpected_block_token(parser: &CssParser<'_>, name: PoolText<CssText>) -> Error {
    let name = parser.stores.text.get(name).to_ascii_lowercase();
    parser.error_unexpected(&format!("token in @{name} block"))
}

/// Check if current token is a CSS boolean operator keyword (and, or, not)
pub(super) fn is_boolean_operator(parser: &CssParser<'_>) -> bool {
    if let TokenKind::Identifier = parser.current_kind() {
        is_boolean_operator_keyword(parser.current_identifier())
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
) -> Result<CssAtrule, Error> {
    // two wire levels: the `Atrule` and its `Block` (the depth limit's count)
    parser.nested(2, |p| parse_atrule_body(p, nested_in_rule))
}

/// [`parse_atrule`] inside its depth levels.
fn parse_atrule_body(parser: &mut CssParser<'_>, nested_in_rule: bool) -> Result<CssAtrule, Error> {
    let start = parser.span_pos(parser.current_start());

    // Expect @ symbol
    parser.expect(TokenKind::AtSign)?;

    // Parse at-rule name (identifier after @)
    if !parser.check(TokenKind::Identifier) {
        return Err(parser.error_expected_after("at-rule name", "@"));
    }

    // At-rule names are ASCII case-insensitive (CSS Syntax 3); dispatch on the name
    // ASCII case-folded so `@MEDIA`/`@Font-Face` route to the right prelude/block
    // grammar instead of the raw fall-through. Classified off the decoded text before
    // anything is pushed, so no borrow of it outlives this statement. The stored `name`
    // keeps its source case (the public AST matches Svelte, which preserves it —
    // `@SUPPORTS` → `"name": "SUPPORTS"`); the printer lowercases it for output.
    let kind = AtruleKind::of(parser.current_identifier());
    // Internal AST: use decoded value (spec-compliant). The name token's source
    // span is captured too — the printer emits the name from source (escape-
    // preserving), not from the decoded string (which may hold a raw control char).
    let name = parser.current_identifier_in_pool();
    let name_span = Span {
        start: parser.span_pos(parser.current_start()),
        end: parser.span_pos(parser.current_end()),
    };
    parser.advance()?;

    parser.skip_whitespace()?;

    // Raw source offset of the first prelude token — for the conditional-at-rule raw
    // fallback after the dispatch. Captured before any prelude parsing consumes tokens.
    let prelude_start_raw = parser.current_start();
    #[cfg(feature = "_internal_ast_census")]
    let ast_census_mark = tsv_lang::ast_census_probe::mark();

    // Parse prelude based on at-rule type
    let prelude = if kind == AtruleKind::Import {
        // Parse @import prelude structurally: a `url()`/string head, `layer`/`supports()`
        // calls, then one verbatim tail (the media-query list, or whatever else the author
        // wrote) to the prelude end. Nothing here rejects short of the prelude's own end —
        // parseCss stores any prelude raw — so the fallback below is not needed; only an
        // empty prelude takes the raw path internally.
        parse_import_prelude(parser)?
    } else if kind == AtruleKind::Scope {
        // Parse @scope prelude as structured selector lists (`PreludeValue::Selectors`).
        parse_scope_prelude(parser)?
    } else if kind == AtruleKind::CustomSelector {
        // `@custom-selector :--name <selector-list>` — the third prelude reader (the
        // selector printer), the one standard-CSS at-rule prettier routes to
        // `parseSelector`. Dispatched on the lowercased name like every at-rule here;
        // prettier's own test is case-sensitive (a cataloged divergence). Falls back to
        // the raw prelude internally when the shape does not match.
        parse_custom_selector_prelude(parser)?
    } else if kind == AtruleKind::Supports {
        // Parse @supports prelude as structured conditions (for line-width wrapping)
        let (condition, span) = parse_condition_query(parser, ConditionReader::Value)?;
        PreludeValue::Supports { condition, span }
    } else if kind == AtruleKind::Container {
        // Parse @container prelude as structured conditions (for line-width wrapping)
        let (name, condition, span) = parse_container_prelude(parser)?;
        PreludeValue::Container {
            name,
            condition,
            span,
        }
    } else if matches!(kind, AtruleKind::Media | AtruleKind::CustomMedia) {
        // Parse @media as raw string to preserve comments
        // Wrapping is handled in the printer by finding and/or boundaries
        // Fully structuring preludes is a deferred design option — see
        // docs/architecture.md § "Red-Green Trees (Deferred)"
        //
        // `@custom-media` reads through the same media-query parser as `@media` —
        // prettier routes `["media", "custom-media"]` to `parseMediaQuery` — so its
        // prelude takes the media reader's whitespace, feature-name case and number
        // rules rather than falling to the verbatim raw branch below. The `--name` it
        // leads with is an `<extension-name>` (css-extensions-1), preserved as any
        // case-sensitive feature name is. Routing is printer-only: the wire prelude is
        // `strip_css_comments(span.extract(source))` for both this arm and `Raw`, and
        // `parse_raw_prelude_content`'s span is flag-independent.
        let (content, span) = parse_raw_prelude_content(parser, true, false)?;
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
        let is_namespace = kind == AtruleKind::Namespace;
        let (content, span) = parse_raw_prelude_content(parser, is_namespace, is_namespace)?;
        PreludeValue::Raw { content, span }
    };

    // Raw fallback for conditional at-rules (`@supports`/`@container`) whose prelude
    // isn't a structurable condition. Per CSS Syntax 3 an at-rule prelude is always
    // consumed as component values; a prelude that isn't a valid
    // `<supports-condition>`/`<container-condition>` makes the rule evaluate false — it
    // is NOT a parse error (parseCss stores it raw, prettier prints it verbatim).
    // `parse_condition_query` breaks off at the first token it can't fold in, leaving
    // those tokens unconsumed; re-emit the whole prelude verbatim instead of erroring at
    // the block boundary below. (`@import` needs none: its reader consumes every prelude to
    // the boundary, structuring what it can and keeping the rest as one verbatim tail.)
    //
    // A prelude the reader found no part in at all is no condition either, and when it holds a
    // boundary run it takes the same path: the run sits among the prelude's words (a container
    // name, an operator with no operand — `@container x <NBSP>;`, `@supports and <NBSP>;`),
    // where only the verbatim reader, whose ends are spelled as gaps, keeps each where it was.
    let partless_with_run = match &prelude {
        PreludeValue::Supports { condition, .. } | PreludeValue::Container { condition, .. } => {
            condition.parts.is_empty()
                && crate::whitespace::holds_boundary_member(
                    parser.source(),
                    prelude_start_raw,
                    parser.current_start(),
                )
        }
        _ => false,
    };
    let prelude = if matches!(kind, AtruleKind::Supports | AtruleKind::Container)
        && (!parser.at_prelude_end() || partless_with_run)
    {
        #[cfg(feature = "_internal_ast_census")]
        tsv_lang::ast_census_probe::rewind(ast_census_mark);
        reconsume_prelude_as_raw(parser, prelude_start_raw)?
    } else {
        prelude
    };

    // Parse block (if present)
    let (block, end) = if parser.check(TokenKind::LeftBrace) {
        let block = parse_atrule_block(parser, kind, name, nested_in_rule)?;
        let end = block.span.end;
        (Some(block), end)
    } else if parser.check(TokenKind::Semicolon) {
        // Statement at-rule (no block)
        let end = parser.span_pos(parser.current_end());
        parser.advance()?;
        return Ok(CssAtrule {
            name,
            name_span,
            prelude,
            block: None,
            span: Span { start, end },
        });
    } else {
        return Err(parser.error_expected_after("'{' or ';'", "at-rule prelude"));
    };

    Ok(CssAtrule {
        name,
        name_span,
        prelude,
        block,
        span: Span { start, end },
    })
}

/// Re-emit an at-rule prelude that couldn't be structured (a non-condition
/// `@supports`/`@container`, or an `@import` prelude holding nothing but whitespace and
/// comments) as a raw verbatim prelude.
///
/// Consumes the remaining prelude tokens up to the block boundary and rebuilds the
/// **whole** prelude — from `prelude_start_raw`, the raw (base-offset-free) source
/// offset of its first token — verbatim and outer-trimmed, as `PreludeValue::Raw`
/// (whose printer emits it as-is and whose convert extracts it from `span`). This
/// matches parseCss (which stores an unstructurable prelude raw) and prettier (which
/// prints it verbatim); see the call sites for the CSS-Syntax rationale.
pub(super) fn reconsume_prelude_as_raw(
    parser: &mut CssParser<'_>,
    prelude_start_raw: usize,
) -> Result<PreludeValue, Error> {
    // Re-read from the prelude's first token, so its ends are observed over all of it. The
    // registry keeps everything the abandoned reader registered (the truncation is to its own
    // length): only the cursor moves. Every token is taken verbatim, so the output length at a
    // token is its raw offset.
    let comments_len = parser.comments.len();
    parser.rewind_to(prelude_start_raw, comments_len)?;
    let mut ends = raw::PreludeEnds::default();
    while !parser.at_prelude_end() {
        ends.observe(parser, parser.current_start() - prelude_start_raw, 0);
        parser.advance()?;
    }

    // Verbatim prelude text runs from its first token to the boundary token; outer
    // whitespace is trimmed so the public AST (from `span`) and printer `content` agree.
    // Escape-aware, and CSS-whitespace-only: a prelude can end in an escape whose payload
    // is that whitespace (`@layer a\ ;`), and an NBSP is prelude content, not padding.
    // Only the printer-facing `content` takes the ends' respelling (`raw::PreludeEnds`); the
    // span, which the wire extracts from, is the author's.
    let raw = &parser.source()[prelude_start_raw..parser.current_start()];
    let lead = crate::escapes::trim_start_css(raw);
    let trimmed = crate::escapes::trim_end_preserving_escape(lead);
    let content_start_raw = prelude_start_raw + (raw.len() - lead.len());
    let content_end_raw = content_start_raw + trimmed.len();
    let respelled = ends.respell(
        parser.source(),
        prelude_start_raw,
        parser.current_start(),
        raw,
    );
    let content = respelled
        .as_deref()
        .map_or(trimmed, |respelled| crate::escapes::trim_css(respelled));

    let content = parser.stores.text.push(content);
    Ok(PreludeValue::Raw {
        content,
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
/// `kind` is the at-rule's name classified, and `name` the name itself, for an error.
/// `nested_in_rule`: true if this at-rule is nested inside a regular rule's declaration block
fn parse_atrule_block(
    parser: &mut CssParser<'_>,
    kind: AtruleKind,
    name: PoolText<CssText>,
    nested_in_rule: bool,
) -> Result<CssAtruleBlock, Error> {
    let start = parser.span_pos(parser.current_start());

    // Expect {
    parser.expect(TokenKind::LeftBrace)?;
    // `allow_comment_or_whitespace`, here and after every child below — so the skip is the
    // boundary one, which also steps a JS-`\s` run the lexer read as the head of an
    // identifier (`CssParser::skip_boundary_whitespace`). This loop is the one such juncture
    // that does not route through `skip_html_comment_markers`: legacy `<!--` markers are a
    // stylesheet-body and style-rule-block form, not an at-rule-block one.
    parser.skip_boundary_whitespace()?;

    let mark = parser.stores.runs.block_child_scratch.mark();

    // Determine what content to expect based on at-rule type and nesting context
    // When nested inside a rule, at-rules that normally contain rules should contain declarations instead
    // NB: `@scope` is deliberately absent — its body is a nested-declarations
    // context (CSS Cascade 6 §"Scoped Style Rules"): the selectors are a
    // `<relative-selector-list>` scoped to `:scope` (a leading combinator like
    // `> p` is valid) and bare declarations apply to `:scope`. It therefore flows
    // through the generic fallback below (relative rules via `parse_rule(nested)`
    // + declarations), not this non-relative rule-list path.
    let expect_rules = kind.holds_rules() && !nested_in_rule;
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
        if matches!(parser.current_kind(), TokenKind::Comment) {
            let comment = parser.parse_block_comment()?;
            parser
                .stores
                .runs
                .block_child_scratch
                .push(CssBlockChild::Comment(comment));
            continue;
        }

        // Handle nested at-rules
        if parser.check(TokenKind::AtSign) {
            // Propagate the nesting context: an at-rule nested inside a conditional
            // group at-rule that is itself inside a style rule is still in a nesting
            // context, so its block holds declarations (per CSS Nesting). Outside a
            // rule, `nested_in_rule` is false and this stays a normal at-rule block.
            let atrule = parse_atrule(parser, nested_in_rule)?;
            parser
                .stores
                .runs
                .block_child_scratch
                .push(CssBlockChild::Atrule(atrule));
            parser.skip_boundary_whitespace()?;
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
            let child = parse_block_child(parser, false, name)?;
            parser.stores.runs.block_child_scratch.push(child);
            parser.skip_boundary_whitespace()?;
            continue;
        }

        // Nested-declarations fallback (declaration-context at-rules like `@page`/
        // `@font-face`, unknown at-rules, `@scope`): declaration-vs-rule disambiguation
        // via `is_nested_rule_start`. `true` allows a leading combinator (a `> .child {}`
        // relative selector in a `@scope` body).
        let child = parse_block_child(parser, true, name)?;
        parser.stores.runs.block_child_scratch.push(child);
        parser.skip_boundary_whitespace()?;
    }

    // Expect }
    if !parser.check(TokenKind::RightBrace) {
        return Err(parser.error_expected("'}'"));
    }
    let end = parser.span_pos(parser.current_end());
    parser.advance()?; // consume }

    let children = flush_run_move(
        &mut parser.stores.runs.block_child_scratch,
        &mut parser.stores.runs.block_children,
        mark,
    );
    Ok(CssAtruleBlock {
        children,
        span: Span { start, end },
    })
}

/// Parse one at-rule block child, disambiguating a nested rule from a bare
/// declaration via `is_nested_rule_start` (CSS block parsing is agnostic — CSS
/// Syntax 3 §"consume a block's contents"). `allow_relative_selectors` picks the
/// selector grammar for the rule case: `false` = non-relative (top-level rule-list
/// blocks — conditional groups, keyframes), `true` = relative (nested-declarations
/// contexts — unknown at-rules, `@scope` — where a leading combinator is valid).
fn parse_block_child(
    parser: &mut CssParser<'_>,
    allow_relative_selectors: bool,
    name: PoolText<CssText>,
) -> Result<CssBlockChild, Error> {
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
        Err(unexpected_block_token(parser, name))
    }
}

#[cfg(test)]
mod tests {
    use super::{AtruleKind, is_keyframes_atrule};

    /// The spellings other than the keyframes', as a plain table.
    const NAMED: [(&str, AtruleKind); 11] = [
        ("import", AtruleKind::Import),
        ("scope", AtruleKind::Scope),
        ("custom-selector", AtruleKind::CustomSelector),
        ("supports", AtruleKind::Supports),
        ("container", AtruleKind::Container),
        ("media", AtruleKind::Media),
        ("custom-media", AtruleKind::CustomMedia),
        ("namespace", AtruleKind::Namespace),
        ("layer", AtruleKind::Layer),
        ("starting-style", AtruleKind::StartingStyle),
        ("font-feature-values", AtruleKind::FontFeatureValues),
    ];

    /// The keyframes spellings: `@keyframes` and its vendor-prefixed forms.
    const KEYFRAMES: [&str; 5] = [
        "keyframes",
        "-webkit-keyframes",
        "-moz-keyframes",
        "-o-keyframes",
        "-ms-keyframes",
    ];

    /// The plain reading `AtruleKind::of` and [`is_keyframes_atrule`] must agree with: an
    /// ASCII-case-insensitive scan of the keyframes spellings, then of the table.
    fn reference(name: &str) -> AtruleKind {
        if KEYFRAMES
            .iter()
            .any(|spelling| name.eq_ignore_ascii_case(spelling))
        {
            return AtruleKind::Keyframes;
        }
        NAMED
            .iter()
            .find(|(spelling, _)| name.eq_ignore_ascii_case(spelling))
            .map_or(AtruleKind::Other, |&(_, kind)| kind)
    }

    fn check(name: &str) {
        let expected = reference(name);
        assert!(
            AtruleKind::of(name) == expected,
            "`AtruleKind::of` disagreed with the plain scan on {name:?}"
        );
        assert!(
            is_keyframes_atrule(name) == (expected == AtruleKind::Keyframes),
            "`is_keyframes_atrule` disagreed with the plain scan on {name:?}"
        );
    }

    #[test]
    fn of_agrees_with_a_plain_scan_of_the_spellings() {
        let keyframes = KEYFRAMES.map(|spelling| (spelling, AtruleKind::Keyframes));
        for &(spelling, kind) in NAMED.iter().chain(&keyframes) {
            assert!(AtruleKind::of(spelling) == kind, "{spelling} lost its kind");
            assert!(AtruleKind::of(&spelling.to_ascii_uppercase()) == kind);
            let mut mixed = spelling.to_string();
            mixed[..2].make_ascii_uppercase();
            assert!(AtruleKind::of(&mixed) == kind, "{mixed} lost its kind");

            // One byte longer or shorter, at either end.
            check(&format!("{spelling}s"));
            check(&format!("x{spelling}"));
            check(&spelling[1..]);
            check(&spelling[..spelling.len() - 1]);
            // Same length, each byte with its ASCII case bit flipped: a letter's other case
            // (still the spelling), a `-` turned into a carriage return (no longer it).
            for i in 0..spelling.len() {
                let mut flipped = spelling.as_bytes().to_vec();
                flipped[i] ^= 0x20;
                check(std::str::from_utf8(&flipped).unwrap());
                // Same length, each byte swapped for another ASCII byte: no longer the spelling.
                let mut swapped = spelling.as_bytes().to_vec();
                swapped[i] ^= 0x01;
                check(std::str::from_utf8(&swapped).unwrap());
            }
            // A non-ASCII character in place of a letter: the fold is ASCII-only, so a
            // character Unicode would case-map to the letter does not match it.
            check(&spelling.replacen('e', "é", 1));
            check(&spelling.replacen('k', "\u{212A}", 1));
            check(&spelling.replacen('s', "\u{17F}", 1));
            check(&spelling.replacen('i', "\u{130}", 1));
        }
        let longest = "font-feature-values".len();
        for name in [
            "",
            "-",
            "font-face",
            "page",
            "tailwind",
            "apply",
            "font-feature-values-x",
            "FONT-FEATURE-VALUES-",
            "\u{0}",
            "m\u{0}dia",
            &"a".repeat(longest + 1),
            &"-webkit-keyframes".repeat(4),
        ] {
            check(name);
        }
    }
}
