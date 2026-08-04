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
use tsv_lang::{Span, comments_to_emit_in_range};

/// The source offsets that bracket a braced specifier list — the window in which
/// `push_braced_specifier_list` locates the `{ … }`, and the wider one its
/// lone-specifier comment scan reads.
pub(super) struct SpecifierListSpans {
    /// Start of the declaration — the `import`/`export` keyword, where the
    /// lone-specifier comment window opens. Everything between it and `kw_end` is
    /// header keywords, so a comment there is the specifier's as far as prettier's
    /// attachment is concerned (see [`Printer::lone_specifier_has_comment`]).
    pub(super) header_start: u32,
    /// End of the keyword header, where the forward `{` search starts.
    pub(super) kw_end: u32,
    /// Upper bound for every scan past the specifiers — the closing `}`, and the
    /// `from` that ends the lone-specifier comment window (source-literal start,
    /// or the declaration end for a local `export {…};`).
    pub(super) bound: u32,
}

impl<'a> Printer<'a> {
    /// Check if an import declaration has empty named braces `{}` in source.
    /// This distinguishes `import {} from 'x'` from `import 'x'`.
    /// Also matches braces containing only whitespace and/or comments:
    /// `import { /* c */ } from 'x'`, `import { // c\n } from 'x'`.
    pub(super) fn has_empty_named_braces(&self, decl: &internal::ImportDeclaration<'_>) -> bool {
        // A surviving named specifier PROVES the braces are non-empty — decide
        // from the AST whenever the AST can answer, and only fall back to the
        // source scan for the specifier-less case it alone can settle (`import
        // {} from 'x'` vs `import 'x'`, which have the same AST). The scan reads
        // `decl.span`, so a declaration whose specifier list was rebuilt (the
        // Svelte compiler's type erasure filters out `import { type X, Y }`'s
        // type-only specifiers) would otherwise be judged against the *original*
        // source text, including the specifiers that no longer exist.
        if decl
            .specifiers
            .iter()
            .any(|spec| matches!(spec, internal::ImportSpecifier::Named(_)))
        {
            return false;
        }
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
        // so the default-binding / namespace comment scan starts after it. Derives
        // from `ImportPhase::as_str` (single source of truth) plus its trailing space.
        base + decl.phase.as_str().map_or(0, |kw| kw.len() as u32 + 1)
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
        self.d().group(self.braced_body(inner))
    }

    /// [`Self::braced_group`] with the group already broken, so the list expands
    /// regardless of width. The `objectWrap: 'preserve'` layout — an authored
    /// newline after the `{` — reached only by the import-attribute clause, which
    /// is the one braced module list prettier prints through `printObject`.
    pub(super) fn braced_group_break(&self, inner: DocId) -> DocId {
        self.d().group_break(self.braced_body(inner))
    }

    /// The braced body the two breakable wrappers share: `{`, the indented content,
    /// and a `line` before the `}` that renders as bracketSpacing when flat and as
    /// the closing newline when broken.
    fn braced_body(&self, inner: DocId) -> DocId {
        let d = self.d();
        d.concat(&[d.text("{"), d.indent_line(inner), d.line(), d.text("}")])
    }

    /// The unbreakable counterpart to [`Self::braced_group`]: the same
    /// bracketSpacing-padded `{ <inner> }`, but as plain text — no group, no
    /// `line`s — so the list can't expand at any width. Both of prettier's
    /// never-break module braces render through it: a lone specifier (the
    /// `can_break` note in [`Self::push_braced_specifier_list`]) and a lone
    /// `type` import attribute (prettier's `removeLines`; see
    /// `is_single_type_attribute`). Only reached when `inner` is itself line-free.
    pub(super) fn braced_flat(&self, inner: DocId) -> DocId {
        let d = self.d();
        d.concat(&[d.text("{ "), inner, d.text(" }")])
    }

    /// Finish a module statement: emit the `;` right after the content, then any
    /// comments between the last content token and the `;`, deferred to **after** it
    /// (prettier 3.9 — `} /* c */;` → `}; /* c */`).
    ///
    /// When `grouped`, `parts` is wrapped in a `group` for width-based wrapping and
    /// the trailing comments are emitted *outside* it, so a line-comment break can't
    /// expand the statement's specifier braces (import/export named declarations).
    /// Otherwise the comments are appended to `parts` — used by export-all and
    /// import-equals, which have no wrapping group.
    pub(super) fn finish_with_pre_semi(
        &self,
        mut parts: DocBuf,
        content_end: u32,
        decl_end: u32,
        grouped: bool,
    ) -> DocId {
        let d = self.d();
        let after = self.collect_post_semi_comments(content_end, decl_end);
        if !grouped {
            parts.push(d.text(";"));
            parts.extend(after);
            return d.concat(&parts);
        }
        // Wrap the content in a group for width-based wrapping; the `;` and trailing
        // comments stay outside it so an own-line/line comment can't expand the braces.
        let mut out = DocBuf::new();
        out.push(d.group(d.concat(&parts)));
        out.push(d.text(";"));
        out.extend(after);
        d.concat(&out)
    }

    /// The source offset of a closing `}` — the first `}` (outside comments, so a
    /// `}` inside a trailing comment is skipped) at or after `search_start`,
    /// bounded by `bound` (the fallback when the brace can't be located). Shared
    /// by the named-specifier brace scans (import/export) and the attribute clause.
    pub(super) fn close_brace_offset(&self, search_start: u32, bound: u32) -> u32 {
        self.find_char_outside_comments(search_start, bound, b'}')
            .unwrap_or(bound)
    }

    /// Whether prettier would see a comment ON a lone braced specifier — the
    /// `hasComment` arm of the `canBreak` rule `push_braced_specifier_list` mirrors.
    ///
    /// Its comment attachment reaches past the braces on both sides: every header-gap
    /// comment becomes the specifier's *leading* comment (`import /* c */ {a}` and
    /// `import /* c */ type {a}` alike → `import type {⏎ /* c */ a⏎}`) and a `}`→`from`
    /// comment its *trailing* one, so any of them restores breaking even though tsv
    /// preserves them outside the braces (the relocation divergences). So the window
    /// opens at the declaration start, not past the header: the `import`/`export`→`type`
    /// gap is a header gap like the rest, and skipping it would hold an over-width lone
    /// specifier on one line where prettier expands it.
    ///
    /// It stops at `from`, since a comment after it belongs to the source literal, and at
    /// the `}` when there is no `from` — the `}`→`;` gap of a local `export {…};` is not
    /// the specifier's (prettier attaches it to the declaration and floats it past the
    /// `;`, leaving `canBreak` false).
    ///
    /// The question is ON-PAGE, not to-emit: a glued block comment is owned by the
    /// specifier and printed from its own doc, but it still occupies the line, and
    /// prettier's `hasComment` counts it.
    fn lone_specifier_has_comment(&self, spans: &SpecifierListSpans, after_brace: u32) -> bool {
        // Cheap superset first (one binary search): with no comment anywhere from
        // the declaration to the source literal, none can be on the specifier — the
        // comment-free path every ordinary `import { a } from 'x'` takes.
        if !self.has_comments_on_page_between(spans.header_start, spans.bound) {
            return false;
        }
        let window_end = self
            .find_keyword_in_range(after_brace, spans.bound, "from")
            .unwrap_or(after_brace);
        self.has_comments_on_page_between(spans.header_start, window_end)
    }

    /// Render a braced, comma-separated specifier list (`{a, b as c}`) with
    /// comment-aware wrapping, push the doc onto `parts`, and return the offset
    /// just past the closing `}` — the caller's trailing-comment anchor.
    ///
    /// Shared by import and export named specifiers (which differed only in the
    /// item type and per-item doc builder). `kw_end` is the offset past the
    /// `import`/`export [type]` header, where the `{` search begins; `bound`
    /// caps the brace scans (the source-literal start, or the `;` for a local
    /// `export {…}`); `header_start` opens the lone-specifier comment window one
    /// step earlier, at the declaration keyword itself.
    ///
    /// `brace_follows_header` states that the `{` directly follows the header —
    /// always so for exports, and for imports only without a preceding
    /// default/namespace binding (prettier's `standaloneSpecifiers`). Two rules read
    /// that one fact: a comment in the keyword→`{` gap (`import /* c */ {a}`,
    /// `export type /* c */ {a}`) is preserved in place only when it holds (prettier
    /// relocates such a comment into the braces as the first specifier's leading
    /// comment — a comment-position divergence; with a binding the caller already
    /// emits that gap's comments as it builds `x, {…}`, so capturing here would
    /// double-emit them), and a lone specifier is unbreakable only when it holds
    /// (see `can_break` below).
    pub(super) fn push_braced_specifier_list<T>(
        &self,
        parts: &mut DocBuf,
        specifiers: &[T],
        spans: SpecifierListSpans,
        brace_follows_header: bool,
        get_span: impl Fn(&T) -> Span,
        build_item: impl Fn(&T) -> DocId,
    ) -> u32 {
        debug_assert!(
            !specifiers.is_empty(),
            "push_braced_specifier_list requires ≥1 specifier; empty `{{}}` is handled separately"
        );
        // Forward search from the header skips a `{` inside comments.
        let first_start = get_span(&specifiers[0]).start;
        let brace_start = self
            .find_char_outside_comments(spans.kw_end, first_start, b'{')
            .unwrap_or(0);

        let last_spec_end = get_span(&specifiers[specifiers.len() - 1]).end;
        let brace_close = self.close_brace_offset(last_spec_end, spans.bound);
        // Just past the closing `}` — the exclusive end of every brace-window scan
        // below, and the caller's trailing-comment anchor returned at the end.
        let after_brace = brace_close + 1;

        // Expanding comments (line comments, or own-line single-line block
        // comments) force the multiline path. One zero-comment window check over
        // the braces gates all three queries (each is bounded within the braces).
        let brace_span = Span::new(brace_start, after_brace);
        let has_expanding_comments = self.has_comments_to_emit_between(brace_start, after_brace)
            && (self.has_line_comments_in_delimited_list(specifiers, &get_span, brace_close)
                || self.has_line_comments_between(brace_start + 1, first_start)
                || self.has_own_line_block_comments_in_bracket_list(
                    brace_span, specifiers, &get_span,
                ));

        let braces_doc = if has_expanding_comments {
            // `first_start` keeps a same-line `{` comment on the brace line
            // (divergence from prettier, which relocates it as the first
            // specifier's leading comment).
            self.build_braced_hardline_comma_list(
                specifiers,
                brace_start,
                brace_close,
                first_start,
                &get_span,
                &build_item,
            )
        } else {
            // No expanding comments: group-based wrapping with comment splitting —
            // unless prettier's `canBreak` (its `printModuleSpecifiers`) is false. A
            // lone braced specifier never breaks there, so `import { a } from 'x'` /
            // `export { a as b } from 'x'` overflows print width rather than
            // expanding; a second specifier, a leading default/namespace binding, or
            // a comment on a specifier restores the group.
            let can_break = specifiers.len() > 1
                || !brace_follows_header
                || self.lone_specifier_has_comment(&spans, after_brace);
            let spec_doc = self.build_softline_comma_list(
                specifiers,
                brace_start,
                brace_close,
                &get_span,
                &build_item,
            );
            if can_break {
                self.braced_group(spec_doc)
            } else {
                self.braced_flat(spec_doc)
            }
        };

        // The keyword→`{` gap comment (`import /* c */ {a}`, `import type // c\n{a}`,
        // and the export forms) is preserved before the brace; prettier relocates it
        // into the braces. A line comment forces `{…}` onto a new line, which the
        // shared helper indents one level (statement continuation) — the leading
        // space comes from the caller's `import `/`export `/`type ` token. Captured
        // only when the `{` directly follows the header (see `brace_follows_header`).
        if brace_follows_header {
            parts.push(self.gap_comment_continuation_tail(spans.kw_end, brace_start, braces_doc));
        } else {
            parts.push(braces_doc);
        }
        after_brace
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
            // A rename (`{a as b}`): the `as`-binding continuation is shared with the
            // namespace `*`→`as` gap via `build_as_binding_continuation` — a *line*
            // comment in the `left`→`as` or `as`→binding gap stays where the author
            // wrote it and continues the tail one indent level (so a `//` can't swallow
            // the `as` or the renamed binding), while a block comment trails inline.
            // Prettier instead relocates before-`as` comments to lead the whole
            // specifier. See conformance_prettier.md §Uniform Forced-Continuation Indent
            // and §Comment relocation. A comment-free `{a as b}` skips the scan and emits
            // `a as b` unchanged.
            if !self.has_comments_to_emit_between(left_span.end, right_span.start) {
                parts.push(d.text(" as "));
                parts.push(self.build_module_export_name_doc(right));
            } else {
                parts.push(self.build_as_binding_continuation(left_span.end, right));
            }
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
            internal::ModuleExportName::Identifier(id) => self.identifier_name_doc(id),
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
        get_span: impl Fn(&T) -> Span,
        build_item_doc: impl Fn(&T) -> DocId,
    ) -> DocId {
        let d = self.d();

        // Zero-comment fast gate (see `build_params_doc_with_comments`): every
        // comment sub-query below — the per-item leading/trailing lookups, the
        // per-gap `find_list_comma` scans (whose results feed only comment
        // placement), and the last item's comma split — is bounded within the
        // braces, so with no comment there the list is plain items joined by
        // `,` + line. Tree-identical: the skipped singleton `concat`s collapse
        // to the item doc, and the skipped pushes are empty docs.
        if !self.has_comments_to_emit_between(brace_start, brace_close + 1) {
            let mut inner_parts = d.pooled_docbuf();
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    inner_parts.push(d.text(","));
                    inner_parts.push(d.line());
                }
                inner_parts.push(build_item_doc(item));
            }
            return d.concat(&inner_parts);
        }

        let mut inner_parts = DocBuf::new();
        // Where the first item's gap opens, per the delimited-list anchor convention.
        let list_start = brace_start + 1;
        let mut prev_end = list_start;
        // The Rule A gap anchors, in the shared closure form `list_item_frozen` takes.
        let item_span = |j: usize| get_span(&items[j]);
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
            for comment in comments_to_emit_in_range(self.comments, prev_end, item_start) {
                if comment.is_block {
                    // One text node (`/*content*/ `; full span = the verbatim
                    // comment, delimiters included), like the lists.rs twins.
                    let mut w = d.pool_writer();
                    w.push_str(comment.span.extract(self.source));
                    w.push(' ');
                    let doc = w.finish_text();
                    // A comment emission that can't route through `build_comment_doc`
                    // (the trailing space must share the node), so it tags its own
                    // ledger node.
                    #[cfg(feature = "comment_check")]
                    d.tag_comment_doc(doc, comment.span, self.source);
                    item_parts.push(doc);
                }
            }

            item_parts
                .push(self.build_span_item_doc(list_start, &item_span, i, || build_item_doc(item)));

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
    /// A same-line `{` comment (`import { // c`, `with { // c`) is kept on the
    /// brace line — the open-brace divergence, shared by the import/export
    /// specifier brace and the `with {…}` import-attribute brace. `first_item_start`
    /// bounds the `{`→first-item gap the brace-line pull scans. See
    /// conformance_prettier.md §Comment relocation.
    pub(super) fn build_braced_hardline_comma_list<T>(
        &self,
        items: &[T],
        brace_start: u32,
        end_boundary: u32,
        first_item_start: u32,
        get_span: impl Fn(&T) -> Span,
        build_item_doc: impl Fn(&T) -> DocId,
    ) -> DocId {
        let d = self.d();
        let (brace_line_prefix, delimiter_pull_pos) =
            self.delimiter_line_comment_prefix(brace_start, first_item_start);
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
        get_span: impl Fn(&T) -> Span,
        build_item_doc: impl Fn(&T) -> DocId,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        // Where the first item's gap opens, per the delimited-list anchor convention.
        let list_start = brace_start + 1;
        let mut prev_end: u32 = list_start;
        // The Rule A gap anchors, in the shared closure form `list_item_frozen` takes.
        let item_span = |j: usize| get_span(&items[j]);

        for (i, item) in items.iter().enumerate() {
            let span = get_span(item);
            let item_start = span.start;
            let is_first = i == 0;
            let is_last = i == items.len() - 1;

            // The rest of the gap, resuming where the previous item's trailing run stopped
            // — the element-comma partition (see `collect_item_leading_comments`).
            let comments = self.collect_item_leading_comments(
                prev_end,
                item_start,
                is_first.then_some(delimiter_pull_pos).flatten(),
            );

            if !is_first {
                let check_pos = if comments.is_empty() {
                    item_start
                } else {
                    comments[0].span.start
                };
                self.push_next_line_empty_hardline(&mut parts, prev_end, check_pos);
            }

            for comment in &comments {
                parts.push(self.build_comment_doc(comment));
                if self.comment_hugs_next(comment, item_start) {
                    parts.push(d.text(" "));
                } else {
                    parts.push(d.hardline());
                }
            }

            parts
                .push(self.build_span_item_doc(list_start, &item_span, i, || build_item_doc(item)));

            // Comma with comment-boundary splitting — the shared element-comma contract
            // (`collect_trailing_comments` / `push_element_comma_trailing`), the same one
            // the object-literal and destructuring-pattern element loops use, so the
            // partition between this item's trailing run and the next item's leading run
            // is decided in one place for all of them.
            let item_end = span.end;
            if !is_last {
                let next_start = get_span(&items[i + 1]).start;
                let trailing = self.collect_trailing_comments(item_end, next_start, false);
                self.push_element_comma_trailing(&mut parts, &trailing, d.text(","));
                prev_end = trailing.end_pos;
            } else {
                // Last item: no trailing comma (trailingComma: 'none'). Same-line block
                // comments hug the item (`a /* c */`), same-line line comments follow
                // (`a // comment`), and own-line comments get hardlines (`a\n// comment`).
                let mut prev_pos = item_end;
                // Track line reference for multi-line block comments
                let mut line_ref = item_end;
                for comment in comments_to_emit_in_range(self.comments, item_end, end_boundary) {
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
        }

        d.concat(&parts)
    }
}
