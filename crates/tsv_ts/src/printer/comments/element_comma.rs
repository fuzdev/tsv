// Trailing comments around a list element's separator comma.
//
// The single source of the `trailingComma: 'none'` comment-position contract for
// element lists: block comments before the comma stay before it, a block comment
// after the comma is preserved after it — on the last element (whose comma isn't
// emitted) and ahead of a line comment in the same trailing run (`after_comma`,
// below) — and line comments go after the comma via `line_suffix` (zero width). Prettier relocates
// every one of those blocks before the comma; see conformance_prettier_ts_comments.md
// §Comment relocation. Shared by the object-literal, object/array
// destructuring-pattern, import/export specifier and enum-member loops so the
// ordering can't drift between them. (The array literal answers the same rule
// through its own paired trailing/leading predicate — holes and the fill path don't
// fit this collector's shape. Its comma arm is [`block_is_before_comma`], shared with
// this collector's classifier, and a demoted line comment's RENDERING is
// [`Printer::push_trailing_line_comment_demotion_aware`], shared likewise; the block's
// ORDER past the comma and the demotion trigger
// ([`TrailingComments::demote_line_after_deferred`]'s question, which the array asks
// itself) are still its own, so a change to THOSE has to be mirrored there.)
//
// This side is half of a partition: what it does NOT claim leads the next element,
// and every caller resumes its own leading scan at `end_pos`. See
// docs/comments.md §The element-comma seam for the two rules that keeps honest.
//
// The comma is located with `find_comma_in_range` (comment/string-skipping,
// bounded by the element's upper boundary), so a comma inside an earlier comment
// (`a /* , */ /* x */, b`) is never mistaken for the separator and the following
// comment is not relocated across it.

use super::{CommentVec, Printer};
use crate::ast::internal::Expression;
use smallvec::SmallVec;
use tsv_lang::Comment;
use tsv_lang::Span;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// Does a block comment at `start` in the element's trailing run sit on the trailing
/// side of its separator? The element-comma partition's **comma arm**, stated once and shared by
/// [`Printer::collect_trailing_comments`] and the array literal's paired predicate
/// (`block_comment_trails_prev_element`) so the two can't drift: before the comma the
/// block trails the element, and on the LAST element the comma is never emitted
/// (`trailingComma: 'none'`), so "before" and "after" it are one position and the whole
/// run trails. A block past a non-last element's comma is NOT covered here — that is
/// the callers' own second arm (the deferred-line-comment order rule).
pub(in crate::printer) fn block_is_before_comma(
    is_last: bool,
    comma_pos: Option<u32>,
    start: u32,
) -> bool {
    is_last || comma_pos.is_none_or(|comma| start < comma)
}

/// Does this trailing run end in a DEFERRING line comment — one that will ride out on a
/// `line_suffix`, so every block ahead of it must be claimed with it or come back
/// reordered behind it?
///
/// A boolean rather than a position, and that is a fact about the run, not a shortcut: a
/// `//` can only be its LAST member ([`Printer::trailing_comment_run`] stops there), so
/// every block in the run provably precedes it. Both readers of that fact — this module's
/// classifier and the array literal's `element_gap_split` — ask it here.
pub(in crate::printer) fn run_defers_line(run: &[&Comment]) -> bool {
    run.last().is_some_and(|c| !c.is_block)
}

/// Where the gap after the real element in slot `idx` ENDS — at the next REAL element,
/// however many elisions lie in between. `None` when only holes follow, which each
/// container answers with its own trailing position (`array_gap_end`,
/// `array_pattern_gap_end`, the two one-line namers that supply that fallback).
///
/// [`Printer::element_gap_split`] divides this gap between its two emitters; this is the
/// gap itself, and only a container with holes has to compute it. **An elision opens no
/// gap of its own**: it prints nothing and its comma is re-emitted structure outside every
/// element span, so however many holes intervene the region between two real elements is
/// ONE gap that both emitters must measure the same way. A one-slot lookup that gives up
/// at a hole hands the whole rest of the container to the earlier element, which then
/// claims a comment the later one claims too — `[a, , b // c] = x` printed the `//` on
/// BOTH (`docs/comments.md` §The element-comma seam: unclaimed is a DROP, doubly-claimed a
/// DOUBLE-PRINT).
///
/// ⚠️ **The BLANK-line scan does not end here** — it stops earlier, at
/// [`Printer::hole_slot_comma`]. Two questions, two far ends, and answering either with
/// the other's is a bug pointing the opposite way.
pub(in crate::printer) fn next_real_element_start(
    elements: &[Option<Expression<'_>>],
    idx: usize,
) -> Option<u32> {
    elements[idx + 1..]
        .iter()
        .find_map(|e| e.as_ref().map(|e| e.span().start))
}

/// Trailing comments collected for a list element (property or array element)
pub(in crate::printer) struct TrailingComments<'a> {
    /// Block comments emitted in source order, before the emitted comma. A last
    /// element's after-comma block is included here too — its comma is `None`
    /// (trailingComma: 'none'), so "before the comma" and "after the comma" are the same
    /// position and the whole run trails the element together (prettier relocates an
    /// after-comma block before the comma; see conformance_prettier_ts_comments.md).
    before_comma: SmallVec<[&'a Comment; 2]>,
    /// Block comments the author wrote **after** the comma that stay with this element
    /// rather than leading the next one: the ones the run's line comment follows
    /// (`a, /* c1 */ // c2`). The line comment defers through `line_suffix`, so a block
    /// left to lead the next element would render *after* it — the pair comes out
    /// reordered, and in the pattern builders (which resume their leading scan past this
    /// run) it was skipped by both emitters and DROPPED outright. Emitted between the
    /// comma and the line suffix, which is exactly where it was written. Same rule, same
    /// reason as `Printer::push_trailing_comments_in_range`'s deferred-run block.
    after_comma: SmallVec<[&'a Comment; 2]>,
    /// The one line comment that goes after the comma (in `line_suffix`), if the run had
    /// one. At most one **by construction**: a `//` runs to end of line, so it can only be
    /// the run's last member ([`Printer::trailing_comment_run`]) — and a second deferred
    /// one would weld onto the first's output line rather than render.
    line: Option<&'a Comment>,
    /// Whether `line` renders DEMOTED — on a fresh line of its own instead of through
    /// `line_suffix` — because the element's own doc already ends in a deferred `//`.
    /// Set by [`TrailingComments::demote_line_after_deferred`].
    line_demoted: bool,
    /// Position after all trailing comments (for updating prev_end)
    pub(in crate::printer) end_pos: u32,
}

impl TrailingComments<'_> {
    /// Give this run's LINE comment a line of its own when the element's own doc
    /// already ends in a DEFERRED `//` — today only a spread whose stripped grouping
    /// parens held one ([`Printer::defers_trailing_line_comment`]).
    ///
    /// Its output line already terminates in a `//`, so nothing more may join it:
    /// deferring a second line comment onto the same line welds the two into ONE comment,
    /// the second `//` becoming text inside the first. The argument-list twin is
    /// [`super::super::calls::PartitionedComments::demote_trailing_line_after_deferred`];
    /// it moves the comments to the *next* element's leading run, which this collector
    /// cannot do — the run stays claimed here (`end_pos` already covers it) and
    /// [`Printer::push_element_comma_trailing`] gives it a line of its own instead. Both
    /// land on the same output, and both are fixed points: a comment printed onto a fresh
    /// line is own-line when it is reparsed.
    ///
    /// `element_defers_line` is the question already answered by the caller, which is the
    /// only place that knows the element's node type.
    pub(in crate::printer) fn demote_line_after_deferred(&mut self, element_defers_line: bool) {
        self.line_demoted = element_defers_line;
    }
}

impl<'a> Printer<'a> {
    /// Where a list element's trailing-comment scan starts: its PRINTED end
    /// ([`crate::ast::internal::Expression::printed_end`], which carries the argument),
    /// except under a format-ignore FREEZE, where it is the end of the frozen slice.
    ///
    /// The freeze arm is the whole reason this is a named question rather than a field
    /// read. A frozen element prints its source **verbatim**, stripped-paren shell and
    /// interior comment included, so the anchor must be the end of what that slice PRINTED;
    /// an inner anchor hands the seam a comment already on the page and prints it TWICE —
    /// and once more on every later pass, since the emitted form still carries the
    /// directive and re-freezes.
    ///
    /// ⚠️ **The slice, not a `bool` plus the node's span end.** The two agree at every
    /// freeze site today, but "did something freeze?" and "how far did we print?" are one
    /// question here, and asking the first is how the second gets answered by the wrong
    /// arm: a parameter has TWO freeze positions — the list gap
    /// ([`Self::param_frozen_span`]) and the decorators→binding gap
    /// ([`Self::param_binding_frozen_span`]) — and the outer one alone reports "not
    /// frozen" for a binding the inner one printed raw. Each family resolves its own
    /// `Option<Span>` and hands it here.
    #[inline]
    pub(in crate::printer) fn element_claim_anchor(frozen: Option<Span>, printed_end: u32) -> u32 {
        frozen.map_or(printed_end, |slice| slice.end)
    }

    /// `elem_end` advanced past the closers of a stripped paren shell — source the
    /// element's doc CONSUMED but did not print, and which no span covers.
    ///
    /// For the callers that measure a *distance* across this gap rather than claim a
    /// comment in it: a blank-line scan. Where the element node's span ends INSIDE its
    /// shell (an array pattern's element is the bare `Identifier`), a scan anchored on the
    /// span counts the shell's own line breaks as an author blank and FABRICATES one —
    /// `[a, (⏎b⏎)⏎/* c */] = x` grew a blank line above `/* c */` that the bare authoring
    /// (`[a, b⏎/* c */] = x`) does not. The fabricated form is stable once printed, so no
    /// ratchet sees it: `fabrication:audit` grades pristine seeds only.
    ///
    /// ⚠️ **Not the seam's anchor**, which must stay at the element's own end so a comment
    /// in the shell's INTERIOR still has a claimant (`docs/comments.md` §The element-comma
    /// seam). The two coexist because this scan **stops at the first comment** (a `/`) and
    /// steps over nothing but `)` and whitespace, so it can never advance past comment
    /// text. It also returns the position just *after* the last closer, never past the
    /// whitespace behind it — a blank line the author wrote below the `)` is authorship and
    /// must still be measured. `bound` is the caller's region end (the closing delimiter,
    /// or an annotation's start).
    pub(in crate::printer) fn element_shell_end(&self, elem_end: u32, bound: u32) -> u32 {
        let bytes = self.source.as_bytes();
        let end = (bound as usize).min(bytes.len());
        let mut i = elem_end as usize;
        let mut shell_end = elem_end;
        while i < end {
            match bytes[i] {
                b')' => {
                    i += 1;
                    shell_end = i as u32;
                }
                b' ' | b'\t' | b'\n' | b'\r' => i += 1,
                _ => break,
            }
        }
        shell_end
    }

    /// Where a blank-line scan STOPS when the next slot is a HOLE: at the hole's own
    /// comma — the SECOND comma past this element, the first being the element's own
    /// separator (`next_real`, the far end of the gap, when either is missing).
    ///
    /// A hole prints nothing, so the next slot's "printed content" is that comma alone and
    /// none of the three spellings a real element's bound has applies. Everything a caller
    /// emits in this gap prints *past* it, and a scan run that far reads the hole's own line
    /// break as an author blank line. The complement is prettier's `node &&`: the blank
    /// **after** a hole is structure, so nothing measures one — only the blank ahead of it,
    /// which the author wrote against this element, survives.
    ///
    /// Stated once because both array containers ask it and only one of them used to: the
    /// pattern skipped the gap entirely when a hole followed and DROPPED the author's blank
    /// (`[a,⏎⏎,⏎b // c] = x`).
    pub(in crate::printer) fn hole_slot_comma(&self, elem_end: u32, next_real: u32) -> u32 {
        self.find_comma_in_range(elem_end, next_real)
            .and_then(|comma| self.find_comma_in_range(comma + 1, next_real))
            .unwrap_or(next_real)
    }

    /// Where a list item's INLINE trailing run ends, for the width-layout emitters that
    /// claim `[item_end, comma_pos)` wholesale and then resume the next item's leading scan
    /// past the comma.
    ///
    /// [`Self::trailing_comment_run`] bounded at the separator, and the same partition:
    /// the prefix of comments that FOLLOW CONTENT on their line trails this item; the first
    /// one starting a line of its own LEADS the next item, and everything behind it goes
    /// with it. Claiming the whole range instead trails a comment the author wrote on its
    /// own line (`{ a⏎/* c */, b }` → `{ a /* c */, b }`), sliding it BACKWARD across the
    /// item's own comma — the one direction `docs/comments.md` §The element-comma seam says
    /// tsv refuses, and a rebinding every other list family (and prettier) declines to make.
    ///
    /// ⚠️ **The caller must resume its leading scan HERE, not past the comma.** The two
    /// answers partition one gap: a comment this run leaves unclaimed has no other emitter
    /// before the separator, so a `prev_end = comma_pos + 1` resume DROPS it.
    pub(in crate::printer) fn inline_trailing_run_end(&self, item_end: u32, comma_pos: u32) -> u32 {
        self.trailing_comment_run(item_end, comma_pos)
            .last()
            .map_or(item_end, |c| c.span.end)
    }

    /// Emit a non-last item's inline trailing run and return the caller's new `prev_end` —
    /// the whole width-layout spelling of the seam in one call, so the emit range and the
    /// resume anchor cannot be given different ends.
    ///
    /// The two halves are one decision ([`Self::inline_trailing_run_end`]): claiming
    /// `[item_end, comma_pos)` wholesale drags an own-line comment BACKWARD across the
    /// item's comma, and resuming past the comma DROPS whatever the run left behind. Every
    /// caller that wrote them as separate statements had to restate that coupling in a
    /// comment; here it is the signature. Shared by the specifier list, the tuple type and
    /// the function-type parameter list — the width-laid-out (`d.line()`) families whose
    /// trailing emitter is [`Self::append_trailing_inline_block_comments`]. The
    /// type-parameter/argument lists emit the same run through
    /// `build_comments_between_filtered` and so take the bare end.
    pub(in crate::printer) fn push_item_trailing_run(
        &self,
        parts: &mut DocBuf,
        item_end: u32,
        next_start: u32,
    ) -> u32 {
        let comma_pos = self.find_list_comma(item_end, next_start);
        let run_end = self.inline_trailing_run_end(item_end, comma_pos);
        self.append_trailing_inline_block_comments(parts, item_end, run_end);
        run_end
    }

    /// The trailing RUN in a list element's gap: the prefix of `[start, end)`'s to-emit
    /// comments that FOLLOW CONTENT on their line, ending AT the first line comment.
    ///
    /// The **positional** half of the element-comma partition, stated once for
    /// [`Self::collect_trailing_comments`] and the array literal's split
    /// (`element_gap_split`, which layers the comma-side test on top per block). The two
    /// have drifted apart before, and both properties are load-bearing:
    ///
    /// - **The source reading** ([`Self::comment_follows_content_on_its_line`]), because
    ///   two kinds of text no item span covers sit in this gap — a stripped paren shell's
    ///   `)` and the list's own comma — and an `is_same_line(elem_end, …)` gate is blind
    ///   to both, so a comment glued to either read as own-line and crossed the separator.
    ///   Asked of BOTH kinds: [`Self::is_own_line_comment`]'s `!is_block` short-circuit is
    ///   a layout answer and calls every line comment own-line. It also subsumes the
    ///   anchor walk this replaces — what the author wrote after a multi-line block's `*/`
    ///   (`{ a: 1 } /* c⏎c2 */ // t`) follows content on its line, so no anchor has to be
    ///   advanced over the block to keep the rest of the run claimed.
    /// - **Ending at the first line comment**, which an anchored gate would guarantee
    ///   for free: a `//` runs to end of line, so nothing could share the element's line
    ///   behind one. Per-comment, every comment after a `//` starts a fresh line and would
    ///   pass the gate — and a second deferred `//` WELDS onto the first's output line,
    ///   its delimiter becoming text inside the first comment, swallowing the code behind
    ///   it (`[A// c⏎,// c2⏎B]`). The print-once ledger is blind to the weld (each comment
    ///   is printed exactly once); `gaps:audit`'s swallow arm is what catches it.
    ///
    /// The run is therefore always a PREFIX of the gap's comments, which is what lets a
    /// caller resume its own leading scan at its end and see everything left over. See
    /// docs/comments.md §The element-comma seam.
    pub(in crate::printer) fn trailing_comment_run(
        &self,
        start: u32,
        end: u32,
    ) -> impl Iterator<Item = &'a Comment> + '_ {
        let mut past_line_comment = false;
        self.comments_to_emit_between(start, end)
            .take_while(move |c| {
                let claimed = !past_line_comment && self.comment_follows_content_on_its_line(c);
                past_line_comment |= !c.is_block;
                claimed
            })
    }

    /// The LAST item→closer gap's trailing claim — the same prefix walk as
    /// [`Self::trailing_comment_run`], with one deliberate difference on **line**
    /// comments.
    ///
    /// A **block** takes the source reading, exactly as the inter-item seam does, and for
    /// the same reason: the list's own comma sits in this gap covered by no item span, so
    /// an `is_same_line(item_end, …)` reading calls a comment glued to it own-line and
    /// opens a list that fits.
    ///
    /// A **line** comment keeps an ANCHOR reading, which is the sanctioned divergence
    /// `last_item_line_after_comma_prettier_divergence` (conformance_prettier_ts_comments.md
    /// §Comment relocation, "**Last**-item trailing-comma line comment"). Prettier hoists a
    /// `//` written after a comma the author gave its own line onto the item; tsv keeps the
    /// line the author gave it, because a last item's comma is **deleted** under
    /// `trailingComma: 'none'` and so is not a position in the output that a comment could
    /// be pulled back onto. A `//` on the item's own line still trails it, which is what
    /// the anchor answers.
    ///
    /// ⚠️ **The two kinds genuinely part ways here, and that is the whole point of a
    /// second walk.** The after-comma anchor's argument — the printer pulls the comma back
    /// onto the item's line — reaches a *block* through the source it is glued to, and
    /// stops at a `//` that has no comma left to be written against
    /// (`docs/comments.md` §Own-line-ness is a SOURCE question).
    ///
    /// ⚠️ **The anchor is the last CLAIMED comment, not `item_end`** — the sanction's
    /// licence stops where its argument does, and its argument is about the **comma**. A
    /// comment this run already claimed is content the printer *itself* pulls onto the
    /// item's line, so a `//` the author glued behind its `*/` has to come along: leaving
    /// it on `item_end`'s line strands it, splitting a pair the author wrote as one
    /// (`f(a⏎, /* b */ // c)` → `a /* b */` with `// c` dropped to its own line, while the
    /// block it was written against moved up without it). Only a `//` with **nothing but
    /// the deleted comma** ahead of it on its line is the sanctioned case, and that is
    /// exactly what an un-advanced anchor still answers. Pinned by
    /// `calls/trailing_arg_glued_run`.
    pub(in crate::printer) fn closer_trailing_comment_run(
        &self,
        item_end: u32,
        end: u32,
    ) -> impl Iterator<Item = &'a Comment> + '_ {
        let mut past_line_comment = false;
        let mut anchor = item_end;
        self.comments_to_emit_between(item_end, end)
            .take_while(move |c| {
                let claimed = !past_line_comment
                    && if c.is_block {
                        self.comment_follows_content_on_its_line(c)
                    } else {
                        self.is_same_line(anchor, c.span.start)
                    };
                if claimed {
                    anchor = c.span.end;
                }
                past_line_comment |= !c.is_block;
                claimed
            })
    }

    /// Where a LAST item→closer gap's trailing run ends — the closer twin of
    /// [`Self::inline_trailing_run_end`], over [`Self::closer_trailing_comment_run`] instead
    /// of the inter-item walk.
    ///
    /// The run is a PREFIX, so its end is also where the gap's own-line half begins: an
    /// emitter walking the whole gap asks `comment.span.end <= run_end` per comment and the
    /// two halves partition it by construction. `item_end` when the run is empty, which is
    /// what makes that comparison false for every comment.
    pub(in crate::printer) fn closer_trailing_run_end(&self, item_end: u32, end: u32) -> u32 {
        self.closer_trailing_comment_run(item_end, end)
            .last()
            .map_or(item_end, |c| c.span.end)
    }

    /// Collect trailing comments for a list element (property or array element)
    ///
    /// Trailing comments are the ones that follow CONTENT on their line
    /// ([`Printer::comment_follows_content_on_its_line`] — the source reading, so the
    /// erased `)` of a stripped paren shell and the comma both count as content):
    /// - Block comments: BEFORE the comma, or after it when a line comment follows
    ///   (see [`TrailingComments::after_comma`])
    /// - Line comments: always belong to this element (they consume the rest of the line)
    ///
    /// The claimed run is always a **prefix** of the gap's comments, so a caller resumes
    /// its own scan at [`TrailingComments::end_pos`] and sees everything left over: an
    /// after-comma block that leads the next element sits behind no claimed comment (only
    /// a line comment could follow it on this line, and that claims the block too).
    pub(in crate::printer) fn collect_trailing_comments(
        &self,
        elem_end: u32,
        upper_bound: u32,
        is_last: bool,
    ) -> TrailingComments<'_> {
        // Zero-comment fast gate: the comma position only classifies comments, so
        // with no comment in the window there is nothing to collect — skip the
        // comma scan entirely.
        if !self.has_comments_to_emit_between(elem_end, upper_bound) {
            return TrailingComments {
                before_comma: SmallVec::new(),
                after_comma: SmallVec::new(),
                line: None,
                line_demoted: false,
                end_pos: elem_end,
            };
        }

        // Find the separator comma in source (if any), skipping commas that sit
        // inside comments or strings so `a /* , */ /* x */, b` is split on the
        // real comma, not the one in `/* , */`. The scan is bounded by
        // `upper_bound` (the old unbounded-then-filter form scanned to the next
        // comma anywhere in the rest of the source).
        let comma_pos = self.find_comma_in_range(elem_end, upper_bound);

        // The element's own trailing run — the positional half of the partition, shared
        // with the array literal's split so the two cannot answer it differently.
        let run: CommentVec<'_> = self.trailing_comment_run(elem_end, upper_bound).collect();

        // Whether the run ends in a `//`, which makes everything ahead of it deferred —
        // what binds an after-comma block to this element rather than to the next one.
        let defers_line = run_defers_line(&run);

        // A block comment after the comma normally belongs to the next element as
        // leading — except on the LAST element, where it is preserved after the comma
        // (prettier relocates it before — see conformance_prettier_ts_comments.md
        // §Comment relocation). With no trailing comma emitted, a last element's after-comma
        // block trails the element in the same run as its before-comma blocks, so all
        // the run's blocks collect into one source-ordered `before_comma` (the comma
        // between them is `None`).
        let is_before_comma = |c: &Comment| block_is_before_comma(is_last, comma_pos, c.span.start);

        let mut before_comma = SmallVec::new();
        let mut after_comma = SmallVec::new();
        let mut line = None;
        let mut end_pos = elem_end;
        for c in run {
            if !c.is_block {
                line = Some(c);
            } else if is_before_comma(c) {
                before_comma.push(c);
            } else if defers_line {
                after_comma.push(c);
            } else {
                // Leads the next element — not this element's to print, and nothing
                // claimed after it (see the prefix note above).
                break;
            }
            end_pos = c.span.end;
        }

        TrailingComments {
            before_comma,
            after_comma,
            line,
            line_demoted: false,
            end_pos,
        }
    }

    /// Build docs for block comments (go before comma)
    fn build_block_comments_doc(&self, comments: &[&Comment]) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        for comment in comments {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }
        d.concat(&parts)
    }

    /// Push one trailing LINE comment, demotion-aware — the single rendering of the
    /// deferred-`//` weld rule, shared by [`Self::push_element_comma_trailing`] and the
    /// array literal's element loop so the two can't drift. Not demoted, the comment
    /// defers past the element through `line_suffix`; demoted (the element's own doc
    /// already ends in a deferred `//` —
    /// [`TrailingComments::demote_line_after_deferred`]), it takes a fresh line of its
    /// own, where a reparse keeps it.
    pub(in crate::printer) fn push_trailing_line_comment_demotion_aware(
        &self,
        parts: &mut DocBuf,
        comment: &Comment,
        demoted: bool,
    ) {
        if demoted {
            parts.push(self.d().hardline());
            parts.push(self.build_comment_doc(comment));
        } else {
            parts.push(self.build_trailing_line_comment_doc(comment));
        }
    }

    /// Push one element's trailing comments around its `comma` doc, in the order
    /// that preserves comment position: the run's before-comma blocks (source-ordered,
    /// including a last element's after-comma block since its comma is `None`),
    /// the comma, the after-comma blocks the author wrote there, then the run's line
    /// comment as a suffix. That is source order for every arrangement of the run, which
    /// is the point: the pieces land on the same side of the comma and in the same
    /// sequence the author gave them. Shared by the object/array pattern element loops and the object-literal
    /// loop so this ordering — the comment-position contract — can't drift between them.
    pub(in crate::printer) fn push_element_comma_trailing(
        &self,
        parts: &mut DocBuf,
        trailing: &TrailingComments<'_>,
        comma: Option<DocId>,
    ) {
        // The comment runs are empty on the common (comment-free) path — collected as
        // empty vecs by the zero-comment gate in `collect_trailing_comments`. Skip
        // pushing their `empty()` docs so a comment-free element leaves no wasted child
        // in the enclosing list concat (which render + every fits pass would still walk).
        // Byte-identical: an empty comment run builds `concat(&[]) == empty()`, so pushing
        // it vs not is the same rendered output.
        if !trailing.before_comma.is_empty() {
            parts.push(self.build_block_comments_doc(&trailing.before_comma));
        }
        // The last element's comma is `None` (trailingComma: 'none'): nothing is pushed,
        // for the same reason the empty runs above are not.
        if let Some(comma) = comma {
            parts.push(comma);
        }
        if !trailing.after_comma.is_empty() {
            parts.push(self.build_block_comments_doc(&trailing.after_comma));
        }
        if let Some(comment) = trailing.line {
            self.push_trailing_line_comment_demotion_aware(parts, comment, trailing.line_demoted);
        }
    }
}
