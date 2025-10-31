// Whitespace preservation adapters for Svelte formatter
//
// These methods extend Formatter to check whitespace preservation by wrapping
// the pure language-level functions from crate::language::html.

use crate::ast::internal;
use crate::formatter::Formatter;
use crate::language::html;

impl Formatter {
    /// Check if element preserves whitespace (like `<pre>`)
    ///
    /// Adapter that resolves the element's tag name and calls the pure
    /// language-level classification function.
    pub(crate) fn preserves_whitespace(&self, element: &internal::Element) -> bool {
        let tag_name = self.resolve_symbol(element.name);
        html::preserves_whitespace(&tag_name)
    }
}
