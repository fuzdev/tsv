// Module statement printing for TypeScript (import and export)

use super::super::Printer;
use crate::ast::internal;
use string_interner::Symbol;
use tsv_lang::{SymbolResolver, doc};

impl<'a> Printer<'a> {
    /// Print an export named declaration
    ///
    /// Uses doc builder with width-based wrapping for specifiers.
    pub(super) fn print_export_named_declaration(
        &mut self,
        decl: &internal::ExportNamedDeclaration,
    ) {
        let export_keyword = match decl.export_kind {
            internal::ExportKind::Value => "export ",
            internal::ExportKind::Type => "export type ",
        };
        if let Some(declaration) = &decl.declaration {
            // For decorated classes, print decorators before export keyword
            if let internal::Statement::ClassDeclaration(class_decl) = declaration.as_ref()
                && !class_decl.decorators.is_empty()
            {
                for decorator in &class_decl.decorators {
                    self.print_decorator(decorator);
                    self.write("\n");
                    self.write_indent();
                }
                // Print export keyword and class without decorators (already printed above)
                self.write(export_keyword);
                let mut class_without_decorators = class_decl.clone();
                class_without_decorators.decorators = Vec::new();
                self.print_class_declaration(&class_without_decorators);
                return;
            }

            // export const x = 1; - use direct printing for declarations
            self.write(export_keyword);
            self.print_statement(declaration);
        } else {
            // export { x, y as z } or export { x } from "y" - use doc builder
            let doc = self.build_export_named_declaration_doc(decl);
            self.write_doc(&doc);
        }
    }

    /// Print an export default declaration
    pub(super) fn print_export_default_declaration(
        &mut self,
        decl: &internal::ExportDefaultDeclaration,
    ) {
        // For decorated classes, print decorators before export keyword
        if let internal::ExportDefaultValue::ClassDeclaration(class) = &decl.declaration
            && !class.decorators.is_empty()
        {
            for decorator in &class.decorators {
                self.print_decorator(decorator);
                self.write("\n");
                self.write_indent();
            }
            // Print export default and class without decorators
            self.write("export default ");
            let mut class_without_decorators = class.as_ref().clone();
            class_without_decorators.decorators = Vec::new();
            self.print_class_declaration(&class_without_decorators);
            return;
        }

        self.write("export default ");
        match &decl.declaration {
            internal::ExportDefaultValue::Expression(expr) => {
                self.print_expression(expr);
                self.write(";");
            }
            internal::ExportDefaultValue::FunctionDeclaration(func) => {
                self.print_function_declaration(func);
            }
            internal::ExportDefaultValue::TSDeclareFunction(func) => {
                self.print_declare_function(func);
            }
            internal::ExportDefaultValue::ClassDeclaration(class) => {
                self.print_class_declaration(class);
            }
        }
    }

    /// Print an export all declaration
    pub(super) fn print_export_all_declaration(&mut self, decl: &internal::ExportAllDeclaration) {
        match decl.export_kind {
            internal::ExportKind::Value => self.write("export *"),
            internal::ExportKind::Type => self.write("export type *"),
        }
        if let Some(exported) = &decl.exported {
            self.write(" as ");
            self.write(&self.resolve_symbol(exported.name));
        }
        self.write(" from ");
        self.print_literal(&decl.source);
        self.write(";");
    }

    /// Print a TypeScript export assignment: `export = value;`
    pub(super) fn print_export_assignment(&mut self, decl: &internal::TSExportAssignment) {
        self.write("export = ");
        self.print_expression(&decl.expression);
        self.write(";");
    }

    /// Build a Doc for a TypeScript export assignment
    pub(super) fn build_export_assignment_doc(
        &self,
        decl: &internal::TSExportAssignment,
    ) -> doc::Doc {
        doc::concat(vec![
            doc::text("export = "),
            self.build_expression_doc(&decl.expression),
            doc::text(";"),
        ])
    }

    /// Build a Doc for an export named declaration
    ///
    /// Uses doc::group() for width-based wrapping of specifiers.
    /// When the line exceeds print_width (100 chars), wraps to multiline format.
    pub(super) fn build_export_named_declaration_doc(
        &self,
        decl: &internal::ExportNamedDeclaration,
    ) -> doc::Doc {
        let export_keyword = match decl.export_kind {
            internal::ExportKind::Value => "export ",
            internal::ExportKind::Type => "export type ",
        };
        if let Some(declaration) = &decl.declaration {
            doc::concat(vec![
                doc::text(export_keyword),
                self.build_statement_doc(declaration),
            ])
        } else {
            // export { x, y as z } or export { x } from "y"
            let mut parts = vec![doc::text(export_keyword)];

            if decl.specifiers.is_empty() {
                // Empty braces case: `export {}`
                parts.push(doc::text("{}"));
            } else {
                // Build specifier docs with line breaks between them
                let spec_docs: Vec<_> = decl
                    .specifiers
                    .iter()
                    .map(|spec| {
                        let local = self.resolve_symbol(spec.local.name);
                        let exported = self.resolve_symbol(spec.exported.name);
                        if local == exported {
                            doc::text_owned(local)
                        } else {
                            doc::concat(vec![
                                doc::text_owned(local),
                                doc::text(" as "),
                                doc::text_owned(exported),
                            ])
                        }
                    })
                    .collect();
                let spec_parts = doc::join_trailing(spec_docs, doc::comma_line());

                // Build the braces content (will be wrapped in outer group)
                parts.push(doc::text("{"));
                parts.push(doc::indent(doc::concat(vec![doc::softline(), spec_parts])));
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
            internal::ExportDefaultValue::TSDeclareFunction(func) => {
                self.build_declare_function_doc(func)
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
        let export_keyword = match decl.export_kind {
            internal::ExportKind::Value => "export *",
            internal::ExportKind::Type => "export type *",
        };
        let mut parts = vec![doc::text(export_keyword)];
        if let Some(exported) = &decl.exported {
            parts.push(doc::text(" as "));
            parts.push(doc::symbol(exported.name.to_usize() as u32));
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
        self.write_doc(&doc);
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
            parts.push(doc::text_owned(default_name));
        }

        // Add namespace import
        if has_namespace {
            if has_default {
                parts.push(doc::text(", "));
            }
            parts.push(doc::text("* as "));
            parts.push(doc::text_owned(namespace_name));
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
                let spec_docs: Vec<_> = named_specs
                    .iter()
                    .map(|named_spec| {
                        let mut parts = Vec::new();
                        // Add inline type modifier if this specifier is type-only
                        // (only when the overall import is NOT type-only)
                        if !is_type_import && named_spec.import_kind == internal::ImportKind::Type {
                            parts.push(doc::text("type "));
                        }
                        let imported = self.resolve_symbol(named_spec.imported.name);
                        let local = self.resolve_symbol(named_spec.local.name);
                        if imported == local {
                            parts.push(doc::text_owned(imported));
                        } else {
                            parts.push(doc::text_owned(imported));
                            parts.push(doc::text(" as "));
                            parts.push(doc::text_owned(local));
                        }
                        doc::concat(parts)
                    })
                    .collect();
                let spec_parts = doc::join_trailing(spec_docs, doc::comma_line());

                // Build the braces content (will be wrapped in outer group)
                parts.push(doc::text("{"));
                parts.push(doc::indent(doc::concat(vec![doc::softline(), spec_parts])));
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
        // PR #17329 (prettier 3.7): Break attributes across lines when long
        if !decl.attributes.is_empty() {
            parts.push(doc::text(" with "));

            // Build attribute docs with line breaks between them
            let attr_docs: Vec<_> = decl
                .attributes
                .iter()
                .map(|attr| {
                    let key = self.resolve_symbol(attr.key.name);
                    doc::concat(vec![
                        doc::text_owned(key),
                        doc::text(": "),
                        self.build_literal_doc(&attr.value),
                    ])
                })
                .collect();
            let attr_parts = doc::join_trailing(attr_docs, doc::comma_line());

            // Build the braces content with same pattern as named specifiers
            parts.push(doc::text("{"));
            parts.push(doc::indent(doc::concat(vec![doc::softline(), attr_parts])));
            parts.push(doc::softline());
            parts.push(doc::text("}"));
        }

        parts.push(doc::text(";"));

        // Wrap entire statement in a group for width-based wrapping
        doc::group(doc::concat(parts))
    }

    /// Print `import x = require("y")` or `import x = A.B`
    pub(super) fn print_import_equals_declaration(
        &mut self,
        decl: &internal::TSImportEqualsDeclaration,
    ) {
        let doc = self.build_import_equals_declaration_doc(decl);
        self.write_doc(&doc);
    }

    /// Build doc for `import x = require("y")` or `import x = A.B`
    pub(super) fn build_import_equals_declaration_doc(
        &self,
        decl: &internal::TSImportEqualsDeclaration,
    ) -> doc::Doc {
        let mut parts = Vec::new();

        // Export prefix if present
        if decl.is_export {
            parts.push(doc::text("export "));
        }

        // import keyword
        parts.push(doc::text("import "));

        // type modifier if present
        if matches!(decl.import_kind, internal::ImportKind::Type) {
            parts.push(doc::text("type "));
        }

        // identifier
        parts.push(doc::text_owned(self.resolve_symbol(decl.id.name)));

        // = sign
        parts.push(doc::text(" = "));

        // module reference
        match &decl.module_reference {
            internal::TSModuleReference::ExternalModuleReference(ext_ref) => {
                // Check for comments inside require() - expand if present
                // The require() span includes `require(` at start and `)` at end
                // Comments can be between `require(` and the string literal
                let require_open_end = ext_ref.span.start + 8; // after "require("
                let literal_start = ext_ref.expression.span.start;
                let has_comments = self.has_line_comments_between(require_open_end, literal_start);

                if has_comments {
                    // Multi-line format with comments
                    // Build comments doc: each comment on its own line
                    let mut comment_parts = Vec::new();
                    for comment in
                        tsv_lang::comments_in_range(self.comments, require_open_end, literal_start)
                    {
                        comment_parts.push(self.build_comment_doc(comment));
                        comment_parts.push(doc::hardline());
                    }

                    parts.push(doc::text("require("));
                    parts.push(doc::indent(doc::concat(vec![
                        doc::hardline(),
                        doc::concat(comment_parts),
                        self.build_literal_doc(&ext_ref.expression),
                    ])));
                    parts.push(doc::hardline());
                    parts.push(doc::text(")"));
                } else {
                    // Check for inline block comments
                    let has_inline_comments =
                        self.has_comments_between(require_open_end, literal_start);
                    if has_inline_comments {
                        parts.push(doc::text("require("));
                        parts.push(
                            self.build_inline_comments_between_doc(require_open_end, literal_start),
                        );
                        parts.push(self.build_literal_doc(&ext_ref.expression));
                        parts.push(doc::text(")"));
                    } else {
                        // Simple compact format
                        parts.push(doc::text("require("));
                        parts.push(self.build_literal_doc(&ext_ref.expression));
                        parts.push(doc::text(")"));
                    }
                }
            }
            internal::TSModuleReference::EntityName(entity_name) => {
                parts.push(super::super::build_entity_name_doc(entity_name));
            }
        }

        parts.push(doc::text(";"));

        doc::concat(parts)
    }
}
