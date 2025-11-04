// CSS rule and declaration formatting
//
// Handles formatting of:
// - CSS rules (selector + declarations block)
// - CSS declarations (property: value;)
//
// Selector formatting is handled by the selectors module.

use super::Printer;
use crate::ast::internal::{self, Color, CssValue};
use tsv_lang::printing::{StringFormatOptions, format_string_literal};

impl<'a> Printer<'a> {
    /// Format a CSS rule (selector + declarations block)
    pub(super) fn print_css_rule(&mut self, rule: &internal::CssRule) {
        // Format selector (uses selectors module)
        self.print_selector_list(&rule.selector);
        self.write(" {\n");

        // Format declarations with indentation
        self.indent_level += 1;
        for decl in &rule.declarations {
            self.print_css_declaration(decl);
        }
        self.indent_level -= 1;

        // Closing brace
        self.write("}");
    }

    /// Extract raw property name from source to preserve escape sequences
    ///
    /// SVELTE QUIRK: Property names preserve raw escapes without decoding
    /// Example: `\00e9motion` stays as `\00e9motion`, not `émotion`
    fn extract_property_name<'b>(&self, decl_source: &'b str) -> &'b str {
        if let Some(colon_pos) = decl_source.find(':') {
            decl_source[..colon_pos].trim() // Trim whitespace for normalization
        } else {
            // Fallback: no colon found (malformed declaration)
            // Return entire source as property name
            decl_source.trim()
        }
    }

    /// Check if a property should use multiline formatting
    fn should_use_multiline(&self, decl: &internal::CssDeclaration) -> bool {
        matches!(decl.property.as_str(), "box-shadow" | "text-shadow")
            && matches!(&decl.value, CssValue::CommaSeparated { values } if values.len() > 1)
    }

    /// Format a string value by extracting raw content from source
    ///
    /// This preserves escape sequences by working with the original source text
    /// rather than the decoded AST content.
    fn print_declaration_string_value(&mut self, decl_source: &str, quote: char) {
        if let Some(colon_pos) = decl_source.find(':') {
            let value_part = decl_source[colon_pos + 1..].trim();
            // String should be quoted
            if value_part.len() >= 2 {
                let raw_content = &value_part[1..value_part.len() - 1];

                // Format using shared utility (handles quote selection and escaping)
                let formatted =
                    format_string_literal(raw_content, quote, StringFormatOptions::default());

                self.write(": ");
                self.write(&formatted);
                self.write(";\n");
                return;
            }
        }

        // Fallback: use AST value (loses escape sequence fidelity)
        self.write(": ");
        self.write(&format!("'{}'", quote)); // Simplified fallback
        self.write(";\n");
    }

    /// Format a CSS declaration (property: value;)
    pub(super) fn print_css_declaration(&mut self, decl: &internal::CssDeclaration) {
        self.write_indent();

        // Extract property name from source to preserve escape sequences
        // See: docs/SVELTE_COMPATIBILITY.md (CSS Quirks section)
        let decl_source = &self.source[decl.span.start as usize..decl.span.end as usize];
        let property_raw = self.extract_property_name(decl_source);
        self.write(property_raw);

        // Check if property needs multiline formatting
        if self.should_use_multiline(decl) {
            self.write(":\n");
            self.indent_level += 1;
            self.print_css_value_multiline(&decl.value);
            self.indent_level -= 1;
            self.write(";\n");
        } else if let CssValue::String { quote, .. } = &decl.value {
            // String values: extract from source to preserve escapes
            self.print_declaration_string_value(decl_source, *quote);
        } else {
            // All other values: use standard formatting
            self.write(": ");
            self.print_css_value(&decl.value);
            self.write(";\n");
        }
    }

    /// Format a CSS value on multiple lines (for shadow properties)
    fn print_css_value_multiline(&mut self, value: &CssValue) {
        match value {
            CssValue::CommaSeparated { values } => {
                for (i, val) in values.iter().enumerate() {
                    self.write_indent();
                    self.print_css_value(val);
                    if i < values.len() - 1 {
                        self.write(",\n");
                    }
                }
            }
            _ => {
                // Fallback to regular formatting
                self.print_css_value(value);
            }
        }
    }

    /// Format a CSS value (right-hand side of declaration)
    pub(super) fn print_css_value(&mut self, value: &CssValue) {
        match value {
            CssValue::Identifier(ident) => {
                self.write(ident);
            }
            CssValue::String { content, quote } => {
                // LIMITATION: CssValue::String doesn't have a span, so we can't extract raw from source.
                // This path is hit for strings in comma-separated values (e.g., font-family: 'Arial', 'Helvetica').
                //
                // We use format_string_literal which handles quote selection and escape swapping,
                // but we lose the ability to preserve Svelte quirks (backslash doubling, etc.) because
                // we're working with decoded content from the AST, not raw source.
                //
                // TODO: Add span to CssValue::String in the parser to enable source extraction
                let formatted =
                    format_string_literal(content, *quote, StringFormatOptions::default());
                self.write(&formatted);
            }
            CssValue::Dimension { source, .. } => {
                self.write(source);
            }
            CssValue::Color(color) => {
                self.print_css_color(color);
            }
            CssValue::Function { name, args } => {
                self.write(name);
                self.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_css_value(arg);
                }
                self.write(")");
            }
            CssValue::List { values } => {
                for (i, val) in values.iter().enumerate() {
                    if i > 0 {
                        self.write(" ");
                    }
                    self.print_css_value(val);
                }
            }
            CssValue::CommaSeparated { values } => {
                for (i, val) in values.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_css_value(val);
                }
            }
        }
    }

    /// Format a CSS color value
    fn print_css_color(&mut self, color: &Color) {
        match color {
            Color::Named(name) => {
                self.write(name);
            }
            Color::Hex(hex) => {
                self.write(hex);
            }
            Color::Rgb { r, g, b, alpha } => {
                if let Some(a) = alpha {
                    self.write(&format!("rgba({}, {}, {}, {})", r, g, b, a));
                } else {
                    self.write(&format!("rgb({}, {}, {})", r, g, b));
                }
            }
            Color::Hsl {
                hue,
                saturation,
                lightness,
                alpha,
            } => {
                if let Some(a) = alpha {
                    self.write(&format!(
                        "hsla({}%, {}%, {}%, {})",
                        hue, saturation, lightness, a
                    ));
                } else {
                    self.write(&format!("hsl({}%, {}%, {}%)", hue, saturation, lightness));
                }
            }
        }
    }
}
