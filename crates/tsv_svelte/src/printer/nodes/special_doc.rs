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

use super::element_doc::{
    AttrGaps, ElementContext, ElementKind, ElementLayout, ElementParts, ThisClaim,
};

impl<'a> Printer<'a> {
    /// Build a doc for a special element (`<svelte:component>`, `<svelte:element>`, `<slot>`, …).
    ///
    /// Runs the same analyze → layout → build pipeline as regular elements: `svelte:*` elements
    /// only differ in how their name and attributes are built (a static tag name; a synthesized
    /// `this={…}`), so everything downstream of [`ElementParts`] is shared. Copies of the hug
    /// predicates and of the multiline decision here would drift — a `<slot>` that never goes
    /// multiline for block children (`<slot><div>a</div><div>b</div></slot>` on one line, where
    /// `<span>` expands), a special path that dangles its tag delimiters where regular elements
    /// are block-style.
    pub(super) fn build_special_element_doc(
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
            // A `svelte:*` element is a distinct AST node type (`SvelteElement` / `SvelteComponent`
            // / …), never a `RegularElement`, so Svelte's `can_remove_entirely` — whose container
            // arm keys on `parent.type === 'RegularElement'` — never applies, whatever `this`
            // resolves to (even `<svelte:element this="table">`). Inter-sibling whitespace stays
            // significant.
            collapses_child_ws: false,
            nodes: element.fragment.nodes,
            span: element.span,
        };
        let ctx = self.analyze_element(&parts, &attr_docs);

        // `<title>` as a head child is whitespace-SENSITIVE, so it leaves the shared pipeline
        // here — the same place `build_element_doc` dispatches `<pre>`/`<textarea>` out of it,
        // and for the same reason. `compute_element_layout` is the only stage skipped: it is
        // what trims the content boundaries, which for this one kind deletes rendered bytes.
        if element.kind.preserves_content_whitespace() && !element.fragment.nodes.is_empty() {
            return self.build_title_content_doc(
                tag_name,
                element.fragment.nodes,
                &attr_docs,
                &ctx,
            );
        }

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

    /// Build `<title>…</title>` for a **head** `<title>`, whose content prints verbatim.
    ///
    /// `<pre>`/`<textarea>` are not the whole of the compiler's whitespace exemption. A
    /// `<title>` that is a (transparent) child of `<svelte:head>` parses as a `TitleElement`,
    /// and both of Svelte's `TitleElement` visitors walk `node.fragment.nodes` **directly** —
    /// the server one wraps them in `$$renderer.title(…)`, the client one concatenates them
    /// into a `document.title` assignment — so **`clean_nodes` never runs over them**. The
    /// content boundaries, the runs around an `{expr}` tag, and a whitespace-only body all
    /// reach the served page as authored, which puts this kind in the whitespace-sensitive
    /// class rather than the trimming one.
    ///
    /// The bytes are observable, so this is content preservation and not layout taste: only
    /// `document.title`'s **getter** strips and collapses them (HTML `Document.title`); its
    /// setter does not, `HTMLTitleElement.text` returns the child text content unchanged, and
    /// `<title>` is deliberately outside the `pre`/`listing`/`textarea` set whose leading
    /// newline the parser drops.
    ///
    /// Two neighbouring facts are untouched. The **hoist** still holds — `clean_nodes` lifts a
    /// `TitleElement` out of its parent fragment, so the run between it and a *sibling* is a
    /// fragment edge and is deleted; that happens in the fragment walk, not here. And an
    /// **empty** title has no content bytes to preserve, so it keeps the shared pipeline's
    /// self-closing / empty layouts (the caller gates on that).
    ///
    /// Pinned by
    /// [`title_content_verbatim`](../../../../../tests/fixtures/svelte/special_elements/title_content_verbatim_prettier_divergence/);
    /// see [conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements).
    fn build_title_content_doc(
        &self,
        tag_name: &'static str,
        nodes: &[internal::FragmentNode<'_>],
        attr_docs: &[DocId],
        ctx: &ElementContext,
    ) -> DocId {
        let d = self.d();
        let name_doc = d.text(tag_name);
        // Attributes are a compile error on a `<title>`, so this list is empty in anything
        // Svelte accepts — but the formatter still has to print what it was handed, and
        // `build_opening_tag` is the one emitter that keeps a `//`-terminated list off the
        // `>`'s line. Its trailing break sits inside the attr group, so the `>` appended here
        // hugs a flat list and takes its own line when the list wraps.
        let opening = self.build_opening_tag(name_doc, attr_docs, ctx.has_multiline_attr);
        let content = self.build_whitespace_sensitive_content_doc(nodes);
        d.concat(&[opening, d.text(">"), content, self.end_tag(name_doc)])
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
    pub(super) fn build_special_element_attrs_doc(
        &self,
        element: &internal::SpecialElement<'_>,
        separator: DocId,
    ) -> DocBuf {
        // `<svelte:element>` / `<svelte:component>` carry their `this` binding in the kind
        // rather than in `attributes` — every other special element has none. The two build
        // apart because their types differ: the component's `this` is always braced, the
        // element's may be a plain string.
        //
        // The doc and the claim describing what it prints travel together — one `Option`, so
        // no reader downstream has to handle a state that cannot exist (a doc without its
        // claim, or the reverse). The claim is the same fact as which form was just built —
        // the value span it routes on is exactly what the doc prints — which is why it is
        // decided here beside the doc: the comments that bind the `this` (or the tag name it
        // rides behind), plus the value's interior ([`ThisClaim`]'s routing). The attribute
        // scan below probes the whole name→`>` window, which all of that sits inside, so the
        // claim is also what the scan must skip — without it every comment here prints
        // twice, once by each.
        let synthesized_this = match &element.kind {
            SpecialElementKind::SvelteElement { tag } => Some((
                self.build_this_attr_doc_for_inline(tag),
                tag.braces().unwrap_or_else(|| tag.span()),
            )),
            SpecialElementKind::SvelteComponent { expression } => {
                Some((self.build_this_braced_doc(expression), expression.span))
            }
            _ => None,
        }
        .map(|(doc, value)| {
            (
                doc,
                ThisClaim::new(element.name_span.end, value, element.attributes),
            )
        });
        let claimed = synthesized_this.map(|(_, claim)| claim);

        // Pre-allocate: 2 docs per attr (separator + attr), plus the synthesized `this={…}`.
        let capacity = (element.attributes.len() + usize::from(synthesized_this.is_some())) * 2;
        let mut docs: DocBuf = DocBuf::with_capacity(capacity);

        if let Some((this_doc, claim)) = synthesized_this {
            // Comments that bind the `this` (or trail the tag name it prints behind) are
            // printed here, through the same seam the attribute loop uses — the only site
            // that can keep them on that side of the binding. Left to the attribute list
            // they would be emitted after a binding that is not in `attributes` at all,
            // which both relocates them past it and, once the tag breaks, is not even a
            // fixed point (see `Printer::comment_starts_its_own_line`). The filter is the
            // claim itself, so this run and the scan's skip cannot disagree.
            self.push_attr_item_with_leading_comments(
                &mut docs,
                separator,
                self.comments_to_emit_between(element.name_span.end, claim.value_start())
                    .filter(|c| claim.claims(self, c)),
                this_doc,
            );
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
    /// leading-comment rule at yet another site — and skipping them drops every comment
    /// in its expression.
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
