// Program-level printing for TypeScript
//
// Top-level orchestration: statement iteration with blank-line preservation,
// leading/trailing comment placement, and format-ignore raw emission.

use crate::ast::internal;
use tsv_lang::doc::DocBuf;
use tsv_lang::{
    CommentPosition, classify_comment_fast, comments_to_emit_in_range, doc::arena::DocId,
};

use super::Printer;

impl<'a> Printer<'a> {
    /// Print a TypeScript program
    ///
    /// Delegates to `build_program_doc` to build the doc tree, then renders it.
    /// This is the same path used by Svelte's `<script>` formatting, ensuring
    /// consistent behavior (e.g., trailing whitespace trimming in comments).
    pub(crate) fn print_program(&mut self, program: &internal::Program<'_>) {
        let doc = self.build_program_doc(program);
        self.write_arena_doc(doc);
    }

    /// Build a DocId tree for a TypeScript program
    ///
    /// Returns a DocId that can be wrapped with `indent()` and rendered.
    /// Used both for standalone TS/JS formatting (via `print_program`) and
    /// when embedding TypeScript in other formats like Svelte's `<script>`.
    ///
    /// The Doc structure preserves:
    /// - Statement separation with hardline
    /// - Blank line preservation between statements using literalline
    /// - Leading comments with proper spacing
    /// - Trailing same-line comments using line_suffix
    /// - Program trailing comments after the last statement
    pub(crate) fn build_program_doc(&self, program: &internal::Program<'_>) -> DocId {
        let d = self.d();
        let mut parts = d.pooled_docbuf();
        let mut prev_end = 0u32;
        let mut has_output = false;
        // Set when the statement just emitted deferred a line comment past its own `;`, so
        // its doc ends on a later line than the `;` and cannot carry that line's comments.
        let mut prev_deferred_line_comment = false;

        for (stmt_idx, statement) in program.body.iter().enumerate() {
            // Skip standalone EmptyStatements but preserve blank lines and comments around them
            if matches!(statement, internal::Statement::EmptyStatement(_)) {
                // Extend the search range to include trailing same-line comments of the
                // empty statement. Without this, `; /* comment */` loses the comment.
                let stmt_end = statement.span().end;
                let trailing_end = self.find_end_with_trailing_comments(stmt_end).max(stmt_end);
                // Use the extended range (covers same-line trailing comments) but cap at
                // next statement's start to avoid capturing comments that belong to the next stmt.
                let next_start = program
                    .body
                    .get(stmt_idx + 1)
                    .map_or(program.span.end, |s| s.span().start);
                // A comment hugging the next printed statement's start leads it
                // (`a(); ; /* c */ b();`) — this orphan scan must stop at the claim
                // split so that statement's leading run still finds it.
                let claim_end = self.statement_claim_end(program.body, stmt_idx, None);
                let search_end = trailing_end.max(stmt_end).min(next_start).min(claim_end);

                // Force non-inline: since we're skipping the semicolon, any "inline" comments
                // (on same line as the semicolon) have nothing to be inline with
                let comments_doc =
                    self.build_leading_comments_doc(prev_end, search_end, !has_output, true);
                if let Some(comments_doc) = comments_doc {
                    if has_output {
                        // Check for blank line before the first comment (same as regular
                        // statements). The scan is over source *bytes*, so it must stop at
                        // the first comment physically in the gap — including one this gap
                        // does not emit (an owned annotation), whose own newlines would
                        // otherwise read as an authored blank line.
                        let first_comment_start =
                            tsv_lang::comments_in_source_range(self.comments, prev_end, search_end)
                                .next()
                                .map(|c| c.span.start);
                        let check_end = first_comment_start.unwrap_or_else(|| statement.span().end);

                        if self.has_blank_line_between(prev_end, check_end) {
                            parts.push(d.literalline()); // Blank line at column 0
                        }
                        parts.push(d.hardline()); // Separator with indent
                    }
                    parts.push(comments_doc);
                    has_output = true;
                }
                prev_end = search_end;
                continue;
            }

            // Separator between statements
            if has_output {
                // Check for blank line before the next item — up to the first comment
                // physically in the gap (see the EmptyStatement arm above), else up to the
                // statement.
                let check_end = self.blank_scan_end(prev_end, statement.span().start);

                if self.has_blank_line_between(prev_end, check_end) {
                    parts.push(d.literalline()); // Blank line at column 0
                }

                parts.push(d.hardline()); // Separator with indent
            }

            // Leading comments (allow inline comments since statement will be printed)
            let has_ignore = self.member_gap_frozen(prev_end, statement.span().start);
            if let Some(leading_doc) = self.build_leading_comments_doc(
                prev_end,
                statement.span().start,
                !has_output || prev_deferred_line_comment,
                false,
            ) {
                parts.push(leading_doc);
            }

            // Statement — if preceded by a format-ignore directive, emit raw source.
            // A Program's body is always directive-prologue eligible. The freeze emitter
            // (not the bare slice) claims the block comment **glued** before the statement:
            // that comment is owned by whatever node heads it, rides inside the doc the
            // slice replaces, and is skipped by the leading run above — so nothing else
            // prints it (docs/comments.md hazard 1).
            if has_ignore {
                parts.push(self.build_frozen_node_doc(statement.span()));
            } else {
                parts.push(self.build_statement_doc(statement, true));
            }

            // Trailing same-line comments — unless this statement deferred a line comment
            // past its own `;`, in which case its doc ends with that comment and nothing
            // may share the line (`terminator_defers_line_comment`). Those comments lead
            // the NEXT statement instead, which is what leaving `prev_end` at the `;`
            // hands over — advancing it (what the trailing case does, to keep blank-line
            // detection honest) would DROP them, since this emitter skipped them and the
            // leading run would start past them.
            let span = statement.span();
            prev_deferred_line_comment = self.terminator_defers_line_comment(span.start, span.end);
            if prev_deferred_line_comment {
                prev_end = span.end;
            } else {
                // The shared trailing arm of the statement-gap seam: the run bounded at
                // the next printed statement and stopped at the claim split, the cursor
                // clamped to the same split ([`Printer::statement_trailing_run`]) — so
                // blank-line detection stays honest and a handed-over comment stays
                // ahead of the cursor for the next statement's leading run.
                let (trailing, new_prev_end) =
                    self.statement_trailing_run(program.body, stmt_idx, program.span.end);
                parts.extend(trailing);
                prev_end = new_prev_end;
            }
            has_output = true;
        }

        // Trailing program comments
        let trailing_comments_doc =
            self.build_program_trailing_comments_doc(prev_end, prev_deferred_line_comment);
        if !trailing_comments_doc.is_empty() {
            has_output = true;
        }
        parts.extend(trailing_comments_doc);

        // Trailing newline (only if there's content — empty files stay empty)
        if has_output {
            parts.push(d.hardline());
        }

        d.concat(&parts)
    }

    /// Build doc for leading comments between prev_end and curr_start
    ///
    /// Returns a Doc containing all leading comments with proper blank line handling.
    /// Returns empty doc if no comments.
    ///
    /// Structure: Each comment is output WITHOUT a trailing hardline.
    /// Separators (hardline or literalline+hardline) are added BEFORE each subsequent
    /// comment and AFTER the last comment (to separate from the statement).
    ///
    /// When `force_non_inline` is true, all comments are treated as non-inline (own line).
    /// This is used for empty statements that will be skipped - their inline comments
    /// have nothing to be inline with.
    /// `claims_trailing` says this run owns the comments sharing `prev_end`'s source line.
    /// Normally the previous statement's trailing emitter takes them, so they are skipped
    /// here; two callers claim them instead — the file-start run (there is no previous
    /// statement) and a run whose previous statement deferred a line comment past its own
    /// `;` (`terminator_defers_line_comment`), whose doc therefore ends on a later line and
    /// cannot carry them. Skipping them in BOTH places is a dropped comment.
    fn build_leading_comments_doc(
        &self,
        prev_end: u32,
        curr_start: u32,
        claims_trailing: bool,
        force_non_inline: bool,
    ) -> Option<DocId> {
        let d = self.d();
        let mut parts = d.pooled_docbuf();
        let mut last_comment_end = prev_end;
        let mut printed_any = false;
        let mut last_was_inline = false;

        for comment in comments_to_emit_in_range(self.comments, prev_end, curr_start) {
            let position =
                classify_comment_fast(comment, prev_end, curr_start, self.comment_line_breaks);

            // Skip trailing comments unless this run claims them (see `claims_trailing`)
            // — or unless the comment LEADS the statement ([`Self::comment_leads_next_item`]):
            // the previous statement's trailing claim stopped at it
            // ([`Self::trailing_claim_end`]), so this run is its only emitter.
            // `force_non_inline` marks the orphan context (a dropped `;`), where nothing
            // prints at `curr_start` and nothing can be led.
            if !claims_trailing
                && matches!(position, CommentPosition::Trailing)
                && (force_non_inline || !self.comment_leads_next_item(comment, curr_start))
            {
                last_comment_end = comment.span.end;
                continue;
            }

            // Handle inline leading comments (same line as statement)
            // These stay on the same line, so DON'T set printed_any (no separator needed)
            // Skip this behavior when force_non_inline is true (e.g., empty statements being skipped)
            //
            // Also handle any block comment whose glue chain reaches curr_start
            // ([`Self::comment_leads_next_item`]) regardless of classified position:
            // consecutive inline blocks at file start (`/** @type {A} */ /** @type {B} */
            // expr;` — classify_comment_fast returns Trailing, same line as prev_end=0), a
            // comment the previous statement's trailing claim handed over
            // (`a(); /* c */ let b = 1;`), and a chain head classified LeadingOwnLine
            // because its own end line differs from curr_start's
            // (`/* c */ /* x⏎y */ b();` — the multi-line tail is OWNED by `b` and rides
            // inside its doc, so this run sees only the head and must still glue it).
            // A Trailing comment that does NOT lead was already skipped above.
            let is_inline = matches!(position, CommentPosition::LeadingInline)
                || (comment.is_block && self.comment_leads_next_item(comment, curr_start));
            if !force_non_inline && is_inline {
                // If a previous comment was printed on a DIFFERENT line, add a line break.
                // E.g., `// line comment\n/** @type {A} */ expr;` — needs newline after
                // the line comment. But consecutive inline comments on the SAME line
                // should stay inline: `/** @type {A} */ /** @type {B} */ expr;`.
                if printed_any && !self.is_same_line(last_comment_end, comment.span.start) {
                    let has_blank = comment.span.start > last_comment_end
                        && self.has_blank_line_between(last_comment_end, comment.span.start);
                    if has_blank {
                        parts.push(d.literalline());
                    }
                    parts.push(d.hardline());
                }
                parts.push(self.build_comment_doc(comment));
                parts.push(d.text(" "));
                // DON'T set printed_any - inline comments don't need separators
                last_comment_end = comment.span.end;
                last_was_inline = true;
                continue;
            }

            // Comment on its own line: check for blank lines BETWEEN comments
            // Note: blank line before FIRST comment is handled by the parent (build_program_doc)
            // We only handle blank lines between subsequent comments here
            //
            // Special case: when the previous comment was a multi-line block comment,
            // a comment on the same line as its closing */ stays inline (e.g.,
            // `/*\ncomment\n*/ /* after */` keeps `/* after */` on the `*/` line).
            if printed_any && self.is_same_line(last_comment_end, comment.span.start) {
                // Same line as previous comment's end — keep inline
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else {
                let has_blank_before = printed_any
                    && comment.span.start > last_comment_end
                    && self.has_blank_line_between(last_comment_end, comment.span.start);

                // Add separator BEFORE this comment (first comment has no separator - parent's hardline handles it)
                if has_blank_before {
                    parts.push(d.literalline()); // Blank line at column 0
                    parts.push(d.hardline()); // Indent for this comment
                } else if printed_any {
                    parts.push(d.hardline()); // Separator from previous comment
                }

                parts.push(self.build_comment_doc(comment));
            }
            // NO hardline after comment - let post-loop or next iteration handle it

            last_comment_end = comment.span.end;
            printed_any = true;
            last_was_inline = false;
        }

        // After all comments: add separator for the statement (if one follows)
        // Skip this when force_non_inline is true - that means the statement is being skipped
        // and there's nothing for the separator to separate from.
        // Skip when last comment was inline - it already has trailing space and the
        // statement continues on the same line: `/** @type {A} */ expr;`
        if printed_any && !force_non_inline && !last_was_inline {
            // Check if there's a blank line after the last comment
            let has_blank_after = last_comment_end < curr_start
                && self.has_blank_line_between(last_comment_end, curr_start);

            if has_blank_after {
                parts.push(d.literalline()); // Blank line at column 0
            }
            parts.push(d.hardline()); // Indent for statement
        }

        if parts.is_empty() {
            None
        } else {
            Some(d.concat(&parts))
        }
    }

    /// Build docs for trailing comments at the end of the program
    ///
    /// Handles comments that appear after all statements but before end of file — the
    /// `}`-less end-of-body run, so it is the shared
    /// [`Printer::build_trailing_body_comments_doc`] with the source length as its bound
    /// (equivalent by construction to an unbounded scan: `self.comments` is already
    /// island-local for an embedded `<script>`). Its one distinguishing state, a
    /// comments-only file with no previous statement, is the `prev_end == 0` the shared
    /// emitter reads.
    ///
    /// `claims_trailing` says this run owns the comments sharing `prev_end`'s source line —
    /// set when the last statement deferred a line comment past its own `;`
    /// (`terminator_defers_line_comment`), so it trailed nothing on that line and, with no
    /// further statement to lead, this emitter is their last chance to be printed.
    fn build_program_trailing_comments_doc(&self, prev_end: u32, claims_trailing: bool) -> DocBuf {
        self.build_trailing_body_comments_doc(prev_end, self.source.len() as u32, claims_trailing)
    }
}
