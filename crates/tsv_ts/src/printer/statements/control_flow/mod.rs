// Control flow statement printing for TypeScript
//
// Statement families live in submodules; this mod.rs keeps the helpers they
// share (comment partitioning, keyword/paren comment placement, and the
// condition-group builders used across the statement families).
//
// - if_else.rs: if/else statements and else-clause layout
// - loops/: for / for-in / for-of headers and bodies (for_loop.rs), while, do-while (while_loop.rs)
// - switch.rs: switch statements and case bodies
// - try_jump.rs: try/catch/finally, throw, break/continue, labeled statements

mod if_else;
mod loops;
mod switch;
mod try_jump;

use smallvec::SmallVec;

use crate::ast::internal::{Expression, Statement, UnaryOperator};
use crate::printer::{CommentVec, LeadingGlue, Printer};
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;
use tsv_lang::{Comment, comments_to_emit_in_range};

/// Which of prettier's three comment runs a control-flow gap's off-the-anchor-line
/// comments form — the single fact all three of
/// [`Printer::build_comments_between_parts`]'s per-gap policies follow from.
///
/// The three gaps this builder serves are three different things in prettier's model,
/// and every difference between them is downstream of that: `handleIfStatementComments`
/// (and its `while`-like / `try` / for-x siblings) attaches a condition→`)` comment as a
/// **trailing** comment of the test, a header→body comment as a **leading** comment of
/// the body, and a `}`→continuation-keyword comment as a **dangling** comment of the
/// statement — then `printTrailingComment` / `printLeadingComment` /
/// `printDanglingComments` answer the questions below differently. Naming the run
/// rather than the policies is what stops a new gap from taking the glue rule of one and
/// the blank rule of another: they are **not** correlated (a leading run glues and
/// drops the leading blank, a trailing run glues and keeps it, a dangling run does
/// neither), so a flag per policy would have more states than are real.
#[derive(Clone, Copy)]
enum GapCommentRun {
    /// A **trailing** run of the thing behind it: the condition→`)` gap, where the
    /// comments trail the test inside the parens.
    ///
    /// A pair the author glued onto one line keeps that line —
    /// `printTrailingComment` gives the second comment of a glued pair a
    /// `lineSuffix([" ", …])` where the first took the `hardline` — which is the same
    /// answer [`Printer::trailing_run_hugs_previous`] states for every other trailing run
    /// in tsv. A blank above the run's first comment survives: there is no body `{` here
    /// for one to sit below, only the `)`. `blank_seed` is where that scan is floored.
    ///
    /// It is `isPreviousLineEmpty` — the blank must sit **directly above the comment** —
    /// because that is what `printTrailingComment` reads
    /// ([`Printer::push_adjacent_blank_hardline`]), and this is the one gap of the three
    /// that can hold re-emitted structure between the two: a grouping paren the
    /// condition's span starts inside. `if (⏎(⏎x⏎⏎)⏎/* c */⏎)` writes its blank inside
    /// the shell, so the shell's erasure takes the blank with it and the gap-wide
    /// reading keeps a blank prettier has no way to emit.
    Trailing { blank_seed: u32 },
    /// A **leading** run of what follows it: the comments introduce the body below them —
    /// every header→body gap (`if` / `while` / `do` / `for` / for-in / for-of / `switch` /
    /// `try` / `catch` / `finally`, block body or not).
    ///
    /// A pair the author glued onto one line **keeps that line**: `printLeadingComment`
    /// asks its glue test of the source right after each `*/`, so a run the author wrote
    /// glued stays glued and only a real newline between two comments starts a new line
    /// (`docs/comments.md` §Own-line-ness is a SOURCE question — a glued run given its
    /// own line does not own it, comment by comment). A blank above the run's **first**
    /// comment is dropped, so a body block's `{` never sits below one.
    Leading,
    /// A **dangling** run in the statement that spans it: the `}`→continuation-keyword
    /// gap (`else`, `catch`, `finally`, a do-while's `while`).
    ///
    /// Every comment takes a line of its own — `printDanglingComments` is a
    /// `join(hardline, …)`, so prettier **splits** a glued pair here and tsv matches it.
    /// That is why the glue rule is a per-run policy rather than one emitter-wide fix:
    /// the same authoring is one line at the other two gaps and two here. A blank above
    /// the first comment survives, measured from `blank_seed` — there is no body `{` to
    /// sit below it, so it separates two branches of a chain.
    Dangling { blank_seed: u32 },
}

impl GapCommentRun {
    /// Whether a pair the author glued onto one line keeps that line.
    fn glues(self) -> bool {
        !matches!(self, Self::Dangling { .. })
    }

    /// The position an author blank *above the run's first comment* is measured from, or
    /// `None` where that blank is dropped. One value rather than a policy flag plus a
    /// position that could contradict it.
    ///
    /// The same Trailing-vs-Leading fact [`crate::printer::RunLeadingBlank`] states for
    /// the anchored-run emitter — one concept, two shapes, deliberately not unified:
    /// that emitter's runs all start at a printed anchor so it needs only the policy,
    /// while this builder's gaps each floor the scan at their own seed, and the variant
    /// carries it so the two cannot disagree.
    fn blank_seed(self) -> Option<u32> {
        match self {
            Self::Leading => None,
            Self::Trailing { blank_seed } | Self::Dangling { blank_seed } => Some(blank_seed),
        }
    }

    /// Which END of the region between two comments this run's separator questions are
    /// asked from — **backward from the comment** (a trailing run) or **forward from the
    /// previous comment's `*/`** (a leading run). One fact, because prettier's two
    /// printers are each internally consistent about it: `printTrailingComment` reads
    /// `hasNewline(…, locStart(comment), { backwards: true })` and
    /// `isPreviousLineEmpty(locStart(comment))`, both anchored on the comment;
    /// `printLeadingComment` reads `hasNewline(locEnd(comment))` and
    /// `skipNewline(skipSpaces(locEnd(comment)))`, both anchored on the `*/`.
    ///
    /// It decides both of this emitter's per-comment questions — the **glue** test
    /// ([`Printer::trailing_run_hugs_previous`] vs [`Printer::comment_hugs_next`]) and the
    /// **blank** test ([`Printer::push_adjacent_blank_hardline`] vs
    /// [`Printer::push_blank_preserving_hardline`]) — and the two readings part on exactly
    /// one thing: re-emitted structure sitting between the two comments. Of the three gaps
    /// only the trailing one can hold any, a grouping paren the condition's span starts
    /// inside, and it broke both questions there: `if (⏎(⏎x⏎/* c1 */)⏎/* c2 */⏎)` WELDED
    /// `c2` onto `c1`'s line (the forward scan stops at the erased `)` and reports "no
    /// newline"), and `if (⏎(⏎x⏎⏎)⏎/* c */⏎)` kept a blank that belongs to the shell.
    ///
    /// ⚠️ **Not a general improvement — it is wrong for a leading run**, which is why it is
    /// keyed on the run. At the `(`→condition gap (a leading run, emitted elsewhere)
    /// prettier really does read forward from the `*/`: it keeps an author blank written
    /// *above* the shell (`if (⏎/* c1 */⏎⏎(⏎/* c2 */⏎x⏎)⏎)`) that the backward reading
    /// would drop, and it welds across a `(` glued to a comment (`/* c1 */(⏎/* c2 */`)
    /// exactly as tsv does.
    fn reads_backward_from_comment(self) -> bool {
        matches!(self, Self::Trailing { .. })
    }
}

/// What happens to a comment the author wrote on the condition's `(` line — the one
/// axis the paren-headed constructs part on in
/// [`Printer::build_condition_group_with_comments`]. Both values are pinned, so
/// neither can absorb the other.
#[derive(Clone, Copy)]
enum OpenParenLineComments {
    /// The comment joins the shared leading run below the opening separator, which
    /// places it by the run's own glue rule rather than on a line of its own —
    /// prettier's placement, pinned by
    /// [`condition_comment`](../../../../../../tests/fixtures/typescript/statements/if/condition_comment/)'s
    /// `unformatted_compact`, whose two arms are the whole rule: `if (// c` drops the
    /// comment to its own line, `if (/* c */ x` keeps it on the condition's. Every
    /// construct but do-while.
    Normalize,
    /// The comment stays on the `(` line — the do-while sanction
    /// ([`open_paren_comment_prettier_divergence`](../../../../../../tests/fixtures/typescript/statements/do_while/open_paren_comment_prettier_divergence/)),
    /// where prettier moves the comment out of the parens entirely: past the `;` for a
    /// `//`, ahead of the `while` for a block comment.
    ///
    /// ⚠️ The sanction is about the comment's **position**, and stops there — the author's
    /// SPACING is normalized in both arms, never preserved. Prettier keeps a block comment
    /// inside the parens whenever the condition follows it on the `(` line, so that shape is
    /// a live spacing oracle and tsv matches it whole: nothing after the `(`, one space per
    /// gap between two comments
    /// ([`open_paren_comment_run`](../../../../../../tests/fixtures/typescript/statements/do_while/open_paren_comment_run/)).
    /// Where the run holds the `(` line open prettier relocates it out and leaves no oracle,
    /// so tsv writes the space it writes at every other opening delimiter it keeps a comment
    /// on — `fn( // c`, `new Foo( /* paren */`. See
    /// [`Printer::push_paren_line_comment_bucket`] for the table.
    Preserve,
}

impl<'a> Printer<'a> {
    /// Build a control-flow *body* whose empty block form collapses (`do {} while (cond)`,
    /// C-style `for (…) {}`). The generic `build_statement_doc` dispatch EXPANDS a
    /// statement-position empty block to `{\n}`, so a collapse-context body must build a
    /// `BlockStatement` directly via the collapse path; a non-block body keeps the generic
    /// dispatch (a non-empty block is identical either way — `expand_empty` only affects the
    /// empty case). The `while` handler and `catch` inline their own block builds (extra
    /// close-paren handling / an always-block body), so they don't route through here.
    fn build_collapsing_body_doc(&self, body: &Statement<'_>) -> DocId {
        if let Statement::BlockStatement(block) = body {
            self.build_block_statement_doc(block)
        } else {
            // Non-block body: its container is the control-flow statement
            // itself, never Program/BlockStatement, so a bare string statement
            // here is never directive-prologue eligible.
            self.build_statement_doc(body, false)
        }
    }

    /// Partition comments between two positions into inline vs own-line.
    ///
    /// Returns `(inline_with_prev, own_line, inline_with_next)` where:
    /// - `inline_with_prev`: Comments on the same line as `prev_end`
    /// - `own_line`: Comments on their own line (not same line as prev or next)
    /// - `inline_with_next`: Comments on the same line as `next_start`
    ///
    /// ⚠️ **The primitive, not the API — emit through one of the two wrappers below.**
    /// No gap emitter wants these three buckets *as three*: every one folds
    /// `inline_with_next` into `own_line` ([`Self::partition_comments_trailing_vs_own_line`]),
    /// and the header→body gaps then pull a body-glued block back out of it
    /// ([`Self::partition_body_gap_comments`]). Destructuring the tuple here is how the
    /// third bucket got bound to `_` and every comment in it silently dropped, so the
    /// wrappers exist to make that fold unforgettable rather than conventional.
    ///
    /// ⚠️ **These are LINE buckets — which line a comment leaves the anchor for — and
    /// they say nothing about how the run's comments sit relative to EACH OTHER.** The
    /// fixed anchors are right for that question (a glued pair shares one physical line,
    /// so both halves answer `is_same_line(prev_end, …)` alike) and wrong for any other:
    /// asked whether a comment ends the line, `is_same_line(c.span.end, next_start)`
    /// measures across every later comment in the gap. Whether the author glued a pair is
    /// therefore the emitter's question, asked per comment against its own neighbour
    /// ([`Self::build_comments_between_parts`], gated by [`GapCommentRun`]) — reading it
    /// off these buckets is what split `if (a)⏎/* c1 */ /* c2 */⏎{}` onto two lines.
    fn partition_comments_by_line(
        &self,
        prev_end: u32,
        next_start: u32,
    ) -> (CommentVec<'a>, CommentVec<'a>, CommentVec<'a>) {
        let mut inline_prev = SmallVec::new();
        let mut own_line = SmallVec::new();
        let mut inline_next = SmallVec::new();

        for comment in comments_to_emit_in_range(self.comments, prev_end, next_start) {
            let same_line_as_prev = self.is_same_line(prev_end, comment.span.start);
            let same_line_as_next = self.is_same_line(comment.span.end, next_start);

            if same_line_as_prev {
                inline_prev.push(comment);
            } else if same_line_as_next {
                inline_next.push(comment);
            } else {
                own_line.push(comment);
            }
        }

        (inline_prev, own_line, inline_next)
    }

    /// The two buckets the **anchor-trailing vs own-line** rule needs: comments
    /// sharing `prev_end`'s line trail the anchor, and *everything else takes its
    /// own line*.
    ///
    /// This is [`Self::partition_comments_by_line`] with its third bucket folded
    /// into the second, and it exists so that fold cannot be forgotten: a comment
    /// sharing the FOLLOWING token's line is the bucket a two-way destructuring
    /// silently drops (`if (a⏎/* c */) {` printed no comment at all, in every
    /// construct whose head this builds — `if`, `while`, `do…while`, `switch`,
    /// `catch`). Callers that must treat that bucket differently — the header→body
    /// gaps, which let a block comment glued to the body lead it inline — take the
    /// three-way form instead, deliberately.
    fn partition_comments_trailing_vs_own_line(
        &self,
        prev_end: u32,
        next_start: u32,
    ) -> (CommentVec<'a>, CommentVec<'a>) {
        let (trailing, mut own_line, inline_next) =
            self.partition_comments_by_line(prev_end, next_start);
        // Order survives the fold: every comment in the range ends before
        // `next_start`, so one sharing its line is always last.
        own_line.extend(inline_next);
        (trailing, own_line)
    }

    /// The three buckets a **header→body** gap needs: comments trailing the anchor,
    /// comments taking their own line, and the **body-glued** run that leads the body
    /// inline (`) /* c */ {`).
    ///
    /// [`Self::partition_comments_trailing_vs_own_line`] plus the one carve-out that
    /// bucket is separately good for — and the glue split must run **before** the fold,
    /// not after: [`Printer::split_glued_comments`] takes a source-keyed *suffix*
    /// (`comment_hugs_next` = a block with no newline after it), so over the folded run
    /// it would also claim an own-line block that merely shares its line with the next
    /// comment (`/* a */ /* b */⏎body`), gluing a comment the author left above the body.
    fn partition_body_gap_comments(
        &self,
        gap_start: u32,
        body_start: u32,
    ) -> (CommentVec<'a>, CommentVec<'a>, CommentVec<'a>) {
        let (anchor_line, mut own_line, inline_next) =
            self.partition_comments_by_line(gap_start, body_start);
        let (rest, glued) = self.split_glued_comments(inline_next);
        own_line.extend(rest);
        (anchor_line, own_line, glued)
    }

    /// Whether any comment between two positions falls in the `inline_prev` bucket of
    /// [`Self::partition_comments_by_line`] — a comment trailing the anchor on its
    /// own line — asked without building the three `SmallVec`s a layout gate would
    /// then throw away to read one `is_empty`.
    ///
    /// This one really is the partition's bucket: a fixed `prev_end` anchor, matching
    /// the emitter it gates. [`Self::has_isolated_comment_between`] no longer is, and
    /// its own doc says why — do not read the two as a pair.
    fn has_anchor_trailing_comment_between(&self, prev_end: u32, next_start: u32) -> bool {
        comments_to_emit_in_range(self.comments, prev_end, next_start)
            .any(|comment| self.is_same_line(prev_end, comment.span.start))
    }

    /// Whether any comment between two positions **owns a line of its own** — the `for`
    /// header's expansion gate. For a caller that needs only the *existence* answer this
    /// short-circuits on the first hit and allocates nothing.
    ///
    /// Isolation is judged by the neighbouring **clause positions**, and for **every**
    /// comment kind — both halves of what separates this from
    /// [`Printer::comment_isolated_on_its_line`], which reads the source and calls every
    /// line comment isolated (a `//` forces the list open wherever it sits). Here a `//`
    /// sharing the previous clause's line is that clause's trailing comment, so it must
    /// not answer yes; and the clause boundaries are the right anchors because this gap's
    /// re-emitted text — a `;` — is what the emitters partition around.
    ///
    /// ⚠️ **But a comment's neighbour is the next COMMENT when there is one**, on both
    /// sides — hence the advancing cursor and the one-ahead peek, the shape
    /// [`Printer::any_comment_on_page_with_next`] states for the source reading. Two
    /// clause-anchored questions asked of each comment independently read a run the author
    /// glued onto one line (`x = 0;⏎/* c1 */ /* c2 */⏎b`) as isolated — `c1`'s trailing
    /// side gets measured against the *clause*, right across `c2` — and force a header
    /// open that prettier keeps flat (`docs/comments.md` §Own-line-ness is a SOURCE
    /// question: a glued run given its own line does not own it).
    ///
    /// ⚠️ **That is why this is NO LONGER the `own_line` bucket of
    /// [`Self::partition_comments_by_line`]**, which it used to be an allocation-free
    /// restatement of. The two now answer different questions rather than the same one
    /// two ways: the partition classifies which LINE a comment leaves the anchor for —
    /// where its fixed anchors are exact — while this asks whether a comment owns a line,
    /// which only its own neighbours can answer. Both readings are now spelled where they
    /// belong; the header→body gap keeps a glued run glued through its emitter
    /// ([`Self::build_comments_between_parts`]), the same place this gate's own emitter
    /// (`Printer::push_for_clause_leading_section` → `push_leading_comment_run`) keeps it.
    fn has_isolated_comment_between(&self, prev_end: u32, next_start: u32) -> bool {
        let mut comments =
            comments_to_emit_in_range(self.comments, prev_end, next_start).peekable();
        let mut prev = prev_end;
        while let Some(comment) = comments.next() {
            let next = comments.peek().map_or(next_start, |c| c.span.start);
            if !self.is_same_line(prev, comment.span.start)
                && !self.is_same_line(comment.span.end, next)
            {
                return true;
            }
            prev = comment.span.end;
        }
        false
    }

    /// Does a header→body gap's comment run force the body onto its own line?
    ///
    /// Two independent reasons, and the second is easy to miss: a **line** comment must
    /// break wherever it sits (a `//` would otherwise swallow the body), and **any**
    /// comment the author put on its own line must break to keep that line. Only a
    /// block comment trailing the anchor leaves the body free to stay inline.
    ///
    /// The single statement of that question, for the caller that must choose an
    /// inline-capable layout *before* building one
    /// ([`Self::build_adjust_clause_with_comments`]) **and** for the emitter it then
    /// calls ([`Self::push_header_to_body_gap`]). Those two must agree — the caller
    /// commits to a layout the emitter has to deliver — so the emitter asks this rather
    /// than restating it over its own buckets, which is what it used to do.
    fn header_to_body_gap_breaks(&self, gap_start: u32, body_start: u32) -> bool {
        comments_to_emit_in_range(self.comments, gap_start, body_start)
            .any(|comment| !comment.is_block || !self.is_same_line(gap_start, comment.span.start))
    }

    /// Build comments between a keyword and its `(`, preserving position.
    ///
    /// Returns the full keyword→`(` transition (leading space included) for comments
    /// between `keyword_end` and `open_paren`, or `None` when there are none. A block
    /// comment glues with a trailing space (`if /* c */ ` → `if /* c */ (a)`); a line
    /// comment trails the keyword and breaks so the `(` drops to the next line and
    /// can't be swallowed (`if // c` + hardline → `if // c⏎(a)`). The caller pushes a
    /// bare `(` after this (no leading space) — see [`Self::build_keyword_paren_comments`]
    /// call sites. Prettier relocates the comment instead (into the parens for
    /// `if`/`while`/`switch`, past the header for `for`); tsv preserves the authored
    /// position uniformly.
    fn build_keyword_paren_comments(
        &self,
        keyword_end: u32,
        open_paren: Option<u32>,
    ) -> Option<DocId> {
        let op = open_paren?;
        if !self.has_comments_to_emit_between(keyword_end, op) {
            return None;
        }
        let d = self.d();
        let mut parts = DocBuf::new();
        for comment in comments_to_emit_in_range(self.comments, keyword_end, op) {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
            // Block: glue with a trailing space so the `(` follows directly. Line: a
            // hardline drops the `(` to the next line so the `//` can't swallow it.
            self.push_comment_kind_separator(&mut parts, comment);
        }
        Some(d.concat(&parts))
    }

    /// Push a control-flow head opener — `keyword`, any `keyword`→`(` comment
    /// (`keyword_comments`, which already carries its own trailing space/break), then
    /// `(`. With no comment it emits a plain `keyword (`. Shared by `if`/`while`/
    /// `switch`/`catch` and the plain-`for` header so the `keyword`→`(` line-comment
    /// break is uniform (`if // c⏎(a)`) rather than swallowing the `(`.
    fn push_keyword_open_paren(
        &self,
        parts: &mut DocBuf,
        keyword: &'static str,
        kc: Option<DocId>,
    ) {
        let d = self.d();
        parts.push(d.text(keyword));
        if let Some(kc) = kc {
            parts.push(kc);
            parts.push(d.text("("));
        } else {
            parts.push(d.text(" ("));
        }
    }

    /// Emit a `}`→continuation-keyword gap: its comments, then the separator before
    /// the keyword. The caller pushes the keyword itself.
    ///
    /// The single place that question is answered, for every keyword that continues a
    /// construct across a `}` — `else`, `catch`, `finally`, and a do-while's `while`.
    /// Comments keep their authored position (trailing stays trailing, own-line keeps
    /// its own line), and the keyword hugs the `}` only when the previous part was a
    /// block, every comment trailed it, and none was a `//` — a line comment there
    /// would swallow the keyword.
    ///
    /// A blank above the first own-line comment **survives** here (`blank_seed` =
    /// `Some`): there is no body `{` to sit below it, so it separates two branches of a
    /// chain and is real authoring intent. Prettier agrees at `else`; at a do-while's
    /// `while` it relocates the comment into the condition parens instead, so it is no
    /// oracle there and tsv's own stance governs. The mirror of
    /// [`Self::push_header_to_body_gap`], which drops it.
    ///
    /// `prev_is_block` is false only for an `if` with a non-block consequent
    /// (`if (a) expr;⏎else …`); a `try`/`catch` block and a do-while body block are
    /// always blocks.
    fn push_block_to_keyword_gap(
        &self,
        parts: &mut DocBuf,
        gap_start: u32,
        keyword_start: u32,
        prev_is_block: bool,
    ) {
        let d = self.d();
        if !self.has_comments_to_emit_between(gap_start, keyword_start) {
            parts.push(if prev_is_block {
                d.text(" ")
            } else {
                d.hardline()
            });
            return;
        }

        // A comment sharing the keyword's line takes its own line above it rather
        // than being dropped: `} \n /* b */ else {` → `}\n/* b */\nelse {`.
        let (inline_prev, all_own_line) =
            self.partition_comments_trailing_vs_own_line(gap_start, keyword_start);

        self.build_comments_between_parts(
            parts,
            &inline_prev,
            &all_own_line,
            GapCommentRun::Dangling {
                blank_seed: gap_start,
            },
        );

        let keyword_hugs_brace =
            prev_is_block && all_own_line.is_empty() && inline_prev.iter().all(|c| c.is_block);
        parts.push(if keyword_hugs_brace {
            d.text(" ")
        } else {
            d.hardline()
        });
    }

    /// Emit a header→body gap: its comments, then the separator before the body. The
    /// caller pushes the anchor token (`)`, or the `try`/`catch`/`finally` keyword)
    /// before this, and the body after.
    ///
    /// The single place that question is answered, for every construct whose body
    /// follows a header — `if` / `while` / `for-in` / `for-of` (via
    /// [`Self::append_close_paren_with_comments`]), the C-style `for` (whose `)` is
    /// already inside its header doc), and `try` / `catch (e)` / bare `catch` /
    /// `finally`. Only the anchor differed between them, which is why it stays the
    /// caller's job.
    ///
    /// Every comment keeps its **authored position** — trailing the anchor stays
    /// trailing, own-line keeps its own line — regardless of kind. A line comment
    /// additionally forces a hardline wherever it sits, so the `//` can't swallow the
    /// body.
    ///
    /// ⚠️ A block comment is **not** flexible here. This emitter used to normalize an
    /// own-line block comment to trail the anchor (`if (a)⏎/* b */⏎{` → `if (a) /* b */ {`)
    /// on the premise that prettier does the same; measured, prettier **preserves** it in
    /// every construct and body kind, and the line-comment path was already preserving.
    /// Own-line-ness is authoring signal, so relocating it also cut against tsv's own
    /// comment-position stance. Pinned by
    /// `syntax/comments/head_body_own_line_block_comment`.
    ///
    /// A blank above the first own-line comment is **dropped** (`blank_seed` = `None`),
    /// so a body block's `{` never sits below one. That is consistent with tsv's own
    /// handling when `{` is on the header line (`if (a) {⏎⏎// c` also collapses), and
    /// prettier drops it here too. The mirror of [`Self::push_block_to_keyword_gap`],
    /// which keeps it.
    ///
    /// **Precondition**: the gap holds at least one comment to emit. Each caller's
    /// no-comment fast path differs (`") "` vs a bare space), so it stays theirs.
    fn push_header_to_body_gap(&self, parts: &mut DocBuf, gap_start: u32, body_start: u32) {
        let d = self.d();
        // A comment sharing the body's line is not trailing the anchor, so it does not
        // stay up there — but a **block** one glued to the body leads it inline; the
        // rest take their own line.
        let (inline_prev, own_line, glued) =
            self.partition_body_gap_comments(gap_start, body_start);

        self.build_comments_between_parts(parts, &inline_prev, &own_line, GapCommentRun::Leading);

        // Anything on its own line already forced a break; otherwise only a `//` does,
        // since it would swallow the body. A glued comment is one of the own-line ones:
        // `partition_comments_by_line` hands the anchor's line to `inline_prev`, so
        // everything in `glued` was authored *below* the anchor and keeps that line —
        // gluing it to the body must not also hoist it up onto the header's.
        //
        // Asked through the shared predicate rather than re-derived from the buckets
        // (`!own_line.is_empty() || !glued.is_empty() || inline_prev.iter().any(|c|
        // !c.is_block)`, which is the same set said in terms of the partition): the
        // caller that must pick an inline-capable layout *before* building one asks
        // `header_to_body_gap_breaks` directly, so a bucket-shaped restatement here is a
        // gate and its emitter answering one question two ways — agreeing today only by
        // hand, and silently diverging the moment a bucket boundary moves.
        if self.header_to_body_gap_breaks(gap_start, body_start) {
            parts.push(d.hardline());
        } else {
            parts.push(d.text(" "));
        }
        self.push_glued_comment_run(parts, &glued);
    }

    /// The header→body gap for a **non-block** body: the comment run, then the body on
    /// its own line, all sharing the body's indent (`if (a)⏎↹// c⏎↹fn();` — prettier's
    /// `adjustClause` shape). The caller pushes its own anchor (`)` / `else`) first.
    ///
    /// ⚠️ **Position-agnostic for a `//` only**, unlike [`Self::push_header_to_body_gap`];
    /// a block comment keeps its authored line here too. For the `)` gap that split is
    /// prettier's, measured on all four combinations:
    ///
    /// | body | authored trailing `)` | authored own-line |
    /// | --- | --- | --- |
    /// | **non-block**, `//` | moved to its own line | own line |
    /// | **non-block**, `/* */` | stays trailing | own line |
    /// | **block**, either | stays trailing | own line |
    ///
    /// So only the non-block `//` normalizes. That is not tsv relocating a comment
    /// against its own stance: a non-block body has no `{` for a trailing `//` to anchor
    /// against, and prettier does the same, so there is nothing to diverge over. Routing
    /// *that* case through the position-preserving emitter regressed
    /// `if/head_body_nonblock_comment`, which pins the trailing authoring normalizing to
    /// the own-line form under **both** formatters.
    ///
    /// A blank line between two own-line comments survives via the shared comment-run
    /// builder ([`Self::build_comments_between_parts`], on the same
    /// [`Printer::push_blank_preserving_hardline`] rule `build_comments_between` follows).
    ///
    /// ⚠️ The **`else`**→non-block gap does NOT share this — see
    /// [`Self::push_indented_else_to_body_gap`], which preserves the authored position.
    fn push_indented_header_to_body_gap(
        &self,
        parts: &mut DocBuf,
        gap_start: u32,
        body_start: u32,
        body_doc: DocId,
    ) {
        let d = self.d();
        // A block comment glued to the body leads it inline rather than taking a line of
        // its own.
        let (anchor_line, own_line, glued) =
            self.partition_body_gap_comments(gap_start, body_start);

        // Only a **block** comment can stay on the anchor's line. A line comment authored
        // trailing `)` normalizes to its own line — that is the position-agnostic half
        // this gap is pinned to (`if/head_body_nonblock_comment`), and it is prettier's
        // behavior too: a non-block body has no `{` for a trailing comment to anchor
        // against. Source order survives the split because a comment trailing `)`
        // precedes every own-line one.
        let mut inline_prev: CommentVec<'a> = SmallVec::new();
        let mut run: CommentVec<'a> = SmallVec::new();
        for comment in anchor_line {
            if comment.is_block {
                inline_prev.push(comment);
            } else {
                run.push(comment);
            }
        }
        run.extend(own_line);

        let mut inner = DocBuf::new();
        self.build_comments_between_parts(&mut inner, &inline_prev, &run, GapCommentRun::Leading);
        inner.push(d.hardline());
        self.push_glued_comment_run(&mut inner, &glued);
        inner.push(body_doc);
        parts.push(d.indent(d.concat(&inner)));
    }

    /// The `else`→**non-block** body gap: the comment run keeps its **authored
    /// position** and the body is indented beneath (`} else // c⏎↹expr;`).
    ///
    /// ⚠️ Deliberately position-**preserving**, where the `)`→non-block gap
    /// ([`Self::push_indented_header_to_body_gap`]) normalizes. Prettier drops a
    /// trailing comment onto its own line in *both*, so keeping it here is a
    /// **sanctioned divergence** (`if/else_line_comment_nonblock_prettier_divergence`,
    /// with a README and an `output_prettier`), while the `)` gap deliberately matches
    /// prettier (`if/head_body_nonblock_comment`, a plain fixture). Two gaps, one
    /// question, two pinned answers — **do not merge them**; routing `else` through the
    /// normalizing helper silently deletes the sanctioned behavior.
    fn push_indented_else_to_body_gap(
        &self,
        parts: &mut DocBuf,
        gap_start: u32,
        body_start: u32,
        body_doc: DocId,
    ) {
        let d = self.d();
        let mut inner = DocBuf::new();
        self.push_header_to_body_gap(&mut inner, gap_start, body_start);
        inner.push(body_doc);
        parts.push(d.indent(d.concat(&inner)));
    }

    /// Build docs for comments between statement parts (e.g., between `}` and `else`).
    ///
    /// - `inline_prev` — emitted on the anchor's line, each after a space
    /// - `own_line` — the comments that leave the anchor's line, emitted one line each
    ///   except where the author glued a pair (see `run`); an authored blank line
    ///   *between* two of them always survives (it separates two distinct remarks)
    /// - `run` — which of prettier's three runs these comments form, the single fact all
    ///   three per-gap policies below follow from ([`GapCommentRun`])
    ///
    /// The glue test and the blank both go through a shared emitter either way — the run
    /// picks which end each reads from ([`GapCommentRun::reads_backward_from_comment`]):
    /// [`Printer::trailing_run_hugs_previous`] + [`Self::push_adjacent_blank_hardline`]
    /// for the trailing run (anchored on the comment) and [`Printer::comment_hugs_next`] +
    /// [`Self::push_blank_preserving_hardline`] for the other two (anchored on the `*/`). **Both read what sits between the newlines rather than counting
    /// them**, and that is the point: a gap's endpoint is a node span that excludes a
    /// delimiter the printer still emits — a grouping paren the condition's span starts
    /// inside, `if (⏎(⏎x⏎)⏎/* c */⏎)` — so a counting scan reads that `)`'s two newlines
    /// as an author blank and **fabricates** one. This emitter re-derived the rule and
    /// paid exactly that bug, in a form every standing gate is blind to: the fabricated
    /// line is its own fixed point.
    ///
    /// ⚠️ The strict arm is still not the `blank_scan_start`/`blank_scan_end` pair
    /// CLAUDE.md §Comment Handling prescribes for in-source scans — a multiline comment
    /// *this caller does not emit* would carry its own blank line into it. Sound here
    /// *only* because **no comment in these gaps can be owned**: both
    /// `owned_by_node = true` sites live in `parser/expression.rs` and bind to expression
    /// starts, and none of the three gaps contains one — a `}`→continuation-keyword gap
    /// is followed by a keyword, a header→body gap by the body's own leading position,
    /// and a condition→`)` gap by the `)`. So the to-emit set and the in-source set
    /// coincide. Verified 2026-07-20, re-checked for the condition gap 2026-08-14. If
    /// ownership ever extends to a token these gaps can hold, this must move to the
    /// helpers (the adjacent arm already carries its floor).
    fn build_comments_between_parts(
        &self,
        parts: &mut DocBuf,
        inline_prev: &[&Comment],
        own_line: &[&Comment],
        run: GapCommentRun,
    ) {
        let d = self.d();
        // Trailing comments stay on same line
        for comment in inline_prev {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }

        // `None` until a comment has been emitted, so the *first* comment's blank is
        // checked only when the gap seeds one; every later blank is always checked.
        let mut prev_end = run.blank_seed();
        // The previous comment *this loop* emitted, and deliberately not the last
        // `inline_prev` one: a comment glued to a comment that trails the anchor is a
        // different question (prettier drops the whole run below the header there, via
        // `shouldPrintLeadingHardline`, a rule tsv does not have), so gluing it here
        // would reach a third form neither formatter produces.
        let mut prev: Option<&Comment> = None;
        for comment in own_line {
            // The author wrote this comment on the previous one's line, so the run keeps
            // that line. Which spelling of that question is the run's
            // ([`GapCommentRun::reads_backward_from_comment`]) — a blank cannot sit here
            // either way, which is why this arm skips the scan.
            let hugs = if run.reads_backward_from_comment() {
                self.trailing_run_hugs_previous(prev, comment.span.start)
            } else {
                prev.is_some_and(|prev| self.comment_hugs_next(prev))
            };
            if run.glues() && hugs {
                parts.push(d.text(" "));
            } else if let Some(end) = prev_end {
                if run.reads_backward_from_comment() {
                    self.push_adjacent_blank_hardline(parts, end, comment.span.start);
                } else {
                    self.push_blank_preserving_hardline(parts, end, comment.span.start);
                }
            } else {
                // The run's first comment in a gap that drops a blank above it — no scan
                // to make, since there is no position to measure one from.
                parts.push(d.hardline());
            }
            parts.push(self.build_comment_doc(comment));
            prev = Some(*comment);
            prev_end = Some(comment.span.end);
        }
    }

    /// Emit an **EmptyStatement body's** gap: its comments, the separator, and the `;`.
    /// The caller pushes the anchor token before this (`)`, or ` else`).
    ///
    /// The single place that question is answered, for both constructs whose body can be
    /// an `EmptyStatement` and whose gap can hold comments:
    ///
    /// - No comments: `if (a);` / `} else;` — no space, so the `;` hugs the anchor
    /// - Block comments: `if (a) /* c */ ;` / `} else /* c */ ;`
    /// - Line comments: `if (a) // c⏎;` / `} else // c⏎;` — the `//` must break or it
    ///   swallows the `;`, and an EmptyStatement body swallowed whole is lost content
    ///
    /// Both sites spelled this out separately (one through `CommentSpacing::None` plus
    /// its own `" "`, one through `Leading`, which is the same doc) — the shape where a
    /// later rule change lands on one construct and not the other.
    fn push_empty_statement_gap(&self, parts: &mut DocBuf, gap_start: u32, empty_start: u32) {
        let d = self.d();
        if !self.has_comments_to_emit_between(gap_start, empty_start) {
            parts.push(d.text(";"));
            return;
        }
        parts.push(self.build_inline_comments_between_doc(gap_start, empty_start));
        if self.has_line_comments_between(gap_start, empty_start) {
            parts.push(d.hardline());
            parts.push(d.text(";"));
        } else {
            parts.push(d.text(" ;"));
        }
    }

    /// Append `)` + an empty statement body's gap ([`Self::push_empty_statement_gap`]).
    fn append_close_paren_empty_stmt_with_comments(
        &self,
        parts: &mut DocBuf,
        paren_end: u32,
        empty_start: u32,
    ) {
        parts.push(self.d().text(")"));
        self.push_empty_statement_gap(parts, paren_end, empty_start);
    }

    /// Push `)` and the gap that follows it — the `)` anchor for
    /// [`Self::push_header_to_body_gap`], which owns the gap's comment rules. The caller
    /// pushes what comes after (a block body, or `switch`'s `{`).
    ///
    /// Used wherever a `)` is followed by a **block**: `if` / `while` / for-in/for-of
    /// bodies and the `switch` body brace. A **non-block** for-in/for-of body takes
    /// `append_close_paren_with_non_block_body` instead, which also indents.
    fn append_close_paren_with_comments(
        &self,
        parts: &mut DocBuf,
        paren_end: u32,
        body_start: u32,
    ) {
        let d = self.d();
        if self.has_comments_to_emit_between(paren_end, body_start) {
            parts.push(d.text(")"));
            self.push_header_to_body_gap(parts, paren_end, body_start);
        } else {
            parts.push(d.text(") "));
        }
    }

    /// Build an adjust-clause doc with head-body comment handling for non-block bodies.
    ///
    /// Used by if/while for `stmt (cond) /* c */ fn();` and `stmt (cond) // c\n fn();`.
    /// Returns the full `keyword (condition) body` doc including comments when present.
    ///
    /// `head_parts` are the docs before the `)` (e.g., `["if (", condition_group]`).
    fn build_adjust_clause_with_comments(
        &self,
        head_parts: &[DocId],
        paren_end: u32,
        body_start: u32,
        body_doc: DocId,
    ) -> DocId {
        let d = self.d();
        if self.has_comments_to_emit_between(paren_end, body_start) {
            let mut parts: DocBuf = SmallVec::from_slice(head_parts);
            parts.push(d.text(")"));
            if self.header_to_body_gap_breaks(paren_end, body_start) {
                // A `//` (which would swallow the body) or any own-line comment forces
                // the break: stmt (cond)\n\t// comment\n\tfn();
                self.push_indented_header_to_body_gap(&mut parts, paren_end, body_start, body_doc);
                d.concat(&parts)
            } else {
                let comment_doc =
                    self.build_inline_comments_between_doc_no_leading_space(paren_end, body_start);
                // Block comment stays with statement: stmt (cond) /* c */ fn();
                // When broken: stmt (cond)\n\t/* c */ fn();
                parts.push(d.indent(d.concat(&[d.line(), comment_doc, d.text(" "), body_doc])));
                d.group(d.concat(&parts))
            }
        } else {
            let mut parts: DocBuf = SmallVec::from_slice(head_parts);
            parts.push(d.text(")"));
            parts.push(d.indent_line(body_doc));
            d.group(d.concat(&parts))
        }
    }

    /// Prettier's `shouldInlineCondition` (miscellaneous.js): a `!` / `!!`-negated
    /// parenthesized logical condition (`if (!(a || b))`, `while (!!(a && b))`) hugs
    /// the `(` instead of breaking onto its own line, so the whole statement reads
    /// `if (!(` … `)) {` rather than `if (⏎ !(…) ⏎) {`.
    ///
    /// True iff the test is `!X` or `!!X` (but not `!!!X`) where `X` is a *logical*
    /// binary expression. This matches only the `printIfOrWhileConditionOrWithStatementObject`
    /// callers (`if` / `while` / `do-while`), never `switch`. Comments on the condition
    /// disable inlining upstream — the caller only reaches the bare-doc path when the
    /// condition parens hold no comments.
    fn condition_should_inline_negation(&self, test: &Expression<'_>) -> bool {
        let Expression::UnaryExpression(outer) = test else {
            return false;
        };
        if outer.operator != UnaryOperator::Bang {
            return false;
        }
        // Peel one optional inner `!` (so `!` and `!!` qualify; a third `!` leaves a
        // UnaryExpression here and fails the logical-binary check below).
        let inner = match outer.argument {
            Expression::UnaryExpression(u) if u.operator == UnaryOperator::Bang => u.argument,
            other => other,
        };
        matches!(inner, Expression::BinaryExpression(b) if b.operator.is_logical())
    }

    /// Build the condition doc for `if` / `while` / do-while, honoring the
    /// negation-inline rule.
    ///
    /// Mirrors Prettier's `printIfOrWhileConditionOrWithStatementObject`: when
    /// `condition_should_inline_negation` holds (and the parens carry no comments) the
    /// test doc is emitted bare so `!(…)` hugs `(`; otherwise the standard condition
    /// group wraps it. Prettier re-exports that one function under three names —
    /// `printIfStatementCondition`, `printWhileStatementCondition` and
    /// `printDoWhileStatementCondition` — so all three constructs share this entry point
    /// too, parting only on `open_paren_line` ([`OpenParenLineComments`]). `switch`
    /// builds its condition group directly and is deliberately excluded.
    ///
    /// ⚠️ **The group is not optional for the do-while, it is what makes its condition
    /// breakable.** The statement's own doc is a bare `concat` (prettier's
    /// `printDoWhileStatement` is too), so a condition emitted through
    /// [`Self::build_expression_doc`] had no enclosing group to open its parens and the
    /// operands wrapped at the *statement's* indent, reading as though they were
    /// statements. The fix is not to keep the self-grouping expression doc but to keep
    /// the condition group: [`Self::build_condition_group`] IS the group the ungrouped
    /// binary chain needs, so the chain breaks with the parens rather than stranded
    /// beside them.
    fn build_statement_condition_doc(
        &self,
        test: &Expression<'_>,
        open_paren: Option<u32>,
        close_paren: Option<u32>,
        open_paren_line: OpenParenLineComments,
    ) -> DocId {
        if self.condition_should_inline_negation(test) {
            let no_comments = match (open_paren, close_paren) {
                (Some(open), Some(close)) => !self.has_comments_to_emit_between(open + 1, close),
                _ => true,
            };
            if no_comments {
                return self.build_condition_doc(test);
            }
        }
        self.build_condition_group_for_parens(test, open_paren, close_paren, open_paren_line)
    }

    /// The condition group for a paren-headed head: the comment-aware builder where both
    /// paren positions were found, the plain [`Self::build_condition_group`] where the scan
    /// came up empty.
    ///
    /// Split out of [`Self::build_statement_condition_doc`] because `switch` needs exactly
    /// this and deliberately **not** the layer above it — prettier reaches
    /// `shouldInlineCondition` only from `printIfOrWhileConditionOrWithStatementObject`, so
    /// `switch (!(a || b))` does not hug its `(` the way `if` / `while` / do-while do.
    /// Naming the shared half is what keeps that a one-line difference rather than a second
    /// spelling of the dispatch for switch to drift from.
    fn build_condition_group_for_parens(
        &self,
        test: &Expression<'_>,
        open_paren: Option<u32>,
        close_paren: Option<u32>,
        open_paren_line: OpenParenLineComments,
    ) -> DocId {
        match (open_paren, close_paren) {
            (Some(open), Some(close)) => {
                self.build_condition_group_with_comments(test, open, close, open_paren_line)
            }
            _ => self.build_condition_group(test),
        }
    }

    /// Build a condition group for if/while/for/switch statements
    ///
    /// Creates the standard Prettier condition structure:
    /// ```text
    /// group([indent([softline, condition]), softline])
    /// ```
    ///
    /// This group decides whether the condition breaks (operators go to new lines).
    /// Binary expressions use ungrouped version so this parent group controls their breaking.
    fn build_condition_group(&self, test_expr: &Expression<'_>) -> DocId {
        let d = self.d();
        let test_doc = self.build_condition_doc(test_expr);
        d.group(d.concat(&[d.indent_softline(test_doc), d.softline()]))
    }

    /// Emit do-while's `(`-line bucket and return what is left for the shared leading run,
    /// plus whether the bucket holds the `(` line open.
    ///
    /// The whole of [`OpenParenLineComments::Preserve`]: a comment the author wrote on the
    /// `(` line stays there, ahead of the opening separator, rather than joining the run
    /// below it. [`OpenParenLineComments::Normalize`] — every other construct — leaves the
    /// bucket empty and hands the whole gap to the run, so this is the one place the five
    /// constructs' shared layout is not shared, and it lives apart from that layout for
    /// exactly that reason.
    ///
    /// The gap's comments are in source order, so the `(`-line ones are a PREFIX and the
    /// bucket is a split POSITION, not a pair of filters the two emitters could drift
    /// apart on (the same statement `element_gap_split` makes for the element-comma seam).
    ///
    /// ⚠️ **Every separator here is the PRINTER's, never the author's** — the whole gap is
    /// normalized, and `paren_line_breaks` (does the run hold the `(` line open?) is the one
    /// question that decides all three:
    ///
    /// | gap | run holds the `(` line | test follows on the `(` line |
    /// | --- | --- | --- |
    /// | `(`→first comment | `" "` | nothing |
    /// | comment→comment | `" "` | `" "` |
    /// | last comment→test | the caller's `hardline` | `" "` |
    ///
    /// The comment→comment gap is prettier's `printLeadingComment` space, which normalizes
    /// whatever the author wrote to exactly one, an author-glued pair included. The
    /// `(`→run gap splits because only one of its two arms has an oracle: where the test
    /// follows on the line prettier keeps the run in the parens and writes it glued to the
    /// `(` (`while (/* c1 */ /* c2 */ x)`), so tsv matches; where the run holds the line
    /// prettier relocates it out of the parens entirely and says nothing, so tsv's own
    /// convention governs — and everywhere else it keeps a comment on an opening
    /// delimiter's line it writes `fn( // c` / `new Foo( /* paren */`, a space, whatever the
    /// author wrote (`expressions/calls/open_paren_comment_prettier_divergence`).
    ///
    /// ⚠️ That is why the space is keyed on `paren_line_breaks` and the INDEX, never on the
    /// source. Reading the author's spacing gave the two arms one answer each authoring
    /// rather than one answer each SHAPE; and a per-comment "is this one glued to what
    /// precedes it" test reads a gap the previous iteration already separated and emits a
    /// second space (`while (/* c1 */  /* c2 */ x)`). No gate can see either: both are fixed
    /// points, so F1, the ledger, the census and the injection audits are blind, and the
    /// width ratchet grades a column, not a run. Only a prettier `compare` finds them —
    /// [`open_paren_comment_run`](../../../../../../tests/fixtures/typescript/statements/do_while/open_paren_comment_run/)
    /// is that comparison for the oracle arm, and
    /// [`open_paren_comment`](../../../../../../tests/fixtures/typescript/statements/do_while/open_paren_comment_prettier_divergence/)
    /// pins the other.
    ///
    /// The returned flag is that same question, handed up: a run holding the `(` line means
    /// the condition can never pull up beside it, so it hardens the caller's opening
    /// separator to a `hardline`.
    fn push_paren_line_comment_bucket<'c>(
        &self,
        parts: &mut DocBuf,
        leading_comments: &'c [&'c Comment],
        open_paren_pos: u32,
        test_start: u32,
        open_paren_line: OpenParenLineComments,
    ) -> (&'c [&'c Comment], bool) {
        let d = self.d();
        let bucket_len = match open_paren_line {
            OpenParenLineComments::Normalize => 0,
            OpenParenLineComments::Preserve => leading_comments
                .partition_point(|c| self.is_same_line(open_paren_pos, c.span.start)),
        };
        let (bucket, run_comments) = leading_comments.split_at(bucket_len);

        // Does the run HOLD the `(` line open? Every bucket comment starts on that line,
        // so only the last can end off it — one question, asked once, deciding both
        // separators below and the caller's opening one.
        let paren_line_breaks = bucket
            .last()
            .is_some_and(|last| !self.is_same_line(last.span.end, test_start));

        for (index, comment) in bucket.iter().enumerate() {
            if index > 0 || paren_line_breaks {
                parts.push(d.text(" "));
            }
            parts.push(self.build_comment_doc(comment));
        }
        if !bucket.is_empty() && !paren_line_breaks {
            parts.push(d.text(" "));
        }

        (run_comments, paren_line_breaks)
    }

    /// Whether the header parens hold a comment this layout must emit — one leading the
    /// expression inside the `(`, or trailing it before the `)`.
    ///
    /// The question [`Self::build_condition_group_with_comments`] answers to fall back to
    /// the plain [`Self::build_condition_group`], named once because one of its callers
    /// must ask it *itself*: `catch` falls back to a doc of its own (the bare parameter)
    /// rather than to the plain group, so it branches before calling. Its branch and the
    /// builder's early return are one decision, and two spellings of one decision is how
    /// they come to disagree — the builder's fallback is harmless where it is reached in
    /// agreement and a silent change of shape where it is not.
    ///
    /// The do-while used to be the second such caller, falling back to the plain
    /// expression doc; that fallback was the reason its condition parens never opened,
    /// and it now routes through [`Self::build_statement_condition_doc`] like `if` and
    /// `while`, asking nothing itself.
    ///
    /// Deliberately **not** the whole `(`…`)` range: a comment *inside* the expression is
    /// that expression's own business and no reason for this layout. The negation-inline
    /// gate in [`Self::build_statement_condition_doc`] does ask the wide range, and means
    /// to — a comment anywhere in the parens disables the `!(…)` hug.
    fn header_parens_hold_comments(
        &self,
        open_paren_pos: u32,
        close_paren_pos: u32,
        inner: &Expression<'_>,
    ) -> bool {
        self.has_comments_to_emit_between(open_paren_pos + 1, inner.span().start)
            || self.has_comments_to_emit_between(inner.span().end, close_paren_pos)
    }

    /// The comment-aware condition group
    /// (`group([indent([opening separator, leading run, test, trailing run]), closing separator])`),
    /// for the five constructs whose header parens hold one expression — `if` / `while` /
    /// do-while / `switch` / `catch`. A `for` header holds parens too and does **not**
    /// come here: its clauses are a list, and their leading gap is
    /// [`Self::push_for_clause_leading_section`] (the ⚠️ below is the same bug read one
    /// construct over).
    ///
    /// ```js
    /// if (
    ///     // before condition
    ///     x // inline with condition
    ///     // trailing after condition
    /// ) {
    /// ```
    ///
    /// The `(`→test gap is prettier's **leading** run of the test
    /// (`handleIfStatementComments` attaches those comments to the condition), and it is
    /// emitted by the shared leading emitter ([`Printer::push_leading_comment_run`]) —
    /// deliberately NOT by [`Self::build_comments_between_parts`] with
    /// [`GapCommentRun::Leading`], though the two name the same run and share the same
    /// primitives ([`Printer::comment_hugs_next`],
    /// [`Printer::push_blank_preserving_hardline`]). That builder is the *between-parts*
    /// shape: separators go BEFORE each comment, the caller owns whatever follows the
    /// last one, and every separator is hard, since its gaps live in already-broken
    /// statement contexts. This gap needs the leading run's own three-way
    /// AFTER-separator — the soft `line` that lets `if (/* c */⏎x)` collapse to
    /// `if (/* c */ x)` — and the run's forced-break report for the closing separator
    /// below. Routing it through the between-parts builder would re-spell that three-way
    /// rule caller-side, the duplication `push_leading_comment_run`'s doc exists to
    /// prevent; what legitimately differs per site is the loop around the run, never the
    /// rule (the sanctioned split that doc names).
    ///
    /// `open_paren_line` is the one axis the constructs part on
    /// ([`OpenParenLineComments`], both values pinned).
    fn build_condition_group_with_comments(
        &self,
        test_expr: &Expression<'_>,
        open_paren_pos: u32,
        close_paren_pos: u32,
        open_paren_line: OpenParenLineComments,
    ) -> DocId {
        let d = self.d();
        let test_start = test_expr.span().start;
        let test_end = test_expr.span().end;

        if !self.header_parens_hold_comments(open_paren_pos, close_paren_pos, test_expr) {
            // No comments - use the standard condition group
            return self.build_condition_group(test_expr);
        }

        // Build with comments.
        //
        // Rule: an own-line directive in the `(`→condition gap freezes the condition WHOLE
        // ([`Printer::value_head_frozen_span`]) — the condition head is a delimiter-owned
        // value head like a `for` clause or a `return` operand. The slice is the
        // condition's node span, so the `)` this layout supplies stays parent-owned; the
        // `StatementTest` clarity parens (`if ((a = b))`) stay outside it too, since they
        // are the printer's rather than the author's.
        let test_doc = match self.value_head_frozen_span(open_paren_pos + 1, test_expr.span()) {
            Some(frozen) => {
                self.build_frozen_value_doc(test_expr, frozen, super::ParenContext::StatementTest)
            }
            None => self.build_condition_doc(test_expr),
        };
        let mut inner_parts = DocBuf::new();

        let leading_comments: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, open_paren_pos + 1, test_start).collect();

        let (run_comments, paren_line_breaks) = self.push_paren_line_comment_bucket(
            &mut inner_parts,
            &leading_comments,
            open_paren_pos,
            test_start,
            open_paren_line,
        );

        // The shared leading run — prettier's `printLeadingComment`, which picks the
        // separator after each comment from the source right after *that* comment's
        // `*/`: a space when the author glued the next one to it, a soft `line` when
        // a newline follows but none precedes, a blank-preserving `hardline` for an
        // own-line block or any line comment.
        //
        // ⚠️ **The opening separator is a `softline` whatever the run holds, and the
        // shell's break is the run's REPORT, not a re-derivation.** This gap used to
        // push a `hardline` per comment off `is_same_line(open_paren, c.start)` and
        // open the parens off a range-shaped "any comment off the `(` line" — the same
        // reading `push_for_clause_leading_section`'s ⚠️ names, one construct over: in
        // `(⏎/* c1 */ /* c2 */⏎x)` only `c1` has a newline before it and only `c2`
        // has one after, so neither owns a line, yet both got a break and the parens
        // were forced open on a condition prettier keeps flat. Everything that must
        // break still does — through the run's own `hardline` and
        // `DocArena::will_break`, which breaks this `softline` with it. The bucket's
        // `hardline` is the one exception, and it is the bucket's per-comment fact,
        // not a range reading.
        inner_parts.push(if paren_line_breaks {
            d.hardline()
        } else {
            d.softline()
        });
        let leading_run_breaks = paren_line_breaks
            | self.push_leading_comment_run(
                &mut inner_parts,
                run_comments.iter().copied(),
                test_start,
                LeadingGlue::Adjacent,
                d.empty(),
            );

        // The condition itself
        inner_parts.push(test_doc);

        // A comment sharing the condition's line trails it; everything else takes its
        // own line before the `)` — including one sharing the `)`'s own line, which is
        // the bucket this gap used to drop: `if (a\n/* c */) {` printed no comment at
        // all. There is no body `{` here to glue a block to, so the two-bucket form is
        // the whole rule.
        let (trailing_inline, trailing_own_line) =
            self.partition_comments_trailing_vs_own_line(test_end, close_paren_pos);

        // Trailing comments — the shared run builder, which owns the blank and glue
        // rules. This is prettier's TRAILING run of the test (`addTrailingComment`, the
        // `nextCharacter === ")"` arm of `handleIfStatementComments`), so an authored
        // blank survives both above the first own-line comment (there is no body `{`
        // here for one to sit below, only the `)`) and between subsequent ones, and a
        // pair the author glued onto one line keeps it. Prettier keeps all three, and
        // tsv's own call-arg and array builders already did; this gap re-derived the run
        // by hand and dropped every blank.
        self.build_comments_between_parts(
            &mut inner_parts,
            &trailing_inline,
            &trailing_own_line,
            GapCommentRun::Trailing {
                blank_seed: test_end,
            },
        );

        // Structure: group([indent([softline/hardline, comments, condition, comments]), softline/hardline])
        // The closing softline/hardline is OUTSIDE the indent so `)` aligns with `(`
        // Force break when trailing inline line comments exist — flattening would cause
        // the // comment to swallow the closing `) {` producing unparseable output
        //
        // ⚠️ The leading half of that question is the RUN's report (`leading_run_breaks`),
        // never "is any leading comment off the `(`'s line": a run the author glued onto
        // one line leaves none of its comments owning one, and reading the range instead
        // forced these parens open on a condition prettier keeps flat.
        let has_trailing_line_comment = trailing_inline.iter().any(|c| !c.is_block);
        let closing =
            if leading_run_breaks || !trailing_own_line.is_empty() || has_trailing_line_comment {
                d.hardline()
            } else {
                d.softline()
            };

        d.group(d.concat(&[d.indent(d.concat(&inner_parts)), closing]))
    }

    /// Find the position of the opening paren for a keyword statement
    /// Returns the position of '(' after the keyword.
    ///
    /// Skips `(` characters inside comments and strings (`if /* (note) */ (cond)`),
    /// so a parenthesis in a leading comment can't be mistaken for the condition's
    /// open paren.
    fn find_open_paren_after(&self, start: u32) -> Option<u32> {
        find_char_skipping_comments(
            self.source.as_bytes(),
            start as usize,
            self.source.len(),
            b'(',
        )
        .map(|p| p as u32)
    }

    /// Build a doc for a condition expression (if/while/for test)
    ///
    /// For binary expressions, uses ungrouped version so parent group controls breaking.
    /// Logical operators (`&&`, `||`, `??`) break with the parent condition group.
    /// Non-logical operators (`<`, `===`, etc.) keep a sub-group for independent evaluation
    /// (e.g., `for (i = 0; i < len; i++)` — the `i < len` stays flat).
    /// Assignment expressions get double-parens for clarity: `while ((x = y))`
    fn build_condition_doc(&self, expr: &Expression<'_>) -> DocId {
        let inner = match expr {
            Expression::BinaryExpression(binary) => {
                self.build_binary_chain_doc_ungrouped_condition(binary)
            }
            _ => self.build_expression_doc(expr),
        };
        self.wrap_statement_test_parens(expr, inner)
    }

    /// The clarity parens a **statement test** puts around an assignment (`while ((x = y))`),
    /// applied to a doc the caller has already built.
    ///
    /// One question, one spelling. The two sites that ask it build their inner doc
    /// differently and can't share more than this: [`Self::build_condition_doc`] (if / while /
    /// do-while / for) takes the ungrouped binary form so the enclosing condition group drives
    /// the break, and a `case` test is emitted behind a comment gap of its own. The predicate is
    /// [`ParenContext::StatementTest`](super::ParenContext::StatementTest) in both.
    ///
    /// The do-while was a third site, passing the plain self-grouping expression doc on the
    /// reading that it has no enclosing group for the ungrouped form to break with. It has
    /// one — [`Self::build_condition_group`], which it now takes like every other paren-headed
    /// construct.
    pub(in crate::printer::statements::control_flow) fn wrap_statement_test_parens(
        &self,
        expr: &Expression<'_>,
        doc: DocId,
    ) -> DocId {
        if self.needs_parens(expr, super::ParenContext::StatementTest) {
            self.d().parens(doc)
        } else {
            doc
        }
    }
}
