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
use crate::printer::{CommentVec, Printer, is_effectively_empty_body, next_printed_stmt_start};
use tsv_lang::Span;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// What [`Printer::build_statement_list_docs_into`] leaves for its caller's end-of-body
/// comment emitter.
pub(in crate::printer) struct StatementListTail {
    /// Source cursor past everything the walk emitted.
    pub prev_end: u32,
    /// End of the last statement actually printed; `None` when the walk printed none.
    pub last_stmt_end: Option<u32>,
    /// The last statement deferred a line comment past its own `;`
    /// ([`Printer::terminator_defers_line_comment`]), so the comments sharing that line
    /// are still UNCLAIMED — the caller's end-of-body emitter must print them, and must
    /// not advance its anchor past them first.
    pub claims_trailing: bool,
}

impl<'a> Printer<'a> {
    /// Build a Doc for a block statement. Its body is a genuine `BlockStatement`
    /// (a function/method/catch body, or a plain block reached through the
    /// non-static-block callers below), so bare string statements inside it are
    /// directive-prologue eligible — see [`Printer::needs_avoid_directive_parens`].
    pub(in crate::printer) fn build_block_statement_doc(
        &self,
        block: &internal::BlockStatement<'_>,
    ) -> DocId {
        self.build_block_statement_doc_core(block, false, true)
    }

    /// Build a Doc for a block statement, expanding empty blocks to `{\n}`
    ///
    /// Used in if/else contexts where empty blocks should not stay on one line.
    pub(in crate::printer) fn build_block_statement_expand_empty_doc(
        &self,
        block: &internal::BlockStatement<'_>,
    ) -> DocId {
        self.build_block_statement_doc_core(block, true, true)
    }

    /// Build a Doc for a `StaticBlock`'s body, reusing this module's block
    /// machinery via a synthetic `BlockStatement` wrapper (see
    /// `build_static_block_doc`). Unlike a real `BlockStatement`, a static
    /// block's ESTree-equivalent node type isn't directive-prologue eligible
    /// (Prettier's grandparent check only matches `Program`/`BlockStatement`),
    /// so a bare string statement here never gets the avoid-directive parens —
    /// see [`Printer::needs_avoid_directive_parens`].
    pub(in crate::printer) fn build_static_block_body_doc(
        &self,
        block: &internal::BlockStatement<'_>,
    ) -> DocId {
        self.build_block_statement_doc_core(block, false, false)
    }

    /// Core implementation for block statement doc building
    ///
    /// When `expand_empty` is true, empty blocks without comments become `{\n}`.
    /// When false, they become `{}`. `in_program_or_block` is threaded to the
    /// contained expression statements — see [`Printer::build_statement_doc`].
    fn build_block_statement_doc_core(
        &self,
        block: &internal::BlockStatement<'_>,
        expand_empty: bool,
        in_program_or_block: bool,
    ) -> DocId {
        // Reset is_expression_statement when entering a block body.
        // This ensures chains inside function bodies don't incorrectly inherit
        // the expression statement context from their parent call (e.g., fn(() => { ... })).
        let prev_is_expr_stmt = self.is_expression_statement.get();
        self.is_expression_statement.set(false);

        let result = self.build_block_statement_doc_inner(block, expand_empty, in_program_or_block);

        self.is_expression_statement.set(prev_is_expr_stmt);
        result
    }

    fn build_block_statement_doc_inner(
        &self,
        block: &internal::BlockStatement<'_>,
        expand_empty: bool,
        in_program_or_block: bool,
    ) -> DocId {
        self.build_block_body_doc(block, expand_empty, DocBuf::new(), in_program_or_block)
    }

    /// Build inner comments doc for empty block — the dangling run, hardline-joined
    /// ([`Printer::push_dangling_comment_run`]). Only the run itself: the caller supplies
    /// the break before it (and any hoisted leading content above it) and the one before
    /// `}`.
    fn build_inner_comments_for_empty_block(&self, block: &internal::BlockStatement<'_>) -> DocBuf {
        let block_start = block.span.start + 1; // After '{'
        let block_end = block.span.end - 1; // Before '}'
        let mut comment_parts = DocBuf::new();
        self.push_dangling_comment_run(
            &mut comment_parts,
            comments_to_emit_in_range(self.comments, block_start, block_end),
        );
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
        in_program_or_block: bool,
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
        // See conformance_prettier_ts_comments.md §Comment relocation (Block body `{`).
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
        let tail = self.build_statement_list_docs_into(
            &mut body_parts,
            block.body,
            Span::new(block_start, block_end),
            has_leading,
            delimiter_pull_pos,
            in_program_or_block,
        );

        // Handle trailing comments after the last statement (on their own line)
        // Preserve blank lines between last statement and trailing comments, and between
        // comments — the shared end-of-body run, same emitter the class/interface/enum/
        // type-literal/namespace bodies use.
        if let Some(last_stmt_end) = tail.last_stmt_end {
            // A last statement that deferred a line comment past its own `;` trailed
            // nothing on that line, and there is no further statement to lead — so this
            // run claims them, and its anchor must stay at the `;` rather than advancing
            // past the very comments it has to print (`terminator_defers_line_comment`).
            let trailing_start = if tail.claims_trailing {
                tail.prev_end
            } else {
                self.find_end_with_trailing_comments(last_stmt_end)
            };
            body_parts.extend(self.build_trailing_body_comments_doc(
                trailing_start,
                block_end,
                tail.claims_trailing,
            ));
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
    /// `body_span` covers just after `{` to just before `}`.
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
    /// `in_program_or_block` is forwarded to each statement — see
    /// [`Printer::build_statement_doc`].
    ///
    /// Callers handle the empty-body case, own-line trailing comments after the
    /// last statement, and the enclosing braces — those differ between contexts.
    pub(in crate::printer) fn build_statement_list_docs_into(
        &self,
        body_parts: &mut DocBuf,
        body: &[internal::Statement<'_>],
        body_span: Span,
        has_leading: bool,
        delimiter_pull_pos: Option<u32>,
        in_program_or_block: bool,
    ) -> StatementListTail {
        let Span {
            start: body_start,
            end: body_end,
        } = body_span;
        let d = self.d();
        let mut prev_end = body_start;
        let mut prev_stmt_end: Option<u32> = None;
        // Set when the statement just emitted deferred a line comment past its own `;`, so
        // its doc ends on a later line than the `;` and cannot carry that line's comments.
        let mut prev_deferred_line_comment = false;

        // Zero-comment fast gate: one binary search over the whole statement-list
        // window short-circuits every per-statement comment sub-query (leading
        // collect, format-ignore lookup, trailing same-line scan, and the
        // trailing-comment end walk). Sound because comments are disjoint +
        // start-sorted and every sub-range lies within `[body_start, body_end]`,
        // so when none sit inside the block all sub-queries are provably
        // empty/false. Blank-line preservation and the `prev_end` cursor are
        // comment-independent and stay outside the gate. Canonical reference:
        // `build_params_doc_with_comments`.
        let body_has_comments = self.has_comments_on_page_between(body_start, body_end);

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
                    self.collect_leading_comments(prev_end, search_end, prev_stmt_end, false)
                } else {
                    CommentVec::new()
                };
                if is_first && let Some(dpos) = delimiter_pull_pos {
                    leading_comments.retain(|c| !self.comment_on_delimiter_line(dpos, c));
                }

                if !leading_comments.is_empty() {
                    if prev_stmt_end.is_some() {
                        let blank_line_check_end = self.blank_scan_end(prev_end, search_end);
                        if self.has_blank_line_between(prev_end, blank_line_check_end) {
                            body_parts.push(d.literalline());
                        }
                        body_parts.push(d.hardline());
                    } else if has_leading {
                        body_parts.push(d.hardline());
                    }
                    self.push_orphaned_comment_run(body_parts, &leading_comments, search_end);
                    prev_stmt_end = Some(stmt_end);
                }

                prev_end = search_end;
                continue;
            }

            // Collect leading comments (skip trailing same-line from previous
            // statement). Skipped entirely on a comment-free block.
            //
            let mut leading_comments = if body_has_comments {
                self.collect_leading_comments(
                    prev_end,
                    stmt_start,
                    prev_stmt_end,
                    prev_deferred_line_comment,
                )
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
                // Check for blank lines between statements — up to the first comment
                // physically in the gap, which is not always the first one this gap
                // *emits* (an owned annotation is printed by the statement it leads).
                //
                // Inside the window gate: `blank_scan_end` is itself a comment search,
                // and with no comment anywhere in the block it provably returns
                // `stmt_start`. The gate is an *on-page* question and this is an
                // *in-source* one, which is sound because only the *to-emit* axis skips
                // an owned comment — on-page and in-source have the same membership.
                let blank_line_check_end = if body_has_comments {
                    self.blank_scan_end(prev_end, stmt_start)
                } else {
                    stmt_start
                };
                if self.has_blank_line_between(prev_end, blank_line_check_end) {
                    body_parts.push(d.literalline());
                }
                body_parts.push(d.hardline());
            }

            // Print leading comments before this statement (with blank line preservation)
            self.push_leading_comments_before(body_parts, &leading_comments, stmt_start);

            // format-ignore: emit raw source instead of formatting. The freeze emitter
            // claims the block comment **glued** before the statement — owned by whatever
            // node heads it, so it rides inside the doc the slice replaces and the leading
            // run above skips it (docs/comments.md hazard 1).
            if body_has_comments && self.member_gap_frozen(prev_end, stmt_start) {
                body_parts.push(self.build_frozen_node_doc(stmt.span()));
            } else {
                body_parts.push(self.build_statement_doc(stmt, in_program_or_block));
            }

            // Handle trailing same-line comments after this statement, and advance
            // `prev_end` past them. Bound the scan by the next statement's start so
            // a comment only attaches to the statement it immediately follows —
            // multiple statements on one source line (`a(); b(); // c`) must not
            // each grab the trailing comment. With no comment in the block,
            // `find_end_with_trailing_comments(end) == end`.
            let stmt_end = stmt.span().end;
            prev_deferred_line_comment =
                body_has_comments && self.terminator_defers_line_comment(stmt_start, stmt_end);
            if prev_deferred_line_comment {
                // …unless this statement's doc ends with a line comment its terminator gap
                // deferred past the `;`. Nothing may share that line, so leaving `prev_end`
                // at the `;` hands the comments to the next statement's leading run;
                // advancing it (what the trailing case does below) would DROP them.
                prev_end = stmt_end;
            } else if body_has_comments {
                // Bound at the next *printed* statement, skipping dropped `;`s, so a
                // same-line comment trailing a dropped `;` (`a();; // c`) attaches here
                // rather than being stranded (the `;` emits nothing to carry it).
                let next_start = next_printed_stmt_start(body, i, body_end);
                body_parts.extend(self.build_trailing_same_line_comment_docs(stmt_end, next_start));
                // Update prev_end past trailing comments (including comments on the
                // closing */ line of multi-line block comments)
                prev_end = self.find_end_with_trailing_comments(stmt_end);
            } else {
                prev_end = stmt_end;
            }
            prev_stmt_end = Some(stmt_end);
        }

        StatementListTail {
            prev_end,
            last_stmt_end: prev_stmt_end,
            claims_trailing: prev_deferred_line_comment,
        }
    }

    /// Collect leading comments for a statement, filtering out the ones the previous
    /// statement's trailing emitter already took ([`Printer::comment_already_trailed`] —
    /// `claims_trailing` is that predicate's escape hatch).
    pub(in crate::printer) fn collect_leading_comments(
        &self,
        prev_end: u32,
        stmt_start: u32,
        prev_stmt_end: Option<u32>,
        claims_trailing: bool,
    ) -> CommentVec<'_> {
        let collected: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, prev_end, stmt_start)
                .filter(|c| !self.comment_already_trailed(prev_stmt_end, c, claims_trailing))
                .collect();
        #[cfg(feature = "buffer_stats")]
        crate::printer::buffer_stats::record_leading_comments(collected.len());
        collected
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
        // Note: expand_empty=false because outer comments will expand the block anyway.
        // This helper is never used for a static block, so the body is always
        // directive-prologue eligible.
        self.build_block_body_doc(block, false, leading_content, true)
    }
}
