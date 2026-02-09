// Module statement printing for TypeScript (import and export)

use super::{Printer, build_entity_name_doc};
use crate::ast::internal;
use tsv_lang::SymbolToU32;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build a Doc for a TypeScript export assignment
    pub(super) fn build_export_assignment_doc(&self, decl: &internal::TSExportAssignment) -> DocId {
        let d = self.d();
        d.concat(&[
            d.text("export = "),
            self.build_expression_doc(&decl.expression),
            d.text(";"),
        ])
    }

    /// Build a Doc for an export named declaration
    ///
    /// Uses d.group() for width-based wrapping of specifiers.
    /// When the line exceeds print_width (100 chars), wraps to multiline format.
    pub(super) fn build_export_named_declaration_doc(
        &self,
        decl: &internal::ExportNamedDeclaration,
    ) -> DocId {
        let d = self.d();
        let export_keyword = match decl.export_kind {
            internal::ExportKind::Value => "export ",
            internal::ExportKind::Type => "export type ",
        };
        if let Some(declaration) = &decl.declaration {
            // For decorated classes, decorators come before export keyword
            if let internal::Statement::ClassDeclaration(class) = declaration.as_ref()
                && let Some(dec_doc) = self.build_decorators_doc(class.decorators.as_ref())
            {
                return d.concat(&[
                    dec_doc,
                    d.text(export_keyword),
                    self.build_class_declaration_without_decorators_doc(class),
                ]);
            }
            d.concat(&[
                d.text(export_keyword),
                self.build_statement_doc(declaration),
            ])
        } else {
            // export { x, y as z } or export { x } from "y"
            let mut parts = vec![d.text(export_keyword)];

            // Check if the overall export is type-only
            let is_type_export = decl.export_kind == internal::ExportKind::Type;

            if decl.specifiers.is_empty() {
                // Empty braces case: `export {}`
                parts.push(d.text("{}"));
            } else {
                // Build specifier docs with line breaks between them
                let spec_parts = d.join_trailing(
                    decl.specifiers.iter().map(|spec| {
                        let mut spec_parts = Vec::new();
                        // Add inline type modifier if this specifier is type-only
                        // (only when the overall export is NOT type-only)
                        if !is_type_export && spec.export_kind == internal::ExportKind::Type {
                            spec_parts.push(d.text("type "));
                        }
                        let local_sym = spec.local.name.to_u32();
                        let exported_sym = spec.exported.name.to_u32();
                        if local_sym == exported_sym {
                            spec_parts.push(d.symbol(local_sym));
                        } else {
                            spec_parts.push(d.symbol(local_sym));
                            spec_parts.push(d.text(" as "));
                            spec_parts.push(d.symbol(exported_sym));
                        }
                        d.concat(&spec_parts)
                    }),
                    d.comma_line(),
                );

                // Check for trailing comments after last specifier (before closing brace)
                // e.g., `export {a /*, b*/}`
                let last_spec_end = decl.specifiers.last().map_or(0, |s| s.span.end);
                // Find the boundary - either the source literal or the end of declaration
                let comment_boundary = decl.source.as_ref().map_or(decl.span.end, |s| s.span.start);
                let trailing_comments_doc =
                    self.build_inline_comments_between_doc(last_spec_end, comment_boundary);

                // Build the braces content (will be wrapped in outer group)
                parts.push(d.text("{"));
                parts.push(d.indent_softline(spec_parts));
                parts.push(trailing_comments_doc);
                parts.push(d.softline());
                parts.push(d.text("}"));
            }

            if let Some(source) = &decl.source {
                parts.push(d.text(" from "));
                parts.push(self.build_literal_doc(source));
            }
            parts.push(d.text(";"));

            // Wrap entire statement in a group for width-based wrapping
            d.group(d.concat(&parts))
        }
    }

    /// Build a Doc for an export default declaration
    pub(super) fn build_export_default_declaration_doc(
        &self,
        decl: &internal::ExportDefaultDeclaration,
    ) -> DocId {
        let d = self.d();
        // For decorated classes, decorators come before export keyword
        if let internal::ExportDefaultValue::ClassDeclaration(class) = &decl.declaration
            && let Some(dec_doc) = self.build_decorators_doc(class.decorators.as_ref())
        {
            return d.concat(&[
                dec_doc,
                d.text("export default "),
                self.build_class_declaration_without_decorators_doc(class),
            ]);
        }

        let value_doc = match &decl.declaration {
            internal::ExportDefaultValue::Expression(expr) => {
                d.concat(&[self.build_expression_doc(expr), d.text(";")])
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
        d.concat(&[d.text("export default "), value_doc])
    }

    /// Build a Doc for an export all declaration
    pub(super) fn build_export_all_declaration_doc(
        &self,
        decl: &internal::ExportAllDeclaration,
    ) -> DocId {
        let d = self.d();
        let export_keyword = match decl.export_kind {
            internal::ExportKind::Value => "export *",
            internal::ExportKind::Type => "export type *",
        };
        let mut parts = vec![d.text(export_keyword)];
        if let Some(exported) = &decl.exported {
            parts.push(d.text(" as "));
            parts.push(d.symbol(exported.name.to_u32()));
        }
        parts.push(d.text(" from "));
        parts.push(self.build_literal_doc(&decl.source));
        parts.push(d.text(";"));
        d.concat(&parts)
    }

    /// Check if an import declaration has empty named braces `{}` in source.
    /// This distinguishes `import {} from 'x'` from `import 'x'`.
    fn has_empty_named_braces(&self, decl: &internal::ImportDeclaration) -> bool {
        let text = decl.span.extract(self.source);
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

    /// Build a Doc for an import declaration
    ///
    /// Uses d.group() for width-based wrapping of named specifiers.
    /// When the line exceeds print_width (100 chars), wraps to multiline format.
    pub(super) fn build_import_declaration_doc(&self, decl: &internal::ImportDeclaration) -> DocId {
        let d = self.d();
        // Check if source has empty braces (for `import {} from 'x'`)
        let has_empty_braces = self.has_empty_named_braces(decl);

        // Check if this is a type-only import
        let is_type_import = decl.import_kind == internal::ImportKind::Type;

        // Collect specifiers
        let mut has_default = false;
        let mut has_named = false;
        let mut has_namespace = false;
        let mut named_specs = Vec::new();
        let mut default_sym = 0u32;
        let mut namespace_sym = 0u32;

        for spec in &decl.specifiers {
            match spec {
                internal::ImportSpecifier::Default(default_spec) => {
                    has_default = true;
                    default_sym = default_spec.local.name.to_u32();
                }
                internal::ImportSpecifier::Named(named_spec) => {
                    has_named = true;
                    named_specs.push(named_spec);
                }
                internal::ImportSpecifier::Namespace(ns_spec) => {
                    has_namespace = true;
                    namespace_sym = ns_spec.local.name.to_u32();
                }
            }
        }

        // Build the full import statement
        let mut parts = vec![d.text("import ")];

        // Add 'type' keyword for type-only imports
        if is_type_import {
            parts.push(d.text("type "));
        }

        // Add default import
        if has_default {
            parts.push(d.symbol(default_sym));
        }

        // Add namespace import
        if has_namespace {
            if has_default {
                parts.push(d.text(", "));
            }
            parts.push(d.text("* as "));
            parts.push(d.symbol(namespace_sym));
        }

        // Build named specifiers with group wrapping (or empty braces if source had them)
        if has_named || has_empty_braces {
            if has_default || has_namespace {
                parts.push(d.text(", "));
            }

            if named_specs.is_empty() {
                // Empty braces case: `import {} from 'x'`
                parts.push(d.text("{}"));
            } else {
                // Build specifier docs with line breaks between them
                let spec_parts = d.join_trailing(
                    named_specs.iter().map(|named_spec| {
                        let mut parts = Vec::new();
                        // Add inline type modifier if this specifier is type-only
                        // (only when the overall import is NOT type-only)
                        if !is_type_import && named_spec.import_kind == internal::ImportKind::Type {
                            parts.push(d.text("type "));
                        }
                        let imported_sym = named_spec.imported.name.to_u32();
                        let local_sym = named_spec.local.name.to_u32();
                        if imported_sym == local_sym {
                            parts.push(d.symbol(imported_sym));
                        } else {
                            parts.push(d.symbol(imported_sym));
                            parts.push(d.text(" as "));
                            parts.push(d.symbol(local_sym));
                        }
                        d.concat(&parts)
                    }),
                    d.comma_line(),
                );

                // Check for trailing comments after last specifier (before closing brace)
                // e.g., `import {a /*, b*/} from 'x'`
                let last_spec_end = named_specs.last().map_or(0, |s| s.span.end);
                let trailing_comments_doc =
                    self.build_inline_comments_between_doc(last_spec_end, decl.source.span.start);

                // Build the braces content (will be wrapped in outer group)
                parts.push(d.text("{"));
                parts.push(d.indent_softline(spec_parts));
                parts.push(trailing_comments_doc);
                parts.push(d.softline());
                parts.push(d.text("}"));
            }
        }

        // Add "from" and source
        if !decl.specifiers.is_empty() || has_empty_braces {
            parts.push(d.text(" from "));
        }
        parts.push(self.build_literal_doc(&decl.source));

        // Add import attributes: `with { type: "json" }`
        // PR #17329 (prettier 3.7): Break attributes across lines when long
        if !decl.attributes.is_empty() {
            parts.push(d.text(" with "));

            // Build attribute docs with line breaks between them
            let attr_parts = d.join_trailing(
                decl.attributes.iter().map(|attr| {
                    d.concat(&[
                        d.symbol(attr.key.name.to_u32()),
                        d.text(": "),
                        self.build_literal_doc(&attr.value),
                    ])
                }),
                d.comma_line(),
            );

            // Build the braces content with same pattern as named specifiers
            parts.push(d.text("{"));
            parts.push(d.indent_softline(attr_parts));
            parts.push(d.softline());
            parts.push(d.text("}"));
        }

        parts.push(d.text(";"));

        // Wrap entire statement in a group for width-based wrapping
        d.group(d.concat(&parts))
    }

    /// Build doc for `import x = require("y")` or `import x = A.B`
    pub(super) fn build_import_equals_declaration_doc(
        &self,
        decl: &internal::TSImportEqualsDeclaration,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // Export prefix if present
        if decl.is_export {
            parts.push(d.text("export "));
        }

        // import keyword
        parts.push(d.text("import "));

        // type modifier if present
        if matches!(decl.import_kind, internal::ImportKind::Type) {
            parts.push(d.text("type "));
        }

        // identifier
        parts.push(d.symbol(decl.id.name.to_u32()));

        // = sign
        parts.push(d.text(" = "));

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
                        comment_parts.push(d.hardline());
                    }

                    parts.push(d.text("require("));
                    parts.push(d.indent(d.concat(&[
                        d.hardline(),
                        d.concat(&comment_parts),
                        self.build_literal_doc(&ext_ref.expression),
                    ])));
                    parts.push(d.hardline());
                    parts.push(d.text(")"));
                } else {
                    // Check for inline block comments
                    let has_inline_comments =
                        self.has_comments_between(require_open_end, literal_start);
                    if has_inline_comments {
                        parts.push(d.text("require("));
                        parts.push(
                            self.build_inline_comments_between_doc(require_open_end, literal_start),
                        );
                        parts.push(self.build_literal_doc(&ext_ref.expression));
                        parts.push(d.text(")"));
                    } else {
                        // Simple compact format
                        parts.push(d.text("require("));
                        parts.push(self.build_literal_doc(&ext_ref.expression));
                        parts.push(d.text(")"));
                    }
                }
            }
            internal::TSModuleReference::EntityName(entity_name) => {
                parts.push(build_entity_name_doc(d, entity_name));
            }
        }

        parts.push(d.text(";"));

        d.concat(&parts)
    }
}
