// Element type classification adapters for Svelte printer
//
// These methods extend Printer to provide convenient element classification
// by wrapping the pure language-level functions from crate::language::html.
//
// The printer-specific part is resolving symbols (interned strings) to
// tag names. The actual classification logic lives in crate::language::html
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

        let tag_name = self.resolve_symbol(element.name);

        // <script> and <style> with content are treated as block elements
        // because their content will be formatted on separate lines
        if (tag_name == "script" || tag_name == "style") && !element.fragment.nodes.is_empty() {
            return true;
        }

        html::is_block_element(&tag_name)
    }
}
