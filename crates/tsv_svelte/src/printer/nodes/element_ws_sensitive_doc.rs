// Doc-based formatting for whitespace-sensitive elements (pre, textarea)
//
// These elements preserve text whitespace exactly as authored, but still
// format embedded expressions, blocks, and other dynamic content normally.
// The nested if/each builders here hug their structure (no added whitespace)
// so the block syntax does not inject rendered whitespace into <pre>/<textarea>.

// Allow Svelte block syntax like `{:else}`, `{:then}`, `{:catch}` which
// look like Rust format args but are valid Svelte template syntax.
#![allow(clippy::literal_string_with_formatting_args)]

use super::blocks_doc::{EACH_BLOCK_OPEN, ELSE_IF_BLOCK_OPEN, IF_BLOCK_OPEN};
use super::element_doc::{AttrListEmission, ElementAttrsDoc};
use super::helpers::each_expr_comment_end;
use crate::ast::internal::{self, Fragment, FragmentNode, is_collapsible_ws_char};
use crate::printer::Printer;
use smallvec::smallvec;
use tsv_lang::doc::{DocBuf, arena::DocId};

/// Which of a whitespace-sensitive element's two delimiters its content lets move.
///
/// Prettier's `shouldHugStart` / `shouldHugEnd`, which inside this family reduce to their
/// last clause each (such an element is never `isBlockElement` to prettier's eye, and always
/// has children when the pair is asked). Both are read off the content's **own** end: the
/// first child for the opening `>`, the last for the closing one.
///
/// ⚠️ **Two fields, not one.** The two ends are independent here, and coupling them is the
/// bug this type exists to make unspellable: the printer used to key both on the *opening*
/// edge, so `<textarea …> text</textarea>` and `<textarea …>text </textarea>` — mirror
/// images — each got the other's answer. That is the exact inverse of
/// [`BoundaryMode`](super::element_doc::BoundaryMode), which carries **one** value for both
/// tags precisely because a render-free boundary is trimmed on both sides at once and the
/// delimiters must move together. Here the boundary run is *literal*: nothing trims it, so
/// there is no shared fact to keep in sync and each delimiter owes its own end an answer.
#[derive(Clone, Copy)]
pub(super) struct ContentEdges {
    /// The content opens on a visible byte, so the `>` may take a break point of its own.
    /// When it opens on collapsible whitespace the author already put a break-worthy byte
    /// right after the `>`, and a second break there buys nothing.
    hugs_open: bool,
    /// The content ends on a visible byte, so the closing tag splits and its `>` dangles.
    hugs_close: bool,
}

impl<'a> Printer<'a> {
    /// Build doc for whitespace-sensitive elements (pre, textarea, etc.)
    ///
    /// These elements preserve text whitespace exactly as-is, but still format
    /// expressions, blocks, and other dynamic content normally.
    ///
    /// All indents are relative to the element's own doc-indent, which its parent's body
    /// wrap (one level per container, like prettier — see
    /// `build_whitespace_sensitive_content_doc`) sets to the element's nesting depth.
    /// Preserved text carries no doc-hardlines, so that wrap never injects rendered
    /// whitespace; only the tag-internal breaks below pick it up.
    ///
    /// The dispatch, in the order the arms are written (each returns, so order is the rule):
    /// - **void** — no closing tag, whatever the whitespace class says.
    /// - **empty and authored self-closing**, where `/>` may survive at all.
    /// - **with content** — [`Self::build_ws_sensitive_head_with_content_doc`], the one shape,
    ///   selected off [`ContentEdges`] and the attribute list. A block `<pre>` joins the inline
    ///   tags here only when its list ends on a `//`.
    /// - **block with a single simple `{expr}`** — the one arm that still weighs the content's
    ///   width against the head's, breaking the `>` to base indent rather than overflowing
    ///   ([`block_multiline_attrs_content_hug`](../../../../../tests/fixtures/svelte/elements/block_multiline_attrs_content_hug_prettier_divergence/)).
    /// - **inline, empty, with attrs** — `/>` drops on wrap; an explicit `></tag>` hugs.
    /// - **fallback** — [`Self::build_ws_sensitive_open_tag_doc`] plus content plus a whole
    ///   closing tag.
    ///
    /// Every hug in those is a deliberate refusal to break before the `>`: a break there is
    /// free for an ordinary element but here it borders literal content. The one comment fact
    /// that overrides it travels on [`AttrListEmission`], from the one attribute-list emitter
    /// ([`Printer::push_attrs_with_comments`]): a list **ending on a `//`** cannot share its
    /// line with the `>` (a line comment runs to end of line — a hugged `>` lands *inside* it
    /// and the output stops re-parsing), so the `>` breaks —
    /// [`ws_sensitive_attr_comment_line`](../../../../../tests/fixtures/svelte/elements/ws_sensitive_attr_comment_line_prettier_divergence/).
    /// A comment that holds a **hardline** in the list needs no fact of its own: the list is
    /// a group like any other head's, so the hardline wraps it one member per line and the `>`
    /// hugs the last —
    /// [`ws_sensitive_attr_comment_own_line`](../../../../../tests/fixtures/svelte/elements/ws_sensitive_attr_comment_own_line/).
    /// Either break stays inside the tag, where no character is content, so the render is
    /// unchanged.
    pub(super) fn build_whitespace_sensitive_element_doc(
        &self,
        element: &internal::Element<'_>,
        attrs: ElementAttrsDoc,
    ) -> DocId {
        let ElementAttrsDoc {
            docs: attr_docs,
            emission,
        } = attrs;
        let d = self.d();
        let name_doc = d.source_span(element.name_span, self.source);

        // A void element has no closing tag — that is a fact about the TAG, not about the
        // surrounding layout, so whitespace-sensitivity has no say in it. Without this the
        // generic path below reached its `></tag>` close form and fabricated one
        // (`<pre><br></pre>` → `<pre><br></br></pre>`), which is not valid HTML (a void element
        // cannot have an end tag) and which tsv's own parser then rejects — a format that
        // corrupts content and whose output will not reparse. Only the empty-with-attrs
        // self-closing branch happened to escape it, so 3 of the 4 attrs × authored-`/`
        // combinations were broken. Pinned by `elements/pre_void_element`.
        let class = self.classify_tag(element);
        if class.is_void {
            let parts = self.element_parts(element, class);
            return self.build_void_element_doc(&parts, &attr_docs, class.is_declaration);
        }

        // Whether an authored `/>` may survive — the SAME question the regular element path
        // asks, for the same reason. Whitespace-sensitivity is about the literalness of
        // CONTENT; it says nothing about how a tag serializes, and `<i />` → `<i></i>` adds no
        // characters to the rendered text. Answering it locally here got it wrong in both
        // directions, split by whether the element had attributes: the with-attrs branch below
        // preserved `/>` for every kind (wrong for a plain `<i … />`, where the HTML spec makes
        // the `/` a parse error the parser ignores), while the no-attrs case fell through to the
        // generic close-tag path and expanded every kind (wrong for `<Comp />` and
        // `<svg:rect />`, where the `/` is meaningful). Pinned by
        // `elements/pre_self_closing_kinds_prettier_divergence`.
        let can_self_close = class.kind.is_component() || class.is_foreign || class.is_namespaced;
        if can_self_close
            && element.fragment.nodes.is_empty()
            && attr_docs.is_empty()
            && self.span_was_self_closing(element.span)
        {
            return d.concat(&[d.text("<"), name_doc, d.text(" />")]);
        }

        // Deliberately NOT `ElementKind::is_inline` (and so not `classify_tag`): inside a
        // whitespace-preserving subtree a *component* counts as inline flow, where the shared
        // classifier splits `Component` out as its own kind. The two predicates agree on
        // `<pre>`/`<textarea>` themselves but diverge on a nested `<Comp>`, so this stays its own
        // question rather than being folded into the shared one.
        let is_inline = !element.facts.is_block();
        let has_content = !element.fragment.nodes.is_empty();

        // Which delimiters the content lets move — one read per boundary, see
        // [`ContentEdges`].
        let edges = self.ws_sensitive_content_edges(element.fragment.nodes);

        // Whitespace-sensitive elements with content: one shape, off `edges` and the list.
        //
        // A **block** element (`<pre>`) reaches it only when its list ends on a `//`, which
        // nothing may share a line with. Everything else about the shape is the same for
        // both tags — two whitespace-sensitive elements answering one question with two
        // layouts would be a distinction with no source in the elements.
        if (is_inline || emission.ends_with_line_comment) && has_content {
            let content_doc = self.build_whitespace_sensitive_content_doc(element.fragment.nodes);
            return self.build_ws_sensitive_head_with_content_doc(
                name_doc,
                &attr_docs,
                emission,
                content_doc,
                edges,
            );
        }

        // Block whitespace-sensitive elements with content and attrs (pre with content)
        // Divergence: When attrs wrap and `>{content}</tag>` would exceed print width, break `>` to new line.
        // This respects print width while preserving whitespace semantics (no text node added).
        //
        // Only apply this logic for simple content. For complex content that can break internally
        // (like function calls), use normal flow so content breaks first.
        if !is_inline && has_content && !attr_docs.is_empty() {
            // Check if content is "simple" - single expression tag without internal break points
            // Complex content (function calls, ternaries, etc.) should break internally first
            let is_simple_content = element.fragment.nodes.len() == 1
                && matches!(
                    &element.fragment.nodes[0],
                    FragmentNode::ExpressionTag(expr) if !Self::expression_has_break_points(&expr.expression)
                );

            if is_simple_content {
                let content_doc =
                    self.build_whitespace_sensitive_content_doc(element.fragment.nodes);

                // Inner group decides if `>` needs to break to new line
                let closing_and_content = d.group(d.concat(&[
                    d.softline(),
                    d.text(">"),
                    content_doc,
                    d.text("</"),
                    name_doc,
                    d.text(">"),
                ]));

                // Outer group decides if attrs need to break
                let dedented = d.dedent(closing_and_content);
                let attr_concat = d.concat(&attr_docs);
                let indented = d.indent(d.concat(&[attr_concat, dedented]));
                return d.group(d.concat(&[d.text("<"), name_doc, indented]));
            }
            // Fall through to normal handling for complex content
        }

        // Empty inline whitespace-sensitive element with attributes — `<textarea
        // attrs></textarea>`, a self-closing `<textarea attrs />`, or an inline
        // element/component inside `<pre>`. The layout splits on the source close form,
        // which is always preserved (never rewritten between `/>` and `></tag>`):
        //
        // - Explicit-empty (`></tag>`): mirror prettier-plugin-svelte's empty
        //   hugStart/hugEnd — the closing `>` lives in its OWN group, so it hugs the last
        //   attribute unless `></tag>` (plus any trailing suffix like
        //   `></textarea></label>`) would overflow, only then breaking to its own line.
        //   Attributes wrap independently of that decision.
        // - Self-closing (`/>`): the `/>` shares the element's outer group, so it drops
        //   to its own line whenever the element breaks — never hugging a wrapped last
        //   attribute, matching prettier and every other self-closing tag.
        //
        // (A block `<pre>` reaches the fallback instead, where
        // [`Self::build_ws_sensitive_open_tag_doc`] hugs the `>` onto whatever the list
        // ends on — the same hug, minus the empty element's width question.)
        if is_inline && !has_content && !attr_docs.is_empty() {
            let attr_indent = self.ws_sensitive_attrs_group(&attr_docs);
            if can_self_close && self.span_was_self_closing(element.span) {
                // line() is a space when flat (`<tag attrs />`), a newline when the outer
                // group breaks. Mirrors build_void_element_doc.
                return d.group(d.concat(&[
                    d.text("<"),
                    name_doc,
                    attr_indent,
                    d.line(),
                    d.text("/>"),
                ]));
            }
            // group(['>', '</tag']): the final `>` is appended outside, so the softline's
            // fits() weighs `></tag>` and the trailing suffix together. A trailing `//` is
            // not a width question — the hug is impossible at any width — so it takes the
            // break directly rather than through the group.
            let close_seq = d.group(d.concat(&[d.text(">"), d.text("</"), name_doc]));
            let before_close = if emission.ends_with_line_comment {
                d.hardline()
            } else {
                d.softline()
            };
            let hugged = d.group(d.concat(&[before_close, close_seq]));
            return d.group(d.concat(&[d.text("<"), name_doc, attr_indent, hugged, d.text(">")]));
        }

        // Build content preserving text whitespace but formatting expressions/blocks
        let opening_tag = self.build_ws_sensitive_open_tag_doc(name_doc, &attr_docs, emission);
        let content_doc = self.build_whitespace_sensitive_content_doc(element.fragment.nodes);

        d.concat(&[opening_tag, content_doc, self.end_tag(name_doc)])
    }

    /// Read both content edges — the pair [`ContentEdges`] documents.
    ///
    /// Each end is a `Text` question and only a `Text` question: any other child (an `{expr}`
    /// tag, a block, a nested element) is a visible byte, so it hugs. An empty fragment hugs
    /// at both ends and is never asked.
    ///
    /// ⚠️ The closing edge is the **last child**, not the last text node — those differ
    /// exactly when an `{expr}` tag closes the content, and reading the last *text* node
    /// there gave the delimiter a run the author did not put against it.
    ///
    /// ⚠️ Deliberately NOT [`Printer::text_glued_before`], whose spelling the opening half is
    /// the negation of: that predicate means "no break may land here, because one would
    /// inject a *collapsible* space", and inside a whitespace-PRESERVING subtree the same
    /// bytes are literal content. Same character class, different claim; folding them would
    /// let a change to one silently retarget the other.
    pub(super) fn ws_sensitive_content_edges(&self, nodes: &[FragmentNode<'_>]) -> ContentEdges {
        let text_raw = |node: Option<&FragmentNode<'_>>| match node {
            Some(FragmentNode::Text(text)) => Some(text.raw(self.source)),
            _ => None,
        };
        ContentEdges {
            hugs_open: !text_raw(nodes.first())
                .is_some_and(|raw| raw.starts_with(is_collapsible_ws_char)),
            hugs_close: !text_raw(nodes.last())
                .is_some_and(|raw| raw.ends_with(is_collapsible_ws_char)),
        }
    }

    /// The attribute list as a **group of its own**, indented — the one spelling for
    /// "this head's list wraps on its own terms".
    ///
    /// Two heads read it: the content-bearing one below and the empty-with-attrs arm, whose
    /// closers both live in a *separate* group and so must not be able to drag the list open
    /// with them. [`Self::build_ws_sensitive_open_tag_doc`] deliberately does **not** use it —
    /// there the `>` sits inside the list's own group with nothing else to break, so an inner
    /// group would only change what `fits` weighs.
    fn ws_sensitive_attrs_group(&self, attr_docs: &[DocId]) -> DocId {
        let d = self.d();
        if attr_docs.is_empty() {
            return d.empty();
        }
        d.indent(d.group(d.concat(attr_docs)))
    }

    /// The **hug-the-`>`** head: `<tag attrs>`, with the `>` glued to whatever the attribute
    /// list ends on and the list wrapping one member per line when it does not fit.
    ///
    /// Prettier reaches the same place from the other side: its `openingTag` spends its
    /// trailing `dedent(softline)` only when `!isPreTagContent(path)`, so inside this family
    /// the `>` never takes a line of its own on the *list's* account. The one thing that does
    /// move it is a list ending on a `//`, which nothing may share a line with; with no
    /// content there is nothing for the `>` to be adjacent to, so it drops to base indent —
    /// where every other element puts it.
    pub(super) fn build_ws_sensitive_open_tag_doc(
        &self,
        name: DocId,
        attr_docs: &[DocId],
        emission: AttrListEmission,
    ) -> DocId {
        let d = self.d();
        if attr_docs.is_empty() {
            return self.start_tag(name);
        }
        let before_close = if emission.ends_with_line_comment {
            d.hardline()
        } else {
            d.empty()
        };
        d.group(d.concat(&[
            d.text("<"),
            name,
            d.indent(d.concat(attr_docs)),
            before_close,
            d.text(">"),
        ]))
    }

    /// Build a **content-bearing whitespace-sensitive head** — the one shape every element
    /// whose content prints verbatim takes when it has content, mirroring the four branches
    /// prettier-plugin-svelte's `print/index.ts` selects between on the same two predicates.
    ///
    /// Read as three independent decisions:
    ///
    /// 1. **the attribute list** goes in a group of its own, from the caller's line-separated
    ///    docs, and wraps one attribute per line when it does not fit — exactly like every
    ///    other head. Rebuilding it space-separated (the shape this replaced) does not merely
    ///    lose prettier parity: it makes the head's width unbounded, since a list that cannot
    ///    wrap has nowhere to put its overflow.
    /// 2. **the opening `>`** hugs the last list member — the family's rule, not a width
    ///    accident. Only [`ContentEdges::hugs_open`] gives it a break point of its own, one
    ///    level in, and only a list ending on a `//` forces that break rather than offering it.
    /// 3. **the closing `>`** dangles behind a softline when [`ContentEdges::hugs_close`], so
    ///    the closing tag splits (`</tag⏎>`); otherwise the closing tag prints whole.
    ///
    /// Every break here lands **inside a tag**, where no character is content, so all four
    /// combinations render identically and the choice is layout. Nothing inside `>content</tag`
    /// may break, which is why the content sits in a group with no line of its own: a break
    /// between the `>` and the first content byte, or between the last and the `</`, would be
    /// rendered text.
    ///
    /// Pinned across the family by
    /// [`ws_sensitive_head_attrs_wrap`](../../../../../tests/fixtures/svelte/elements/ws_sensitive_head_attrs_wrap/)
    /// (the list), at the exact 100/101 boundary by
    /// [`textarea_attrs_long`](../../../../../tests/fixtures/svelte/elements/textarea_attrs_long/),
    /// and over both edges by
    /// [`ws_sensitive_head_content_edges`](../../../../../tests/fixtures/svelte/elements/ws_sensitive_head_content_edges/).
    pub(super) fn build_ws_sensitive_head_with_content_doc(
        &self,
        name: DocId,
        attr_docs: &[DocId],
        emission: AttrListEmission,
        content: DocId,
        edges: ContentEdges,
    ) -> DocId {
        let d = self.d();

        if !edges.hugs_open {
            // The `>` glues to whatever the list ends on, so the head is the ordinary
            // hug-the-`>` open tag and the content follows it directly.
            let head = self.build_ws_sensitive_open_tag_doc(name, attr_docs, emission);
            return if edges.hugs_close {
                let body = d.group(d.concat(&[content, d.text("</"), name]));
                d.group(d.concat(&[head, body, d.softline(), d.text(">")]))
            } else {
                d.concat(&[head, content, self.end_tag(name)])
            };
        }

        let attrs = self.ws_sensitive_attrs_group(attr_docs);
        let before_gt = if emission.ends_with_line_comment {
            d.hardline()
        } else {
            d.softline()
        };

        // ⚠️ Whether the `>`'s break point gets a **group of its own** is the closing edge's
        // second consequence, and it is what decides the `>` in the mixed cell. With the
        // closing tag split, `>content</tag` is a unit that either fits on the list's last
        // line or does not, so it owns the decision. With the closing tag whole there is
        // nothing after the content for that break to serve, so the break rides the head's
        // own group and moves with it — the `>` takes its own line exactly when the list
        // wrapped. Prettier draws the same line by wrapping `[softline, …]` in a group in its
        // `hugStart && hugEnd` branch and leaving it bare in the `hugStart`-only one.
        if edges.hugs_close {
            let interior = d.group(d.concat(&[d.text(">"), content, d.text("</"), name]));
            let hugged = d.group(d.indent(d.concat(&[before_gt, interior])));
            return d.group(d.concat(&[
                d.text("<"),
                name,
                attrs,
                hugged,
                d.softline(),
                d.text(">"),
            ]));
        }
        let interior = d.group(d.concat(&[d.text(">"), content]));
        let hugged = d.indent(d.concat(&[before_gt, interior]));
        d.group(d.concat(&[d.text("<"), name, attrs, hugged, self.end_tag(name)]))
    }

    /// Build content for whitespace-sensitive elements (pre, textarea).
    ///
    /// Text nodes preserve their exact whitespace (significant for pre/textarea).
    /// Expressions, blocks, and other dynamic content are formatted normally
    /// (their internal whitespace is not significant).
    pub(super) fn build_whitespace_sensitive_content_doc(
        &self,
        nodes: &[FragmentNode<'_>],
    ) -> DocId {
        // Whitespace is significant here (`<pre>`/`<textarea>`): a block must not
        // dangle its `}` or expand its body — that would inject rendered whitespace.
        // The dedicated ws-sensitive if/each builders already hug; this also gates
        // await/key/snippet, which fall through to the normal (dangling) builders.
        let prev_dangle = self.set_block_dangle_allowed(false);
        let d = self.d();
        let body = d.concat_iter(
            nodes
                .iter()
                .map(|node| self.build_whitespace_sensitive_node_doc(node)),
        );
        self.set_block_dangle_allowed(prev_dangle);
        // One body-indent level per container (element body, block body), matching
        // prettier's uniform "each container adds a level" model. Preserved text has
        // no doc-hardlines so this never injects rendered whitespace into <pre> — it
        // only accumulates the depth that nested elements' wrapped attributes and
        // dangling `>` breaks resolve against. See nodes/element_ws_sensitive_doc.rs
        // header + docs/conformance_prettier_svelte.md §Svelte: Elements.
        d.indent(body)
    }

    /// Build doc for a single node in whitespace-sensitive context.
    ///
    /// - **Text**: preserve raw whitespace (significant in pre/textarea).
    /// - **Elements**: recursively use whitespace-sensitive formatting (e.g., `<code>` inside `<pre>`).
    /// - **If/Each blocks**: use inline ws-sensitive block formatting (no added whitespace,
    ///   body nodes formatted whitespace-sensitively).
    /// - **Expressions and other blocks**: format normally; the per-container body-indent
    ///   level is applied collectively by `build_whitespace_sensitive_content_doc`, not here.
    fn build_whitespace_sensitive_node_doc(&self, node: &FragmentNode<'_>) -> DocId {
        let d = self.d();
        match node {
            // Text: preserve exact whitespace (significant in pre/textarea)
            FragmentNode::Text(text) => d.source_span(text.raw_span, self.source),

            // Elements: recursively build as whitespace-sensitive. The body-indent level
            // comes from the parent's collective wrap (build_whitespace_sensitive_content_doc),
            // so no per-node wrapper here. Handles <pre><code> where <code> inherits ws preservation.
            FragmentNode::Element(element) => {
                let ws_is_html = element.kind == internal::ElementKind::Html;
                // Always use whitespace-sensitive path when nested inside whitespace-sensitive elements
                let attrs = self.build_element_attrs_doc(
                    element.attributes,
                    self.d().line(),
                    element.name_span.end,
                    element.open_tag_end,
                    ws_is_html,
                );
                self.build_whitespace_sensitive_element_doc(element, attrs)
            }
            FragmentNode::SpecialElement(element) => {
                // Special elements in whitespace-sensitive context: format normally without indent
                self.build_special_element_doc(element)
            }

            // Expressions and blocks: format normally. The body-indent level is
            // applied collectively by build_whitespace_sensitive_content_doc, so each
            // node sits at the container's body level without its own wrapper.
            FragmentNode::ExpressionTag(tag) => self.build_expression_tag_doc(tag),
            FragmentNode::Comment(comment) => self.build_html_comment_doc(comment),
            FragmentNode::IfBlock(block) => self.build_ws_sensitive_if_block_doc(block),
            FragmentNode::EachBlock(block) => self.build_ws_sensitive_each_block_doc(block),
            FragmentNode::AwaitBlock(block) => self.build_await_block_doc(block),
            FragmentNode::KeyBlock(block) => self.build_key_block_doc(block),
            FragmentNode::SnippetBlock(block) => self.build_snippet_block_doc(block),
            FragmentNode::HtmlTag(tag) => self.build_html_tag_doc(tag),
            FragmentNode::ConstTag(tag) => self.build_const_tag_doc(tag),
            FragmentNode::DeclarationTag(tag) => self.build_declaration_tag_doc(tag),
            FragmentNode::DebugTag(tag) => self.build_debug_tag_doc(tag),
            FragmentNode::RenderTag(tag) => self.build_render_tag_doc(tag),
        }
    }

    /// Build if block doc for whitespace-sensitive context (inside `<pre>`).
    ///
    /// Emits block structure inline without added whitespace. Body nodes are
    /// formatted with whitespace-sensitive content formatting to preserve
    /// significant whitespace.
    fn build_ws_sensitive_if_block_doc(&self, block: &internal::IfBlock<'_>) -> DocId {
        let d = self.d();
        // Pass false for in_multiline_context: inside whitespace-sensitive elements,
        // block expressions must not wrap (adding line breaks changes visible content)
        let head = self.build_block_head_expr(
            IF_BLOCK_OPEN,
            block.opening_tag_span,
            &block.test,
            block.opening_tag_span.end - 1,
            false,
        );

        let body_doc = self.build_whitespace_sensitive_content_doc(block.consequent.nodes);

        // The `}` hugs the frozen slice's last line here, as it does on every
        // dangle-suppressed path — inside a whitespace-significant element the dangle is
        // off by construction (`block_dangle_allowed`).
        let open_doc = self.head_open_doc(IF_BLOCK_OPEN, head.layout.opens_own_line());
        let mut parts: DocBuf = smallvec![open_doc, head.doc, d.text("}"), body_doc];

        if let Some(alt) = &block.alternate {
            self.build_ws_sensitive_if_alternate(alt, &mut parts);
        }

        parts.push(d.text("{/if}"));
        d.concat(&parts)
    }

    /// Build if alternate (else/else-if) for whitespace-sensitive context.
    fn build_ws_sensitive_if_alternate(&self, alt: &Fragment<'_>, parts: &mut DocBuf) {
        let d = self.d();

        // Check if this can be flattened to {:else if ...}
        if let Some(else_if) = Self::get_flattenable_else_if(alt) {
            let head = self.build_else_if_expr_doc(else_if, false);

            let body_doc = self.build_whitespace_sensitive_content_doc(else_if.consequent.nodes);
            parts.push(self.head_open_doc(ELSE_IF_BLOCK_OPEN, head.layout.opens_own_line()));
            parts.push(head.doc);
            parts.push(d.text("}"));
            parts.push(body_doc);

            if let Some(nested_alt) = &else_if.alternate {
                self.build_ws_sensitive_if_alternate(nested_alt, parts);
            }
            return;
        }

        // Plain {:else}
        let body_doc = self.build_whitespace_sensitive_content_doc(alt.nodes);
        parts.push(d.text("{:else}"));
        parts.push(body_doc);
    }

    /// Build each block doc for whitespace-sensitive context (inside `<pre>`).
    ///
    /// Emits block structure inline without added whitespace. Body nodes are
    /// formatted with whitespace-sensitive content formatting.
    fn build_ws_sensitive_each_block_doc(&self, block: &internal::EachBlock<'_>) -> DocId {
        let d = self.d();
        let expr_comment_end = each_expr_comment_end(block);
        // Pass false for in_multiline_context: expressions must not wrap in ws-sensitive context
        let head = self.build_block_head_expr(
            EACH_BLOCK_OPEN,
            block.opening_tag_span,
            &block.expression,
            expr_comment_end,
            false,
        );

        let open_doc = self.head_open_doc(EACH_BLOCK_OPEN, head.layout.opens_own_line());
        let mut opening: DocBuf = smallvec![open_doc, head.doc];

        if let Some(context) = &block.context {
            opening.push(d.text(" as "));
            let pattern_doc = self.build_pattern_doc(context);
            opening.push(pattern_doc);
            if let Some(index) = block.index {
                opening.push(d.text(", "));
                opening.push(d.text_pooled(index));
            }
        } else if let Some(index) = block.index {
            opening.push(d.text(", "));
            opening.push(d.text_pooled(index));
        }

        if let Some(key) = &block.key {
            // The `(` carries no trailing space, and the dangle is suppressed here, so the
            // key's freeze verdict changes nothing about this layout.
            let key_doc = self.build_each_key_expr(key, false).doc;
            opening.push(d.text(" ("));
            opening.push(key_doc);
            opening.push(d.text(")"));
        }

        opening.push(d.text("}"));

        let body_doc = self.build_whitespace_sensitive_content_doc(block.body.nodes);

        let opening_concat = d.concat(&opening);
        let mut parts: DocBuf = smallvec![opening_concat, body_doc];

        if let Some(fallback) = &block.fallback {
            let fallback_doc = self.build_whitespace_sensitive_content_doc(fallback.nodes);
            parts.push(d.text("{:else}"));
            parts.push(fallback_doc);
        }

        parts.push(d.text("{/each}"));
        d.concat(&parts)
    }
}
