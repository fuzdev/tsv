// Module statement printing for TypeScript (import and export)

use super::super::Printer;
use crate::ast::internal;
use tsv_lang::{SymbolResolver, doc};

impl<'a> Printer<'a> {
    /// Print an export named declaration
    ///
    /// Uses doc builder with width-based wrapping for specifiers.
    pub(super) fn print_export_named_declaration(
        &mut self,
        decl: &internal::ExportNamedDeclaration,
    ) {
        if let Some(declaration) = &decl.declaration {
            // export const x = 1; - use direct printing for declarations
            self.write("export ");
            self.print_statement(declaration);
        } else {
            // export { x, y as z } or export { x } from "y" - use doc builder
            let doc = self.build_export_named_declaration_doc(decl);
            let base_offset = self.config.base_indent_offset * self.config.tab_width;
            let current_col = self.current_column() + base_offset;
            let output =
                doc::print_doc_with_indent(&doc, &self.config, current_col, self.indent_level);
            self.write(&output);
        }
    }

    /// Print an export default declaration
    pub(super) fn print_export_default_declaration(
        &mut self,
        decl: &internal::ExportDefaultDeclaration,
    ) {
        self.write("export default ");
        match &decl.declaration {
            internal::ExportDefaultValue::Expression(expr) => {
                self.print_expression(expr);
                self.write(";");
            }
            internal::ExportDefaultValue::FunctionDeclaration(func) => {
                self.print_function_declaration(func);
            }
            internal::ExportDefaultValue::ClassDeclaration(class) => {
                self.print_class_declaration(class);
            }
        }
    }

    /// Print an export all declaration
    pub(super) fn print_export_all_declaration(&mut self, decl: &internal::ExportAllDeclaration) {
        self.write("export *");
        if let Some(exported) = &decl.exported {
            self.write(" as ");
            self.write(&self.resolve_symbol(exported.name));
        }
        self.write(" from ");
        self.print_literal(&decl.source);
        self.write(";");
    }

    /// Build a Doc for an export named declaration
    ///
    /// Uses doc::group() for width-based wrapping of specifiers.
    /// When the line exceeds print_width (100 chars), wraps to multiline format.
    pub(super) fn build_export_named_declaration_doc(
        &self,
        decl: &internal::ExportNamedDeclaration,
    ) -> doc::Doc {
        if let Some(declaration) = &decl.declaration {
            doc::concat(vec![
                doc::text("export "),
                self.build_statement_doc(declaration),
            ])
        } else {
            // export { x, y as z } or export { x } from "y"
            let mut parts = vec![doc::text("export ")];

            if decl.specifiers.is_empty() {
                // Empty braces case: `export {}`
                parts.push(doc::text("{}"));
            } else {
                // Build specifier docs with line breaks between them
                let mut spec_parts = Vec::new();
                for (i, spec) in decl.specifiers.iter().enumerate() {
                    if i > 0 {
                        spec_parts.push(doc::text(","));
                        spec_parts.push(doc::line());
                    }
                    let local = self.resolve_symbol(spec.local.name);
                    let exported = self.resolve_symbol(spec.exported.name);
                    if local == exported {
                        spec_parts.push(doc::text(local));
                    } else {
                        spec_parts.push(doc::text(local));
                        spec_parts.push(doc::text(" as "));
                        spec_parts.push(doc::text(exported));
                    }
                }
                // Add trailing comma only when broken
                spec_parts.push(doc::if_break(doc::text(","), doc::text("")));

                // Build the braces content (will be wrapped in outer group)
                parts.push(doc::text("{"));
                parts.push(doc::indent(doc::concat(vec![
                    doc::softline(),
                    doc::concat(spec_parts),
                ])));
                parts.push(doc::softline());
                parts.push(doc::text("}"));
            }

            if let Some(source) = &decl.source {
                parts.push(doc::text(" from "));
                parts.push(self.build_literal_doc(source));
            }
            parts.push(doc::text(";"));

            // Wrap entire statement in a group for width-based wrapping
            doc::group(doc::concat(parts))
        }
    }

    /// Build a Doc for an export default declaration
    pub(super) fn build_export_default_declaration_doc(
        &self,
        decl: &internal::ExportDefaultDeclaration,
    ) -> doc::Doc {
        let value_doc = match &decl.declaration {
            internal::ExportDefaultValue::Expression(expr) => {
                doc::concat(vec![self.build_expression_doc(expr), doc::text(";")])
            }
            internal::ExportDefaultValue::FunctionDeclaration(func) => {
                self.build_function_declaration_doc(func)
            }
            internal::ExportDefaultValue::ClassDeclaration(class) => {
                self.build_class_declaration_doc(class)
            }
        };
        doc::concat(vec![doc::text("export default "), value_doc])
    }

    /// Build a Doc for an export all declaration
    pub(super) fn build_export_all_declaration_doc(
        &self,
        decl: &internal::ExportAllDeclaration,
    ) -> doc::Doc {
        let mut parts = vec![doc::text("export *")];
        if let Some(exported) = &decl.exported {
            parts.push(doc::text(" as "));
            parts.push(doc::text(self.resolve_symbol(exported.name)));
        }
        parts.push(doc::text(" from "));
        parts.push(self.build_literal_doc(&decl.source));
        parts.push(doc::text(";"));
        doc::concat(parts)
    }

    /// Check if an import declaration has empty named braces `{}` in source.
    /// This distinguishes `import {} from 'x'` from `import 'x'`.
    fn has_empty_named_braces(&self, decl: &internal::ImportDeclaration) -> bool {
        let start = decl.span.start as usize;
        let end = decl.span.end as usize;
        if start >= self.source.len() || end > self.source.len() {
            return false;
        }
        let text = &self.source[start..end];
        // Find "from" (with or without surrounding spaces) and check if there are empty braces before it
        // Handles both `import {} from 'x'` and `import{}from'x'` and `import {  } from 'x'`
        if let Some(from_pos) = text.find("from") {
            let before_from = &text[..from_pos];
            // Check for empty braces (with any amount of whitespace inside)
            if let Some(brace_start) = before_from.rfind('{')
                && let Some(brace_end) = before_from[brace_start..].find('}')
            {
                let inside = &before_from[brace_start + 1..brace_start + brace_end];
                return inside.trim().is_empty();
            }
            false
        } else {
            false
        }
    }

    /// Print an import declaration
    ///
    /// Uses doc builder with width-based wrapping for named specifiers.
    pub(super) fn print_import_declaration(&mut self, decl: &internal::ImportDeclaration) {
        let doc = self.build_import_declaration_doc(decl);
        let base_offset = self.config.base_indent_offset * self.config.tab_width;
        let current_col = self.current_column() + base_offset;
        let output = doc::print_doc_with_indent(&doc, &self.config, current_col, self.indent_level);
        self.write(&output);
    }

    /// Build a Doc for an import declaration
    ///
    /// Uses doc::group() for width-based wrapping of named specifiers.
    /// When the line exceeds print_width (100 chars), wraps to multiline format.
    pub(super) fn build_import_declaration_doc(
        &self,
        decl: &internal::ImportDeclaration,
    ) -> doc::Doc {
        // Check if source has empty braces (for `import {} from 'x'`)
        let has_empty_braces = self.has_empty_named_braces(decl);

        // Check if this is a type-only import
        let is_type_import = decl.import_kind == internal::ImportKind::Type;

        // Collect specifiers
        let mut has_default = false;
        let mut has_named = false;
        let mut has_namespace = false;
        let mut named_specs = Vec::new();
        let mut default_name = String::new();
        let mut namespace_name = String::new();

        for spec in &decl.specifiers {
            match spec {
                internal::ImportSpecifier::Default(default_spec) => {
                    has_default = true;
                    default_name = self.resolve_symbol(default_spec.local.name);
                }
                internal::ImportSpecifier::Named(named_spec) => {
                    has_named = true;
                    named_specs.push(named_spec);
                }
                internal::ImportSpecifier::Namespace(ns_spec) => {
                    has_namespace = true;
                    namespace_name = self.resolve_symbol(ns_spec.local.name);
                }
            }
        }

        // Build the full import statement
        let mut parts = vec![doc::text("import ")];

        // Add 'type' keyword for type-only imports
        if is_type_import {
            parts.push(doc::text("type "));
        }

        // Add default import
        if has_default {
            parts.push(doc::text(default_name));
        }

        // Add namespace import
        if has_namespace {
            if has_default {
                parts.push(doc::text(", "));
            }
            parts.push(doc::text("* as "));
            parts.push(doc::text(namespace_name));
        }

        // Build named specifiers with group wrapping (or empty braces if source had them)
        if has_named || has_empty_braces {
            if has_default || has_namespace {
                parts.push(doc::text(", "));
            }

            if named_specs.is_empty() {
                // Empty braces case: `import {} from 'x'`
                parts.push(doc::text("{}"));
            } else {
                // Build specifier docs with line breaks between them
                let mut spec_parts = Vec::new();
                for (i, named_spec) in named_specs.iter().enumerate() {
                    if i > 0 {
                        spec_parts.push(doc::text(","));
                        spec_parts.push(doc::line());
                    }
                    // Add inline type modifier if this specifier is type-only
                    // (only when the overall import is NOT type-only)
                    if !is_type_import && named_spec.import_kind == internal::ImportKind::Type {
                        spec_parts.push(doc::text("type "));
                    }
                    let imported = self.resolve_symbol(named_spec.imported.name);
                    let local = self.resolve_symbol(named_spec.local.name);
                    if imported == local {
                        spec_parts.push(doc::text(imported));
                    } else {
                        spec_parts.push(doc::text(imported));
                        spec_parts.push(doc::text(" as "));
                        spec_parts.push(doc::text(local));
                    }
                }
                // Add trailing comma only when broken
                spec_parts.push(doc::if_break(doc::text(","), doc::text("")));

                // Build the braces content (will be wrapped in outer group)
                parts.push(doc::text("{"));
                parts.push(doc::indent(doc::concat(vec![
                    doc::softline(),
                    doc::concat(spec_parts),
                ])));
                parts.push(doc::softline());
                parts.push(doc::text("}"));
            }
        }

        // Add "from" and source
        if !decl.specifiers.is_empty() || has_empty_braces {
            parts.push(doc::text(" from "));
        }
        parts.push(self.build_literal_doc(&decl.source));

        // Add import attributes: `with { type: "json" }`
        if !decl.attributes.is_empty() {
            parts.push(doc::text(" with {"));
            for (i, attr) in decl.attributes.iter().enumerate() {
                if i > 0 {
                    parts.push(doc::text(", "));
                }
                let key = self.resolve_symbol(attr.key.name);
                parts.push(doc::text(key));
                parts.push(doc::text(": "));
                parts.push(self.build_literal_doc(&attr.value));
            }
            parts.push(doc::text("}"));
        }

        parts.push(doc::text(";"));

        // Wrap entire statement in a group for width-based wrapping
        doc::group(doc::concat(parts))
    }
}
