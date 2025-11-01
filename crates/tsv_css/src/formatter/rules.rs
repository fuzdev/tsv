// CSS rule and declaration formatting
//
// Handles formatting of:
// - CSS rules (selector + declarations block)
// - CSS declarations (property: value;)
//
// Selector formatting is handled by the selectors module.

use crate::ast::internal::{self, Color, CssValue};
use crate::formatter::Formatter;

impl Formatter {
    /// Format a CSS rule (selector + declarations block)
    pub(super) fn format_css_rule(&mut self, rule: &internal::CssRule) {
        // Format selector (uses selectors module)
        self.format_selector_list(&rule.selector);
        self.write(" {\n");

        // Format declarations with indentation
        self.indent_level += 1;
        for decl in &rule.declarations {
            self.format_css_declaration(decl);
        }
        self.indent_level -= 1;

        // Closing brace
        self.write("}");
    }

    /// Format a CSS declaration (property: value;)
    pub(super) fn format_css_declaration(&mut self, decl: &internal::CssDeclaration) {
        self.write_indent();
        self.write(&decl.property);

        // Check if this property should use multi-line formatting for comma-separated values
        let use_multiline = matches!(decl.property.as_str(), "box-shadow" | "text-shadow")
            && matches!(&decl.value, CssValue::CommaSeparated { values } if values.len() > 1);

        if use_multiline {
            self.write(":\n");
            self.indent_level += 1;
            self.format_css_value_multiline(&decl.value);
            self.indent_level -= 1;
            self.write(";\n");
        } else {
            self.write(": ");
            self.format_css_value(&decl.value);
            self.write(";\n");
        }
    }

    /// Format a CSS value on multiple lines (for shadow properties)
    fn format_css_value_multiline(&mut self, value: &CssValue) {
        match value {
            CssValue::CommaSeparated { values } => {
                for (i, val) in values.iter().enumerate() {
                    self.write_indent();
                    self.format_css_value(val);
                    if i < values.len() - 1 {
                        self.write(",\n");
                    }
                }
            }
            _ => {
                // Fallback to regular formatting
                self.format_css_value(value);
            }
        }
    }

    /// Format a CSS value (right-hand side of declaration)
    pub(super) fn format_css_value(&mut self, value: &CssValue) {
        match value {
            CssValue::Identifier(ident) => {
                self.write(ident);
            }
            CssValue::String(s) => {
                // Choose quote style: prefer single quotes unless string contains them
                let single_count = s.chars().filter(|&c| c == '\'').count();
                let double_count = s.chars().filter(|&c| c == '"').count();

                let quote_char = if single_count > 0 && double_count == 0 {
                    // Contains ' but no " → use double quotes
                    '"'
                } else if double_count > 0 && single_count == 0 {
                    // Contains " but no ' → use single quotes
                    '\''
                } else if single_count > double_count {
                    // More ' than " → use double quotes
                    '"'
                } else {
                    // Default or equal → prefer single quotes
                    '\''
                };

                self.write(&quote_char.to_string());
                self.write(s);
                self.write(&quote_char.to_string());
            }
            CssValue::Dimension { source, .. } => {
                self.write(source);
            }
            CssValue::Color(color) => {
                self.format_css_color(color);
            }
            CssValue::Function { name, args } => {
                self.write(name);
                self.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_css_value(arg);
                }
                self.write(")");
            }
            CssValue::List { values } => {
                for (i, val) in values.iter().enumerate() {
                    if i > 0 {
                        self.write(" ");
                    }
                    self.format_css_value(val);
                }
            }
            CssValue::CommaSeparated { values } => {
                for (i, val) in values.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_css_value(val);
                }
            }
        }
    }

    /// Format a CSS color value
    fn format_css_color(&mut self, color: &Color) {
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
