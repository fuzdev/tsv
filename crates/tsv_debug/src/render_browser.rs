//! Browser-render model for the render-equivalence **fallback** arm.
//!
//! [`crate::render_normalize`] models the Svelte 5 *compiler*: what the compiler
//! trims out of a template before it ever reaches the browser. This module adds
//! the layer above it — what the *browser* then ignores when it renders that
//! output — and is the AST-level counterpart of the sidecar's `visibleSegments`
//! reduction, the authoritative compile arm's oracle.
//!
//! It is deliberately **not** part of `render_normalize`, and no other caller
//! uses it. `ast_diff --render`, `roundtrip_audit` and `fuzz` ask a narrower
//! question — did the *document* change — and answering it with the browser
//! model would blind them to a whitespace `Text` node appearing or vanishing
//! next to a block element, which for them is real structural drift.
//!
//! Two rules, both keyed on facts the compile arm already applies:
//!
//! 1. **Block-boundary whitespace vanishes.** Whitespace adjacent to a
//!    block-level element is not visible render — `</div> <div>` and
//!    `</div><div>` paint identically — while the same whitespace between inline
//!    elements is a visible space. This is the rule the fallback lacked.
//! 2. **A single-expression attribute value has one meaning, two shapes.**
//!    Svelte parses `a={x}` to `value: ExpressionTag` but `a="{x}"` to
//!    `value: [ExpressionTag]`. The quoting is representation; the expression
//!    and the render are identical (verified by compiling both forms for the
//!    server target across regular attributes, `style:` directives, component
//!    props and literal expressions — byte-identical output every time).
//!
//! ## Soundness
//!
//! Both rules only ever make the model *more* permissive, and each mirrors a
//! reduction the authoritative arm already performs — so neither can report a
//! false equivalence the compile arm would not also report. Where the AST cannot
//! see what the baked HTML would (a block element reached through an `{#if}` /
//! `{#each}` branch is not visible as a sibling), the model stays conservative
//! and keeps the whitespace: over-flagging is a loud failure, under-flagging
//! would be a silent one.

use serde_json::Value;

use crate::fixtures::remove_locations;
use crate::render_normalize::{
    TrimEnd, for_each_fragment, is_empty_text, is_text, render_normalize, trim_text,
};

/// Block-level tags for the **browser render** model.
///
/// ⚠️ This is NOT `tsv_html::is_block_element`, which is a *formatting* set
/// (prettier's inline/block split — it carries `details`/`dialog`/`hgroup`/
/// `menu`/`pre` and deliberately treats table cells as inline). This set must
/// mirror the sidecar's `BLOCK_TAGS`, or the fallback arm and the authoritative
/// compile arm would disagree about the same document. `sidecar_block_tags_match`
/// asserts that agreement against the embedded `sidecar.ts` source.
const BLOCK_TAGS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "div",
    "dl",
    "dt",
    "dd",
    "fieldset",
    "figure",
    "figcaption",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "main",
    "nav",
    "ol",
    "p",
    "section",
    "table",
    "thead",
    "tbody",
    "tr",
    "td",
    "th",
    "ul",
];

/// Return `value` reduced to what a browser actually renders: the Svelte 5
/// compiler model ([`render_normalize`]) followed by this module's two browser
/// rules.
#[must_use]
pub fn browser_render_normalize(value: Value) -> Value {
    let mut value = render_normalize(value);
    // Rule 1. Inside `<pre>` / `<textarea>` whitespace is verbatim, so the
    // block-boundary trim is suspended exactly where the compiler model's is.
    for_each_fragment(&mut value, false, &mut |nodes, preserve| {
        if !preserve {
            trim_fragment_at_block_boundaries(nodes);
        }
    });
    unwrap_single_expression_values(&mut value);
    value
}

/// The fallback arm's counterpart to [`crate::render_normalize::normalize_pair`]:
/// browser-normalize both sides, then strip locations.
#[must_use]
pub fn browser_normalize_pair(a: Value, b: Value) -> (Value, Value) {
    (
        remove_locations(browser_render_normalize(a)),
        remove_locations(browser_render_normalize(b)),
    )
}

/// Rule 1. Trim one fragment's node list at its block boundaries.
///
/// [`render_normalize`] has already collapsed every whitespace run to a single
/// space and trimmed the content boundaries, so the only whitespace left to
/// remove here is a single leading/trailing `' '` on a `Text` node that abuts a
/// block-level sibling. This reproduces the sidecar's segment split: whitespace
/// at either end of a flow segment is dropped, whitespace inside one is kept.
fn trim_fragment_at_block_boundaries(nodes: &mut Vec<Value>) {
    let block: Vec<bool> = nodes.iter().map(is_block_element).collect();

    for i in 0..nodes.len() {
        if !is_text(&nodes[i]) {
            continue;
        }
        let after_block = i > 0 && block[i - 1];
        let before_block = i + 1 < block.len() && block[i + 1];
        if after_block {
            trim_text(&mut nodes[i], TrimEnd::Start);
        }
        if before_block {
            trim_text(&mut nodes[i], TrimEnd::End);
        }
    }

    // A `Text` node emptied by the trim carried nothing but block-boundary
    // whitespace, so it is not render at all.
    nodes.retain(|node| !is_empty_text(node));
}

/// True when this node is a plain HTML element whose tag renders as block-level.
///
/// Only `RegularElement` qualifies: a component, a `<svelte:element>` (dynamic
/// tag) or a block (`{#if}` / `{#each}`) may well *contain* a block element, but
/// the AST cannot see through it the way the baked HTML can — so it is treated
/// as inline and its adjacent whitespace is kept. That is the conservative
/// direction (see the module docs' soundness note).
fn is_block_element(node: &Value) -> bool {
    node.get("type").and_then(Value::as_str) == Some("RegularElement")
        && node
            .get("name")
            .and_then(Value::as_str)
            // Case-insensitive, matching the sidecar's `tag.toLowerCase()`.
            .is_some_and(|name| BLOCK_TAGS.iter().any(|tag| tag.eq_ignore_ascii_case(name)))
}

/// Rule 2. Rewrite `value: [ExpressionTag]` to the bare `value: ExpressionTag`,
/// so a quoted single-expression attribute compares equal to its bare spelling.
///
/// Scoped to the two node types that carry the two-shape `value` — `Attribute`
/// and `StyleDirective`. Every other directive (`bind:` / `class:` / `on:` /
/// `use:` / `transition:` / `animate:`) parses to the same `expression` field
/// quoted or bare, so there is nothing to normalize there. A multi-chunk value
/// (`a="{x}{y}"`, `a="t{x}"`) keeps its array: the concatenation is real.
fn unwrap_single_expression_values(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let two_shaped = matches!(
                map.get("type").and_then(Value::as_str),
                Some("Attribute" | "StyleDirective")
            );
            if two_shaped
                && let Some(Value::Array(chunks)) = map.get("value")
                && let [only] = chunks.as_slice()
                && only.get("type").and_then(Value::as_str) == Some("ExpressionTag")
            {
                let unwrapped = only.clone();
                map.insert("value".to_string(), unwrapped);
            }

            for v in map.values_mut() {
                unwrap_single_expression_values(v);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                unwrap_single_expression_values(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn text(data: &str) -> Value {
        json!({"type": "Text", "raw": data, "data": data})
    }

    fn element(name: &str, nodes: Vec<Value>) -> Value {
        json!({
            "type": "RegularElement",
            "name": name,
            "attributes": [],
            "fragment": {"type": "Fragment", "nodes": nodes},
        })
    }

    fn root(nodes: Vec<Value>) -> Value {
        json!({"type": "Root", "fragment": {"type": "Fragment", "nodes": nodes}})
    }

    fn attribute(name: &str, value: Value) -> Value {
        json!({"type": "Attribute", "name": name, "value": value})
    }

    fn expression_tag(name: &str) -> Value {
        json!({
            "type": "ExpressionTag",
            "expression": {"type": "Identifier", "name": name},
        })
    }

    /// The entry that this model retires: `<div/> <input/>` renders like
    /// `<div/><input/>` because `<div>` is block-level.
    #[test]
    fn whitespace_against_a_block_element_is_dropped() {
        let spaced = browser_render_normalize(root(vec![
            element("div", vec![]),
            text("\n"),
            element("input", vec![]),
        ]));
        let compact =
            browser_render_normalize(root(vec![element("div", vec![]), element("input", vec![])]));
        assert_eq!(spaced, compact);
    }

    /// The rule is a *boundary* rule, not a blanket collapse: between two inline
    /// elements the space is visible render and must stay.
    #[test]
    fn whitespace_between_inline_elements_is_kept() {
        let spaced = browser_render_normalize(root(vec![
            element("span", vec![text("a")]),
            text(" "),
            element("span", vec![text("b")]),
        ]));
        let compact = browser_render_normalize(root(vec![
            element("span", vec![text("a")]),
            element("span", vec![text("b")]),
        ]));
        assert_ne!(spaced, compact);
    }

    /// Only the boundary space goes — text against a block keeps its content.
    #[test]
    fn block_boundary_trim_keeps_text_content() {
        let normalized = browser_render_normalize(root(vec![
            element("div", vec![]),
            text(" tail "),
            element("div", vec![]),
        ]));
        let nodes = normalized["fragment"]["nodes"].as_array().unwrap();
        assert_eq!(nodes[1]["data"], "tail");
    }

    /// A `<pre>` ancestor suspends the rule, exactly as it suspends the compiler
    /// model beneath it.
    #[test]
    fn pre_content_is_not_trimmed_at_block_boundaries() {
        let a = browser_render_normalize(root(vec![element(
            "pre",
            vec![element("div", vec![]), text("  x"), element("div", vec![])],
        )]));
        let b = browser_render_normalize(root(vec![element(
            "pre",
            vec![element("div", vec![]), text("x"), element("div", vec![])],
        )]));
        assert_ne!(a, b);
    }

    /// A block element reached through a block is invisible to the AST, so the
    /// whitespace is conservatively kept rather than wrongly dropped.
    #[test]
    fn whitespace_against_a_non_element_sibling_is_kept() {
        let spaced = browser_render_normalize(root(vec![
            json!({"type": "IfBlock", "test": {}, "consequent": {"type": "Fragment", "nodes": []}}),
            text(" "),
            element("span", vec![text("a")]),
        ]));
        let compact = browser_render_normalize(root(vec![
            json!({"type": "IfBlock", "test": {}, "consequent": {"type": "Fragment", "nodes": []}}),
            element("span", vec![text("a")]),
        ]));
        assert_ne!(spaced, compact);
    }

    /// The other entry this model retires: `a="{x}"` compares equal to `a={x}`.
    #[test]
    fn quoted_single_expression_value_equals_bare() {
        let quoted = browser_render_normalize(root(vec![json!({
            "type": "RegularElement",
            "name": "div",
            "attributes": [attribute("a", json!([expression_tag("x")]))],
            "fragment": {"type": "Fragment", "nodes": []},
        })]));
        let bare = browser_render_normalize(root(vec![json!({
            "type": "RegularElement",
            "name": "div",
            "attributes": [attribute("a", expression_tag("x"))],
            "fragment": {"type": "Fragment", "nodes": []},
        })]));
        assert_eq!(quoted, bare);
    }

    /// A multi-chunk value is a real concatenation — it keeps its array.
    #[test]
    fn multi_chunk_value_is_not_unwrapped() {
        let normalized = browser_render_normalize(root(vec![json!({
            "type": "RegularElement",
            "name": "div",
            "attributes": [attribute("a", json!([expression_tag("x"), expression_tag("y")]))],
            "fragment": {"type": "Fragment", "nodes": []},
        })]));
        let value = &normalized["fragment"]["nodes"][0]["attributes"][0]["value"];
        assert!(value.is_array(), "a two-chunk value must stay an array");
    }

    /// A quoted *text* value is not an `ExpressionTag`, so it is left alone.
    #[test]
    fn text_value_is_not_unwrapped() {
        let normalized = browser_render_normalize(root(vec![json!({
            "type": "RegularElement",
            "name": "div",
            "attributes": [attribute("a", json!([{"type": "Text", "raw": "t", "data": "t"}]))],
            "fragment": {"type": "Fragment", "nodes": []},
        })]));
        let value = &normalized["fragment"]["nodes"][0]["attributes"][0]["value"];
        assert!(value.is_array(), "a text value must stay an array");
    }

    /// [`BLOCK_TAGS`] must stay identical to the sidecar's own set, or the
    /// fallback arm and the authoritative compile arm would model the same
    /// document differently. Read from the embedded `sidecar.ts` source, so the
    /// two cannot drift apart silently.
    #[test]
    fn sidecar_block_tags_match() {
        let source = crate::deno::SIDECAR_SCRIPT;
        let start = source
            .find("const BLOCK_TAGS = new Set([")
            .expect("sidecar.ts must declare `const BLOCK_TAGS = new Set([`");
        let body = &source[start..];
        let end = body
            .find("]);")
            .expect("BLOCK_TAGS must be closed by `]);`");

        let sidecar: std::collections::BTreeSet<&str> =
            body[..end].split('\'').skip(1).step_by(2).collect();
        let ours: std::collections::BTreeSet<&str> = BLOCK_TAGS.iter().copied().collect();

        assert_eq!(
            ours, sidecar,
            "BLOCK_TAGS must mirror the sidecar's browser-render set"
        );
    }
}
