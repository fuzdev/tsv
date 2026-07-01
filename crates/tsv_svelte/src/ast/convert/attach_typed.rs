//! Template-expression comment attachment as a walk over the typed public AST.
//!
//! Counterpart to the `Value` dispatcher (`walk_and_attach_expressions`, which
//! the `Value` oracle path runs over the whole serialized document): same
//! per-node-type comment windows, same island machinery
//! (`try_attach_comments_to_node` and the shared `attach_*` helpers), but
//! driven by the typed tree so `convert_ast_json_string` can serialize
//! directly — a template comment converts only the expression it lands on
//! into a `serde_json::Value` island (`ExpressionIsland::Attached`), instead
//! of forcing the whole document through `to_value`.
//!
//! Per island, a cheap superset pre-check ("any template comment starting in
//! `[container_start, range_end)`?") decides whether to leave the expression
//! typed. The exact filter (`try_attach_comments_to_node`'s
//! `[container_start, effective_end]` window, where `effective_end` scans past
//! trailing trivia from the expression's end, bounded by `range_end`) only
//! admits comments that start inside the pre-check window, so a pre-check miss
//! is always safe; a false positive costs one `to_value` of that expression
//! (`Attached(to_value(expr))` serializes byte-identically to `Typed(expr)`
//! under `preserve_order`).
//!
//! Parity contract: output must be byte-identical to the `Value` dispatcher's.
//! The walk's reach mirrors it exactly: only the template fragment is visited
//! (never `css`/`instance`/`module`/`options` — the `Value` walk's recursion
//! keys don't include them), and fields the `Value` walk skips (`EachBlock`
//! context, `AwaitBlock` value/error — patterns don't collect comments) stay
//! untouched. Already-`Value` fields (bind/class expressions,
//! const/declaration tags, `SvelteElement` tag) get the same treatment
//! operating on the `Value` in place. Gates: the fixture suite's wire-path
//! identity check plus its synthesized template-comment parity probe
//! (`fixtures_validate`), `json_profile`'s per-file `direct == value`
//! comparison, and `corpus:compare:parse` against Svelte's parser.

use tsv_lang::Comment;

use super::super::public::*;
use super::comment_attachment::{
    attach_const_tag_declaration, attach_declaration_tag_declaration, attach_snippet_parameters,
    is_template_comment, try_attach_comments_to_node,
};
use super::to_json_value;

/// Attach template expression comments to a typed public Root, in place.
///
/// No-op when every comment lies inside a `<script>` content span (the common
/// case — the whole tree stays typed).
pub fn attach_template_expression_comments_typed(
    root: &mut Root<'_>,
    comments: &[Comment],
    script_spans: &[(u32, u32)],
    source: &str,
) {
    let template_comments: Vec<&Comment> = comments
        .iter()
        .filter(|c| is_template_comment(c, script_spans))
        .collect();

    if template_comments.is_empty() {
        return;
    }

    let attacher = Attacher {
        comments: &template_comments,
        source,
    };
    attacher.fragment(&mut root.fragment);
}

struct Attacher<'a> {
    /// Template comments (outside `<script>` content spans), sorted by position.
    comments: &'a [&'a Comment],
    source: &'a str,
}

impl Attacher<'_> {
    /// Superset pre-check: does any template comment *start* in `[start, end)`?
    ///
    /// Every comment `try_attach_comments_to_node` can admit for this window
    /// starts inside it (leading comments start before the expression, inside
    /// the container; trailing comments are scanned from positions `< end`),
    /// so a miss safely skips the island conversion.
    fn any_comment_in(&self, start: u32, end: u32) -> bool {
        self.comments
            .iter()
            .any(|c| c.span.start >= start && c.span.start < end)
    }

    /// Run the scoped DFS on a typed expression island: convert to a `Value`,
    /// attach, and swap the carrier to `Attached`. Stays typed when the
    /// pre-check misses.
    fn island(&self, island: &mut ExpressionIsland<'_>, container_start: u32, range_end: u32) {
        if !self.any_comment_in(container_start, range_end) {
            return;
        }
        let ExpressionIsland::Typed(expression) = island else {
            return; // conversion only produces Typed; the pass runs once
        };
        let mut value = to_json_value(expression);
        try_attach_comments_to_node(
            &mut value,
            self.comments,
            self.source,
            container_start,
            range_end,
        );
        *island = ExpressionIsland::Attached(value);
    }

    /// An attribute or style-directive value: comments can only land on the
    /// ExpressionTag parts (the `Value` dispatcher matches those recursively
    /// inside the serialized value; Text parts and `true` carry nothing).
    fn attribute_value_field(&self, field: &mut AttributeValueField<'_>) {
        match field {
            AttributeValueField::True(_) => {}
            AttributeValueField::Single(part) => self.attribute_value_part(part),
            AttributeValueField::Sequence(parts) => {
                for part in parts {
                    self.attribute_value_part(part);
                }
            }
        }
    }

    fn attribute_value_part(&self, part: &mut AttributeValue<'_>) {
        match part {
            AttributeValue::Text(_) => {}
            AttributeValue::ExpressionTag(t) => self.island(&mut t.expression, t.start, t.end),
        }
    }

    fn fragment(&self, fragment: &mut Fragment<'_>) {
        for node in &mut fragment.nodes {
            self.fragment_node(node);
        }
    }

    fn fragment_node(&self, node: &mut FragmentNode<'_>) {
        match node {
            FragmentNode::Component(e) | FragmentNode::RegularElement(e) => self.element(e),
            FragmentNode::SpecialElement(e) => self.special_element(e),
            FragmentNode::ExpressionTag(t) => self.island(&mut t.expression, t.start, t.end),
            FragmentNode::Text(_) | FragmentNode::Comment(_) => {}
            FragmentNode::IfBlock(b) => {
                // Tighten the window to the first child's start so sibling
                // expression contexts ({:else if}) don't bleed into this test.
                let range_end = fragment_first_node_start(&b.consequent).unwrap_or(b.end);
                self.island(&mut b.test, b.start, range_end);
                self.fragment(&mut b.consequent);
                if let Some(alternate) = &mut b.alternate {
                    self.fragment(alternate);
                }
            }
            FragmentNode::EachBlock(b) => {
                let range_end = fragment_first_node_start(&b.body).unwrap_or(b.end);
                self.island(&mut b.expression, b.start, range_end);
                // context: skip (patterns don't collect comments)
                if let Some(key) = &mut b.key {
                    self.island(key, b.start, range_end);
                }
                self.fragment(&mut b.body);
                if let Some(fallback) = &mut b.fallback {
                    self.fragment(fallback);
                }
            }
            FragmentNode::AwaitBlock(b) => {
                // Earliest child across pending/then/catch, matching the
                // `Value` walk's first_child_start over all three keys.
                let range_end = [
                    b.pending.as_ref(),
                    b.then_block.as_ref(),
                    b.catch_block.as_ref(),
                ]
                .into_iter()
                .flatten()
                .filter_map(fragment_first_node_start)
                .min()
                .unwrap_or(b.end);
                self.island(&mut b.expression, b.start, range_end);
                // value/error: skip (patterns don't collect comments)
                if let Some(pending) = &mut b.pending {
                    self.fragment(pending);
                }
                if let Some(then_block) = &mut b.then_block {
                    self.fragment(then_block);
                }
                if let Some(catch_block) = &mut b.catch_block {
                    self.fragment(catch_block);
                }
            }
            FragmentNode::KeyBlock(b) => {
                let range_end = fragment_first_node_start(&b.fragment).unwrap_or(b.end);
                self.island(&mut b.expression, b.start, range_end);
                self.fragment(&mut b.fragment);
            }
            FragmentNode::SnippetBlock(b) => {
                let range_end = fragment_first_node_start(&b.body).unwrap_or(b.end);
                self.island(&mut b.expression, b.start, range_end);
                self.snippet_parameters(&mut b.parameters, b.start, range_end);
                self.fragment(&mut b.body);
            }
            FragmentNode::HtmlTag(t) => self.island(&mut t.expression, t.start, t.end),
            FragmentNode::ConstTag(t) => attach_const_tag_declaration(
                &mut t.declaration,
                self.comments,
                self.source,
                t.start,
                t.end,
            ),
            FragmentNode::DeclarationTag(t) => attach_declaration_tag_declaration(
                &mut t.declaration,
                self.comments,
                self.source,
                t.start,
                t.end,
            ),
            FragmentNode::DebugTag(t) => {
                for identifier in &mut t.identifiers {
                    self.island(identifier, t.start, t.end);
                }
            }
            FragmentNode::RenderTag(t) => self.island(&mut t.expression, t.start, t.end),
        }
    }

    /// The snippet parameter list shares one advancing cursor (see
    /// `attach_snippet_parameters`), which needs every parameter's end even
    /// when nothing attaches to it — so a pre-check hit converts the whole
    /// list to `Value`s and runs the shared cursor loop over them.
    fn snippet_parameters(
        &self,
        parameters: &mut [ExpressionIsland<'_>],
        c_start: u32,
        range_end: u32,
    ) {
        if parameters.is_empty() || !self.any_comment_in(c_start, range_end) {
            return;
        }
        let mut values: Vec<serde_json::Value> = parameters.iter().map(to_json_value).collect();
        attach_snippet_parameters(&mut values, self.comments, self.source, c_start, range_end);
        for (island, value) in parameters.iter_mut().zip(values) {
            *island = ExpressionIsland::Attached(value);
        }
    }

    fn element(&self, element: &mut Element<'_>) {
        for attribute in &mut element.attributes {
            self.attribute_node(attribute);
        }
        self.fragment(&mut element.fragment);
    }

    fn special_element(&self, element: &mut SpecialElement<'_>) {
        // The `Value` dispatcher keys these arms on the node type string; the
        // other special elements carry no comment-attachable expression.
        match element.node_type {
            "SvelteElement" => {
                if let Some(tag) = &mut element.tag {
                    try_attach_comments_to_node(
                        tag,
                        self.comments,
                        self.source,
                        element.start,
                        element.end,
                    );
                }
            }
            "SvelteComponent" => {
                self.optional_island(&mut element.expression, element.start, element.end);
            }
            _ => {}
        }
        for attribute in &mut element.attributes {
            self.attribute_node(attribute);
        }
        self.fragment(&mut element.fragment);
    }

    fn attribute_node(&self, node: &mut AttributeNode<'_>) {
        match node {
            AttributeNode::Attribute(a) => {
                // The value can contain ExpressionTags (quoted sequences);
                // the `Value` dispatcher finds them recursively.
                self.attribute_value_field(&mut a.value);
            }
            AttributeNode::SpreadAttribute(a) => self.island(&mut a.expression, a.start, a.end),
            AttributeNode::AttachTag(a) => self.island(&mut a.expression, a.start, a.end),
            AttributeNode::OnDirective(d) => {
                self.optional_island(&mut d.expression, d.start, d.end);
            }
            AttributeNode::BindDirective(d) => {
                self.directive_value(&mut d.expression, d.start, d.end);
            }
            AttributeNode::ClassDirective(d) => {
                self.directive_value(&mut d.expression, d.start, d.end);
            }
            AttributeNode::StyleDirective(d) => {
                // value can be: true, ExpressionTag object, or array of parts
                self.attribute_value_field(&mut d.value);
            }
            AttributeNode::UseDirective(d) => {
                self.optional_island(&mut d.expression, d.start, d.end);
            }
            AttributeNode::TransitionDirective(d) => {
                self.optional_island(&mut d.expression, d.start, d.end);
            }
            AttributeNode::AnimateDirective(d) => {
                self.optional_island(&mut d.expression, d.start, d.end);
            }
            AttributeNode::LetDirective(d) => {
                self.optional_island(&mut d.expression, d.start, d.end);
            }
        }
    }

    /// A directive's optional expression island (`on:`/`use:`/`transition:`/
    /// `animate:`/`let:` — absent for the bare form).
    fn optional_island(
        &self,
        island: &mut Option<ExpressionIsland<'_>>,
        container_start: u32,
        range_end: u32,
    ) {
        if let Some(island) = island {
            self.island(island, container_start, range_end);
        }
    }

    /// A `bind:`/`class:` directive's `Value` expression. Shorthand directives
    /// serialize a loc-free Identifier; the is_object guard mirrors the
    /// `Value` dispatcher's.
    fn directive_value(&self, expression: &mut serde_json::Value, start: u32, end: u32) {
        if expression.is_object() {
            try_attach_comments_to_node(expression, self.comments, self.source, start, end);
        }
    }
}

/// Start position of a fragment's first node — the typed mirror of the `Value`
/// dispatcher's `first_child_start` (which reads `child.nodes[0].start`).
fn fragment_first_node_start(fragment: &Fragment<'_>) -> Option<u32> {
    fragment.nodes.first().map(|node| match node {
        FragmentNode::Component(e) | FragmentNode::RegularElement(e) => e.start,
        FragmentNode::SpecialElement(e) => e.start,
        FragmentNode::ExpressionTag(t) => t.start,
        FragmentNode::Text(t) => t.start,
        FragmentNode::Comment(c) => c.start,
        FragmentNode::IfBlock(b) => b.start,
        FragmentNode::EachBlock(b) => b.start,
        FragmentNode::AwaitBlock(b) => b.start,
        FragmentNode::KeyBlock(b) => b.start,
        FragmentNode::SnippetBlock(b) => b.start,
        FragmentNode::HtmlTag(t) => t.start,
        FragmentNode::ConstTag(t) => t.start,
        FragmentNode::DeclarationTag(t) => t.start,
        FragmentNode::DebugTag(t) => t.start,
        FragmentNode::RenderTag(t) => t.start,
    })
}
