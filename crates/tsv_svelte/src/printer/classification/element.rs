// Element type classification adapters for Svelte printer
//
// These methods extend Printer to provide convenient element classification
// by wrapping the pure language-level functions from crate::language::html.
//
// The printer-specific part is resolving symbols (interned strings) to
// tag names. The actual classification logic lives in crate::language::html
// and can be reused by other tools (linter, type-checker, language server).

use crate::ast::internal;
use crate::printer::Printer;
use tsv_html as html;
use tsv_lang::SymbolResolver;

impl<'a> Printer<'a> {
    /// Check if element is inline (phrasing content)
    ///
    /// Adapter that resolves the element's tag name and calls the pure
    /// language-level classification function.
    ///
    /// Components are treated as inline to preserve surrounding whitespace.
    pub(crate) fn is_inline_element(&self, element: &internal::Element) -> bool {
        // Components are always treated as inline
        use crate::ast::internal::ElementKind;
        if element.kind == ElementKind::Component {
            return true;
        }

        let tag_name = self.resolve_symbol(element.name);
        html::is_inline_element(&tag_name)
    }

    /// Check if element is block (flow content)
    ///
    /// Adapter that resolves the element's tag name and calls the pure
    /// language-level classification function.
    ///
    /// Components are treated as inline, not block elements.
    pub(crate) fn is_block_element(&self, element: &internal::Element) -> bool {
        // Components are treated as inline, not block
        use crate::ast::internal::ElementKind;
        if element.kind == ElementKind::Component {
            return false;
        }

        let tag_name = self.resolve_symbol(element.name);
        html::is_block_element(&tag_name)
    }
}
