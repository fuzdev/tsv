// Module statement printing for TypeScript (import and export)
//
// ## Module Organization
//
// - **mod.rs** (this file): Shared constants, entry-point methods (import/export
//   declarations), and re-exports for submodules
// - **header_comments.rs**: Header-gap comment continuation helpers, `from`/source
//   rendering, and namespace `as` bindings
// - **import_attributes.rs**: `with { … }` import-attribute clause printing
// - **specifier_list.rs**: Braced comma-separated specifier/attribute list machinery

mod header_comments;
mod import_attributes;
mod specifier_list;

// Re-export for submodules to use `super::X` instead of `super::super::X`
pub(super) use super::{Printer, build_entity_name_doc};

use crate::ast::internal;
use smallvec::smallvec;
use tsv_lang::SymbolToU32;
use tsv_lang::comments_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

/// Byte length of the leading `import`/`export` keyword (both 6 chars). Added to
/// a declaration's span start to reach the position just past the keyword.
pub(super) const MODULE_KW_LEN: u32 = 6;
/// Byte length of `import type` / `export type` (both 11 chars) — fallback when
/// the `type` keyword can't be located by scanning (e.g. malformed source).
pub(super) const MODULE_TYPE_KW_LEN: u32 = 11;

impl<'a> Printer<'a> {
    /// Build a Doc for a TypeScript export assignment
    pub(super) fn build_export_assignment_doc(&self, decl: &internal::TSExportAssignment) -> DocId {
        let d = self.d();
        let expr_doc = self.build_expression_doc(&decl.expression);
        let argument_end = decl.expression.span().end;
        let has_trailing_comments = self.has_comments_between(argument_end, decl.span.end);
        if has_trailing_comments {
            let mut parts = smallvec![d.text("export = "), expr_doc];
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

            // A comment between `export` and the declaration is preserved in
            // place; a line comment indents the declaration one level (uniform
            // header rule). The keyword→declaration gap routes through the shared
            // continuation helper, so block/no-comment cases stay inline.
            //
            // For decorated classes, decorators come before the export keyword —
            // find the `export` keyword position from the last decorator end
            // (decl.span.start may include decorators in the internal AST).
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
                let continuation = self.build_class_declaration_without_decorators_doc(class);
                let tail = self.build_keyword_to_name_continuation(
                    export_keyword_end,
                    decl_start,
                    continuation,
                );
                return d.concat(&[dec_doc, d.text(export_keyword), tail]);
            }
            let continuation = self.build_statement_doc(declaration);
            let tail = self.build_keyword_to_name_continuation(
                export_keyword_end,
                decl_start,
                continuation,
            );
            d.concat(&[d.text(export_keyword), tail])
        } else {
            // export { x, y as z } or export { x } from "y"
            // Check if the overall export is type-only
            let is_type_export = decl.export_kind == internal::ExportKind::Type;

            let mut parts = if is_type_export {
                // Split the keyword so a type-only re-export (empty `{}` or named
                // specifiers) can keep a comment between `export` and `type` in place
                // — prettier relocates it (after `from` for empty, into the braces for
                // named specifiers). A line comment indents the `type …` continuation
                // one level; the leading space comes from the `export ` token. The
                // `type`→`{` gap is handled beside each brace path below. Mirrors the
                // import side.
                let kw_end = decl.span.start + MODULE_KW_LEN;
                let search_end = decl.source.as_ref().map_or(decl.span.end, |s| s.span.start);
                let type_start = self
                    .find_keyword_in_range(kw_end, search_end, "type")
                    .unwrap_or(kw_end);
                smallvec![
                    d.text("export "),
                    self.gap_comment_continuation_tail(kw_end, type_start, d.text("type ")),
                ]
            } else {
                smallvec![d.text("export ")]
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
                    // No re-export (`export {}`): keyword→`{}` gap, preserved in place.
                    // A line comment indents the `{}` continuation one level (indent-only
                    // divergence — prettier keeps the comment in place and flat). The gap
                    // spans to `}` so inside-braces comments are captured too (prettier
                    // relocates `export {/* c */}` → `export /* c */ {}`). The leading
                    // space comes from the `export `/`type ` token.
                    parts.push(self.gap_comment_continuation_tail(
                        keyword_end,
                        brace_close,
                        d.text("{}"),
                    ));
                } else {
                    // Re-export (`export {} from 'x'`): divergence — prettier relocates
                    // the keyword→`{` comment after `from`, so tsv preserves it in place
                    // and a line comment indents the `{}` continuation one level (the
                    // leading space comes from the `export `/`type ` token). Without
                    // this, `export /* c */ {} from 'x'` silently drops the comment.
                    // Inside-braces comments are relocated after `from` below.
                    let braces_doc = if let Some(brace_pos) =
                        self.find_char_outside_comments(keyword_end, semi_or_source, b'{')
                    {
                        self.gap_comment_continuation_tail(keyword_end, brace_pos, d.text("{}"))
                    } else {
                        d.text("{}")
                    };
                    parts.push(braces_doc);
                }
            } else {
                // Named specifiers: comment-aware braced list. `close_brace_end`
                // is the offset past `}`, for the trailing pre-`;` comment scan.
                let kw_end = self.export_header_end(decl, decl.specifiers[0].span.start);
                let bound = decl.source.as_ref().map_or(decl.span.end, |s| s.span.start);
                // Export named specifiers always have the `{` directly after the
                // header (no default/namespace binding), so the keyword→`{` comment
                // (`export /* c */ {a}`, `export type /* c */ {a}`) is always captured.
                close_brace_end = self.push_braced_specifier_list(
                    &mut parts,
                    &decl.specifiers,
                    kw_end,
                    bound,
                    true,
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
                    let mut parts = smallvec![expr_doc];
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

        // The `export default`→value gap (a line comment indents the value).
        let default_keyword = "export default";
        let keyword_end = decl.span.start + default_keyword.len() as u32;
        let decl_start = match &decl.declaration {
            internal::ExportDefaultValue::Expression(expr) => expr.span().start,
            internal::ExportDefaultValue::FunctionDeclaration(func) => func.span.start,
            internal::ExportDefaultValue::TSDeclareFunction(func) => func.span.start,
            internal::ExportDefaultValue::ClassDeclaration(class) => class.span.start,
        };
        // `export default`→value gap: a line comment indents the value one level
        // (uniform header rule); block/no-comment cases stay inline. Routes through
        // the shared continuation helper.
        d.concat(&[
            d.text("export default"),
            self.build_keyword_to_name_continuation(keyword_end, decl_start, value_doc),
        ])
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
            .map_or(decl.source.span.start, |e| e.span().start);
        let star_pos = self
            .find_char_outside_comments(export_end, star_limit, b'*')
            .unwrap_or(export_end);

        // Header comments (around `export`, `type`, `*`) are preserved where the user
        // placed them; prettier relocates every one to after `from`. A line comment
        // in any header gap indents the continuation one level (statement
        // continuation); the `*` is emitted via the tail helper so an `export`→`*`
        // or `type`→`*` line comment carries it onto the indented line.
        let mut parts = smallvec![d.text("export ")];
        if is_type {
            // `export`→`type` gap, then `type`→`*` gap.
            let type_start = self
                .find_keyword_in_range(export_end, star_pos, "type")
                .unwrap_or(export_end);
            parts.push(self.gap_comment_continuation_tail(export_end, type_start, d.text("type ")));
            let type_end = self
                .find_keyword_end("type", export_end, star_pos)
                .unwrap_or(export_end);
            parts.push(self.gap_comment_continuation_tail(type_end, star_pos, d.text("*")));
        } else {
            // `export`→`*` gap.
            parts.push(self.gap_comment_continuation_tail(export_end, star_pos, d.text("*")));
        }
        let star_end = star_pos + 1; // position just past `*`

        if let Some(exported) = &decl.exported {
            self.append_namespace_as_binding(&mut parts, star_end, exported);
        }

        // Comment between `*` (or `as ns`) and `from`, preserved in place — a same-line
        // block comment trails inline (`* /* c */ from`), a line comment indents the
        // `from …` continuation; prettier relocates it after `from`. Handled by
        // `build_from_source_doc`'s binding→`from` gap (the end of `*`/`as ns`).
        let prev_end = decl.exported.as_ref().map_or(star_end, |e| e.span().end);
        parts.push(self.build_from_source_doc(decl.span.start, &decl.source, None, Some(prev_end)));
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
        let mut parts = smallvec![d.text("import ")];

        // Add 'type' keyword for type-only imports
        if is_type_import {
            // Type-only import: preserve a comment between `import` and the `type`
            // keyword in place — prettier relocates it (after `from` for empty, into
            // the braces for named specifiers, to the binding side of `type` for
            // default/namespace). A line comment indents the `type …` continuation
            // one level (statement continuation); the leading space comes from the
            // `import ` token above. The `type`→binding/`{` gap is handled beside
            // each form.
            let kw_end = decl.span.start + MODULE_KW_LEN;
            let type_start = self
                .find_keyword_in_range(kw_end, decl.source.span.start, "type")
                .unwrap_or(kw_end);
            parts.push(self.gap_comment_continuation_tail(kw_end, type_start, d.text("type ")));
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
            // keyword(s)→default-binding gap, preserved in place. A line comment
            // indents the binding one level (statement continuation); prettier keeps
            // the comment in place and flattens the binding, so this is an indent-only
            // divergence (like `as`→binding). The leading space comes from the
            // `import `/`type ` token.
            parts.push(self.gap_comment_continuation_tail(
                header_end,
                default_spec_start,
                d.symbol(default_sym),
            ));
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
                parts.push(d.text("*"));
            } else {
                // keyword(s)→namespace-`*` gap, preserved in place. A line comment
                // indents the `* as ns` continuation (indent-only divergence — prettier
                // keeps it flat in place); the `*` rides the helper so the line comment
                // carries it onto the indented line.
                parts.push(self.gap_comment_continuation_tail(
                    header_end,
                    ns_spec.span.start,
                    d.text("*"),
                ));
            }
            // `* as ns` binding; preserves the `*`→`as` comment in place (mirrors the
            // export-all side).
            let star_end = ns_spec.span.start + 1;
            // An import namespace binding is always an identifier; wrap it to share
            // the `ModuleExportName`-based renderer with the export-all side.
            let binding = internal::ModuleExportName::Identifier(ns_spec.local.clone());
            self.append_namespace_as_binding(&mut parts, star_end, &binding);
            from_content_end = Some(ns_spec.span.end);
        }

        // Build named specifiers with group wrapping (or empty braces if source had them).
        //
        // An empty named group *after a default/namespace binding* (`import a, {}
        // from 'x'`) carries no specifiers, so it's dropped to match prettier
        // (`import a from 'x'`): the binding→`from` gap — and any comment in it — is
        // then handled exactly like a plain default import (a plain match, and
        // idempotent; emitting `, {}` here instead duplicated the gap comment on each
        // reformat). A *bare* empty group (`import {} from 'x'`, no binding) is kept.
        // A leading `default` / `* as ns` binding (as opposed to the named group).
        let has_binding = has_default || namespace_spec.is_some();
        let drop_empty_after_binding = has_empty_braces && named_specs.is_empty() && has_binding;

        if !named_specs.is_empty() || (has_empty_braces && !drop_empty_after_binding) {
            if has_binding {
                // For named imports after default: prettier moves all comments between
                // default end and `{` to before the comma: `import x /* c */, {a}`.
                // The `{` is found outside comments so a `{` glyph in a comment isn't
                // mistaken for it. (Empty braces after a binding were dropped above,
                // so `named_specs` is non-empty here.)
                let prev_end = namespace_spec.map_or(default_spec_end, |ns| ns.span.end);
                let brace_pos = self
                    .find_char_outside_comments(prev_end, named_specs[0].span.start, b'{')
                    .unwrap_or(named_specs[0].span.start);
                parts.push(self.build_inline_comments_between_doc(prev_end, brace_pos));
                parts.push(d.text(", "));
            }

            if named_specs.is_empty() {
                // Bare empty braces: `import {} from 'x'` (no binding — the binding
                // case was dropped above). Preserve the keyword→`{` (or `type`→`{`)
                // comment in place — prettier relocates it after `from`; a line
                // comment indents the `{}` continuation one level (the leading space
                // comes from the `import `/`type ` token). Without this,
                // `import /* c */ {} from 'x'` silently drops the comment.
                let braces_doc = if let Some(brace_pos) =
                    self.find_char_outside_comments(header_end, decl.source.span.start, b'{')
                {
                    self.gap_comment_continuation_tail(header_end, brace_pos, d.text("{}"))
                } else {
                    d.text("{}")
                };
                parts.push(braces_doc);
            } else {
                // Named specifiers: comment-aware braced list. `from_content_end`
                // is the offset past `}`, for the `}`→`from` gap comment scan.
                let kw_end = self.import_header_end(decl, named_specs[0].span.start);
                // Capture the keyword→`{` comment here only when the brace directly
                // follows the header; with a default/namespace binding its own→`{`
                // comments are handled above (line builds `x, {…}`), so capturing
                // here too would double-emit them.
                let capture_keyword_comment = !has_binding;
                from_content_end = Some(self.push_braced_specifier_list(
                    &mut parts,
                    &named_specs,
                    kw_end,
                    decl.source.span.start,
                    capture_keyword_comment,
                    |s| s.span,
                    |s| self.build_import_specifier_doc(s, is_type_import),
                ));
            }
        }

        // Add "from" and source, extracting comments between keywords and source literal
        if !decl.specifiers.is_empty() || has_empty_braces {
            // Only a *bare* empty group relocates its inside-braces comment after
            // `from`; a dropped empty group (after a binding) carries no braces, so
            // its gap comment stays in place via `from_content_end` (plain-default path).
            let empty_brace_start =
                if has_empty_braces && named_specs.is_empty() && !drop_empty_after_binding {
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
            // Bare import (`import 'x'`): keyword→source gap, preserved in place. A line
            // comment indents the source one level (indent-only divergence — prettier
            // keeps it flat in place); the leading space comes from the `import ` token.
            let keyword = if is_type_import {
                "import type"
            } else {
                "import"
            };
            let keyword_end = decl.span.start + keyword.len() as u32;
            parts.push(self.gap_comment_continuation_tail(
                keyword_end,
                decl.source.span.start,
                self.build_literal_doc(&decl.source),
            ));
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

    /// Build doc for `import x = require("y")` or `import x = A.B`
    pub(super) fn build_import_equals_declaration_doc(
        &self,
        decl: &internal::TSImportEqualsDeclaration,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

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
                let require_open_end = ext_ref.span.start + "require(".len() as u32;
                let literal_start = ext_ref.expression.span.start;
                let has_comments = self.has_line_comments_between(require_open_end, literal_start);

                if has_comments {
                    // Multi-line format with comments
                    // Build comments doc: each comment on its own line
                    let mut comment_parts = DocBuf::new();
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
}
