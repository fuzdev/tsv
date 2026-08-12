// Program-level printing for TypeScript
//
// Top-level orchestration: statement iteration with blank-line preservation,
// leading/trailing comment placement, and format-ignore raw emission.

use crate::ast::internal;
use tsv_lang::Span;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

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

        // The shared statement-list walk — the same one every block, function body and
        // `namespace` body takes. A program differs from those only in its BOUNDS and in
        // having no opening delimiter: the body starts at byte 0 rather than past a `{`,
        // it hoists nothing above the first statement (`has_leading: false`, which is what
        // makes the walk's first-statement arm emit no separator — the program's own rule)
        // and it pulls no comment onto a brace line (`delimiter_pull_pos: None`).
        //
        // Keeping a second copy of this loop here is what let the two drift: the copy's
        // leading run answered the separator question from the ITEM rather than from the
        // source after each `*/`, and its blank scan was emit-keyed where the shared one is
        // in-source — so a glued run split and an owned comment's own newlines read as an
        // author blank, at the top level only.
        let tail = self.build_statement_list_docs_into(
            &mut parts,
            program.body,
            Span::new(0, program.span.end),
            false,
            None,
            true,
        );

        // Trailing program comments
        let mut has_output = tail.last_stmt_end.is_some();
        let trailing_comments_doc = self.build_program_trailing_comments_doc(
            tail.prev_end,
            tail.claims_trailing,
            has_output,
        );
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

    /// Build docs for trailing comments at the end of the program
    ///
    /// Handles comments that appear after all statements but before end of file — the
    /// `}`-less end-of-body run, so it is the shared
    /// [`Printer::build_trailing_body_comments_doc`] with the source length as its bound
    /// (equivalent by construction to an unbounded scan: `self.comments` is already
    /// island-local for an embedded `<script>`).
    ///
    /// `claims_trailing` says this run owns the comments sharing `prev_end`'s source line —
    /// set when the last statement deferred a line comment past its own `;`
    /// (`terminator_defers_line_comment`), so it trailed nothing on that line and, with no
    /// further statement to lead, this emitter is their last chance to be printed.
    ///
    /// ⚠️ `has_output` is the program's own answer to "is there a previous item", the
    /// question the shared emitter otherwise derives from `prev_end > 0`. That proxy holds
    /// for every `{}` body — their cursor opens at the `{`, and a body's first item always
    /// prints — but a dropped `EmptyStatement` at the head of a program advances the cursor
    /// while printing NOTHING, and the run then breaks away from content that isn't there
    /// (a blank first line, and a form the reparse doesn't reproduce). So with no output
    /// the cursor is handed back as 0, the emitter's "no previous item at all" state.
    /// Sound because nothing printed means nothing was skipped either — an orphan run that
    /// finds a comment always emits it — so `[0, prev_end)` provably holds no comments the
    /// widened scan could double-print.
    fn build_program_trailing_comments_doc(
        &self,
        prev_end: u32,
        claims_trailing: bool,
        has_output: bool,
    ) -> DocBuf {
        let cursor = if has_output { prev_end } else { 0 };
        self.build_trailing_body_comments_doc(cursor, self.source.len() as u32, claims_trailing)
    }
}
