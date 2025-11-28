// Attribute formatting for Svelte elements
//
// Handles formatting of HTML attributes on elements, including:
// - Boolean attributes (e.g., `disabled`)
// - String attributes (e.g., `class="foo"`)
// - Future: Directives (on:, bind:, use:, transition:, etc.)
// - Future: Dynamic attributes ({...spread})

use crate::ast::internal;
use crate::printer::Printer;
use tsv_lang::SymbolResolver;
use tsv_lang::doc::{self, Doc};

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

    /// Build a Doc for a single attribute (used for line wrapping calculations)
    ///
    /// This is used in the hybrid doc-builder approach for attribute wrapping.
    pub(super) fn build_attribute_doc(&self, attr: &internal::Attribute) -> Doc {
        let name = self.resolve_symbol(attr.name);

        if let Some(value_parts) = &attr.value {
            let is_pure_expression = value_parts.len() == 1
                && matches!(value_parts[0], internal::AttributeValue::ExpressionTag(_));

            let mut parts = vec![doc::text(name)];

            if is_pure_expression {
                parts.push(doc::text("="));
            } else {
                parts.push(doc::text("=\""));
            }

            for part in value_parts {
                parts.push(self.build_attribute_value_doc(part));
            }

            if !is_pure_expression {
                parts.push(doc::text("\""));
            }

            doc::concat(parts)
        } else {
            // Boolean attribute
            doc::text(name)
        }
    }

    /// Build a Doc for an attribute value part
    fn build_attribute_value_doc(&self, value: &internal::AttributeValue) -> Doc {
        match value {
            internal::AttributeValue::Text(text) => doc::text(&text.raw),
            internal::AttributeValue::ExpressionTag(expr_tag) => {
                // TODO: For now, render the expression tag to a string
                // In future full refactor, this would build a Doc tree for the expression
                let mut temp_printer =
                    Printer::with_config(self.source, self.interner.clone(), self.config);
                temp_printer.print_expression_tag(expr_tag);
                doc::text(temp_printer.into_string())
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
