// Braced comma-separated specifier/attribute list machinery for module
// statements: header-offset helpers, the brace group wrapper, the generic
// softline/hardline comma-list builders, and the per-specifier docs shared by
// import and export named specifiers.

use super::header_comments::is_only_whitespace_and_comments;
use super::{MODULE_KW_LEN, MODULE_TYPE_KW_LEN, Printer};
use crate::ast::internal;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;
use tsv_lang::{SymbolToU32, comments_in_range};

impl<'a> Printer<'a> {
    /// Check if an import declaration has empty named braces `{}` in source.
    /// This distinguishes `import {} from 'x'` from `import 'x'`.
    /// Also matches braces containing only whitespace and/or comments:
    /// `import { /* c */ } from 'x'`, `import { // c\n } from 'x'`.
    pub(super) fn has_empty_named_braces(&self, decl: &internal::ImportDeclaration<'_>) -> bool {
        let text = decl.span.extract(self.source);
        // Find the `from` keyword, skipping comments and not matching inside an
        // identifier — so empty-brace detection isn't fooled by a `from` in a
        // comment (`import // {} from\n'a'`) or a specifier name (`fromage`).
        let from_pos = tsv_lang::source_scan::find_keyword(
            text.as_bytes(),
            0,
            text.len(),
            b"from",
            tsv_lang::source_scan::TriviaProfile::COMMENTS,
        );
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

    /// Position just past the leading keyword(s) of an import declaration: after the
    /// `type` keyword for a type-only import (located by scanning, so a comment in
    /// the `import`→`type` gap doesn't throw off the offset), else after `import`.
    /// `search_end` bounds the `type` scan — the source literal start, or the first
    /// specifier start when a tighter bound is needed to avoid matching `type`
    /// inside the specifier list.
    pub(super) fn import_header_end(
        &self,
        decl: &internal::ImportDeclaration<'_>,
        search_end: u32,
    ) -> u32 {
        let is_type = decl.import_kind == internal::ImportKind::Type;
        let base = self.module_header_end(is_type, decl.span.start, search_end);
        // Skip the phase keyword (`source `/`defer `) for the import-phase proposals
        // so the default-binding / namespace comment scan starts after it.
        base + match decl.phase {
            internal::ImportPhase::Source => "source ".len() as u32,
            internal::ImportPhase::Defer => "defer ".len() as u32,
            internal::ImportPhase::None => 0,
        }
    }

    /// Position just past the leading keyword(s) of an export named declaration:
    /// after the `type` keyword for a type-only re-export (located by scanning, so a
    /// comment in the `export`→`type` gap doesn't throw off the offset), else after
    /// `export`. `search_end` bounds the `type` scan — the source/`;`, or the first
    /// specifier start to avoid matching `type` inside the specifier list.
    pub(super) fn export_header_end(
        &self,
        decl: &internal::ExportNamedDeclaration<'_>,
        search_end: u32,
    ) -> u32 {
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
    /// statement: `{ <inner> }` with bracketSpacing padding (a space when flat,
    /// `{ a }`, a newline when the group breaks). The independent group keeps a
    /// preserved header line comment (which forces the outer group to break) from
    /// expanding a `{ a }` that would otherwise stay inline. Shared by named
    /// imports, named exports, and `with {…}`/`assert {…}` import attributes.
    pub(super) fn braced_group(&self, inner: DocId) -> DocId {
        let d = self.d();
        d.group(d.concat(&[d.text("{"), d.indent_line(inner), d.line(), d.text("}")]))
    }

    /// Finish a module statement: emit any comments between the last content token
    /// and the terminating `;`, then the `;` itself.
    ///
    /// When `grouped`, `parts` is wrapped in a `group` for width-based wrapping and
    /// the pre-`;` comments are emitted *outside* it, so a line-comment break can't
    /// expand the statement's specifier braces (import/export named declarations).
    /// Otherwise the comments are appended inline to `parts` — used by export-all
    /// and import-equals, which have no wrapping group.
    pub(super) fn finish_with_pre_semi(
        &self,
        mut parts: DocBuf,
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
        let mut trailing = DocBuf::new();
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

    /// The source offset of a closing `}` — the first `}` (outside comments, so a
    /// `}` inside a trailing comment is skipped) at or after `search_start`,
    /// bounded by `bound` (the fallback when the brace can't be located). Shared
    /// by the named-specifier brace scans (import/export) and the attribute clause.
    pub(super) fn close_brace_offset(&self, search_start: u32, bound: u32) -> u32 {
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
    /// `export {…}`). When `capture_keyword_comment`, a comment in the keyword→`{`
    /// gap (`import /* c */ {a}`, `export type /* c */ {a}`) is preserved in place
    /// (prettier relocates it into the braces as the first specifier's leading
    /// comment — a comment-position divergence). The caller sets it only when the
    /// `{` directly follows the header — always so for exports, and for imports only
    /// without a preceding default/namespace binding (whose own→`{` comments are
    /// handled separately, so capturing here would double-emit them).
    // Two closures (span + per-item doc) plus positional context — inherent to a
    // generic list builder; sibling `build_braced_hardline_comma_list` is at 7.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn push_braced_specifier_list<T>(
        &self,
        parts: &mut DocBuf,
        specifiers: &[T],
        kw_end: u32,
        bound: u32,
        capture_keyword_comment: bool,
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

        let braces_doc = if has_expanding_comments {
            // `Some(first_start)` keeps a same-line `{` comment on the brace line
            // (divergence from prettier, which relocates it as the first
            // specifier's leading comment).
            self.build_braced_hardline_comma_list(
                specifiers,
                brace_start,
                brace_close,
                Some(first_start),
                &get_span,
                &build_item,
            )
        } else {
            // No expanding comments: group-based wrapping with comment splitting.
            let spec_doc = self.build_softline_comma_list(
                specifiers,
                brace_start,
                brace_close,
                &get_span,
                &build_item,
            );
            self.braced_group(spec_doc)
        };

        // The keyword→`{` gap comment (`import /* c */ {a}`, `import type // c\n{a}`,
        // and the export forms) is preserved before the brace; prettier relocates it
        // into the braces. A line comment forces `{…}` onto a new line, which the
        // shared helper indents one level (statement continuation) — the leading
        // space comes from the caller's `import `/`export `/`type ` token. Captured
        // only when the `{` directly follows the header (see `capture_keyword_comment`).
        if capture_keyword_comment {
            parts.push(self.gap_comment_continuation_tail(kw_end, brace_start, braces_doc));
        } else {
            parts.push(braces_doc);
        }
        brace_close + 1
    }

    /// Build a doc for a renamed `{a}` / `{a as b}` specifier — shared by import and
    /// export specifiers, which differ only in field order (import reads
    /// `imported`→`local`, export reads `local`→`exported`).
    ///
    /// Emits an optional per-specifier `type ` prefix (skipped when the whole
    /// declaration is already `import type` / `export type`), the `left` identifier,
    /// and — when it's a rename — the ` as ` join with any comments in the `as` gap
    /// split around the keyword (before-`as` inline, after-`as` with trailing space).
    fn build_renamed_specifier_doc(
        &self,
        declaration_is_type_only: bool,
        specifier_is_type: bool,
        left: &internal::ModuleExportName<'_>,
        right: &internal::ModuleExportName<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        if !declaration_is_type_only && specifier_is_type {
            parts.push(d.text("type "));
        }
        parts.push(self.build_module_export_name_doc(left));
        let (left_span, right_span) = (left.span(), right.span());
        // Compare spans, not values: `{a}` has one span, `{a as a}` has two.
        if left_span != right_span {
            // Split comments at the `as` keyword: before-as and after-as.
            if let Some(as_pos) = self.find_keyword_in_range(left_span.end, right_span.start, "as")
            {
                parts.push(self.build_inline_comments_between_doc(left_span.end, as_pos));
                parts.push(d.text(" as "));
                let as_end = as_pos + "as".len() as u32;
                parts.push(
                    self.build_inline_comments_between_doc_trailing_space(as_end, right_span.start),
                );
            } else {
                parts.push(d.text(" as "));
            }
            parts.push(self.build_module_export_name_doc(right));
        }
        d.concat(&parts)
    }

    /// Build a doc for a `ModuleExportName`: a bare identifier emits its symbol;
    /// a string name (`'str'`) emits a quote-normalized string literal (preserved
    /// as a string — prettier keeps the form, never stripping to a bare identifier).
    pub(super) fn build_module_export_name_doc(
        &self,
        name: &internal::ModuleExportName<'_>,
    ) -> DocId {
        match name {
            internal::ModuleExportName::Identifier(id) => self.d().symbol(id.name.to_u32()),
            internal::ModuleExportName::Literal(lit) => self.build_literal_doc(lit),
        }
    }

    /// Build a doc for a single import specifier
    pub(super) fn build_import_specifier_doc(
        &self,
        named_spec: &internal::ImportNamedSpecifier<'_>,
        is_type_import: bool,
    ) -> DocId {
        // The local binding is always an identifier; wrap it so it shares the
        // `ModuleExportName`-based renamed-specifier renderer with the imported name.
        let local = internal::ModuleExportName::Identifier(named_spec.local.clone());
        self.build_renamed_specifier_doc(
            is_type_import,
            named_spec.import_kind == internal::ImportKind::Type,
            &named_spec.imported,
            &local,
        )
    }

    /// Build a doc for a single export specifier
    pub(super) fn build_export_specifier_doc(
        &self,
        spec: &internal::ExportSpecifier<'_>,
        is_type_export: bool,
    ) -> DocId {
        self.build_renamed_specifier_doc(
            is_type_export,
            spec.export_kind == internal::ExportKind::Type,
            &spec.local,
            &spec.exported,
        )
    }

    /// Build a comma-separated list with group-based wrapping and comment splitting.
    /// Returns the inner doc to be wrapped with `{ indent_softline(...) softline }`.
    pub(super) fn build_softline_comma_list<T>(
        &self,
        items: &[T],
        brace_start: u32,
        brace_close: u32,
        get_span: impl Fn(&T) -> tsv_lang::Span,
        build_item_doc: impl Fn(&T) -> DocId,
    ) -> DocId {
        let d = self.d();
        let mut inner_parts = DocBuf::new();
        let mut prev_end = brace_start + 1; // After opening `{`
        // Block comment trailing the LAST item after its source comma — preserved past
        // where the comma was (no trailing comma; trailingComma: 'none') rather than
        // relocated before it (prettier relocates before; see conformance_prettier.md
        // §Comment relocation).
        let mut last_after_comma = DocBuf::new();

        for (i, item) in items.iter().enumerate() {
            let span = get_span(item);
            let item_start = span.start;
            let item_end = span.end;
            let is_last = i == items.len() - 1;

            let mut item_parts = DocBuf::new();

            // Leading block comments before this item (after prev comma or `{`)
            for comment in comments_in_range(self.comments, prev_end, item_start) {
                if comment.is_block {
                    item_parts.push(d.text_owned(format!("/*{}*/ ", comment.content(self.source))));
                }
            }

            item_parts.push(build_item_doc(item));

            if !is_last {
                let next_start = get_span(&items[i + 1]).start;
                let comma_pos = self.find_list_comma(item_end, next_start);
                self.append_trailing_inline_block_comments(&mut item_parts, item_end, comma_pos);
                prev_end = comma_pos + 1;
            } else {
                // Split the last item's trailing block comments around a source comma:
                // before-comma stay with the item; after-comma are preserved below, past
                // where the comma was (no trailing comma; trailingComma: 'none').
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

        // No trailing comma when the list breaks (trailingComma: 'none').
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
    pub(super) fn build_braced_hardline_comma_list<T>(
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
            None => (DocBuf::new(), None),
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
        let mut parts = DocBuf::new();
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
                // Last item: no trailing comma (trailingComma: 'none'). Same-line block
                // comments hug the item (`a /* c */`), same-line line comments follow
                // (`a // comment`), and own-line comments get hardlines (`a\n// comment`).
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
                            parts.push(self.build_trailing_line_comment_doc(comment));
                        }
                    } else {
                        if self.has_blank_line_between(prev_pos, comment.span.start) {
                            parts.push(d.literalline());
                        }
                        parts.push(d.hardline());
                        parts.push(self.build_comment_doc(comment));
                    }
                    prev_pos = comment.span.end;
                }
            }

            prev_end = item_end;
        }

        d.concat(&parts)
    }
}
