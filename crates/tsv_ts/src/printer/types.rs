// Type annotation printing for TypeScript
//
// Handles printing of TypeScript-specific type syntax:
// - Type annotations (: Type)
// - Type keywords (number, string, boolean, etc.)
// - Future: Complex types (unions, intersections, generics, etc.)

use super::Printer;
use crate::ast::internal::{self, TSLiteralType, TSType, TemplateLiteralType};

impl<'a> Printer<'a> {
    /// Print a TypeScript type annotation (e.g., `: number`)
    pub(super) fn print_type_annotation(&mut self, annotation: &internal::TSTypeAnnotation) {
        self.write(": ");
        self.print_type(&annotation.type_annotation);
    }

    /// Print a TypeScript type expression
    pub(super) fn print_type(&mut self, ts_type: &TSType) {
        match ts_type {
            TSType::Keyword(kw) => self.write(kw.kind.as_str()),
            TSType::Literal(lit) => self.print_literal_type(lit),
        }
    }

    /// Print a TypeScript literal type (template literal types, etc.)
    fn print_literal_type(&mut self, lit: &TSLiteralType) {
        match lit {
            TSLiteralType::TemplateLiteral(template) => {
                self.print_template_literal_type(template);
            }
        }
    }

    /// Print a template literal type: `hello ${string} world`
    fn print_template_literal_type(&mut self, template: &TemplateLiteralType) {
        self.write("`");

        for (i, quasi) in template.quasis.iter().enumerate() {
            // Print the raw template content (preserving escapes)
            self.write(&quasi.raw);

            // Print type interpolation if there's a corresponding type
            if i < template.types.len() {
                self.write("${");
                self.print_type(&template.types[i]);
                self.write("}");
            }
        }

        self.write("`");
    }
}
