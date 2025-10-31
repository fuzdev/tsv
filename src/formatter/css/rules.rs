// CSS rule and declaration formatting
//
// Handles formatting of:
// - CSS rules (selector + declarations block)
// - CSS declarations (property: value;)
// - Future: At-rules (@media, @keyframes, etc.)

use crate::ast::internal;
use crate::formatter::Formatter;

impl Formatter {
    /// Format a CSS rule (selector + declarations block)
    pub(super) fn format_css_rule(&mut self, rule: &internal::CssRule) {
        // Format selector
        self.write(&rule.selector);
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
    fn format_css_declaration(&mut self, decl: &internal::CssDeclaration) {
        self.write_indent();
        self.write(&decl.property);
        self.write(": ");
        self.write(&decl.value);
        self.write(";\n");
    }
}
