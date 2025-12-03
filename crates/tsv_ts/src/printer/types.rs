// Type annotation printing for TypeScript
//
// Handles printing of TypeScript-specific type syntax:
// - Type annotations (: Type)
// - Type keywords (number, string, boolean, etc.)
// - Future: Complex types (unions, intersections, generics, etc.)

use super::Printer;
use crate::ast::internal::{self, TSArrayType, TSLiteralType, TSType, TemplateLiteralType};
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    /// Print a TypeScript type annotation (e.g., `: number`)
    pub(super) fn print_type_annotation(&mut self, annotation: &internal::TSTypeAnnotation) {
        self.write(": ");
        self.print_type(&annotation.type_annotation);
    }

    /// Build a Doc for a type annotation (e.g., `: number`)
    pub(super) fn build_type_annotation_doc(&self, annotation: &internal::TSTypeAnnotation) -> Doc {
        doc::concat(vec![
            doc::text(": "),
            self.build_type_doc(&annotation.type_annotation),
        ])
    }

    /// Build a Doc for a TypeScript type expression
    pub(super) fn build_type_doc(&self, ts_type: &TSType) -> Doc {
        match ts_type {
            TSType::Keyword(kw) => doc::text(kw.kind.as_str().to_string()),
            TSType::Literal(lit) => self.build_literal_type_doc(lit),
            TSType::Array(arr) => self.build_array_type_doc(arr),
        }
    }

    /// Build a Doc for an array type (e.g., `number[]`)
    fn build_array_type_doc(&self, arr: &TSArrayType) -> Doc {
        doc::concat(vec![
            self.build_type_doc(&arr.element_type),
            doc::text("[]"),
        ])
    }

    /// Build a Doc for a literal type
    fn build_literal_type_doc(&self, lit: &TSLiteralType) -> Doc {
        match lit {
            TSLiteralType::TemplateLiteral(template) => {
                self.build_template_literal_type_doc(template)
            }
        }
    }

    /// Build a Doc for a template literal type
    fn build_template_literal_type_doc(&self, template: &TemplateLiteralType) -> Doc {
        let mut parts = vec![doc::text("`")];
        for (i, quasi) in template.quasis.iter().enumerate() {
            parts.push(doc::text(quasi.raw.clone()));
            if i < template.types.len() {
                parts.push(doc::text("${"));
                parts.push(self.build_type_doc(&template.types[i]));
                parts.push(doc::text("}"));
            }
        }
        parts.push(doc::text("`"));
        doc::concat(parts)
    }

    /// Print a TypeScript type expression
    pub(super) fn print_type(&mut self, ts_type: &TSType) {
        match ts_type {
            TSType::Keyword(kw) => self.write(kw.kind.as_str()),
            TSType::Literal(lit) => self.print_literal_type(lit),
            TSType::Array(arr) => {
                self.print_type(&arr.element_type);
                self.write("[]");
            }
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

    /// Convert a type annotation to a string (for inline building)
    pub(super) fn type_annotation_to_string(
        &self,
        annotation: &internal::TSTypeAnnotation,
    ) -> String {
        format!(": {}", self.type_to_string(&annotation.type_annotation))
    }

    /// Convert a type to a string (for inline building)
    fn type_to_string(&self, ts_type: &TSType) -> String {
        match ts_type {
            TSType::Keyword(kw) => kw.kind.as_str().to_string(),
            TSType::Literal(lit) => self.literal_type_to_string(lit),
            TSType::Array(arr) => format!("{}[]", self.type_to_string(&arr.element_type)),
        }
    }

    /// Convert a literal type to a string
    fn literal_type_to_string(&self, lit: &TSLiteralType) -> String {
        match lit {
            TSLiteralType::TemplateLiteral(template) => {
                self.template_literal_type_to_string(template)
            }
        }
    }

    /// Convert a template literal type to a string
    fn template_literal_type_to_string(&self, template: &TemplateLiteralType) -> String {
        let mut result = String::from("`");
        for (i, quasi) in template.quasis.iter().enumerate() {
            result.push_str(&quasi.raw);
            if i < template.types.len() {
                result.push_str("${");
                result.push_str(&self.type_to_string(&template.types[i]));
                result.push('}');
            }
        }
        result.push('`');
        result
    }
}
