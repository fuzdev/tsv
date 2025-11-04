// Attribute formatting for Svelte elements
//
// Handles formatting of HTML attributes on elements, including:
// - Boolean attributes (e.g., `disabled`)
// - String attributes (e.g., `class="foo"`)
// - Future: Directives (on:, bind:, use:, transition:, etc.)
// - Future: Dynamic attributes ({...spread})

use crate::ast::internal;
use crate::printer::Printer;

impl<'a> Printer<'a> {
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
    pub(super) fn print_attribute(&mut self, attr: &internal::Attribute) {
        // Resolve attribute name from interner
        let name = self.resolve_symbol(attr.name);
        self.write(&name);

        // Format value if present
        if let Some(value_parts) = &attr.value {
            // Check if value is a single expression (no quotes needed)
            let is_pure_expression = value_parts.len() == 1
                && matches!(value_parts[0], internal::AttributeValue::ExpressionTag(_));

            if is_pure_expression {
                self.write("=");
            } else {
                self.write("=\"");
            }

            for part in value_parts {
                self.print_attribute_value(part);
            }

            if !is_pure_expression {
                self.write("\"");
            }
        }
    }

    /// Format an attribute value part
    ///
    /// Attribute values can contain static text or dynamic expressions.
    fn print_attribute_value(&mut self, value: &internal::AttributeValue) {
        match value {
            internal::AttributeValue::Text(text) => {
                self.write(&text.raw);
            }
            internal::AttributeValue::ExpressionTag(expr_tag) => {
                self.print_expression_tag(expr_tag);
            }
        }
    }
}
