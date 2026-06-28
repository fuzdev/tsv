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
use super::helpers::each_expr_comment_end;
use crate::ast::internal::{self, Fragment, FragmentNode};
use crate::printer::Printer;
use tsv_lang::doc::{DocBuf, arena::DocId};
use tsv_lang::{SymbolResolver, SymbolToU32};

impl<'a> Printer<'a> {
    /// Build doc for whitespace-sensitive elements (pre, textarea, etc.)
    ///
    /// These elements preserve text whitespace exactly as-is, but still format
    /// expressions, blocks, and other dynamic content normally.
    ///
    /// Behavior differs by content type:
    /// - **Inline with multiline content** (e.g., `<span>` inside `<pre>` with `\n` in text):
    ///   break `>` to new line at indent+2, preserve content literally. Only when first
    ///   text starts with non-whitespace (space/newline keeps `>` inline).
    /// - **Inline with single-line content and attrs** (textarea with content): keep attrs
    ///   inline, wrap `>content</tag` together based on width.
    /// - **Block with simple content** (pre with single expression): break `>` when
    ///   attrs + content would exceed print width.
    /// - **Fallback**: hug `>` with attrs (block) or break `>` on wrap (inline empty).
    pub(super) fn build_whitespace_sensitive_element_doc(
        &self,
        tag_name: &str,
        element: &internal::Element<'_>,
        attr_docs: DocBuf,
    ) -> DocId {
        let d = self.d();
        let tag_sym = element.name.to_u32();
        let is_inline = !tsv_html::is_block_element(tag_name);
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
                        starts_with_ws = raw.starts_with(|c: char| c.is_ascii_whitespace());
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
        //   inline + multiline content              → break `>` to its own line (2 levels)
        //   inline + single-line content + attrs    → if_break: hug `>content` flat, else break `>`
        //   block  + content + attrs (simple expr)  → hug `>` with the last attr
        //   inline + empty + attrs                  → self-closing `/>` drops; explicit `></tag>` hugs unless overflow
        //   no attrs                                → `<tag>`
        //   block, otherwise (empty/complex) + attrs → hug `>`, tolerating overflow
        //
        // Inline elements with multiline content inside whitespace-sensitive context:
        // Always break `>` to new line (indented 2 levels), preserve content literally.
        // Attrs stay inline if short, wrap to separate lines if long.
        // Example: <pre><span attr="val"\n\t\t>text\n</span></pre>
        if is_inline && content_has_newlines {
            let content_doc = self.build_whitespace_sensitive_content_doc(element.fragment.nodes);

            // Indents below are relative to this element's own (ambient) doc-indent,
            // which its parent's body wrap already set to the element's nesting depth.
            // When content doesn't end with \n, the closing </tag> has its `>` split
            // to a new line at the element level: `line2</span\n\t>`.
            let closing = if last_text_ends_with_newline {
                self.end_tag(tag_sym)
            } else {
                // </tag\n> — closing > on its own line at the element's level
                d.concat(&[d.text("</"), d.symbol(tag_sym), d.hardline(), d.text(">")])
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
                d.symbol(tag_sym),
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
        if is_inline && has_content && !attr_docs.is_empty() {
            let content_doc = self.build_whitespace_sensitive_content_doc(element.fragment.nodes);
            // Rebuild as space-separated (caller passes line-separated which we can't use here)
            let space_attrs = self.build_element_attrs_doc(
                element.attributes,
                self.d().text(" "),
                element.name_span.end,
                element.open_tag_end,
                is_html,
            );

            // In break mode: \n\t>content</tag (closing > handled by outer group)
            let break_doc = d.indent(d.concat(&[
                d.hardline(),
                d.text(">"),
                content_doc,
                d.text("</"),
                d.symbol(tag_sym),
            ]));
            // In flat mode: >content</tag (no closing > — it's outside the group)
            let flat_doc = d.concat(&[d.text(">"), content_doc, d.text("</"), d.symbol(tag_sym)]);
            let if_break = d.if_break(break_doc, flat_doc);
            let inner = d.group(d.concat(&[
                d.text("<"),
                d.symbol(tag_sym),
                d.concat(&space_attrs),
                if_break,
            ]));
            // Outer group: closing `>` with softline breaks to new line at boundary.
            // Inner group stays flat when attrs+content fit, outer breaks only for the `>`.
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
                    d.symbol(tag_sym),
                    d.text(">"),
                ]));

                // Outer group decides if attrs need to break
                let dedented = d.dedent(closing_and_content);
                let attr_concat = d.concat(&attr_docs);
                let indented = d.indent(d.concat(&[attr_concat, dedented]));
                return d.group(d.concat(&[d.text("<"), d.symbol(tag_sym), indented]));
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
            if self.span_was_self_closing(element.span) {
                // line() is a space when flat (`<tag attrs />`), a newline when the outer
                // group breaks. Mirrors build_void_element_doc.
                return d.group(d.concat(&[
                    d.text("<"),
                    d.symbol(tag_sym),
                    attr_indent,
                    d.line(),
                    d.text("/>"),
                ]));
            }
            // group(['>', '</tag']): the final `>` is appended outside, so the softline's
            // fits() weighs `></tag>` and the trailing suffix together.
            let close_seq = d.group(d.concat(&[d.text(">"), d.text("</"), d.symbol(tag_sym)]));
            let hugged = d.group(d.concat(&[d.softline(), close_seq]));
            return d.group(d.concat(&[
                d.text("<"),
                d.symbol(tag_sym),
                attr_indent,
                hugged,
                d.text(">"),
            ]));
        }

        // Build opening tag
        let opening_tag = if attr_docs.is_empty() {
            self.start_tag(tag_sym)
        } else {
            // Block whitespace-sensitive elements (pre): hug `>` with the last attr when
            // attrs wrap (prettier tolerates the overflow rather than breaking `>`).
            let attr_concat = d.concat(&attr_docs);
            let attr_indent = d.indent(attr_concat);
            d.group(d.concat(&[d.text("<"), d.symbol(tag_sym), attr_indent, d.text(">")]))
        };

        // Build content preserving text whitespace but formatting expressions/blocks
        let content_doc = self.build_whitespace_sensitive_content_doc(element.fragment.nodes);

        d.concat(&[
            opening_tag,
            content_doc,
            d.text("</"),
            d.symbol(tag_sym),
            d.text(">"),
        ])
    }

    /// Build content for whitespace-sensitive elements (pre, textarea).
    ///
    /// Text nodes preserve their exact whitespace (significant for pre/textarea).
    /// Expressions, blocks, and other dynamic content are formatted normally
    /// (their internal whitespace is not significant).
    fn build_whitespace_sensitive_content_doc(&self, nodes: &[FragmentNode<'_>]) -> DocId {
        // Whitespace is significant here (`<pre>`/`<textarea>`): a block must not
        // dangle its `}` or expand its body — that would inject rendered whitespace.
        // The dedicated ws-sensitive if/each builders already hug; this also gates
        // await/key/snippet, which fall through to the normal (dangling) builders.
        let prev_dangle = self.set_block_dangle_allowed(false);
        let node_docs: Vec<_> = nodes
            .iter()
            .map(|node| self.build_whitespace_sensitive_node_doc(node))
            .collect();
        self.set_block_dangle_allowed(prev_dangle);
        // One body-indent level per container (element body, block body), matching
        // prettier's uniform "each container adds a level" model. Preserved text has
        // no doc-hardlines so this never injects rendered whitespace into <pre> — it
        // only accumulates the depth that nested elements' wrapped attributes and
        // dangling `>` breaks resolve against. See nodes/element_ws_sensitive_doc.rs
        // header + docs/conformance_prettier.md §Svelte.
        let d = self.d();
        d.indent(d.concat(&node_docs))
    }

    /// Build doc for a single node in whitespace-sensitive context.
    ///
    /// - **Text**: preserve raw whitespace (significant in pre/textarea).
    /// - **Elements**: recursively use whitespace-sensitive formatting (e.g., `<code>` inside `<pre>`).
    /// - **If/Each blocks**: use inline ws-sensitive block formatting (no added whitespace,
    ///   body nodes formatted whitespace-sensitively).
    /// - **Expressions and other blocks**: format normally WITH indent wrapper (double-indented:
    ///   once for being inside `<pre>`, once for internal structure).
    fn build_whitespace_sensitive_node_doc(&self, node: &FragmentNode<'_>) -> DocId {
        let d = self.d();
        match node {
            // Text: preserve exact whitespace (significant in pre/textarea)
            FragmentNode::Text(text) => d.text_owned(text.raw(self.source).to_string()),

            // Elements: recursively build as whitespace-sensitive (no indent wrapper needed -
            // the element's own indentation logic handles it)
            // This handles cases like <pre><code> where <code> inherits whitespace preservation
            FragmentNode::Element(element) => {
                let tag_name = self.resolve_symbol(element.name);
                let ws_is_html = element.kind == internal::ElementKind::Html;
                // Always use whitespace-sensitive path when nested inside whitespace-sensitive elements
                let attr_docs = self.build_element_attrs_doc(
                    element.attributes,
                    self.d().line(),
                    element.name_span.end,
                    element.open_tag_end,
                    ws_is_html,
                );
                self.build_whitespace_sensitive_element_doc(&tag_name, element, attr_docs)
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

    /// Build if block doc for whitespace-sensitive context (inside <pre>).
    ///
    /// Emits block structure inline without added whitespace. Body nodes are
    /// formatted with whitespace-sensitive content formatting to preserve
    /// significant whitespace.
    fn build_ws_sensitive_if_block_doc(&self, block: &internal::IfBlock<'_>) -> DocId {
        let d = self.d();
        // Pass false for in_multiline_context: inside whitespace-sensitive elements,
        // block expressions must not wrap (adding line breaks changes visible content)
        let expr_doc = self.build_block_head_expr(
            IF_BLOCK_OPEN,
            block.opening_tag_span,
            &block.test,
            block.opening_tag_span.end - 1,
            false,
        );

        let body_doc = self.build_whitespace_sensitive_content_doc(block.consequent.nodes);

        let mut parts = vec![d.text(IF_BLOCK_OPEN), expr_doc, d.text("}"), body_doc];

        if let Some(alt) = &block.alternate {
            self.build_ws_sensitive_if_alternate(alt, &mut parts);
        }

        parts.push(d.text("{/if}"));
        d.concat(&parts)
    }

    /// Build if alternate (else/else-if) for whitespace-sensitive context.
    fn build_ws_sensitive_if_alternate(&self, alt: &Fragment<'_>, parts: &mut Vec<DocId>) {
        let d = self.d();

        // Check if this can be flattened to {:else if ...}
        if let Some(else_if) = Self::get_flattenable_else_if(alt) {
            let expr_doc = self.build_else_if_expr_doc(else_if, false);

            let body_doc = self.build_whitespace_sensitive_content_doc(else_if.consequent.nodes);
            parts.push(d.text(ELSE_IF_BLOCK_OPEN));
            parts.push(expr_doc);
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

    /// Build each block doc for whitespace-sensitive context (inside <pre>).
    ///
    /// Emits block structure inline without added whitespace. Body nodes are
    /// formatted with whitespace-sensitive content formatting.
    fn build_ws_sensitive_each_block_doc(&self, block: &internal::EachBlock<'_>) -> DocId {
        let d = self.d();
        let expr_comment_end = each_expr_comment_end(block);
        // Pass false for in_multiline_context: expressions must not wrap in ws-sensitive context
        let expr_doc = self.build_block_head_expr(
            EACH_BLOCK_OPEN,
            block.opening_tag_span,
            &block.expression,
            expr_comment_end,
            false,
        );

        let mut opening = vec![d.text(EACH_BLOCK_OPEN), expr_doc];

        if let Some(context) = &block.context {
            opening.push(d.text(" as "));
            let pattern_doc = self.build_pattern_doc(context);
            opening.push(pattern_doc);
            if let Some(index) = block.index {
                opening.push(d.text(", "));
                opening.push(d.text_owned(index.to_string()));
            }
        } else if let Some(index) = block.index {
            opening.push(d.text(", "));
            opening.push(d.text_owned(index.to_string()));
        }

        if let Some(key) = &block.key {
            let key_doc = if let Some(key_span) = block.key_span {
                self.build_expression_doc_for_block(
                    key,
                    key_span.start + 1,
                    key_span.end - 1,
                    1,
                    false,
                )
            } else {
                self.build_ts_expression_doc(key)
            };
            opening.push(d.text(" ("));
            opening.push(key_doc);
            opening.push(d.text(")"));
        }

        opening.push(d.text("}"));

        let body_doc = self.build_whitespace_sensitive_content_doc(block.body.nodes);

        let opening_concat = d.concat(&opening);
        let mut parts = vec![opening_concat, body_doc];

        if let Some(fallback) = &block.fallback {
            let fallback_doc = self.build_whitespace_sensitive_content_doc(fallback.nodes);
            parts.push(d.text("{:else}"));
            parts.push(fallback_doc);
        }

        parts.push(d.text("{/each}"));
        d.concat(&parts)
    }
}
