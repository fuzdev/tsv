// Module statement printing for TypeScript (import and export)

use super::{Printer, build_entity_name_doc};
use crate::ast::internal;
use crate::printer::CommentSpacing;
use crate::printer::analysis::find_char_skipping_comments;
use tsv_lang::SymbolToU32;
use tsv_lang::comments_in_range;
use tsv_lang::doc::arena::DocId;

/// Check if a string contains only whitespace and/or comments.
/// Used to detect empty braces that may contain comments: `{ /* c */ }`.
fn is_only_whitespace_and_comments(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                // Block comment: scan to */
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 < bytes.len() {
                    i += 2;
                } else {
                    return false;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                // Line comment: scan to newline
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            _ => return false,
        }
    }
    true
}

impl<'a> Printer<'a> {
    /// Build a Doc for a TypeScript export assignment
    pub(super) fn build_export_assignment_doc(&self, decl: &internal::TSExportAssignment) -> DocId {
        let d = self.d();
        let expr_doc = self.build_expression_doc(&decl.expression);
        let argument_end = decl.expression.span().end;
        let has_trailing_comments = self.has_comments_between(argument_end, decl.span.end);
        if has_trailing_comments {
            let mut parts = vec![d.text("export = "), expr_doc];
            self.append_trailing_paren_comments(&mut parts, argument_end, decl.span.end);
            parts.push(d.text(";"));
            d.concat(&parts)
        } else {
            d.concat(&[d.text("export = "), expr_doc, d.text(";")])
        }
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
            let export_keyword = "export";
            let export_keyword_end = decl.span.start + export_keyword.len() as u32;
            let decl_start = declaration.span().start;

            // Check for comments between `export` and declaration.
            // Line comments need hardline after to prevent absorbing the declaration.
            let has_line = self.has_line_comments_between(export_keyword_end, decl_start);
            let comment_doc = if has_line {
                self.build_name_to_type_params_comments(
                    export_keyword_end,
                    decl_start,
                    CommentSpacing::Leading,
                )
            } else if self.has_comments_between(export_keyword_end, decl_start) {
                self.build_inline_comments_between_doc(export_keyword_end, decl_start)
            } else {
                d.empty()
            };
            // After line comments, hardline provides separation; otherwise need a space
            let space_after = if has_line { d.empty() } else { d.text(" ") };

            // For decorated classes, decorators come before export keyword.
            // Find the `export` keyword position — decl.span.start may include decorators
            // in the internal AST, so search from the last decorator end.
            if let internal::Statement::ClassDeclaration(class) = declaration.as_ref()
                && let Some(dec_doc) = self.build_decorators_doc(
                    class.decorators.as_deref(),
                    self.find_keyword_after_decorators(
                        class.decorators.as_deref(),
                        "export",
                        decl.span.start,
                    ),
                )
            {
                return d.concat(&[
                    dec_doc,
                    d.text(export_keyword),
                    comment_doc,
                    space_after,
                    self.build_class_declaration_without_decorators_doc(class),
                ]);
            }
            d.concat(&[
                d.text(export_keyword),
                comment_doc,
                space_after,
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

            // Position just past the specifier list's closing `}` (no-source case);
            // used to scan for comments between `}` and the terminating `;`.
            // Set unconditionally in both the empty and non-empty branches below.
            let close_brace_end: u32;

            if decl.specifiers.is_empty() {
                // Empty braces case: `export {}` or `export /* c */ {}`
                // Extract comments between keyword and braces + inside braces.
                // Prettier relocates comments from inside empty braces:
                //   `export { /* c */ }` → `export /* c */ {}`
                //   `export { /* c */ } from 'a'` → `export {} from /* c */ 'a'`
                let keyword_end = decl.span.start + export_keyword.trim_end().len() as u32;
                let semi_or_source = decl.source.as_ref().map_or(decl.span.end, |s| s.span.start);
                // Find closing brace outside of comments — naive find('}') matches
                // inside comments like `export // {}\n{}`, breaking comment extraction.
                let brace_close = self
                    .find_char_outside_comments(keyword_end, semi_or_source, b'}')
                    .unwrap_or(semi_or_source);
                close_brace_end = brace_close + 1;
                if decl.source.is_none() {
                    // No re-export: comments go before braces (`export /* c */ {}`)
                    if let Some(comments_doc) =
                        self.build_rhs_comments_opt(keyword_end, brace_close)
                    {
                        parts.push(comments_doc);
                        // Line comments end with hardline; add space before `{}`
                        // to match prettier's continuation indent (block comments
                        // already have trailing space from build_rhs_comments_opt)
                        if self.has_line_comments_between(keyword_end, brace_close) {
                            parts.push(d.text(" "));
                        }
                    }
                }
                // Preserve comments between keyword and `{` for re-exports.
                // Without this, `export /* c */ {} from 'x'` silently drops the comment.
                if decl.source.is_some()
                    && let Some(brace_pos) =
                        self.find_char_outside_comments(keyword_end, semi_or_source, b'{')
                    && let Some(comments_doc) = self.build_rhs_comments_opt(keyword_end, brace_pos)
                {
                    parts.push(comments_doc);
                }
                // Comments inside braces are captured in from-to-source below
                parts.push(d.text("{}"));
            } else {
                // Find the opening and closing brace positions.
                // Use forward search from keyword end to skip `{` inside comments.
                let export_kw_end = decl.span.start + export_keyword.trim_end().len() as u32;
                let first_start = decl.specifiers[0].span.start;
                let brace_start = self
                    .find_char_outside_comments(export_kw_end, first_start, b'{')
                    .unwrap_or(0);
                let last_spec_end = decl.specifiers.last().map_or(0, |s| s.span.end);
                let semi_or_source = decl.source.as_ref().map_or(decl.span.end, |s| s.span.start);
                let brace_close = self.source[last_spec_end as usize..semi_or_source as usize]
                    .find('}')
                    .map_or(semi_or_source, |p| last_spec_end + p as u32);
                close_brace_end = brace_close + 1;

                // Check for expanding comments (force multiline):
                // line comments, or own-line single-line block comments
                let brace_span = tsv_lang::Span::new(brace_start, brace_close + 1);
                let has_expanding_comments = self.has_line_comments_in_delimited_list(
                    &decl.specifiers,
                    |s| s.span,
                    brace_close,
                ) || self
                    .has_line_comments_between(brace_start + 1, decl.specifiers[0].span.start)
                    || self.has_own_line_block_comments_in_bracket_list(
                        brace_span,
                        &decl.specifiers,
                        |s| s.span,
                    );

                if has_expanding_comments {
                    // Comment-aware path: manually iterate with hardlines
                    let inner_doc = self.build_hardline_comma_list(
                        &decl.specifiers,
                        brace_start,
                        brace_close,
                        |s| s.span,
                        |s| self.build_export_specifier_doc(s, is_type_export),
                    );
                    parts.push(d.text("{"));
                    parts.push(d.indent(d.concat(&[d.hardline(), inner_doc])));
                    parts.push(d.hardline());
                    parts.push(d.text("}"));
                } else {
                    // No expanding comments: group-based wrapping with comment splitting
                    let spec_doc = self.build_softline_comma_list(
                        &decl.specifiers,
                        brace_start,
                        brace_close,
                        |s| s.span,
                        |s| self.build_export_specifier_doc(s, is_type_export),
                    );

                    parts.push(d.text("{"));
                    parts.push(d.indent_softline(spec_doc));
                    parts.push(d.softline());
                    parts.push(d.text("}"));
                }
            }

            if let Some(source) = &decl.source {
                let empty_brace_start = if decl.specifiers.is_empty() {
                    Some(decl.span.start + export_keyword.trim_end().len() as u32)
                } else {
                    None
                };
                parts.push(self.build_from_source_doc(decl.span.start, source, empty_brace_start));
            }

            // Comments between the closing `}` (or source literal) and the
            // terminating `;` — preserved where the user placed them (prettier
            // relocates no-`from` ones inside the braces). Emitted outside the
            // content group so a line-comment break doesn't expand the braces.
            let content_end = decl.source.as_ref().map_or(close_brace_end, |s| s.span.end);
            let mut trailing = Vec::new();
            let broke = self.append_pre_semi_comments(&mut trailing, content_end, decl.span.end);
            if trailing.is_empty() {
                parts.push(d.text(";"));
                // Wrap entire statement in a group for width-based wrapping
                d.group(d.concat(&parts))
            } else {
                let group = d.group(d.concat(&parts));
                trailing.insert(0, group);
                // A line comment ends its line — the `;` must follow on a new line.
                if broke {
                    trailing.push(d.hardline());
                }
                trailing.push(d.text(";"));
                d.concat(&trailing)
            }
        }
    }

    /// Build a Doc for an export default declaration
    pub(super) fn build_export_default_declaration_doc(
        &self,
        decl: &internal::ExportDefaultDeclaration,
    ) -> DocId {
        let d = self.d();
        // For decorated classes, decorators come before export keyword.
        // Find the `export` keyword position (same issue as named exports — span may include decorators).
        if let internal::ExportDefaultValue::ClassDeclaration(class) = &decl.declaration
            && let Some(dec_doc) = self.build_decorators_doc(
                class.decorators.as_deref(),
                self.find_keyword_after_decorators(
                    class.decorators.as_deref(),
                    "export",
                    decl.span.start,
                ),
            )
        {
            return d.concat(&[
                dec_doc,
                d.text("export default "),
                self.build_class_declaration_without_decorators_doc(class),
            ]);
        }

        let value_doc = match &decl.declaration {
            internal::ExportDefaultValue::Expression(expr) => {
                let expr_doc = self.build_expression_doc(expr);
                let argument_end = expr.span().end;
                let has_trailing_comments = self.has_comments_between(argument_end, decl.span.end);
                if has_trailing_comments {
                    let mut parts = vec![expr_doc];
                    self.append_trailing_paren_comments(&mut parts, argument_end, decl.span.end);
                    parts.push(d.text(";"));
                    d.concat(&parts)
                } else {
                    d.concat(&[expr_doc, d.text(";")])
                }
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

        // Check for comments between `export default` and declaration.
        // Line comments need hardline after to prevent absorbing the declaration.
        let default_keyword = "export default";
        let keyword_end = decl.span.start + default_keyword.len() as u32;
        let decl_start = match &decl.declaration {
            internal::ExportDefaultValue::Expression(expr) => expr.span().start,
            internal::ExportDefaultValue::FunctionDeclaration(func) => func.span.start,
            internal::ExportDefaultValue::TSDeclareFunction(func) => func.span.start,
            internal::ExportDefaultValue::ClassDeclaration(class) => class.span.start,
        };
        let has_line = self.has_line_comments_between(keyword_end, decl_start);
        if has_line {
            let comment_doc = self.build_name_to_type_params_comments(
                keyword_end,
                decl_start,
                CommentSpacing::Leading,
            );
            d.concat(&[d.text("export default"), comment_doc, value_doc])
        } else if self.has_comments_between(keyword_end, decl_start) {
            let comment_doc = self.build_inline_comments_between_doc(keyword_end, decl_start);
            d.concat(&[
                d.text("export default"),
                comment_doc,
                d.text(" "),
                value_doc,
            ])
        } else {
            d.concat(&[d.text("export default "), value_doc])
        }
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
            // Comments between `as` and exported name
            let star_pos = decl.span.start + export_keyword.len() as u32;
            let as_end = self
                .find_keyword_in_range(star_pos, exported.span.start, "as")
                .map_or(exported.span.start, |p| p + 2); // "as".len()
            parts.push(
                self.build_inline_comments_between_doc_trailing_space(as_end, exported.span.start),
            );
            parts.push(d.symbol(exported.name.to_u32()));
        }
        parts.push(self.build_from_source_doc(decl.span.start, &decl.source, None));
        // Comments between the source literal and `;` — preserved in place.
        if self.append_pre_semi_comments(&mut parts, decl.source.span.end, decl.span.end) {
            parts.push(d.hardline());
        }
        parts.push(d.text(";"));
        d.concat(&parts)
    }

    /// Check if an import declaration has empty named braces `{}` in source.
    /// This distinguishes `import {} from 'x'` from `import 'x'`.
    /// Also matches braces containing only whitespace and/or comments:
    /// `import { /* c */ } from 'x'`, `import { // c\n } from 'x'`.
    fn has_empty_named_braces(&self, decl: &internal::ImportDeclaration) -> bool {
        let text = decl.span.extract(self.source);
        let decl_start = decl.span.start;
        // Find "from" outside of comments — naive text.find("from") matches inside
        // comments like `import // {} from\n'a'`, falsely detecting empty braces.
        let mut search_offset = 0;
        let from_pos = loop {
            match text[search_offset..].find("from") {
                Some(offset) => {
                    let abs_pos = decl_start + (search_offset + offset) as u32;
                    if self.is_pos_inside_comment(abs_pos) {
                        search_offset += offset + 4; // skip past this "from"
                    } else {
                        break Some(search_offset + offset);
                    }
                }
                None => break None,
            }
        };
        if let Some(from_pos) = from_pos {
            let before_from = &text[..from_pos];
            // Check for empty braces (with any amount of whitespace/comments inside)
            if let Some(brace_start) = before_from.rfind('{') {
                let abs_brace = decl_start + brace_start as u32;
                if self.is_pos_inside_comment(abs_brace) {
                    return false;
                }
                if let Some(brace_end) = before_from[brace_start..].find('}') {
                    let inside = &before_from[brace_start + 1..brace_start + brace_end];
                    return is_only_whitespace_and_comments(inside);
                }
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
        let mut named_specs = Vec::new();
        let mut default_sym = 0u32;
        let mut default_spec_end = 0u32;
        let mut namespace_spec: Option<&internal::ImportNamespaceSpecifier> = None;

        for spec in &decl.specifiers {
            match spec {
                internal::ImportSpecifier::Default(default_spec) => {
                    has_default = true;
                    default_sym = default_spec.local.name.to_u32();
                    default_spec_end = default_spec.span.end;
                }
                internal::ImportSpecifier::Named(named_spec) => {
                    named_specs.push(named_spec);
                }
                internal::ImportSpecifier::Namespace(ns_spec) => {
                    namespace_spec = Some(ns_spec);
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
        if let Some(ns_spec) = namespace_spec {
            if has_default {
                // Comments between default specifier and comma → emit before comma
                // Comments between comma and `*` → emit after comma
                let comma_pos = find_char_skipping_comments(
                    self.source.as_bytes(),
                    default_spec_end as usize,
                    ns_spec.span.start as usize,
                    b',',
                )
                .unwrap_or(default_spec_end as usize);
                parts.push(
                    self.build_inline_comments_between_doc(default_spec_end, comma_pos as u32),
                );
                parts.push(d.text(", "));
                parts.push(self.build_inline_comments_between_doc_trailing_space(
                    comma_pos as u32 + 1,
                    ns_spec.span.start,
                ));
            }
            parts.push(d.text("* as "));
            // Comments between `as` and namespace name
            let as_end = self
                .find_keyword_in_range(ns_spec.span.start, ns_spec.local.span.start, "as")
                .map_or(ns_spec.local.span.start, |p| p + 2); // "as".len()
            parts.push(self.build_inline_comments_between_doc_trailing_space(
                as_end,
                ns_spec.local.span.start,
            ));
            parts.push(d.symbol(ns_spec.local.name.to_u32()));
        }

        // Build named specifiers with group wrapping (or empty braces if source had them)
        if !named_specs.is_empty() || has_empty_braces {
            if has_default || namespace_spec.is_some() {
                // For named imports after default: prettier moves all comments between
                // default end and `{` to before the comma: `import x /* c */, {a}`
                let prev_end = namespace_spec.map_or(default_spec_end, |ns| ns.span.end);
                let brace_or_source = if named_specs.is_empty() {
                    // Empty braces: find `{` before source
                    self.source[prev_end as usize..decl.source.span.start as usize]
                        .find('{')
                        .map_or(decl.source.span.start, |p| prev_end + p as u32)
                } else {
                    self.find_char_outside_comments(prev_end, named_specs[0].span.start, b'{')
                        .unwrap_or(named_specs[0].span.start)
                };
                parts.push(self.build_inline_comments_between_doc(prev_end, brace_or_source));
                parts.push(d.text(", "));
            }

            if named_specs.is_empty() {
                // Empty braces case: `import {} from 'x'`
                // Preserve comments between keyword and `{` in their original position.
                // Without this, `import /* c */ {} from 'x'` silently drops the comment.
                // Skip when default/namespace specifier exists — the handler above
                // already collects comments between the specifier and `{`.
                if !has_default && namespace_spec.is_none() {
                    let keyword_end = if is_type_import {
                        self.find_keyword_end("type", decl.span.start, decl.source.span.start)
                            .unwrap_or(decl.span.start + 11) // "import type".len()
                    } else {
                        decl.span.start + 6 // "import".len()
                    };
                    if let Some(brace_pos) =
                        self.find_char_outside_comments(keyword_end, decl.source.span.start, b'{')
                        && let Some(comments_doc) =
                            self.build_rhs_comments_opt(keyword_end, brace_pos)
                    {
                        parts.push(comments_doc);
                    }
                }
                parts.push(d.text("{}"));
            } else {
                // Find the opening and closing brace positions.
                // Use forward search from keyword end to skip `{` inside comments.
                let import_kw_end = if is_type_import {
                    self.find_keyword_end("type", decl.span.start, named_specs[0].span.start)
                        .unwrap_or(decl.span.start + 11)
                } else {
                    decl.span.start + 6 // "import".len()
                };
                let brace_start = self
                    .find_char_outside_comments(import_kw_end, named_specs[0].span.start, b'{')
                    .unwrap_or(0);
                let last_spec_end = named_specs.last().map_or(0, |s| s.span.end);
                let brace_close = self.source
                    [last_spec_end as usize..decl.source.span.start as usize]
                    .find('}')
                    .map_or(decl.source.span.start, |p| last_spec_end + p as u32);

                // Check for expanding comments (force multiline):
                // line comments, or own-line single-line block comments
                let brace_span = tsv_lang::Span::new(brace_start, brace_close + 1);
                let has_expanding_comments =
                    self.has_line_comments_in_delimited_list(&named_specs, |s| s.span, brace_close)
                        || self
                            .has_line_comments_between(brace_start + 1, named_specs[0].span.start)
                        || self.has_own_line_block_comments_in_bracket_list(
                            brace_span,
                            &named_specs,
                            |s| s.span,
                        );

                if has_expanding_comments {
                    // Comment-aware path: manually iterate with hardlines
                    let inner_doc = self.build_hardline_comma_list(
                        &named_specs,
                        brace_start,
                        brace_close,
                        |s| s.span,
                        |s| self.build_import_specifier_doc(s, is_type_import),
                    );
                    parts.push(d.text("{"));
                    parts.push(d.indent(d.concat(&[d.hardline(), inner_doc])));
                    parts.push(d.hardline());
                    parts.push(d.text("}"));
                } else {
                    // No expanding comments: group-based wrapping with comment splitting
                    let spec_doc = self.build_softline_comma_list(
                        &named_specs,
                        brace_start,
                        brace_close,
                        |s| s.span,
                        |s| self.build_import_specifier_doc(s, is_type_import),
                    );

                    parts.push(d.text("{"));
                    parts.push(d.indent_softline(spec_doc));
                    parts.push(d.softline());
                    parts.push(d.text("}"));
                }
            }
        }

        // Add "from" and source, extracting comments between keywords and source literal
        if !decl.specifiers.is_empty() || has_empty_braces {
            let empty_brace_start = if has_empty_braces && named_specs.is_empty() {
                Some(decl.span.start)
            } else {
                None
            };
            parts.push(self.build_from_source_doc(
                decl.span.start,
                &decl.source,
                empty_brace_start,
            ));
        } else {
            // Bare import: extract comments between import keyword and source
            let keyword = if is_type_import {
                "import type"
            } else {
                "import"
            };
            let keyword_end = decl.span.start + keyword.len() as u32;
            if let Some(comments_doc) =
                self.build_rhs_comments_opt(keyword_end, decl.source.span.start)
            {
                parts.push(comments_doc);
            }
            parts.push(self.build_literal_doc(&decl.source));
        }

        // Add import attributes: `with { type: "json" }`
        // PR #17329 (prettier 3.7): Break attributes across lines when long
        if !decl.attributes.is_empty() {
            parts.push(d.text(" with "));

            // Find the opening brace position for comment checking.
            // Use forward search to skip `{` inside comments.
            let first_attr_start = decl.attributes[0].span.start;
            let with_end = self
                .find_keyword_end("with", decl.source.span.end, first_attr_start)
                .unwrap_or(decl.source.span.end);
            let brace_start = self
                .find_char_outside_comments(with_end, first_attr_start, b'{')
                .unwrap_or(0);

            // Check for line comments between/around attributes (force multiline)
            let has_line_comments = self.has_line_comments_in_delimited_list(
                &decl.attributes,
                |a| a.span,
                decl.span.end,
            ) || self
                .has_line_comments_between(brace_start + 1, decl.attributes[0].span.start);

            if has_line_comments {
                let inner_doc = self.build_hardline_comma_list(
                    &decl.attributes,
                    brace_start,
                    decl.span.end,
                    |a| a.span,
                    |a| self.build_import_attribute_doc(a),
                );
                parts.push(d.text("{"));
                parts.push(d.indent(d.concat(&[d.hardline(), inner_doc])));
                parts.push(d.hardline());
                parts.push(d.text("}"));
            } else {
                let last_attr_end = decl.attributes.last().map_or(0, |a| a.span.end);
                let brace_close_pos = self.source[last_attr_end as usize..decl.span.end as usize]
                    .find('}')
                    .map_or(decl.span.end, |p| last_attr_end + p as u32);

                let attr_doc = self.build_softline_comma_list(
                    &decl.attributes,
                    brace_start,
                    brace_close_pos,
                    |a| a.span,
                    |a| self.build_import_attribute_doc(a),
                );

                parts.push(d.text("{"));
                parts.push(d.indent_softline(attr_doc));
                parts.push(d.softline());
                parts.push(d.text("}"));
            }
        }

        // Comments between the last content token (source literal, or attribute
        // `}` if present) and the terminating `;` — preserved where the user
        // placed them. Emitted outside the content group (see export above).
        let content_end = if decl.attributes.is_empty() {
            decl.source.span.end
        } else {
            let last_attr_end = decl.attributes.last().map_or(0, |a| a.span.end);
            self.source[last_attr_end as usize..decl.span.end as usize]
                .find('}')
                .map_or(decl.span.end, |p| last_attr_end + p as u32 + 1)
        };
        let mut trailing = Vec::new();
        let broke = self.append_pre_semi_comments(&mut trailing, content_end, decl.span.end);
        if trailing.is_empty() {
            parts.push(d.text(";"));
            // Wrap entire statement in a group for width-based wrapping
            d.group(d.concat(&parts))
        } else {
            let group = d.group(d.concat(&parts));
            trailing.insert(0, group);
            // A line comment ends its line — the `;` must follow on a new line.
            if broke {
                trailing.push(d.hardline());
            }
            trailing.push(d.text(";"));
            d.concat(&trailing)
        }
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

        // Comments between the module reference and `;` — preserved in place.
        let ref_end = match &decl.module_reference {
            internal::TSModuleReference::ExternalModuleReference(ext_ref) => ext_ref.span.end,
            internal::TSModuleReference::EntityName(entity_name) => entity_name.span().end,
        };
        if self.append_pre_semi_comments(&mut parts, ref_end, decl.span.end) {
            parts.push(d.hardline());
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
        // Compare spans, not symbols: {a} has same span, {a as a} has different spans
        if named_spec.imported.span == named_spec.local.span {
            parts.push(d.symbol(imported_sym));
        } else {
            parts.push(d.symbol(imported_sym));
            // Split comments at the `as` keyword: before-as and after-as
            if let Some(as_pos) = self.find_keyword_in_range(
                named_spec.imported.span.end,
                named_spec.local.span.start,
                "as",
            ) {
                let before_as =
                    self.build_inline_comments_between_doc(named_spec.imported.span.end, as_pos);
                parts.push(before_as);
                parts.push(d.text(" as "));
                let as_end = as_pos + 2; // "as" is 2 chars
                let after_as = self.build_inline_comments_between_doc_trailing_space(
                    as_end,
                    named_spec.local.span.start,
                );
                parts.push(after_as);
            } else {
                parts.push(d.text(" as "));
            }
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
        // Compare spans, not symbols: {a} has same span, {a as a} has different spans
        if spec.local.span == spec.exported.span {
            spec_parts.push(d.symbol(local_sym));
        } else {
            spec_parts.push(d.symbol(local_sym));
            // Split comments at the `as` keyword: before-as and after-as
            if let Some(as_pos) =
                self.find_keyword_in_range(spec.local.span.end, spec.exported.span.start, "as")
            {
                let before_as = self.build_inline_comments_between_doc(spec.local.span.end, as_pos);
                spec_parts.push(before_as);
                spec_parts.push(d.text(" as "));
                let as_end = as_pos + 2;
                let after_as = self.build_inline_comments_between_doc_trailing_space(
                    as_end,
                    spec.exported.span.start,
                );
                spec_parts.push(after_as);
            } else {
                spec_parts.push(d.text(" as "));
            }
            spec_parts.push(d.symbol(exported_sym));
        }
        d.concat(&spec_parts)
    }

    /// Build ` from [comments] ` followed by source literal.
    ///
    /// Handles comments between `from` keyword and source literal, and optionally
    /// captures comments from inside empty braces (relocated after `from` by prettier).
    fn build_from_source_doc(
        &self,
        decl_start: u32,
        source: &internal::Literal,
        empty_brace_search_start: Option<u32>,
    ) -> DocId {
        let d = self.d();
        #[allow(clippy::expect_used)] // "from" must exist in a valid import/export declaration
        let from_end = self
            .find_keyword_end("from", decl_start, source.span.start)
            .expect("'from' keyword must exist in import/export declaration");
        let comment_search_start = if let Some(search_start) = empty_brace_search_start {
            // Include comments from inside empty braces (relocated after "from")
            self.source[search_start as usize..from_end as usize]
                .find('{')
                .map_or(from_end, |p| search_start + p as u32 + 1)
        } else {
            from_end
        };
        let mut parts = vec![d.text(" from")];
        if let Some(comments_doc) =
            self.build_rhs_comments_opt(comment_search_start, source.span.start)
        {
            parts.push(d.text(" "));
            parts.push(comments_doc);
        } else {
            parts.push(d.text(" "));
        }
        parts.push(self.build_literal_doc(source));
        d.concat(&parts)
    }

    /// Build a comma-separated list with group-based wrapping and comment splitting.
    /// Returns the inner doc to be wrapped with `{ indent_softline(...) softline }`.
    fn build_softline_comma_list<T>(
        &self,
        items: &[T],
        brace_start: u32,
        brace_close: u32,
        get_span: impl Fn(&T) -> tsv_lang::Span,
        build_item_doc: impl Fn(&T) -> DocId,
    ) -> DocId {
        let d = self.d();
        let mut inner_parts = Vec::new();
        let mut prev_end = brace_start + 1; // After opening `{`
        // Block comment trailing the LAST item after the comma — preserved after
        // the (synthetic) trailing comma rather than relocated before it (prettier
        // relocates before; see conformance_prettier.md §Comment relocation).
        let mut last_after_comma = Vec::new();

        for (i, item) in items.iter().enumerate() {
            let span = get_span(item);
            let item_start = span.start;
            let item_end = span.end;
            let is_last = i == items.len() - 1;

            let mut item_parts = Vec::new();

            // Leading block comments before this item (after prev comma or `{`)
            for comment in comments_in_range(self.comments, prev_end, item_start) {
                if comment.is_block {
                    item_parts.push(d.text_owned(format!("/*{}*/ ", comment.content)));
                }
            }

            item_parts.push(build_item_doc(item));

            if !is_last {
                let next_start = get_span(&items[i + 1]).start;
                let comma_pos = self.find_list_comma(item_end, next_start);
                self.append_trailing_inline_block_comments(&mut item_parts, item_end, comma_pos);
                prev_end = comma_pos + 1;
            } else {
                // Split the last item's trailing block comments around a source
                // trailing comma: before-comma stay with the item; after-comma are
                // preserved after the comma (emitted below, after `trailing_comma`).
                self.append_last_trailing_block_comments_split(
                    &mut item_parts,
                    &mut last_after_comma,
                    item_end,
                    brace_close,
                );
            }

            if i > 0 {
                inner_parts.push(d.line());
            }
            inner_parts.push(d.concat(&item_parts));
            if !is_last {
                inner_parts.push(d.text(","));
            }
        }

        // Trailing comma when broken (matches join_trailing behavior)
        let trailing_comma = d.if_break(d.text(","), d.text(""));
        inner_parts.push(trailing_comma);
        // Preserved after-comma block comment(s) on the last item
        inner_parts.extend(last_after_comma);

        d.concat(&inner_parts)
    }

    /// Build a comma-separated list with hardline breaks and full comment handling.
    /// Used when expanding comments force multiline formatting.
    fn build_hardline_comma_list<T>(
        &self,
        items: &[T],
        brace_start: u32,
        end_boundary: u32,
        get_span: impl Fn(&T) -> tsv_lang::Span,
        build_item_doc: impl Fn(&T) -> DocId,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        let mut prev_end: u32 = brace_start + 1; // After opening brace

        for (i, item) in items.iter().enumerate() {
            let span = get_span(item);
            let item_start = span.start;
            let is_first = i == 0;
            let is_last = i == items.len() - 1;

            let search_start = self.leading_comment_search_start(prev_end, is_first);
            let comments: Vec<_> = comments_in_range(self.comments, search_start, item_start)
                .filter(|c| is_first || c.is_block || !self.is_same_line(prev_end, c.span.start))
                .collect();

            if !is_first {
                let check_pos = if comments.is_empty() {
                    item_start
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
                if comment.is_block && self.is_same_line(comment.span.end, item_start) {
                    parts.push(d.text(" "));
                } else {
                    parts.push(d.hardline());
                }
            }

            parts.push(build_item_doc(item));

            // Comma with comment-boundary splitting
            let item_end = span.end;
            if !is_last {
                let next_start = get_span(&items[i + 1]).start;
                let comma_pos = self.find_list_comma(item_end, next_start);

                let mut line_ref = item_end;
                for comment in comments_in_range(self.comments, item_end, comma_pos) {
                    if comment.is_block && self.is_same_line(line_ref, comment.span.start) {
                        parts.push(d.text(" "));
                        parts.push(self.build_comment_doc(comment));
                        // Follow multi-line block comments to their closing line
                        if !self.is_same_line(comment.span.start, comment.span.end) {
                            line_ref = comment.span.end;
                        }
                    }
                }

                parts.push(d.text(","));

                parts.extend(self.build_trailing_same_line_comment_docs(comma_pos + 1, next_start));
            } else {
                // Last item: emit comma between same-line block and line comments,
                // then own-line comments before the closing brace.
                // Block comments go before comma: `a /* c */ ,`
                // Line comments go after comma: `a, // comment`
                // Own-line comments get hardlines: `a,\n// comment`
                let mut emitted_comma = false;
                let mut prev_pos = item_end;
                // Track line reference for multi-line block comments
                let mut line_ref = item_end;
                for comment in comments_in_range(self.comments, item_end, end_boundary) {
                    if self.is_same_line(line_ref, comment.span.start) {
                        if comment.is_block {
                            parts.push(d.text(" "));
                            parts.push(self.build_comment_doc(comment));
                            // Follow multi-line block comments to their closing line
                            if !self.is_same_line(comment.span.start, comment.span.end) {
                                line_ref = comment.span.end;
                            }
                        } else {
                            if !emitted_comma {
                                parts.push(d.text(","));
                                emitted_comma = true;
                            }
                            parts.push(self.build_trailing_line_comment_doc(comment));
                        }
                    } else {
                        if !emitted_comma {
                            parts.push(d.text(","));
                            emitted_comma = true;
                        }
                        if self.has_blank_line_between(prev_pos, comment.span.start) {
                            parts.push(d.literalline());
                        }
                        parts.push(d.hardline());
                        parts.push(self.build_comment_doc(comment));
                    }
                    prev_pos = comment.span.end;
                }
                if !emitted_comma {
                    parts.push(d.text(","));
                }
            }

            prev_end = item_end;
        }

        d.concat(&parts)
    }

    /// Build doc for a single import attribute: `key: value`
    ///
    /// Handles comments between key, `:`, and value.
    fn build_import_attribute_doc(&self, attr: &internal::ImportAttribute) -> DocId {
        let d = self.d();
        let key_end = attr.key.span.end;
        let value_start = attr.value.span.start;

        // Check for comments between key and value (around the `:`)
        let has_comments = comments_in_range(self.comments, key_end, value_start)
            .next()
            .is_some();

        if !has_comments {
            // Fast path: no comments
            return d.concat(&[
                d.symbol(attr.key.name.to_u32()),
                d.text(": "),
                self.build_literal_doc(&attr.value),
            ]);
        }

        // Find `:` position to split comments
        #[allow(clippy::expect_used)] // colon must exist when key and value are present
        let colon_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            key_end as usize,
            value_start as usize,
            b':',
        )
        .expect("colon must exist in import attribute") as u32;

        let mut parts = vec![d.symbol(attr.key.name.to_u32())];

        // Comments between key and `:`
        self.append_trailing_inline_block_comments(&mut parts, key_end, colon_pos);

        parts.push(d.text(":"));

        // Comments between `:` and value
        let after_colon = colon_pos + 1;
        parts.push(d.text(" "));
        for comment in comments_in_range(self.comments, after_colon, value_start) {
            if comment.is_block {
                parts.push(self.build_comment_doc(comment));
                parts.push(d.text(" "));
            }
        }

        parts.push(self.build_literal_doc(&attr.value));

        d.concat(&parts)
    }
}
