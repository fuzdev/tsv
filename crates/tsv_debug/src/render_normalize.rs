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
//! The Svelte 5 model (see `tsv` root CLAUDE.md):
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

use crate::fixtures::remove_locations;

/// Return `value` with Svelte 5 render-time whitespace normalization applied to
/// every template `Fragment`. Pairs with [`crate::fixtures::remove_locations`]
/// for a render-equivalence AST comparison.
#[must_use]
pub fn render_normalize(mut value: Value) -> Value {
    for_each_fragment(&mut value, false, &mut normalize_fragment_nodes);
    value
}

/// Apply the shared AST-comparison prep to a pair: Svelte-5 render-time
/// whitespace normalization (when `render`) followed by location stripping.
///
/// `ast_diff`'s exact-equality compare builds on this pair, and so does the AST diff the
/// round-trip audits PRINT for a divergence. The skeleton VERDICT no longer does:
/// [`skeletons_equal`] folds both rewrites into its walk, so the pair is built only where its
/// two trees are the thing wanted.
#[must_use]
pub fn normalize_pair(a: Value, b: Value, render: bool) -> (Value, Value) {
    let (a, b) = if render {
        (render_normalize(a), render_normalize(b))
    } else {
        (a, b)
    };
    (remove_locations(a), remove_locations(b))
}

/// Reduce an AST to its structure: preserve object keys, array lengths, nesting,
/// and each node's `type` discriminator; erase every other leaf scalar. Two ASTs
/// with equal skeletons differ only in reformattable leaf content, not shape.
///
/// The acorn/acorn-typescript **`extra`** metadata bag is dropped entirely (on
/// both sides) rather than erased: it records source-formatting artifacts —
/// trailing-comma presence (which tsv's `trailingComma: 'none'` removes),
/// `parenthesized` / `parenStart`, `raw` — so its *key presence* itself flips
/// under formatting and would otherwise read as a shape change.
///
/// **Comment attachment** — a node's `leadingComments` / `trailingComments`, which
/// Svelte's parser attaches to the nearest node (acorn-typescript attaches none, and
/// tsv's wire mirrors each) — is dropped the same way. Where a comment ATTACHES is a
/// placement record, not shape: a same-line block comment before a statement's `;`
/// legitimately trails past it (`a /* c */;` → `a; /* c */`, tsv's terminator-gap rule
/// and prettier's), re-attaching from the expression to the statement with no node
/// added, lost or re-typed. The comment itself stays pinned: the root `comments` array
/// keeps its length and each entry's `type` here, and its text is a conserved leaf
/// (`leaf_conservation_diff`), so a dropped, doubled or re-kinded comment still reads as
/// a divergence — only its attachment does not. A comment whose placement IS semantic
/// (a bundler annotation crossing a synthesized paren) is invisible to this skeleton
/// either way, since a grouping paren is not a node: that is `binding_audit`'s question.
///
/// Used by `roundtrip_audit`'s corruption hunt (a re-quoted `attr='a"b'` →
/// `attr="a"b"` reparses to two attributes, an array-length change the skeleton
/// catches while ignoring the legitimate leaf-content reformatting around it).
///
/// This is the **definition**, and the oracle its tests grade against. What production asks is
/// [`skeletons_equal`], which reaches the same verdict on a pair without materializing either
/// skeleton — every consumer of this rule compares two documents and throws the trees away.
#[must_use]
pub fn structural_skeleton(v: &Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, val) in map {
                if k == "extra" || k == "leadingComments" || k == "trailingComments" {
                    // Parser source-metadata / comment attachment — omit the key on both
                    // sides (see above).
                    continue;
                }
                // `type` is the node discriminator — a change (Attribute →
                // SpreadAttribute, …) is structural, so keep its value.
                if k == "type" && val.is_string() {
                    out.insert(k.clone(), val.clone());
                } else {
                    out.insert(k.clone(), structural_skeleton(val));
                }
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(structural_skeleton).collect()),
        // Erase scalar leaves — reformattable source content.
        _ => Value::Null,
    }
}

/// Do `a` and `b` reduce to the SAME [structural skeleton](structural_skeleton) under
/// [`normalize_pair`] — that verdict, without building any of the four trees that spell it.
///
/// `structural_skeleton(remove_locations(render_normalize(a))) == …(b)` is a question about two
/// documents, and every tree it names is discarded the moment it is answered: the two normalized
/// clones, and the two skeletons built only to be compared. On a per-injection audit that is the
/// most expensive thing on the page — the skeleton pair alone rebuilds both trees with a fresh
/// `String` per key, for one `bool`. This walks the two inputs together and answers the same
/// question with no allocation at all. [`structural_skeleton`] stays the DEFINITION (and this
/// walk's differential test oracle); this is what production asks.
///
/// Each rewrite the pipeline names is folded in where it is decided:
///
/// - **render normalization** ([`normalize_fragment_nodes`]) rewrites a `Text`'s `data` / `raw`
///   and then drops the nodes the boundary trim emptied. The skeleton erases every scalar, so
///   only the DROP is observable — folded in as [`fragment_node_dropped`], a predicate on a
///   node's position in its fragment. Each side carries its own whitespace-`preserve` context,
///   because the tag `name` that flips it is itself a scalar the skeleton erases: two documents
///   can normalize under different contexts and still be skeleton-equal.
/// - **location stripping** ([`remove_locations`]) and the skeleton's own metadata drops are one
///   key filter, [`skeleton_skips_key`].
/// - **scalar erasure** makes every leaf pair equal, whatever it held — except a `type`, the
///   node discriminator, whose string value is compared.
///
/// Objects compare by KEY, never by iteration order: with `serde_json`'s `preserve_order` a `Map`
/// is an `IndexMap`, whose equality is order-free, and field order on the wire is the writer's
/// property, not the shape's.
#[must_use]
pub fn skeletons_equal(a: &Value, b: &Value, render: bool) -> bool {
    skeleton_eq(a, b, render, false, false)
}

/// The keys [`skeletons_equal`] drops: [`remove_locations`]' three positions, then
/// [`structural_skeleton`]'s own three metadata bags.
fn skeleton_skips_key(key: &str) -> bool {
    matches!(
        key,
        "start" | "end" | "loc" | "extra" | "leadingComments" | "trailingComments"
    )
}

/// True when this object is a template `Fragment` — the one node whose child list
/// [`normalize_fragment_nodes`] rewrites.
fn is_fragment(map: &serde_json::Map<String, Value>) -> bool {
    map.get("type").and_then(Value::as_str) == Some("Fragment")
}

/// Is the fragment node at `index` (of `len`) one [`normalize_fragment_nodes`] drops — the only
/// part of render normalization the skeleton can see?
///
/// [`collapse_ws`] empties an already-empty string and nothing else; every other all-collapsible
/// `data` becomes exactly one `' '`, which [`trim_text`] then removes — and the trim reaches the
/// first and the last node only (the same node when the list holds one). A `data` holding any
/// non-collapsible character keeps it, so it survives the trim, and a mid-list `" "` is a
/// significant inter-sibling space that stays.
fn fragment_node_dropped(node: &Value, index: usize, len: usize) -> bool {
    if !is_text(node) {
        return false;
    }
    // A non-string `data` is what `collapse_text_ws` and `is_empty_text` both decline to touch.
    let Some(data) = node.get("data").and_then(Value::as_str) else {
        return false;
    };
    data.is_empty() || ((index == 0 || index + 1 == len) && data.chars().all(is_collapsible_ws))
}

/// One step of [`skeletons_equal`], carrying each side's own whitespace-`preserve` context.
fn skeleton_eq(a: &Value, b: &Value, render: bool, preserve_a: bool, preserve_b: bool) -> bool {
    match (a, b) {
        (Value::Object(ma), Value::Object(mb)) => {
            // The flip is keyed on the element, so it is read before any child is compared —
            // and once per side, since the two may disagree (see `skeletons_equal`).
            let child_a = preserve_a || node_preserves_whitespace(ma);
            let child_b = preserve_b || node_preserves_whitespace(mb);
            let live = |m: &serde_json::Map<String, Value>| {
                m.keys().filter(|k| !skeleton_skips_key(k)).count()
            };
            if live(ma) != live(mb) {
                return false;
            }
            // A fragment's own node list is normalized in the context its PARENT established,
            // and `normalize_fragment_nodes` returns untouched under `preserve`.
            let filter_a = render && !preserve_a && is_fragment(ma);
            let filter_b = render && !preserve_b && is_fragment(mb);
            for (key, value_a) in ma {
                if skeleton_skips_key(key) {
                    continue;
                }
                let Some(value_b) = mb.get(key) else {
                    return false;
                };
                if key == "type" {
                    match (value_a.as_str(), value_b.as_str()) {
                        // The discriminator is the one scalar the skeleton keeps.
                        (Some(ta), Some(tb)) => {
                            if ta != tb {
                                return false;
                            }
                            continue;
                        }
                        // One side keeps a string where the other erases or recurses.
                        (Some(_), None) | (None, Some(_)) => return false,
                        // Neither is a string, so neither is kept — an ordinary value.
                        (None, None) => {}
                    }
                }
                if key == "nodes"
                    && (filter_a || filter_b)
                    && let (Value::Array(nodes_a), Value::Array(nodes_b)) = (value_a, value_b)
                {
                    if !fragment_nodes_eq(
                        nodes_a, nodes_b, filter_a, filter_b, render, child_a, child_b,
                    ) {
                        return false;
                    }
                    continue;
                }
                if !skeleton_eq(value_a, value_b, render, child_a, child_b) {
                    return false;
                }
            }
            true
        }
        (Value::Array(items_a), Value::Array(items_b)) => {
            items_a.len() == items_b.len()
                && items_a
                    .iter()
                    .zip(items_b)
                    .all(|(x, y)| skeleton_eq(x, y, render, preserve_a, preserve_b))
        }
        // A container against anything else is a shape change; two scalars both erase to `Null`.
        (Value::Object(_) | Value::Array(_), _) | (_, Value::Object(_) | Value::Array(_)) => false,
        _ => true,
    }
}

/// Compare two fragment node lists with each side's render-normalization drops applied —
/// [`normalize_fragment_nodes`]'s `retain`, read rather than performed.
fn fragment_nodes_eq(
    a: &[Value],
    b: &[Value],
    filter_a: bool,
    filter_b: bool,
    render: bool,
    preserve_a: bool,
    preserve_b: bool,
) -> bool {
    let mut kept_a = a
        .iter()
        .enumerate()
        .filter(|(i, node)| !(filter_a && fragment_node_dropped(node, *i, a.len())));
    let mut kept_b = b
        .iter()
        .enumerate()
        .filter(|(i, node)| !(filter_b && fragment_node_dropped(node, *i, b.len())));
    loop {
        match (kept_a.next(), kept_b.next()) {
            (Some((_, x)), Some((_, y))) => {
                if !skeleton_eq(x, y, render, preserve_a, preserve_b) {
                    return false;
                }
            }
            (None, None) => return true,
            _ => return false,
        }
    }
}

/// Call `f` on every template `Fragment`'s node list, threading the
/// whitespace-**preserve** context (`<pre>` / `<textarea>`, inside which
/// `Fragment` content is left verbatim).
///
/// `pub(crate)` because [`crate::render_browser`] — the browser-model layer
/// above this one — walks the same shape. Two subtleties make this worth
/// sharing rather than re-deriving per layer:
///
/// - A `Fragment` has no tag name of its own, so its node list belongs to the
///   context its *parent element* established: `f` gets the `preserve` passed
///   into this call, while descendants get `child_preserve`.
/// - The flip is keyed on the element, not the fragment, so it must be computed
///   before recursing into *any* of the map's values.
pub(crate) fn for_each_fragment(
    value: &mut Value,
    preserve: bool,
    f: &mut impl FnMut(&mut Vec<Value>, bool),
) {
    match value {
        Value::Object(map) => {
            // An element whose tag preserves whitespace flips the context for
            // its descendants (the `fragment` it owns and everything below).
            let child_preserve = preserve || node_preserves_whitespace(map);

            if map.get("type").and_then(Value::as_str) == Some("Fragment")
                && let Some(Value::Array(nodes)) = map.get_mut("nodes")
            {
                f(nodes, preserve);
            }

            for v in map.values_mut() {
                for_each_fragment(v, child_preserve, f);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                for_each_fragment(v, preserve, f);
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

/// True when this node is a `Text` node. `pub(crate)` for
/// [`crate::render_browser`], the browser-model layer above this one.
pub(crate) fn is_text(node: &Value) -> bool {
    node.get("type").and_then(Value::as_str) == Some("Text")
}

/// True when this node is a `Text` node left empty by a trim — nothing a
/// browser renders, so callers drop it. `pub(crate)`: the browser layer's
/// block-boundary trim empties nodes the same way.
pub(crate) fn is_empty_text(node: &Value) -> bool {
    is_text(node) && node.get("data").and_then(Value::as_str) == Some("")
}

/// Collapse collapsible-whitespace runs to a single space in a Text node's `data` and
/// `raw`. Both are treated identically so they stay consistent and non-whitespace
/// differences (e.g. entity encoding in `raw`) still surface in the diff.
fn collapse_text_ws(node: &mut Value) {
    for key in ["data", "raw"] {
        if let Some(Value::String(s)) = node.get_mut(key) {
            *s = collapse_ws(s);
        }
    }
}

/// Which end of a `Text` node's content a trim applies to.
pub(crate) enum TrimEnd {
    Start,
    End,
}

/// Strip the boundary `' '` from one end of a `Text` node's `data`/`raw`.
///
/// `pub(crate)`: the browser layer trims the identical way at a block boundary,
/// and the "only `' '`, so U+00A0 survives" rule below must not be re-derived
/// per layer.
pub(crate) fn trim_text(node: &mut Value, which: TrimEnd) {
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

/// Collapse every run of [collapsible whitespace](is_collapsible_ws) to a single
/// space. Every other separator — U+00A0, U+202F, the form feed — is left intact
/// (significant).
fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for ch in s.chars() {
        if is_collapsible_ws(ch) {
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

/// The whitespace the Svelte compiler collapses: `[ \t\n\r]`, its own
/// `regex_not_whitespace` = `/[^ \t\r\n]/` read positively. Mirrors
/// `tsv_svelte::ast::internal::is_collapsible_ws_char`.
///
/// ⚠️ **Narrower than the HTML tokenizer's class and than Rust's
/// `is_ascii_whitespace`, both of which include the form feed** — and the difference is
/// a soundness one, not cosmetic. This function decides what the model declares
/// INVISIBLE, so every character it accepts is one two documents may differ by and still
/// compare equal. U+000C is not in CSS Text 3's document white space characters, so the
/// compiler keeps it verbatim: with it in the set, `<b>a</b>␌<b>b</b>` and
/// `<b>a</b> <b>b</b>` normalized to the same tree, and this model vouched "render
/// equivalent" for a formatter respelling a form feed as a space — while the sidecar's
/// authoritative render key (`svelte-render-key`, whose collapse is the same narrow
/// class) correctly called them different pages. Both arms of the fixture
/// render-equivalence gate run through here, so a wide class here reopens the hole the
/// narrow one closed there.
fn is_collapsible_ws(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r')
}

/// Minimal Svelte-AST builders for the whitespace-model tests.
///
/// Shared with [`crate::render_browser`]'s tests: it layers the browser model on
/// top of this one, so its cases are built from the same shapes and would
/// otherwise be a verbatim copy of these.
#[cfg(test)]
pub(crate) mod test_ast {
    use serde_json::{Value, json};

    pub(crate) fn text(data: &str) -> Value {
        json!({"type": "Text", "raw": data, "data": data})
    }

    pub(crate) fn element(name: &str, nodes: Vec<Value>) -> Value {
        element_with_attributes(name, vec![], nodes)
    }

    pub(crate) fn element_with_attributes(
        name: &str,
        attributes: Vec<Value>,
        nodes: Vec<Value>,
    ) -> Value {
        json!({
            "type": "RegularElement",
            "name": name,
            "attributes": attributes,
            "fragment": {"type": "Fragment", "nodes": nodes},
        })
    }

    pub(crate) fn root(nodes: Vec<Value>) -> Value {
        json!({"type": "Root", "fragment": {"type": "Fragment", "nodes": nodes}})
    }

    /// Extract a fragment's node list for assertions.
    pub(crate) fn frag_nodes(v: &Value) -> &Vec<Value> {
        v["fragment"]["nodes"].as_array().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::test_ast::*;
    use super::*;

    #[test]
    fn block_style_content_equals_flowed_content() {
        // <small>⏎\tword word⏎</small> renders the same as <small>word word</small>.
        let block = render_normalize(root(vec![element("small", vec![text("\n\tword word\n")])]));
        let flowed = render_normalize(root(vec![element("small", vec![text("word word")])]));
        assert_eq!(block, flowed);
    }

    /// A form feed is rendered CONTENT, not a collapsible separator — so a document
    /// that spells a separator `␌` is a different document from its space twin, and
    /// this model must say so. With U+000C in [`is_collapsible_ws`] the two normalized
    /// equal, and every consumer of this model (`ast_diff --render`, the fixture
    /// render-equivalence FALLBACK arm) vouched for a formatter that respells one.
    /// The sidecar's authoritative render key already refuses this pair.
    #[test]
    fn form_feed_is_content_not_collapsible_whitespace() {
        let ff = render_normalize(root(vec![element("span", vec![text("a\u{c}b")])]));
        let space = render_normalize(root(vec![element("span", vec![text("a b")])]));
        assert_ne!(ff, space);

        // …and it does not merge with an adjacent collapsible run either.
        let ff_run = render_normalize(root(vec![element("span", vec![text("a \u{c} b")])]));
        assert_ne!(ff_run, space);
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

    /// Comment ATTACHMENT is not shape: the per-node placement arrays are dropped like
    /// `extra`, so a comment that legitimately trails past a `;` reads equal, while the
    /// root `comments` array still pins the count and the kind of every comment.
    #[test]
    fn skeleton_erases_comment_attachment_but_not_the_comment() {
        let comment = json!({"type": "Block", "value": " c "});
        let on_expression = json!({
            "type": "Program",
            "body": [{
                "type": "ExpressionStatement",
                "expression": {"type": "Identifier", "name": "a", "trailingComments": [comment]}
            }],
            "comments": [comment]
        });
        let on_statement = json!({
            "type": "Program",
            "body": [{
                "type": "ExpressionStatement",
                "trailingComments": [comment],
                "expression": {"type": "Identifier", "name": "a"}
            }],
            "comments": [comment]
        });
        assert_eq!(
            structural_skeleton(&on_expression),
            structural_skeleton(&on_statement)
        );

        // A dropped comment still changes the skeleton (the root array's length)…
        let dropped = json!({
            "type": "Program",
            "body": [{"type": "ExpressionStatement", "expression": {"type": "Identifier", "name": "a"}}],
            "comments": []
        });
        assert_ne!(
            structural_skeleton(&on_expression),
            structural_skeleton(&dropped)
        );
        // …and so does a comment whose KIND changed (`type` is kept, not erased).
        let line = json!({"type": "Line", "value": " c "});
        let re_kinded = json!({
            "type": "Program",
            "body": [{"type": "ExpressionStatement", "expression": {"type": "Identifier", "name": "a"}}],
            "comments": [line]
        });
        assert_ne!(
            structural_skeleton(&on_statement),
            structural_skeleton(&re_kinded)
        );
    }

    /// The verdict [`skeletons_equal`] replaces, spelled the long way — the oracle every test
    /// below grades it against, so the fused walk can never quietly drift from its definition.
    fn skeleton_oracle(a: &Value, b: &Value, render: bool) -> bool {
        let (a, b) = normalize_pair(a.clone(), b.clone(), render);
        structural_skeleton(&a) == structural_skeleton(&b)
    }

    /// Both readings of one pair, asserted to agree — and to reach `expected`.
    #[track_caller]
    fn assert_skeletons(a: &Value, b: &Value, render: bool, expected: bool) {
        let oracle = skeleton_oracle(a, b, render);
        assert_eq!(
            oracle, expected,
            "the oracle itself disagrees with the case"
        );
        assert_eq!(
            skeletons_equal(a, b, render),
            oracle,
            "skeletons_equal diverged from structural_skeleton(normalize_pair(..))"
        );
    }

    /// A `loc` bag is not shape: the fused walk skips the key where the pipeline stripped it, so
    /// a wire that carries line/column compares equal to the same document with different ones.
    /// The canonical parsers emit `loc`; tsv's audit wire does not — both reach this walk.
    #[test]
    fn skeletons_equal_ignores_positions() {
        let with_loc = json!({
            "type": "Program",
            "start": 0, "end": 1,
            "loc": {"start": {"line": 1, "column": 0}, "end": {"line": 1, "column": 1}},
            "body": [{"type": "Identifier", "name": "a", "start": 0, "end": 1}]
        });
        let moved = json!({
            "type": "Program",
            "start": 4, "end": 5,
            "loc": {"start": {"line": 3, "column": 2}, "end": {"line": 3, "column": 3}},
            "body": [{"type": "Identifier", "name": "b", "start": 4, "end": 5}]
        });
        let position_free = json!({
            "type": "Program",
            "body": [{"type": "Identifier", "name": "a"}]
        });
        assert_skeletons(&with_loc, &moved, false, true);
        assert_skeletons(&with_loc, &position_free, false, true);
    }

    /// The node discriminator is the one scalar kept, and a key set is shape.
    #[test]
    fn skeletons_equal_reads_type_and_key_set() {
        let identifier = json!({"type": "Identifier", "name": "a"});
        let literal = json!({"type": "Literal", "name": "a"});
        let extra_key = json!({"type": "Identifier", "name": "a", "optional": false});
        assert_skeletons(&identifier, &literal, false, false);
        assert_skeletons(&identifier, &extra_key, false, false);
        // A non-string `type` is erased like any other scalar, so two of them compare equal.
        assert_skeletons(&json!({"type": 1}), &json!({"type": 2}), false, true);
        // …but a kept string against an erased scalar is not.
        assert_skeletons(&json!({"type": "A"}), &json!({"type": 1}), false, false);
        // A container against a scalar is a shape change; two scalars both erase to `Null`.
        assert_skeletons(&json!({"k": []}), &json!({"k": 0}), false, false);
        assert_skeletons(&json!({"k": "x"}), &json!({"k": 7}), false, true);
    }

    /// Field order on the wire is the writer's property, not the shape's — `Map` equality is
    /// order-free, so the walk compares by key.
    #[test]
    fn skeletons_equal_is_key_order_free() {
        let a = json!({"type": "Identifier", "name": "a", "start": 0});
        let b = json!({"start": 9, "name": "a", "type": "Identifier"});
        assert_skeletons(&a, &b, false, true);
    }

    /// Only the boundary-emptied `Text` nodes are dropped, and only under `render`.
    #[test]
    fn skeletons_equal_folds_the_fragment_drop() {
        let padded = root(vec![text("\n\t"), element("b", vec![]), text("\n")]);
        let bare = root(vec![element("b", vec![])]);
        assert_skeletons(&padded, &bare, true, true);
        // Without render normalization the two `Text` nodes are ordinary siblings.
        assert_skeletons(&padded, &bare, false, false);
        // A mid-list space is a significant inter-sibling separator, never dropped.
        let spaced = root(vec![element("b", vec![]), text(" "), element("i", vec![])]);
        let glued = root(vec![element("b", vec![]), element("i", vec![])]);
        assert_skeletons(&spaced, &glued, true, false);
        // A boundary text holding real content survives the trim.
        let worded = root(vec![text(" word "), element("b", vec![])]);
        assert_skeletons(&worded, &bare, true, false);
    }

    /// The `preserve` context is read per side, because the tag `name` that flips it is a scalar
    /// the skeleton erases — so the two documents can normalize under different contexts.
    #[test]
    fn skeletons_equal_reads_preserve_per_side() {
        let in_pre = element("pre", vec![text("\n"), element("b", vec![])]);
        let in_div = element("div", vec![text("\n"), element("b", vec![])]);
        let stripped = element("div", vec![element("b", vec![])]);
        // `<pre>` keeps its boundary text; `<div>` drops it — and the tag name is erased, so the
        // verdict turns entirely on each side's own context.
        assert_skeletons(&in_pre, &stripped, true, false);
        assert_skeletons(&in_div, &stripped, true, true);
        assert_skeletons(&in_pre, &in_div, true, false);
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
