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
use super::element_doc::ElementAttrsDoc;
use super::helpers::each_expr_comment_end;
use crate::ast::internal::{self, Fragment, FragmentNode, is_collapsible_ws_char};
use crate::printer::Printer;
use smallvec::smallvec;
use tsv_lang::doc::{DocBuf, arena::DocId};

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
    /// Behavior differs by content type:
    /// - **Inline with multiline content** (e.g., `<span>` inside `<pre>` with `\n` in text):
    ///   break `>` to its own line one level past the element (attr indent), preserve content
    ///   literally. Only when first text starts with non-whitespace (space/newline keeps `>` inline).
    /// - **Inline with single-line content and attrs** (textarea with content): keep attrs
    ///   inline, wrap `>content</tag` together based on width.
    /// - **Block with simple content** (pre with single expression): break `>` when
    ///   attrs + content would exceed print width.
    /// - **Inline empty with attrs**: self-closing `/>` drops on wrap; explicit `></tag>` hugs.
    /// - **Fallback**: block hugs `>` with the last attr; no attrs → plain `<tag>`.
    ///
    /// Every one of those hugs is a deliberate refusal to break before the `>`: a break there
    /// is free for an ordinary element but here it borders literal content. The
    /// [`AttrListEmission`](super::element_doc::AttrListEmission) from the one attribute-list emitter
    /// ([`Printer::push_attrs_with_comments`]) carries the two comment facts that override
    /// it, each selecting a shape that is already pinned elsewhere, not a new one. A list
    /// **ending on a `//`** cannot share its line with the `>` (a line comment runs to end
    /// of line — a hugged `>` lands *inside* it and the output stops re-parsing), so the `>`
    /// breaks — [`ws_sensitive_attr_comment_line`](../../../../../tests/fixtures/svelte/elements/ws_sensitive_attr_comment_line_prettier_divergence/).
    /// A list a comment **holds a hardline in** cannot render flat at any width, so the
    /// attributes wrap one per line —
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
        let name_doc = d.source_span_ident(element.name_span);

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
        let is_html = element.kind == internal::ElementKind::Html;
        let has_content = !element.fragment.nodes.is_empty();

        // Analyze text nodes in one pass for multiline content detection.
        // When an inline element inside <pre> has multiline content that starts with
        // visible text (not whitespace), the `>` must break to a new line.
        // If content starts with whitespace (space or newline), `>` stays inline:
        // - `<code>\ncontent` → stays inline (\n naturally separates)
        // - `<span> {expr}\n` → stays inline (space provides natural break)
        // - `<span>text\n` → `>` breaks (non-whitespace directly after `>`)
        //
        // Also tracks whether the last text node ends with \n (used for closing tag).
        let (content_has_newlines, last_text_ends_with_newline) = if has_content {
            let mut is_first_node = true;
            let mut starts_with_ws = false;
            let mut has_newline = false;
            let mut last_ends_newline = true;
            for node in element.fragment.nodes {
                if let FragmentNode::Text(text) = node {
                    let raw = text.raw(self.source);
                    if is_first_node {
                        // Deliberately NOT `Printer::text_glued_before` (whose spelling this is the
                        // negation of), for the same reason the `is_inline` probe above is its own
                        // question: that predicate means "no break may land here, because one would
                        // inject a *collapsible* space", and inside a whitespace-PRESERVING subtree
                        // this whitespace is literal content — a break here injects a literal
                        // newline. Same character class, different claim; folding them would let a
                        // change to one silently retarget the other.
                        starts_with_ws = raw.starts_with(is_collapsible_ws_char);
                    }
                    if raw.contains('\n') {
                        has_newline = true;
                    }
                    last_ends_newline = raw.trim_end_matches([' ', '\t']).ends_with('\n');
                }
                is_first_node = false;
            }
            (!starts_with_ws && has_newline, last_ends_newline)
        } else {
            (false, true)
        };

        // Opening-tag layout splits on (is_inline, has_content, has-attrs). Each arm
        // below returns, so order matters. Cases:
        //   inline + multiline content              → break `>` to its own line (attr indent)
        //   inline + single-line content + attrs    → if_break: hug `>content` flat, else break `>`
        //   block  + content + attrs (simple expr)  → hug `>` with the last attr
        //   inline + empty + attrs                  → self-closing `/>` drops; explicit `></tag>` hugs unless overflow
        //   no attrs                                → `<tag>`
        //   block, otherwise (empty/complex) + attrs → hug `>`, tolerating overflow
        //
        // Inline elements with multiline content inside whitespace-sensitive context:
        // break `>` to its own line one level past the element (attr indent), preserve
        // content literally. Attrs stay inline if short, wrap to separate lines if long.
        // Example: <pre><span attr="val"\n\t\t>text\n</span></pre>
        if is_inline && content_has_newlines {
            let content_doc = self.build_whitespace_sensitive_content_doc(element.fragment.nodes);

            // Indents below are relative to this element's own (ambient) doc-indent,
            // which its parent's body wrap already set to the element's nesting depth.
            // When content doesn't end with \n, the closing </tag> has its `>` split
            // to a new line at the element level: `line2</span\n\t>`.
            let closing = if last_text_ends_with_newline {
                self.end_tag(name_doc)
            } else {
                // </tag\n> — closing > on its own line at the element's level
                d.concat(&[d.text("</"), name_doc, d.hardline(), d.text(">")])
            };

            // Opening `>` at element level + 1 (attr indent). Attrs (if any) go in a
            // group at the same level — flat when short, wrapped when long.
            let opening_break = d.concat(&[d.hardline(), d.text(">")]);
            let opening_inner = if attr_docs.is_empty() {
                opening_break
            } else {
                let attr_group = d.group(d.concat(&attr_docs));
                d.concat(&[attr_group, opening_break])
            };

            return d.concat(&[
                d.text("<"),
                name_doc,
                d.indent(opening_inner),
                content_doc,
                closing,
            ]);
        }

        // Inline whitespace-sensitive elements with content and attrs (textarea with content)
        // have special formatting that depends on whether attrs fit on one line:
        // - If fits: <tag attrs>content</tag>
        // - If breaks: <tag attrs\n\t>content</tag\n>
        //
        // This preserves no leading whitespace before content while allowing attrs to stay inline when short.
        //
        // The closing `>` of `</tag>` is outside the group so fits() doesn't count it.
        // At the boundary (e.g. 100 chars), `<tag attr>content</tag` fits but adding `>`
        // would be 101. The softline puts `>` on its own line in that case.
        //
        // A **block** element (`<pre>`) whose list ends on a `//` comes here too, rather than
        // to the hugging arm below: this is already the shape for "the `>` cannot share the
        // attribute line but the content must still start right after it", and the two
        // whitespace-sensitive tags answering that with different layouts would be a
        // distinction with no source in the elements.
        if (is_inline || emission.ends_with_line_comment) && has_content && !attr_docs.is_empty() {
            let content_doc = self.build_whitespace_sensitive_content_doc(element.fragment.nodes);

            let inner = if emission.has_hardline {
                // A comment holds a line of its own inside the list, so the head can never
                // render flat and the flat machinery below has nothing to decide: the
                // attributes wrap one per line — the line-separated docs the caller already
                // built, indented — and the `>` hugs the last one, prettier's own shape here.
                // A list ending on a `//` is the one thing nothing may share a line with, so
                // there the `>` takes the next line one level in instead — the same slot the
                // flat path's break form puts it in. Feeding these lists to the
                // space-separated rebuild below instead is how a broken head rendered its
                // remaining attributes at ambient indent: a hardline inside a plain concat
                // has no indent to land in.
                let close_sep = if emission.ends_with_line_comment {
                    d.hardline()
                } else {
                    d.empty()
                };
                d.concat(&[
                    d.text("<"),
                    name_doc,
                    d.indent(d.concat(&[
                        d.concat(&attr_docs),
                        close_sep,
                        d.text(">"),
                        content_doc,
                        d.text("</"),
                        name_doc,
                    ])),
                ])
            } else {
                // Rebuild as space-separated (caller passes line-separated which we can't
                // use here: behind this arm's own group they would wrap on width, and a
                // whitespace-sensitive head holds its attributes flat, tolerating overflow).
                let space_attrs = self
                    .build_element_attrs_doc(
                        element.attributes,
                        self.d().text(" "),
                        element.name_span.end,
                        element.open_tag_end,
                        is_html,
                    )
                    .docs;

                // In break mode: \n\t>content</tag (closing > handled by outer group)
                let break_doc = d.indent(d.concat(&[
                    d.hardline(),
                    d.text(">"),
                    content_doc,
                    d.text("</"),
                    name_doc,
                ]));
                // In flat mode: >content</tag (no closing > — it's outside the group)
                let flat_doc = d.concat(&[d.text(">"), content_doc, d.text("</"), name_doc]);
                let if_break = d.if_break(break_doc, flat_doc);
                d.group(d.concat(&[d.text("<"), name_doc, d.concat(&space_attrs), if_break]))
            };

            // Outer group: closing `>` with softline breaks to new line at boundary — for
            // the flat form only at the width boundary, for the wrapped form always (its
            // hardlines break this group).
            let sl = d.softline();
            return d.group(d.concat(&[inner, sl, d.text(">")]));
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
        // (Block whitespace-sensitive elements like `<pre>` always hug `>`; see the
        // `else` branch below — prettier never breaks `>` there, tolerating overflow.)
        if is_inline && !has_content && !attr_docs.is_empty() {
            let attr_indent = d.indent(d.group(d.concat(&attr_docs)));
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

        // Build opening tag
        let opening_tag = if attr_docs.is_empty() {
            self.start_tag(name_doc)
        } else {
            // Block whitespace-sensitive elements (pre): hug `>` with the last attr when
            // attrs wrap (prettier tolerates the overflow rather than breaking `>`). Only a
            // trailing `//` overrides that, and only the empty-content shapes reach here with
            // one — anything with content took the break arm above. With no content there is
            // nothing for the `>` to be adjacent to, so it drops to base indent, which is
            // where every other element puts it.
            let attr_concat = d.concat(&attr_docs);
            let attr_indent = d.indent(attr_concat);
            let before_close = if emission.ends_with_line_comment {
                d.hardline()
            } else {
                d.empty()
            };
            d.group(d.concat(&[
                d.text("<"),
                name_doc,
                attr_indent,
                before_close,
                d.text(">"),
            ]))
        };

        // Build content preserving text whitespace but formatting expressions/blocks
        let content_doc = self.build_whitespace_sensitive_content_doc(element.fragment.nodes);

        d.concat(&[opening_tag, content_doc, self.end_tag(name_doc)])
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
        let node_docs: DocBuf = nodes
            .iter()
            .map(|node| self.build_whitespace_sensitive_node_doc(node))
            .collect();
        self.set_block_dangle_allowed(prev_dangle);
        // One body-indent level per container (element body, block body), matching
        // prettier's uniform "each container adds a level" model. Preserved text has
        // no doc-hardlines so this never injects rendered whitespace into <pre> — it
        // only accumulates the depth that nested elements' wrapped attributes and
        // dangling `>` breaks resolve against. See nodes/element_ws_sensitive_doc.rs
        // header + docs/conformance_prettier_svelte.md §Svelte: Elements.
        let d = self.d();
        d.indent(d.concat(&node_docs))
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
        let open_doc = self.head_open_doc(IF_BLOCK_OPEN, head.frozen);
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
            parts.push(self.head_open_doc(ELSE_IF_BLOCK_OPEN, head.frozen));
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

        let open_doc = self.head_open_doc(EACH_BLOCK_OPEN, head.frozen);
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
            let key_doc = if let Some(key_span) = block.key_span {
                self.build_expression_doc_for_block(
                    key,
                    key_span.start + 1,
                    key_span.end - 1,
                    1,
                    false,
                )
                .doc
            } else {
                self.build_ts_expression_doc_cannot_hang(key)
            };
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
