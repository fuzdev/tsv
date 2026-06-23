// Header-gap comment continuation helpers for module statements: the
// keyword/binding gaps prettier relocates but tsv preserves in place, the
// `from`/source rendering, and namespace `as` bindings.

use super::Printer;
use crate::ast::internal;
use tsv_lang::SymbolToU32;
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
    /// Emit the ` as <binding>` tail of a namespace binding (`* as ns`), starting
    /// just past the `*`. Preserves a comment in the `*`→`as` gap and the `as`→binding
    /// gap in place. `star_end` is the position just past `*`; `binding` is the
    /// namespace identifier (`exported` for a re-export, `local` for an import).
    ///
    /// Both gaps route through the shared header-gap helper, so a *line* comment in
    /// either indents its continuation one level and a block comment trails inline.
    /// In the `*`→`as` gap that matches prettier's freedom (it relocates the comment
    /// after `as`). In the `as`→binding gap it is a deliberate indent-only divergence:
    /// prettier keeps the comment in place but flattens the binding (`* as // c\nns`),
    /// while tsv indents it for uniformity with the sibling gaps — and the forced
    /// hardline also avoids pulling the binding onto the comment line. See
    /// conformance_prettier.md §Comment relocation.
    pub(super) fn append_namespace_as_binding(
        &self,
        parts: &mut Vec<DocId>,
        star_end: u32,
        binding: &internal::Identifier,
    ) {
        let d = self.d();
        let as_pos = self.find_keyword_in_range(star_end, binding.span.start, "as");
        // `as` + the `as`→binding gap (line comment indents the binding, block trails
        // inline) + the binding name. The `as ` token supplies the leading space.
        let as_end = as_pos.map_or(binding.span.start, |p| p + "as".len() as u32);
        let as_clause = d.concat(&[
            d.text("as "),
            self.gap_comment_continuation_tail(
                as_end,
                binding.span.start,
                d.symbol(binding.name.to_u32()),
            ),
        ]);
        // `*`→`as` gap, preserved in place; the leading space comes from the helper
        // (the preceding `*` has no trailing space).
        let gap_end = as_pos.unwrap_or(binding.span.start);
        parts.push(self.gap_comment_indented_continuation(star_end, gap_end, as_clause));
    }

    /// The comment-and-continuation tail of a preserved header gap, *without* a
    /// leading space — for callers whose preceding token already ends in a space
    /// (`import `, `export `, `type `). A *line* comment in `[start, end)` ends
    /// with a hardline, so the continuation is wrapped in `indent` to read as a
    /// statement continuation rather than a second statement; a block comment
    /// trails inline (` /* c */ `); an empty range yields the continuation
    /// unchanged. The forced hardline is the only thing the `indent` shifts —
    /// the comment itself stays on the preceding token's line.
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
        match self.build_rhs_comments_opt(start, end) {
            // Line comment: it ends with a hardline, so indent the continuation.
            Some(c) if self.has_line_comments_between(start, end) => {
                d.indent(d.concat(&[c, continuation]))
            }
            // Block comment: trails inline (its own trailing space), no break.
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
    /// source→`with`, `with`→`{`, and `*`→`as`. See conformance_prettier.md
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
