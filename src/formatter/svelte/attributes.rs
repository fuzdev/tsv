// Attribute formatting for Svelte elements
//
// Handles formatting of HTML attributes on elements, including:
// - Boolean attributes (e.g., `disabled`)
// - String attributes (e.g., `class="foo"`)
// - Future: Directives (on:, bind:, use:, transition:, etc.)
// - Future: Dynamic attributes ({...spread})

use crate::ast::internal;
use crate::formatter::Formatter;

impl Formatter {
    /// Format an attribute (name="value" or name)
    ///
    /// Formats both boolean attributes (no value) and valued attributes.
    ///
    /// # Examples
    /// ```text
    /// disabled          → disabled
    /// class="foo"       → class="foo"
    /// context="module"  → context="module"
    /// ```
    pub(super) fn format_attribute(&mut self, attr: &internal::Attribute) {
        // Resolve attribute name from interner
        let name = self.resolve_symbol(attr.name);
        self.write(&name);

        // Format value if present
        if let Some(value_parts) = &attr.value {
            self.write("=\"");
            for part in value_parts {
                self.format_attribute_value(part);
            }
            self.write("\"");
        }
    }

    /// Format an attribute value part
    ///
    /// Attribute values can contain static text or (in future) dynamic expressions.
    /// Currently only text values are supported.
    fn format_attribute_value(&mut self, value: &internal::AttributeValue) {
        match value {
            internal::AttributeValue::Text(text) => {
                self.write(&text.raw);
            } // TODO: Handle expression attribute values in future sprint
        }
    }
}
