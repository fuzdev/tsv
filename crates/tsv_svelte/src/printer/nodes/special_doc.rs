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

use super::element_doc::{ElementKind, ElementLayout, ElementParts};

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
            ElementLayout::Empty => self.build_special_empty_doc(element, tag_name, &attr_docs),
            ElementLayout::WithContent(boundary) => {
                self.build_content_element_doc(&parts, &ctx, &attr_docs, boundary)
            }
        }
    }

    /// Build `<tag></tag>` for a special element with no content, wrapping the attributes in the
    /// three-state conditional group (all inline / attrs inline + `>` on its own line / attrs
    /// wrapped) when it has any.
    fn build_special_empty_doc(
        &self,
        element: &internal::SpecialElement<'_>,
        tag_name: &'static str,
        attr_docs: &[DocId],
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
        // Pre-allocate: 2 docs per attr (separator + attr), plus potential this={} attr
        let has_this = matches!(
            element.kind,
            SpecialElementKind::SvelteComponent { .. } | SpecialElementKind::SvelteElement { .. }
        );
        let capacity = (element.attributes.len() + usize::from(has_this)) * 2;
        let mut docs: DocBuf = DocBuf::with_capacity(capacity);

        // Add this={...} for component/element
        match &element.kind {
            SpecialElementKind::SvelteComponent { expression } => {
                docs.push(separator);
                docs.push(self.build_this_attr_doc_for_inline(expression));
            }
            SpecialElementKind::SvelteElement { tag } => {
                docs.push(separator);
                docs.push(self.build_this_attr_doc_for_inline(tag));
            }
            _ => {}
        }

        // svelte:element renders as HTML, so normalize class attribute whitespace
        let normalize_class = matches!(element.kind, SpecialElementKind::SvelteElement { .. });
        self.push_attrs_with_comments(
            &mut docs,
            element.attributes,
            separator,
            element.name_span.end,
            element.open_tag_end,
            normalize_class,
        );

        docs
    }

    /// Build doc for this={expression} attribute (for inline doc building)
    fn build_this_attr_doc_for_inline(&self, expr: &tsv_ts::Expression<'_>) -> DocId {
        let d = self.d();
        use tsv_ts::ast::internal::{Expression, LiteralValue};

        // Handle plain string attribute: this="value" (no braces in source)
        // Distinguished from expression form this={"value"} by checking source:
        // - Plain string: span covers `hello` (no quote at span start)
        // - Expression: span covers `"hello"` (quote char at span start)
        if let Expression::Literal(lit) = expr
            && let LiteralValue::String(cooked) = &lit.value
        {
            let first_byte = self.source.as_bytes().get(lit.span.start as usize).copied();
            if first_byte != Some(b'"') && first_byte != Some(b'\'') {
                let content = cooked.resolve(lit.span, self.source);
                // Same delimiter rule as quoted attribute values: content holding a
                // literal `"` takes single quotes (double quotes cannot hold it —
                // HTML §13.1.2.3), else double. Plain-string `this=` content carries at
                // most one literal quote kind, so single quotes are lossless here too.
                let (open, close) = if content.contains('"') {
                    ("this='", '\'')
                } else {
                    ("this=\"", '"')
                };
                let mut w = d.pool_writer();
                w.push_str(open);
                w.push_str(content);
                w.push(close);
                return w.finish_text();
            }
        }

        // Expression (including braced string literals): this={expr}
        let expr_doc_id = self.build_ts_expression_doc_no_comments(expr);
        d.concat(&[d.text("this={"), expr_doc_id, d.text("}")])
    }
}
