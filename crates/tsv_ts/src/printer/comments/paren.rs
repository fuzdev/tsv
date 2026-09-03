// Stripped-grouping-paren comment handling.
//
// When the parser strips redundant grouping parens, comments that lived inside
// them are orphaned in the source. These helpers preserve such comments in the
// user's position — trailing the expression, promoted before `=` / an operator,
// re-added with the parens when stripping would relocate them, or prepended at a
// chain base.

use super::{CommentSpacing, CommentVec, LeadingGlue, Printer, RunLeadingBlank};
use crate::ast::internal;
use crate::printer::ParenContext;
use crate::printer::expressions::operators::SeqLayout;
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::{TriviaProfile, find_char_skipping_comments, skip_trivia};

/// What follows a stripped grouping shell, and so whether a trailing block comment has a
/// statement terminator to defer past.
///
/// The distinction cannot be read off the source at the shell: both spellings put a `;`
/// there, and only the caller knows whether it TERMINATES a statement or SEPARATES a
/// clause. Reading the byte alone sent a `for` header's init comment past the header's
/// own separator, out of the declarator that owned it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ShellTail {
    /// A statement value position — a declarator initializer, an assignment RHS, a
    /// `return` / `throw` argument, `export default`. A following `;` ends the statement,
    /// so a trailing block defers past it (see [`Printer::build_expression_doc_with_paren_comments`]).
    StatementTerminator,
    /// A `for` header's init clause. A following `;` separates clauses, so nothing defers.
    ForClauseSeparator,
}

/// How [`Printer::build_paren_leading_value_doc`] splits a `(`→value gap: the run the
/// `(` line keeps, the value with the rest of the run prepended, and whether the caller
/// must open its parens.
///
/// A struct rather than a tuple because the three travel together to two callers (the
/// dynamic-import EXPRESSION and the TS import TYPE) and two of them are trivially
/// swappable at a call site.
pub(crate) struct ParenLeadingValue {
    /// Emitted by the caller directly after its `(`, before any break. Empty unless a
    /// comment the author left on the `(` line was pulled onto it.
    pub paren_line: DocBuf,
    /// `value_doc` with the leading run that does *not* ride the `(` line prepended.
    pub value: DocId,
    /// Whether the gap ends a line unconditionally.
    pub forces_break: bool,
}

/// Whether a REQUIRED paren pair around `expr` — at a call callee or a tagged
/// template's tag — keeps a leading run INSIDE it.
///
/// A function or arrow is the one operand kind prettier prints the comments inside
/// those parens for (`isIifeCalleeOrTaggedTemplateExpressionTag` feeding its
/// `printCommentsForFunction`, `src/language-js/print/index.js`); every other
/// required pair at those positions — a ternary, a sequence, a cast, a class
/// expression, a `new` callee (whose parent is not a call at all) — takes the run
/// out in front, and tsv matches. Read by the three positions that print such a
/// pair: the bare callee, the tag, and the chain's IIFE base.
pub(crate) fn paren_pair_keeps_leading_run(expr: &internal::Expression<'_>) -> bool {
    matches!(
        expr,
        internal::Expression::ArrowFunctionExpression(_)
            | internal::Expression::FunctionExpression(_)
    )
}

/// The index of the next byte in `source` that is neither whitespace nor trivia, at or
/// after `start` and before `end`; `None` when the range holds nothing else.
///
/// The "what actually comes next in the source?" step every shell question asks. A
/// comment occupies bytes even where nothing emits it, so a walk that stepped over
/// whitespace alone would stop on the `/` and answer about the wrong token.
///
/// A free function over the source because the chain LINEARIZER asks it too, before any
/// `Printer` method is reached — [`paren_shell_close_after`].
pub(crate) fn next_significant_byte(source: &str, start: u32, end: u32) -> Option<usize> {
    let bytes = source.as_bytes();
    let end = end as usize;
    let mut i = start as usize;
    while i < end {
        if let Some(next) = skip_trivia(bytes, i, end, TriviaProfile::JS) {
            i = next;
        } else if bytes[i].is_ascii_whitespace() {
            i += 1;
        } else {
            return Some(i);
        }
    }
    None
}

/// The index just past the `)` that closes a REQUIRED pair around an operand ending at
/// `operand_end`, when the author wrote that pair — `None` when the next thing in the
/// source is something else (a `(` of an argument list, a `` ` ``, a `.`), which means
/// the pair this position prints is tsv's own and the source has no trailing gap inside
/// it.
///
/// The distinction is load-bearing: `(fn /* t */)()` writes the pair around the callee
/// and the comment is INSIDE it, while `(/* t */ fn())` writes it around the whole call
/// and the comment belongs to the call, not to a pair the callee prints.
pub(crate) fn paren_shell_close_after(source: &str, operand_end: u32) -> Option<u32> {
    let bytes = source.as_bytes();
    let pos = next_significant_byte(source, operand_end, bytes.len() as u32)?;
    (bytes[pos] == b')').then_some(pos as u32 + 1)
}

impl<'a> Printer<'a> {
    /// Split the comments between an opening `(` and the value that follows into the run
    /// the `(` LINE keeps and the run that LEADS the value, and report whether the caller
    /// must open its parens ([`Printer::push_leading_comment_run`]'s own report — tsv
    /// has no `propagateBreaks`, so a hardline in here is invisible to the group).
    ///
    /// ⚠️ **The `(`-line share is the call family's, and `import(…)` is a call shape.**
    /// A `//` the author parked after the `(` stays there in every other spelling —
    /// `fn( // c`, `new Foo( // c`, `obj.fn( // c`, `require( // c`, a cataloged
    /// divergence from prettier, which relocates it to the argument's leading line — and
    /// both `import(…)` spellings take the same answer (a dissent there would be an
    /// unconsidered difference, not a decision). Routing the share through
    /// [`Printer::delimiter_line_comment_prefix`] is what makes it one rule: the same
    /// pull, the same "only when the shell breaks anyway" gate, and the same
    /// pulled-comment exclusion. Pinned by
    /// `expressions/calls/import_open_paren_comment_prettier_divergence`, which carries
    /// both spellings because they share this seam.
    ///
    /// The gap is a call-argument leading run — `import(…)`'s first argument, for both
    /// the dynamic-import EXPRESSION and the TS import TYPE — so it takes the argument
    /// list's mode and the shared emitter decides its separators per comment.
    ///
    /// ⚠️ **The three-way rule cannot be approximated by a whole-range scan**, which is
    /// what this site did before: it asked "is there a newline anywhere between the `(`
    /// and this comment", so the second comment of a run the author GLUED
    /// (`import(⏎/* a */ /* b */⏎'./a')`) read as own-line, split the pair, and forced
    /// the parens open — where prettier keeps the pair and, for a run with no line
    /// comment, collapses the whole call. Own-line-ness is per comment and anchored on
    /// its own neighbours ([`Printer::is_own_line_comment`]), and the author blank the
    /// old loop's bare `hardline` deleted is likewise the emitter's to preserve.
    pub(crate) fn build_paren_leading_value_doc(
        &self,
        open_paren_end: u32,
        value_start: u32,
        value_doc: DocId,
    ) -> ParenLeadingValue {
        // The `(`-LINE share, on the shared delimiter-line rule: pull a comment the author
        // left on the `(` line onto that line only when the shell is breaking anyway, so a
        // call that would have fit still fits (`docs/comments.md` §The delimiter-line
        // question). `pull_pos` is the exclusion every consumer of that prefix owes.
        let (paren_line, pull_pos) = match self.paren_line_share_anchor(open_paren_end, value_start)
        {
            Some(paren) => {
                let (prefix, pull_pos) = self.delimiter_line_comment_prefix(paren, value_start);
                // The field stays a buffer (its two consumers read a slice and gate on
                // `is_empty`); the `Option` collects into it only at this cold site.
                (prefix.into_iter().collect(), pull_pos)
            }
            None => (DocBuf::new(), None),
        };
        let run = self.build_leading_comment_run_with_break(
            open_paren_end,
            value_start,
            LeadingGlue::AdjacentStrippedParen,
            pull_pos,
        );
        let (value, run_breaks) = match run {
            Some((run, force_break)) => (self.d().concat(&[run, value_doc]), force_break),
            None => (value_doc, false),
        };
        ParenLeadingValue {
            // A pull is only ever made on the break path, so it reports the break itself —
            // the pulled run is no longer in `value` for the run's own report to see.
            forces_break: run_breaks || pull_pos.is_some(),
            paren_line,
            value,
        }
    }

    /// The `(` a `(`-line share would be claimed against, or `None` when this gap has no
    /// share to claim.
    ///
    /// ⚠️ **The share is what the author wrote AFTER the `(`, so the anchor is the paren
    /// and not the gap's start.** The gap deliberately spans the `(` — one slot, one
    /// emitter, which is what keeps `import /* c */ ('m')` from being dropped — but a
    /// comment written *before* the paren belongs to the head, and the claim has to stay a
    /// **PREFIX** of the gap's comments (`docs/comments.md` §The element-comma seam):
    /// claiming only the after-`(` tail would render it AHEAD of the head comment it was
    /// written under. So a run that begins before the `(` is not split — it stays whole in
    /// the leading position, which is where both formatters already put it
    /// (`import /* pre */ ( // post`, the null control in
    /// `expressions/calls/import_open_paren_comment_prettier_divergence`).
    fn paren_line_share_anchor(&self, gap_start: u32, value_start: u32) -> Option<u32> {
        let paren = find_char_skipping_comments(
            self.source.as_bytes(),
            gap_start as usize,
            value_start as usize,
            b'(',
        )? as u32;
        comments_to_emit_in_range(self.comments, gap_start, value_start)
            .next()
            .is_some_and(|first| first.span.start > paren)
            .then_some(paren)
    }

    /// Append the trailing comments in an operand's closing gap to a parts vec.
    ///
    /// The gap holds comments that belong to no node, so some caller has to place them.
    /// It arises both ways round: the parser *strips* grouping parens (`await (x /* c */)`
    /// → the arg is `x`, orphaning `/* c */` before the expression's span end), and the
    /// restricted-production hanging layout *retains* them (the comment prints inside the
    /// parens, bounded by the `)` rather than the span end). Layout per comment:
    /// - Glued to the operand's line, block: inline with leading space (`x /* c */`)
    /// - Own line: deferred via `line_suffix` with a hardline, keeping an author blank
    ///   above it (`x;\n\n/* c */`) — prettier's `printTrailingComment`, its
    ///   `hasNewline(…, { backwards: true })` + `isPreviousLineEmpty` arm
    /// - Anything else: deferred via `line_suffix` on the previous comment's line
    ///   (`x; // c`, `x;\n/* c1 */ /* c2 */`)
    ///
    /// ⚠️ **The question is the SOURCE, asked per comment, and only then the kind.** The
    /// anchor advances over every comment emitted here, so the second half of a run the
    /// author glued (`x⏎/* c1 */ /* c2 */`) is not own-line and keeps that line; a fixed
    /// `argument_end` anchor read it across `c1` and split the pair. Asking the KIND
    /// first is the mirror-image formulation `docs/comments.md` §Trailing and dangling
    /// runs names: it gave an own-line `//` the inline suffix, WELDING it onto the
    /// previous comment's output line (`x; // c1 // c2` — the second delimiter becomes
    /// text inside the first, and the comment stops existing).
    ///
    /// ⚠️ **A comment glued BEHIND a deferred one is deferred too.** Deferral is what
    /// carries this run past the terminator, so an inline block emitted after the run has
    /// started renders *ahead* of it and the authored pair comes out REORDERED. Unlike
    /// [`Printer::push_trailing_comments_in_range`], where only a `//` can open the run
    /// (and nothing can follow one on its line), an own-line **block** opens it here —
    /// this gap floats own-line comments out rather than keeping them inline — so the
    /// glued-behind case is reachable and needs its own arm.
    ///
    /// Keeps a same-line block comment with its operand (before any terminator) — the
    /// expression-level operand callers (await, yield, binary, sequence) where the
    /// comment is inside the stripped operand parens, plus `export =` (which, like
    /// `import =`, keeps a same-line trailing block before the `;`).
    ///
    /// Statement terminators that move the block *after* the `;` — `export default`, and
    /// return/throw's non-hanging paths — use `split_terminator_gap_comments` instead.
    /// return/throw's hanging layout uses **both**: this method for the region inside the
    /// retained parens, then that one for anything past the `)`.
    /// Returns whether the run **deferred** — whether anything went out on a
    /// `line_suffix` rather than inline. A caller whose construct has no break of its own
    /// to flush against needs that answer: a deferred run rides to the end of the output
    /// line, and where that line ends outside the construct the comment re-binds there
    /// (the `{#snippet}` head floated a `//` past `{/snippet}`, into template text). Such
    /// a caller pushes a flush-scoped break on `true` — and only on `true`, since forcing
    /// it for an inline block breaks a construct that had no reason to open, which the
    /// reparse then closes again. Callers that already end the line — the operand shells,
    /// whose deferral lands past their own terminator — ignore it.
    pub(crate) fn append_trailing_paren_comments(
        &self,
        parts: &mut DocBuf,
        argument_end: u32,
        span_end: u32,
    ) -> bool {
        // Whether anything has been deferred yet — a `//`, or an own-line comment.
        let mut deferred_run = false;
        // What physically precedes the next comment: an **in-source** cursor, so it
        // advances over every comment in the gap (docs/comments.md §the three axes).
        let mut prev_end = argument_end;
        for comment in comments_to_emit_in_range(self.comments, argument_end, span_end) {
            // The *comment* line-break table, never the layout one: this decides whether
            // a `//` is followed by a break, so it must stay real under the canonical
            // reprint, where an erased read would weld the run.
            let own_line = self.comment_has_newline_between(prev_end, comment.span.start);
            parts.push(if own_line {
                self.build_trailing_comment_doc_own_line_blank(
                    comment,
                    self.previous_line_is_empty(
                        self.blank_scan_start(prev_end, comment.span.start),
                        comment.span.start,
                    ),
                )
            } else if deferred_run || !comment.is_block {
                self.build_trailing_line_comment_doc(comment)
            } else {
                self.build_trailing_comment_doc(comment)
            });
            deferred_run |= own_line || !comment.is_block;
            prev_end = comment.span.end;
        }
        deferred_run
    }

    /// Split the trailing comments in a statement terminator's content→`;` gap
    /// the way prettier 3.9 does, returning the docs to emit **after** the `;`.
    ///
    /// A same-line **block** comment trails *after* the `;` (`return x; /* c */`) —
    /// *unless* it is still enclosed by a grouping paren around the operand that this
    /// caller PRINTS (`return (a = b /* c */);`), in which case it stays inline before
    /// the `;` (it is attached to the operand, not the statement). `operand_parens_printed`
    /// is that caller fact, and it must be a fact about the OUTPUT, never about the source:
    /// a shell the caller strips leaves a `)` in the source and none in the output, so a
    /// source-keyed carve-out has no fixed point — the next pass sees no shell, reads the
    /// same comment as statement-trailing, and moves it. Line comments (`line_suffix`) and
    /// own-line block comments also trail after the `;`. The inline (operand-attached)
    /// comments are pushed into `parts`; the rest are returned.
    ///
    /// Caller idiom: `let after = self.split_terminator_gap_comments(parts, arg_end,
    /// span_end, keep_operand_line_inline); parts.push(";"); parts.extend(after);`.
    /// Used by return/throw, `export default`, and `export =` — the terminator callers
    /// whose argument may be parenthesized (unlike the expression-statement/var/
    /// class-property terminators, whose operand parens are consumed by inner printers —
    /// they use `push_semicolon_with_gap_comments`).
    ///
    /// `keep_operand_line_inline` is set by callers that render the operand inside
    /// conditional grouping parens (the binary return/throw path). A same-line **line**
    /// comment still enclosed by a stripped grouping paren (`return (a && b // c\n);`) is
    /// operand-attached: keeping it after the `;` would float it out of the parens
    /// (a #18837 over-reach). With the flag set it stays inline before the `)` (pushed to
    /// `parts`); the caller must force the group to break so the line comment never lands
    /// on the flat `expr // c;` path (which would swallow the `;`). Callers that render the
    /// operand bare (no parens) leave the flag `false` — there's nothing to keep it inside.
    ///
    /// `clause_tail` is the statement CONTAINER's deferral fact
    /// (`StatementContext::clause_tail`): `Some(dedent)` when the statement is a
    /// non-block clause body, whose tail line stays open to the enclosing construct —
    /// there each own-line member's interior break is wrapped in `dedent` levels
    /// ([`Printer::build_clause_tail_comment_doc`]), so the freed comment renders at
    /// the flushing construct's level in one pass instead of settling on the reparse.
    /// The same-line arms are indent-free (no interior break), so only the own-line
    /// arm reads it.
    pub(crate) fn split_terminator_gap_comments(
        &self,
        parts: &mut DocBuf,
        argument_end: u32,
        span_end: u32,
        keep_operand_line_inline: bool,
        operand_parens_printed: bool,
        clause_tail: Option<u8>,
    ) -> DocBuf {
        let d = self.d();
        let mut deferred = DocBuf::new();
        // What physically precedes each comment — an **in-source** cursor that ADVANCES
        // over every comment emitted, because own-line-ness is a question about a
        // comment's own neighbours (docs/comments.md §Trailing and dangling runs). Held
        // at `argument_end` it read the second half of an author-glued run across the
        // first, called it own-line, and split the pair onto two lines.
        let mut prev_end = argument_end;
        // Whether anything is already buffered in a `line_suffix`. Once it is, a
        // same-line block must buffer too: an inline emission renders at its doc
        // position — right after the `;` — while the buffer flushes at the line's end,
        // so the pair comes out REORDERED.
        let mut run_deferred = false;
        for comment in comments_to_emit_in_range(self.comments, argument_end, span_end) {
            let same_line = !self.has_newline_between(prev_end, comment.span.start);
            let operand_enclosed =
                same_line && self.gap_has_close_paren(comment.span.end, span_end);
            if comment.is_block && operand_enclosed && operand_parens_printed {
                // Operand-attached (inside stripped parens): `return (x /* c */);`.
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else if !comment.is_block && operand_enclosed && keep_operand_line_inline {
                // Operand-attached line comment (inside stripped parens):
                // `return (a && b // c\n);`. Stays inline before the `)`. Emitted as
                // plain text — the caller's forced break means the following softline
                // becomes the newline before `)`, so the comment never swallows it.
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else if same_line {
                // Statement-trailing, on the line it was written on: a block trails
                // inline after the `;` (prettier 3.9), a line comment via `line_suffix`
                // (`return x; // c`).
                deferred.push(if run_deferred {
                    self.build_trailing_line_comment_doc(comment)
                } else {
                    self.build_trailing_comment_doc(comment)
                });
                run_deferred |= !comment.is_block;
            } else {
                // Own-line — of EITHER kind. The break rides *inside* the `line_suffix`
                // so it replays in run order after the `;`; a bare `hardline` here would
                // render ahead of an already-buffered line comment and reorder the pair,
                // and it dropped an author blank the terminator's own line survives.
                let blank = self.has_blank_line_between(prev_end, comment.span.start);
                deferred.push(match clause_tail {
                    Some(dedent) => self.build_clause_tail_comment_doc(comment, blank, dedent),
                    None => self.build_trailing_comment_doc_own_line_blank(comment, blank),
                });
                run_deferred = true;
            }
            prev_end = comment.span.end;
        }
        deferred
    }

    /// Whether a (comment-skipping) `)` appears in `[start, end)` — i.e. a stripped
    /// grouping paren follows a trailing comment before the terminator, marking the
    /// comment as operand-enclosed rather than statement-trailing.
    pub(crate) fn gap_has_close_paren(&self, start: u32, end: u32) -> bool {
        find_char_skipping_comments(self.source.as_bytes(), start as usize, end as usize, b')')
            .is_some()
    }

    /// Collect comments between a module statement's last content token and its
    /// terminating `;`, returned to emit **after** the `;` (prettier 3.9 — the `;`
    /// is structure; trailing past it is lossless). A same-line block trails inline
    /// (`} /* c */` → `}; /* c */`); a same-line line comment trails via `line_suffix`
    /// (`}; // c`); an own-line comment stays on its own line after the `;`. Module
    /// statements (import/export source, specifiers, attributes) have no operand
    /// parens, so every trailing comment is statement-attached. The caller emits the
    /// `;` right after the content, then calls this to push what follows it.
    pub(crate) fn push_post_semi_comments(&self, deferred: &mut DocBuf, start: u32, end: u32) {
        let d = self.d();
        let mut prev_end = start;
        for comment in comments_to_emit_in_range(self.comments, start, end) {
            let same_line = self.is_same_line(prev_end, comment.span.start);
            if comment.is_block && same_line {
                // Same-line block comment trails inline after the `;`.
                deferred.push(d.text(" "));
                deferred.push(self.build_comment_doc(comment));
            } else if same_line {
                // Trailing line comment: after the `;` via `line_suffix` (zero width).
                deferred
                    .push(d.line_suffix(d.concat(&[d.text(" "), self.build_comment_doc(comment)])));
            } else {
                // Own-line comment (line or block): preserve its own line after the `;`.
                if self.has_blank_line_between(prev_end, comment.span.start) {
                    deferred.push(d.literalline());
                }
                deferred.push(d.hardline());
                deferred.push(self.build_comment_doc(comment));
            }
            prev_end = comment.span.end;
        }
    }

    /// Append trailing comments from stripped grouping parens in spread elements,
    /// excluding own-line comments (which are handled by the parent array/call/object).
    ///
    /// The spread's doc prints only what shares the argument's line: same-line blocks
    /// inline, and the same-line `//` deferred via `line_suffix` so text the parent
    /// appends inline (a comma, an after-comma block) still lands ahead of it. Every
    /// own-line comment — block or line — needs a line of its own that only the parent
    /// can give (a `line_suffix` raised here would escape past the enclosing `]`/`)`),
    /// so the parent picks them up via [`Self::spread_own_line_comments`], in source
    /// order.
    ///
    /// At most ONE comment can defer: a `//` ends its line, so everything after it in
    /// the interior is own-line by construction — which is also what makes the deferred
    /// run structurally weld-free here.
    pub(crate) fn append_spread_trailing_paren_comments(
        &self,
        parts: &mut DocBuf,
        argument_end: u32,
        span_end: u32,
    ) {
        let d = self.d();
        for comment in comments_to_emit_in_range(self.comments, argument_end, span_end) {
            if self.has_newline_between(argument_end, comment.span.start) {
                // Own-line comments (line or block): skip — the parent's share.
                continue;
            }
            if comment.is_block {
                // Same-line block comment: `...x /* c */`
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            } else {
                // Same-line line comment: defer past the argument's own text.
                parts
                    .push(d.line_suffix(d.concat(&[d.text(" "), self.build_comment_doc(comment)])));
            }
        }
    }

    /// The PARENT's share of a spread's stripped-paren interior.
    ///
    /// When the parser strips grouping parens (`...(x⏎/* c */)`) the comments land in
    /// `[spread.argument.end, spread.span.end)`, and that region has **two** emitters
    /// which must partition it exactly once (the seam in `docs/comments.md` §The
    /// element-comma seam, applied to a node's own interior rather than to a list gap).
    /// The split is the source's own-line question — the same question every other
    /// trailing seam asks:
    ///
    /// - [`Self::append_spread_trailing_paren_comments`] — the spread's own doc — prints
    ///   what shares the argument's line: the same-line blocks inline, and the same-line
    ///   `//` deferred through `line_suffix`.
    /// - the parent (array element loop, call/`new`/member-chain last-argument emitter,
    ///   object property loop) prints the **own-line comments** — block or line — which
    ///   need a line of their own that only the parent can give (a `line_suffix` raised
    ///   from inside the spread would escape past the enclosing `]`/`)`), each on its own
    ///   line in source order.
    ///
    /// This function is the ONLY spelling of that share — the emitting form,
    /// [`Self::push_spread_own_line_comments`], reads it from here rather than
    /// re-deriving the predicate. Expressing it as an *anchor shift* instead — scanning
    /// the parent's trailing gap from `spread.argument.end` rather than from the spread's
    /// own end — reads as equivalent and is not: it hands the parent the spread's share
    /// too, so every same-line block and same-line `//` prints twice.
    pub(crate) fn spread_own_line_comments(
        &self,
        expr: &internal::Expression<'_>,
    ) -> CommentVec<'_> {
        expr.as_spread()
            .map(|spread| self.spread_element_own_line_comments(spread))
            .unwrap_or_default()
    }

    /// [`Self::spread_own_line_comments`] on the node itself, for the parents whose
    /// element type is a [`internal::SpreadElement`] rather than an
    /// [`internal::Expression`] — the object literal's property list.
    pub(crate) fn spread_element_own_line_comments(
        &self,
        spread: &internal::SpreadElement<'_>,
    ) -> CommentVec<'_> {
        let arg_end = spread.argument.span().end;
        comments_to_emit_in_range(self.comments, arg_end, spread.span.end)
            .filter(|c| self.has_newline_between(arg_end, c.span.start))
            .collect()
    }

    /// Whether the parent's share ([`Self::spread_own_line_comments`]) ends in a `//` —
    /// the END-of-list ordering question, stated once for both last-argument emitters
    /// (`emit_last_arg_trailing_comments` and the `call_formatting` loop that mirrors
    /// it): a share whose last comment is a `//` can have nothing glued after it, so
    /// the ordinary `[spread.end, closer)` gap must emit FIRST, its inline blocks
    /// landing on the argument's line ahead of any deferred suffix; a block-ending
    /// share keeps source order, the gap gluing onto the share's own line
    /// (`...b⏎/* i */ /* t */` — the order divergence pin).
    pub(crate) fn spread_share_ends_in_line_comment(
        &self,
        expr: &internal::Expression<'_>,
    ) -> bool {
        let Some(spread) = expr.as_spread() else {
            return false;
        };
        let arg_end = spread.argument.span().end;
        comments_to_emit_in_range(self.comments, arg_end, spread.span.end)
            .filter(|c| self.has_newline_between(arg_end, c.span.start))
            .last()
            .is_some_and(|c| !c.is_block)
    }

    /// Whether a spread's stripped-paren interior holds a comment the enclosing argument
    /// list must EXPAND around. Two kinds, for two different reasons:
    ///
    /// - an **own-line comment** — block or line, the parent's share above — the parent
    ///   prints it, and it needs a line of its own that only a broken list has;
    /// - a **same-line `//`** — the spread's own doc defers it through `line_suffix`, and
    ///   on a list that stays collapsed that buffer flushes past the call's `)` *and* its
    ///   `;`, re-binding the comment from the argument to the statement
    ///   (`fn(a, ...(b // c⏎))` → `fn(a, ...b); // c`). A deferred run must not leave the
    ///   construct it was written in — `docs/comments.md`. The array family has no such
    ///   hazard: its brackets already break around a spread carrying a comment.
    ///
    /// The predicate below spells the union as "any line comment, or any own-line
    /// comment" — the same set (a line comment is either same-line, forcing via the
    /// deferral, or own-line, forcing as the parent's share).
    pub(crate) fn spread_paren_comment_forces_expansion(
        &self,
        expr: &internal::Expression<'_>,
    ) -> bool {
        let Some(spread) = expr.as_spread() else {
            return false;
        };
        let arg_end = spread.argument.span().end;
        comments_to_emit_in_range(self.comments, arg_end, spread.span.end)
            .any(|c| !c.is_block || self.has_newline_between(arg_end, c.span.start))
    }

    /// [`Self::spread_paren_comment_forces_expansion`] asked of a whole element list — the
    /// **entry-gate** form, and the only one an argument-list builder should use to decide
    /// whether it must run its comment-aware path at all.
    ///
    /// Asked of EVERY element, not just the last: an interior lies *before* its own
    /// element's end, so no gap scan in any of these builders can see it, and a non-last
    /// spread's interior is exactly as invisible as a last one's. Spelling the gate on
    /// `arguments.last()` is what dropped it at three of the call family's entry points
    /// (the same reach `any_comment_forces_expansion` already has per argument).
    pub(crate) fn any_spread_paren_comment_forces_expansion(
        &self,
        elements: &[internal::Expression<'_>],
    ) -> bool {
        elements
            .iter()
            .any(|e| self.spread_paren_comment_forces_expansion(e))
    }

    /// Whether this expression's own doc ends in a DEFERRED line comment — today only a
    /// spread whose stripped grouping parens held a **same-line** `//` (`...(b // c⏎)`),
    /// which [`Self::append_spread_trailing_paren_comments`] emits through `line_suffix`
    /// (an own-line `//` is the parent's share and defers nothing).
    ///
    /// The caller that owns the gap *after* such a node must not let its own same-line
    /// `//` defer onto the same output line: two deferred line comments emitted back to
    /// back weld into ONE comment, the second `//` becoming text inside the first
    /// (`// c1 // c2`). That is the merge prettier performs here and tsv refuses — see
    /// `docs/comments.md` §Trailing and dangling runs.
    pub(crate) fn defers_trailing_line_comment(&self, expr: &internal::Expression<'_>) -> bool {
        expr.as_spread()
            .is_some_and(|spread| self.spread_element_defers_trailing_line_comment(spread))
    }

    /// [`Self::defers_trailing_line_comment`] on the node itself — see
    /// [`Self::spread_element_own_line_comments`] for why both spellings exist.
    pub(crate) fn spread_element_defers_trailing_line_comment(
        &self,
        spread: &internal::SpreadElement<'_>,
    ) -> bool {
        let arg_end = spread.argument.span().end;
        comments_to_emit_in_range(self.comments, arg_end, spread.span.end)
            .any(|c| !c.is_block && !self.has_newline_between(arg_end, c.span.start))
    }

    /// Emit the parent's share of a spread's stripped-paren interior
    /// ([`Self::spread_own_line_comments`]) into `parts`, each on its own line with
    /// author blank lines preserved. Returns whether anything was emitted — which is also
    /// the caller's signal to force its argument list open, since an own-line comment is
    /// a sibling of the argument rather than a trailer on its line.
    ///
    /// Where this sits relative to the caller's own `[spread.span.end, closer)` gap
    /// depends on whether a **comma** follows the spread, and that is a position
    /// question, not a source-order one:
    ///
    /// - at the END of a list there is no comma, so both the interior and anything
    ///   written after the `)` merely trail the element; source order decides, and this
    ///   run is emitted FIRST (`emit_last_arg_trailing_comments`).
    /// - between two elements the comma gives an outside block a home on the element's
    ///   own line, so the ordinary gap goes first and this run follows it, past the comma
    ///   ([`Printer::open_inter_arg_gap`], the array element loop, the object property
    ///   loop).
    ///
    /// Either way the caller does NOT carry a `prev_end` out of here: its own gap starts
    /// at the spread's end, which already lies past every interior comment, so its blank
    /// scan cannot double-count a blank this loop already consumed.
    ///
    /// Every caller is a hard-broken (or comment-force-expanded) layout: an own-line
    /// comment is exactly the thing that forces one. A `//` in the run is always
    /// last-on-its-line (nothing can share a line behind it), so the hardline before
    /// the next comment — or the caller's own break after the run — is what keeps it
    /// from swallowing what follows.
    pub(crate) fn push_spread_own_line_comments(
        &self,
        parts: &mut DocBuf,
        expr: &internal::Expression<'_>,
    ) -> bool {
        self.push_spread_own_line_comments_with_blanks(parts, expr, true)
    }

    /// [`Self::push_spread_own_line_comments`] with the author-blank policy named.
    ///
    /// `preserve_blanks: false` is for a run the caller emits **past an elision comma** (the
    /// array element loop, when holes follow the spread). The blank was authored between the
    /// argument and the comment, a gap the run no longer occupies once a structural comma
    /// sits in front of it — and the array's own rule is that a hole carries **no** blank
    /// line after it (`has_blank_line_after_slot`, prettier's `node &&`), so the reprint
    /// drops it. Preserving it here would print a blank the next pass removes.
    pub(crate) fn push_spread_own_line_comments_with_blanks(
        &self,
        parts: &mut DocBuf,
        expr: &internal::Expression<'_>,
        preserve_blanks: bool,
    ) -> bool {
        expr.as_spread().is_some_and(|spread| {
            self.push_spread_element_own_line_comments_with_blanks(parts, spread, preserve_blanks)
        })
    }

    /// [`Self::push_spread_own_line_comments`] on the node itself — see
    /// [`Self::spread_element_own_line_comments`] for why both spellings exist.
    pub(crate) fn push_spread_element_own_line_comments(
        &self,
        parts: &mut DocBuf,
        spread: &internal::SpreadElement<'_>,
    ) -> bool {
        self.push_spread_element_own_line_comments_with_blanks(parts, spread, true)
    }

    /// [`Self::push_spread_element_own_line_comments`] with the author-blank policy
    /// named — see [`Self::push_spread_own_line_comments_with_blanks`].
    pub(crate) fn push_spread_element_own_line_comments_with_blanks(
        &self,
        parts: &mut DocBuf,
        spread: &internal::SpreadElement<'_>,
        preserve_blanks: bool,
    ) -> bool {
        let comments = self.spread_element_own_line_comments(spread);
        let mut prev_end = spread.argument.span().end;
        let mut prev_comment: Option<&internal::Comment> = None;
        for comment in &comments {
            // A pair the author GLUED onto one line keeps that line, whichever blank policy
            // is in force ([`Printer::trailing_run_hugs_previous`]) — the glue question and
            // the author-blank question are separate, so the predicate is asked directly
            // rather than through `push_trailing_run_separator`, whose non-glue arm is
            // always blank-preserving.
            if self.trailing_run_hugs_previous(prev_comment, comment.span.start) {
                parts.push(self.d().text(" "));
            } else if preserve_blanks {
                self.push_blank_preserving_hardline(parts, prev_end, comment.span.start);
            } else {
                parts.push(self.d().hardline());
            }
            parts.push(self.build_comment_doc(comment));
            prev_end = comment.span.end;
            prev_comment = Some(comment);
        }
        !comments.is_empty()
    }

    /// Check if a grouping pair in `[expr_end, boundary_end)` holds trailing comments —
    /// comments the author wrote INSIDE the pair, between the operand and its `)`.
    ///
    /// True when the gap holds a grouping `)` at all (confirming the parser stripped a
    /// `ParenthesizedExpression`; without that check this would false-positive on normal
    /// operator comments, e.g. ternary `? c /* comment */ :`) **and** at least one
    /// comment lies before it.
    ///
    /// ⚠️ The window is `[expr_end, ')')`, not the caller's whole gap
    /// ([`Self::collapsed_grouping_close`]). Stated the other way round — "is there a `)`
    /// after the LAST comment" — one comment written *outside* the pair flipped the whole
    /// question false, and the shell that asked it then emitted **nothing** for the gap,
    /// dropping the run inside the pair along with it (`( // c⏎g // c9⏎) /* t */++` printed
    /// `( // c⏎g⏎)++`). A gap that spans the `)` holds two runs belonging to two emitters,
    /// so the predicate has to be about one of them.
    ///
    /// The existence check is **on page**, not *to emit*: every caller is a layout gate —
    /// does this gap's content force the shell open, keep the pair, break the arrow — and
    /// an owned comment still occupies the page it is not this gap's job to print
    /// (`docs/comments.md` §the three axes). No owned comment can reach one of these gaps
    /// today (ownership needs a node starting after the comment, and every shell gap ends
    /// at a delimiter), so the axis is stated for the reader and for the next boundary
    /// that widens, not for a behaviour it changes.
    pub(crate) fn has_trailing_paren_comments(&self, expr_end: u32, boundary_end: u32) -> bool {
        // The wide window first: it is a binary search over `self.comments` (trivially
        // false in a comment-free document) and a strictly weaker precondition, so the
        // source walk below never runs for the overwhelmingly common gap.
        self.has_comments_on_page_between(expr_end, boundary_end)
            && self
                .collapsed_grouping_close(expr_end, boundary_end)
                .is_some_and(|close| self.has_comments_on_page_between(expr_end, close))
    }

    /// The **outermost** grouping `)` a self-parenthesizing value's shells collapse into.
    ///
    /// A sequence supplies its own required parens, so the printer emits ONE pair where
    /// the source may hold several — those parens plus every redundant shell the parser
    /// stripped around them. The single emitted `)` stands in for all of them, and no
    /// enclosing emitter can see inside a paren pair
    /// ([`Self::trailing_paren_comment_parts`]), so every comment up to the LAST close
    /// has to be emitted inside. Scanning to the FIRST one bounded the range at the
    /// innermost paren and dropped the comment that sits past it outright
    /// (`const k = ((a, b) /* c */);` → `const k = (a, b);`).
    ///
    /// The walk steps over `)` and trivia only, so it stops at the shell run's end rather
    /// than at the caller's boundary — an enclosing construct's own `)` is unreachable
    /// even where that boundary is loose (an `as` cast's operand shell, whose boundary
    /// spans the keyword and its type).
    ///
    /// `None` when the gap holds no `)` at all, leaving the caller its own fallback.
    pub(in crate::printer) fn collapsed_grouping_close(
        &self,
        expr_end: u32,
        boundary_end: u32,
    ) -> Option<u32> {
        let bytes = self.source.as_bytes();
        let mut i = expr_end;
        let mut close = None;
        while let Some(pos) = self.next_significant_byte(i, boundary_end) {
            if bytes[pos] != b')' {
                break;
            }
            close = Some(pos as u32);
            i = pos as u32 + 1;
        }
        close
    }

    /// A sequence value sitting inside a stripped grouping shell: its own required parens
    /// ARE the pair the printer emits, and a trailing comment stays inside them
    /// (`const x = (a, b /* c */)`) rather than floating out after `)` or doubling the
    /// shell ([`Printer::build_sequence_doc_value`], prettier #19263).
    ///
    /// The one seam both shell builders route through, because locating that pair's close
    /// is one question with one answer: they held separate copies of the scan and both
    /// were wrong the same way, dropping the comment of a doubly-shelled value at every
    /// position they serve (`const k = ((a, b) /* c */);`, `() => ((a, b) /* c */)`).
    ///
    /// The `boundary_end` fallback is defensive: a value only reaches a *shell* builder
    /// with a grouping pair around it in source, so the walk has a `)` to find. The
    /// restricted-production site answers this differently and keeps its own scan —
    /// there a bare `return a, b;` has no pair at all, and sweeping to the boundary
    /// would pull the statement's own terminator-gap comments inside the parens.
    fn build_shell_sequence_doc(
        &self,
        seq: &internal::SequenceExpression<'_>,
        expr_end: u32,
        boundary_end: u32,
        layout: SeqLayout,
    ) -> DocId {
        let grouping_close = self
            .collapsed_grouping_close(expr_end, boundary_end)
            .unwrap_or(boundary_end);
        self.build_sequence_doc_value(seq, grouping_close, layout)
    }

    /// The index of the next byte that is neither whitespace nor trivia, at or after
    /// `start` and before `end`; `None` when the range holds nothing else.
    ///
    /// The "what actually comes next in the source?" step both shell questions ask —
    /// [`Self::collapsed_grouping_close`] takes it once per `)`, and
    /// [`Self::shell_meets_statement_terminator`] once. A comment occupies bytes even
    /// where nothing emits it, so a walk that stepped over whitespace alone would stop
    /// on the `/` and answer about the wrong token.
    fn next_significant_byte(&self, start: u32, end: u32) -> Option<usize> {
        next_significant_byte(self.source, start, end)
    }

    /// Whether a comment in `[expr_end, boundary_end)` forces the operand's grouping
    /// parens to **survive**, at a gap the grammar marks `[no LineTerminator here]`.
    ///
    /// Two gaps qualify, and both are ASI-sensitive: an `as`/`satisfies` cast's
    /// operand→keyword gap and a postfix `++`/`--`'s operand→operator gap (tsc breaks
    /// out of each construct on `scanner.hasPrecedingLineBreak()`). A comment that
    /// occupies more than one line — a `//`, which runs to end of line, or a multi-line
    /// block — therefore cannot be inlined here, because inlining **rewrites the
    /// program**: the `//` swallows the tail (`(1 // c⏎) as const;` → `1 // c as const;`,
    /// which parses as a bare `1`), and the multi-line block puts a real line break
    /// before the keyword (output that does not reparse at all).
    ///
    /// Such a comment can only ever have been authored *inside* a grouping paren shell,
    /// since the bare form is unparseable — so the shell is what holds it in place, and
    /// stripping it is not available the way it is at the sibling keyword→value gaps
    /// ([`Self::build_expression_doc_with_paren_comments`]). A shell is redundant only
    /// when the stripped form can still express the comment's position.
    ///
    /// A single-line block comment inlines as before (`x /* c */ as A`, `x /* c */++`),
    /// matching prettier. Finding the `)` is what keeps this keyed on a real shell, and
    /// the window it bounds — the pair's INTERIOR, not the caller's whole gap — is the
    /// one this question is about: only a comment the pair actually HOLDS can need the
    /// pair kept, and a comment past the `)` belongs to the enclosing gap
    /// ([`Self::append_shell_outside_run`]).
    ///
    /// The multi-line predicate is asked LAST: the two steps before it are a binary
    /// search over `self.comments` and a short source walk, both false on nearly every
    /// cast and update in a real file.
    pub(crate) fn asi_gap_needs_parens(&self, expr_end: u32, boundary_end: u32) -> bool {
        if !self.has_comments_on_page_between(expr_end, boundary_end) {
            return false;
        }
        let Some(close) = self.collapsed_grouping_close(expr_end, boundary_end) else {
            return false;
        };
        comments_to_emit_in_range(self.comments, expr_end, close)
            .any(|c| !c.is_block || c.multiline)
    }

    /// The operand of an ASI-sensitive gap (an `as`/`satisfies` keyword, a postfix
    /// `++`/`--`) rendered inside the grouping-paren shell that holds its comments,
    /// emitting **both** of the shell's gaps — `(`→operand and operand→`)`.
    ///
    /// `None` when no shell needs keeping, so the caller falls through to its ordinary
    /// inline path.
    ///
    /// The trailing gap is what makes the shell load-bearing ([`Self::asi_gap_needs_parens`]):
    /// the keyword may not start a line, so a comment spanning lines has nowhere else to go.
    /// The **leading** gap keeps the shell for a different reason — nothing else emits it.
    /// A comment there that is neither glued to the operand (which would make it
    /// `owned_by_node`, printed from the operand's own doc) nor inside the operand's span
    /// belongs to no node at all, so stripping the shell **drops** it outright
    /// (`( // c⏎x) as A` → `x as A`, the comment gone). Retaining on either gap is one
    /// rule for one shell rather than two half-rules that disagree about `( // b⏎x // c⏎)`.
    ///
    /// A **sequence** operand's own required parens are the grouping, so re-wrapping
    /// would double them: with no leading run it delegates to the value builder, which
    /// keeps the trailing run inside the pair
    /// ([`Self::build_expression_doc_keep_paren_comments`]); with one, the pair is built
    /// HERE — the leading gap is the pair's and the value builder's window opens at the
    /// operand, so delegating dropped the run outright (`(// c⏎x, y) as A` →
    /// `(x, y) as A`). The sequence rides the expanded shell BARE
    /// ([`Printer::build_sequence_doc_bare`]), the same one-shell-for-both-gaps layering
    /// every required pair in the family takes.
    ///
    /// ⚠️ `boundary_end` is the KEYWORD / operator, so the gap it bounds spans the pair's
    /// `)` and holds **two** runs: the pair's own trailing run and, past the `)`, the
    /// enclosing gap's. The shell claims the whole window, so it emits both — the first
    /// inside the pair, the second after `close` — and the split is
    /// [`Self::collapsed_grouping_close`], the same `)` every window that opens after an
    /// operand takes its start from (`docs/comments.md` hazard 3). Emitting the window as
    /// one run instead would have carried the outside comment *into* the parens, and the
    /// all-or-nothing gate that preceded this dropped both runs outright.
    pub(crate) fn build_asi_operand_shell_doc(
        &self,
        node_start: u32,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
    ) -> Option<DocId> {
        let expr_start = expr.span().start;
        let expr_end = expr.span().end;
        let open = find_char_skipping_comments(
            self.source.as_bytes(),
            node_start as usize,
            expr_start as usize,
            b'(',
        )
        .map(|p| p as u32);

        let leading_start = open.map_or(node_start, |p| p + 1);
        let has_leading =
            open.is_some() && self.has_comments_to_emit_between(leading_start, expr_start);
        if !has_leading && !self.asi_gap_needs_parens(expr_end, boundary_end) {
            return None;
        }

        // The pair's `)` splits the window into the run it holds and the run the
        // enclosing gap holds. `None` is the shell kept for its LEADING run alone with no
        // `)` reachable past the operand, where the whole window is the pair's. The
        // interior window ends PAST the `)` (`close + 1`) — it is what proves a pair is
        // there, and [`Self::has_trailing_paren_comments`] looks for it inside the range
        // it is given.
        let close = self.collapsed_grouping_close(expr_end, boundary_end);
        let inner_end = close.map_or(boundary_end, |c| c + 1);

        if matches!(expr, internal::Expression::SequenceExpression(_)) && !has_leading {
            let seq =
                self.build_expression_doc_keep_paren_comments(expr, inner_end, SeqLayout::Aligned);
            return Some(self.append_shell_outside_run(seq, close, boundary_end));
        }

        // The shell's `(` is the statement's first token whenever the statement's leftmost
        // node is inside the operand, so it already discharges what the expression-statement
        // wrap exists for — keeping the statement from starting with `{` / `function` /
        // `class`. Clearing the target stops the operand adding a second pair inside this
        // one (`(⏎\t({ a: 1 }) // c⏎) as const`). Not restored afterwards: a rebuild under a
        // conditional group must reach the same answer, and the target is cleared per
        // statement by `build_expression_statement_doc` regardless. A nested cast never
        // owns the target (it is set for the statement's leftmost node only), so this is a
        // no-op there.
        self.expr_stmt_paren_target.set(None);

        // The opening-delimiter rule at this shell's `(`: a `//` the author glued to it
        // keeps that line ([`Self::split_located_paren_glued_run`]), as at every other
        // opening delimiter. The doc carries its own leading space.
        let (paren_trailing, run_start) =
            self.split_located_paren_glued_run(leading_start, open, expr_start);
        let leading_run = self.build_rhs_comments_opt(run_start, expr_start);

        let mut body = DocBuf::new();
        if let Some(run) = leading_run {
            body.push(run);
        }
        body.push(match expr {
            // Bare: the shell composed below IS the sequence's required pair.
            internal::Expression::SequenceExpression(seq) => self.build_sequence_doc_bare(seq),
            _ => self.build_expression_doc(expr),
        });
        if let Some((trailing, _needs_break)) =
            self.trailing_paren_comment_parts(expr_end, inner_end)
        {
            body.extend(trailing);
        }

        let shell = self.compose_expanded_shell_doc(paren_trailing, &body, ")");
        Some(self.append_shell_outside_run(shell, close, boundary_end))
    }

    /// Append the run written PAST a shell's `)` — the enclosing gap's, not the pair's —
    /// inline before the keyword / operator the caller adds next.
    ///
    /// Both arms of [`Self::build_asi_operand_shell_doc`] need it, and for the same
    /// reason: the shell claims the whole `[operand_end, keyword)` window, so whatever it
    /// does not emit is DROPPED. Only a single-line block can be here — anything
    /// occupying a second line would put a line terminator before an ASI-sensitive token,
    /// which is unparseable — so this is the same emitter the shell-LESS path uses for
    /// the whole gap, and the two agree on the one shape they share.
    fn append_shell_outside_run(
        &self,
        shell: DocId,
        close: Option<u32>,
        boundary_end: u32,
    ) -> DocId {
        let Some(start) = close.map(|c| c + 1).filter(|&start| {
            start < boundary_end && self.has_comments_to_emit_between(start, boundary_end)
        }) else {
            return shell;
        };
        self.d().concat(&[
            shell,
            self.build_inline_comments_between_doc(start, boundary_end),
        ])
    }

    /// The expanded paren-shell rendering every shell emitter shares — the
    /// leading-gap builders (via [`Self::build_leading_run_expanded_shell_doc`])
    /// and the anchored trailing-run shell
    /// ([`Self::build_paren_operand_comment_doc`]'s line arm): `( // c` when the
    /// author glued a `//` to the `(` (the comment runs to end of line and the
    /// indent's hardline supplies the break, so nothing following it is
    /// swallowed; the space is the glued split's), the body one indent in, the
    /// closer back out on its own line. `close` is `")"` where a separate node
    /// prints what follows, `")!"` where this doc owns the non-null's `!`.
    pub(in crate::printer) fn compose_expanded_shell_doc(
        &self,
        paren_trailing: Option<DocId>,
        body: &DocBuf,
        close: &'static str,
    ) -> DocId {
        let d = self.d();
        let open_doc = match paren_trailing {
            Some(comment) => d.concat(&[d.text("("), comment]),
            None => d.text("("),
        };
        d.concat(&[
            open_doc,
            d.indent_hardline(d.concat(body)),
            d.hardline(),
            d.text(close),
        ])
    }

    /// The family's expanded shell for a LEADING run — the one emission every
    /// required-pair leading gap shares: split the `(`-glued `//` (the
    /// opening-delimiter rule), emit the rest of the run above the operand with
    /// its authored own-line placements kept, close with `close`. Callers: the
    /// assignment-target / instantiation-head operand shell
    /// ([`Self::build_shell_operand_doc`]), the non-null's needs-parens arm, and
    /// the sealed optional chain (both `build_ts_non_null_doc` /
    /// `build_sealed_non_null_paren_doc`, whose `close` is `")!"`).
    ///
    /// A commented TRAILING gap rides the SAME shell rather than a second one — the
    /// pair is already expanded, so the operand→`)` run simply follows the operand
    /// inside it ([`Self::push_shell_trailing_run`]). Declining instead and folding both
    /// runs flat, which is what a shell that bailed on a commented trailing gap forced,
    /// left the one authoring that commented both gaps rendering with no shell at all:
    /// the `//` glued to a `(` that never broke, the operand at the ENCLOSING indent,
    /// and the `)` welded to it — the family's shape at neither gap, and prettier's at
    /// neither either.
    pub(in crate::printer) fn build_leading_run_expanded_shell_doc(
        &self,
        gap_start: u32,
        operand_start: u32,
        trailing_gap: Option<(u32, u32)>,
        inner_doc: DocId,
        close: &'static str,
    ) -> DocId {
        let (paren_trailing, run_start) =
            self.split_open_delimiter_glued_run(gap_start, operand_start);
        let mut body = DocBuf::new();
        if let Some(run) = self.build_rhs_comments_opt(run_start, operand_start) {
            body.push(run);
        }
        body.push(inner_doc);
        if let Some((start, end)) = trailing_gap {
            self.push_shell_trailing_run(&mut body, start, end);
        }
        self.compose_expanded_shell_doc(paren_trailing, &body, close)
    }

    /// The operand→`)` run inside a pair the LEADING run already expanded — the same
    /// two arms [`Self::build_paren_operand_comment_doc`] takes when it builds the shell
    /// itself, minus the shell (this body is already inside one).
    ///
    /// A `//` takes the anchored emitter, since every comment here was authored AFTER
    /// the operand and the closer's hardline below ends the line the run defers onto; a
    /// block-only run trails the operand inline.
    fn push_shell_trailing_run(&self, body: &mut DocBuf, start: u32, end: u32) {
        if self.has_line_comments_between(start, end) {
            self.push_anchored_trailing_run(body, start, end, RunLeadingBlank::Keep);
        } else if self.has_comments_to_emit_between(start, end) {
            body.push(self.build_chain_block_comments_doc(start, end, CommentSpacing::Leading));
        }
    }

    /// The family's answer for a REQUIRED pair's LEADING gap — the one rule every
    /// position that prints such a pair asks, rather than five spellings of it.
    ///
    /// `Some` is the expanded shell ([`Self::build_leading_run_expanded_shell_doc`]):
    /// the run occupies a line, so the `(`-glued `//` keeps the `(` line, the operand
    /// takes one indent and `close` comes back out. `None` says this gap does not take
    /// the shell and the caller folds the run flat above the operand instead — the run
    /// fits on one line (a glued single-line block run), which is the only reason left.
    ///
    /// `trailing_gap` is `[operand_end, boundary_end)`, `None` at a position whose pair
    /// has no trailing gap of its own. A commented one does NOT decline the shell: it
    /// rides inside it, after the operand.
    pub(in crate::printer) fn build_required_pair_leading_shell_doc(
        &self,
        gap_start: u32,
        operand_start: u32,
        trailing_gap: Option<(u32, u32)>,
        inner: DocId,
        close: &'static str,
    ) -> Option<DocId> {
        if !self.has_comments_to_emit_between(gap_start, operand_start)
            || !self.has_newline_between(gap_start, operand_start)
        {
            return None;
        }
        Some(self.build_leading_run_expanded_shell_doc(
            gap_start,
            operand_start,
            trailing_gap,
            inner,
            close,
        ))
    }

    /// The index just past the `)` that closes a REQUIRED pair around an operand
    /// ending at `operand_end`, when the author wrote that pair — `None` when the next
    /// thing in the source is something else (a `(` of an argument list, a `` ` ``),
    /// which means the pair this position prints is tsv's own and the source has no
    /// trailing gap inside it.
    ///
    /// The distinction is load-bearing: `(fn /* t */)()` writes the pair around the
    /// callee and the comment is INSIDE it, while `(/* t */ fn())` writes it around the
    /// whole call and the comment belongs to the call, not to a pair the callee prints.
    pub(in crate::printer) fn paren_shell_close_after(&self, operand_end: u32) -> Option<u32> {
        paren_shell_close_after(self.source, operand_end)
    }

    /// The operand→`)` gap a REQUIRED pair owns, when the pair KEEPS ITS INTERIOR at
    /// this position **and** the author wrote it.
    ///
    /// Both halves matter. `keeps_interior` is the **position's** answer — a pair keeps
    /// its interior at a function/arrow callee or tag (prettier's
    /// `printCommentsForFunction` covers leading and trailing alike) and at a **sealed
    /// optional chain**, where the pair is what stops the chain and both formatters keep
    /// the comment between the operand and the `)`. It is deliberately a parameter and
    /// never re-derived from the operand's KIND here: a kind is a property of the
    /// subtree and holds wherever that subtree appears, while a gap belongs to exactly
    /// one seam, which is a property of the position — `(() => 1 /* c */).p` prints the
    /// same pair as an IIFE callee but its trailing gap is the member seam's, and
    /// claiming it here double-printed the comment. The position-shaped predicates are
    /// [`chain_paren_leading_gap`](crate::printer::chain::chain_paren_leading_gap)'s
    /// family, and the leading half reads the same ones.
    /// The second half is the author's pair, since `(fn /* t */)()` writes it around the
    /// callee while `(/* t */ fn())` writes it around the whole call, where the comment
    /// belongs to the call and to no pair printed here
    /// ([`paren_shell_close_after`]).
    ///
    /// Every window that opens AFTER the operand — the type-argument gap, an argument
    /// list's own leading scan, a tag→`` ` `` gap, a member seam's `object_end` — takes
    /// its start from the returned `)` rather than from the operand's span end, so a
    /// comment emitted inside the parens is not emitted a second time outside them
    /// (`docs/comments.md` hazard 3).
    pub(in crate::printer) fn owned_pair_trailing_gap(
        &self,
        operand_end: u32,
        keeps_interior: bool,
    ) -> Option<(u32, u32)> {
        keeps_interior
            .then(|| self.paren_shell_close_after(operand_end))
            .flatten()
            .map(|close| (operand_end, close))
    }

    /// Where the window AFTER an operand opens: past the `)` of a pair that emitted its
    /// own trailing gap, or the operand's own span end where there is no such pair.
    ///
    /// `trailing_gap` is that operand's [`Self::owned_pair_trailing_gap`], passed in
    /// rather than re-derived so the offset and the claim come off ONE lookup. The one
    /// spelling of it, so the three positions that print such a pair — the bare callee,
    /// a member chain's callee, a tagged template's tag — and every window they open
    /// (type arguments, an argument list's leading scan, the tag→`` ` `` gap) cannot
    /// disagree about where the pair ends.
    pub(in crate::printer) fn gap_start_after_owned_pair(
        operand_end: u32,
        trailing_gap: Option<(u32, u32)>,
    ) -> u32 {
        trailing_gap.map_or(operand_end, |(_, close)| close)
    }

    /// A REQUIRED pair that owns BOTH of its gaps — `(`→operand and operand→`)`.
    ///
    /// The one emission for every position where the pair the printer emits is the
    /// author's own and nothing outside it can reach between the parens: the chain's
    /// sealed / IIFE base, the bare IIFE callee, a tagged template's function tag.
    /// Layering, which is the family's convention rather than this function's choice:
    ///
    /// - a leading run that occupies a line takes the expanded shell
    ///   ([`Self::build_required_pair_leading_shell_doc`]) — and a commented TRAILING
    ///   gap rides that SAME shell, after the operand, rather than declining it;
    /// - otherwise the leading run folds flat above the operand, and a commented
    ///   TRAILING gap then builds the shell around that
    ///   ([`Self::build_paren_operand_comment_doc`]);
    /// - with neither, the pair is bare.
    ///
    /// So exactly ONE shell is built however many of the two gaps hold comments. Two
    /// nest; none leaves the `//` glued to a `(` that never breaks and the operand at
    /// the enclosing indent.
    ///
    /// `flat_body` and `broken_body` are the operand's two renderings, which differ
    /// only where a caller shapes the flat one (the chain base's hang / ternary
    /// forms); the fold is applied to **both**, since the trailing emitter's line-comment
    /// arm prints the broken one and folding into the shaped body alone would DROP the
    /// leading run at exactly the authoring that reaches it. A caller whose SHELL body
    /// differs from its folded one asks the two halves separately —
    /// [`Self::build_folded_required_pair_doc`].
    pub(in crate::printer) fn build_owned_required_pair_doc(
        &self,
        leading_gap: (u32, u32),
        trailing_gap: Option<(u32, u32)>,
        flat_body: DocId,
        broken_body: DocId,
        close: &'static str,
    ) -> DocId {
        self.build_required_pair_leading_shell_doc(
            leading_gap.0,
            leading_gap.1,
            trailing_gap,
            flat_body,
            close,
        )
        .unwrap_or_else(|| {
            self.build_folded_required_pair_doc(
                leading_gap,
                trailing_gap,
                flat_body,
                broken_body,
                close,
            )
        })
    }

    /// [`Self::build_owned_required_pair_doc`] past its leading-shell arm — the form for
    /// a caller whose shell and fold take DIFFERENT bodies.
    ///
    /// The non-null's `needs_parens` arm is the one such caller: its operand may carry a
    /// ternary's width-driven expanding parens, which belong in the folded body but not
    /// inside a hard-expanded shell, where there is no width left to decide. Every other
    /// position hands one body to both and takes the combined form above.
    pub(in crate::printer) fn build_folded_required_pair_doc(
        &self,
        leading_gap: (u32, u32),
        trailing_gap: Option<(u32, u32)>,
        flat_body: DocId,
        broken_body: DocId,
        close: &'static str,
    ) -> DocId {
        let d = self.d();
        let (gap_start, operand_start) = leading_gap;
        let fold = |body: DocId| match self.build_rhs_comments_opt(gap_start, operand_start) {
            Some(lead) => d.concat(&[lead, body]),
            None => body,
        };
        let flat_body = fold(flat_body);
        if let Some((start, end)) = trailing_gap
            && let Some(doc) = self.build_paren_operand_comment_doc(
                start,
                end,
                flat_body,
                fold(broken_body),
                close,
            )
        {
            return doc;
        }
        d.concat(&[d.text("("), flat_body, d.text(close)])
    }

    /// An operand in a position that prints a REQUIRED pair around some operand
    /// kinds, with the node's `^`→operand gap emitted — the whole operand doc for
    /// an assignment expression's / assignment pattern's target
    /// ([`ParenContext::AssignmentTarget`]) and an instantiation expression's head
    /// ([`ParenContext::InstantiationExpression`]).
    ///
    /// The gap is `[node_start, operand.span().start)`, `node_start` being the
    /// enclosing node's own start — for a parenthesized operand, its authored `(`.
    /// Nothing else emits it: a comment there that is not glued to the operand
    /// (which would make it `owned_by_node`, printed from the operand's own doc)
    /// belongs to no node, so the bare `"(" + operand + ")"` spelling DROPPED it
    /// outright (`( // c⏎x as T) = 1;` → `(x as T) = 1;`).
    ///
    /// Where the position REQUIRES the pair — a type-assertion target (`x as T = 1;`
    /// is a parse error), an arrow instantiation head — the pair prints whatever the
    /// comment does, and tsv keeps the run INSIDE it, where the author wrote it;
    /// prettier hoists it out in front, re-binding it from the operand to the whole
    /// statement (cataloged: conformance_prettier_ts_comments.md §Comment
    /// relocation, "Assignment-target shell, leading comment"). A run that occupies
    /// a line — a `//`, an own-line block — expands the pair, with the `( // c` glue
    /// and the author's own-line placements kept
    /// ([`Self::build_asi_operand_shell_doc`]'s rendering one construct over); a
    /// glued single-line block run stays flat.
    ///
    /// Any other operand's pair is REDUNDANT and strips (`(// c⏎x) = 1;`,
    /// `(// c⏎fn)<string>;`), and the stripped form still expresses the run's
    /// position — it leads the statement, exactly where prettier lands it.
    pub(in crate::printer) fn build_shell_operand_doc(
        &self,
        node_start: u32,
        target: &internal::Expression<'_>,
        context: ParenContext,
    ) -> DocId {
        let d = self.d();
        let target_start = target.span().start;
        let needs_parens = self.needs_parens(target, context);
        // Zero-comment fast gate, emit-keyed on purpose: the layouts below differ
        // only in where EMITTED comments render — an owned (target-glued) block
        // prints identically inside the flat pair from the target's own doc, and
        // never asks the pair to expand.
        if !self.has_comments_to_emit_between(node_start, target_start) {
            let inner = self.build_expression_doc(target);
            return if needs_parens { d.parens(inner) } else { inner };
        }
        let open = find_char_skipping_comments(
            self.source.as_bytes(),
            node_start as usize,
            target_start as usize,
            b'(',
        )
        .map(|p| p as u32);
        let leading_start = open.map_or(node_start, |p| p + 1);
        // A comment that occupies a line expands the pair, on the family's shared
        // rule. The read is in-source over the whole gap, which cannot over-fire:
        // the gate above already found a comment to emit (a comment-free author
        // break never reaches here), a `//` always ends its line, and an own-line
        // block carries its break. This position's pair encloses the operand alone,
        // so it has no trailing gap to yield the shell to.
        if needs_parens
            && let Some(shell) = self.build_required_pair_leading_shell_doc(
                leading_start,
                target_start,
                None,
                self.build_expression_doc(target),
                ")",
            )
        {
            return shell;
        }
        // The flat tail serves both regimes: a glued-block run leads the operand —
        // inside the pair where one prints, ahead of the bare operand where the
        // stripped run leads the statement (and there a line comment's hardline
        // separator is what keeps the statement after it on its own line).
        let lead = self.build_rhs_comments_opt(leading_start, target_start);
        let inner = self.build_expression_doc(target);
        let core = match lead {
            Some(lead) => d.concat(&[lead, inner]),
            None => inner,
        };
        if needs_parens { d.parens(core) } else { core }
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
    /// `position_parens` says the CALLING POSITION will parenthesize this value anyway
    /// (`const x = (a = b)`), which makes the shell **retained** rather than stripped —
    /// see [`Self::shell_value_keeps_own_parens`], the one predicate that answer is read
    /// through.
    ///
    /// Used for variable init, assignment RHS, and ternary branches. A `for` header's
    /// init declarator takes [`Self::build_for_init_value_doc`] instead — same handling,
    /// minus the statement-terminator deferral its `;` does not license.
    pub(crate) fn build_expression_doc_with_paren_comments(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
        position_parens: bool,
    ) -> DocId {
        self.build_shell_value_doc(
            expr,
            boundary_end,
            ShellTail::StatementTerminator,
            position_parens,
            None,
        )
    }

    /// [`Self::build_expression_doc_with_paren_comments`] for a value the position froze
    /// (`prettier-ignore` in its `=`→value gap): the same shell, with the verbatim slice
    /// standing in for the expression doc.
    ///
    /// ⚠️ **The freeze does not take the value out of its shell.** The slice is the value's
    /// own node span, so the author's grouping parens lie OUTSIDE it and the gap between
    /// the slice and that `)` is the shell's, exactly as in the unfrozen form — a frozen
    /// arm that skipped this builder printed its own bare `parens()` and left that gap with
    /// no emitter at all, DROPPING every comment in it (`docs/comments.md` hazard 4; a
    /// printer that synthesizes its own `(`…`)` owns the gap inside it, per
    /// [`Self::trailing_paren_comment_parts`]). Routing through here is what keeps the
    /// frozen and unfrozen forms answering the gap identically — which shell is retained,
    /// where the comment renders, and when it defers past the terminator are all questions
    /// about the GAP, not about what renders between the parens.
    pub(crate) fn build_frozen_value_shell_doc(
        &self,
        expr: &internal::Expression<'_>,
        frozen: Span,
        boundary_end: u32,
        position_parens: bool,
    ) -> DocId {
        self.build_shell_value_doc(
            expr,
            boundary_end,
            ShellTail::StatementTerminator,
            position_parens,
            Some(frozen),
        )
    }

    /// Whether [`Self::build_expression_doc_with_paren_comments`] supplies the value's
    /// paren pair ITSELF, so the calling position must not add a second one.
    ///
    /// The single predicate both re-parenthesizing positions ask — a declarator
    /// initializer and a ternary branch. They each own a `needs_parens` question of their
    /// own (`position_parens`), but "did the callee already wrap?" is one question with
    /// one answer, and answering it twice is how the pair gets doubled at one site and
    /// dropped at the other (a ternary CONSEQUENT bounds this scan empty, so the callee
    /// never wraps there however the position answers `needs_parens`).
    pub(crate) fn shell_value_keeps_own_parens(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
        position_parens: bool,
    ) -> bool {
        // A sequence self-parenthesizes on every path, so it never takes the caller's
        // pair and is excluded rather than reported here.
        if matches!(expr, internal::Expression::SequenceExpression(_)) {
            return false;
        }
        let expr_end = expr.span().end;
        self.has_trailing_paren_comments(expr_end, boundary_end)
            && self.shell_gap_retains_parens(expr_end, boundary_end, position_parens)
    }

    /// Add a value position's clarity parens around a shell-built value — unless the shell
    /// builder already supplied the pair ([`Self::shell_value_keeps_own_parens`]).
    ///
    /// The two declarator positions (statement-level and `for`-header) resolve
    /// `position_parens` themselves, then hand it here with the doc the shell builder
    /// returned for it. `position_parens` must be the value the **builder received**: the
    /// two sides answer one question, and asking it twice is how the pair gets doubled at
    /// one site and dropped at the other. A ternary branch answers a different pair
    /// question than the flag it passes the builder, so it applies the predicate at its own
    /// seam (`parenthesize_ternary_branch`) rather than through here.
    pub(crate) fn wrap_value_position_parens(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
        position_parens: bool,
        inner: DocId,
    ) -> DocId {
        if position_parens
            && !self.shell_value_keeps_own_parens(expr, boundary_end, position_parens)
        {
            self.d().parens(inner)
        } else {
            inner
        }
    }

    /// Whether the gap's own content forces the shell to be RETAINED — the layout half of
    /// [`Self::shell_value_keeps_own_parens`], asked by the builder that acts on it and by
    /// the caller that must not double the pair.
    ///
    /// ⚠️ **One question, one predicate, one AXIS.** Spelling this on the two sides
    /// separately AND on different axes — the caller counting comments **on page**, the
    /// builder only those it would **emit** — has an owned comment in the gap make
    /// the caller skip a wrap the builder never makes, stripping the value's clarity parens.
    /// This is a layout gate ("does anything occupy the page here?"), so the on-page axis is
    /// the correct one for both (`docs/comments.md` §the three axes).
    fn shell_gap_retains_parens(
        &self,
        expr_end: u32,
        boundary_end: u32,
        position_parens: bool,
    ) -> bool {
        // The calling position parenthesizes this value anyway, so the pair is in the
        // output whatever this builder does — nothing may cross it.
        position_parens
            // A line comment would swallow the following `;`, and an own-line comment has
            // no inline placement, so either needs the parens on its own account.
            || self
                .comments_on_page_between(expr_end, boundary_end)
                .any(|c| !c.is_block || self.has_newline_between(expr_end, c.span.start))
    }

    /// The `for`-header init counterpart of
    /// [`Self::build_expression_doc_with_paren_comments`].
    ///
    /// A header init declarator's shell is followed by the header's **clause separator**,
    /// not by a statement `;`, so the deferral arm below must not fire: a comment sent
    /// past that separator leaves the declarator it was written in, and there is nothing
    /// out there to hold it — prettier, which does relocate it, cannot keep it either
    /// (its next pass carries a run's later comment clean out of the header, past the
    /// `)`, into the body's leading position). Everything else is shared with the
    /// statement-level path, which is the point: the block comment strips inline and the
    /// line comment retains the shell for exactly the same reasons there.
    ///
    /// `position_parens` carries the declarator's own clarity-paren answer
    /// (`ParenContext::VariableInit` — an assignment as a value takes a pair), exactly as
    /// the statement-level path carries it: a header declarator is a declarator, and the
    /// `for` exemption prettier applies is to the init **clause's own expression**
    /// (`for (a = b = c; ;)`), not to a value one binding deeper.
    ///
    /// ⚠️ **This is the declarator's own value, not "lexically under a for header".** The
    /// ambient `in_for_init` flag spans nested function and class bodies, where a real
    /// statement terminator does exist and the deferral is correct
    /// (`for (let i = (() => { const k = (a /* c */); })(); ;)` keeps `k`'s comment past
    /// its `;`) — so the distinction is threaded from the one builder that knows it
    /// rather than read from that flag.
    ///
    /// `frozen` is the value-head freeze this position resolved, exactly as
    /// [`Self::build_frozen_value_shell_doc`] carries it for the statement-level twin: the
    /// slice replaces the expression doc and nothing else moves, because which shell is
    /// retained and where its comment renders are questions about the GAP, not about what
    /// renders between the parens.
    pub(crate) fn build_for_init_value_doc(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
        position_parens: bool,
        frozen: Option<Span>,
    ) -> DocId {
        self.build_shell_value_doc(
            expr,
            boundary_end,
            ShellTail::ForClauseSeparator,
            position_parens,
            frozen,
        )
    }

    /// The value's own doc inside a shell arm that does NOT print the pair itself: the
    /// verbatim frozen slice where the position resolved a freeze, else the ordinary
    /// expression doc. Both spellings supply a self-parenthesizing value's own required
    /// pair ([`Self::build_frozen_expression_doc`] is `build_expression_doc`'s twin in
    /// exactly that), so the arms that return it need no sequence case of their own.
    fn build_shell_inner_doc(
        &self,
        expr: &internal::Expression<'_>,
        frozen: Option<Span>,
    ) -> DocId {
        match frozen {
            Some(frozen) => self.build_frozen_expression_doc(expr, frozen),
            None => self.build_expression_doc(expr),
        }
    }

    fn build_shell_value_doc(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
        tail: ShellTail,
        position_parens: bool,
        frozen: Option<Span>,
    ) -> DocId {
        let expr_end = expr.span().end;
        // The for-header's `[~In]` parens are applied HERE rather than by the caller,
        // because only the paths below know where they belong relative to the shell's
        // comment. They are tsv's parens, not the author's, so a comment written AFTER
        // the shell has to land outside them (`(a in b) /* c */`, not `(a in b /* c */)`)
        // — the same rule that keeps a synthesized paren from landing inside an owned
        // comment (`docs/comments.md`). The two paths that return early supply their own
        // pair: a sequence self-parenthesizes, and the keep-paren path RETAINS the shell,
        // which already parenthesizes the `in` — wrapping either would double it.
        let wrap_in = |doc: DocId| match tail {
            ShellTail::ForClauseSeparator => self.wrap_for_init_in(expr, doc),
            ShellTail::StatementTerminator => doc,
        };

        if !self.has_trailing_paren_comments(expr_end, boundary_end) {
            return wrap_in(self.build_shell_inner_doc(expr, frozen));
        }

        // Every position this serves — variable init, assignment RHS, ternary branch —
        // is prettier's default layout arm; the two that hang (a `return`/`throw`
        // argument, an arrow body) claim their sequence before reaching here.
        if let internal::Expression::SequenceExpression(seq) = expr {
            // A FROZEN sequence prints verbatim, so the operand-per-line layout the
            // sequence builder chooses is not available to it — its required pair takes
            // the retained-shell rendering below instead, which is where this gap's
            // comment goes on either path.
            return match frozen {
                Some(frozen) => self.build_frozen_kept_paren_doc(frozen, boundary_end),
                None => {
                    self.build_shell_sequence_doc(seq, expr_end, boundary_end, SeqLayout::Aligned)
                }
            };
        }

        // Two reasons the shell is RETAINED rather than stripped, and either sends the
        // comment inside the pair:
        //
        // - a line / own-line comment needs the parens on its own account (a bare line
        //   comment would swallow the following `;`);
        // - the calling POSITION parenthesizes this value anyway (`const x = (a = b)`),
        //   so the pair is in the output whatever this builder does.
        //
        // The second is what stops the deferral below from marching a comment across a
        // `)` the output still prints. That arm's licence is "this output erases the
        // `)`" — true for a plain value (`const a = (x /* t */);` → `const a = x; /* t */`),
        // false here, and a licence stops where its argument stops: the block comment of
        // a parenthesized assignment was relocating out of a surviving pair
        // (`const k = (x = y /* c */);` → `const k = (x = y); /* c */`) while the same
        // construct one comma over — a non-last declarator, with no terminator to defer
        // past — already kept it inside, and prettier keeps it inside in both.
        if self.shell_gap_retains_parens(expr_end, boundary_end, position_parens) {
            return match frozen {
                Some(frozen) => self.build_frozen_kept_paren_doc(frozen, boundary_end),
                None => self.build_expression_doc_keep_paren_comments(
                    expr,
                    boundary_end,
                    SeqLayout::Aligned,
                ),
            };
        }

        let d = self.d();
        let inner = wrap_in(self.build_shell_inner_doc(expr, frozen));

        // Every comment left here is a same-line block. Where the shell is the last
        // thing before a statement `;`, that block defers past the terminator — the same
        // answer the statement's own value-to-`;` gap gives once the shell is gone
        // ([`Printer::push_semicolon_with_gap_comments`] and its terminator sibling), which is
        // what makes one pass enough. Keying the choice on the stripped `)` instead cannot
        // reach a fixed point: this output erases that `)`, so the next pass reads the
        // comment as statement-trailing and moves it (`(x /* t */);` → `x /* t */;` →
        // `x; /* t */`). A shell that is NOT terminator-adjacent — a ternary CONSEQUENT
        // (whose gap ends at the `:`), an object value, a non-last declarator, a nested
        // assignment — keeps the block inline, where it is already its own fixed point.
        // A ternary ALTERNATE is terminator-adjacent and does reach this arm, which is
        // prettier's answer there too; the pair its branch may print is applied outside,
        // by `parenthesize_ternary_branch`, so nothing here crosses a surviving `)`.
        if tail == ShellTail::StatementTerminator
            && self.shell_meets_statement_terminator(boundary_end)
        {
            let mut parts: DocBuf = smallvec![inner];
            for comment in comments_to_emit_in_range(self.comments, expr_end, boundary_end) {
                let suffix = d.concat(&[d.text(" "), self.build_comment_doc(comment)]);
                parts.push(d.line_suffix(suffix));
            }
            return d.concat(&parts);
        }

        let comments = self.build_comments_between(expr_end, boundary_end, CommentSpacing::Leading);
        d.concat(&[inner, comments])
    }

    /// True when the next significant byte at or after `boundary_end` — the end of a
    /// stripped grouping shell — is the statement's `;`.
    ///
    /// The question a deferred trailing block must ask: is this gap the statement's
    /// terminator gap? Asking it of the SOURCE (not of the stripped `)`, which the
    /// output deletes) is what makes the answer survive the strip, so pass 2 — which
    /// sees the same `;` and no shell — agrees.
    fn shell_meets_statement_terminator(&self, boundary_end: u32) -> bool {
        let bytes = self.source.as_bytes();
        self.next_significant_byte(boundary_end, bytes.len() as u32)
            .is_some_and(|pos| bytes[pos] == b';')
    }

    /// The comments between an expression's end and a following `)`, as ready-to-append
    /// separator+comment parts, plus whether they force the broken `(⏎\texpr // c⏎)` frame.
    ///
    /// **A printer that synthesizes its own `(`…`)` owns this gap** — no enclosing emitter
    /// can see between those parens, so a comment left unclaimed here is DROPPED, not
    /// relocated. Both such printers call this: the stripped-paren restorer below and
    /// `build_jsdoc_cast_doc`, which lacked the gap entirely
    /// (`parenthesized/jsdoc_cast_trailing_paren_comment`).
    ///
    /// The separator is newline-aware — a comment the author put on a new line relative to
    /// the *previous item* (the expression, or the prior comment) breaks; otherwise it
    /// trails inline. Tracking the previous item rather than `expr_end` keeps a same-line
    /// group together (`x⏎ /* a */ // b`) while stopping a line comment that follows
    /// another comment from being swallowed by it. In the inline case every comment is a
    /// same-line block comment, so the rule collapses to a plain space.
    ///
    /// Returns `None` when the gap is empty, so callers keep their no-comment fast path.
    pub(crate) fn trailing_paren_comment_parts(
        &self,
        expr_end: u32,
        boundary_end: u32,
    ) -> Option<(DocBuf, bool)> {
        if !self.has_trailing_paren_comments(expr_end, boundary_end) {
            return None;
        }
        let d = self.d();

        // A line comment runs to end-of-line, an own-line block comment was authored on
        // its own line, and a multi-line block spans lines of its own — in every case the
        // shell already occupies more than one line, and a shell that breaks expands
        // rather than gluing its content to the `(` (the same rule
        // `build_expanded_parenthesized_union_opt` states for a breaking paren). Without
        // the `multiline` term the two authorings of one comment disagreed: `(⏎\tx // c⏎)`
        // expanded while `(x /* m1⏎m2 */)` stayed glued, at the same gap and for the same
        // reason.
        let needs_break =
            comments_to_emit_in_range(self.comments, expr_end, boundary_end).any(|c| {
                !c.is_block || c.multiline || self.has_newline_between(expr_end, c.span.start)
            });

        let mut parts = DocBuf::new();
        let mut prev_end = expr_end;
        for comment in comments_to_emit_in_range(self.comments, expr_end, boundary_end) {
            if self.has_newline_between(prev_end, comment.span.start) {
                parts.push(d.hardline());
            } else {
                parts.push(d.text(" "));
            }
            parts.push(self.build_comment_doc(comment));
            prev_end = comment.span.end;
        }
        Some((parts, needs_break))
    }

    /// Build expression doc re-adding the stripped grouping parens around trailing
    /// comments, producing `(expr /* c */)` or `(\n\texpr // c\n)`.
    ///
    /// Used where stripping the parens would relocate the comment — arrow bodies
    /// (prettier moves the comment into the params) and other non-sequence operands
    /// with an own-line/line trailing comment. Keeping the parens preserves the
    /// comment where the user wrote it. (Sequence operands take the dedicated
    /// `build_sequence_doc_value` path, which keeps the comment inside the sequence's
    /// own parens instead of adding a second pair.)
    pub(in crate::printer) fn build_expression_doc_keep_paren_comments(
        &self,
        expr: &internal::Expression<'_>,
        boundary_end: u32,
        layout: SeqLayout,
    ) -> DocId {
        let expr_end = expr.span().end;

        // A sequence self-parenthesizes, so it takes the shared arm rather than the
        // paren-restoring path below — `build_expression_doc` would emit its parens and
        // this method would re-wrap them (`() => ((1, 2, 3) /* c */)`). `layout` is the
        // caller's: an arrow body hangs its operands, the ASI-shell operands align.
        if let internal::Expression::SequenceExpression(seq) = expr {
            return self.build_shell_sequence_doc(seq, expr_end, boundary_end, layout);
        }

        let inner = self.build_expression_doc(expr);
        self.build_kept_paren_shell_doc(inner, expr_end, boundary_end)
            .unwrap_or(inner)
    }

    /// The RETAINED shell's rendering: the value inside the pair the author wrote, with
    /// the operand→`)` run behind it — inline where the run is a same-line block, on the
    /// operand's own indented line where a `//` or an own-line comment forces the pair
    /// open ([`Self::trailing_paren_comment_parts`] decides which).
    ///
    /// `inner` is whatever renders the value at this position, so the ordinary expression
    /// doc and a frozen verbatim slice share one rendering rather than two. `None` says
    /// the gap holds nothing to emit, leaving the caller its own bare form.
    fn build_kept_paren_shell_doc(
        &self,
        inner: DocId,
        expr_end: u32,
        boundary_end: u32,
    ) -> Option<DocId> {
        let d = self.d();
        let (comment_parts, needs_break) =
            self.trailing_paren_comment_parts(expr_end, boundary_end)?;

        Some(if needs_break {
            let mut indent_parts: DocBuf = smallvec![d.hardline(), inner];
            indent_parts.extend(comment_parts);
            d.concat(&[
                d.text("("),
                d.indent(d.concat(&indent_parts)),
                d.hardline(),
                d.text(")"),
            ])
        } else {
            let mut parts: DocBuf = smallvec![d.text("("), inner];
            parts.extend(comment_parts);
            parts.push(d.text(")"));
            d.concat(&parts)
        })
    }

    /// [`Self::build_kept_paren_shell_doc`] over a FROZEN value's verbatim slice — the
    /// retained arm of [`Self::build_shell_value_doc`] and the frozen sequence's own
    /// required pair, which is the same emission.
    ///
    /// The pair is unconditional here, unlike the unfrozen twin's bare fallback: every
    /// path that reaches this one prints a `)` in the output (the position's clarity pair,
    /// or the sequence's required one), so a gap that turns out to hold nothing to emit
    /// still owes the parens.
    ///
    /// The **arrow body** asks it directly rather than through [`Self::build_shell_value_doc`]:
    /// its retained-paren arm reassembles the body itself
    /// ([`Printer::build_arrow_expression_body`]), and answering that gap with a bare
    /// `parens()` would leave it with no emitter and DROP the comment inside it
    /// (`docs/comments.md` hazard 4). One emitter, so the frozen and unfrozen forms of every
    /// retained shell keep agreeing about where the comment renders.
    pub(crate) fn build_frozen_kept_paren_doc(&self, frozen: Span, boundary_end: u32) -> DocId {
        let inner = self.build_frozen_node_doc(frozen);
        self.build_kept_paren_shell_doc(inner, frozen.end, boundary_end)
            .unwrap_or_else(|| self.d().parens(inner))
    }

    /// Promote block comments that appear before an assignment operator to the LHS.
    ///
    /// In `a /* comment */ = b`, the comment is between `left.span().end` and `right.span().start`
    /// but positioned before the `=` in source. Prettier places such comments before the operator,
    /// so we promote them to the LHS doc.
    ///
    /// Returns the promoted comments doc (with leading space) and the new RHS comment start
    /// position, or None if no comments need promoting.
    ///
    /// **Block comments only** — the ones that can sit inline before the operator. A
    /// comment that cannot (a `//`, or a multiline block the author broke after)
    /// stays put and takes the operator's tail with it onto a continuation line;
    /// that gap is answered by [`Printer::build_operator_value_continuation`], which
    /// the caller consults first. Emitting such a comment here would swallow the
    /// operator into it; leaving it to the RHS emitter would relocate it *past* the
    /// operator.
    ///
    /// `op_pos` is the operator's offset, found once by the caller
    /// ([`Printer::find_operator_in_source`]) and shared with that gate.
    pub(crate) fn promote_comments_before_operator(
        &self,
        start: u32,
        op_pos: u32,
    ) -> Option<(DocId, u32)> {
        let d = self.d();
        // Collect block comments that appear before the operator
        let mut promoted_parts = DocBuf::new();
        let mut last_promoted_end = start;
        for comment in comments_to_emit_in_range(self.comments, start, op_pos) {
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
    ///
    /// The multi-byte sibling of [`Printer::find_equals_position`] (a bare `=`, with a
    /// midpoint fallback rather than `None`); both step over comments through
    /// [`tsv_lang::source_scan::skip_comment`], so a `//` or `/* */` in the gap can
    /// never hide the operator behind its text.
    pub(crate) fn find_operator_in_source(
        &self,
        start: u32,
        end: u32,
        operator: &str,
    ) -> Option<u32> {
        let bytes = self.source.as_bytes();
        let op_bytes = operator.as_bytes();
        let op_len = op_bytes.len();
        let end_usize = end as usize;
        let mut i = start as usize;

        while i + op_len <= end_usize {
            if let Some(past_comment) = tsv_lang::source_scan::skip_comment(bytes, i, end_usize) {
                i = past_comment;
                continue;
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

    /// The retained-shell form for a **statement value** whose authored grouping parens
    /// hold a `//` — `export default (⏎\tx // c⏎)`, `export = (⏎\tx // c⏎)`.
    ///
    /// These positions print no pair of their own around the value, so without this the
    /// comment falls to the statement's terminator gap and defers past the `)` and the
    /// `;`, onto a line that may already hold a `//` — where the two merge into one
    /// comment. Only a **line** comment needs the shell: a block trails without ending
    /// its line, so its relocation past the `;` is lossless and stable, and tsv matches
    /// prettier there.
    ///
    /// Returns the shell and the position just past the retained `)`, which is where the
    /// caller's terminator-gap scan resumes. `None` leaves the caller's plain path alone —
    /// no authored paren, or no `//` inside one.
    pub(crate) fn build_value_paren_line_comment_shell(
        &self,
        value_doc: DocId,
        value_end: u32,
        span_end: u32,
    ) -> Option<(DocId, u32)> {
        let close = self.value_paren_line_comment_close(value_end, span_end)?;
        let shell =
            self.build_paren_operand_comment_doc(value_end, close, value_doc, value_doc, ")")?;
        Some((shell, Self::past_grouping_close(close, span_end)))
    }

    /// Where the shell above closes, when it applies — the value's authored grouping `)`
    /// with a `//` inside it. Split out for the caller that must know the close *before*
    /// it can build the value doc (`export =` builds its value inside a keyword-header
    /// closure), so the two answer one question rather than two.
    pub(crate) fn value_paren_line_comment_close(
        &self,
        value_end: u32,
        span_end: u32,
    ) -> Option<u32> {
        let close = self.retained_grouping_close(value_end, span_end)?;
        self.has_line_comments_between(value_end, close)
            .then_some(close)
    }

    /// Where a caller's terminator-gap scan resumes: just past the retained `)`.
    pub(crate) fn past_grouping_close(close: u32, span_end: u32) -> u32 {
        close.saturating_add(1).min(span_end)
    }

    /// The one emitter for a comment the author wrote between a **parenthesized**
    /// operand and the `)` that closes its shell — `(x + y /* c */)!`,
    /// `(a?.b // c⏎)!`, `<T>(x /* c */)`.
    ///
    /// tsv keeps such a comment INSIDE the parens, where it was written; prettier
    /// relocates it past the `)` (cataloged as the non-null grouped-operand and
    /// angle-bracket assertion-operand divergences). Four constructs reach this gap and
    /// must answer it identically: the standalone non-null whose operand needs its
    /// parens (`build_non_null_doc`'s needs-parens arm), the chain's parenthesized
    /// base (`ChainNode::Base`'s `paren_comment_end`), the required-paren positions
    /// that never enter a chain — a `new` callee and a template tag
    /// (`build_sealed_non_null_paren_doc`) — and the angle-bracket type assertion,
    /// whose own span is what ends at the `)` (`build_ts_type_assertion_doc`, which
    /// calls from each of its two return paths).
    ///
    /// `flat_body` and `broken_body` are the same doc at every caller but the chain
    /// base, which alone has two renderings of its operand — the split is earned there
    /// and nowhere else.
    ///
    /// Returns `None` when the gap holds nothing to emit, leaving the caller to
    /// render its own bare parens — which is what makes the retention the comment's
    /// doing: an empty gap still strips a redundant shell.
    ///
    /// `broken_body` renders the line-comment layout — a `//` cannot trail inline
    /// before the `)` (it would swallow it), so the operand goes multiline with the
    /// comment inside; `flat_body` renders the inline block-comment one. `close` is
    /// what follows the operand: `")"` where a separate node prints the `!` (or where
    /// nothing does), `")!"` where this doc owns it.
    pub(crate) fn build_paren_operand_comment_doc(
        &self,
        start: u32,
        end: u32,
        flat_body: DocId,
        broken_body: DocId,
        close: &'static str,
    ) -> Option<DocId> {
        let d = self.d();
        if self.has_line_comments_between(start, end) {
            // Every comment in this gap was authored AFTER the operand — there is no
            // next node for one to lead — so the whole run trails, in authored order,
            // on the anchored emitter (the layout is vertical: the closer's hardline
            // below ends every line, and flushes the run's deferred `//`s; a boundary
            // instead would end the line first, landing a blank before the closer).
            // A chain-gap classification here is a category error: its `leading_*`
            // buckets would hoist an own-line comment above the operand.
            let mut body = DocBuf::with_capacity(3);
            body.push(broken_body);
            self.push_anchored_trailing_run(&mut body, start, end, RunLeadingBlank::Keep);
            return Some(self.compose_expanded_shell_doc(None, &body, close));
        }
        if self.has_comments_to_emit_between(start, end) {
            let trailing = self.build_chain_block_comments_doc(start, end, CommentSpacing::Leading);
            return Some(d.concat(&[d.text("("), flat_body, trailing, d.text(close)]));
        }
        None
    }
}
