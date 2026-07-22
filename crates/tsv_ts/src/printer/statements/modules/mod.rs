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
use crate::printer::calls::PartitionedComments;
use crate::printer::needs_parens::export_default_needs_parens;
use smallvec::SmallVec;
use smallvec::smallvec;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// Byte length of the leading `import`/`export` keyword (both 6 chars). Added to
/// a declaration's span start to reach the position just past the keyword.
pub(super) const MODULE_KW_LEN: u32 = 6;
/// Byte length of `import type` / `export type` (both 11 chars) — fallback when
/// the `type` keyword can't be located by scanning (e.g. malformed source).
pub(super) const MODULE_TYPE_KW_LEN: u32 = 11;

impl<'a> Printer<'a> {
    /// Build a Doc for a TypeScript export assignment
    pub(super) fn build_export_assignment_doc(
        &self,
        decl: &internal::TSExportAssignment<'_>,
    ) -> DocId {
        let d = self.d();
        let expr_doc = self.build_expression_doc(&decl.expression);
        let argument_end = decl.expression.span().end;
        // `export =` word by word: the `export`→`=` gap is a position an author can
        // comment in. Emitting the two as one text never scans it — the comment would
        // be dropped.
        let head = self.build_keyword_header_doc(
            &["export", "="],
            decl.span.start,
            decl.expression.span().start,
            expr_doc,
        );
        let has_trailing_comments = self.has_comments_to_emit_between(argument_end, decl.span.end);
        if has_trailing_comments {
            // `export =` keeps a same-line trailing block comment *before* the `;`
            // (operand-attached — prettier 3.9 does not move it, unlike `export default`
            // / named exports). A line comment still floats after the `;` via `line_suffix`.
            let mut parts = smallvec![head];
            self.append_trailing_paren_comments(&mut parts, argument_end, decl.span.end);
            parts.push(d.text(";"));
            d.concat(&parts)
        } else {
            d.concat(&[head, d.text(";")])
        }
    }

    /// Build a Doc for an export named declaration
    ///
    /// Uses d.group() for width-based wrapping of specifiers.
    /// When the line exceeds print_width (100 chars), wraps to multiline format.
    pub(super) fn build_export_named_declaration_doc(
        &self,
        decl: &internal::ExportNamedDeclaration<'_>,
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
            // Decorated classes are the exception: the decorators and `export` can
            // appear in either order (`@dec export class` / `export @dec class`), so
            // the block below emits them in the source order rather than through the
            // plain keyword→declaration continuation.
            if let internal::Statement::ClassDeclaration(class) = declaration
                && let Some(decorators) = class.decorators.filter(|d| !d.is_empty())
            {
                let continuation = self.build_class_declaration_without_decorators_doc(class);
                // Decorators can sit before OR after `export` — prettier preserves the
                // author's choice as two distinct stable forms (`@dec export class` vs
                // `export @dec class`). The parser records which by the export decl's
                // start: the first-decorator start for the decorator-first form, the
                // `export` keyword for the export-first form. So a first decorator
                // positioned *after* the export start is the export-first form.
                let export_first = decorators[0].span.start > decl.span.start;

                // The token right after the decorators is the trailing-comment boundary
                // for `build_decorators_doc`: the class keyword (`abstract`/`class`) for
                // the export-first form, `export` for the decorator-first form.
                let next_after_decorators = if export_first {
                    let class_kw = if class.r#abstract {
                        "abstract"
                    } else {
                        "class"
                    };
                    self.find_keyword_after_decorators(class.decorators, class_kw, class.span.start)
                } else {
                    self.find_keyword_after_decorators(class.decorators, "export", decl.span.start)
                };

                // `decorators` is non-empty, so this is always `Some`; the `if let`
                // keeps the code off `expect()` (clippy::expect_used) and falls through
                // to the plain declaration path in the (unreachable) `None` case.
                if let Some(dec_doc) =
                    self.build_decorators_doc(class.decorators, next_after_decorators)
                {
                    if export_first {
                        // `export` on its own line, then the (always own-line) decorators,
                        // then the class.
                        let mut parts: DocBuf = smallvec![d.text(export_keyword)];
                        // A comment between `export` and the first decorator is rare but
                        // must be preserved (never dropped).
                        if let Some(c) = self.build_inline_comments_between_doc_opt(
                            export_keyword_end,
                            decorators[0].span.start,
                        ) {
                            parts.push(c);
                        }
                        parts.push(d.hardline());
                        parts.push(dec_doc);
                        parts.push(continuation);
                        return d.concat(&parts);
                    }

                    // Decorator-first (`@dec export class`): decorators, then `export`,
                    // then the class.
                    let tail = self.build_keyword_to_name_continuation(
                        export_keyword_end,
                        decl_start,
                        continuation,
                    );
                    return d.concat(&[dec_doc, d.text(export_keyword), tail]);
                }
            }
            // `declaration` is always a declaration form (never an
            // ExpressionStatement), so `in_program_or_block` is never consulted here.
            let continuation = self.build_statement_doc(declaration, true);
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
                    decl.specifiers,
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
                    decl.attributes,
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
        decl: &internal::ExportDefaultDeclaration<'_>,
    ) -> DocId {
        let d = self.d();
        // Decorator-*first* `@dec export default class` — the decorators precede
        // `export`, and the whole thing parses as a ClassDeclaration. Emit the
        // decorators, then `export default`, then the class without them.
        if let internal::ExportDefaultValue::ClassDeclaration(class) = &decl.declaration {
            let export_start =
                self.find_keyword_after_decorators(class.decorators, "export", decl.span.start);
            if let Some(dec_doc) = self.build_decorators_doc(class.decorators, export_start) {
                // `export default` word by word, bounded by the class's own first
                // keyword: both the gap *inside* the keyword and the one after it are
                // positions an author can comment in. Emitting the two words as one
                // fixed text scans neither, so both comments are dropped.
                return d.concat(&[
                    dec_doc,
                    self.build_keyword_header_doc(
                        &["export", "default"],
                        export_start,
                        self.class_declaration_keyword_start(class),
                        self.build_class_declaration_without_decorators_doc(class),
                    ),
                ]);
            }
        }

        // Decorator-*after*-`default` `export default @dec class {}` — the decorator
        // makes it a class *expression* (acorn), so it lands in the generic Expression
        // arm below. But prettier formats it declaration-style: `export default` on its
        // own line, the (always own-line) decorators, and NO trailing `;`. The class-
        // expression doc already renders the decorators own-line, so emit `export
        // default`, a hardline, then that doc.
        if let internal::ExportDefaultValue::Expression(internal::Expression::ClassExpression(
            class_expr,
        )) = &decl.declaration
            && let Some(decorators) = class_expr.decorators.filter(|dec| !dec.is_empty())
        {
            // `export default` word by word: the gap *inside* the keyword is a
            // position an author can comment in, so the words are located rather than
            // measured (measuring never scans that gap — the comment would be dropped).
            let (keyword_doc, keyword_end) = self.build_keyword_words_doc(
                &["export", "default"],
                decl.span.start,
                decorators[0].span.start,
            );
            let mut parts: DocBuf = smallvec![keyword_doc];
            // A comment between `export default` and the first decorator is rare but
            // must be preserved (never dropped). Two authorings, two owners: a comment
            // *glued* to `@dec` is owned by the class expression (every glued block
            // comment is), so it is skipped here by design and claimed below; one the
            // author left on its own line is unowned and belongs to this gap.
            if let Some(c) =
                self.build_inline_comments_between_doc_opt(keyword_end, decorators[0].span.start)
            {
                parts.push(c);
            }
            parts.push(d.hardline());
            // This path **reassembles** the class expression rather than routing it
            // through `build_expression_doc`, so the owned-comment seam there never
            // runs for it — the comment must be claimed here or nothing prints it.
            parts.push(self.prepend_owned_leading_comment_at(
                class_expr.span.start,
                self.build_class_expression_doc(class_expr),
            ));
            return d.concat(&parts);
        }

        let value_doc = match &decl.declaration {
            internal::ExportDefaultValue::Expression(expr) => {
                let mut expr_doc = self.build_expression_doc(expr);
                // Prettier wraps the exported expression when its leftmost
                // (first-printed) token is a function/class keyword — else
                // `export default function () {}.m()` reparses the function as a
                // *declaration* and the trailing `.m()` / `= …` / `as T` dangles.
                // Mirrors prettier's `startsWithNoLookaheadToken(expr, isFunctionOrClass)`
                // (parentheses/needs-parentheses.js). Decorated class expressions are
                // handled above; the FunctionDeclaration/ClassDeclaration arms cover
                // bare `export default function/class …`.
                if export_default_needs_parens(expr) {
                    expr_doc = d.concat(&[d.text("("), expr_doc, d.text(")")]);
                }
                let argument_end = expr.span().end;
                let has_trailing_comments =
                    self.has_comments_to_emit_between(argument_end, decl.span.end);
                if has_trailing_comments {
                    let mut parts = smallvec![expr_doc];
                    let after = self.split_terminator_gap_comments(
                        &mut parts,
                        argument_end,
                        decl.span.end,
                        false,
                    );
                    parts.push(d.text(";"));
                    parts.extend(after);
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
            internal::ExportDefaultValue::TSInterfaceDeclaration(iface) => {
                self.build_interface_declaration_doc(iface)
            }
        };

        let decl_start = match &decl.declaration {
            internal::ExportDefaultValue::Expression(expr) => expr.span().start,
            internal::ExportDefaultValue::FunctionDeclaration(func) => func.span.start,
            internal::ExportDefaultValue::TSDeclareFunction(func) => func.span.start,
            internal::ExportDefaultValue::ClassDeclaration(class) => class.span.start,
            internal::ExportDefaultValue::TSInterfaceDeclaration(iface) => iface.span.start,
        };
        // The `export default`→value gap (a line comment indents the value). The
        // keyword's own words are located, not measured — the gap *between* them is a
        // position an author can comment in, and measuring never scans it.
        let (keyword_doc, keyword_end) =
            self.build_keyword_words_doc(&["export", "default"], decl.span.start, decl_start);
        // A comment that can't stay inline forces the value onto its own indented
        // line, keeping the comment where the author wrote it. This gap uses the
        // SHARED keyword→value gate — the same one `as`/`satisfies`, `keyof`/`typeof`,
        // `infer`, and the type-alias `=` use — so only a **line** comment (runs to
        // end-of-line) or a **multiline** block the author broke after hangs the value.
        // A single-line block does not: nothing forces it off the line, so the author's
        // break is ordinary layout and is reflowed (§Authored breaks in value position).
        // This gap used to carve itself out via `comment_hangs_binary_operand`,
        // which also hangs a single-line block — that made `export default` the lone
        // value gap preserving an unforced break, disagreeing with its own twin
        // `export =`. Prettier keeps the break at both; tsv reflows at both.
        if self.comments_force_own_line_between(keyword_end, decl_start) {
            let mut parts: DocBuf = smallvec![keyword_doc];
            self.append_keyword_value_line_comments(&mut parts, keyword_end, decl_start, value_doc);
            return d.concat(&parts);
        }
        // No forcing comment (inline block / none): the value stays on the keyword
        // line via the shared continuation helper.
        d.concat(&[
            keyword_doc,
            self.build_keyword_to_name_continuation(keyword_end, decl_start, value_doc),
        ])
    }

    /// Build a Doc for an export all declaration
    pub(super) fn build_export_all_declaration_doc(
        &self,
        decl: &internal::ExportAllDeclaration<'_>,
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
            decl.attributes,
            decl.source.span.end,
            decl.span.end,
        );
        self.finish_with_pre_semi(parts, content_end, decl.span.end, false)
    }

    /// Build a Doc for an import declaration
    ///
    /// Uses d.group() for width-based wrapping of named specifiers.
    /// When the line exceeds print_width (100 chars), wraps to multiline format.
    pub(super) fn build_import_declaration_doc(
        &self,
        decl: &internal::ImportDeclaration<'_>,
    ) -> DocId {
        let d = self.d();
        // Check if source has empty braces (for `import {} from 'x'`)
        let has_empty_braces = self.has_empty_named_braces(decl);

        // Check if this is a type-only import
        let is_type_import = decl.import_kind == internal::ImportKind::Type;

        // Collect specifiers
        let mut has_default = false;
        // Inline up to 8 named specifiers (covers the common import) before spilling.
        let mut named_specs: SmallVec<[&internal::ImportNamedSpecifier<'_>; 8]> = SmallVec::new();
        let mut default_name = internal::IdentName {
            escaped: None,
            raw_len: 0,
        };
        let mut default_name_start = 0u32;
        let mut default_spec_start = 0u32;
        let mut default_spec_end = 0u32;
        let mut namespace_spec: Option<&internal::ImportNamespaceSpecifier<'_>> = None;

        for spec in decl.specifiers {
            match spec {
                internal::ImportSpecifier::Default(default_spec) => {
                    has_default = true;
                    default_name = default_spec.local.ident_name();
                    default_name_start = default_spec.local.span.start;
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

        // Phase keyword for the import-phase proposals: `import source …` (binding)
        // / `import defer …` (namespace). Mutually exclusive with `type`. The
        // keyword spelling is `ImportPhase::as_str`'s — the single source of truth.
        if let Some(kw) = decl.phase.as_str() {
            parts.push(d.text(kw));
            parts.push(d.text(" "));
        }

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
                self.ident_name_doc(default_name, default_name_start),
            ));
            from_content_end = Some(default_spec_end);
        }

        // Add namespace import
        if let Some(ns_spec) = namespace_spec {
            if has_default {
                // Comments between default specifier and comma → emit before comma
                // Comments between comma and `*` → emit after comma
                let comma_pos = self.comma_between(default_spec_end, ns_spec.span.start);
                parts.push(self.build_inline_comments_between_doc(default_spec_end, comma_pos));
                parts.push(d.text(", "));
                parts.push(self.build_inline_comments_between_doc_trailing_space(
                    comma_pos + 1,
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
            decl.attributes,
            decl.source.span.end,
            decl.span.end,
        );
        self.finish_with_pre_semi(parts, content_end, decl.span.end, true)
    }

    /// Build doc for `import x = require("y")` or `import x = A.B`
    pub(super) fn build_import_equals_declaration_doc(
        &self,
        decl: &internal::TSImportEqualsDeclaration<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

        // The header keyword, word by word — an optional `export` prefix, `import`, and
        // an optional `type` modifier. Every gap between them is a position an author
        // can comment in; emitting the words as separate fixed texts scans none of
        // them, so a comment in any gap is dropped. Prettier preserves all of these
        // except `import`→`type`, which it moves to the binding side.
        let mut words: SmallVec<[&'static str; 3]> = SmallVec::new();
        if decl.is_export {
            words.push("export");
        }
        words.push("import");
        if matches!(decl.import_kind, internal::ImportKind::Type) {
            words.push("type");
        }
        parts.push(self.build_keyword_header_doc(
            &words,
            decl.span.start,
            decl.id.span.start,
            self.identifier_name_doc(&decl.id),
        ));

        // One walk of the reference: its span bounds and its doc. Built whole (rather
        // than pushed piecemeal) so the `=`→reference gap below can pass it as its
        // continuation — a line comment there indents the reference, and every break
        // *inside* it has to land at that level too.
        let (module_ref_start, ref_end, module_ref_doc) = match &decl.module_reference {
            internal::TSModuleReference::ExternalModuleReference(ext) => (
                ext.span.start,
                ext.span.end,
                self.build_external_module_reference_doc(ext),
            ),
            internal::TSModuleReference::EntityName(entity) => (
                entity.span().start,
                entity.span().end,
                build_entity_name_doc(self, entity),
            ),
        };

        // identifier→`=` and `=`→module-reference gaps. Both are preserved in place
        // (prettier keeps them there too). Nothing else scans them — a module
        // reference is not an expression, so a comment glued to `require` is not
        // owned by any node and would be dropped if this gap didn't emit it.
        match self.find_keyword_in_range(decl.id.span.end, module_ref_start, "=") {
            Some(eq_start) => {
                // Both gaps FLAT, one `indent` for the whole tail — the multi-word
                // keyword rule (`build_keyword_words_doc`), which this header is the
                // other instance of. Indenting per gap compounds; and the tail must be
                // *inside* the indent, not a sibling after it, or the reference's own
                // line breaks resolve at the outer level — leaving `require(`'s
                // contents level with it and its `)` a level above.
                let (before_eq, line_before) =
                    self.build_keyword_gap_doc(decl.id.span.end, eq_start);
                let (after_eq, line_after) =
                    self.build_keyword_gap_doc(eq_start + 1, module_ref_start);
                let tail = d.concat(&[before_eq, d.text("="), after_eq, module_ref_doc]);
                parts.push(if line_before || line_after {
                    d.indent(tail)
                } else {
                    tail
                });
            }
            None => {
                // Same landmine as `build_keyword_words_doc`'s fallback: this arm scans
                // no gap, so a comment either side of the `=` is dropped. An
                // import-equals always spells its `=` between the name and the module
                // reference, so this is unreachable — assert it in debug rather than
                // let a future caller degrade silently.
                debug_assert!(
                    false,
                    "import-equals has no `=` in source[{}..{module_ref_start}]",
                    decl.id.span.end
                );
                parts.push(d.text(" = "));
                parts.push(module_ref_doc);
            }
        }

        // Comments between the module reference and `;`: like `export =` (and unlike
        // named imports / re-exports), a same-line trailing **block** comment stays
        // *before* the `;` (operand-attached — prettier 3.9 keeps it), while a same-line
        // **line** comment floats after the `;` via `line_suffix`. So this uses the
        // comma-style `block_after_separator: false`, not `finish_with_pre_semi`.
        let semicolon_pos = decl.span.end.saturating_sub(1);
        self.push_semicolon_with_gap_comments(&mut parts, ref_end, semicolon_pos, false);
        d.concat(&parts)
    }

    /// The `require('m')` form of an import-equals module reference, as one doc.
    ///
    /// **Both** in-paren gaps are emitted — `require(`→literal and literal→`)`. Nothing
    /// else scans either: a module reference is not an expression, so no node owns a
    /// comment written here and a gap this printer skips is a gap whose comment is
    /// dropped outright (silent content loss, which is what the literal→`)` gap used to
    /// do).
    ///
    /// The close gap takes the shape a call's **last argument** already has
    /// ([`PartitionedComments::emit_last_arg_comments`]): a same-line comment trails the
    /// literal, an own-line one dangles above the `)`. There is no trailing comma to
    /// split around (`trailingComma: 'none'`), which is exactly that helper's premise.
    fn build_external_module_reference_doc(
        &self,
        ext_ref: &internal::TSExternalModuleReference<'_>,
    ) -> DocId {
        let d = self.d();
        let require_open_end = ext_ref.span.start + "require(".len() as u32;
        let literal_start = ext_ref.expression.span.start;
        let close_paren = ext_ref.span.end.saturating_sub(1);

        // The literal plus whatever trails it inside the parens.
        let close = PartitionedComments::new(
            self.comments,
            self.comment_line_breaks,
            ext_ref.expression.span.end,
            close_paren,
        );
        let mut value: DocBuf = smallvec![self.build_literal_doc(&ext_ref.expression)];
        close.emit_last_arg_comments(&mut value, self);
        let value_doc = d.concat(&value);

        // A line comment runs to EOL and an own-line comment must keep its own line, so
        // either side having one forces the parens open; a lone same-line block stays
        // inline. Same rule as the dynamic-import parens (`calls/import_expr.rs`).
        let open_has_line = self.has_line_comments_between(require_open_end, literal_start);
        let open_has_any = self.has_comments_to_emit_between(require_open_end, literal_start);
        let close_forces_break = close.has_trailing_line() || !close.leading.is_empty();

        if open_has_line || close_forces_break {
            let mut inner = DocBuf::new();
            if open_has_line {
                // Each open-gap comment on its own line — a line comment there can't
                // share one with the literal.
                for comment in
                    comments_to_emit_in_range(self.comments, require_open_end, literal_start)
                {
                    inner.push(self.build_comment_doc(comment));
                    inner.push(d.hardline());
                }
            } else if open_has_any {
                // Only the close gap forced the break, so an open-gap block keeps
                // hugging the literal rather than being relocated to its own line.
                inner.push(self.build_inline_comments_between_doc_trailing_space(
                    require_open_end,
                    literal_start,
                ));
            }
            inner.push(value_doc);
            return d.concat(&[
                d.text("require("),
                d.indent(d.concat(&[d.hardline(), d.concat(&inner)])),
                d.hardline(),
                d.text(")"),
            ]);
        }
        if open_has_any {
            // The comment hugs the `(` and keeps its space on the literal's side
            // (`require(/* c */ 'm')`) — the dotted pair's after-`.` rule, and prettier's.
            // An author who wrote it *before* the `(` lands here too: both formatters move
            // it inside, so the two authorings converge (`unformatted_before_paren`).
            return d.concat(&[
                d.text("require("),
                self.build_inline_comments_between_doc_trailing_space(
                    require_open_end,
                    literal_start,
                ),
                value_doc,
                d.text(")"),
            ]);
        }
        d.concat(&[d.text("require("), value_doc, d.text(")")])
    }

    /// `export as namespace Foo;` — TypeScript UMD global export declaration.
    pub(super) fn build_namespace_export_declaration_doc(
        &self,
        decl: &internal::TSNamespaceExportDeclaration<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        // `export as namespace` word by word: each gap between the three is a position
        // an author can comment in, and the `namespace`→name gap is one prettier keeps
        // too. Emitting the keyword as one text never scans any of them.
        parts.push(self.build_keyword_header_doc(
            &["export", "as", "namespace"],
            decl.span.start,
            decl.id.span.start,
            self.identifier_name_doc(&decl.id),
        ));
        // Trailing comment between the name and `;` (mirrors `export =` / import-equals):
        // a same-line block comment stays before `;`, a line comment floats after it.
        let semicolon_pos = decl.span.end.saturating_sub(1);
        self.push_semicolon_with_gap_comments(&mut parts, decl.id.span.end, semicolon_pos, false);
        d.concat(&parts)
    }
}
