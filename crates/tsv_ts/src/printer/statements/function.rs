// Function declaration printing for TypeScript

use super::super::Printer;
use crate::ast::internal;
use tsv_lang::{SymbolResolver, doc};

impl<'a> Printer<'a> {
    /// Print a function declaration: `function foo(x) { return x + 1; }`
    /// or anonymous: `function () {}` (Prettier adds space before parenthesis for anonymous functions)
    /// or async: `async function foo() {}`
    pub(super) fn print_function_declaration(&mut self, decl: &internal::FunctionDeclaration) {
        if decl.r#async {
            self.write("async ");
        }
        self.write("function");
        if let Some(id) = &decl.id {
            self.write(" ");
            self.print_identifier(id);
        } else {
            // Prettier adds a space before () for anonymous functions
            self.write(" ");
        }
        self.write("(");

        // Print parameters (can be Identifier, ArrayPattern, ObjectPattern, AssignmentPattern)
        for (i, param) in decl.params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_expression(param);
        }

        self.write(")");

        // Print return type annotation if present
        if let Some(return_type) = &decl.return_type {
            self.print_type_annotation(return_type);
        }

        self.write(" ");
        self.print_block_statement(&decl.body);
    }

    /// Build a Doc for a function declaration
    pub(super) fn build_function_declaration_doc(
        &self,
        decl: &internal::FunctionDeclaration,
    ) -> doc::Doc {
        let mut parts = Vec::new();
        if decl.r#async {
            parts.push(doc::text("async "));
        }
        parts.push(doc::text("function"));
        if let Some(id) = &decl.id {
            let id_str = self.resolve_symbol(id.name);
            parts.push(doc::text(" "));
            parts.push(doc::text(id_str));
        } else {
            // Prettier adds a space before () for anonymous functions
            parts.push(doc::text(" "));
        }
        parts.push(doc::text("("));

        // Build params (can be Identifier, ArrayPattern, ObjectPattern, AssignmentPattern)
        for (i, param) in decl.params.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(", "));
            }
            parts.push(self.build_expression_doc(param));
        }

        parts.push(doc::text(")"));

        // Return type annotation (e.g., `: number`)
        if let Some(return_type) = &decl.return_type {
            parts.push(self.build_type_annotation_doc(return_type));
        }

        parts.push(doc::text(" "));
        parts.push(self.build_block_statement_doc(&decl.body));

        doc::concat(parts)
    }
}
