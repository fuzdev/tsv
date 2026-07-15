// Doc-based formatting for Svelte special elements
//
// Handles svelte:* elements:
// - svelte:component, svelte:element, svelte:self
// - svelte:window, svelte:body, svelte:document, svelte:head
// - svelte:fragment, svelte:boundary
// - slot, title

use crate::ast::internal::{self, SpecialElementKind};
use crate::printer::Printer;
use tsv_lang::doc::{DocBuf, arena::DocId};

use super::element_doc::{AttrGaps, ElementContext, ElementKind, ElementLayout, ElementParts};

impl<'a> Printer<'a> {
    /// Build a doc for a special element (`<svelte:component>`, `<svelte:element>`, `<slot>`, …).
    ///
    /// Runs the same analyze → layout → build pipeline as regular elements: `svelte:*` elements
    /// only differ in how their name and attributes are built (a static tag name; a synthesized
    /// `this={…}`), so everything downstream of [`ElementParts`] is shared. They used to carry
    /// their own copies of the hug predicates and of the multiline decision, and the copies had
    /// drifted — `<slot>` never went multiline for block children
    /// (`<slot><div>a</div><div>b</div></slot>` stayed on one line, where `<span>` expands), and
    /// the special path still dangled its tag delimiters after regular elements had moved to
    /// block-style layout.
    pub(crate) fn build_special_element_doc(
        &self,
        element: &internal::SpecialElement<'_>,
    ) -> DocId {
        let tag_name = element.kind.tag_name();

        // Attribute docs (including the synthesized `this={…}` for component/element)
        let attr_docs = self.build_special_element_attrs_doc(element, self.d().line());

        let parts = ElementParts {
            name: self.d().text(tag_name),
            // Every special element is block-kind. `ElementKind::Inline` means *HTML inline flow
            // content*, whose content-boundary whitespace is preserved as a space
            // (`<span> text </span>`) — and a `svelte:*` element (or `<slot>` / `<title>`) is never
            // that. Prettier draws the same line from the other side: its `isInlineElement`
            // requires `node.type === 'RegularElement'`, so a `SlotElement` is neither inline nor
            // block there and its boundary whitespace is trimmed. Block-kind reproduces exactly
            // that: boundaries trimmed (`<slot> {x} </slot>` → `<slot>{x}</slot>`) and a leading
            // boundary break alone expands the element.
            kind: ElementKind::Block,
            is_void: false,
            // Every `svelte:*` kind may print self-closing when the source wrote it that way.
            can_self_close: true,
            attributes: element.attributes,
            nodes: element.fragment.nodes,
            span: element.span,
        };
        let ctx = self.analyze_element(&parts, &attr_docs);

        match self.compute_element_layout(&parts, &ctx) {
            // Identical shape to a regular element's `<tag … />` — `is_declaration: false`
            // (`<!DOCTYPE>` is not a `svelte:*` tag).
            ElementLayout::Void | ElementLayout::SelfClosing => {
                self.build_void_element_doc(&parts, &attr_docs, false)
            }
            ElementLayout::Empty => {
                self.build_special_empty_doc(element, tag_name, &attr_docs, &ctx)
            }
            ElementLayout::WithContent(boundary) => {
                self.build_content_element_doc(&parts, &ctx, &attr_docs, boundary)
            }
        }
    }

    /// Build `<tag></tag>` for a special element with no content, wrapping the attributes in the
    /// three-state conditional group (all inline / attrs inline + `>` on its own line / attrs
    /// wrapped) when it has any.
    ///
    /// An attribute that *itself* breaks (a line comment in `this={…}`, say) settles the
    /// layout before the group is consulted: neither the inline nor the hug state can hold
    /// it, since both put the attributes on the tag's own line. That case takes the same
    /// shape a regular block element's empty branch does — the shared
    /// [`Printer::build_opening_tag`] with a forced break.
    ///
    /// TODO: the three states below are the last layout decision the special path still
    /// makes on its own, and the drift this crate's [`ElementParts`] doc warns about has
    /// already happened here twice: `<slot>`'s multiline rule, and then `has_multiline_attr`
    /// (which this builder ignored entirely until the line-comment case above forced it —
    /// a `svelte:*` element simply never expanded for a breaking attribute). A regular
    /// block-kind element does not use a three-state group at all; it takes
    /// `build_opening_tag` + a plain group, which is what the branch above now does. Folding
    /// the rest onto that path is the obvious end state, but the hug state is pinned by
    /// [`svelte_element_hug_long_prettier_divergence`](../../../../../tests/fixtures/svelte/special_elements/svelte_element_hug_long_prettier_divergence/),
    /// so it needs a fixtures-first pass, not a delete.
    fn build_special_empty_doc(
        &self,
        element: &internal::SpecialElement<'_>,
        tag_name: &'static str,
        attr_docs: &[DocId],
        ctx: &ElementContext,
    ) -> DocId {
        let d = self.d();
        if attr_docs.is_empty() {
            return d.concat(&[
                d.text("<"),
                d.text(tag_name),
                d.text("></"),
                d.text(tag_name),
                d.text(">"),
            ]);
        }

        let closing = d.concat(&[d.text("></"), d.text(tag_name), d.text(">")]);

        if ctx.has_multiline_attr {
            let opening = self.build_opening_tag(d.text(tag_name), attr_docs, true);
            return d.group(d.concat(&[opening, closing]));
        }

        // State 1: All inline
        let attr_concat_inline = d.concat(attr_docs);
        let inline_state = d.concat(&[
            d.text("<"),
            d.text(tag_name),
            d.indent(attr_concat_inline),
            closing,
        ]);

        // State 2: Hug mode - attrs inline (space-separated), > on new line
        let hug_attrs = self.build_special_element_attrs_doc(element, self.d().text(" "));
        let hug_attrs_concat = d.concat(&hug_attrs);
        let hug_state = d.concat(&[
            d.text("<"),
            d.text(tag_name),
            hug_attrs_concat,
            d.hardline(),
            closing,
        ]);

        // State 3: Full multiline - attrs on separate lines
        let attr_concat_multiline = d.concat(attr_docs);
        let multiline_state = d.concat(&[
            d.text("<"),
            d.text(tag_name),
            d.indent(attr_concat_multiline),
            d.hardline(),
            closing,
        ]);

        d.conditional_group(&[inline_state, hug_state, multiline_state])
    }

    /// Build docs for special element attributes.
    ///
    /// `separator`: emitted between attributes — `d.line()` for the wrapping
    /// (line-separated) layout, `d.text(" ")` for hug mode (space-separated).
    pub(crate) fn build_special_element_attrs_doc(
        &self,
        element: &internal::SpecialElement<'_>,
        separator: DocId,
    ) -> DocBuf {
        // `<svelte:element>` / `<svelte:component>` carry their `this` binding in the kind
        // rather than in `attributes` — every other special element has none. The two build
        // apart because their types differ: the component's `this` is always braced, the
        // element's may be a plain string.
        //
        // `claimed` is decided here, beside the doc, because it is the same fact as which
        // form was just built: the braces the `this` doc has now printed the comments of.
        // The attribute scan below probes the whole name→`>` gap, which those braces sit
        // inside — without the claim it prints them a second time. A plain-string `this`
        // claims nothing: no braces, so no gap a comment could occupy.
        let (this_doc, claimed) = match &element.kind {
            SpecialElementKind::SvelteElement { tag } => {
                (Some(self.build_this_attr_doc_for_inline(tag)), tag.braces())
            }
            SpecialElementKind::SvelteComponent { expression } => (
                Some(self.build_this_braced_doc(expression)),
                Some(expression.span),
            ),
            _ => (None, None),
        };

        // Pre-allocate: 2 docs per attr (separator + attr), plus the synthesized `this={…}`.
        let capacity = (element.attributes.len() + usize::from(this_doc.is_some())) * 2;
        let mut docs: DocBuf = DocBuf::with_capacity(capacity);

        if let Some(this_doc) = this_doc {
            docs.push(separator);
            docs.push(this_doc);
        }

        // svelte:element renders as HTML, so normalize class attribute whitespace
        let normalize_class = matches!(element.kind, SpecialElementKind::SvelteElement { .. });
        self.push_attrs_with_comments(
            &mut docs,
            element.attributes,
            separator,
            AttrGaps {
                first_range_start: element.name_span.end,
                open_tag_end: element.open_tag_end,
                claimed,
            },
            normalize_class,
        );

        docs
    }

    /// Build `this={…}` — the braced form, shared by `<svelte:element>` and the
    /// always-braced `<svelte:component>`.
    ///
    /// Routes through [`Printer::build_expression_tag_doc`], the same emitter every other
    /// `{…}` attribute value uses, so the `{`→expression and expression→`}` gaps print
    /// rather than being skipped. Rebuilding those gaps here instead would fork the
    /// leading-comment rule at yet another site — and skipping them is exactly how this
    /// binding used to drop every comment in its expression.
    fn build_this_braced_doc(&self, tag: &internal::ExpressionTag<'_>) -> DocId {
        let d = self.d();
        d.concat(&[d.text("this="), self.build_expression_tag_doc(tag)])
    }

    /// Build doc for a `<svelte:element>` `this=` binding (for inline doc building), which
    /// unlike the component's may also be a plain string.
    fn build_this_attr_doc_for_inline(&self, this: &internal::SpecialThis<'_>) -> DocId {
        let d = self.d();

        let content = match this {
            internal::SpecialThis::Braced(tag) => return self.build_this_braced_doc(tag),
            internal::SpecialThis::Plain { content, .. } => content,
        };

        // `this="value"`: a plain HTML attribute, printed as one. Same delimiter rule as any
        // quoted attribute value: content holding a literal `"` takes single quotes (double
        // quotes cannot hold it — HTML §13.1.2.3), else double. Plain-string `this=` content
        // carries at most one literal quote kind, so single quotes are lossless here too.
        let (open, close) = if content.contains('"') {
            ("this='", '\'')
        } else {
            ("this=\"", '"')
        };
        let mut w = d.pool_writer();
        w.push_str(open);
        w.push_str(content);
        w.push(close);
        w.finish_text()
    }
}
