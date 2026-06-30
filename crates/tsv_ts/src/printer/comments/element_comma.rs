// Trailing comments around a list element's separator comma.
//
// The single source of the `trailingComma: 'none'` comment-position contract for
// inline (group-based) element lists: block comments before the comma stay before
// it, a block comment after the comma on the last element is preserved in place
// (prettier relocates it before — see conformance_prettier.md §Comment relocation),
// and line comments go after the comma via `line_suffix` (zero width). Shared by
// the object/array destructuring-pattern builders and the object-literal builder
// so the ordering can't drift between them.
//
// The comma is located with `find_comma_after` (comment/string-skipping), so a
// comma inside an earlier comment (`a /* , */ /* x */, b`) is never mistaken for
// the separator and the following comment is not relocated across it.

use super::{CommentVec, Printer};
use smallvec::SmallVec;
use tsv_lang::Comment;
use tsv_lang::comments_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// Trailing comments collected for a list element (property or array element)
pub(in crate::printer) struct TrailingComments<'a> {
    /// Block comments that go before the comma
    block: SmallVec<[&'a Comment; 2]>,
    /// Block comments after the comma, preserved in place (last element only —
    /// prettier relocates these before the comma; see conformance_prettier.md)
    block_after: SmallVec<[&'a Comment; 2]>,
    /// Line comments that go after the comma (in line_suffix)
    line: SmallVec<[&'a Comment; 2]>,
    /// Position after all trailing comments (for updating prev_end)
    pub(in crate::printer) end_pos: u32,
}

impl<'a> Printer<'a> {
    /// Collect trailing comments for a list element (property or array element)
    ///
    /// Trailing comments are same-line comments after the element:
    /// - Block comments: only if they appear BEFORE the comma
    /// - Line comments: always belong to this element (they consume the rest of the line)
    pub(in crate::printer) fn collect_trailing_comments(
        &self,
        elem_end: u32,
        upper_bound: u32,
        is_last: bool,
    ) -> TrailingComments<'_> {
        // Find the separator comma in source (if any), skipping commas that sit
        // inside comments or strings so `a /* , */ /* x */, b` is split on the
        // real comma, not the one in `/* , */`.
        let comma_pos = self.find_comma_after(elem_end).filter(|c| *c < upper_bound);

        // Collect same-line trailing comments. A block comment after the comma
        // normally belongs to the next element as leading — except on the LAST
        // element, where it is preserved after the comma (prettier relocates it
        // before — see conformance_prettier.md §Comment relocation).
        let all: CommentVec<'_> = comments_in_range(self.comments, elem_end, upper_bound)
            .filter(|c| {
                self.is_same_line(elem_end, c.span.start)
                    && (!c.is_block
                        || is_last
                        || comma_pos.is_none_or(|comma| c.span.start < comma))
            })
            .collect();

        let is_after_comma =
            |c: &Comment| c.is_block && comma_pos.is_some_and(|comma| c.span.start > comma);

        let block = all
            .iter()
            .filter(|c| c.is_block && !is_after_comma(c))
            .copied()
            .collect();
        let block_after = all.iter().filter(|c| is_after_comma(c)).copied().collect();
        let line = all.iter().filter(|c| !c.is_block).copied().collect();
        let end_pos = all.last().map_or(elem_end, |c| c.span.end);

        TrailingComments {
            block,
            block_after,
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
    /// that preserves comment position: block comments before the comma, the
    /// comma, block comments after the comma (last-element case), then line
    /// comments as a suffix. Shared by the object/array pattern element loops and
    /// the object-literal loop so this ordering — the comment-position contract —
    /// can't drift between them.
    pub(in crate::printer) fn push_element_comma_trailing(
        &self,
        parts: &mut DocBuf,
        trailing: &TrailingComments<'_>,
        comma: DocId,
    ) {
        parts.push(self.build_block_comments_doc(&trailing.block));
        parts.push(comma);
        parts.push(self.build_block_comments_doc(&trailing.block_after));
        parts.push(self.build_line_comments_suffix_doc(&trailing.line));
    }
}
