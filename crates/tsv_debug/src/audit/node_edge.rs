//! The wire-tree **node-edge** walker: the enclosing AST node and child-role
//! span-containment lookup behind `gap_audit --by-node`'s coarse emitter rollup.
//!
//! Walks a parsed wire tree (see
//! [`tsv_parse_to_value`](crate::audit::properties::tsv_parse_to_value)) to find the
//! innermost node strictly containing a byte offset, then names the child-role edge the
//! offset sits between (see [`NodeEdgeKey`]). Split out of `sites.rs`, which keeps only
//! site enumeration and shape keying.

use serde_json::Value;

use crate::audit::properties::Utf16ToByte;

/// A coarse **node-edge** key for an injection offset: the enclosing AST node and the
/// child-role edge its gap sits in.
///
/// Where [`site_shape`](crate::audit::sites::site_shape) keys a finding by the raw source
/// tokens on each side — the *fine* ratchet key — this keys it by STRUCTURE: the innermost
/// wire node whose span contains the offset, and which pair of that node's child roles the
/// offset falls between. It is the COARSE emitter rollup that complements the token shape:
/// the ratchet already pins the fine view, so rolling the ~700 token shapes up onto
/// `(node_type, edge)` collapses them into the few dozen emitter clusters — each ≈ one
/// printer function — that a burn-down works through.
///
/// `edge` is `"{left}→{right}"` over child ROLES, where a role is the child's wire field key
/// with any array index collapsed (an element of `"arguments": [...]` is role `arguments`),
/// `^` is the parent's own start (offset before the first child), and `$` is its own end
/// (after the last). So `(CallExpression, callee→arguments)` is a comment in the `f⟨⟩(` gap,
/// `(VariableDeclarator, id→init)` the `=`-gap, `(ImportDeclaration, ^→specifiers)` the
/// `import⟨⟩{` gap.
///
/// Report-only: nothing here feeds the ratchet key or the snapshot.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct NodeEdgeKey {
    pub(crate) node_type: String,
    pub(crate) edge: String,
}

impl std::fmt::Display for NodeEdgeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.node_type, self.edge)
    }
}

/// A direct span-bearing child of a wire node, in byte space, tagged with the field role it
/// sits under (spanless containers seen through — see [`collect_child`]).
struct WireChild<'a> {
    role: &'a str,
    start: usize,
    end: usize,
    node: &'a Value,
}

/// Wire keys that are NOT structural children.
///
/// `loc` is line/column, never a span. The rest are the comment-attachment decoration the wire
/// re-attaches to mirror acorn (`leadingComments` / `trailingComments` per node, plus the
/// language root's detached `comments` list) — arrays of comment NODES, not emitter children.
/// Left in, they become child ROLES, and since a comment attaches only where the seed happens
/// to carry one, the SAME structural gap would label `leadingComments→id` on the hits whose seed
/// carries a comment there and `^→id` on the hits without one — data-dependent across hits,
/// splitting one emitter's edge in two. Skipping them is sound: a comment's span lies outside its host node's
/// structural span (and comment interiors are excluded from injection sites), so one never
/// *strictly contains* an offset the descent would enter, and it carries no span-bearing
/// descendant — so this only relabels a mislabeled `leadingComments→X` / `X→trailingComments`
/// back to the structural `^→X` / `X→$`, leaving node selection and descent unchanged.
fn is_non_structural_key(key: &str) -> bool {
    matches!(
        key,
        "loc" | "leadingComments" | "trailingComments" | "comments"
    )
}

/// Collect a wire node's **direct** span-bearing children.
///
/// A field's value is dispatched by [`collect_child`]: a span-bearing object is one child
/// (role = the field key), an array contributes each span-bearing element (role = the field
/// key, index collapsed), and a SPANLESS object is seen THROUGH — its span-bearing
/// descendants become children of this node under the outer field key. That last case is
/// load-bearing for Svelte: `Fragment` carries no span yet holds the template `nodes`, so
/// without it a template offset could reach nothing below the `Root`. (`loc` / `metadata` /
/// `name_loc` are spanless too but hold no span-bearing descendants, so seeing through them
/// yields nothing.) Comment-attachment decoration keys are skipped — see
/// [`is_non_structural_key`].
fn wire_children<'a>(node: &'a Value, map: &Utf16ToByte, out: &mut Vec<WireChild<'a>>) {
    let Value::Object(obj) = node else {
        return;
    };
    for (key, value) in obj {
        if is_non_structural_key(key) {
            continue;
        }
        collect_child(key, value, map, out);
    }
}

/// Route one field value into the child list under `role`, seeing through spanless objects
/// and arrays. See [`wire_children`].
fn collect_child<'a>(
    role: &'a str,
    value: &'a Value,
    map: &Utf16ToByte,
    out: &mut Vec<WireChild<'a>>,
) {
    match value {
        Value::Object(inner) => match map.node_byte_span(value) {
            Some((start, end)) => out.push(WireChild {
                role,
                start,
                end,
                node: value,
            }),
            // A spanless container (Svelte's `Fragment` is the one that matters): see through
            // it, keeping the OUTER role so a template node reads as a child of its element.
            None => {
                for (k, v) in inner {
                    if is_non_structural_key(k) {
                        continue;
                    }
                    collect_child(role, v, map, out);
                }
            }
        },
        Value::Array(items) => {
            for item in items {
                collect_child(role, item, map, out);
            }
        }
        _ => {}
    }
}

/// The `{left}→{right}` edge an `offset` falls in among a node's `children`.
///
/// None of `children` strictly contains `offset` — the descent already entered any that did —
/// so each sits wholly at-or-before it (`end <= offset`) or wholly at-or-after it
/// (`start >= offset`). The edge names the nearest child on each side: the largest-`end` child
/// to the left (`^` when none), the smallest-`start` child to the right (`$` when none). A
/// childless leaf is therefore `^→$`.
fn edge_between(children: &[WireChild<'_>], offset: usize) -> String {
    let left = children
        .iter()
        .filter(|c| c.end <= offset)
        .max_by_key(|c| (c.end, c.start))
        .map_or("^", |c| c.role);
    let right = children
        .iter()
        .filter(|c| c.start >= offset)
        .min_by_key(|c| (c.start, c.end))
        .map_or("$", |c| c.role);
    format!("{left}→{right}")
}

/// Key a byte `offset` to the AST node-edge whose gap it sits in — the coarse structural
/// companion to [`site_shape`](crate::audit::sites::site_shape).
///
/// `wire` is the parse of `source`
/// ([`tsv_parse_to_value`](crate::audit::properties::tsv_parse_to_value)); `offset` is a
/// **byte** offset (wire positions are UTF-16, translated through [`Utf16ToByte`]). The
/// walk descends into the innermost node strictly containing `offset` — a `.svelte` script
/// offset through the embedded `Program`, a template offset through the spanless `Fragment`
/// into the tag it lands in — then names the child-role edge the offset sits between.
///
/// `None` when `offset` is outside the wire root's span, the root carries no span, or the
/// innermost node has no `type`.
///
/// Thin wrapper over [`node_edge_key_with_map`]: it builds the [`Utf16ToByte`] map from `source`
/// per call. The record-time keyer instead builds the map **once per file** and reuses it across
/// every hit via [`node_edge_key_with_map`] — the reason this one must never move into a loop
/// over all injection sites. The source-based convenience form: currently only this module's
/// tests drive it (so they exercise `with_map` and the map-build transitively — the multibyte
/// coverage rides here), hence `allow(dead_code)` for the non-test build.
#[allow(dead_code)] // source-based wrapper; exercised by this module's tests, kept as the single-offset API
pub(crate) fn node_edge_key(wire: &Value, source: &str, offset: usize) -> Option<NodeEdgeKey> {
    let map = Utf16ToByte::new(source);
    node_edge_key_with_map(wire, &map, offset)
}

/// [`node_edge_key`]'s walk, over a **prebuilt** [`Utf16ToByte`] map.
///
/// `map` must be the wire→byte map of the same `source` `wire` was parsed from (the wire's own
/// positions are UTF-16, translated through it). Split from [`node_edge_key`] so a caller keying
/// many offsets against one file — the record-time by-node keyer — pays the map build once rather
/// than per offset.
pub(crate) fn node_edge_key_with_map(
    wire: &Value,
    map: &Utf16ToByte,
    offset: usize,
) -> Option<NodeEdgeKey> {
    let (root_start, root_end) = map.node_byte_span(wire)?;
    if offset < root_start || offset > root_end {
        return None;
    }
    let mut node = wire;
    loop {
        let mut children = Vec::new();
        wire_children(node, map, &mut children);
        let inner = children
            .iter()
            .find(|c| c.start < offset && offset < c.end)
            .map(|c| c.node);
        match inner {
            Some(next) => node = next,
            None => {
                let node_type = node.get("type").and_then(Value::as_str)?;
                return Some(NodeEdgeKey {
                    node_type: node_type.to_string(),
                    edge: edge_between(&children, offset),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::properties::tsv_parse_to_value;
    use tsv_cli::cli::input::ParserType;

    /// Parse `src` and key `offset` — the node-edge harness the tests below share. Both
    /// arms must succeed (the snippets parse; the offsets are in range), so `expect` here
    /// is asserting the test's own premise, not swallowing a real failure.
    fn edge_at(src: &str, parser: ParserType, offset: usize) -> NodeEdgeKey {
        let wire = tsv_parse_to_value(src, parser).expect("snippet parses");
        node_edge_key(&wire, src, offset).expect("offset in range")
    }

    /// A key spelled out, so a test reads as the `(node, edge)` it asserts.
    fn key(node_type: &str, edge: &str) -> NodeEdgeKey {
        NodeEdgeKey {
            node_type: node_type.to_string(),
            edge: edge.to_string(),
        }
    }

    /// The three call-argument edges the whole rollup is named for: the `f⟨⟩(` gap before the
    /// first argument (`callee→arguments`), the gap between two arguments
    /// (`arguments→arguments`), and the gap after the last (`arguments→$`). All three key to
    /// the one `CallExpression` — the coarse emitter — not to the arguments themselves.
    #[test]
    fn node_edge_keys_the_call_argument_edges() {
        let src = "f(a, b)";
        // `f⟨⟩(` — after the callee, before the argument list.
        assert_eq!(
            edge_at(src, ParserType::TypeScript, src.find('(').unwrap()),
            key("CallExpression", "callee→arguments")
        );
        // `a⟨⟩,` — between the two arguments.
        assert_eq!(
            edge_at(src, ParserType::TypeScript, src.find(',').unwrap()),
            key("CallExpression", "arguments→arguments")
        );
        // `b⟨⟩)` — after the last argument, before the node's own end.
        assert_eq!(
            edge_at(src, ParserType::TypeScript, src.rfind(')').unwrap()),
            key("CallExpression", "arguments→$")
        );
    }

    /// `^→{first}` — an offset before every child names the parent's own start on the left.
    /// The `import⟨⟩{` gap sits between `ImportDeclaration`'s start and its first specifier.
    #[test]
    fn node_edge_keys_a_before_first_edge() {
        let src = "import {a} from 'm';";
        assert_eq!(
            edge_at(src, ParserType::TypeScript, src.find('{').unwrap()),
            key("ImportDeclaration", "^→specifiers")
        );
    }

    /// The `=`-gap rollup: a comment between a declarator's `id` and `init` keys to
    /// `(VariableDeclarator, id→init)`, one of the D2 worked examples.
    #[test]
    fn node_edge_keys_the_declarator_assignment_gap() {
        let src = "const x = 1;";
        assert_eq!(
            edge_at(src, ParserType::TypeScript, src.find('=').unwrap()),
            key("VariableDeclarator", "id→init")
        );
    }

    /// Innermost-node selection: the `.` in `g(a.b, c)` sits in the FIRST argument's member
    /// access, so the walk must descend PAST the `CallExpression` into that argument's
    /// `MemberExpression` before naming the edge — not stop at the call.
    #[test]
    fn node_edge_descends_to_the_innermost_node() {
        let src = "g(a.b, c)";
        assert_eq!(
            edge_at(src, ParserType::TypeScript, src.find('.').unwrap()),
            key("MemberExpression", "object→property")
        );
    }

    /// A childless leaf is `^→$`: an offset strictly inside an `Identifier` (which has no
    /// span-bearing children) names the node's own start-to-end, since there is no child on
    /// either side.
    #[test]
    fn node_edge_keys_a_childless_leaf() {
        // Offset 1 is strictly inside `foo` [0,3), so the walk descends into the Identifier
        // and finds it childless.
        assert_eq!(
            edge_at("foo;", ParserType::TypeScript, 1),
            key("Identifier", "^→$")
        );
    }

    /// A CSS edge — `(Rule, prelude→block)` at the `{` — proving the walk covers parseCss too,
    /// and that `Rule`'s spanless `metadata` object is NOT mistaken for a child (it would
    /// otherwise wedge a bogus role between the prelude and the block).
    #[test]
    fn node_edge_keys_a_css_rule_edge() {
        let src = "a { color: red; }";
        assert_eq!(
            edge_at(src, ParserType::Css, src.find('{').unwrap()),
            key("Rule", "prelude→block")
        );
    }

    /// NON-ASCII: the `é` is two bytes but one UTF-16 unit, so every wire span past it is off
    /// by one from its byte position. The `b` of `bar` sits at byte 7; only a correct
    /// `Utf16ToByte` translation keeps `property` starting at byte 7 (not the wire's 6), so
    /// the offset lands on the `object→property` edge. An identity ("offset == char index")
    /// map would instead read the offset as strictly inside `property` and descend into the
    /// leaf, keying `(Identifier, ^→$)` — the exact ASCII-invisible bug this guards.
    #[test]
    fn node_edge_keys_through_a_multibyte_offset() {
        let src = "foo(é.bar)";
        let b = src.find("bar").unwrap(); // byte 7, a char boundary just past the `é`
        assert_eq!(
            edge_at(src, ParserType::TypeScript, b),
            key("MemberExpression", "object→property")
        );
    }

    /// A `.svelte` SCRIPT offset resolves to a JS node: the walk descends `Root` → `Script`
    /// (both span-bearing) → the embedded `Program`, so the `a, b` comma keys to the script's
    /// `CallExpression`, exactly as the standalone `.ts` case does.
    #[test]
    fn node_edge_descends_into_a_svelte_script() {
        let src = "<script>f(a, b)</script>";
        assert_eq!(
            edge_at(src, ParserType::Svelte, src.find(',').unwrap()),
            key("CallExpression", "arguments→arguments")
        );
    }

    /// A `.svelte` TEMPLATE offset resolves through the SPANLESS `Fragment`: the walk sees
    /// through `Root`'s fragment into the element, then through the element's fragment into
    /// the `ExpressionTag`'s expression — so the `.` in `{a.b}` keys to the member access.
    /// Without transparency every template offset would collapse onto the `Root`.
    #[test]
    fn node_edge_descends_through_a_svelte_fragment() {
        let src = "<div>{a.b}</div>";
        assert_eq!(
            edge_at(src, ParserType::Svelte, src.find('.').unwrap()),
            key("MemberExpression", "object→property")
        );
    }

    /// Comment-attachment decoration is NOT a structural child. The wire attaches this
    /// leading comment onto the `FunctionDeclaration` as a `leadingComments` array, so without
    /// the skip the `function⟨⟩ f` gap would key on that comment role
    /// (`leadingComments→id`) — and only because a comment happens to sit there. The skip
    /// keeps the edge STRUCTURAL: `^→id`. (The comment leads a non-first statement so it
    /// attaches to the node rather than landing on the root's detached `comments` list.)
    #[test]
    fn node_edge_skips_comment_attachment_decoration() {
        let src = "<script>let x;\n/* c */\nfunction f() {}</script>";
        // The gap just after the `function` keyword, before the name.
        let offset = src.find("function").unwrap() + "function".len();
        assert_eq!(
            edge_at(src, ParserType::Svelte, offset),
            key("FunctionDeclaration", "^→id")
        );
    }
}
