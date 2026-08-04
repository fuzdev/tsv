// Trailing comments around a list element's separator comma.
//
// The single source of the `trailingComma: 'none'` comment-position contract for
// element lists: block comments before the comma stay before it, a block comment
// after the comma is preserved after it — on the last element (whose comma isn't
// emitted) and ahead of a same-line line comment (`after_comma`, below) — and line
// comments go after the comma via `line_suffix` (zero width). Prettier relocates
// every one of those blocks before the comma; see conformance_prettier.md §Comment
// relocation. Shared by the object-literal, object/array destructuring-pattern,
// import/export specifier and enum-member loops so the ordering can't drift
// between them. (The array literal answers the same rule through its own paired
// trailing/leading predicate — holes and the fill path don't fit this collector's
// shape — so a change here has to be mirrored there.)
//
// This side is half of a partition: what it does NOT claim leads the next element,
// and every caller resumes its own leading scan at `end_pos`. See
// docs/comments.md §The element-comma seam for the two rules that keeps honest.
//
// The comma is located with `find_comma_in_range` (comment/string-skipping,
// bounded by the element's upper boundary), so a comma inside an earlier comment
// (`a /* , */ /* x */, b`) is never mistaken for the separator and the following
// comment is not relocated across it.

use super::{CommentVec, Printer};
use smallvec::SmallVec;
use tsv_lang::Comment;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// Trailing comments collected for a list element (property or array element)
pub(in crate::printer) struct TrailingComments<'a> {
    /// Block comments emitted in source order, before the emitted comma. A last
    /// element's after-comma block is included here too: with no trailing comma
    /// emitted (trailingComma: 'none') the last comma is `d.empty()`, so before- and
    /// after-comma blocks both trail the element in one run (prettier relocates an
    /// after-comma block before the comma; see conformance_prettier.md).
    block: SmallVec<[&'a Comment; 2]>,
    /// Block comments the author wrote **after** the comma that stay with this element
    /// rather than leading the next one: the ones a same-line line comment follows
    /// (`a, /* c1 */ // c2`). The line comment defers through `line_suffix`, so a block
    /// left to lead the next element would render *after* it — the pair comes out
    /// reordered, and in the pattern builders (which resume their leading scan past this
    /// run) it was skipped by both emitters and DROPPED outright. Emitted between the
    /// comma and the line suffix, which is exactly where it was written. Same rule, same
    /// reason as `Printer::push_trailing_comments_in_range`'s deferred-run block.
    after_comma: SmallVec<[&'a Comment; 2]>,
    /// Line comments that go after the comma (in line_suffix)
    line: SmallVec<[&'a Comment; 2]>,
    /// Position after all trailing comments (for updating prev_end)
    pub(in crate::printer) end_pos: u32,
}

impl<'a> Printer<'a> {
    /// Collect trailing comments for a list element (property or array element)
    ///
    /// Trailing comments are same-line comments after the element:
    /// - Block comments: BEFORE the comma, or after it when a line comment follows
    ///   (see [`TrailingComments::after_comma`])
    /// - Line comments: always belong to this element (they consume the rest of the line)
    ///
    /// The claimed run is always a **prefix** of the gap's comments, so a caller resumes
    /// its own scan at [`TrailingComments::end_pos`] and sees everything left over: an
    /// after-comma block that leads the next element sits behind no claimed comment (only
    /// a line comment could follow it on this line, and that claims the block too).
    pub(in crate::printer) fn collect_trailing_comments(
        &self,
        elem_end: u32,
        upper_bound: u32,
        is_last: bool,
    ) -> TrailingComments<'_> {
        // Zero-comment fast gate: the comma position only classifies comments, so
        // with no comment in the window there is nothing to collect — skip the
        // comma scan entirely.
        if !self.has_comments_to_emit_between(elem_end, upper_bound) {
            return TrailingComments {
                block: SmallVec::new(),
                after_comma: SmallVec::new(),
                line: SmallVec::new(),
                end_pos: elem_end,
            };
        }

        // Find the separator comma in source (if any), skipping commas that sit
        // inside comments or strings so `a /* , */ /* x */, b` is split on the
        // real comma, not the one in `/* , */`. The scan is bounded by
        // `upper_bound` (the old unbounded-then-filter form scanned to the next
        // comma anywhere in the rest of the source).
        let comma_pos = self.find_comma_in_range(elem_end, upper_bound);

        // The element's own trailing LINE of comments, in source order. `line_ref` follows
        // a multi-line block to its closing line, the same walk
        // [`Self::find_end_with_trailing_comments`] makes: what the author wrote after
        // `*/` is still trailing this element (`{ a: 1 } /* c⏎c2 */ // t`), and prettier
        // agrees. Keying on `elem_end` alone instead cuts the run at the block, leaving
        // the rest to a caller that resumes past it — where nothing prints it.
        let mut line_ref = elem_end;
        let mut same_line: CommentVec<'_> = CommentVec::new();
        for c in comments_to_emit_in_range(self.comments, elem_end, upper_bound) {
            if !self.is_same_line(line_ref, c.span.start) {
                break;
            }
            if c.is_block && !self.is_same_line(c.span.start, c.span.end) {
                line_ref = c.span.end;
            }
            same_line.push(c);
        }

        // A same-line LINE comment makes the rest of the run deferred, which is what
        // binds an after-comma block to this element rather than to the next one.
        let line_comment_start = same_line.iter().find(|c| !c.is_block).map(|c| c.span.start);

        // A block comment after the comma normally belongs to the next element as
        // leading — except on the LAST element, where it is preserved after the comma
        // (prettier relocates it before — see conformance_prettier.md §Comment
        // relocation). With no trailing comma emitted, a last element's after-comma
        // block trails the element in the same run as its before-comma blocks, so all
        // same-line blocks collect into one source-ordered `block` (the comma between
        // them is `d.empty()`).
        let before_comma =
            |c: &Comment| is_last || comma_pos.is_none_or(|comma| c.span.start < comma);

        let mut block = SmallVec::new();
        let mut after_comma = SmallVec::new();
        let mut line = SmallVec::new();
        let mut end_pos = elem_end;
        for c in same_line {
            if !c.is_block {
                line.push(c);
            } else if before_comma(c) {
                block.push(c);
            } else if line_comment_start.is_some_and(|start| c.span.start < start) {
                after_comma.push(c);
            } else {
                // Leads the next element — not this element's to print, and nothing
                // claimed after it (see the prefix note above).
                break;
            }
            end_pos = c.span.end;
        }

        TrailingComments {
            block,
            after_comma,
            line,
            end_pos,
        }
    }

    /// Build docs for block comments (go before comma)
    fn build_block_comments_doc(&self, comments: &[&Comment]) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        for comment in comments {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }
        d.concat(&parts)
    }

    /// Build docs for line comments (go after comma, excluded from width)
    fn build_line_comments_suffix_doc(&self, comments: &[&Comment]) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        for comment in comments {
            parts.push(self.build_trailing_line_comment_doc(comment));
        }
        d.concat(&parts)
    }

    /// Push one element's trailing comments around its `comma` doc, in the order
    /// that preserves comment position: same-line block comments (source-ordered,
    /// including a last element's after-comma block since its comma is `d.empty()`),
    /// the comma, the after-comma blocks the author wrote there, then line comments as a
    /// suffix. That is source order for every arrangement of the run, which is the point:
    /// the pieces land on the same side of the comma and in the same sequence the author
    /// gave them. Shared by the object/array pattern element loops and the object-literal
    /// loop so this ordering — the comment-position contract — can't drift between them.
    pub(in crate::printer) fn push_element_comma_trailing(
        &self,
        parts: &mut DocBuf,
        trailing: &TrailingComments<'_>,
        comma: DocId,
    ) {
        // The comment runs are empty on the common (comment-free) path — collected as
        // empty vecs by the zero-comment gate in `collect_trailing_comments`. Skip
        // pushing their `empty()` docs so a comment-free element leaves no wasted child
        // in the enclosing list concat (which render + every fits pass would still walk).
        // Byte-identical: an empty comment run builds `concat(&[]) == empty()`, so pushing
        // it vs not is the same rendered output.
        if !trailing.block.is_empty() {
            parts.push(self.build_block_comments_doc(&trailing.block));
        }
        parts.push(comma);
        if !trailing.after_comma.is_empty() {
            parts.push(self.build_block_comments_doc(&trailing.after_comma));
        }
        if !trailing.line.is_empty() {
            parts.push(self.build_line_comments_suffix_doc(&trailing.line));
        }
    }
}
