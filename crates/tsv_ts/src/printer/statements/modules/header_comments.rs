// Header-gap comment continuation helpers for module statements: the
// keyword/binding gaps prettier relocates but tsv preserves in place, the
// `from`/source rendering, and namespace `as` bindings.

use super::Printer;
use crate::ast::internal;
use crate::printer::comments::CommentSpacing;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// Check if a string contains only whitespace and/or comments.
/// Used to detect empty braces that may contain comments: `{ /* c */ }`.
pub(super) fn is_only_whitespace_and_comments(text: &str) -> bool {
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
    /// Where the gap between a leading binding and the clause after it splits around
    /// their separating `,`: comments before the returned offset are emitted **before**
    /// the comma, comments at or after it **past** it.
    ///
    /// The comma separating a default/namespace binding from what follows
    /// (`import a, {b}` / `import a, * as ns`) is a **pure separator**, so neither side
    /// of it carries authorship signal and a *line* comment authored on either side
    /// trails past it — the carve-out the braced specifier list already takes between
    /// its own elements (`a // c⏎, b` → `a, // c`), which is what makes the two
    /// authorings reach one fixed point. A `//` has no representable position before
    /// the comma anyway: emitting it there runs it over the comma and everything after
    /// (the clause, `from`, the source, the `;`) — lost CODE whose output does not
    /// reparse, which is what this split replaced.
    ///
    /// `scan_end` carries the caller's *block* policy, which is the one thing the two
    /// arms do not share: prettier hoists a block **before** the comma for a named list
    /// (`import a, /* c */ {b}` → `import a /* c */, {b}`) but leaves it on its
    /// authored side for a namespace (`import a, /* c */ * as ns` stays), and tsv
    /// matches both. So the named-list caller scans to the `{` — every block is then
    /// ahead of the split and hoists — while the namespace caller scans only to the
    /// comma, leaving the blocks past it where the author wrote them. A block ahead of
    /// the `//` keeps its place in the run either way rather than reordering across it,
    /// which is why this is a split point and not a per-comment filter.
    pub(super) fn binding_separator_split(&self, binding_end: u32, scan_end: u32) -> u32 {
        comments_to_emit_in_range(self.comments, binding_end, scan_end)
            .find(|c| !c.is_block)
            .map_or(scan_end, |c| c.span.start)
    }

    /// Emit a namespace `* [as <binding>]` clause preceded by the gap comments in
    /// `[gap_start, star_pos)` — the one emitter for every namespace star in the module
    /// statements: the import forms (`import * as ns`, and `import a, * as ns`, whose
    /// gap opens past the separating `,` instead of at the header) and the export-all
    /// forms (`export * from`, `export * as ns from`, with or without `type`), which
    /// differ only in where that gap starts and whether a binding follows.
    ///
    /// ⚠️ **Only the `*` rides [`Self::gap_comment_continuation_tail`]; the binding
    /// follows OUTSIDE it.** A line comment in the gap carries the `*` onto the
    /// indented continuation line and the binding lands on that same line, so a second
    /// comment one gap further in (`* as // c⏎ns`) takes the same ONE level — the
    /// module headers' "never a staircase" rule. Folding the binding into the
    /// continuation argument instead reads as the obvious simplification and is a bug:
    /// [`Self::build_as_binding_continuation`] is *itself* an `indent`, so nesting it
    /// composes the two levels. Single-gap output is byte-identical either way, which
    /// is why the whole fixture suite is blind to it — hence one emitter rather than a
    /// rule restated per site. See conformance_prettier.md §Uniform Forced-Continuation
    /// Indent.
    ///
    /// `binding` is the namespace name — `exported` for a re-export (which may be a
    /// string, `export * as 'str' from`) or `local` for an import (always an
    /// identifier) — and `None` for a bare `export * from`. Its own `*`→`as` and
    /// `as`→binding gaps are the shared continuation helper's; in the `*`→`as` gap
    /// preserving matches prettier's freedom (it relocates the comment after `as`), in
    /// the `as`→binding gap it is a deliberate indent-only divergence (prettier keeps
    /// the comment in place but flattens the binding, `* as // c\nns`). See
    /// conformance_prettier_ts_comments.md §Comment relocation.
    pub(super) fn push_namespace_star_binding(
        &self,
        parts: &mut DocBuf,
        gap_start: u32,
        star_pos: u32,
        binding: Option<&internal::ModuleExportName<'_>>,
    ) {
        let d = self.d();
        parts.push(self.gap_comment_continuation_tail(gap_start, star_pos, d.text("*")));
        if let Some(binding) = binding {
            parts.push(self.build_as_binding_continuation(star_pos + 1, binding));
        }
    }

    /// Build the ` as <binding>` continuation shared by the namespace `*`→`as` binding
    /// ([`Self::push_namespace_star_binding`]) and the renamed named-specifier `a as b`
    /// rename ([`Self::build_renamed_specifier_doc`]) — the two differ only in what
    /// precedes `as` (a `*`, or the `imported`/`local` name), so both route their
    /// `as`-gap comments here. Starting just past `left_end`, locate `as`, then build
    /// ` as ` + the `as`→binding gap continuation: a *line* comment in the `left`→`as`
    /// gap or the `as`→binding gap stays where the author wrote it and drops its tail
    /// one indent level (so a `//` can't swallow the `as` or the binding), while a block
    /// comment trails inline. Both gaps route through the same preserve-in-place
    /// header-gap helpers the rest of the module header uses. See conformance_prettier.md
    /// §Uniform Forced-Continuation Indent and §Comment relocation.
    pub(super) fn build_as_binding_continuation(
        &self,
        left_end: u32,
        binding: &internal::ModuleExportName<'_>,
    ) -> DocId {
        let d = self.d();
        let binding_start = binding.span().start;
        let as_pos = self.find_keyword_in_range(left_end, binding_start, "as");
        // `as ` + the `as`→binding gap (line comment indents the binding, block trails
        // inline) + the binding name. The `as ` token supplies the trailing space.
        let as_end = as_pos.map_or(binding_start, |p| p + "as".len() as u32);
        let as_clause = d.concat(&[
            d.text("as "),
            self.gap_comment_continuation_tail(
                as_end,
                binding_start,
                self.build_module_export_name_doc(binding),
            ),
        ]);
        // `left`→`as` gap, preserved in place; the leading space comes from the helper
        // (the preceding `*`/name has no trailing space).
        let gap_end = as_pos.unwrap_or(binding_start);
        self.gap_comment_indented_continuation(left_end, gap_end, as_clause)
    }

    /// The comment-and-continuation tail of a preserved header gap, *without* a
    /// leading space — for callers whose preceding token already ends in a space
    /// (`import `, `export `, `type `). A *line* comment in `[start, end)` ends
    /// with a hardline, so the continuation is wrapped in `indent` to read as a
    /// statement continuation rather than a second statement; a *multiline* block
    /// keeps the author's layout; a *single-line* block trails inline
    /// (` /* c */ `) from **any** authored position — glued, trailing the head, or
    /// on its own line — because nothing forces it off the line, so the author's
    /// break is ordinary layout and is reflowed. An empty range yields the
    /// continuation unchanged. The forced hardline is the only thing the `indent`
    /// shifts — the comment itself stays on the preceding token's line.
    ///
    /// Used for the keyword→`{`, `type`→`{`, keyword→`type`, `*`→`as`, and
    /// keyword→empty-`{}` header gaps (whose continuation rides the space already
    /// emitted by `import `/`export `/`type `/`*`). [`Self::gap_comment_indented_continuation`]
    /// wraps this with a leading space for the gaps whose preceding token has none.
    pub(super) fn gap_comment_continuation_tail(
        &self,
        start: u32,
        end: u32,
        continuation: DocId,
    ) -> DocId {
        let d = self.d();
        // An HONORED format-ignore directive keeps the line the author gave it, in both
        // spellings: pulled up flush against the header keyword it would share that line,
        // which the placement floor classifies as inert, so the freeze it earns would be lost
        // on the second pass. Routing the whole run through the declaration headers' emitter
        // gets that rule (and only that rule — every other comment keeps the flush-first
        // layout) from one place instead of a second spelling of it here.
        if self.member_gap_frozen(start, end) {
            let run = self.build_header_comment_run(start, end, CommentSpacing::Leading, true);
            return d.indent(d.concat(&[run, continuation]));
        }
        // Line comment: it ends with a hardline, so indent the continuation.
        if self.has_line_comments_between(start, end) {
            return match self.build_rhs_comments_opt(start, end) {
                Some(c) => d.indent(d.concat(&[c, continuation])),
                None => continuation,
            };
        }
        // A multiline block breaks here rather than staying inline — this gap's
        // deliberate difference from its `build_keyword_to_name_continuation` twin.
        // The author broke after it and reflowing would swallow the `*/` line into
        // the header, so the break is forced; the continuation is therefore indented
        // one level, exactly as the line-comment branch above does and as every value
        // gap does for a multiline block (`const x =⏎\t/* x⏎y */⏎\t1;`). See
        // conformance_prettier.md §Uniform Forced-Continuation Indent. The comment's
        // own interior lines stay flush — a comment body is never re-indented.
        if self.has_multiline_block_comments_on_page_between(start, end) {
            return match self.build_rhs_comments_opt(start, end) {
                Some(c) => d.indent(d.concat(&[c, continuation])),
                None => continuation,
            };
        }
        // A single-line block, in ANY authored position — glued, trailing the head,
        // or on its own line. Nothing forces it off the line, so it trails inline and
        // the author's break is reflowed: the shared keyword→value rule
        // (`comment_hangs_next`), which a module header gap follows like its
        // `export default` / `export =` siblings. See conformance_prettier.md
        // §Authored breaks in value position.
        //
        // ⚠️ Emitting this through `build_rhs_comments_opt` instead reads as the
        // obvious code and is the bug it replaced: that builder picks each separator
        // from the comment's AUTHORED position, so an own-line comment kept a
        // hardline while the concat glued it to the head token. The result — comment
        // pulled up, break kept — is the glued authoring, which reflows inline on the
        // next pass, so the format was not idempotent on its own output.
        match self.build_inline_comments_between_doc_trailing_space_opt(start, end) {
            Some(c) => d.concat(&[c, continuation]),
            None => continuation,
        }
    }

    /// Build the doc for a header-gap comment in `[start, end)` followed by
    /// `continuation`, preserving the comment where the user placed it, with a
    /// leading space before the comment/continuation.
    ///
    /// A *line* comment forces `continuation` onto a new line; tsv indents that
    /// continuation one level — a single statement spanning lines reads as a
    /// continuation, not a second statement. A block comment trails inline
    /// (` /* c */ `); an empty range emits just a leading space. The leading space
    /// and the comment stay on the preceding token's line — `indent` applies only
    /// at the forced hardline within the returned group. Used for the import/export
    /// header gaps prettier relocates but tsv preserves where the preceding token
    /// has no trailing space: binding/specifiers→`from`, `from`→source,
    /// source→`with`, `with`→`{`, and `*`→`as`. See conformance_prettier_ts_comments.md
    /// §Comment relocation and §"Import attributes header comments".
    ///
    /// Module-side twin of [`Self::build_keyword_to_name_continuation`] (comments.rs):
    /// same leading-space + indent-on-line-comment shape, but the two use different
    /// comment emitters (this one via [`Self::gap_comment_continuation_tail`] →
    /// `build_rhs_comments_opt`), so a multi-line block comment breaks here but stays
    /// inline there. Intentionally separate — don't merge.
    pub(super) fn gap_comment_indented_continuation(
        &self,
        start: u32,
        end: u32,
        continuation: DocId,
    ) -> DocId {
        let d = self.d();
        d.concat(&[
            d.text(" "),
            self.gap_comment_continuation_tail(start, end, continuation),
        ])
    }

    /// Build ` from [comments] ` followed by source literal.
    ///
    /// Handles comments between `from` keyword and source literal, and optionally
    /// captures comments from inside empty braces (relocated after `from` by prettier).
    pub(super) fn build_from_source_doc(
        &self,
        decl_start: u32,
        source: &internal::Literal<'_>,
        empty_brace_search_start: Option<u32>,
        content_end: Option<u32>,
    ) -> DocId {
        let d = self.d();
        #[allow(clippy::expect_used)] // "from" must exist in a valid import/export declaration
        let from_end = self
            .find_keyword_end("from", decl_start, source.span.start)
            .expect("'from' keyword must exist in import/export declaration");
        let from_start = from_end - "from".len() as u32;

        let comment_search_start = if let Some(search_start) = empty_brace_search_start {
            // Include comments from inside empty braces (relocated after "from").
            // Locate `{` outside comments so a `{` glyph in a comment isn't mistaken for it.
            self.find_char_outside_comments(search_start, from_end, b'{')
                .map_or(from_end, |p| p + 1)
        } else {
            from_end
        };

        // `from` + the `from`→source gap (comments incl. those relocated out of empty
        // braces), preserved in place. A line comment indents the source one level
        // (statement continuation); prettier's relocation varies by binding shape
        // (flat for empty/bare/export-all, floats past `;` for a default/namespace
        // binding, into the braces for named specifiers), so this is a deliberate
        // indent-only divergence uniform with the other header gaps. The leading space
        // comes from the helper.
        let from_clause = d.concat(&[
            d.text("from"),
            self.gap_comment_indented_continuation(
                comment_search_start,
                source.span.start,
                self.build_literal_doc(source),
            ),
        ]);

        // Binding/specifiers (or export-all `*`/`as ns`) → `from`: prettier *relocates*
        // a comment here (floats a line past `;`, or into named braces — a divergence,
        // from_comment_prettier_divergence), so tsv keeps it in place and indents the
        // `from …` continuation when a line comment forces the break. `content_end` is
        // the end of the last binding/specifier/`*` (None to skip — empty braces, import
        // or re-export, which relocate after `from` — emitted as an empty range so only
        // ` from …` is produced).
        self.gap_comment_indented_continuation(
            content_end.unwrap_or(from_start),
            from_start,
            from_clause,
        )
    }
}
