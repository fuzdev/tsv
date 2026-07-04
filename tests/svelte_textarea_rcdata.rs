// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! `<textarea>` RCDATA content in the Svelte parser.
//!
//! `<textarea>` is Svelte's sole RCDATA element (HTML §13.2.5.2): its children are
//! raw text with live `{expr}` interpolation but no nested elements — a `<p>` inside
//! is *text*, not a `RegularElement` — read up to a whitespace/attribute-tolerant
//! `</textarea…>` (`regex_closing_textarea_tag = /<\/textarea(\s[^>]*)?>/iy` in
//! `../svelte/packages/svelte/src/compiler/phases/1-parse/state/element.js`). tsv
//! previously parsed the children as elements (and so rejected the wild close forms
//! below outright — an over-rejection of valid Svelte markup).
//!
//! The everyday case (`<textarea>\n\t<p>x {expr}</p>\n</textarea>`) is pinned by the
//! `tests/fixtures/svelte/elements/textarea_rcdata` fixture. These root tests pin
//! what a fixture can NOT see — the whitespace/attribute/case tolerance of the close
//! and the character-reference decode — because prettier reflows the wild close
//! forms (its `output != input`, so they can't be an idempotent `input.svelte`).
//!
//! The expected skeletons/offsets are transcribed from the live modern Svelte parser
//! (`tsv_debug canonical_parse`), several of them over Svelte's own test inputs
//! (`parser-legacy/samples/textarea-*`).

use serde_json::Value;

/// Parse `src`, convert to the wire AST, and reduce `Root.fragment.nodes` to a
/// compact `name(start-end)[kids…]` skeleton. Elements recurse through
/// `fragment.nodes`; other nodes (Text, ExpressionTag, …) render as `Type(start-end)`
/// leaves — enough to pin the RCDATA structure (which children are Text vs
/// ExpressionTag) and the close-tolerant element `end`.
fn rcdata_skeleton(src: &str) -> String {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("parser should accept <textarea> RCDATA");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    let nodes = json["fragment"]["nodes"]
        .as_array()
        .expect("Root.fragment.nodes array");
    reduce(nodes)
}

fn reduce(nodes: &[Value]) -> String {
    let mut parts = Vec::new();
    for node in nodes {
        let ty = node["type"].as_str().unwrap_or("?");
        let start = node["start"].as_i64().unwrap_or(-1);
        let end = node["end"].as_i64().unwrap_or(-1);
        let label = node.get("name").and_then(Value::as_str).unwrap_or(ty);
        let kids = node
            .get("fragment")
            .and_then(|f| f.get("nodes"))
            .and_then(Value::as_array);
        match kids {
            Some(kids) if !kids.is_empty() => {
                parts.push(format!("{label}({start}-{end})[{}]", reduce(kids)));
            }
            _ => parts.push(format!("{label}({start}-{end})")),
        }
    }
    parts.join(" ")
}

/// The `raw` / `data` of the first `<textarea>`'s first `Text` child.
fn first_textarea_text(src: &str) -> (String, String) {
    let arena = bumpalo::Bump::new();
    let ast = tsv_svelte::parse(src, &arena).expect("parser should accept <textarea> RCDATA");
    let json = tsv_svelte::convert_ast_json(&ast, src);
    let text = &json["fragment"]["nodes"][0]["fragment"]["nodes"][0];
    (
        text["raw"].as_str().expect("Text.raw").to_owned(),
        text["data"].as_str().expect("Text.data").to_owned(),
    )
}

/// Svelte's `parser-legacy/samples/textarea-children`: a nested `<p>` (with a `{expr}`)
/// is RCDATA *text*, not an element.
#[test]
fn nested_tag_is_text() {
    let src = "<textarea>\n\t<p>not actually an element. {foo}</p>\n</textarea>\n";
    assert_eq!(
        rcdata_skeleton(src),
        "textarea(0-61)[Text(10-40) ExpressionTag(40-45) Text(45-50)]",
    );
}

/// Svelte's `parser-legacy/samples/textarea-end-tag`: the close is whitespace-tolerant
/// and greedy, so three earlier `</textar…`/`</textarea…` runs are all *text* — the
/// element closes only at the final `</textarea\n\n\n</textarea\n\n>` (the `[^>]*` runs to
/// the first `>`). tsv used to reject this at the first `</textar `.
#[test]
fn whitespace_tolerant_close_skips_false_ends() {
    let src = "<textarea>\n\t<p>not actu </textar ally an element. {foo}</p>\n</textare\n\n\n> </textaread >asdf</textarea\n\n\n</textarea\n\n>\n\n";
    assert_eq!(
        rcdata_skeleton(src),
        "textarea(0-117)[Text(10-50) ExpressionTag(50-55) Text(55-91)]",
    );
}

/// The close tag may carry attributes after a required whitespace (`</textarea data-x >`);
/// the element ends past that close's `>`.
#[test]
fn attribute_tolerant_close() {
    let src = "<textarea>abc</textarea data-x >";
    assert_eq!(rcdata_skeleton(src), "textarea(0-32)[Text(10-13)]");
}

/// The close tag name is matched case-insensitively (`</TEXTAREA>`).
#[test]
fn case_insensitive_close() {
    let src = "<textarea>abc</TEXTAREA>";
    assert_eq!(rcdata_skeleton(src), "textarea(0-24)[Text(10-13)]");
}

/// An expression-only body emits a single `ExpressionTag` — the empty text chunks on
/// either side are skipped (Svelte's `flush` only emits a non-empty chunk).
#[test]
fn expression_only_skips_empty_text() {
    let src = "<textarea>{value}</textarea>";
    assert_eq!(rcdata_skeleton(src), "textarea(0-28)[ExpressionTag(10-17)]");
}

/// An empty `<textarea>` has no children (both flush chunks are empty).
#[test]
fn empty_textarea_has_no_children() {
    let src = "<textarea></textarea>";
    assert_eq!(rcdata_skeleton(src), "textarea(0-21)");
}

/// A run that looks like a close but isn't (`</textareax`, `</textarea/`) stays text; the
/// real `</textarea>` after it closes.
#[test]
fn near_miss_close_stays_text() {
    let src = "<textarea></textareax></textarea>";
    assert_eq!(rcdata_skeleton(src), "textarea(0-33)[Text(10-22)]");
}

/// A `{` with no matching `}` is an unterminated expression tag — rejected, matching
/// Svelte (which also rejects the malformed brace).
#[test]
fn unterminated_expression_rejected() {
    let arena = bumpalo::Bump::new();
    assert!(tsv_svelte::parse("<textarea>{foo</textarea>", &arena).is_err());
}

/// A close-less `<textarea>` (EOF before any `</textarea…>`) is rejected, matching
/// Svelte's `unexpected_eof`.
#[test]
fn unclosed_textarea_rejected() {
    let arena = bumpalo::Bump::new();
    assert!(tsv_svelte::parse("<textarea>abc</textarea", &arena).is_err());
}

/// RCDATA text decodes character references with attribute-value rules — Svelte's
/// `read_sequence` calls `decode_character_references(raw, true)`, so `data` is the
/// decoded form while `raw` keeps the entity.
#[test]
fn character_references_decode() {
    let (raw, data) = first_textarea_text("<textarea>&amp;</textarea>");
    assert_eq!(raw, "&amp;");
    assert_eq!(data, "&");
}
