// Element type classification adapters for Svelte formatter
//
// These methods extend Formatter to provide convenient element classification
// by wrapping the pure language-level functions from crate::language::html.
//
// The formatter-specific part is resolving symbols (interned strings) to
// tag names. The actual classification logic lives in crate::language::html
// and can be reused by other tools (linter, type-checker, language server).

use crate::ast::internal;
use crate::formatter::Formatter;
use crate::language::html;

impl Formatter {
    /// Check if element is inline (phrasing content)
    ///
    /// Adapter that resolves the element's tag name and calls the pure
    /// language-level classification function.
    pub(crate) fn is_inline_element(&self, element: &internal::Element) -> bool {
        let tag_name = self.resolve_symbol(element.name);
        html::is_inline_element(&tag_name)
    }

    /// Check if element is block (flow content)
    ///
    /// Adapter that resolves the element's tag name and calls the pure
    /// language-level classification function.
    pub(crate) fn is_block_element(&self, element: &internal::Element) -> bool {
        let tag_name = self.resolve_symbol(element.name);
        html::is_block_element(&tag_name)
    }

    /// Check if element is void (self-closing by spec)
    ///
    /// Adapter that resolves the element's tag name and calls the pure
    /// language-level classification function.
    pub(crate) fn is_void_element(&self, element: &internal::Element) -> bool {
        let tag_name = self.resolve_symbol(element.name);
        html::is_void_element(&tag_name)
    }
}
