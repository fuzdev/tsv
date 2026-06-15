// Module statement printing for TypeScript (import and export)

use super::{Printer, build_entity_name_doc};
use crate::ast::internal;
use crate::printer::CommentSpacing;
use tsv_lang::SymbolToU32;
use tsv_lang::comments_in_range;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

/// Byte length of the leading `import`/`export` keyword (both 6 chars). Added to
/// a declaration's span start to reach the position just past the keyword.
const MODULE_KW_LEN: u32 = 6;
/// Byte length of `import type` / `export type` (both 11 chars) — fallback when
/// the `type` keyword can't be located by scanning (e.g. malformed source).
const MODULE_TYPE_KW_LEN: u32 = 11;

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
            // Check if the overall export is type-only
            let is_type_export = decl.export_kind == internal::ExportKind::Type;

            let mut parts = if is_type_export {
                // Split the keyword so a type-only re-export (empty `{}` or named
                // specifiers) can keep a comment between `export` and `type` in place
                // — prettier relocates it (after `from` for empty, into the braces for
                // named specifiers). The `type`→`{` gap is handled beside each brace
                // path below. Mirrors the import side.
                let mut p = vec![d.text("export ")];
                if let Some(comments_doc) = self.build_pre_type_keyword_comment(
                    decl.span.start + MODULE_KW_LEN,
                    decl.source.as_ref().map_or(decl.span.end, |s| s.span.start),
                ) {
                    p.push(comments_doc);
                }
                p.push(d.text("type "));
                p
            } else {
                vec![d.text("export ")]
            };

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
                let semi_or_source = decl.source.as_ref().map_or(decl.span.end, |s| s.span.start);
                let keyword_end = self.export_header_end(decl, semi_or_source);
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
                // Named specifiers: comment-aware braced list. `close_brace_end`
                // is the offset past `}`, for the trailing pre-`;` comment scan.
                let kw_end = self.export_header_end(decl, decl.specifiers[0].span.start);
                let bound = decl.source.as_ref().map_or(decl.span.end, |s| s.span.start);
                close_brace_end = self.push_braced_specifier_list(
                    &mut parts,
                    &decl.specifiers,
                    kw_end,
                    bound,
                    is_type_export,
                    |s| s.span,
                    |s| self.build_export_specifier_doc(s, is_type_export),
                );
            }

            // Comments between the last content token (attribute `}`, source
            // literal, or closing `}`) and the terminating `;` — preserved where
            // the user placed them (prettier relocates no-`from` ones inside the
            // braces). Emitted outside the content group so a line-comment break
            // doesn't expand the braces.
            let content_end = if let Some(source) = &decl.source {
                let empty_brace_start = if decl.specifiers.is_empty() {
                    Some(self.export_header_end(decl, source.span.start))
                } else {
                    None
                };
                // Named specifiers: preserve a comment in the `}`→`from` gap in place
                // (prettier relocates it into the braces). Empty braces relocate after
                // `from` instead, so skip the in-place scan there.
                let from_content_end = if decl.specifiers.is_empty() {
                    None
                } else {
                    Some(close_brace_end)
                };
                parts.push(self.build_from_source_doc(
                    decl.span.start,
                    source,
                    empty_brace_start,
                    from_content_end,
                ));
                // Import attributes: `export { x } from "y" with { type: "json" }`
                self.push_import_attributes_clause(
                    &mut parts,
                    decl.attributes.as_deref(),
                    source.span.end,
                    decl.span.end,
                )
            } else {
                close_brace_end
            };
            self.finish_with_pre_semi(parts, content_end, decl.span.end, true)
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
        let is_type = decl.export_kind == internal::ExportKind::Type;
        let export_end = decl.span.start + MODULE_KW_LEN;
        // The `*` token, after the keyword(s) and before any `as`/`from`.
        let star_limit = decl
            .exported
            .as_ref()
            .map_or(decl.source.span.start, |e| e.span.start);
        let star_pos = self
            .find_char_outside_comments(export_end, star_limit, b'*')
            .unwrap_or(export_end);

        // Header comments (around `export`, `type`, `*`) are preserved where the user
        // placed them; prettier relocates every one to after `from`.
        let mut parts = vec![d.text("export ")];
        if is_type {
            // `export`→`type` gap
            if let Some(c) = self.build_pre_type_keyword_comment(export_end, star_pos) {
                parts.push(c);
            }
            parts.push(d.text("type "));
            // `type`→`*` gap
            let type_end = self
                .find_keyword_end("type", export_end, star_pos)
                .unwrap_or(export_end);
            if let Some(c) = self.build_rhs_comments_opt(type_end, star_pos) {
                parts.push(c);
            }
        } else if let Some(c) = self.build_rhs_comments_opt(export_end, star_pos) {
            // `export`→`*` gap
            parts.push(c);
        }
        parts.push(d.text("*"));
        let star_end = star_pos + 1; // position just past `*`

        if let Some(exported) = &decl.exported {
            self.append_namespace_as_binding(&mut parts, star_end, exported);
        }

        // Comment between `*` (or `as ns`) and `from`, preserved in place — a same-line
        // block comment trails inline (`* /* c */ from`); prettier relocates it after
        // `from`. The leading space here pairs with `from`'s leading space below.
        let prev_end = decl.exported.as_ref().map_or(star_end, |e| e.span.end);
        let from_start = self
            .find_keyword_in_range(prev_end, decl.source.span.start, "from")
            .unwrap_or(decl.source.span.start);
        for comment in comments_in_range(self.comments, prev_end, from_start) {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
            if !comment.is_block {
                parts.push(d.hardline());
            }
        }

        parts.push(self.build_from_source_doc(decl.span.start, &decl.source, None, None));
        // Import attributes: `export * from "y" with { type: "json" }`.
        // Returns the offset past the attribute `}` (or source) for the trailing
        // pre-`;` comment scan, preserved in place.
        let content_end = self.push_import_attributes_clause(
            &mut parts,
            decl.attributes.as_deref(),
            decl.source.span.end,
            decl.span.end,
        );
        self.finish_with_pre_semi(parts, content_end, decl.span.end, false)
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
            // Check for empty braces (with any amount of whitespace/comments inside).
            // Find the opening `{` outside comments — a naive rfind('{') matches a `{`
            // glyph inside an enclosed comment (`{/* { */}`), landing on the wrong brace
            // and misclassifying the named braces (which silently drops `{}` + `from`).
            if let Some(brace_start) =
                find_char_skipping_comments(before_from.as_bytes(), 0, before_from.len(), b'{')
            {
                // Likewise find the closing `}` outside comments — a naive find('}')
                // matches a `}` glyph inside the enclosed comment (`{/* } */}`),
                // truncating `inside` mid-comment and misclassifying as non-empty.
                if let Some(brace_end) = find_char_skipping_comments(
                    before_from.as_bytes(),
                    brace_start,
                    before_from.len(),
                    b'}',
                ) {
                    let inside = &before_from[brace_start + 1..brace_end];
                    return is_only_whitespace_and_comments(inside);
                }
            }
            false
        } else {
            false
        }
    }

    /// Build the doc for a comment between the `import`/`export` keyword and the
    /// `type` keyword of an empty type-only import/re-export, preserved in place
    /// (prettier relocates it after `from`). `keyword_end` is the position just
    /// past `import`/`export`; `search_end` bounds the scan for the `type` keyword
    /// (the source literal start, or the statement end when there's no `from`).
    /// Returns `None` when no such comment exists. A block comment trails inline
    /// (` /* c */ `); a line comment ends with a hardline (forcing `type` to the
    /// next line) — both via `build_rhs_comments_opt`.
    fn build_pre_type_keyword_comment(&self, keyword_end: u32, search_end: u32) -> Option<DocId> {
        let type_start = self.find_keyword_in_range(keyword_end, search_end, "type")?;
        self.build_rhs_comments_opt(keyword_end, type_start)
    }

    /// Position just past the leading keyword(s) of an import declaration: after the
    /// `type` keyword for a type-only import (located by scanning, so a comment in
    /// the `import`→`type` gap doesn't throw off the offset), else after `import`.
    /// `search_end` bounds the `type` scan — the source literal start, or the first
    /// specifier start when a tighter bound is needed to avoid matching `type`
    /// inside the specifier list.
    fn import_header_end(&self, decl: &internal::ImportDeclaration, search_end: u32) -> u32 {
        let is_type = decl.import_kind == internal::ImportKind::Type;
        self.module_header_end(is_type, decl.span.start, search_end)
    }

    /// Position just past the leading keyword(s) of an export named declaration:
    /// after the `type` keyword for a type-only re-export (located by scanning, so a
    /// comment in the `export`→`type` gap doesn't throw off the offset), else after
    /// `export`. `search_end` bounds the `type` scan — the source/`;`, or the first
    /// specifier start to avoid matching `type` inside the specifier list.
    fn export_header_end(&self, decl: &internal::ExportNamedDeclaration, search_end: u32) -> u32 {
        let is_type = decl.export_kind == internal::ExportKind::Type;
        self.module_header_end(is_type, decl.span.start, search_end)
    }

    /// Position just past a module declaration's leading keyword(s): after the
    /// `type` keyword for a type-only import/re-export (located by scanning, so a
    /// comment in the `import`/`export`→`type` gap doesn't throw off the offset),
    /// else after the 6-char `import`/`export`. `search_end` bounds the `type`
    /// scan. Shared by [`Self::import_header_end`] and [`Self::export_header_end`].
    fn module_header_end(&self, is_type: bool, span_start: u32, search_end: u32) -> u32 {
        if is_type {
            self.find_keyword_end("type", span_start, search_end)
                .unwrap_or(span_start + MODULE_TYPE_KW_LEN)
        } else {
            span_start + MODULE_KW_LEN
        }
    }

    /// Wrap a specifier list in its own group so it fits independently of the outer
    /// statement: `{ <inner> }` with softline padding. The independent group keeps a
    /// preserved header line comment (which forces the outer group to break) from
    /// expanding a `{a}` that would otherwise stay inline.
    fn braced_softline_group(&self, inner: DocId) -> DocId {
        let d = self.d();
        d.group(d.concat(&[
            d.text("{"),
            d.indent_softline(inner),
            d.softline(),
            d.text("}"),
        ]))
    }

    /// Finish a module statement: emit any comments between the last content token
    /// and the terminating `;`, then the `;` itself.
    ///
    /// When `grouped`, `parts` is wrapped in a `group` for width-based wrapping and
    /// the pre-`;` comments are emitted *outside* it, so a line-comment break can't
    /// expand the statement's specifier braces (import/export named declarations).
    /// Otherwise the comments are appended inline to `parts` — used by export-all
    /// and import-equals, which have no wrapping group.
    fn finish_with_pre_semi(
        &self,
        mut parts: Vec<DocId>,
        content_end: u32,
        decl_end: u32,
        grouped: bool,
    ) -> DocId {
        let d = self.d();
        if !grouped {
            if self.append_pre_semi_comments(&mut parts, content_end, decl_end) {
                parts.push(d.hardline());
            }
            parts.push(d.text(";"));
            return d.concat(&parts);
        }
        let mut trailing = Vec::new();
        let broke = self.append_pre_semi_comments(&mut trailing, content_end, decl_end);
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

    /// Emit the ` as <binding>` tail of a namespace binding (`* as ns`), starting
    /// just past the `*`. Preserves a comment in the `*`→`as` gap in place (prettier
    /// relocates it after `as`, before the binding) and comments in the `as`→binding
    /// gap. `star_end` is the position just past `*`; `binding` is the namespace
    /// identifier (`exported` for a re-export, `local` for an import). A block comment
    /// trails inline; a line comment forces `as` onto the next line.
    fn append_namespace_as_binding(
        &self,
        parts: &mut Vec<DocId>,
        star_end: u32,
        binding: &internal::Identifier,
    ) {
        let d = self.d();
        let as_pos = self.find_keyword_in_range(star_end, binding.span.start, "as");
        if let Some(p) = as_pos
            && let Some(c) = self.build_rhs_comments_opt(star_end, p)
        {
            parts.push(d.text(" "));
            parts.push(c);
            parts.push(d.text("as "));
        } else {
            parts.push(d.text(" as "));
        }
        // Comments between `as` and the binding name.
        let as_end = as_pos.map_or(binding.span.start, |p| p + 2); // "as".len()
        parts.push(
            self.build_inline_comments_between_doc_trailing_space(as_end, binding.span.start),
        );
        parts.push(d.symbol(binding.name.to_u32()));
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
        let mut default_spec_start = 0u32;
        let mut default_spec_end = 0u32;
        let mut namespace_spec: Option<&internal::ImportNamespaceSpecifier> = None;

        for spec in &decl.specifiers {
            match spec {
                internal::ImportSpecifier::Default(default_spec) => {
                    has_default = true;
                    default_sym = default_spec.local.name.to_u32();
                    default_spec_start = default_spec.span.start;
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
            // Type-only import: preserve a comment between `import` and the `type`
            // keyword in place — prettier relocates it (after `from` for empty, into
            // the braces for named specifiers, to the binding side of `type` for
            // default/namespace). The `type`→binding gap is handled beside each form.
            if let Some(comments_doc) = self.build_pre_type_keyword_comment(
                decl.span.start + MODULE_KW_LEN,
                decl.source.span.start,
            ) {
                parts.push(comments_doc);
            }
            parts.push(d.text("type "));
        }

        // Position just past the leading keyword(s), used to bound the scan for
        // comments preserved before a default binding or namespace `*`.
        let header_end = self.import_header_end(decl, decl.source.span.start);

        // End of the last binding/specifier, used to scan for a comment in the
        // gap before `from` (preserved in place). Set in each binding branch below;
        // left None for bare imports and empty braces (which relocate after `from`).
        let mut from_content_end: Option<u32> = None;

        // Add default import
        if has_default {
            // Comment between the keyword(s) and the default binding, preserved in
            // place (prettier keeps it adjacent to the binding — dual-stable).
            if let Some(comments_doc) = self.build_rhs_comments_opt(header_end, default_spec_start)
            {
                parts.push(comments_doc);
            }
            parts.push(d.symbol(default_sym));
            from_content_end = Some(default_spec_end);
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
            } else if let Some(comments_doc) =
                self.build_rhs_comments_opt(header_end, ns_spec.span.start)
            {
                // Comment between the keyword(s) and the namespace `*`, preserved in
                // place (prettier keeps it adjacent to `*` — dual-stable).
                parts.push(comments_doc);
            }
            parts.push(d.text("*"));
            // `* as ns` binding; preserves the `*`→`as` comment in place (mirrors the
            // export-all side).
            let star_end = ns_spec.span.start + 1;
            self.append_namespace_as_binding(&mut parts, star_end, &ns_spec.local);
            from_content_end = Some(ns_spec.span.end);
        }

        // Build named specifiers with group wrapping (or empty braces if source had them)
        if !named_specs.is_empty() || has_empty_braces {
            if has_default || namespace_spec.is_some() {
                // For named imports after default: prettier moves all comments between
                // default end and `{` to before the comma: `import x /* c */, {a}`
                let prev_end = namespace_spec.map_or(default_spec_end, |ns| ns.span.end);
                let brace_or_source = if named_specs.is_empty() {
                    // Empty braces: find `{` before source.
                    // TODO: this naive find('{') matches a `{` inside a comment, and the
                    // surrounding default+empty-braces comment path has a separate
                    // comment-duplication bug — both deferred (harden together).
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
                if !has_default
                    && namespace_spec.is_none()
                    && let Some(brace_pos) =
                        self.find_char_outside_comments(header_end, decl.source.span.start, b'{')
                    && let Some(comments_doc) = self.build_rhs_comments_opt(header_end, brace_pos)
                {
                    parts.push(comments_doc);
                }
                parts.push(d.text("{}"));
            } else {
                // Named specifiers: comment-aware braced list. `from_content_end`
                // is the offset past `}`, for the `}`→`from` gap comment scan.
                let kw_end = self.import_header_end(decl, named_specs[0].span.start);
                from_content_end = Some(self.push_braced_specifier_list(
                    &mut parts,
                    &named_specs,
                    kw_end,
                    decl.source.span.start,
                    is_type_import,
                    |s| s.span,
                    |s| self.build_import_specifier_doc(s, is_type_import),
                ));
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
                from_content_end,
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

        // Add import attributes: `with { type: "json" }`. Returns the offset
        // past the attribute `}` (or source literal) — the anchor for the
        // trailing pre-`;` comment scan, preserved where the user placed it.
        // Emitted outside the content group (see export above).
        let content_end = self.push_import_attributes_clause(
            &mut parts,
            decl.attributes.as_deref(),
            decl.source.span.end,
            decl.span.end,
        );
        self.finish_with_pre_semi(parts, content_end, decl.span.end, true)
    }

    /// Append the `with { … }` import-attributes clause to `parts`, if any, and
    /// return the byte offset just past the clause's closing `}` — the anchor for
    /// the caller's trailing pre-`;` comment scan (`source_end` when there is no
    /// `with` clause).
    ///
    /// Shared by the import declaration and the two re-export hosts
    /// (`export { … } from …` and `export * from …`). `attributes` is `None`
    /// when there is no `with` clause (nothing emitted) and `Some(_)` when one
    /// is present — `Some([])` emits an empty `with {}` (preserved to match
    /// acorn/prettier). `source_end` is the byte offset just past the source
    /// string literal — where the `with` keyword search begins; `stmt_end` is
    /// the declaration's span end (the `;`). PR #17329 (prettier 3.7): break
    /// attributes across lines when long.
    fn push_import_attributes_clause(
        &self,
        parts: &mut Vec<DocId>,
        attributes: Option<&[internal::ImportAttribute]>,
        source_end: u32,
        stmt_end: u32,
    ) -> u32 {
        let Some(attributes) = attributes else {
            return source_end;
        };
        let d = self.d();
        // Find the `with` keyword and the opening brace (forward search skips
        // `{`/keyword text inside comments). The search upper bound is the first
        // attribute, or the statement end for an empty `with {}`.
        let header_bound = attributes.first().map_or(stmt_end, |a| a.span.start);
        let with_end = self
            .find_keyword_end("with", source_end, header_bound)
            .unwrap_or(source_end);
        let with_start = with_end - "with".len() as u32;
        let brace_start = self
            .find_char_outside_comments(with_end, header_bound, b'{')
            .unwrap_or(0);
        // The closing `}`: the first `}` (outside comments) at or after the last
        // attribute — or after the opening `{` for an empty `with {}`. Returned
        // (plus one) as the content end for the trailing comment scan.
        let close_search_start = attributes.last().map_or(brace_start, |a| a.span.end);
        let brace_close = self.close_brace_offset(close_search_start, stmt_end);

        // Comments in the attributes header — source→`with` and `with`→`{` —
        // preserved in place. Prettier keeps a source→`with` block in place but
        // floats a line past `;`, and relocates a `with`→`{` block to before
        // `with`. A block trails inline; a line forces the next token onto a new
        // line. See conformance_prettier.md §Comment relocation.
        self.push_gap_comment_keyword(parts, source_end, with_start, "with");
        // `with`→`{` gap; the brace group below emits the `{` itself.
        self.push_gap_comment(parts, with_end, brace_start);

        // Empty `with {}` — preserved (acorn/prettier keep it). A comment between
        // the braces is kept in place (`with {/* c */}`); prettier instead
        // relocates it before `with` — a comment-position divergence, like the
        // `with`→`{` gap. See attributes_empty_comment_prettier_divergence.
        if attributes.is_empty() {
            let mut inner = vec![d.text("{")];
            let mut last_was_line = false;
            let mut any_comment = false;
            for comment in comments_in_range(self.comments, brace_start + 1, brace_close) {
                if any_comment {
                    inner.push(d.text(" "));
                }
                inner.push(self.build_comment_doc(comment));
                last_was_line = !comment.is_block;
                any_comment = true;
            }
            // A trailing line comment would swallow the `}`; push it to a new line.
            if last_was_line {
                inner.push(d.hardline());
            }
            inner.push(d.text("}"));
            parts.push(d.concat(&inner));
            return brace_close + 1;
        }

        // Check for line comments between/around attributes (force multiline)
        let has_line_comments =
            self.has_line_comments_in_delimited_list(attributes, |a| a.span, stmt_end)
                || self.has_line_comments_between(brace_start + 1, attributes[0].span.start);

        if has_line_comments {
            // `None`: the import-attribute `with {…}` brace keeps relocating
            // a same-line comment (separate, rarer delimiter — scoped out).
            parts.push(self.build_braced_hardline_comma_list(
                attributes,
                brace_start,
                stmt_end,
                None,
                |a| a.span,
                |a| self.build_import_attribute_doc(a),
            ));
        } else {
            let attr_doc = self.build_softline_comma_list(
                attributes,
                brace_start,
                brace_close,
                |a| a.span,
                |a| self.build_import_attribute_doc(a),
            );

            // Own group so the braces fit independently of the outer statement —
            // a preserved header line comment (source→`with` / `with`→`{`) forces
            // the outer group to break but must not expand inline attributes.
            parts.push(self.braced_softline_group(attr_doc));
        }
        brace_close + 1
    }

    /// The source offset of a closing `}` — the first `}` (outside comments, so a
    /// `}` inside a trailing comment is skipped) at or after `search_start`,
    /// bounded by `bound` (the fallback when the brace can't be located). Shared
    /// by the named-specifier brace scans (import/export) and the attribute clause.
    fn close_brace_offset(&self, search_start: u32, bound: u32) -> u32 {
        self.find_char_outside_comments(search_start, bound, b'}')
            .unwrap_or(bound)
    }

    /// Render a braced, comma-separated specifier list (`{a, b as c}`) with
    /// comment-aware wrapping, push the doc onto `parts`, and return the offset
    /// just past the closing `}` — the caller's trailing-comment anchor.
    ///
    /// Shared by import and export named specifiers (which differed only in the
    /// item type and per-item doc builder). `kw_end` is the offset past the
    /// `import`/`export [type]` header, where the `{` search begins; `bound`
    /// caps the brace scans (the source-literal start, or the `;` for a local
    /// `export {…}`). When `is_type`, a comment in the `type`→`{` gap is preserved
    /// in place (prettier relocates it into the braces as the first specifier's
    /// leading comment).
    // Two closures (span + per-item doc) plus positional context — inherent to a
    // generic list builder; sibling `build_braced_hardline_comma_list` is at 7.
    #[allow(clippy::too_many_arguments)]
    fn push_braced_specifier_list<T>(
        &self,
        parts: &mut Vec<DocId>,
        specifiers: &[T],
        kw_end: u32,
        bound: u32,
        is_type: bool,
        get_span: impl Fn(&T) -> tsv_lang::Span,
        build_item: impl Fn(&T) -> DocId,
    ) -> u32 {
        debug_assert!(
            !specifiers.is_empty(),
            "push_braced_specifier_list requires ≥1 specifier; empty `{{}}` is handled separately"
        );
        // Forward search from the header skips a `{` inside comments.
        let first_start = get_span(&specifiers[0]).start;
        let brace_start = self
            .find_char_outside_comments(kw_end, first_start, b'{')
            .unwrap_or(0);

        if is_type && let Some(comments_doc) = self.build_rhs_comments_opt(kw_end, brace_start) {
            parts.push(comments_doc);
        }

        let last_spec_end = specifiers.last().map_or(0, |s| get_span(s).end);
        let brace_close = self.close_brace_offset(last_spec_end, bound);

        // Expanding comments (line comments, or own-line single-line block
        // comments) force the multiline path.
        let brace_span = tsv_lang::Span::new(brace_start, brace_close + 1);
        let has_expanding_comments =
            self.has_line_comments_in_delimited_list(specifiers, &get_span, brace_close)
                || self.has_line_comments_between(brace_start + 1, first_start)
                || self
                    .has_own_line_block_comments_in_bracket_list(brace_span, specifiers, &get_span);

        if has_expanding_comments {
            // `Some(first_start)` keeps a same-line `{` comment on the brace line
            // (divergence from prettier, which relocates it as the first
            // specifier's leading comment).
            parts.push(self.build_braced_hardline_comma_list(
                specifiers,
                brace_start,
                brace_close,
                Some(first_start),
                &get_span,
                &build_item,
            ));
        } else {
            // No expanding comments: group-based wrapping with comment splitting.
            let spec_doc = self.build_softline_comma_list(
                specifiers,
                brace_start,
                brace_close,
                &get_span,
                &build_item,
            );
            parts.push(self.braced_softline_group(spec_doc));
        }
        brace_close + 1
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
        self.finish_with_pre_semi(parts, ref_end, decl.span.end, false)
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

    /// Emit a leading space and any comments in `[start, end)` into `parts`, preserving
    /// them in place between the preceding token and whatever the caller emits next. A
    /// block comment trails inline (` /* c */`); a line comment forces a break —
    /// `build_rhs_comments_opt` supplies the trailing space (block) or hardline (line),
    /// so the following token carries no separator of its own. An empty range emits ` `.
    fn push_gap_comment(&self, parts: &mut Vec<DocId>, start: u32, end: u32) {
        let d = self.d();
        parts.push(d.text(" "));
        if let Some(c) = self.build_rhs_comments_opt(start, end) {
            parts.push(c);
        }
    }

    /// `push_gap_comment` followed by a keyword (`from`, `with`) — the keyword carries no
    /// leading space of its own. Used where prettier relocates the gap comment but tsv
    /// preserves it (binding/specifiers→`from`, source→`with`). An empty range emits
    /// just ` kw`. See conformance_prettier.md §Comment relocation.
    fn push_gap_comment_keyword(
        &self,
        parts: &mut Vec<DocId>,
        start: u32,
        end: u32,
        keyword: &'static str,
    ) {
        self.push_gap_comment(parts, start, end);
        parts.push(self.d().text(keyword));
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
        content_end: Option<u32>,
    ) -> DocId {
        let d = self.d();
        #[allow(clippy::expect_used)] // "from" must exist in a valid import/export declaration
        let from_end = self
            .find_keyword_end("from", decl_start, source.span.start)
            .expect("'from' keyword must exist in import/export declaration");
        let from_start = from_end - "from".len() as u32;

        // Comments between the binding/specifiers and `from`, preserved in place.
        // `content_end` is the end of the last binding/specifier (None to skip — e.g.
        // empty braces or export-all, which handle their own header comments — emitted
        // as an empty range so only ` from` is pushed).
        let mut parts = Vec::new();
        self.push_gap_comment_keyword(
            &mut parts,
            content_end.unwrap_or(from_start),
            from_start,
            "from",
        );

        let comment_search_start = if let Some(search_start) = empty_brace_search_start {
            // Include comments from inside empty braces (relocated after "from").
            // Locate `{` outside comments so a `{` glyph in a comment isn't mistaken for it.
            self.find_char_outside_comments(search_start, from_end, b'{')
                .map_or(from_end, |p| p + 1)
        } else {
            from_end
        };
        // Comments between `from` and the source literal (incl. those relocated out of
        // empty braces), preserved in place.
        self.push_gap_comment(&mut parts, comment_search_start, source.span.start);
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

    /// Emit a multiline `{ … }` brace group for a specifier/attribute list that
    /// comments have forced multiline: opening brace, optional brace-line comment
    /// prefix, the indented hardline comma-list, and the closing brace.
    ///
    /// `brace_comment_first` is `Some(first_item_start)` to keep a same-line `{`
    /// comment on the brace line (the open-brace divergence — import/export
    /// specifiers), or `None` to let it relocate to its own line (the `with {…}`
    /// import-attribute brace). See conformance_prettier.md §Comment relocation.
    fn build_braced_hardline_comma_list<T>(
        &self,
        items: &[T],
        brace_start: u32,
        end_boundary: u32,
        brace_comment_first: Option<u32>,
        get_span: impl Fn(&T) -> tsv_lang::Span,
        build_item_doc: impl Fn(&T) -> DocId,
    ) -> DocId {
        let d = self.d();
        let (brace_line_prefix, delimiter_pull_pos) = match brace_comment_first {
            Some(first) => self.delimiter_line_comment_prefix(brace_start, first),
            None => (Vec::new(), None),
        };
        let inner_doc = self.build_hardline_comma_list(
            items,
            brace_start,
            end_boundary,
            delimiter_pull_pos,
            get_span,
            build_item_doc,
        );
        d.concat(&[
            d.text("{"),
            d.concat(&brace_line_prefix),
            d.indent(d.concat(&[d.hardline(), inner_doc])),
            d.hardline(),
            d.text("}"),
        ])
    }

    /// Build a comma-separated list with hardline breaks and full comment handling.
    /// Used when expanding comments force multiline formatting.
    fn build_hardline_comma_list<T>(
        &self,
        items: &[T],
        brace_start: u32,
        end_boundary: u32,
        delimiter_pull_pos: Option<u32>,
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
            // First item: drop comments pulled onto the `{` line (emitted as the
            // brace-line prefix by the caller). No-op when `delimiter_pull_pos`
            // is `None` (the import-attribute `with {…}` caller).
            let comments = if is_first {
                self.first_member_leading_comments(comments, delimiter_pull_pos)
            } else {
                comments
            };

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

    /// Build doc for an import attribute key: a bare identifier emits verbatim;
    /// a string-literal key follows prettier's `quoteProps: as-needed` — quotes
    /// are stripped when the value is a valid identifier with no escapes
    /// (`'type'` → `type`), else kept and normalized (`'resolution-mode'`).
    fn build_import_attribute_key_doc(&self, key: &internal::ImportAttributeKey) -> DocId {
        match key {
            internal::ImportAttributeKey::Identifier(id) => self.d().symbol(id.name.to_u32()),
            internal::ImportAttributeKey::Literal(lit) => {
                if let internal::LiteralValue::String { content, .. } = &lit.value {
                    self.build_string_literal_key_doc(lit, content)
                } else {
                    // Attribute keys are only identifiers or string literals.
                    self.build_literal_doc(lit)
                }
            }
        }
    }

    /// Build doc for a single import attribute: `key: value`
    ///
    /// Handles comments between key, `:`, and value.
    fn build_import_attribute_doc(&self, attr: &internal::ImportAttribute) -> DocId {
        let d = self.d();
        let key_end = attr.key.span().end;
        let value_start = attr.value.span.start;

        // Check for comments between key and value (around the `:`)
        let has_comments = comments_in_range(self.comments, key_end, value_start)
            .next()
            .is_some();

        if !has_comments {
            // Fast path: no comments
            return d.concat(&[
                self.build_import_attribute_key_doc(&attr.key),
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

        let mut parts = vec![self.build_import_attribute_key_doc(&attr.key)];

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
