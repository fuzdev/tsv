// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Shorthand-attribute spans (`<div { x }>`) — the identifier, not the braces interior.
//!
//! Svelte's `read_attribute` (`.../1-parse/state/element.js`) eats `{`, runs
//! `allow_whitespace()`, then `read_identifier()`, and hands the identifier's own
//! position to the attribute: `create_attribute(id.name, id.loc, start, parser.index,
//! expression)`. So a padded `{ x }` names just the `x` — the braces AND the padding are
//! outside `name_loc`, and the synthesized `ExpressionTag` / `Identifier` carry that same
//! narrow span (`{start: id.start, end: id.end}`).
//!
//! tsv used to take the whole braces interior for all three, which put the padding inside
//! every span (`name_loc` 6..9 instead of 7..8 on `<div { x }>`). That made the
//! **span-only** `--no-locations` wire wrong too, not just `loc`.
//!
//! Not fixturable: `format` normalizes `{ x }` → `{x}`, so no format-stable `input.svelte`
//! can hold the trigger (the same reason the `:nth-*()` span trims live in
//! `tests/svelte_css_nth_span.rs`). These root tests pin the offsets offline, transcribed
//! from the live modern Svelte parser (`tsv_debug canonical_parse`).

use serde_json::Value;

/// Parse `src` and return its first `Attribute` wire node.
fn first_attribute(src: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("parser should accept the component");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    find_attribute(&json).expect("an Attribute node")
}

fn find_attribute(node: &Value) -> Option<Value> {
    match node {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("Attribute") {
                return Some(node.clone());
            }
            map.values().find_map(find_attribute)
        }
        Value::Array(items) => items.iter().find_map(find_attribute),
        _ => None,
    }
}

/// Assert the three spans a shorthand attribute derives from its identifier: `name_loc`
/// (both endpoints, with the line/column the oracle reports), the `ExpressionTag`, and the
/// `Identifier` inside it. `attr` itself keeps the full `{…}` span.
fn assert_shorthand_spans(
    attr: &Value,
    attr_span: (i64, i64),
    ident_span: (i64, i64),
    name_line_column: (i64, i64),
) {
    assert_eq!(attr["name"], "x", "attribute name");
    assert_eq!(attr["start"].as_i64(), Some(attr_span.0), "attr.start");
    assert_eq!(attr["end"].as_i64(), Some(attr_span.1), "attr.end");

    let name_loc = &attr["name_loc"];
    assert_eq!(
        name_loc["start"]["character"].as_i64(),
        Some(ident_span.0),
        "name_loc.start.character"
    );
    assert_eq!(
        name_loc["end"]["character"].as_i64(),
        Some(ident_span.1),
        "name_loc.end.character"
    );
    assert_eq!(
        name_loc["start"]["line"].as_i64(),
        Some(name_line_column.0),
        "name_loc.start.line"
    );
    assert_eq!(
        name_loc["start"]["column"].as_i64(),
        Some(name_line_column.1),
        "name_loc.start.column"
    );

    let tag = &attr["value"];
    assert_eq!(tag["type"], "ExpressionTag");
    assert_eq!(tag["start"].as_i64(), Some(ident_span.0), "tag.start");
    assert_eq!(tag["end"].as_i64(), Some(ident_span.1), "tag.end");

    let ident = &tag["expression"];
    assert_eq!(ident["type"], "Identifier");
    assert_eq!(ident["name"], "x");
    assert_eq!(ident["start"].as_i64(), Some(ident_span.0), "ident.start");
    assert_eq!(ident["end"].as_i64(), Some(ident_span.1), "ident.end");
}

/// `<div {x}>` — no padding, so the braces interior *is* the identifier. The case every
/// fixture covers; pinned here as the control.
#[test]
fn tight_shorthand_names_the_identifier() {
    let attr = first_attribute("<div {x}>t</div>");
    assert_shorthand_spans(&attr, (5, 8), (6, 7), (1, 6));
}

/// `<div { x }>` — one space each side. The identifier is at 7..8; the braces interior
/// (6..9) is what tsv used to report.
#[test]
fn padded_shorthand_excludes_the_padding() {
    let attr = first_attribute("<div { x }>t</div>");
    assert_shorthand_spans(&attr, (5, 10), (7, 8), (1, 7));
}

/// `<div {\n\tx\n}>` — the padding spans lines, so the wrong span also reported the wrong
/// LINE (1 instead of 2). `{`=5, `\n`=6, `\t`=7, `x`=8, `\n`=9, `}`=10.
#[test]
fn multiline_shorthand_names_the_identifier_line() {
    let attr = first_attribute("<div {\n\tx\n}>t</div>");
    assert_shorthand_spans(&attr, (5, 11), (8, 9), (2, 1));
}

/// The carve-out that keeps a padded shorthand distinguishable: a top-level
/// `<script>`/`<style>` attribute is read by `read_static_attribute`, which never parses a
/// shorthand — `<script {x}>` is an attribute *named* `{x}`, value `true`, whose `name_loc`
/// covers the whole braced run. A shorthand's name is never the verbatim source at its
/// start; this one's is.
#[test]
fn script_static_brace_attribute_names_the_whole_run() {
    let attr = first_attribute("<script {x}>\nlet y = 1;\n</script>");
    assert_eq!(attr["name"], "{x}");
    assert_eq!(attr["start"].as_i64(), Some(8), "attr.start");
    assert_eq!(attr["end"].as_i64(), Some(11), "attr.end");
    assert_eq!(attr["value"], Value::Bool(true), "static attribute value");
    assert_eq!(attr["name_loc"]["start"]["character"].as_i64(), Some(8));
    assert_eq!(attr["name_loc"]["end"]["character"].as_i64(), Some(11));
}
