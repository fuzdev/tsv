// Module statement printing for TypeScript (import and export)

use super::{Printer, build_entity_name_doc};
use crate::ast::internal;
use tsv_lang::SymbolToU32;
use tsv_lang::comments_in_range;
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
        if let Some(declaration) = &decl.declaration {
            // When exporting a declaration, always use plain "export " because
            // the type/interface/declare keyword is part of the declaration itself
            let export_keyword = "export ";
            // For decorated classes, decorators come before export keyword
            if let internal::Statement::ClassDeclaration(class) = declaration.as_ref()
                && let Some(dec_doc) = self.build_decorators_doc(class.decorators.as_deref())
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
            let export_keyword = match decl.export_kind {
                internal::ExportKind::Value => "export ",
                internal::ExportKind::Type => "export type ",
            };
            let mut parts = vec![d.text(export_keyword)];

            // Check if the overall export is type-only
            let is_type_export = decl.export_kind == internal::ExportKind::Type;

            if decl.specifiers.is_empty() {
                // Empty braces case: `export {}`
                parts.push(d.text("{}"));
            } else {
                let comment_boundary = decl.source.as_ref().map_or(decl.span.end, |s| s.span.start);

                // Find the opening brace position
                let first_start = decl.specifiers[0].span.start as usize;
                let brace_start = self.source[..first_start].rfind('{').unwrap_or(0) as u32;

                // Check for line comments between/around specifiers (force multiline)
                let has_line_comments = self.has_line_comments_in_delimited_list(
                    &decl.specifiers,
                    |s| s.span,
                    comment_boundary,
                ) || self
                    .has_line_comments_between(brace_start + 1, decl.specifiers[0].span.start);

                if has_line_comments {
                    // Comment-aware path: manually iterate with hardlines
                    let inner_doc = self.build_export_specifiers_with_comments(
                        &decl.specifiers,
                        is_type_export,
                        brace_start,
                        comment_boundary,
                    );
                    parts.push(d.text("{"));
                    parts.push(d.indent(d.concat(&[d.hardline(), inner_doc])));
                    parts.push(d.hardline());
                    parts.push(d.text("}"));
                } else {
                    // No line comments: use join_trailing with group-based wrapping
                    let spec_parts = d.join_trailing(
                        decl.specifiers
                            .iter()
                            .map(|spec| self.build_export_specifier_doc(spec, is_type_export)),
                        d.comma_line(),
                    );

                    // Check for trailing comments after last specifier
                    let last_spec_end = decl.specifiers.last().map_or(0, |s| s.span.end);
                    let trailing_comments_doc =
                        self.build_inline_comments_between_doc(last_spec_end, comment_boundary);

                    parts.push(d.text("{"));
                    parts.push(d.indent_softline(spec_parts));
                    parts.push(trailing_comments_doc);
                    parts.push(d.softline());
                    parts.push(d.text("}"));
                }
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
            && let Some(dec_doc) = self.build_decorators_doc(class.decorators.as_deref())
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
                // Find the opening brace position (search backward from first specifier)
                let first_start = named_specs[0].span.start as usize;
                let brace_start = self.source[..first_start].rfind('{').unwrap_or(0) as u32;

                // Check for line comments between/around specifiers (force multiline)
                let has_line_comments = self.has_line_comments_in_delimited_list(
                    &named_specs,
                    |s| s.span,
                    decl.source.span.start,
                ) || self
                    .has_line_comments_between(brace_start + 1, named_specs[0].span.start);

                if has_line_comments {
                    // Comment-aware path: manually iterate with hardlines
                    let inner_doc = self.build_import_specifiers_with_comments(
                        &named_specs,
                        is_type_import,
                        brace_start,
                        decl.source.span.start,
                    );
                    parts.push(d.text("{"));
                    parts.push(d.indent(d.concat(&[d.hardline(), inner_doc])));
                    parts.push(d.hardline());
                    parts.push(d.text("}"));
                } else {
                    // No line comments: use join_trailing with group-based wrapping
                    let spec_parts = d.join_trailing(
                        named_specs.iter().map(|named_spec| {
                            self.build_import_specifier_doc(named_spec, is_type_import)
                        }),
                        d.comma_line(),
                    );

                    // Check for trailing comments after last specifier (before closing brace)
                    let last_spec_end = named_specs.last().map_or(0, |s| s.span.end);
                    let trailing_comments_doc = self
                        .build_inline_comments_between_doc(last_spec_end, decl.source.span.start);

                    parts.push(d.text("{"));
                    parts.push(d.indent_softline(spec_parts));
                    parts.push(trailing_comments_doc);
                    parts.push(d.softline());
                    parts.push(d.text("}"));
                }
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

            // Find the opening brace position for comment checking
            let first_start = decl.attributes[0].span.start as usize;
            let brace_start = self.source[..first_start].rfind('{').unwrap_or(0) as u32;

            // Check for line comments between/around attributes (force multiline)
            let has_line_comments = self.has_line_comments_in_delimited_list(
                &decl.attributes,
                |a| a.span,
                decl.span.end,
            ) || self
                .has_line_comments_between(brace_start + 1, decl.attributes[0].span.start);

            if has_line_comments {
                let inner_doc = self.build_import_attributes_with_comments(
                    &decl.attributes,
                    brace_start,
                    decl.span.end,
                );
                parts.push(d.text("{"));
                parts.push(d.indent(d.concat(&[d.hardline(), inner_doc])));
                parts.push(d.hardline());
                parts.push(d.text("}"));
            } else {
                // No line comments: use join_trailing with group-based wrapping
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

                parts.push(d.text("{"));
                parts.push(d.indent_softline(attr_parts));
                parts.push(d.softline());
                parts.push(d.text("}"));
            }
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
                    for comment in comments_in_range(self.comments, require_open_end, literal_start)
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

    /// Build a doc for a single import specifier
    fn build_import_specifier_doc(
        &self,
        named_spec: &internal::ImportNamedSpecifier,
        is_type_import: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
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
    }

    /// Build a doc for a single export specifier
    fn build_export_specifier_doc(
        &self,
        spec: &internal::ExportSpecifier,
        is_type_export: bool,
    ) -> DocId {
        let d = self.d();
        let mut spec_parts = Vec::new();
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
    }

    /// Build import specifier list with comment-aware formatting (hardlines).
    /// Used when line comments exist between specifiers.
    fn build_import_specifiers_with_comments(
        &self,
        named_specs: &[&internal::ImportNamedSpecifier],
        is_type_import: bool,
        brace_start: u32,
        end_boundary: u32,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        let mut prev_end: u32 = brace_start + 1; // After opening brace

        for (i, named_spec) in named_specs.iter().enumerate() {
            let spec_start = named_spec.span.start;
            let is_first = i == 0;
            let is_last = i == named_specs.len() - 1;

            // Get comments between previous position and this specifier
            let search_start = self.leading_comment_search_start(prev_end, is_first);
            let comments: Vec<_> = comments_in_range(self.comments, search_start, spec_start)
                .filter(|c| is_first || c.is_block || !self.is_same_line(prev_end, c.span.start))
                .collect();

            if !is_first {
                // Check for blank line before comments or specifier
                let check_pos = if comments.is_empty() {
                    spec_start
                } else {
                    comments[0].span.start
                };
                if self.has_blank_line_between(search_start, check_pos) {
                    parts.push(d.literalline());
                }
                parts.push(d.hardline());
            }

            // Print leading comments
            for comment in &comments {
                parts.push(self.build_comment_doc(comment));
                if comment.is_block && self.is_same_line(comment.span.end, spec_start) {
                    // Block comment on same line as specifier - space after
                    parts.push(d.text(" "));
                } else {
                    parts.push(d.hardline());
                }
            }

            parts.push(self.build_import_specifier_doc(named_spec, is_type_import));
            parts.push(d.text(","));

            // Handle trailing inline comments on same line after specifier
            if !is_last {
                let next_start = named_specs[i + 1].span.start;
                let trailing: Vec<_> =
                    comments_in_range(self.comments, named_spec.span.end, next_start)
                        .filter(|c| self.is_same_line(named_spec.span.end, c.span.start))
                        .collect();
                for comment in &trailing {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
            } else {
                // Trailing comments after last specifier
                let trailing: Vec<_> =
                    comments_in_range(self.comments, named_spec.span.end, end_boundary)
                        .filter(|c| self.is_same_line(named_spec.span.end, c.span.start))
                        .collect();
                for comment in &trailing {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
            }

            prev_end = named_spec.span.end;
        }

        d.concat(&parts)
    }

    /// Build export specifier list with comment-aware formatting (hardlines).
    /// Used when line comments exist between specifiers.
    fn build_export_specifiers_with_comments(
        &self,
        specifiers: &[internal::ExportSpecifier],
        is_type_export: bool,
        brace_start: u32,
        end_boundary: u32,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        let mut prev_end: u32 = brace_start + 1; // After opening brace

        for (i, spec) in specifiers.iter().enumerate() {
            let spec_start = spec.span.start;
            let is_first = i == 0;
            let is_last = i == specifiers.len() - 1;

            // Get comments between previous position and this specifier
            let search_start = self.leading_comment_search_start(prev_end, is_first);
            let comments: Vec<_> = comments_in_range(self.comments, search_start, spec_start)
                .filter(|c| is_first || c.is_block || !self.is_same_line(prev_end, c.span.start))
                .collect();

            if !is_first {
                let check_pos = if comments.is_empty() {
                    spec_start
                } else {
                    comments[0].span.start
                };
                if self.has_blank_line_between(search_start, check_pos) {
                    parts.push(d.literalline());
                }
                parts.push(d.hardline());
            }

            for comment in &comments {
                parts.push(self.build_comment_doc(comment));
                if comment.is_block && self.is_same_line(comment.span.end, spec_start) {
                    parts.push(d.text(" "));
                } else {
                    parts.push(d.hardline());
                }
            }

            parts.push(self.build_export_specifier_doc(spec, is_type_export));
            parts.push(d.text(","));

            // Handle trailing inline comments
            if !is_last {
                let next_start = specifiers[i + 1].span.start;
                let trailing: Vec<_> = comments_in_range(self.comments, spec.span.end, next_start)
                    .filter(|c| self.is_same_line(spec.span.end, c.span.start))
                    .collect();
                for comment in &trailing {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
            } else {
                let trailing: Vec<_> =
                    comments_in_range(self.comments, spec.span.end, end_boundary)
                        .filter(|c| self.is_same_line(spec.span.end, c.span.start))
                        .collect();
                for comment in &trailing {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
            }

            prev_end = spec.span.end;
        }

        d.concat(&parts)
    }

    /// Build import attribute list with comment-aware formatting (hardlines).
    /// Used when line comments exist between attributes.
    fn build_import_attributes_with_comments(
        &self,
        attributes: &[internal::ImportAttribute],
        brace_start: u32,
        end_boundary: u32,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        let mut prev_end: u32 = brace_start + 1; // After opening brace

        for (i, attr) in attributes.iter().enumerate() {
            let attr_start = attr.span.start;
            let is_first = i == 0;
            let is_last = i == attributes.len() - 1;

            let search_start = self.leading_comment_search_start(prev_end, is_first);
            let comments: Vec<_> = comments_in_range(self.comments, search_start, attr_start)
                .filter(|c| is_first || c.is_block || !self.is_same_line(prev_end, c.span.start))
                .collect();

            if !is_first {
                let check_pos = if comments.is_empty() {
                    attr_start
                } else {
                    comments[0].span.start
                };
                if self.has_blank_line_between(search_start, check_pos) {
                    parts.push(d.literalline());
                }
                parts.push(d.hardline());
            }

            for comment in &comments {
                parts.push(self.build_comment_doc(comment));
                if comment.is_block && self.is_same_line(comment.span.end, attr_start) {
                    parts.push(d.text(" "));
                } else {
                    parts.push(d.hardline());
                }
            }

            // Build attribute: `key: value`
            parts.push(d.concat(&[
                d.symbol(attr.key.name.to_u32()),
                d.text(": "),
                self.build_literal_doc(&attr.value),
            ]));
            parts.push(d.text(","));

            // Handle trailing inline comments
            let upper = if !is_last {
                attributes[i + 1].span.start
            } else {
                end_boundary
            };
            let trailing: Vec<_> = comments_in_range(self.comments, attr.span.end, upper)
                .filter(|c| self.is_same_line(attr.span.end, c.span.start))
                .collect();
            for comment in &trailing {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }

            prev_end = attr.span.end;
        }

        d.concat(&parts)
    }
}
