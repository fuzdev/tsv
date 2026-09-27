// Element-instance classification overlay for the Svelte printer.
//
// The name-derived facts are computed once at parse
// ([`TagFacts`](crate::ast::internal::TagFacts), stored on `Element::facts`); this module holds
// the printer-specific overlay on top of them — a Component is inline, and a `<script>`/`<style>`
// with content is block. The pure per-name classification lives in the `tsv_html` crate and can
// be reused by other tools (linter, type-checker, language server).

use crate::ast::internal;
use crate::ast::internal::ElementKind;
use crate::printer::Printer;

impl<'a> Printer<'a> {
    /// Check if element is block (flow content)
    ///
    /// Reads the parse-time name facts (`Element::facts`) plus the two element-instance overlays.
    ///
    /// Components are treated as inline, not block elements.
    ///
    /// Note: `<script>` and `<style>` elements with content are treated as block
    /// elements for formatting purposes, since their content will be formatted
    /// on separate lines. Empty `<script>`/`<style>` remain inline. This is the NODE-only answer:
    /// such an element renders no box, so where a break beside it would render, the positional
    /// readers lay it out as a glued inline element instead (`Printer::is_glued_raw_text_element`).
    pub(crate) fn is_block_element(&self, element: &internal::Element<'_>) -> bool {
        // Components are treated as inline, not block
        if element.kind == ElementKind::Component {
            return false;
        }

        let facts = element.facts;

        // <script>/<style> are block only when they carry real content, which
        // formats on its own lines. An empty <script></script> / <style></style>
        // stays inline (prettier parity). The raw-text parser always emits one
        // (possibly empty) Text node, so node-presence alone is not "has content".
        if facts.is_raw_text() && has_raw_content(element) {
            return true;
        }

        facts.is_block()
    }
}

/// Whether an element's content prints anything — asked of a raw-text element
/// (`<script>`/`<style>`), whose parse emits exactly one `Text` node holding the verbatim body
/// (empty for `<script></script>`, which Svelte still gives that node), so node-presence is not
/// "has content" and an empty `raw` means none.
///
/// The one reading for every asker: the block classification above (a non-empty body is
/// block), the nested body's own builder (`build_raw_content_element_doc`, where an empty body
/// is the one arm that collapses) and the whitespace-sensitive head
/// (`build_whitespace_sensitive_element_doc`, where an empty body takes the empty element's
/// layouts). Two spellings of it would let them answer differently for the same element.
pub(crate) fn has_raw_content(element: &internal::Element<'_>) -> bool {
    use crate::ast::internal::FragmentNode;
    element
        .fragment
        .nodes
        .iter()
        .any(|node| !matches!(node, FragmentNode::Text(t) if t.raw_span.range().is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::internal::FragmentNode;
    use tsv_html as html;

    /// Find the first child element of `parent` whose resolved tag name is `tag`.
    fn child<'p, 'arena>(
        printer: &Printer<'_>,
        parent: &'p internal::Element<'arena>,
        tag: &str,
    ) -> &'p internal::Element<'arena> {
        parent
            .fragment
            .nodes
            .iter()
            .find_map(|n| match n {
                FragmentNode::Element(el) if el.name(printer.source) == tag => Some(el),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no <{tag}> child"))
    }

    #[test]
    fn block_adapter_delegates_and_treats_components_as_inline() {
        let src = "<div><span>i</span><Comp>c</Comp></div>";
        let arena = bumpalo::Bump::new();
        let root = crate::parse(src, &arena).expect("template should parse");
        let doc_arena = tsv_lang::doc::arena::DocArena::for_source(src);
        let printer = Printer::new(&doc_arena, src, &[]);
        let div = match &root.fragment.nodes[0] {
            FragmentNode::Element(el) => el,
            other => panic!("expected a <div>, got: {other:?}"),
        };

        // Plain HTML tags delegate straight to tsv_html: <div> block, <span> inline.
        assert!(printer.is_block_element(div));
        assert!(!printer.is_block_element(child(&printer, div, "span")));
        // A component is always inline, regardless of its (uppercase) name.
        assert!(!printer.is_block_element(child(&printer, div, "Comp")));
    }

    #[test]
    fn block_adapter_promotes_nonempty_script_style_to_block() {
        // The overlay is the printer-specific part: a <script>/<style> with content
        // is block (its body formats on its own lines), even though tsv_html
        // classifies the bare tag as inline.
        assert!(!html::is_block_element("script"));
        assert!(!html::is_block_element("style"));

        let src = "<div><script>let x = 1;</script><style>a { color: red }</style></div>";
        let arena = bumpalo::Bump::new();
        let root = crate::parse(src, &arena).expect("template should parse");
        let doc_arena = tsv_lang::doc::arena::DocArena::for_source(src);
        let printer = Printer::new(&doc_arena, src, &[]);
        let div = match &root.fragment.nodes[0] {
            FragmentNode::Element(el) => el,
            other => panic!("expected a <div>, got: {other:?}"),
        };

        assert!(printer.is_block_element(child(&printer, div, "script")));
        assert!(printer.is_block_element(child(&printer, div, "style")));
    }

    #[test]
    fn block_adapter_treats_empty_script_style_as_inline() {
        // An empty <script></script> / <style></style> has no content to format
        // on its own lines, so it stays inline (prettier keeps the parent on one
        // line). The raw-text parser still emits a single empty Text node here, so
        // `has_raw_content` — not node-presence — is what makes this inline.
        let src = "<div><script></script><style></style></div>";
        let arena = bumpalo::Bump::new();
        let root = crate::parse(src, &arena).expect("template should parse");
        let doc_arena = tsv_lang::doc::arena::DocArena::for_source(src);
        let printer = Printer::new(&doc_arena, src, &[]);
        let div = match &root.fragment.nodes[0] {
            FragmentNode::Element(el) => el,
            other => panic!("expected a <div>, got: {other:?}"),
        };

        assert!(!printer.is_block_element(child(&printer, div, "script")));
        assert!(!printer.is_block_element(child(&printer, div, "style")));
    }
}
