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

use super::super::Printer;
use crate::ast::internal;
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    /// Print a block statement with outer comments moved inside.
    ///
    /// `outer_start` is the position from which to search for "dangling" comments
    /// that should be moved inside the block. This handles cases like:
    ///
    /// ```text
    /// fn() // comment
    /// {
    ///     return 1;
    /// }
    /// ```
    ///
    /// Where the comment between `)` and `{` should be formatted as:
    ///
    /// ```text
    /// fn() {
    ///     // comment
    ///     return 1;
    /// }
    /// ```
    pub(in crate::printer) fn print_block_statement_with_outer_comments(
        &mut self,
        block: &internal::BlockStatement,
        outer_start: u32,
    ) {
        // Check for comments between outer_start and block start
        // These are "dangling" comments that should be moved inside the block
        let block_open = block.span.start; // Position of '{'
        let has_outer_comments = self.has_comments_between(outer_start, block_open);

        if block.body.is_empty() {
            // Check for comments inside empty block AND outer comments
            let block_start = block.span.start + 1; // After '{'
            let block_end = block.span.end - 1; // Before '}'
            let has_inner_comments = self.has_comments_between(block_start, block_end);

            if has_outer_comments || has_inner_comments {
                self.write("{\n");
                self.indent_level += 1;
                // Print outer comments first (moved inside)
                if has_outer_comments {
                    for comment in
                        tsv_lang::comments_in_range(self.comments, outer_start, block_open)
                    {
                        self.write_indent();
                        self.print_comment(comment);
                        self.write("\n");
                    }
                }
                // Then print inner comments
                if has_inner_comments {
                    self.print_leading_comments(block_start, block_end, false);
                }
                self.indent_level -= 1;
                self.write_indent();
                self.write("}");
            } else {
                self.write("{}");
            }
            return;
        }

        self.write("{\n");
        self.indent_level += 1;

        // Print outer comments first (moved inside the block)
        if has_outer_comments {
            for comment in tsv_lang::comments_in_range(self.comments, outer_start, block_open) {
                self.write_indent();
                self.print_comment(comment);
                self.write("\n");
            }
        }

        // Track previous statement end for comment printing
        let mut prev_end = block.span.start + 1; // Start after '{'

        for (i, stmt) in block.body.iter().enumerate() {
            let is_first = i == 0;

            // Check for blank lines between statements (when no comments)
            if !is_first {
                let has_comments = self
                    .comments
                    .iter()
                    .any(|c| c.span.start >= prev_end && c.span.end <= stmt.span().start);

                if !has_comments
                    && tsv_lang::printing::has_blank_line_between(
                        self.source,
                        prev_end,
                        stmt.span().start,
                    )
                {
                    self.write("\n");
                }
            }

            // Print leading comments before this statement
            // For the first statement, include same-line comments after '{'
            self.print_block_leading_comments(prev_end, stmt.span().start, is_first);

            self.write_indent();
            self.print_statement(stmt);
            self.write("\n");

            prev_end = stmt.span().end;
        }

        self.indent_level -= 1;
        self.write_indent();
        self.write("}");
    }

    /// Print a block statement with empty blocks always expanded to `{\n}`
    ///
    /// Used for if/else/try/finally where prettier expands empty blocks.
    pub(in crate::printer) fn print_block_statement_expand_empty(
        &mut self,
        block: &internal::BlockStatement,
    ) {
        self.print_block_statement_core(block, true);
    }

    /// Print a block statement: `{ stmt1; stmt2; }`
    ///
    /// Handles comments between statements similar to print_program.
    pub(in crate::printer) fn print_block_statement(&mut self, block: &internal::BlockStatement) {
        self.print_block_statement_core(block, false);
    }

    /// Core implementation for block statement printing
    ///
    /// When `expand_empty` is true, empty blocks without comments become `{\n}`.
    /// When false, they become `{}`.
    fn print_block_statement_core(&mut self, block: &internal::BlockStatement, expand_empty: bool) {
        if block.body.is_empty() {
            // Check for comments inside empty block
            let block_start = block.span.start + 1; // After '{'
            let block_end = block.span.end - 1; // Before '}'
            let has_inner_comments = self.has_comments_between(block_start, block_end);

            if has_inner_comments || expand_empty {
                self.write("{\n");
                self.indent_level += 1;
                if has_inner_comments {
                    self.print_leading_comments(block_start, block_end, false);
                }
                self.indent_level -= 1;
                self.write_indent();
                self.write("}");
            } else {
                self.write("{}");
            }
            return;
        }

        self.write("{\n");
        self.indent_level += 1;

        // Track previous statement end for comment printing (similar to print_program)
        let mut prev_end = block.span.start + 1; // Start after '{'

        for (i, stmt) in block.body.iter().enumerate() {
            let is_first = i == 0;

            // Check for blank lines between statements (when no comments)
            if !is_first {
                let has_comments = self
                    .comments
                    .iter()
                    .any(|c| c.span.start >= prev_end && c.span.end <= stmt.span().start);

                if !has_comments
                    && tsv_lang::printing::has_blank_line_between(
                        self.source,
                        prev_end,
                        stmt.span().start,
                    )
                {
                    self.write("\n");
                }
            }

            // Print leading comments before this statement
            // For the first statement, include same-line comments after '{'
            self.print_block_leading_comments(prev_end, stmt.span().start, is_first);

            self.write_indent();
            self.print_statement(stmt);
            self.write("\n");

            prev_end = stmt.span().end;
        }

        self.indent_level -= 1;
        self.write_indent();
        self.write("}");
    }

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

        for (i, stmt) in block.body.iter().enumerate() {
            let stmt_start = stmt.span().start;

            // Check for comments between previous position and this statement
            let has_comments = self
                .comments
                .iter()
                .any(|c| c.span.start >= prev_end && c.span.end <= stmt_start);

            if i > 0 {
                // Check for blank lines between statements (when no comments)
                if !has_comments
                    && tsv_lang::printing::has_blank_line_between(self.source, prev_end, stmt_start)
                {
                    // Blank line: literalline (no indent) + hardline (with indent for next stmt)
                    body_parts.push(doc::literalline());
                }
                body_parts.push(doc::hardline());
            }

            // Print leading comments before this statement
            if has_comments {
                for comment in tsv_lang::comments_in_range(self.comments, prev_end, stmt_start) {
                    body_parts.push(self.build_comment_doc(comment));
                    // Line comments need a hardline after, block comments just need spacing
                    if !comment.is_block {
                        body_parts.push(doc::hardline());
                    } else {
                        body_parts.push(doc::text(" "));
                    }
                }
            }

            body_parts.push(self.build_statement_doc(stmt));
            prev_end = stmt.span().end;
        }

        // Structure: `{` + indent(hardline + statements) + hardline + `}`
        doc::concat(vec![
            doc::text("{"),
            doc::indent(doc::concat(vec![doc::hardline(), doc::concat(body_parts)])),
            doc::hardline(),
            doc::text("}"),
        ])
    }
}
