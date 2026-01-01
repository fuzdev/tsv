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

use super::Printer;
use crate::ast::internal;
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    /// Build a Doc for a block statement
    pub(in crate::printer) fn build_block_statement_doc(
        &self,
        block: &internal::BlockStatement,
    ) -> Doc {
        self.build_block_statement_doc_core(block, false)
    }

    /// Build a Doc for a block statement, expanding empty blocks to `{\n}`
    ///
    /// Used in if/else contexts where empty blocks should not stay on one line.
    pub(in crate::printer) fn build_block_statement_expand_empty_doc(
        &self,
        block: &internal::BlockStatement,
    ) -> Doc {
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
    ) -> Doc {
        if block.body.is_empty() {
            // Check for comments inside empty block
            let block_start = block.span.start + 1; // After '{'
            let block_end = block.span.end - 1; // Before '}'
            let has_inner_comments = self.has_comments_between(block_start, block_end);

            if has_inner_comments {
                let mut comment_parts = Vec::new();
                for comment in tsv_lang::comments_in_range(self.comments, block_start, block_end) {
                    comment_parts.push(self.build_comment_doc(comment));
                    if !comment.is_block {
                        // Line comments need a hardline after
                        comment_parts.push(doc::hardline());
                    }
                }
                return doc::concat(vec![
                    doc::text("{"),
                    doc::indent(doc::concat(vec![
                        doc::hardline(),
                        doc::concat(comment_parts),
                    ])),
                    doc::hardline(),
                    doc::text("}"),
                ]);
            }

            // Empty block without comments
            return if expand_empty {
                doc::concat(vec![doc::text("{"), doc::hardline(), doc::text("}")])
            } else {
                doc::text("{}")
            };
        }

        // Build statements with line breaks between them
        // Preserve blank lines and comments from source
        let mut body_parts = Vec::new();
        let mut prev_end = block.span.start + 1; // Start after '{'
        let mut prev_stmt_end: Option<u32> = None;

        for (i, stmt) in block.body.iter().enumerate() {
            let stmt_start = stmt.span().start;

            // Check for comments between previous position and this statement
            // But skip trailing same-line comments from the previous statement
            let comments: Vec<_> =
                tsv_lang::comments_in_range(self.comments, prev_end, stmt_start).collect();
            let leading_comments: Vec<_> = if let Some(prev_stmt) = prev_stmt_end {
                comments
                    .iter()
                    .filter(|c| {
                        !tsv_lang::printing::is_same_line(self.source, prev_stmt, c.span.start)
                    })
                    .copied()
                    .collect()
            } else {
                comments.clone()
            };
            let has_leading_comments = !leading_comments.is_empty();

            if i > 0 {
                // Check for blank lines between statements (when no leading comments)
                if !has_leading_comments
                    && tsv_lang::printing::has_blank_line_between(self.source, prev_end, stmt_start)
                {
                    // Blank line: literalline (no indent) + hardline (with indent for next stmt)
                    body_parts.push(doc::literalline());
                }
                body_parts.push(doc::hardline());
            }

            // Print leading comments before this statement (excluding trailing same-line from previous)
            for comment in &leading_comments {
                body_parts.push(self.build_comment_doc(comment));
                // Line comments need a hardline after
                // Block comments on their own line get a hardline, block comments on same line as statement get a space
                if !comment.is_block {
                    body_parts.push(doc::hardline());
                } else if !tsv_lang::printing::is_same_line(
                    self.source,
                    comment.span.end,
                    stmt_start,
                ) {
                    // Block comment not on same line as statement - add hardline
                    body_parts.push(doc::hardline());
                } else {
                    // Block comment on same line as statement - add space
                    body_parts.push(doc::text(" "));
                }
            }

            body_parts.push(self.build_statement_doc(stmt));

            // Handle trailing same-line comments after this statement
            let stmt_end = stmt.span().end;
            for comment in tsv_lang::comments_in_range(
                self.comments,
                stmt_end,
                block.span.end - 1, // Before '}'
            ) {
                if tsv_lang::printing::is_same_line(self.source, stmt_end, comment.span.start) {
                    body_parts.push(doc::text(" "));
                    body_parts.push(self.build_comment_doc(comment));
                } else {
                    break; // Only same-line comments
                }
            }

            prev_end = stmt.span().end;
            prev_stmt_end = Some(stmt_end);
        }

        // Structure: `{` + indent(hardline + statements) + hardline + `}`
        doc::concat(vec![
            doc::text("{"),
            doc::indent(doc::concat(vec![doc::hardline(), doc::concat(body_parts)])),
            doc::hardline(),
            doc::text("}"),
        ])
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
    ) -> Vec<Doc> {
        let block_open = block.span.start;
        let mut comments = Vec::new();
        for comment in tsv_lang::comments_in_range(self.comments, sig_end, block_open) {
            comments.push(self.build_comment_doc(comment));
        }
        comments
    }

    /// Build a Doc for a block statement with outer comments moved inside
    ///
    /// The outer_comments are comments from between the signature and opening brace
    /// that should appear at the start of the block body.
    pub(in crate::printer) fn build_block_statement_with_outer_comments_doc(
        &self,
        block: &internal::BlockStatement,
        outer_comments: Vec<Doc>,
    ) -> Doc {
        if outer_comments.is_empty() {
            // No outer comments, use regular doc
            return self.build_block_statement_doc(block);
        }

        // Build outer comments as leading content in block body
        let mut comment_parts = Vec::new();
        for (i, comment_doc) in outer_comments.into_iter().enumerate() {
            if i > 0 {
                comment_parts.push(doc::hardline());
            }
            comment_parts.push(comment_doc);
        }

        if block.body.is_empty() {
            // Empty block with only outer comments
            let block_start = block.span.start + 1;
            let block_end = block.span.end - 1;
            let has_inner_comments = self.has_comments_between(block_start, block_end);

            if has_inner_comments {
                // Also include inner comments
                for comment in tsv_lang::comments_in_range(self.comments, block_start, block_end) {
                    comment_parts.push(doc::hardline());
                    comment_parts.push(self.build_comment_doc(comment));
                }
            }

            return doc::concat(vec![
                doc::text("{"),
                doc::indent(doc::concat(vec![
                    doc::hardline(),
                    doc::concat(comment_parts),
                ])),
                doc::hardline(),
                doc::text("}"),
            ]);
        }

        // Block has statements - build body with outer comments first
        let mut body_parts = Vec::new();

        // Outer comments first
        body_parts.push(doc::concat(comment_parts));

        // Then regular statements
        let mut prev_end = block.span.start + 1;
        let mut prev_stmt_end: Option<u32> = None;

        for (i, stmt) in block.body.iter().enumerate() {
            let stmt_start = stmt.span().start;
            let is_first = i == 0;

            // For all statements (including first), add hardline before
            // (outer comments already present, so always need a separator)
            if !is_first
                && tsv_lang::printing::has_blank_line_between(self.source, prev_end, stmt_start)
            {
                body_parts.push(doc::literalline());
            }
            body_parts.push(doc::hardline());

            // Handle leading comments (skip trailing same-line from previous)
            let comments: Vec<_> =
                tsv_lang::comments_in_range(self.comments, prev_end, stmt_start).collect();
            let leading_comments: Vec<_> = if let Some(prev_stmt) = prev_stmt_end {
                comments
                    .iter()
                    .filter(|c| {
                        !tsv_lang::printing::is_same_line(self.source, prev_stmt, c.span.start)
                    })
                    .copied()
                    .collect()
            } else {
                comments.clone()
            };

            for comment in &leading_comments {
                body_parts.push(self.build_comment_doc(comment));
                if !comment.is_block
                    || !tsv_lang::printing::is_same_line(self.source, comment.span.end, stmt_start)
                {
                    body_parts.push(doc::hardline());
                } else {
                    body_parts.push(doc::text(" "));
                }
            }

            // Statement itself
            let stmt_doc = self.build_statement_doc(stmt);
            body_parts.push(stmt_doc);

            // Trailing same-line comments
            let stmt_end = stmt.span().end;
            for comment in tsv_lang::comments_in_range(self.comments, stmt_end, block.span.end - 1)
            {
                if tsv_lang::printing::is_same_line(self.source, stmt_end, comment.span.start) {
                    body_parts.push(doc::text(" "));
                    body_parts.push(self.build_comment_doc(comment));
                } else {
                    break;
                }
            }

            prev_end = stmt.span().end;
            prev_stmt_end = Some(stmt_end);
        }

        doc::concat(vec![
            doc::text("{"),
            doc::indent(doc::concat(vec![doc::hardline(), doc::concat(body_parts)])),
            doc::hardline(),
            doc::text("}"),
        ])
    }
}
