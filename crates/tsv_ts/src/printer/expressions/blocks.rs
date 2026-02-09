// Block statement printing for TypeScript
//
// This module provides reusable block statement printing utilities.
// Block statements are used in multiple contexts:
// - Function bodies (function expressions, arrow functions)
// - Statement contexts (if/while/for blocks, standalone blocks)
// - Class methods
// - Try/catch blocks
//
// By extracting to a separate module, we avoid code duplication across
// expressions/ and statements/ modules.

use smallvec::SmallVec;

use super::Printer;
use crate::ast::internal;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build a Doc for a block statement
    pub(in crate::printer) fn build_block_statement_doc(
        &self,
        block: &internal::BlockStatement,
    ) -> DocId {
        self.build_block_statement_doc_core(block, false)
    }

    /// Build a Doc for a block statement, expanding empty blocks to `{\n}`
    ///
    /// Used in if/else contexts where empty blocks should not stay on one line.
    pub(in crate::printer) fn build_block_statement_expand_empty_doc(
        &self,
        block: &internal::BlockStatement,
    ) -> DocId {
        self.build_block_statement_doc_core(block, true)
    }

    /// Core implementation for block statement doc building
    ///
    /// When `expand_empty` is true, empty blocks without comments become `{\n}`.
    /// When false, they become `{}`.
    fn build_block_statement_doc_core(
        &self,
        block: &internal::BlockStatement,
        expand_empty: bool,
    ) -> DocId {
        // Reset is_expression_statement when entering a block body.
        // This ensures chains inside function bodies don't incorrectly inherit
        // the expression statement context from their parent call (e.g., fn(() => { ... })).
        let prev_is_expr_stmt = self.is_expression_statement.get();
        self.is_expression_statement.set(false);

        let result = self.build_block_statement_doc_inner(block, expand_empty);

        self.is_expression_statement.set(prev_is_expr_stmt);
        result
    }

    fn build_block_statement_doc_inner(
        &self,
        block: &internal::BlockStatement,
        expand_empty: bool,
    ) -> DocId {
        self.build_block_body_doc(block, expand_empty, Vec::new())
    }

    /// Build inner comments doc for empty block
    fn build_inner_comments_for_empty_block(&self, block: &internal::BlockStatement) -> Vec<DocId> {
        let d = self.d();
        let block_start = block.span.start + 1; // After '{'
        let block_end = block.span.end - 1; // Before '}'
        let comments: Vec<_> =
            tsv_lang::comments_in_range(self.comments, block_start, block_end).collect();
        let mut comment_parts = Vec::new();
        for (i, comment) in comments.iter().enumerate() {
            comment_parts.push(self.build_comment_doc(comment));
            // Add hardline after line comments, except for the last one
            // (the hardline before `}` handles that)
            if !comment.is_block && i < comments.len() - 1 {
                comment_parts.push(d.hardline());
            }
        }
        comment_parts
    }

    /// Build a Doc for a block body with optional leading content
    ///
    /// This is the unified implementation for block statement doc building.
    /// The `leading_content` is prepended to the body (used for outer comments).
    fn build_block_body_doc(
        &self,
        block: &internal::BlockStatement,
        expand_empty: bool,
        leading_content: Vec<DocId>,
    ) -> DocId {
        let d = self.d();
        let has_leading = !leading_content.is_empty();
        let block_start = block.span.start + 1; // After '{'
        let block_end = block.span.end - 1; // Before '}'

        if block.body.is_empty() {
            let inner_comments = self.build_inner_comments_for_empty_block(block);
            let has_inner_comments = !inner_comments.is_empty();

            if has_leading || has_inner_comments {
                // Block with comments (outer and/or inner)
                let mut all_content = leading_content;
                if has_inner_comments {
                    if has_leading {
                        all_content.push(d.hardline());
                    }
                    all_content.extend(inner_comments);
                }
                return d.concat(&[
                    d.text("{"),
                    d.indent(d.concat(&[d.hardline(), d.concat(&all_content)])),
                    d.hardline(),
                    d.text("}"),
                ]);
            }

            // Empty block without any comments
            return if expand_empty {
                d.concat(&[d.text("{"), d.hardline(), d.text("}")])
            } else {
                d.text("{}")
            };
        }

        // Build statements with line breaks between them
        // Preserve blank lines and comments from source
        let mut body_parts = Vec::new();

        // Add leading content first (outer comments when present)
        if has_leading {
            body_parts.extend(leading_content);
        }

        let mut prev_end = block_start;
        let mut prev_stmt_end: Option<u32> = None;

        for (i, stmt) in block.body.iter().enumerate() {
            let stmt_start = stmt.span().start;
            let is_first = i == 0;

            // Collect leading comments (skip trailing same-line from previous statement)
            let leading_comments =
                self.collect_leading_comments(prev_end, stmt_start, prev_stmt_end);

            // Handle blank lines and separators
            if is_first && has_leading {
                // First statement after leading content - always need separator
                body_parts.push(d.hardline());
            } else if !is_first {
                // Check for blank lines between statements
                let blank_line_check_end = if !leading_comments.is_empty() {
                    leading_comments[0].span.start
                } else {
                    stmt_start
                };
                if !self.in_template_interpolation.get()
                    && self.has_blank_line_between(prev_end, blank_line_check_end)
                {
                    body_parts.push(d.literalline());
                }
                body_parts.push(d.hardline());
            }

            // Print leading comments before this statement (with blank line preservation)
            body_parts.extend(
                self.build_leading_comments_with_blank_lines(&leading_comments, stmt_start),
            );

            body_parts.push(self.build_statement_doc(stmt));

            // Handle trailing same-line comments after this statement
            let stmt_end = stmt.span().end;
            body_parts.extend(self.build_trailing_same_line_comment_docs(stmt_end, block_end));

            prev_end = stmt_end;
            prev_stmt_end = Some(stmt_end);
        }

        // Handle trailing comments after the last statement (on their own line)
        // Preserve blank lines between last statement and trailing comments, and between comments
        if let Some(last_stmt_end) = prev_stmt_end {
            let mut trailing_prev_end = last_stmt_end;
            for comment in tsv_lang::comments_in_range(self.comments, last_stmt_end, block_end) {
                if self.is_same_line(last_stmt_end, comment.span.start) {
                    continue; // Skip same-line comments (already handled above)
                }
                // Check for blank line before this comment
                if !self.in_template_interpolation.get()
                    && self.has_blank_line_between(trailing_prev_end, comment.span.start)
                {
                    body_parts.push(d.literalline());
                }
                body_parts.push(d.hardline());
                body_parts.push(self.build_comment_doc(comment));
                trailing_prev_end = comment.span.end;
            }
        }

        d.concat(&[
            d.text("{"),
            d.indent(d.concat(&[d.hardline(), d.concat(&body_parts)])),
            d.hardline(),
            d.text("}"),
        ])
    }

    /// Collect leading comments for a statement, filtering out trailing same-line from previous
    fn collect_leading_comments(
        &self,
        prev_end: u32,
        stmt_start: u32,
        prev_stmt_end: Option<u32>,
    ) -> SmallVec<[&internal::Comment; 4]> {
        let comments: SmallVec<[_; 4]> =
            tsv_lang::comments_in_range(self.comments, prev_end, stmt_start).collect();
        if let Some(prev_stmt) = prev_stmt_end {
            comments
                .into_iter()
                .filter(|c| !self.is_same_line(prev_stmt, c.span.start))
                .collect()
        } else {
            comments
        }
    }

    /// Collect outer comments to be moved inside a block
    ///
    /// Used to collect "dangling" comments between a signature and its body:
    /// ```text
    /// fn() // comment
    /// {
    ///     return 1;
    /// }
    /// ```
    pub(in crate::printer) fn build_outer_comments_for_block(
        &self,
        sig_end: u32,
        block: &internal::BlockStatement,
    ) -> Vec<DocId> {
        tsv_lang::comments_in_range(self.comments, sig_end, block.span.start)
            .map(|c| self.build_comment_doc(c))
            .collect()
    }

    /// Build a Doc for a block statement with outer comments moved inside
    ///
    /// The outer_comments are comments from between the signature and opening brace
    /// that should appear at the start of the block body.
    pub(in crate::printer) fn build_block_statement_with_outer_comments_doc(
        &self,
        block: &internal::BlockStatement,
        outer_comments: Vec<DocId>,
    ) -> DocId {
        if outer_comments.is_empty() {
            return self.build_block_statement_doc(block);
        }

        let d = self.d();
        // Build outer comments as leading content
        let mut leading_content = Vec::new();
        for (i, comment_doc) in outer_comments.into_iter().enumerate() {
            if i > 0 {
                leading_content.push(d.hardline());
            }
            leading_content.push(comment_doc);
        }

        // Use unified body builder with leading content
        // Note: expand_empty=false because outer comments will expand the block anyway
        self.build_block_body_doc(block, false, leading_content)
    }
}
