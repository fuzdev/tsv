//! Render-aware AST normalization (Svelte 5 whitespace model).
//!
//! Svelte's parser keeps boundary/inter-node whitespace **verbatim** in `Text`
//! nodes, but the Svelte 5 *compiler* trims it at render time. So a plain
//! `ast_diff` (parse equivalence) flags two sources that render identically —
//! e.g. `<small>text</small>` vs the block-style `<small>⏎\ttext⏎</small>` —
//! as different, even though they are render-equivalent.
//!
//! This module applies the Svelte 5 whitespace rules to a parsed Svelte AST
//! (`serde_json::Value`) so render-equivalent forms compare equal. It is the
//! "render-aware safety check" behind `ast_diff --render`: it lets us confirm
//! block-style inline content is render-equivalent at corpus scale.
//!
//! The Svelte 5 model (see `tsv` root CLAUDE.md and the inline-layout lore):
//!
//! - whitespace **between** nodes collapses to a single space (presence is
//!   significant, kind — space vs newline — is not);
//! - whitespace at the **start and end of an element's content** is removed
//!   completely;
//! - exceptions: `<pre>` / `<textarea>` (`tsv_html::preserves_whitespace`),
//!   inside which whitespace is verbatim.
//!
//! Only **ASCII** whitespace (space, tab, LF, CR, FF) collapses; U+00A0
//! (`&nbsp;`) is significant and is preserved, matching Svelte/HTML.
//!
//! ## Soundness
//!
//! The normalization is exactly the set of transformations Svelte 5 applies,
//! so two ASTs that normalize equal really do render equal — the check never
//! reports a false equivalence for the whitespace question. It is intentionally
//! scoped to `Fragment` node lists (template content); attribute values and JS
//! expressions are never touched.

use serde_json::Value;

/// Return `value` with Svelte 5 render-time whitespace normalization applied to
/// every template `Fragment`. Pairs with [`crate::fixtures::remove_locations`]
/// for a render-equivalence AST comparison.
#[must_use]
pub fn render_normalize(mut value: Value) -> Value {
    normalize_node(&mut value, false);
    value
}

/// `preserve` = whether we are inside a whitespace-preserving element
/// (`<pre>` / `<textarea>`), in which case `Fragment` content is left verbatim.
fn normalize_node(value: &mut Value, preserve: bool) {
    match value {
        Value::Object(map) => {
            // An element whose tag preserves whitespace flips the context for
            // its descendants (the `fragment` it owns and everything below).
            let child_preserve = preserve || node_preserves_whitespace(map);

            // A `Fragment` is a child of its element, so its node list is
            // normalized under the context the *parent element* established —
            // i.e. the `preserve` flag passed into this call, not
            // `child_preserve` (a Fragment has no tag name of its own).
            if map.get("type").and_then(Value::as_str) == Some("Fragment")
                && let Some(Value::Array(nodes)) = map.get_mut("nodes")
            {
                normalize_fragment_nodes(nodes, preserve);
            }

            for v in map.values_mut() {
                normalize_node(v, child_preserve);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                normalize_node(v, preserve);
            }
        }
        _ => {}
    }
}

/// True when this node is an element whose tag name preserves whitespace.
fn node_preserves_whitespace(map: &serde_json::Map<String, Value>) -> bool {
    map.get("name")
        .and_then(Value::as_str)
        .is_some_and(tsv_html::preserves_whitespace)
}

/// Apply collapse + content-boundary trim to one fragment's node list.
fn normalize_fragment_nodes(nodes: &mut Vec<Value>, preserve: bool) {
    if preserve || nodes.is_empty() {
        return;
    }

    // 1. Collapse each Text node's whitespace runs to a single space.
    for node in nodes.iter_mut() {
        if is_text(node) {
            collapse_text_ws(node);
        }
    }

    // 2. Trim content-boundary whitespace: leading on the first node, trailing
    //    on the last node (the same node when there is exactly one). After
    //    collapse, any boundary whitespace is a single ASCII space.
    if is_text(&nodes[0]) {
        trim_text(&mut nodes[0], TrimEnd::Start);
    }
    let last = nodes.len() - 1;
    if is_text(&nodes[last]) {
        trim_text(&mut nodes[last], TrimEnd::End);
    }

    // 3. Drop Text nodes emptied by the boundary trim (pure-boundary
    //    whitespace). A significant inter-sibling space (a mid-list " ") stays.
    nodes.retain(|node| !is_empty_text(node));
}

fn is_text(node: &Value) -> bool {
    node.get("type").and_then(Value::as_str) == Some("Text")
}

fn is_empty_text(node: &Value) -> bool {
    is_text(node) && node.get("data").and_then(Value::as_str) == Some("")
}

/// Collapse ASCII-whitespace runs to a single space in a Text node's `data` and
/// `raw`. Both are treated identically so they stay consistent and non-whitespace
/// differences (e.g. entity encoding in `raw`) still surface in the diff.
fn collapse_text_ws(node: &mut Value) {
    for key in ["data", "raw"] {
        if let Some(Value::String(s)) = node.get_mut(key) {
            *s = collapse_ws(s);
        }
    }
}

enum TrimEnd {
    Start,
    End,
}

fn trim_text(node: &mut Value, which: TrimEnd) {
    for key in ["data", "raw"] {
        if let Some(Value::String(s)) = node.get_mut(key) {
            // After collapse the only boundary whitespace is an ASCII space;
            // strip *only* ' ' so a leading/trailing U+00A0 is preserved.
            let trimmed = match which {
                TrimEnd::Start => s.trim_start_matches(' '),
                TrimEnd::End => s.trim_end_matches(' '),
            };
            if trimmed.len() != s.len() {
                *s = trimmed.to_string();
            }
        }
    }
}

/// Collapse every run of ASCII whitespace (space, tab, LF, CR, FF) to a single
/// space. U+00A0 and other non-ASCII whitespace are left intact (significant).
fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for ch in s.chars() {
        if is_ascii_html_ws(ch) {
            in_ws = true;
        } else {
            if in_ws {
                out.push(' ');
                in_ws = false;
            }
            out.push(ch);
        }
    }
    if in_ws {
        out.push(' ');
    }
    out
}

fn is_ascii_html_ws(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{0C}')
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

    /// Extract a fragment's node list for assertions.
    fn frag_nodes(v: &Value) -> &Vec<Value> {
        v["fragment"]["nodes"].as_array().unwrap()
    }

    #[test]
    fn block_style_content_equals_flowed_content() {
        // <small>⏎\tword word⏎</small> renders the same as <small>word word</small>.
        let block = render_normalize(root(vec![element("small", vec![text("\n\tword word\n")])]));
        let flowed = render_normalize(root(vec![element("small", vec![text("word word")])]));
        assert_eq!(block, flowed);
    }

    #[test]
    fn internal_whitespace_runs_collapse() {
        // Multiple spaces / a newline+indent between words → one space.
        let a = render_normalize(root(vec![element(
            "small",
            vec![text("word   word\n\t\tword")],
        )]));
        let b = render_normalize(root(vec![element("small", vec![text("word word word")])]));
        assert_eq!(a, b);
    }

    #[test]
    fn inter_sibling_space_and_newline_are_equivalent() {
        // </strong> tail  ==  </strong>⏎tail  (both render one space).
        let with_space = render_normalize(root(vec![element(
            "p",
            vec![element("strong", vec![text("x")]), text(" tail\n")],
        )]));
        let with_newline = render_normalize(root(vec![element(
            "p",
            vec![element("strong", vec![text("x")]), text("\ntail\n")],
        )]));
        assert_eq!(with_space, with_newline);
    }

    #[test]
    fn inter_sibling_presence_is_significant() {
        // </strong> tail  !=  </strong>tail  (presence of the space matters).
        let with_space = render_normalize(root(vec![element(
            "p",
            vec![element("strong", vec![text("x")]), text(" tail")],
        )]));
        let without_space = render_normalize(root(vec![element(
            "p",
            vec![element("strong", vec![text("x")]), text("tail")],
        )]));
        assert_ne!(with_space, without_space);
    }

    #[test]
    fn leading_boundary_whitespace_node_is_dropped() {
        // <p>⏎\t<strong>…</strong></p>: the leading whitespace-only Text is removed.
        let normalized = render_normalize(root(vec![element(
            "p",
            vec![text("\n\t"), element("strong", vec![text("x")])],
        )]));
        let p = &frag_nodes(&normalized)[0];
        let p_children = frag_nodes(p);
        assert_eq!(
            p_children.len(),
            1,
            "leading whitespace Text should be dropped"
        );
        assert_eq!(p_children[0]["name"], "strong");
    }

    #[test]
    fn nbsp_is_not_collapsed() {
        // &nbsp; (U+00A0) is significant: "a\u{a0}b" must not normalize to "a b".
        let nbsp = render_normalize(root(vec![element("small", vec![text("a\u{a0}b")])]));
        let space = render_normalize(root(vec![element("small", vec![text("a b")])]));
        assert_ne!(nbsp, space);
        // And the nbsp survives unchanged.
        let small = &frag_nodes(&nbsp)[0];
        assert_eq!(frag_nodes(small)[0]["data"], "a\u{a0}b");
    }

    #[test]
    fn pre_preserves_whitespace() {
        // Inside <pre>, two differently-spaced contents stay distinct.
        let a = render_normalize(root(vec![element("pre", vec![text("x   y")])]));
        let b = render_normalize(root(vec![element("pre", vec![text("x y")])]));
        assert_ne!(a, b);
    }

    #[test]
    fn textarea_preserves_nested_whitespace() {
        // The preserve context propagates to descendants of <textarea>.
        let a = render_normalize(root(vec![element(
            "textarea",
            vec![element("span", vec![text("x   y")])],
        )]));
        let b = render_normalize(root(vec![element(
            "textarea",
            vec![element("span", vec![text("x y")])],
        )]));
        assert_ne!(a, b);
    }
}
