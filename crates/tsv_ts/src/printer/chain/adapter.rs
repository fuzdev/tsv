// Chain helpers on the main Printer for the chain renderer.
//
// These let the chain builder/printer delegate back to the main Printer for
// expression building and comment handling. Formerly the `ChainPrinter` /
// `SymbolLookup` traits — collapsed to inherent methods, since `Printer` was
// their only implementor.

use crate::ast::internal;
use crate::printer::comments::CommentVec;
use crate::printer::{CommentSpacing, Printer, comments_to_emit_in_range};
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::{ClassifiedComments, Comment, Span};

impl<'a> Printer<'a> {
    pub(crate) fn arena(&self) -> &DocArena {
        self.arena
    }

    pub(crate) fn ident_doc(&self, name: internal::IdentName<'_>, name_start: u32) -> DocId {
        self.ident_name_doc(name, name_start)
    }

    pub(crate) fn in_for_init(&self) -> bool {
        self.in_for_init.get()
    }

    pub(crate) fn print_call_args(
        &self,
        call: &internal::CallExpression<'_>,
        optional: bool,
    ) -> DocId {
        self.build_call_args_doc_for_chain(call, optional)
    }

    pub(crate) fn print_call_args_expanded(
        &self,
        call: &internal::CallExpression<'_>,
        optional: bool,
    ) -> DocId {
        self.build_call_args_doc_for_chain_expanded(call, optional)
    }

    pub(crate) fn print_call_args_standard_expanded(
        &self,
        call: &internal::CallExpression<'_>,
        optional: bool,
    ) -> DocId {
        self.build_call_args_doc_for_chain_standard_expanded(call, optional)
    }

    pub(crate) fn build_chain_block_comments_doc(
        &self,
        start: u32,
        end: u32,
        spacing: CommentSpacing,
    ) -> DocId {
        let block_comments: CommentVec<'_> = comments_to_emit_in_range(self.comments, start, end)
            .filter(|c| c.is_block)
            .collect();
        self.format_block_comments(&block_comments, spacing)
    }

    pub(crate) fn build_computed_member_line_comment_bracket(
        &self,
        open: &'static str,
        inside_start: u32,
        prop_start: u32,
        prop_end: u32,
        bracket_end: u32,
        inner: DocId,
    ) -> Option<DocId> {
        // Only the break path — a line comment before the index or after it (before
        // `]`). A block-only or comment-free bracket falls through to the caller.
        if !self.has_line_comments_between(inside_start, prop_start)
            && !self.has_line_comments_between(prop_end, bracket_end)
        {
            return None;
        }
        let d = self.d();
        // Build the body (index + any index→`]` trailing comments), then hand it to the
        // shared bracket-break helper (it owns the `[`→index line-comment prefix and the
        // break shell, mirroring the computed-key bracket). `[`→index: a `[`-line comment
        // is pulled onto the `[` line, an own-line one keeps its line (blank-preserving).
        // index→`]`: a same-line comment trails the index, an own-line one keeps its line.
        let mut body_parts = DocBuf::new();
        body_parts.push(inner);
        let mut prev = prop_end;
        for comment in comments_to_emit_in_range(self.comments, prop_end, bracket_end) {
            if self.is_same_line(prev, comment.span.start) {
                body_parts.push(d.text(" "));
            } else {
                body_parts.push(d.hardline());
            }
            body_parts.push(self.build_comment_doc(comment));
            prev = comment.span.end;
        }
        // The `[` is the char just before the index region (past `?.` for `?.[`).
        Some(self.build_bracket_line_comment_break(
            open,
            inside_start - 1,
            prop_start,
            d.concat(&body_parts),
        ))
    }

    pub(crate) fn get_property_span(&self, expr: &internal::Expression<'_>) -> Span {
        expr.span()
    }

    pub(crate) fn is_expression_statement(&self) -> bool {
        self.is_expression_statement.get()
    }

    pub(crate) fn clear_expression_statement(&self) {
        self.is_expression_statement.set(false);
    }

    pub(crate) fn get_layout_line_breaks(&self) -> &[u32] {
        self.layout_line_breaks
    }

    pub(crate) fn chain_has_comments(&self) -> bool {
        self.chain_has_comments.get()
    }

    pub(crate) fn set_chain_has_comments(&self, has_comments: bool) -> bool {
        let prev = self.chain_has_comments.get();
        self.chain_has_comments.set(has_comments);
        prev
    }

    pub(crate) fn restore_chain_has_comments(&self, prev: bool) {
        self.chain_has_comments.set(prev);
    }

    /// The chain gaps' one classification seam — every chain consumer (the builder's
    /// gap ownership, the dot/computed print paths, `push_gap_comments_and_break`)
    /// classifies through here, so they take the ADVANCING trailing anchor together
    /// and the gap's two sides cannot drift into a double-print
    /// (`ClassifiedComments::from_range_advancing` — see its ⚠️ for why the advance
    /// is not `from_range`'s one behavior).
    pub(crate) fn classify_comments(&self, start: u32, end: u32) -> ClassifiedComments<'_> {
        ClassifiedComments::from_range_advancing(
            self.comments,
            start,
            end,
            self.comment_line_breaks,
        )
    }

    /// A gap's same-line block run, each comment behind a space
    /// (`method() /* c */`). The named entry for
    /// [`format_block_comments`](Self::format_block_comments) at `Leading` spacing,
    /// so the spacing rule lives once.
    pub(crate) fn build_trailing_block_doc(&self, comments: &[&Comment]) -> DocId {
        self.format_block_comments(comments, CommentSpacing::Leading)
    }

    /// A chain gap's trailing line comments, deferred via `line_suffix` — the run
    /// rides to the end of the output line and is flushed by whatever break the
    /// caller emits next.
    ///
    /// **The caller must emit that break.** A `line_suffix_boundary` here would flush
    /// the run *and end the line itself* (see `tsv_lang`'s `render_line_node`), so a
    /// caller that also breaks — every chain gap does, `build_chain_line_break` being
    /// a hardline at minimum — would render a blank line under the comment. Prettier
    /// plants its boundary at a member lookup whose following `softline` may collapse
    /// (`print/member.js`); a chain gap is the case where it may not.
    pub(crate) fn build_deferred_line_comments_doc(&self, comments: &[&Comment]) -> DocId {
        let d = self.d();
        if comments.is_empty() {
            return d.empty();
        }

        let mut parts = DocBuf::with_capacity(comments.len());
        for comment in comments {
            parts.push(self.build_trailing_line_comment_doc(comment));
        }
        d.concat(&parts)
    }

    pub(crate) fn get_source(&self) -> &str {
        self.source
    }
}

impl<'a> Printer<'a> {
    /// Format an already-filtered slice of **block** comments, each with `spacing`
    /// applied to its outer edges (` /* c */` after a chain element, `/* c */ ` before
    /// one). Every comment gets the space — a chain gap's block run is never at line
    /// start — so this is the plain-`true` consumer of
    /// [`Printer::push_block_comment_spaced`], which owns what the spacing means.
    pub(crate) fn format_block_comments(
        &self,
        block_comments: &[&Comment],
        spacing: CommentSpacing,
    ) -> DocId {
        let d = self.d();
        if block_comments.is_empty() {
            return d.empty();
        }

        let mut parts = DocBuf::new();
        for comment in block_comments {
            self.push_block_comment_spaced(&mut parts, comment, spacing, true);
        }
        d.concat(&parts)
    }
}
