// Element type classification adapters for Svelte printer
//
// These methods extend Printer to provide convenient element classification
// by wrapping the pure language-level functions from the tsv_html crate.
//
// The printer-specific part is resolving symbols (interned strings) to
// tag names. The actual classification logic lives in tsv_html
// and can be reused by other tools (linter, type-checker, language server).

use crate::ast::internal;
use crate::ast::internal::ElementKind;
use crate::printer::Printer;
use tsv_html as html;
use tsv_lang::SymbolResolver;

impl<'a> Printer<'a> {
    /// Check if element is block (flow content)
    ///
    /// Adapter that resolves the element's tag name and calls the pure
    /// language-level classification function.
    ///
    /// Components are treated as inline, not block elements.
    ///
    /// Note: `<script>` and `<style>` elements with content are treated as block
    /// elements for formatting purposes, since their content will be formatted
    /// on separate lines. Empty `<script>`/`<style>` remain inline.
    pub(crate) fn is_block_element(&self, element: &internal::Element) -> bool {
        // Components are treated as inline, not block
        if element.kind == ElementKind::Component {
            return false;
        }

        // Borrow the interned tag rather than allocating a `String` per check:
        // this predicate runs once per element on the hot Svelte format path.
        self.with_resolved_symbol(element.name, |tag_name| {
            // <script>/<style> are block only when they carry real content, which
            // formats on its own lines. An empty <script></script> / <style></style>
            // stays inline (prettier parity). The raw-text parser always emits one
            // (possibly empty) Text node, so node-presence alone is not "has content".
            if (tag_name == "script" || tag_name == "style") && has_raw_content(element) {
                return true;
            }

            html::is_block_element(tag_name)
        })
    }
}

/// Whether a raw-text element (`<script>`/`<style>`) carries non-empty content.
/// Raw-text parsing emits exactly one `Text` node whose `raw` is the verbatim
/// body (empty for `<script></script>`), so an empty `raw` means no content.
fn has_raw_content(element: &internal::Element) -> bool {
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
    use crate::printer::Printer;
    use std::rc::Rc;

    /// Find the first child element of `parent` whose resolved tag name is `tag`.
    fn child<'p>(
        printer: &Printer<'_>,
        parent: &'p internal::Element,
        tag: &str,
    ) -> &'p internal::Element {
        parent
            .fragment
            .nodes
            .iter()
            .find_map(|n| match n {
                FragmentNode::Element(el) if printer.resolve_symbol(el.name) == tag => Some(el),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no <{tag}> child"))
    }

    #[test]
    fn block_adapter_delegates_and_treats_components_as_inline() {
        let src = "<div><span>i</span><Comp>c</Comp></div>";
        let root = crate::parse(src).expect("template should parse");
        // Reuse the parse's interner so the tag-name symbols resolve.
        let printer = Printer::new(src, Rc::clone(&root.interner), &[]);
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
        let root = crate::parse(src).expect("template should parse");
        let printer = Printer::new(src, Rc::clone(&root.interner), &[]);
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
        let root = crate::parse(src).expect("template should parse");
        let printer = Printer::new(src, Rc::clone(&root.interner), &[]);
        let div = match &root.fragment.nodes[0] {
            FragmentNode::Element(el) => el,
            other => panic!("expected a <div>, got: {other:?}"),
        };

        assert!(!printer.is_block_element(child(&printer, div, "script")));
        assert!(!printer.is_block_element(child(&printer, div, "style")));
    }
}
