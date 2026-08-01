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
/// Membership asks one question — does this tag's **UA-stylesheet display**
/// (`~/dev/html/source`, rendering section) make adjacent collapsible whitespace
/// non-rendering? — and it must be answered UNCONDITIONALLY. That is a different
/// question from `tsv_html::is_block_element`, the *formatting* set, and the two
/// differ in **both** directions rather than one being a subset of the other:
///
/// - Here but not there: `thead`/`tbody`/`tr`/`td`/`th`. Their display is
///   table-internal, so the whitespace does vanish, but prettier lays table cells
///   out as inline and the formatting set follows prettier.
/// - There but not here: `dialog`. `dialog:not([open]) { display: none; }`
///   precedes `dialog { … display: block }`, so the attribute-less spelling — the
///   default authoring — generates no box at all. Its neighbours then share ONE
///   inline formatting context and the surrounding whitespace collapses to a
///   RENDERED space: `a<dialog>x</dialog>b` paints `ab`, `a <dialog>x</dialog> b`
///   paints `a b`. Admitting it would make this model vouch a false equivalence,
///   the one direction the soundness note above forbids — so the model keeps
///   flagging a formatter break there, and that verdict is correct. (Prettier core
///   reaches the opposite answer only because its generator drops every
///   non-bare-tag selector, `dialog:not([open])` included.)
///
/// The five tags that ARE shared with the formatting set — `details`, `hgroup`,
/// `menu`, `pre`, `summary` — are unconditionally block there, and they are also
/// the whole of this oracle's exposure: a break can only ever appear at a
/// `tsv_html` block element, so any tag outside that set costs nothing to omit and
/// omitting it keeps the sensitivity.
///
/// TODO: neither arm models `display: none` at all, so a closed `<dialog>`'s
/// CONTENT still counts as flow text here — which is why the boundary shape above
/// is the one that bites, while `<div><dialog>x</dialog>text</div>` over-flags for
/// an unrelated reason. Modelling it properly is a bigger question than one tag
/// (`[hidden]`, `<template>`, `<datalist>`, a non-selected `<option>`); until then
/// the omission keeps this tag in the loud direction, which is the safe one.
///
/// ⚠️ This set must mirror the sidecar's `BLOCK_TAGS`, or the fallback arm and the
/// authoritative compile arm would disagree about the same document.
/// `sidecar_block_tags_match` asserts that agreement against the embedded
/// `sidecar.ts` source.
const BLOCK_TAGS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "details",
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
    "hgroup",
    "hr",
    "li",
    "main",
    "menu",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "summary",
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
    // `pre` being a block tag is about the OUTSIDE — whitespace against a `<pre>`
    // sibling, which the sidecar drops via its separate verbatim-run split.
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
    use crate::render_normalize::test_ast::*;
    use serde_json::json;

    fn attribute(name: &str, value: Value) -> Value {
        json!({"type": "Attribute", "name": name, "value": value})
    }

    fn expression_tag(name: &str) -> Value {
        json!({
            "type": "ExpressionTag",
            "expression": {"type": "Identifier", "name": name},
        })
    }

    /// The `value` of the first attribute of the root's first element.
    fn attribute_value(normalized: &Value) -> &Value {
        &frag_nodes(normalized)[0]["attributes"][0]["value"]
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

    /// Format both spellings of a one-element sandwich, so a tag's block-ness is
    /// read off the only thing that distinguishes them: the boundary whitespace.
    fn sandwich_pair(tag: &str) -> (Value, Value) {
        let glued = browser_render_normalize(root(vec![
            text("a"),
            element(tag, vec![text("x")]),
            text("b"),
        ]));
        let spaced = browser_render_normalize(root(vec![
            text("a "),
            element(tag, vec![text("x")]),
            text(" b"),
        ]));
        (glued, spaced)
    }

    /// The five tags the formatting set carries that this one lacked. Each is
    /// UNCONDITIONALLY `display: block` in the UA stylesheet, so a formatter break
    /// at its boundary is not a render change — and since a break can only appear
    /// at a `tsv_html` block element, these five were the whole of the gap.
    #[test]
    fn unconditionally_block_tags_drop_boundary_whitespace() {
        for tag in ["details", "hgroup", "menu", "pre", "summary"] {
            let (glued, spaced) = sandwich_pair(tag);
            assert_eq!(
                glued, spaced,
                "<{tag}> is display:block in the UA stylesheet, so the whitespace \
                 against it is not render"
            );
        }
    }

    /// ⚠️ `dialog` is deliberately NOT a member, though `tsv_html` classifies it
    /// block for formatting. `dialog:not([open]) { display: none; }` means the
    /// attribute-less spelling generates no box, so its neighbours share one inline
    /// formatting context and the boundary whitespace collapses to a RENDERED
    /// space. Admitting it would buy a false equivalence — an under-flag, the
    /// silent failure. This pins the omission against a "finish the set" cleanup.
    #[test]
    fn dialog_keeps_its_boundary_whitespace() {
        let (glued, spaced) = sandwich_pair("dialog");
        assert_ne!(
            glued, spaced,
            "a closed <dialog> is display:none, so `a<dialog>x</dialog>b` and its \
             spaced twin render differently — this model must keep flagging it"
        );
    }

    /// The counterpart control: an inline tag's boundary whitespace is a visible
    /// space, and widening the block set must not have touched that.
    #[test]
    fn inline_tag_keeps_its_boundary_whitespace() {
        let (glued, spaced) = sandwich_pair("span");
        assert_ne!(glued, spaced, "a space against <span> is visible render");
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
        let quoted = browser_render_normalize(root(vec![element_with_attributes(
            "div",
            vec![attribute("a", json!([expression_tag("x")]))],
            vec![],
        )]));
        let bare = browser_render_normalize(root(vec![element_with_attributes(
            "div",
            vec![attribute("a", expression_tag("x"))],
            vec![],
        )]));
        assert_eq!(quoted, bare);
    }

    /// A multi-chunk value is a real concatenation — it keeps its array.
    #[test]
    fn multi_chunk_value_is_not_unwrapped() {
        let normalized = browser_render_normalize(root(vec![element_with_attributes(
            "div",
            vec![attribute(
                "a",
                json!([expression_tag("x"), expression_tag("y")]),
            )],
            vec![],
        )]));
        assert!(
            attribute_value(&normalized).is_array(),
            "a two-chunk value must stay an array"
        );
    }

    /// A quoted *text* value is not an `ExpressionTag`, so it is left alone.
    #[test]
    fn text_value_is_not_unwrapped() {
        let normalized = browser_render_normalize(root(vec![element_with_attributes(
            "div",
            vec![attribute("a", json!([text("t")]))],
            vec![],
        )]));
        assert!(
            attribute_value(&normalized).is_array(),
            "a text value must stay an array"
        );
    }

    /// The render key models a `${…}` hole / template-chunk seam as a
    /// NON-WHITESPACE sentinel. `HOLE = ' '` once merged holes with adjacent
    /// whitespace, so the key vouched "same page" for real render changes
    /// (`{x}{z}` vs `{x} {z}` bakes a rendered space into the template;
    /// `a{@render f()}b` vs its spaced form likewise) — the exact class the
    /// oracle exists to refuse. Read from the embedded `sidecar.ts` source so a
    /// "cleanup" back to a space (or to `\x00`, which would collide with the
    /// block-boundary sentinel `BR`) cannot land silently.
    #[test]
    fn sidecar_hole_is_a_non_whitespace_sentinel() {
        let source = crate::deno::SIDECAR_SCRIPT;
        let needle = "const HOLE = '";
        let start = source
            .find(needle)
            .expect("sidecar.ts must declare `const HOLE = '…'`");
        let rest = &source[start + needle.len()..];
        let end = rest.find('\'').expect("HOLE literal must close");
        let literal = &rest[..end];

        // Decode the one-character TS string literal (the source text carries the
        // ESCAPE, e.g. `\x01`, not the control character itself).
        let decoded: char = if let Some(hex) = literal.strip_prefix("\\x") {
            char::from(u8::from_str_radix(hex, 16).expect("valid \\xNN escape in HOLE"))
        } else {
            match literal {
                "\\t" => '\t',
                "\\n" => '\n',
                "\\r" => '\r',
                "\\f" => '\u{c}',
                "\\0" => '\0',
                other => {
                    let mut chars = other.chars();
                    let c = chars.next().expect("HOLE must not be empty");
                    assert!(chars.next().is_none(), "HOLE must be one character");
                    c
                }
            }
        };
        assert!(
            !decoded.is_whitespace() && decoded != '\0',
            "HOLE must be a non-whitespace, non-NUL sentinel — got {decoded:?}: a whitespace \
             hole merges with adjacent runs and blinds the render key, and NUL collides with BR"
        );
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

        // Odd-indexed `'`-split segments are the quoted tag names.
        let sidecar: std::collections::BTreeSet<&str> =
            body[..end].split('\'').skip(1).step_by(2).collect();
        // Distinguish "the two sets disagree" from "this parse stopped working"
        // (e.g. the sidecar list reformatted to double quotes) — otherwise the
        // latter reports as a bogus mismatch against an empty set.
        assert!(
            !sidecar.is_empty(),
            "failed to parse any tag out of the sidecar's BLOCK_TAGS — \
             single-quoted entries expected; update this parse, not the assertion"
        );

        let ours: std::collections::BTreeSet<&str> = BLOCK_TAGS.iter().copied().collect();
        assert_eq!(
            ours, sidecar,
            "BLOCK_TAGS must mirror the sidecar's browser-render set"
        );
    }
}
