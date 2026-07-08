// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Legacy `<!-- ... -->` HTML-comment markers (CDO/CDC) in CSS.
//!
//! Svelte's `parseCss` treats `<!--` … `-->` as a comment span at its
//! `allow_comment_or_whitespace` boundaries (`.../read/style.js`): it reads to the
//! required `-->` and **discards everything between**, emitting no node. This
//! departs from CSS Syntax 3, where `<!--` (CDO) and `-->` (CDC) are two
//! *independent* no-op tokens and the content between them parses as ordinary CSS.
//! tsv is a drop-in for `parseCss`, so it matches the swallow — see
//! `docs/conformance_svelte.md` §CSS Compat Behaviors.
//!
//! Not fixturable: the swallow *drops* content (so an idempotent input can't show
//! it), and prettier can't format CDO/CDC — it mangles the markers into invalid CSS
//! (`<!-- -- >`), so there is no format oracle (the `css/tokens/html_comment_prettier_divergence`
//! fixture pins the format-drop; prettier's mangling is the `◆prettier_bug`). These
//! root tests pin the parse-parity the fixture can't: which positions swallow, which
//! keep the markers raw, and which reject — transcribed from the live modern Svelte
//! parser (`tsv_debug canonical_parse`).

use serde_json::Value;

fn parse_json(src: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let ast = tsv_css::parse(src, &arena).expect("parser should accept the CSS");
    tsv_css::convert_ast_json(&ast, src)
}

fn accepts(src: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_css::parse(src, &arena).is_ok()
}

/// Top-level `children` count (rules + at-rules).
fn child_count(src: &str) -> usize {
    parse_json(src)["children"].as_array().map_or(0, Vec::len)
}

fn format(src: &str) -> String {
    let arena = bumpalo::Bump::new();
    let ast = tsv_css::parse(src, &arena).expect("parser should accept the CSS");
    tsv_css::format(&ast, src)
}

/// At `read_body` boundaries — top level and inside a block — a marker is discarded
/// and the surrounding rules/declarations survive.
#[test]
fn statement_boundaries_swallow_markers() {
    assert_eq!(
        child_count("<!-- --> h1 { color: red }"),
        1,
        "top level before rule"
    );
    assert_eq!(
        child_count("h1 { color: red } <!-- --> p { color: blue }"),
        2,
        "between rules"
    );
    assert_eq!(
        child_count("h1 { <!-- --> color: red }"),
        1,
        "block body before decl"
    );
    assert_eq!(
        child_count("h1 { color: red; <!-- --> }"),
        1,
        "block body after decl"
    );
    // Nested block body (CSS Nesting) is a `read_block` boundary too.
    assert!(
        accepts("div { & h1 { <!-- --> color: red } }"),
        "nested block body"
    );
}

/// At `read_selector_list` boundaries a marker is allowed after a complete selector
/// (before `{`/`,`) and after a comma — because `parse_combinator` stops at `<`,
/// leaving the marker for the list loop rather than reading it as a compound.
#[test]
fn selector_list_boundaries_accept_markers() {
    assert!(
        accepts("h1 <!-- --> { color: red }"),
        "after selector, before {{"
    );
    assert!(
        accepts("h1 <!-- -->, p { color: red }"),
        "after selector, before ,"
    );
    assert!(accepts(".a, <!-- --> .b { color: red }"), "after comma");
}

/// But NOT between compounds: there the `<` is hit inside `read_selector`, so the
/// marker never reaches a boundary — a reject, matching `parseCss`.
#[test]
fn between_compounds_rejects() {
    assert!(!accepts("h1 <!-- --> p { color: red }"), "descendant gap");
}

/// The canonical browser-hiding idiom wraps the WHOLE stylesheet: `<!--` at the top,
/// `-->` at the bottom. The span swallows every rule, so the stylesheet is empty —
/// and `format` therefore deletes all the CSS (matching Svelte's compiled output;
/// the rules are already dead there). Per the CSS spec the rules would survive, but
/// tsv is a drop-in for `parseCss`, not the spec.
#[test]
fn canonical_idiom_swallows_whole_stylesheet() {
    let idiom = "<!--\nh1 { color: red }\np { color: blue }\n-->";
    assert_eq!(child_count(idiom), 0, "all wrapped rules swallowed");
    assert!(
        format(idiom).trim().is_empty(),
        "format deletes the wrapped CSS"
    );
}

/// A non-empty marker between statements drops its wrapped content entirely (the
/// `p` rule vanishes), leaving only the sibling rule.
#[test]
fn content_between_markers_is_discarded() {
    let json = parse_json("<!-- p { color: blue } --> h1 { color: red }");
    let children = json["children"].as_array().expect("children");
    assert_eq!(children.len(), 1, "only the h1 rule survives");
    let name = &children[0]["prelude"]["children"][0]["children"][0]["selectors"][0]["name"];
    assert_eq!(name, "h1", "the surviving rule is h1, not the swallowed p");
}

/// In value and at-rule-prelude position the markers are NOT special — those readers
/// scan raw, so `<!--`/`-->` stay literal text and a `;`/`{` between them is
/// significant (unlike at a boundary). Matches `parseCss`.
#[test]
fn markers_are_raw_in_value_and_prelude() {
    let json = parse_json("h1 { color: a <!-- b --> d }");
    assert_eq!(
        json["children"][0]["block"]["children"][0]["value"], "a <!-- b --> d",
        "marker kept raw in value"
    );

    let json = parse_json("@media <!-- --> screen { h1 { color: red } }");
    assert_eq!(
        json["children"][0]["prelude"], "<!-- --> screen",
        "marker kept raw in prelude"
    );

    // A `;` between the markers in value position splits the declaration (raw text),
    // exactly as `parseCss` does — it does NOT swallow to `-->`. The resulting
    // colon-less `c --> d` is then rejected by tsv's (deliberately stricter,
    // prettier-tracking) declaration reader — the separate, sanctioned colon
    // divergence documented in conformance_svelte.md §CSS Parser Scope, not a marker
    // effect. `h1 { color: a; c d }` (no marker) rejects the same way.
    assert!(
        !accepts("h1 { color: a <!-- b; c --> d }"),
        "inner ; splits, colon-less half rejects"
    );
}

/// A `<!--` with no `-->` is an error, mirroring Svelte's `eat('-->', true)`.
#[test]
fn unterminated_marker_errors() {
    assert!(!accepts("<!-- h1 { color: red }"), "unterminated <!--");
    assert!(
        !accepts("h1 { color: red } <!-- p { color: blue }"),
        "unterminated between rules"
    );
}

/// A bare `<` that does not begin `<!--` is untouched — still a range operator / an
/// error in selector context, never a marker. `<!--`-adjacent inputs don't change a
/// plain `<`'s meaning.
#[test]
fn bare_less_than_is_not_a_marker() {
    // `@container (width < 700px)` keeps `<` as an ordinary range operator.
    assert!(
        accepts("@container (width < 700px) { h1 { color: red } }"),
        "container range op"
    );
}
