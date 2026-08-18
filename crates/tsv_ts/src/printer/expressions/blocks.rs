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
use crate::printer::statements::StatementContext;
use crate::printer::{CommentVec, Printer, is_effectively_empty_body};
use tsv_lang::Span;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// A statement list's **blank** cursor, which is not its comment cursor.
///
/// The two part over one thing and one thing only: a dropped `EmptyStatement`. The comment
/// cursor advances past its `;` (the statement-gap seam's slot floor — a dropped `;` closes
/// its slot, `docs/comments.md`), while prettier's `printStatementSequence` skips an
/// `EmptyStatement` outright and asks `isNextLineEmpty` of the previous **printed**
/// statement, so the `;` may neither move the anchor nor be scanned past. Measuring from the
/// shared cursor was wrong in both directions at once — `a();⏎⏎;⏎b();` lost the author's
/// blank (it sits above the `;`, behind the cursor) and `a();⏎;⏎⏎b();` gained one prettier
/// drops (it sits below).
///
/// One type rather than a pair of locals per loop because the rule has three moving parts
/// and **two** copies of the walk (the program/block/namespace list and the switch
/// consequent's own): a hand-rolled second copy is how the two would drift, which is the
/// bug class this whole area is about.
pub(in crate::printer) struct StatementBlankScan {
    /// End of the last thing this list actually PRINTED.
    anchor: u32,
    /// The first dropped `;` since the anchor last moved; `u32::MAX` when none is in the way,
    /// so an ordinary gap keeps whatever bound its own comment scan picks.
    bound: u32,
}

impl StatementBlankScan {
    pub(in crate::printer) fn new(start: u32) -> Self {
        Self {
            anchor: start,
            bound: u32::MAX,
        }
    }

    /// Where the blank question measures FROM.
    pub(in crate::printer) fn anchor(&self) -> u32 {
        self.anchor
    }

    /// The caller's own upper bound, capped at a dropped `;` in the way.
    pub(in crate::printer) fn bound(&self, check_end: u32) -> u32 {
        check_end.min(self.bound)
    }

    /// Content reached the output (a statement, or a dropped `;`'s orphan comment run), so
    /// the two cursors rejoin and nothing is in the way any more.
    pub(in crate::printer) fn printed(&mut self, end: u32) {
        self.anchor = end;
        self.bound = u32::MAX;
    }

    /// A dropped `;` that printed nothing: it leaves the anchor alone and becomes the bound.
    ///
    /// Two guards, each of which a fixture found. It must **start a line** — one the author
    /// left on the previous statement's own line (`a();; // c`) is trivia prettier walks
    /// straight over, its `skipToLineEnd` being `skip(",; \t")`, and bounding there would
    /// drop the blank BELOW that line, which is the previous statement's own. And it must lie
    /// **ahead** of the anchor — a `;` the previous statement's trailing run already consumed
    /// sits behind it, where bounding inverts the range and every later blank reads as absent.
    pub(in crate::printer) fn skipped_semi(&mut self, printer: &Printer<'_>, semi_start: u32) {
        if semi_start >= self.anchor && !printer.is_same_line(self.anchor, semi_start) {
            self.bound = self.bound.min(semi_start);
        }
    }
}

/// What [`Printer::build_statement_list_docs_into`] leaves for its caller's end-of-body
/// comment emitter.
pub(in crate::printer) struct StatementListTail {
    /// Source cursor past everything the walk emitted — and therefore the position the
    /// caller's end-of-body run must open at. It is not `last_stmt_end` advanced past that
    /// statement's trailing comments: a body ending in dropped `;`s leaves the cursor past
    /// them, which is what keeps a blank the author left before a `;` from reading as a
    /// blank before the trailing run.
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
                    d.indent_hardline(d.concat(&all_content)),
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
        if tail.last_stmt_end.is_some() {
            // The walk's own cursor, never a recomputed `find_end_with_trailing_comments`
            // over the last PRINTED statement: the two agree everywhere except a body
            // ending in dropped `;`s, where the recompute rewinds behind them and reads a
            // blank the author left *before* a `;` as a blank before this run
            // (`{ a();⏎⏎;⏎/* c */ }` gained a blank line prettier drops — and the program
            // walk, which already used the cursor, did not). It also carries the deferred
            // case for free: a last statement that deferred a line comment past its own
            // `;` left the cursor AT that `;`, which is where this run's anchor has to
            // stand — advancing past the very comments it must print would drop them.
            body_parts.extend(self.build_trailing_body_comments_doc(
                tail.prev_end,
                block_end,
                tail.claims_trailing,
            ));
        }

        d.concat(&[
            d.text("{"),
            d.concat(&brace_line_prefix),
            d.indent_hardline(d.concat(&body_parts)),
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
        // The BLANK cursor, which `prev_end` above cannot serve — see [`StatementBlankScan`].
        let mut blanks = StatementBlankScan::new(body_start);
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
                    // A comment hugging the next printed statement's start leads it
                    // (`a(); ; /* c */ b();`) — the orphan run must stop at the claim
                    // split so that statement's leading run still finds it.
                    let claim_end = self.statement_claim_end(body, i, None);
                    self.find_end_with_trailing_comments(stmt_end)
                        .min(next_start)
                        .min(claim_end)
                } else {
                    stmt_end
                };

                // `prev_deferred_line_comment` reaches the ORPHAN run too, the way it does
                // the printing run below: the previous statement deferred a line comment
                // past its own `;` and so trailed NOTHING on that line, which leaves this
                // run its only emitter. Answering `false` here made the filter call the
                // gap's comments already-trailed and skip them — a DROP, since the dropped
                // `;` puts no leading run after them either
                // (`a = 1 // c1⏎; /* c2 */ ;⏎b = 2;`). The switch consequent's own orphan
                // arm has always passed it; this copy is the one that drifted.
                let mut leading_comments = if body_has_comments {
                    self.collect_leading_comments(
                        prev_end,
                        search_end,
                        prev_stmt_end,
                        prev_deferred_line_comment,
                        None,
                    )
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
                // The orphan run is content and moves the blank anchor exactly as a
                // statement does; a `;` that printed nothing becomes the bound instead.
                if leading_comments.is_empty() {
                    blanks.skipped_semi(self, stmt.span().start);
                } else {
                    blanks.printed(search_end);
                }
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
                    Some(stmt_start),
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
                    self.blank_scan_end(blanks.anchor(), stmt_start)
                } else {
                    stmt_start
                };
                // ⚠️ A dropped `;` CAPS the scan as well as failing to move its start —
                // the two halves of one question ([`StatementBlankScan`]): what did the
                // author write between the last printed statement and the next thing in
                // the gap? The `;` is such a thing, exactly as a comment is
                // (`blank_scan_end`'s own rule). Capping the table-based count rather than
                // switching to prettier's byte-scanning `isNextLineEmpty` keeps the
                // line-break TABLE as the oracle, which is what sees U+2028 / U+2029 as
                // line terminators — the byte scan does not, and
                // `syntax/whitespace/line_terminators` is the fixture that says so.
                if self.has_blank_line_between(blanks.anchor(), blanks.bound(blank_line_check_end))
                {
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
                body_parts.push(self.build_statement_doc(
                    stmt,
                    if in_program_or_block {
                        StatementContext::PROGRAM_OR_BLOCK
                    } else {
                        StatementContext::OTHER_LIST
                    },
                ));
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
                // The shared trailing arm of the statement-gap seam: the run bounded at
                // the next printed statement and stopped at the claim split, the cursor
                // clamped to the same split ([`Printer::statement_trailing_run`]) — so
                // a handed-over comment stays ahead of it for the next statement's
                // leading run to find.
                let (trailing, new_prev_end) = self.statement_trailing_run(body, i, body_end);
                body_parts.extend(trailing);
                prev_end = new_prev_end;
            } else {
                prev_end = stmt_end;
            }
            blanks.printed(prev_end);
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
    ///
    /// `leads_target` is `Some(stmt_start)` when the statement at `stmt_start` actually
    /// prints — a comment hugging its start then leads it even from the previous
    /// statement's line ([`Printer::comment_leads_next_item`], the trailing claim's
    /// other half) — and `None` for an orphaned run (a dropped `;`'s comments), where
    /// nothing follows to lead.
    pub(in crate::printer) fn collect_leading_comments(
        &self,
        prev_end: u32,
        stmt_start: u32,
        prev_stmt_end: Option<u32>,
        claims_trailing: bool,
        leads_target: Option<u32>,
    ) -> CommentVec<'_> {
        let collected: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, prev_end, stmt_start)
                .filter(|c| {
                    !self.comment_already_trailed(prev_stmt_end, c, claims_trailing, leads_target)
                })
                .collect();
        #[cfg(feature = "buffer_stats")]
        crate::printer::buffer_stats::record_leading_comments(collected.len());
        collected
    }
}
