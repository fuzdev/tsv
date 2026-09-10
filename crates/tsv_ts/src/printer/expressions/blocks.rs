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
    /// The CONTENT-END arm's answer for the statement the anchor belongs to
    /// ([`Printer::statement_content_tail_blank`]), carried because it is a fact about that
    /// statement's own tail rather than about the gap that follows it — no upper bound the
    /// asking gap picks can reach inside a span the walk has already left behind.
    content_tail_blank: bool,
}

impl StatementBlankScan {
    pub(in crate::printer) fn new(start: u32) -> Self {
        Self {
            anchor: start,
            bound: u32::MAX,
            content_tail_blank: false,
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

    /// Does an author blank line separate the last printed statement from what comes next —
    /// prettier's `isNextLineEmpty` (`src/language-js/utilities/is-next-line-empty.js`), which
    /// is a **disjunction over two ends of one statement**: its content end OR its full end.
    ///
    /// The full-end arm is the gap scan every statement list has always run. The content-end
    /// arm is what sees the blank in `a()⏎⏎;`, where the terminator the printer re-glues sits
    /// two lines below the content it terminates: measured from the full end that blank is
    /// *behind* the cursor, and dropping it is the whole `js/no-semi` family. Asking one
    /// predicate rather than two at each of the two walks is what keeps them from drifting —
    /// the reason this type exists.
    pub(in crate::printer) fn blank_before(&self, printer: &Printer<'_>, check_end: u32) -> bool {
        self.content_tail_blank
            || printer.has_blank_line_between(self.anchor, self.bound(check_end))
    }

    /// Content reached the output (a statement, or a dropped `;`'s orphan comment run), so
    /// the two cursors rejoin and nothing is in the way any more.
    ///
    /// `content_tail_blank` is the new anchor's own [`Self::blank_before`] arm — `false` for
    /// an orphan comment run, which is not a statement and so has no terminator to be
    /// separated from.
    pub(in crate::printer) fn printed(&mut self, end: u32, content_tail_blank: bool) {
        self.anchor = end;
        self.bound = u32::MAX;
        self.content_tail_blank = content_tail_blank;
    }

    /// Advance over a statement the walk PRINTED — the loop-tail both statement walks run.
    ///
    /// ⚠️ **The blank cursor takes the statement's FULL end, never the comment cursor**, which
    /// is the whole content of the `.max` here. `prev_end` may have been lowered INTO the
    /// statement's terminator gap (prettier's `__contentEnd`,
    /// [`Printer::statement_gap_scan_start`]) so the next statement's leading run can find the
    /// comment written there, and a blank scan opened at that position would cross the
    /// comment's own bytes — hazard 5, a FABRICATED blank (`docs/comments.md` §The five
    /// hazards). Prettier splits the two ends the same way: `isNextLineEmpty` measures from the
    /// full end while attachment measures from `__contentEnd`. Every handed-over comment lies
    /// below the span end, so opening there crosses none of them.
    ///
    /// The [`Printer::statement_content_tail_blank`] verdict travels with it because it is the
    /// new anchor's own [`Self::blank_before`] arm — a fact about the statement the anchor
    /// belongs to, unreachable once the walk has moved past it. Pairing them here is what
    /// keeps a caller from advancing the cursor and forgetting the arm; before this the rule
    /// was spelled out at the block walk and left to a "see the block walk for why"
    /// cross-reference at the consequent's, which is a shared rule with no shared spelling.
    pub(in crate::printer) fn printed_statement(
        &mut self,
        printer: &Printer<'_>,
        prev_end: u32,
        stmt: &internal::Statement<'_>,
        frozen: bool,
    ) {
        self.printed(
            prev_end.max(stmt.span().end),
            printer.statement_content_tail_blank(stmt, frozen),
        );
    }

    /// Whether the separator before a statement's LEADING RUN must hoist an author blank
    /// above the whole run — the one statement of a question BOTH statement walks ask (the
    /// program/block/namespace list and the switch consequent's own), so the two cannot
    /// drift. They had: the block walk bounded its scan on the **in-source** axis (past a
    /// comment this gap does not itself emit — an owned annotation) while the consequent
    /// bounded it on the first comment it *emits*.
    ///
    /// Three readings, keyed on where the run sits relative to the previous statement's own
    /// terminator, because an ejected gap (prettier's `__contentEnd`,
    /// [`Printer::statement_comment_claim_end`]) puts a leading run INSIDE the span above it:
    ///
    /// - The run **spans** the terminator (`x // a⏎;⏎⏎// b`) — the blank sits *between* the
    ///   run's own members, where its own separator already places it
    ///   ([`Printer::push_leading_comments_before`]). Hoisting it as well prints the author's
    ///   single blank TWICE, so this arm answers `false`.
    /// - The run is **wholly inside** the gap — the run is hoisted past the `;`, so this arm
    ///   is a DISJUNCTION over the two cursors, because neither can see both sides. Above the
    ///   run (`a() // c1⏎⏎// c2⏎;⏎b()`) only the COMMENT cursor reaches, the blank cursor
    ///   being anchored at the statement's FULL end, below the whole run — which is how that
    ///   blank came to be DROPPED outright, and nothing else can place it, so that disjunct is
    ///   unconditional. Below the terminator (`a()⏎/* c */;⏎⏎b()`) only the BLANK cursor
    ///   does, and there the run's own last separator is a second candidate emitter — so that
    ///   disjunct is taken only when the run's last comment GLUES to the item
    ///   ([`Printer::glued_run_blank_anchor`]) and so keeps no line below itself: the "rides
    ///   above instead, the break it sat on is gone" reading
    ///   `syntax/comments/comment_before_detached_semicolon` pins. A run that ends its own
    ///   line keeps the blank below, where `push_leading_comment_run` puts it back — reading
    ///   it here as well printed the author's single blank TWICE. One `literalline` either
    ///   way, so an author blank on both sides still prints once.
    /// - Otherwise the ordinary [`Self::blank_before`], over the blank cursor.
    ///
    /// `blank_scan_end` keeps every count off comment bytes (`docs/comments.md` hazard 5).
    pub(in crate::printer) fn blank_before_leading_run(
        &self,
        printer: &Printer<'_>,
        prev_end: u32,
        prev_stmt_end: Option<u32>,
        leading_comments: &[&internal::Comment],
        stmt_start: u32,
    ) -> bool {
        let run_start = leading_comments
            .first()
            .map_or(stmt_start, |c| c.span.start);
        let (spans_terminator, inside_gap) = prev_stmt_end.map_or((false, false), |end| {
            let first_inside = leading_comments.first().is_some_and(|c| c.span.start < end);
            let last_inside = leading_comments.last().is_some_and(|c| c.span.start < end);
            (first_inside && !last_inside, last_inside)
        });
        let below_terminator =
            || self.blank_before(printer, printer.blank_scan_end(self.anchor(), stmt_start));
        if inside_gap {
            // The blank ABOVE the run: no other seam can reach it, so this one always does.
            printer.has_blank_line_between(prev_end, printer.blank_scan_end(prev_end, run_start))
                // The blank BELOW the terminator is placed ONCE, and which seam places it is
                // decided by the run's own last separator. A run whose last comment GLUES to
                // the next item (`a()⏎/* c */;⏎⏎b()` → `a();⏎⏎/* c */ b();`) leaves no line
                // below itself to carry the blank, so it rides above instead — the "the break
                // it sat on is gone" reading `syntax/comments/comment_before_detached_semicolon`
                // pins. A run that ends its own line keeps it below, where
                // [`Printer::push_leading_comment_run`]'s `push_blank_preserving_hardline`
                // finds it and puts it back — the authored relative order the
                // `clause_terminator_comment_then_blank` divergence records.
                //
                // ⚠️ Reading it here **as well** printed the author's single blank TWICE
                // (`a()⏎// c⏎;⏎⏎b()` gained a line neither formatter emits). A fabricated
                // blank is its own fixed point, so no gate reaches it: `blanks:audit` grades
                // an INJECTED blank against the pristine output, and both sides fabricate.
                || (printer.glued_run_blank_anchor(leading_comments).is_some()
                    && below_terminator())
        } else {
            !spans_terminator && below_terminator()
        }
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

/// The walk state a statement list carries INTO a dropped `EmptyStatement`'s slot.
///
/// Read-only and `Copy`: both walks keep these as plain locals shared with their printing
/// arm, and the slot only asks them questions.
#[derive(Clone, Copy)]
pub(in crate::printer) struct OrphanSemiCursor {
    /// The comment cursor — everything the list has already emitted sits behind it.
    pub prev_end: u32,
    /// End of the last thing the list PRINTED; `None` when it has printed nothing yet.
    pub prev_stmt_end: Option<u32>,
    /// [`Printer::comment_already_trailed`]'s anchor: where the previous statement's own
    /// emission actually ENDED, which parts from its span end once it hands its terminator
    /// gap to the list.
    pub prev_claim_anchor: Option<u32>,
    /// The previous statement deferred a line comment past its own `;`, so it trailed
    /// NOTHING on that line ([`Printer::terminator_defers_line_comment`]).
    pub prev_deferred_line_comment: bool,
    /// The list's zero-comment fast gate — see `body_has_comments` in
    /// [`Printer::build_statement_list_docs_into`].
    pub has_comments: bool,
}

/// What a dropped `EmptyStatement`'s slot DECIDES — the shared half of the orphan arm both
/// statement walks run ([`Printer::orphan_semi_slot`] builds it).
///
/// The arm has two halves and only one of them is shareable. The decision — how far the
/// slot reaches, which comments it claims, whether an author blank sits above them, and how
/// the walk's cursors move over it — is this type. The EMISSION genuinely differs (the block
/// list pushes into its caller-owned `body_parts`; the switch consequent wraps the run in
/// `d.indent`) and stays at each callsite.
///
/// A type for the same reason [`StatementBlankScan`] is one: the two arms were hand-rolled
/// copies, and the copy is what let them drift THREE ways in a single session — the monotone
/// cursor clamp only the block walk carried (a real DOUBLE-PRINT, pinned by
/// `switch/consequent_empty_statement_run_multiline_trailer`), the blank question's axis
/// (in-source vs to-emit), and the claim-anchor update. Each is stated once here.
pub(in crate::printer) struct OrphanSemiSlot<'c> {
    /// Upper bound of this `;`'s slot: the run emitter's own bound, the blank scan's far
    /// end, and the position the comment cursor advances to.
    pub search_end: u32,
    /// The comments this slot must print; empty when it claims none. Mutable because the
    /// block walk's first slot must still drop what the opening `{`'s line already carries.
    pub comments: CommentVec<'c>,
    /// An author blank line separates the run from the last thing the list printed. Folds in
    /// "something HAS printed" — with nothing above it there is no gap to be blank.
    pub blank_above: bool,
    /// Whether this slot claims its comments at all ([`Printer::orphan_semi_slot`]'s
    /// `claims`), kept so [`Self::advance`] moves the cursor only over what was emitted.
    claims: bool,
}

impl OrphanSemiSlot<'_> {
    /// Move the walk's cursors over the slot — the third of the three drifts, and the one
    /// with no local spelling left: a caller states its emission, never this bookkeeping.
    ///
    /// ⚠️ **An orphan slot moves NO claim anchor**, which is why none is passed. The anchor
    /// answers "did the previous item TRAIL this comment, ahead of the cursor"
    /// ([`Printer::comment_already_trailed`]), and only a trailing run can leave one there —
    /// its cursor is clamped to the claim split, while this run's IS its upper bound, so
    /// everything the slot emitted is already behind `prev_end`. [`Self::search_end`] is also
    /// capped by the NEXT dropped `;`'s start, a third position that predicate's ⚠️ does not
    /// admit and that can sit on a later comment's line: anchored there, the next slot calls
    /// that comment already-trailed and DROPS it (`a()// c1⏎;;;; // c2`, and again with a
    /// comment in the terminator gap — both found by the gap-injection audit, and both
    /// needing two `;`s, so a single one never showed either). Keeping the last printed
    /// statement's anchor preserves the one reading that IS meaningful: a comment its
    /// trailing run claimed and its clamped cursor stopped short of.
    ///
    /// The COMMENT cursor moves only over what the slot claimed: a slot that claims nothing
    /// leaves every comment in it ahead of the cursor, for the seam past the list to place.
    ///
    /// The orphan run is content and moves the BLANK anchor exactly as a statement does; a
    /// `;` that printed nothing becomes that scan's bound instead
    /// ([`StatementBlankScan::skipped_semi`]).
    pub(in crate::printer) fn advance(
        &self,
        printer: &Printer<'_>,
        prev_end: &mut u32,
        blanks: &mut StatementBlankScan,
        semi_start: u32,
    ) {
        if self.claims {
            *prev_end = self.search_end;
        }
        if self.comments.is_empty() {
            blanks.skipped_semi(printer, semi_start);
        } else {
            blanks.printed(self.search_end, false);
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
    /// The **content-end** arm of [`StatementBlankScan::blank_before`]: did the author leave
    /// a blank line inside the statement's own tail — between where its content ends
    /// ([`Self::statement_content_end`], prettier's `locEnd` table) and the `;` the printer
    /// re-emits glued?
    ///
    /// `a()⏎⏎;` is the whole shape. The terminator moves back up to the content, so the blank
    /// the author wrote below `a()` has nowhere to go but the gap after the statement — which is
    /// prettier's answer, and the last hunk on every `js/no-semi` file. Every kind the table
    /// lists carries it, `debugger` and `break`/`continue` included (their content ends at the
    /// keyword, so the `;` may be a whole blank line below it), and every header kind reaches it
    /// through the body the table recurses into. A kind the table does not list has no split at
    /// all, which is why `type A = B⏎⏎;` and the empty-statement bodies (`for (;;)⏎⏎;`,
    /// `while (a)⏎⏎;`, `l:⏎⏎;` — there the `;` IS the body, not a terminator) keep dropping the
    /// blank on both formatters.
    ///
    /// **The comment question answers itself, and that is why there is no comment code here.**
    /// [`Self::statement_content_end`] counts a comment as CONTENT (⚠️ where prettier measures
    /// its comment-STRIPPED text), so the tail this measures is whitespace and nothing else —
    /// the `debug_assert!` states it. Two consequences fall out rather than being coded for: a
    /// blank the author wrote ABOVE a comment in the tail is outside the measured range, so the
    /// comment's own emitter keeps owning it and no second blank is fabricated below it
    /// (`a()⏎⏎// c⏎;`, where prettier's separator emits a single break); and a scan can never
    /// cross a comment's own interior newlines (docs/comments.md §The five hazards, hazard 5),
    /// which is what an explicit `blank_scan_end` ceiling would otherwise be here to buy.
    /// Guarding it a second time reads as a live rule and is dead code.
    ///
    /// The scan is the line-break TABLE, like the full-end arm beside it and unlike
    /// [`Self::is_next_line_empty`]'s byte walk: prettier's own `skipNewline` counts
    /// `<LS>` / `<PS>`, so the table is the closer reading, and it is the form
    /// [`Self::set_canonical`] erases — a canonical reprint must not resurrect an author
    /// blank from raw source.
    pub(in crate::printer) fn statement_content_tail_blank(
        &self,
        stmt: &internal::Statement<'_>,
        frozen: bool,
    ) -> bool {
        let full_end = stmt.span().end;
        let content_end = self.statement_content_end(stmt);
        if content_end == full_end {
            return false;
        }
        // The tail's LOWER end — below the last thing in it, comments counted as content, so
        // `a()⏎⏎;` and `a() // c⏎⏎;` both read here. This is prettier's scan walking off the
        // content's line over a trailing comment (`skipTrailingComment`) before it looks.
        if self.has_blank_line_between(content_end, full_end) {
            return true;
        }
        // The tail's UPPER end — above the FIRST comment in it (`a()⏎⏎/* c */;`), which the
        // reading above cannot see because that comment is what it measures from. Only a kind
        // whose gap the LIST claims has an upper end to ask about: where the statement prints
        // the comment itself, the blank above it is its own emitter's
        // (`push_gap_comments`'s `preserve_blank`) and reading it here would double it.
        //
        // ⚠️ Both ends, never the whole tail: a blank the author left BETWEEN two comments
        // there belongs to the leading run that prints them, and a scan spanning the tail
        // would claim it — and would cross a comment's own interior newlines besides
        // (hazard 5, `docs/comments.md`). `blank_scan_end` is that first-comment ceiling; with
        // no comment in the tail it IS `full_end` and the two ends are one question.
        //
        // ⚠️ A FROZEN statement has no upper end to ask about either, and for the opposite
        // reason: its slice is copied verbatim from `stmt.span().start`, so every blank
        // inside the tail is ALREADY printed — by the slice itself. Reading one here emits
        // it a SECOND time (`// prettier-ignore⏎fn(  a  )⏎⏎// c⏎;` gained a blank the author
        // never wrote below the `;`), which is a FABRICATION, and a fabricated blank is its
        // own fixed point — no gate reaches it. The freeze is the caller's verdict, so the
        // caller states it, exactly as at [`Printer::statement_emitted_end`].
        if frozen || !self.statement_hands_off_terminator_gap(stmt) {
            return false;
        }
        let claim_end = self.statement_comment_claim_end(stmt);
        self.has_blank_line_between(claim_end, self.blank_scan_end(claim_end, full_end))
    }

    /// The blank question for an ORPHAN run — the comments a dropped `EmptyStatement`'s slot
    /// claims, which no printed statement separates from the previous emission.
    ///
    /// Measured from the COMMENT cursor (nothing has printed since) up to the first comment
    /// PHYSICALLY in the slot, so the count never crosses comment bytes (`docs/comments.md`
    /// hazard 5) and an owned comment sitting ahead of the run's own first member still stops
    /// it. Both statement walks ask it here; the consequent's copy had measured to the first
    /// comment it EMITS instead — the same in-source-vs-to-emit drift the leading run's own
    /// question carried ([`StatementBlankScan::blank_before_leading_run`]).
    pub(in crate::printer) fn blank_before_orphan_run(
        &self,
        prev_end: u32,
        search_end: u32,
    ) -> bool {
        self.has_blank_line_between(prev_end, self.blank_scan_end(prev_end, search_end))
    }

    /// Where a blank BELOW a leading run would have to sit — `Some(end)` when the run's last
    /// comment GLUES to what follows it, `None` otherwise.
    ///
    /// The glue erases the gap the author wrote that blank in, so the run keeps no line below
    /// itself to carry one and the seam ABOVE the run becomes its only emitter. Both statement
    /// seams that can hoist a blank ask exactly this — the statement list's leading run
    /// ([`StatementBlankScan::blank_before_leading_run`]) and the switch's between-case run —
    /// and asking it in one place is the point: two hand-rolled copies of one comment question
    /// is the drift this whole area has paid for repeatedly.
    ///
    /// Each caller then MEASURES with its own cursor, and those genuinely differ: the list
    /// walk's blank cursor carries the content-end arm ([`StatementBlankScan::blank_before`])
    /// that a raw source scan cannot express, while the case seam scans plain source to the
    /// label. One question, two scans — which is why this returns the anchor rather than a
    /// verdict.
    pub(in crate::printer) fn glued_run_blank_anchor(
        &self,
        comments: &[&internal::Comment],
    ) -> Option<u32> {
        comments
            .last()
            .filter(|c| self.comment_hugs_next(c))
            .map(|c| c.span.end)
    }

    /// Decide the slot the dropped `EmptyStatement` at `body[index]` opens — the shared half
    /// of both statement walks' orphan arm. See [`OrphanSemiSlot`] for why it is one type.
    ///
    /// `list_end` is the far edge of the walk's own window (a body's `}`, a case's boundary)
    /// and `tail_target` the construct past the list's end that a gap comment could still
    /// lead ([`Self::statement_claim_end`]) — `None` where nothing prints there.
    ///
    /// `claims` is the one asymmetry between the two walks, and it is principled. A TRAILING
    /// run of dropped `;`s — one with no printed statement after it — claims nothing in a
    /// switch consequent: an orphan run is a leading run with its item dropped, and there is
    /// no item left to lead, so the comments are the CASE seam's and the between-case (or
    /// after-last-case) run places them at the case's level, which is where a reformat reads
    /// them back from. Claiming them at consequent level printed a form that was not its own
    /// fixed point (`case 1: fn1();⏎// c⏎;` dedented on the second pass). A block body has no
    /// such seam past its end, so its slots always claim.
    pub(in crate::printer) fn orphan_semi_slot(
        &self,
        body: &[internal::Statement<'_>],
        index: usize,
        list_end: u32,
        tail_target: Option<u32>,
        cursor: OrphanSemiCursor,
        claims: bool,
    ) -> OrphanSemiSlot<'_> {
        let stmt_end = body[index].span().end;
        // Zero-comment fast gate, the list's own: with no comment anywhere in its window
        // every query below is provably empty, so the slot reaches exactly its own `;` and
        // claims nothing. The block walk had this; the consequent's copy ran the searches
        // anyway and then discarded their answer.
        if !cursor.has_comments {
            return OrphanSemiSlot {
                search_end: stmt_end,
                comments: CommentVec::new(),
                blank_above: false,
                claims,
            };
        }
        let next_start = body.get(index + 1).map_or(list_end, |s| s.span().start);
        // A comment hugging the next printed statement's start leads it
        // (`a(); ; /* c */ b();`) — or, past the last one, the construct at `tail_target` (a
        // consequent's next `case` label). The orphan run must stop at the claim split so
        // that leading run still finds it.
        let claim_end = self.statement_claim_end(body, index, tail_target, false);
        // A dropped `;` owns no terminator gap, so its own run reads one line.
        let search_end = self
            .find_end_with_trailing_comments(stmt_end, u32::MAX)
            .min(next_start)
            .min(claim_end)
            // ⚠️ **The cursor is MONOTONE.** Both `min`s above are bounds on this `;`'s own
            // slot and neither knows where the last PRINTED statement's trailing run ended —
            // a run that follows a multi-line block to its closing line ends past a following
            // `;` (`a();;; /* m⏎n */ // c`), so taking them raw moves the cursor BACKWARD over
            // comments already emitted and the next slot prints one of them a SECOND time.
            // Nothing else caught it: `comment_already_trailed`'s span-end anchor is a line
            // below such a comment, and its ⚠️ argues from exactly the invariant this restores
            // — a comment inside the trailing run stays BEHIND the cursor and never reaches
            // that filter.
            .max(cursor.prev_end);

        // `prev_deferred_line_comment` reaches the ORPHAN run too, the way it does a printing
        // one: the previous statement deferred a line comment past its own `;` and so trailed
        // NOTHING on that line, which leaves this run its only emitter. Answering `false` made
        // the filter call the gap's comments already-trailed and skip them — a DROP, since the
        // dropped `;` puts no leading run after them either (`a = 1 // c1⏎; /* c2 */ ;⏎b = 2;`).
        let comments = if claims {
            self.collect_leading_comments(
                cursor.prev_end,
                search_end,
                cursor.prev_claim_anchor,
                cursor.prev_deferred_line_comment,
                None,
            )
        } else {
            CommentVec::new()
        };

        OrphanSemiSlot {
            // Asked only where there is a run to hoist it above — the emitters read it under
            // their own `is_empty` guard, and a caller that filters the run down to nothing
            // never reaches it either.
            blank_above: !comments.is_empty()
                && cursor.prev_stmt_end.is_some()
                && self.blank_before_orphan_run(cursor.prev_end, search_end),
            search_end,
            comments,
            claims,
        }
    }

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
        self.build_block_body_doc(block, expand_empty, in_program_or_block)
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
            self.comments_to_emit_between(block_start, block_end),
        );
        comment_parts
    }

    /// Build a Doc for a block body.
    ///
    /// This is the unified implementation for block statement doc building.
    fn build_block_body_doc(
        &self,
        block: &internal::BlockStatement<'_>,
        expand_empty: bool,
        in_program_or_block: bool,
    ) -> DocId {
        let d = self.d();
        let block_start = block.span.start + 1; // After '{'
        let block_end = block.span.end - 1; // Before '}'

        // Comments attached to a body whose only statements are dropped
        // `EmptyStatement`s are still picked up by
        // `build_inner_comments_for_empty_block`, which scans the full brace
        // range rather than the statement list.
        if is_effectively_empty_body(block.body) {
            let inner_comments = self.build_inner_comments_for_empty_block(block);
            if !inner_comments.is_empty() {
                // Block with inner comments
                return d.concat(&[
                    d.text("{"),
                    d.indent_hardline(d.concat(&inner_comments)),
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
        // to its own line as the body's leading comment).
        // See conformance_prettier_ts_comments.md §Comment relocation (Block body `{`).
        let first_stmt_start = block.body[0].span().start;
        let (brace_line_prefix, delimiter_pull_pos) =
            self.delimiter_line_comment_prefix(block.span.start, first_stmt_start);

        // Build statements (leading comments, blank-line separators,
        // format-ignore, trailing same-line comments) via the shared walk,
        // filling a pooled buffer in place — one RAII owner, released back to the
        // free-list on scope exit.
        let mut body_parts = d.pooled_docbuf();
        let tail = self.build_statement_list_docs_into(
            &mut body_parts,
            block.body,
            Span::new(block_start, block_end),
            false,
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
            self.push_trailing_body_comments(
                &mut body_parts,
                tail.prev_end,
                block_end,
                tail.claims_trailing,
            );
        }

        self.build_delimited_doc(
            d.text("{"),
            brace_line_prefix,
            d.indent_hardline(d.concat(&body_parts)),
            d.hardline(),
            d.text("}"),
        )
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
        // The anchor [`Printer::comment_already_trailed`] filters against: where the previous
        // statement's own emission actually ENDED, which is the cursor rather than its span
        // end. The two part once a statement hands its terminator gap to this list — its
        // trailing run then stops ABOVE the `;`, on the content's line, and a comment sitting
        // on the `;`'s line was claimed by nobody. Anchored at the span end that comment reads
        // as already-trailed and is DROPPED.
        let mut prev_claim_anchor: Option<u32> = None;
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
                // A block body has no seam past its end that could re-place a trailing run's
                // comments, so every slot here claims — [`Printer::orphan_semi_slot`].
                let mut slot = self.orphan_semi_slot(
                    body,
                    i,
                    body_end,
                    None,
                    OrphanSemiCursor {
                        prev_end,
                        prev_stmt_end,
                        prev_claim_anchor,
                        prev_deferred_line_comment,
                        has_comments: body_has_comments,
                    },
                    true,
                );
                // The opening `{`'s line is the caller's to print, so a comment the author
                // glued there is emitted as a prefix on it and this slot must not claim it
                // too. Only the FIRST slot can: no later statement shares that line.
                if is_first && let Some(dpos) = delimiter_pull_pos {
                    slot.comments
                        .retain(|c| !self.comment_on_delimiter_line(dpos, c));
                }

                if !slot.comments.is_empty() {
                    if prev_stmt_end.is_some() {
                        if slot.blank_above {
                            body_parts.push(d.literalline());
                        }
                        body_parts.push(d.hardline());
                    } else if has_leading {
                        body_parts.push(d.hardline());
                    }
                    self.push_orphaned_comment_run(body_parts, &slot.comments, slot.search_end);
                    prev_stmt_end = Some(stmt_end);
                }

                slot.advance(self, &mut prev_end, &mut blanks, stmt_start);
                continue;
            }

            // Collect leading comments (skip trailing same-line from previous
            // statement). Skipped entirely on a comment-free block.
            //
            let mut leading_comments = if body_has_comments {
                self.collect_leading_comments(
                    prev_end,
                    stmt_start,
                    prev_claim_anchor,
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
                // ⚠️ A dropped `;` CAPS the scan as well as failing to move its start —
                // the two halves of one question ([`StatementBlankScan`]): what did the
                // author write between the last printed statement and the next thing in
                // the gap? The `;` is such a thing, exactly as a comment is
                // (`blank_scan_end`'s own rule). Capping the table-based count rather than
                // switching to prettier's byte-scanning `isNextLineEmpty` keeps the
                // line-break TABLE as the oracle, which is what sees U+2028 / U+2029 as
                // line terminators — the byte scan does not, and
                // `syntax/whitespace/line_terminators` is the fixture that says so.
                //
                // Which anchor the question takes, and the two ejected-gap readings that
                // are not the ordinary one, are stated once for both walks
                // ([`StatementBlankScan::blank_before_leading_run`]). Its scans are comment
                // searches, so a comment-free block skips them.
                let hoist_blank = if body_has_comments {
                    blanks.blank_before_leading_run(
                        self,
                        prev_end,
                        prev_stmt_end,
                        &leading_comments,
                        stmt_start,
                    )
                } else {
                    // With no comment anywhere in the block the helper's searches provably
                    // return `stmt_start` and its run is empty, so this is the same answer
                    // without them.
                    blanks.blank_before(self, stmt_start)
                };
                if hoist_blank {
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
            let stmt_frozen = body_has_comments && self.member_gap_frozen(prev_end, stmt_start);
            if stmt_frozen {
                body_parts.push(self.build_frozen_statement_doc(stmt));
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
            // A statement that hands its terminator gap to this list defers nothing of its
            // own — the `//` there rides the gap's trailing run like any other, so the
            // question is only the clause-body site's ([`Printer::push_statement_semicolon`]).
            // Asking it anyway leaves `prev_end` at the `;`, ABOVE the handed comment, which
            // drops it outright.
            prev_deferred_line_comment = body_has_comments
                && (stmt_frozen || !self.statement_hands_off_terminator_gap(stmt))
                && self.terminator_defers_line_comment(stmt_start, stmt_end);
            if prev_deferred_line_comment {
                // …unless this statement's doc ends with a line comment its terminator gap
                // deferred past the `;`. Nothing may share that line, so leaving `prev_end`
                // at the `;` hands the comments to the next statement's leading run;
                // advancing it (what the trailing case does below) would DROP them.
                prev_end = stmt_end;
            } else if body_has_comments {
                // The shared trailing arm of the statement-gap seam: the run bounded at
                // the next printed statement and stopped at the claim split, the cursor
                // clamped to the same split ([`Printer::push_statement_trailing_run`]) — so
                // a handed-over comment stays ahead of it for the next statement's
                // leading run to find.
                prev_end =
                    self.push_statement_trailing_run(body_parts, body, i, body_end, stmt_frozen);
            } else {
                prev_end = stmt_end;
            }
            blanks.printed_statement(self, prev_end, stmt, stmt_frozen);
            prev_stmt_end = Some(stmt_end);
            prev_claim_anchor = Some(prev_end);
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
        let collected: CommentVec<'_> = self
            .comments_to_emit_between(prev_end, stmt_start)
            .filter(|c| {
                !self.comment_already_trailed(prev_stmt_end, c, claims_trailing, leads_target)
            })
            .collect();
        #[cfg(feature = "buffer_stats")]
        crate::printer::buffer_stats::record_leading_comments(collected.len());
        collected
    }
}
