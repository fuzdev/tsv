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

use crate::ast::internal;
use crate::printer::{CommentVec, Printer, is_effectively_empty_body};
use tsv_lang::comments_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build a Doc for a block statement
    pub(in crate::printer) fn build_block_statement_doc(
        &self,
        block: &internal::BlockStatement<'_>,
    ) -> DocId {
        self.build_block_statement_doc_core(block, false)
    }

    /// Build a Doc for a block statement, expanding empty blocks to `{\n}`
    ///
    /// Used in if/else contexts where empty blocks should not stay on one line.
    pub(in crate::printer) fn build_block_statement_expand_empty_doc(
        &self,
        block: &internal::BlockStatement<'_>,
    ) -> DocId {
        self.build_block_statement_doc_core(block, true)
    }

    /// Core implementation for block statement doc building
    ///
    /// When `expand_empty` is true, empty blocks without comments become `{\n}`.
    /// When false, they become `{}`.
    fn build_block_statement_doc_core(
        &self,
        block: &internal::BlockStatement<'_>,
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
        block: &internal::BlockStatement<'_>,
        expand_empty: bool,
    ) -> DocId {
        self.build_block_body_doc(block, expand_empty, DocBuf::new())
    }

    /// Build inner comments doc for empty block
    fn build_inner_comments_for_empty_block(&self, block: &internal::BlockStatement<'_>) -> DocBuf {
        let d = self.d();
        let block_start = block.span.start + 1; // After '{'
        let block_end = block.span.end - 1; // Before '}'
        let comments: CommentVec<'_> =
            comments_in_range(self.comments, block_start, block_end).collect();
        let mut comment_parts = DocBuf::new();
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
        block: &internal::BlockStatement<'_>,
        expand_empty: bool,
        leading_content: DocBuf,
    ) -> DocId {
        let d = self.d();
        let has_leading = !leading_content.is_empty();
        let block_start = block.span.start + 1; // After '{'
        let block_end = block.span.end - 1; // Before '}'

        // Comments attached to a body whose only statements are dropped
        // `EmptyStatement`s are still picked up by
        // `build_inner_comments_for_empty_block`, which scans the full brace
        // range rather than the statement list.
        if is_effectively_empty_body(block.body) {
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
                d.braces(d.hardline())
            } else {
                d.text("{}")
            };
        }

        // A comment trailing the opening `{` on its own line is kept on the `{`
        // line when the body expands (divergence from prettier, which relocates it
        // to its own line as the body's leading comment). Only when there's no
        // hoisted outer content (which would already occupy the first body line).
        // See conformance_prettier.md §Comment relocation (Block body `{`).
        let first_stmt_start = block.body[0].span().start;
        let (brace_line_prefix, delimiter_pull_pos) = if has_leading {
            (DocBuf::new(), None)
        } else {
            self.delimiter_line_comment_prefix(block.span.start, first_stmt_start)
        };

        // Build statements (leading comments, blank-line separators,
        // format-ignore, trailing same-line comments) via the shared walk,
        // filling a pooled buffer (pre-loaded with any hoisted leading content)
        // in place — one RAII owner, released back to the free-list on scope exit.
        let mut body_parts = d.pooled_docbuf();
        body_parts.extend(leading_content);
        let (_prev_end, prev_stmt_end) = self.build_statement_list_docs_into(
            &mut body_parts,
            block.body,
            block_start,
            block_end,
            has_leading,
            delimiter_pull_pos,
        );

        // Handle trailing comments after the last statement (on their own line)
        // Preserve blank lines between last statement and trailing comments, and between comments
        if let Some(last_stmt_end) = prev_stmt_end {
            let trailing_start = self.find_end_with_trailing_comments(last_stmt_end);
            let mut trailing_prev_end = trailing_start;
            for comment in comments_in_range(self.comments, trailing_start, block_end) {
                if self.is_same_line(trailing_start, comment.span.start) {
                    continue; // Skip same-line comments (already handled above)
                }
                // Check for blank line before this comment
                if self.has_blank_line_between(trailing_prev_end, comment.span.start) {
                    body_parts.push(d.literalline());
                }
                body_parts.push(d.hardline());
                body_parts.push(self.build_comment_doc(comment));
                trailing_prev_end = comment.span.end;
            }
        }

        d.concat(&[
            d.text("{"),
            d.concat(&brace_line_prefix),
            d.indent(d.concat(&[d.hardline(), d.concat(&body_parts)])),
            d.hardline(),
            d.text("}"),
        ])
    }

    /// Build docs for a `{ }`-delimited statement list — the shared per-statement
    /// walk for block-statement bodies and `namespace`/`module` bodies.
    ///
    /// For each statement, appends (in order): blank-line separators, leading
    /// comments, the statement doc (or raw source under format-ignore), and
    /// trailing same-line comments — filling the caller-owned `body_parts` buffer
    /// in place. The caller pre-loads `body_parts` with any hoisted outer comments
    /// (emitted first) and passes `has_leading` for that state; the buffer is
    /// drawn from the arena's `DocBuf` free-list, so a fill-in-place seam (rather
    /// than take-by-value + return) keeps a single RAII owner and no aliasing.
    ///
    /// `body_start` is the offset just after `{`; `body_end` is the offset of `}`.
    /// Returns `(prev_end, prev_stmt_end)` where `prev_end` is advanced past the
    /// final statement's trailing same-line comments (the start position for
    /// own-line trailing-comment handling) and `prev_stmt_end` is the final
    /// statement's span end (`None` for an empty body).
    ///
    /// `delimiter_pull_pos`, when `Some(pos)`, excludes the first statement's
    /// leading comments that share a source line with `pos` (the opening `{`) —
    /// the caller emits those as a prefix on the `{` line instead (the open-brace
    /// trailing-comment divergence). Pass `None` to keep the default behavior.
    ///
    /// Callers handle the empty-body case, own-line trailing comments after the
    /// last statement, and the enclosing braces — those differ between contexts.
    pub(in crate::printer) fn build_statement_list_docs_into(
        &self,
        body_parts: &mut DocBuf,
        body: &[internal::Statement<'_>],
        body_start: u32,
        body_end: u32,
        has_leading: bool,
        delimiter_pull_pos: Option<u32>,
    ) -> (u32, Option<u32>) {
        let d = self.d();
        let mut prev_end = body_start;
        let mut prev_stmt_end: Option<u32> = None;

        // Zero-comment fast gate: one binary search over the whole statement-list
        // window short-circuits every per-statement comment sub-query (leading
        // collect, format-ignore lookup, trailing same-line scan, and the
        // trailing-comment end walk). Sound because comments are disjoint +
        // start-sorted and every sub-range lies within `[body_start, body_end]`,
        // so when none sit inside the block all sub-queries are provably
        // empty/false. Blank-line preservation and the `prev_end` cursor are
        // comment-independent and stay outside the gate. Canonical reference:
        // `build_params_doc_with_comments`.
        let body_has_comments = self.has_comments_between(body_start, body_end);

        for (i, stmt) in body.iter().enumerate() {
            let stmt_start = stmt.span().start;
            let is_first = i == 0;

            // Standalone EmptyStatements are dropped entirely (Prettier's
            // `printStatementSequence` never prints them), but any comments
            // attached to one must survive — printed as orphaned comments
            // with nothing following them in this iteration to glue to.
            if matches!(stmt, internal::Statement::EmptyStatement(_)) {
                let stmt_end = stmt.span().end;
                let next_start = body.get(i + 1).map_or(body_end, |s| s.span().start);
                let search_end = if body_has_comments {
                    self.find_end_with_trailing_comments(stmt_end)
                        .min(next_start)
                } else {
                    stmt_end
                };

                let mut leading_comments = if body_has_comments {
                    self.collect_leading_comments(prev_end, search_end, prev_stmt_end)
                } else {
                    CommentVec::new()
                };
                if is_first && let Some(dpos) = delimiter_pull_pos {
                    leading_comments.retain(|c| !self.comment_on_delimiter_line(dpos, c));
                }

                if !leading_comments.is_empty() {
                    if prev_stmt_end.is_some() {
                        let blank_line_check_end = leading_comments[0].span.start;
                        if self.has_blank_line_between(prev_end, blank_line_check_end) {
                            body_parts.push(d.literalline());
                        }
                        body_parts.push(d.hardline());
                    } else if has_leading {
                        body_parts.push(d.hardline());
                    }
                    body_parts.extend(self.build_leading_comments_with_blank_lines(
                        &leading_comments,
                        search_end,
                        true,
                    ));
                    prev_stmt_end = Some(stmt_end);
                }

                prev_end = search_end;
                continue;
            }

            // Collect leading comments (skip trailing same-line from previous
            // statement). Skipped entirely on a comment-free block.
            let mut leading_comments = if body_has_comments {
                self.collect_leading_comments(prev_end, stmt_start, prev_stmt_end)
            } else {
                CommentVec::new()
            };

            // First statement: drop comments pulled onto the opening `{` line (they
            // are emitted as the brace-line prefix by the caller).
            if is_first && let Some(dpos) = delimiter_pull_pos {
                leading_comments.retain(|c| !self.comment_on_delimiter_line(dpos, c));
            }

            // Handle blank lines and separators (comment-independent)
            if prev_stmt_end.is_none() {
                // First visible content after (optional) hoisted leading content —
                // always need a separator if there was hoisted content, never
                // check for a blank line (matches the historical `is_first` behavior).
                if has_leading {
                    body_parts.push(d.hardline());
                }
            } else {
                // Check for blank lines between statements
                let blank_line_check_end = if !leading_comments.is_empty() {
                    leading_comments[0].span.start
                } else {
                    stmt_start
                };
                if self.has_blank_line_between(prev_end, blank_line_check_end) {
                    body_parts.push(d.literalline());
                }
                body_parts.push(d.hardline());
            }

            // Print leading comments before this statement (with blank line preservation)
            body_parts.extend(self.build_leading_comments_with_blank_lines(
                &leading_comments,
                stmt_start,
                false,
            ));

            // format-ignore: emit raw source instead of formatting
            if body_has_comments && self.has_format_ignore_in_range(prev_end, stmt_start) {
                body_parts.push(self.raw_source_doc(stmt.span()));
            } else {
                body_parts.push(self.build_statement_doc(stmt));
            }

            // Handle trailing same-line comments after this statement, and advance
            // `prev_end` past them. Bound the scan by the next statement's start so
            // a comment only attaches to the statement it immediately follows —
            // multiple statements on one source line (`a(); b(); // c`) must not
            // each grab the trailing comment. With no comment in the block,
            // `find_end_with_trailing_comments(end) == end`.
            let stmt_end = stmt.span().end;
            if body_has_comments {
                let next_start = body.get(i + 1).map_or(body_end, |s| s.span().start);
                body_parts.extend(self.build_trailing_same_line_comment_docs(stmt_end, next_start));
                // Update prev_end past trailing comments (including comments on the
                // closing */ line of multi-line block comments)
                prev_end = self.find_end_with_trailing_comments(stmt_end);
            } else {
                prev_end = stmt_end;
            }
            prev_stmt_end = Some(stmt_end);
        }

        (prev_end, prev_stmt_end)
    }

    /// Collect leading comments for a statement, filtering out trailing same-line from previous
    pub(in crate::printer) fn collect_leading_comments(
        &self,
        prev_end: u32,
        stmt_start: u32,
        prev_stmt_end: Option<u32>,
    ) -> CommentVec<'_> {
        let comments: CommentVec<'_> =
            comments_in_range(self.comments, prev_end, stmt_start).collect();
        if let Some(prev_stmt) = prev_stmt_end {
            comments
                .into_iter()
                .filter(|c| !self.is_same_line(prev_stmt, c.span.start))
                .collect()
        } else {
            comments
        }
    }

    /// Build a Doc for a block statement with outer comments moved inside
    ///
    /// The outer_comments are comments from between the signature and opening brace
    /// that should appear at the start of the block body.
    pub(in crate::printer) fn build_block_statement_with_outer_comments_doc(
        &self,
        block: &internal::BlockStatement<'_>,
        outer_comments: DocBuf,
    ) -> DocId {
        if outer_comments.is_empty() {
            return self.build_block_statement_doc(block);
        }

        let d = self.d();
        // Build outer comments as leading content
        let mut leading_content = DocBuf::new();
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
