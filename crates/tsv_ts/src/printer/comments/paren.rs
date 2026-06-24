// Stripped-grouping-paren comment handling.
//
// When the parser strips redundant grouping parens, comments that lived inside
// them are orphaned in the source. These helpers preserve such comments in the
// user's position — trailing the expression, promoted before `=` / an operator,
// re-added with the parens when stripping would relocate them, or prepended at a
// chain base.

use super::{CommentSpacing, Printer};
use crate::ast::internal;
use tsv_lang::comments_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build the leading-comment doc for comments between an opening `(` and the
    /// value that follows, concatenated with `value_doc`. Returns the combined doc
    /// plus whether a line or own-line block comment forces the enclosing parens to
    /// break across lines.
    ///
    /// An own-line block comment requires a newline BOTH before and after it —
    /// prettier keeps `(\n/* c */value)` inline because nothing separates the comment
    /// from the value. Shared by dynamic `import(...)` and TS `import(...)` types.
    pub(crate) fn build_paren_leading_value_doc(
        &self,
        open_paren_end: u32,
        value_start: u32,
        value_doc: DocId,
    ) -> (DocId, bool) {
        let d = self.d();
        let own_line = comments_in_range(self.comments, open_paren_end, value_start).any(|c| {
            c.is_block
                && self.has_newline_between(open_paren_end, c.span.start)
                && self.has_newline_between(c.span.end, value_start)
        });
        let line = self.has_line_comments_between(open_paren_end, value_start);
        let force_break = own_line || line;

        let doc = if force_break {
            // Each comment on its own line inside the broken parens.
            let mut parts = DocBuf::new();
            for comment in comments_in_range(self.comments, open_paren_end, value_start) {
                parts.push(self.build_comment_doc(comment));
                parts.push(d.hardline());
            }
            parts.push(value_doc);
            d.concat(&parts)
        } else if let Some(lead) = self.build_rhs_comments_opt(open_paren_end, value_start) {
            // Inline block comment(s): `/* c */ value`
            d.concat(&[lead, value_doc])
        } else {
            value_doc
        };
        (doc, force_break)
    }

    /// Append trailing comments from stripped grouping parens to a parts vec.
    ///
    /// When the parser strips grouping parens (e.g., `await (x /* c */)` → arg is `x`),
    /// comments between the argument end and the expression span end are orphaned.
    /// This method emits them with appropriate layout:
    /// - Same-line block comments: inline with leading space (`x /* c */`)
    /// - Line comments: deferred via `line_suffix` to appear after the semicolon (`x; // c`)
    /// - Own-line block comments: deferred via `line_suffix` with hardline (`x;\n/* c */`)
    ///
    /// Used by await, yield, return, throw, and export default.
    pub(crate) fn append_trailing_paren_comments(
        &self,
        parts: &mut DocBuf,
        argument_end: u32,
        span_end: u32,
    ) {
        let d = self.d();
        for comment in comments_in_range(self.comments, argument_end, span_end) {
            if comment.is_block && !self.has_newline_between(argument_end, comment.span.start) {
                // Same-line block comment: `expr /* c */`
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else if !comment.is_block {
                // Line comment: defer to after semicolon via line_suffix
                let suffix = d.concat(&[d.text(" "), self.build_comment_doc(comment)]);
                parts.push(d.line_suffix(suffix));
            } else {
                // Own-line block comment: defer to own line after semicolon
                let suffix = d.concat(&[d.hardline(), self.build_comment_doc(comment)]);
                parts.push(d.line_suffix(suffix));
            }
        }
    }

    /// Append comments between a declaration's last content token and its
    /// terminating `;`, preserving the user's placement (consistent with the
    /// before-semicolon and do-while `)`→`;` divergences — see
    /// `conformance_prettier.md` §Comment relocation). A same-line block comment
    /// trails the content inline (` /* c */`); line comments and own-line block
    /// comments stay on their own line, forcing the `;` onto a following line.
    ///
    /// Returns `true` if any comment forced a line break, so the caller emits the
    /// `;` after a `hardline` (and keeps these comments outside the content group
    /// so the break doesn't expand the specifier braces).
    pub(crate) fn append_pre_semi_comments(
        &self,
        parts: &mut DocBuf,
        start: u32,
        end: u32,
    ) -> bool {
        let d = self.d();
        let mut prev_end = start;
        let mut broke = false;
        for comment in comments_in_range(self.comments, start, end) {
            let same_line = self.is_same_line(prev_end, comment.span.start);
            if comment.is_block && same_line {
                // Same-line block comment trails inline.
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else if same_line {
                // Trailing line comment: stays on the content line, forces a break.
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
                broke = true;
            } else {
                // Own-line comment (line or block): preserve its own line.
                if self.has_blank_line_between(prev_end, comment.span.start) {
                    parts.push(d.literalline());
                }
                parts.push(d.hardline());
                parts.push(self.build_comment_doc(comment));
                broke = true;
            }
            prev_end = comment.span.end;
        }
        broke
    }

    /// Append trailing comments from stripped grouping parens in spread elements,
    /// excluding own-line block comments (which are handled by the parent array/call).
    ///
    /// Own-line block comments in spread (`...(x\n/* c */)`) need to become siblings
    /// in the parent list, after the spread's comma. Using `line_suffix` would defer
    /// them past the enclosing `]`/`)` bracket. Instead, the parent formatter picks
    /// them up via `spread_own_line_block_comments()`.
    pub(crate) fn append_spread_trailing_paren_comments(
        &self,
        parts: &mut DocBuf,
        argument_end: u32,
        span_end: u32,
    ) {
        let d = self.d();
        for comment in comments_in_range(self.comments, argument_end, span_end) {
            if comment.is_block && !self.has_newline_between(argument_end, comment.span.start) {
                // Same-line block comment: `...x /* c */`
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else if !comment.is_block {
                // Line comment: defer to after semicolon via line_suffix
                let suffix = d.concat(&[d.text(" "), self.build_comment_doc(comment)]);
                parts.push(d.line_suffix(suffix));
            }
            // Own-line block comments: skip (handled by parent array/call)
        }
    }

    /// Get own-line block comments from stripped parens in a spread element.
    ///
    /// When the parser strips grouping parens (e.g., `...(x\n/* c */)`), own-line
    /// block comments between `argument.end` and `spread.span.end` need to be emitted
    /// by the parent formatter (array/call) as siblings after the spread's comma,
    /// not by the spread doc itself.
    pub(crate) fn spread_own_line_block_comments(
        &self,
        expr: &internal::Expression,
    ) -> Vec<&tsv_lang::Comment> {
        if let internal::Expression::SpreadElement(spread) = expr {
            let arg_end = spread.argument.span().end;
            comments_in_range(self.comments, arg_end, spread.span.end)
                .filter(|c| c.is_block && self.has_newline_between(arg_end, c.span.start))
                .collect()
        } else {
            vec![]
        }
    }

    /// Detect a block comment that should be promoted from after `=` to before `=`.
    ///
    /// When JSDoc cast parens are stripped (e.g., `var a = /** @type {T} */ (\n\texpr\n)`),
    /// multiple block comments end up after `=`. Prettier places the first one before `=`
    /// when it's on a different source line than the second. Returns the promoted comment's
    /// doc (with leading space) and the end position to use as the new RHS comment start.
    pub(crate) fn promote_block_comment_before_eq(
        &self,
        start: u32,
        end: u32,
    ) -> Option<(DocId, u32)> {
        let d = self.d();
        let blocks: Vec<_> = comments_in_range(self.comments, start, end)
            .filter(|c| c.is_block)
            .collect();
        if blocks.len() >= 2 && !self.is_same_line(blocks[0].span.start, blocks[1].span.start) {
            let doc = d.concat(&[d.text(" "), self.build_comment_doc(blocks[0])]);
            Some((doc, blocks[0].span.end))
        } else {
            None
        }
    }

    /// Check if stripped grouping parens left trailing comments.
    ///
    /// Returns true when there are comments between `expr_end` and `boundary_end`
    /// AND a `)` exists in the source after those comments (confirming that the
    /// parser stripped a `ParenthesizedExpression`). Without the `)` check, this
    /// would false-positive on normal operator comments (e.g. ternary `? c /* comment */ :`).
    pub(crate) fn has_trailing_paren_comments(&self, expr_end: u32, boundary_end: u32) -> bool {
        if !self.has_comments_between(expr_end, boundary_end) {
            return false;
        }
        // Find the last comment's end, then check for `)` between there and boundary
        let last_comment_end = comments_in_range(self.comments, expr_end, boundary_end)
            .last()
            .map_or(expr_end as usize, |c| c.span.end as usize);
        self.source[last_comment_end..boundary_end as usize]
            .bytes()
            .any(|b| b == b')')
    }

    /// Build expression doc, stripping a redundant grouping paren around a trailing
    /// comment and keeping the comment inline after the expression.
    ///
    /// When the parser strips parens from `(expr /* c */)`, comments between
    /// `expr.span().end` and `boundary_end` would be lost. For an inline same-line
    /// block comment we keep it trailing the expression (`expr /* c */`), matching
    /// prettier — stripping the redundant parens does not move the comment. Line /
    /// own-line comments need the parens (a bare line comment would swallow the
    /// following token), so those defer to `build_expression_doc_keep_paren_comments`.
    ///
    /// Used for variable init, assignment RHS, and ternary branches.
    pub(crate) fn build_expression_doc_with_paren_comments(
        &self,
        expr: &internal::Expression,
        boundary_end: u32,
    ) -> DocId {
        let expr_end = expr.span().end;

        if !self.has_trailing_paren_comments(expr_end, boundary_end) {
            return self.build_expression_doc(expr);
        }

        // Line / own-line comments need the paren wrapping (a bare line comment
        // would swallow the following `;`); defer those to the keep variant.
        let has_multiline = comments_in_range(self.comments, expr_end, boundary_end)
            .any(|c| !c.is_block || self.has_newline_between(expr_end, c.span.start));
        if has_multiline {
            return self.build_expression_doc_keep_paren_comments(expr, boundary_end);
        }

        let d = self.d();
        let inner = self.build_expression_doc(expr);
        let comments = self.build_comments_between(expr_end, boundary_end, CommentSpacing::Leading);
        d.concat(&[inner, comments])
    }

    /// Build expression doc re-adding the stripped grouping parens around trailing
    /// comments, producing `(expr /* c */)` or `(\n\texpr // c\n)`.
    ///
    /// Used where stripping the parens would relocate the comment: arrow bodies
    /// (prettier moves the comment into the params) and sequence operands (prettier
    /// floats it out of the sequence). Keeping the parens preserves the comment where
    /// the user wrote it.
    pub(crate) fn build_expression_doc_keep_paren_comments(
        &self,
        expr: &internal::Expression,
        boundary_end: u32,
    ) -> DocId {
        let d = self.d();
        let expr_end = expr.span().end;

        if !self.has_trailing_paren_comments(expr_end, boundary_end) {
            return self.build_expression_doc(expr);
        }

        let inner = self.build_expression_doc(expr);

        // Determine if multiline layout is needed
        let has_multiline = comments_in_range(self.comments, expr_end, boundary_end)
            .any(|c| !c.is_block || self.has_newline_between(expr_end, c.span.start));

        if has_multiline {
            let mut indent_parts = vec![d.hardline()];
            indent_parts.push(inner);
            for comment in comments_in_range(self.comments, expr_end, boundary_end) {
                if !comment.is_block || !self.has_newline_between(expr_end, comment.span.start) {
                    indent_parts.push(d.text(" "));
                    indent_parts.push(self.build_comment_doc(comment));
                } else {
                    indent_parts.push(d.hardline());
                    indent_parts.push(self.build_comment_doc(comment));
                }
            }
            d.concat(&[
                d.text("("),
                d.indent(d.concat(&indent_parts)),
                d.hardline(),
                d.text(")"),
            ])
        } else {
            let mut parts = vec![d.text("(")];
            parts.push(inner);
            for comment in comments_in_range(self.comments, expr_end, boundary_end) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }
            parts.push(d.text(")"));
            d.concat(&parts)
        }
    }

    /// Promote block comments that appear before an assignment operator to the LHS.
    ///
    /// In `a /* comment */ = b`, the comment is between `left.span().end` and `right.span().start`
    /// but positioned before the `=` in source. Prettier places such comments before the operator,
    /// so we promote them to the LHS doc.
    ///
    /// Returns the promoted comments doc (with leading space) and the new RHS comment start
    /// position, or None if no comments need promoting.
    pub(crate) fn promote_comments_before_operator(
        &self,
        start: u32,
        end: u32,
        operator: &str,
    ) -> Option<(DocId, u32)> {
        let d = self.d();
        // Find the operator position by scanning forward, skipping whitespace and comments
        let op_pos = self.find_operator_in_source(start, end, operator)?;

        // Collect block comments that appear before the operator
        let mut promoted_parts = DocBuf::new();
        let mut last_promoted_end = start;
        for comment in comments_in_range(self.comments, start, op_pos) {
            if comment.is_block {
                promoted_parts.push(d.text(" "));
                promoted_parts.push(self.build_comment_doc(comment));
                last_promoted_end = comment.span.end;
            }
        }

        if promoted_parts.is_empty() {
            None
        } else {
            Some((d.concat(&promoted_parts), last_promoted_end))
        }
    }

    /// Find the position of an operator string between two positions, skipping
    /// whitespace and comments in the source.
    fn find_operator_in_source(&self, start: u32, end: u32, operator: &str) -> Option<u32> {
        let bytes = self.source.as_bytes();
        let op_bytes = operator.as_bytes();
        let op_len = op_bytes.len();
        let end_usize = end as usize;
        let mut i = start as usize;

        while i + op_len <= end_usize {
            let b = bytes[i];
            if b.is_ascii_whitespace() {
                i += 1;
                continue;
            }
            if b == b'/' && i + 1 < end_usize {
                match bytes[i + 1] {
                    b'*' => {
                        i += 2;
                        while i + 1 < end_usize && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                            i += 1;
                        }
                        i += 2;
                        continue;
                    }
                    b'/' => {
                        while i < end_usize && bytes[i] != b'\n' {
                            i += 1;
                        }
                        i += 1;
                        continue;
                    }
                    _ => {}
                }
            }
            if &bytes[i..i + op_len] == op_bytes {
                return Some(i as u32);
            }
            i += 1;
        }
        None
    }

    /// Prepend comments from removed parentheses to a doc.
    ///
    /// When parentheses are removed during parsing (e.g., `(/* comment */ expr)` becomes `expr`),
    /// the expression's span extends to include the removed parens. Comments between
    /// `outer_start` (the paren) and `inner_start` (the expression) need to be preserved.
    ///
    /// Returns the original doc unchanged if no comments or if `outer_start >= inner_start`.
    #[inline]
    pub(crate) fn prepend_removed_paren_comments(
        &self,
        outer_start: u32,
        inner_start: u32,
        doc: DocId,
    ) -> DocId {
        if outer_start < inner_start {
            if let Some(comments) = self.build_rhs_comments_opt(outer_start, inner_start) {
                let d = self.d();
                d.concat(&[comments, doc])
            } else {
                doc
            }
        } else {
            doc
        }
    }
}
