// Array expression printing for TypeScript
//
// Handles printing of array expressions with:
// - Width-based wrapping
// - Fill mode for number-only arrays
// - Forced expansion for multiline content
// - Comment preservation

use crate::ast::internal::{self, Expression, LiteralValue};
use crate::printer::comments::{block_is_before_comma, next_real_element_start, run_defers_line};
use crate::printer::{
    CommentVec, Printer, container_may_have_multiline_content, has_multiline_content,
};
use smallvec::{SmallVec, smallvec};
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::{comments_to_emit_in_range, has_multiline_block_comments_on_page_in_range};

/// The gap after array slot `i`, in this container's terms: [`next_real_element_start`]
/// with the literal's own fallback — just inside `]`, where a comment past the last real
/// element slides to. The array PATTERN's twin is `array_pattern_gap_end`, whose fallback
/// stops before a `: T` the pattern's span swallowed; each container names its own end
/// once and the walk past the holes is shared.
fn array_gap_end(arr: &internal::ArrayExpression<'_>, i: usize) -> u32 {
    next_real_element_start(arr.elements, i).unwrap_or(arr.span.end - 1)
}

impl<'a> Printer<'a> {
    /// Check if array should force break based on Prettier's heuristic
    ///
    /// Returns true when:
    /// - More than 1 element
    /// - ALL elements are arrays (or ALL are objects - no mixing)
    /// - EACH inner array/object has more than 1 item
    ///
    /// This matches prettier's shouldBreak logic in array.js:89-106
    fn should_break_nested_array(&self, arr: &internal::ArrayExpression<'_>) -> bool {
        if arr.elements.len() <= 1 {
            return false;
        }

        let mut expect_arrays: Option<bool> = None;

        for elem in arr.elements {
            let Some(expr) = elem else { return false };

            let (is_array, inner_len) = match expr {
                Expression::ArrayExpression(inner) => (true, inner.elements.len()),
                Expression::ObjectExpression(inner) => (false, inner.properties.len()),
                _ => return false,
            };

            // All elements must be same type (all arrays or all objects)
            if expect_arrays.is_some_and(|expected| expected != is_array) {
                return false;
            }
            expect_arrays = Some(is_array);

            // Each inner must have more than 1 item
            if inner_len <= 1 {
                return false;
            }
        }

        true
    }

    /// Blank-line rule for the gap after array slot `i` — prettier's
    /// `node && isLineAfterElementEmpty(node)` (`print/array.js`, `printArrayElements`).
    ///
    /// Both terms are load-bearing for elisions:
    ///
    /// - A **hole carries no blank line after it** (prettier's `node &&`): it has no node
    ///   to anchor the scan on, and its own line break is structure, not authorship.
    /// - The scan stops at **slot `i + 1`**, not at the next real element. A hole is empty
    ///   — its slot is the point just before the comma that terminates it — so when slot
    ///   `i + 1` is a hole the next real element lies past that comma, and a scan running
    ///   that far reads the hole's own line break as an author's blank line.
    ///
    /// The scan stops where slot `i + 1`'s **printed content** begins, so a blank line the
    /// author left ahead of that content is inside the measured range. That position has
    /// three spellings, and this function owns all three so no caller can answer with a
    /// subset:
    ///
    /// 1. slot `i + 1`'s first **emitted** leading comment — the only one a caller can know,
    ///    since which comments a gap emits is the caller's own collection
    ///    (`emitted_leading_start`, supplied by the expanding printer; every other printer
    ///    passes `None`);
    /// 2. else slot `i + 1`'s **owned** comment, which prints ahead of the element's first
    ///    token from *inside* the element's own doc and so is invisible to the **to emit**
    ///    axis a caller's list rides — a bound taken from that list alone lands past the
    ///    comment, putting the author's blank line, which lies *before* it, outside the
    ///    range where it is silently dropped;
    /// 3. else the element itself.
    ///
    /// ⚠️ 2 is not the expanding printer's business either, though it once derived it
    /// alongside 1: a glued block comment is not an expansion trigger (neither a line, a
    /// multi-line, nor an own-line comment), so it reaches the width-wrapping and
    /// multiline-content printers too. Two copies of one derivation, and only the copy here
    /// carried the range guard.
    ///
    /// The hole guard runs first, so the scan always anchors on a real element's end —
    /// which also keeps a nested element's commas (`[[1, 2], , x]`) behind it.
    ///
    /// ⚠️ **There is a SECOND implementation of this rule**, in the array PATTERN's
    /// expanding builder (`expressions/patterns.rs`), and it is not foldable into this one:
    /// the pattern anchors on the claimed run's shell-peeled end and takes its bound from
    /// the first comment in SOURCE, where this one anchors on the element's span end and
    /// takes the caller's emitted-leading list. The two share what they *can* — the walk
    /// past the holes ([`next_real_element_start`]) and the hole ceiling
    /// ([`Printer::hole_slot_comma`]) — so a change to either of THOSE is already shared,
    /// but a change to the rule itself has to be made twice. The pattern lacking the hole
    /// arm entirely is what DROPPED an author blank line before an elision.
    fn has_blank_line_after_slot(
        &self,
        arr: &internal::ArrayExpression<'_>,
        i: usize,
        emitted_leading_start: Option<u32>,
    ) -> bool {
        let Some(elem) = arr.elements[i].as_ref() else {
            return false;
        };
        let elem_end = elem.span().end;
        // The next real element's start, or the closing `]`. It bounds the SEPARATOR
        // search, which runs past `upper`: the comma may sit below slot `i + 1`'s leading
        // comment, and `upper` stops at that comment's start
        // ([`Printer::has_blank_line_after_comma`]).
        let next_real = array_gap_end(arr, i);

        // A HOLE's printed content is its comma alone, so none of the three spellings above
        // applies — whatever a caller emits in this gap prints *past* that comma, and
        // bounding there reads the hole's own line break as an author's blank line.
        if matches!(arr.elements.get(i + 1), Some(None)) {
            let hole_comma = self.hole_slot_comma(elem_end, next_real);
            // …but a comment BELOW this element's line does not print at that comma either —
            // it slides forward past it to lead the next real element — so the scan still
            // stops at the comment. Left to run the whole span,
            // `has_blank_line_after_comma`'s own comment-skip steps over it and takes the
            // author's blank line in FRONT of it along (`x,⏎⏎// c⏎,⏎y`).
            // The ceiling opens at the element-comma SPLIT, not at `elem_end`: a comment the
            // split claims TRAILS this element on its line, and prettier's
            // `isNextLineEmptyAfterIndex` skips exactly those before measuring, so bounding
            // at one would read the blank *after* it as absent (`x, // c⏎⏎,⏎y`). Both
            // spellings are live, one per side of the split.
            let split = self.element_gap_split(arr, i, elem_end, next_real);
            let upper = self.blank_scan_end(split, hole_comma);
            return self.has_blank_line_after_comma(elem_end, upper, next_real);
        }

        let upper = emitted_leading_start
            .or_else(|| {
                // Guarded to this gap: the lookup is keyed on the element's own span start,
                // which says nothing about which gap the comment it finds sits in.
                arr.elements
                    .get(i + 1)
                    .and_then(|e| e.as_ref())
                    .and_then(|e| self.owned_leading_comment_start(e))
                    .filter(|&p| p > elem_end && p < next_real)
            })
            .unwrap_or(next_real);
        self.has_blank_line_after_comma(elem_end, upper, next_real)
    }

    /// Format a block comment for inline use (with appropriate spacing)
    ///
    /// - `leading: true` for comments before elements → space after: `/*c*/ elem`
    /// - `leading: false` for comments after elements → space before: `elem /*c*/`
    fn format_inline_block_comment(&self, comment: &tsv_lang::Comment, leading: bool) -> DocId {
        let d = self.d();
        // One text node either way (the full span is the verbatim `/*content*/`,
        // delimiters included) — array fill items must not gain a separate
        // space node.
        let mut w = d.pool_writer();
        if leading {
            w.push_str(comment.span.extract(self.source));
            w.push(' ');
        } else {
            w.push(' ');
            w.push_str(comment.span.extract(self.source));
        }
        let doc = w.finish_text();
        // A comment emission that can't route through `build_comment_doc` (the space must
        // share the node), so it tags its own ledger node.
        #[cfg(feature = "comment_check")]
        d.tag_comment_doc(doc, comment.span, self.source);
        doc
    }

    /// The last real element before slot `i` — its slot index and the element itself.
    ///
    /// A hole has no span, so this is the only anchor a scan across earlier slots can start
    /// from, and the slot index is what [`Self::element_gap_split`] derives `is_last` from.
    /// `None` when every earlier slot is a hole (or there are none): there is no element to
    /// have claimed anything, so the region opens at `[`.
    ///
    /// `i == arr.elements.len()` asks it of the whole array — "the last real element" — which
    /// is the form [`Self::end_scan_start`] and [`Self::last_element_trailing_split`] take, so
    /// the three sites share one walk rather than restating the search past holes.
    fn prev_real_slot<'e>(
        &self,
        arr: &'e internal::ArrayExpression<'e>,
        i: usize,
    ) -> Option<(usize, &'e Expression<'e>)> {
        arr.elements[..i]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(idx, e)| e.as_ref().map(|e| (idx, e)))
    }

    /// Does this **block** comment trail the element before it, rather than lead the one after?
    /// The separator decides, and is the whole rule: before the comma the comment trails
    /// (`[A /* c */, B]`); past it, it leads the next element (`[A, /* c */ B]`). On the
    /// LAST element (`is_last`) there is no next element to lead and the comma is never
    /// emitted (`trailingComma: 'none'`), so the whole same-line run trails — the shared
    /// comma arm, [`block_is_before_comma`], stated once with the collector's classifier.
    ///
    /// A newline after the comment does **not** carry it across the comma. Prettier classifies
    /// on newlines alone (`endOfLine`, `main/comments/attach.js` — a comma is not a node) and so
    /// rewrites `[A, /* c */⏎B]` to `[A /* c */, B]`, flipping the binding from `B` to `A`; tsv
    /// preserves the authored position. The comma is what carries the association, and unlike a
    /// `//` — which runs to end-of-line, so trailing it past the comma is the only rendering
    /// that exists (the sanctioned pure-separator trail) — a block comment renders fine either
    /// side, making the move unforced. See conformance_prettier_ts_comments.md §Comment relocation.
    ///
    /// The separator rule has one exception, and it is the run's **order**:
    /// `run_defers_line` — the run ends in a `//` (`A, /* c */ // x⏎B`), which defers
    /// through `line_suffix`, so a block left to lead `B` would render *after* it and the
    /// authored pair comes back reversed across two lines. Such a block trails `A`
    /// instead — emitted past the comma, in front of the suffix, exactly where it was
    /// written. Same rule, same reason as `Printer::push_trailing_comments_in_range`'s
    /// deferred-run block, and as the shared element-comma emitter's `after_comma` run.
    /// It is a **boolean, not a position**, because the deferring `//` can only be the
    /// run's last member, so every block being classified provably precedes it.
    ///
    /// ⚠️ **This decides the comma SIDE only. The positional half — is the comment in this
    /// element's run at all — is [`Printer::trailing_comment_run`]**, which every caller
    /// goes through first, so the source reading and the run's end are answered once for
    /// both this family and the shared collector. An own-line comment therefore never
    /// reaches here: the run stops before it.
    ///
    /// Only the expanding printer can reach the order exception, and the whole-array gate
    /// is what guarantees it: a `//` anywhere between `[` and `]` sets
    /// `has_expanding_comments`, so a gap holding one is never classified by the
    /// non-expanding callers, where the second arm is provably inert. That is now a
    /// statement about *reachability* only — the complement no longer rests on it, since
    /// [`Self::add_inline_leading_block_comments`] emits a range the split already ends.
    ///
    /// The `is_last` arm's double-print hazard is the end-of-array scans, which must not
    /// re-emit what it claims; [`Self::end_scan_emits_comment`] is the one statement of
    /// that exclusion, and it too keys on the split rather than re-deriving this question.
    fn block_comment_trails_prev_element(
        comment: &tsv_lang::Comment,
        comma_pos: Option<u32>,
        is_last: bool,
        run_defers_line: bool,
    ) -> bool {
        block_is_before_comma(is_last, comma_pos, comment.span.start) || run_defers_line
    }

    /// The SPLIT POINT of the gap after the real element in slot `idx`: where that
    /// element's claimed trailing run ends and the next element's leading run begins.
    ///
    /// The one place the array's partition is decided, so its three readers — the trailing
    /// emitter, the next element's leading scan, and the end-of-array scan — cannot
    /// disagree about a comment. **Every emitter in this file goes through here**, each
    /// taking a *range* from the position rather than re-asking a predicate and hoping the
    /// readings stay complementary; they did not, in all three directions at once.
    ///
    /// ⚠️ **The claim is a PREFIX of the gap's comments**, which is why this is a walk and
    /// not a filter: it stops at the first comment that does not trail, so a later comment
    /// can never be claimed over an earlier one left to lead. Dropping that invariant
    /// REORDERS the authored run — `[a⏎/* c1 */ /* c2 */, b]` has c1 leading its line
    /// (so it leads `b`) while c2, with c1 before it on that line, reads as trailing `a`,
    /// and the pair comes back as `a /* c2 */, /* c1 */ b`. Same rule and same reason as
    /// [`Printer::collect_trailing_comments`]' `break`; see docs/comments.md §The
    /// element-comma seam.
    ///
    /// ⚠️ `is_last` is derived here, not passed in, and it is the **slot** question —
    /// whether `idx` is the final slot, not whether it is the final real element. It is
    /// load-bearing: on the last slot no comma is emitted (`trailingComma: 'none'`), so
    /// "before" and "after" the separator are one position and the whole run trails. Every
    /// hand-written copy of this rule has at some point got that half wrong.
    fn element_gap_split(
        &self,
        arr: &internal::ArrayExpression<'_>,
        idx: usize,
        elem_end: u32,
        gap_end: u32,
    ) -> u32 {
        let comma_pos = self.find_comma_in_range(elem_end, gap_end);
        let is_last = idx + 1 == arr.elements.len();

        // The positional half is the shared run — the source reading, ending at the first
        // line comment — so this printer and the collector cannot disagree about which
        // comments are even candidates. A line comment in the run always trails (nothing
        // can follow it on its line); a block additionally has to keep the author's side
        // of the comma, which is this family's own arm. The run's deferring `//` is read
        // ONCE, above the loop: it is the run's last member, so the answer is the same for
        // every block being classified.
        let run: CommentVec<'_> = self.trailing_comment_run(elem_end, gap_end).collect();
        let defers_line = run_defers_line(&run);

        let mut split = elem_end;
        for comment in run {
            if comment.is_block
                && !Self::block_comment_trails_prev_element(
                    comment,
                    comma_pos,
                    is_last,
                    defers_line,
                )
            {
                break;
            }
            split = comment.span.end;
        }
        split
    }

    /// Emit block comments in `[search_start, elem_start)` as inline-leading
    /// (`/*c*/ elem`). Used by both the first-element and subsequent-element
    /// paths in the non-expanding array printers.
    ///
    /// `search_start` is the gap's split point — everything from there on leads this
    /// element, by [`Self::element_gap_split`] — so no trailing-side filter is
    /// applied or needed here.
    ///
    /// ⚠️ That split must be computed from the previous element's END, not from past the
    /// separator. A scan starting at `comma + 1` cannot see what the author wrote on the
    /// other side of a comma, and the trailing side claims only what precedes the comment
    /// on its line — so a block the author put on its own line before a glued comma
    /// (`[a⏎/* c */, b]`) had no emitter at all. Same rule and same drop as the four
    /// sites `Printer::collect_trailing_comments`' callers were converged onto; see
    /// docs/comments.md §The element-comma seam.
    fn add_inline_leading_block_comments(
        &self,
        search_start: u32,
        elem_start: u32,
        parts: &mut DocBuf,
    ) {
        for comment in comments_to_emit_in_range(self.comments, search_start, elem_start) {
            if comment.is_block {
                parts.push(self.format_inline_block_comment(comment, true));
            }
        }
    }

    /// Where the array element at `i`'s leading-comment scan starts. `elem_start` is this
    /// element's own start, which bounds the scan (only a real element collects leading
    /// comments, so there is always one to bound it).
    ///
    /// **One rule, whatever sits in between**: the scan resumes at the last REAL element's
    /// gap SPLIT POINT ([`Self::element_gap_split`]) — or at `[` when there is no earlier
    /// element to have claimed anything. Everything from there on leads this element,
    /// which is the complement of that element's trailing claim, so the gap is partitioned
    /// exactly once however many elisions lie in it.
    ///
    /// ⚠️ **Intervening holes do not move the anchor forward.** They used to: the scan
    /// started past the hole's comma, located by *counting* commas from the last real
    /// element, on the reading that the region before that comma was "the hole seams'
    /// business". No hole seam prints, so it was nobody's — every comment in it was
    /// DROPPED (`[x⏎/* c */, , y]`, `[/* c */, y]`, `[x, /* c */ , y]`, and a run of two
    /// came back with only its tail). A hole is a **slot**, not an element: it prints
    /// nothing and its comma is re-emitted structure outside every element span, so a
    /// comment slides *forward* past it to the element it leads. Sliding backward across
    /// the previous element's own comma is the move tsv refuses — see
    /// conformance_prettier_ts_comments.md §Comment relocation (`across an elision`).
    fn leading_comment_search_start_for(
        &self,
        arr: &internal::ArrayExpression<'_>,
        i: usize,
        elem_start: u32,
    ) -> u32 {
        match self.prev_real_slot(arr, i) {
            Some((idx, prev)) => self.element_gap_split(arr, idx, prev.span().end, elem_start),
            None => arr.span.start + 1,
        }
    }

    /// Add trailing block comments for an array element — the ones
    /// [`Self::block_comment_trails_prev_element`] binds to it.
    ///
    /// `is_last` is the SLOT question, not the real-element one: a trailing elision keeps
    /// its (syntactically significant) comma, and the comments past it belong to the
    /// trailing-hole seams — so a real element followed by holes must not claim them.
    fn add_trailing_array_comments(
        &self,
        arr: &internal::ArrayExpression<'_>,
        elem_end: u32,
        current_index: usize,
        parts: &mut DocBuf,
    ) {
        // Bounded at `next_boundary`: this element's separator, if it has one, lies before the
        // next element. A SOURCE trailing comma past the last element is still found — the
        // `is_last` arm is what keeps the comments past it on this element.
        let next_boundary = array_gap_end(arr, current_index);
        let split = self.element_gap_split(arr, current_index, elem_end, next_boundary);

        // Everything below the split is this element's, by construction — the next
        // element's leading scan resumes exactly there.
        for comment in comments_to_emit_in_range(self.comments, elem_end, split) {
            if comment.is_block {
                parts.push(self.format_inline_block_comment(comment, false));
            }
        }
    }

    /// Is this comment the end-of-array scan's to emit as an array sibling, rather than
    /// already printed by someone closer?
    ///
    /// The one statement of that exclusion, shared by [`Self::build_array_group_doc`]'s
    /// end-of-array scan and [`Self::build_array_doc_with_expanding_comments`]'s final
    /// scan (it drifted between them once — the group side learned the element anchor and
    /// the final scan kept re-emitting what the element claimed).
    ///
    /// It emits what BOTH closer emitters left, so the two exclusions compose rather than
    /// pick one:
    ///
    /// - **The last real element's trailing CLAIM** — everything below its gap SPLIT
    ///   ([`Self::last_element_trailing_split`]) is already printed. Re-deriving that from
    ///   an anchor plus a same-line test is what let the two drift: the claim's own gate is
    ///   a source reading (a block glued to a stripped `)`, or one written before a glued
    ///   comma, is claimed though it does not share the element's line), so a same-line
    ///   re-derivation DOUBLE-PRINTS every one of those. Taking the split makes the
    ///   exclusion exact by construction, including its prefix rule, which no per-comment
    ///   test can reproduce.
    /// - **The anchor's own line** — a comment glued there belongs to whoever owns that
    ///   line: `Printer::append_spread_trailing_paren_comments` **INSIDE a spread's
    ///   stripped parens** (`scan_start` is the argument's end, below the element's), and
    ///   the hole seams past a **trailing elision**, where the last real element's claim
    ///   stops at its comma and the region beyond is not this scan's to re-parent.
    ///
    /// `last_real` is `None` only when the array holds no real element at all.
    fn end_scan_emits_comment(
        &self,
        comment: &tsv_lang::Comment,
        scan_start: u32,
        last_real: Option<(u32, u32)>,
    ) -> bool {
        if let Some((elem_end, split)) = last_real
            && comment.span.start >= elem_end
            && comment.span.start < split
        {
            return false;
        }
        !self.is_same_line(scan_start, comment.span.start)
    }

    /// Where an end-of-array scan starts: the last REAL element's end — or, when that
    /// element is a spread whose stripped parens hold own-line comments, the spread
    /// ARGUMENT's end, so the scan reaches into that interior to re-parent them.
    ///
    /// The one spelling of that reach, shared by [`Self::build_array_group_doc`]'s
    /// end-of-array scan and [`Self::build_array_doc_with_expanding_comments`]'s final
    /// scan. Asking it two ways — "any to-emit comment in the interior"
    /// vs "own-line BLOCKS in it" — agrees only because
    /// [`Self::end_scan_emits_comment`]'s same-line arm discards the difference
    /// downstream: the drift-risk shape the anchor rule beside it guards against, so
    /// the two are stated once.
    ///
    /// `None` when the array holds no real element: there is nothing to scan past.
    fn end_scan_start(&self, arr: &internal::ArrayExpression<'_>) -> Option<u32> {
        let (_, elem) = self.prev_real_slot(arr, arr.elements.len())?;
        Some(match elem.as_spread() {
            Some(spread) if !self.spread_element_own_line_comments(spread).is_empty() => {
                spread.argument.span().end
            }
            _ => elem.span().end,
        })
    }

    /// The last REAL element's end paired with its gap SPLIT — the
    /// [`Self::end_scan_emits_comment`] key.
    ///
    /// Keyed on the last real element rather than the last SLOT, because a trailing
    /// elision does not stop that element from claiming: its gap still runs to the closing
    /// `]`. `is_last` is the slot question ([`Self::add_trailing_array_comments`]), so with
    /// holes after it the claim stops at its own comma and the hole seams keep the rest —
    /// which is exactly what this reproduces, by computing the split the same way that
    /// emitter does.
    fn last_element_trailing_split(
        &self,
        arr: &internal::ArrayExpression<'_>,
    ) -> Option<(u32, u32)> {
        let (idx, elem) = self.prev_real_slot(arr, arr.elements.len())?;
        let elem_end = elem.span().end;
        // No real element follows, so the gap runs to the closing `]`.
        let gap_end = arr.span.end - 1;
        Some((
            elem_end,
            self.element_gap_split(arr, idx, elem_end, gap_end),
        ))
    }

    /// Build a Doc for an array expression, wrapping on width.
    ///
    /// The single entry point for every array position — top-level and nested alike — so
    /// multiline content triggers the same expansion everywhere.
    pub(in crate::printer) fn build_array_doc(&self, arr: &internal::ArrayExpression<'_>) -> DocId {
        if arr.elements.is_empty() {
            return self.build_empty_brackets_inline_with_comments_doc(arr.span);
        }

        // Whole-array comment-presence flag (one binary search over the `[…]` span).
        // A false gate is exact: every per-element comment sub-range — the
        // expanding-comment checks below, and the inline leading/trailing lookups in
        // the fill/group builders — lies within [span.start, span.end], so when the
        // array holds no comment, none can lie in any of them (canonical reference:
        // build_params_doc_with_comments).
        let has_comments = self.has_comments_on_page_between(arr.span.start, arr.span.end);

        // Check for comments that force expansion: line comments (can't be inline),
        // multi-line block comments (contain hardlines that must propagate),
        // or own-line single-line block comments (on a separate line from adjacent tokens).
        // The gate skips all three sub-queries — and sub-query 3's eager element
        // collect — on the comment-free common case.
        let has_expanding_comments = has_comments
            && (self.has_line_comments_between(arr.span.start, arr.span.end)
                || has_multiline_block_comments_on_page_in_range(
                    self.comments,
                    arr.span.start,
                    arr.span.end,
                )
                || self.has_own_line_block_comments_in_array(arr));

        if has_expanding_comments {
            return self.build_array_doc_with_expanding_comments(arr);
        }

        // Check if any element has multiline content (e.g., line continuation strings)
        // Prettier expands arrays containing multiline strings (recursively)
        let has_multiline = container_may_have_multiline_content(arr.span, self.source)
            && arr
                .elements
                .iter()
                .flatten()
                .any(|elem| has_multiline_content(elem, self.source));

        // Check if this is a "numbers-only" array (use fill) vs other (one-per-line)
        let is_numbers_only = self.is_numbers_only_array(arr);

        if has_multiline {
            // Force expansion with hardlines for multiline content
            self.build_array_group_doc_forced(arr, has_comments)
        } else if is_numbers_only {
            // Use fill for greedy packing of numbers
            self.build_array_fill_doc(arr, has_comments)
        } else {
            // Use group with one-per-line for other content
            self.build_array_group_doc(arr, has_comments)
        }
    }

    /// Check if array contains own-line single-line block comments that force expansion.
    ///
    /// Delegates to the generic `has_own_line_block_comments_in_bracket_list` helper,
    /// filtering out holes (elisions) from the element list.
    ///
    /// The helper reads the SOURCE, the same reading the element→`,` seam takes
    /// ([`Self::block_comment_trails_prev_element`]): a comment with the comma or a
    /// stripped `)` before it on its line is not own-line, so it collapses onto the
    /// element it binds to instead of expanding the list. On an item-boundary reading
    /// those two spellings expanded — a third fixed point neither the bare authoring nor
    /// prettier produces, and, since the reprint puts the comment back on the element's
    /// line, one the next pass immediately collapsed (`[a⏎, /* c */⏎b]` was a 2-pass).
    fn has_own_line_block_comments_in_array(&self, arr: &internal::ArrayExpression<'_>) -> bool {
        let non_null: SmallVec<[_; 8]> = arr.elements.iter().flatten().collect();
        self.has_own_line_block_comments_in_bracket_list(arr.span, &non_null, |e| e.span())
    }

    /// Check if array contains only numeric literals (for fill behavior)
    fn is_numbers_only_array(&self, arr: &internal::ArrayExpression<'_>) -> bool {
        arr.elements.iter().all(|elem| match elem {
            Some(Expression::Literal(lit)) => {
                matches!(lit.value, LiteralValue::Number(_))
            }
            Some(Expression::UnaryExpression(unary)) => {
                // -1, +1 are also numeric
                matches!(
                    unary.operator,
                    internal::UnaryOperator::Minus | internal::UnaryOperator::Plus
                ) && matches!(
                    unary.argument,
                    Expression::Literal(lit) if matches!(lit.value, LiteralValue::Number(_))
                )
            }
            _ => false,
        })
    }

    /// Build fill doc for numbers-only arrays (greedy packing)
    ///
    /// Includes inline block comments between elements.
    /// Uses binary search to find comments: O(log n + k)
    fn build_array_fill_doc(
        &self,
        arr: &internal::ArrayExpression<'_>,
        has_comments: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

        for i in 0..arr.elements.len() {
            // Elements and their glued comments (a hole pushes nothing — though
            // `is_numbers_only_array` already excludes elisions from this path).
            self.push_array_element_with_inline_comments(arr, i, has_comments, &mut parts);

            if i < arr.elements.len() - 1 {
                parts.push(d.comma_line());
            }
        }

        let inner = d.concat(&[d.softline(), d.fill(&parts)]);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.softline());

        d.group(d.concat(&[d.text("["), indented_content, closing_line, d.text("]")]))
    }

    /// Push slot `i`'s element doc plus its inline leading/trailing block comments.
    ///
    /// The one definition of "an array element and the comments glued around it",
    /// shared by [`Self::build_array_fill_doc`], [`Self::build_array_group_doc`] and its
    /// forced twin [`Self::build_array_group_doc_forced`] — the three printers a *glued*
    /// block comment can reach, since gluing is not an expansion trigger and so does not
    /// divert the array to the expanding printer. Emitting the element alone DROPS the
    /// trailing side (the leading side is owned by the element and rides inside its own
    /// doc), so the pairing is an invariant worth having one home rather than three.
    ///
    /// TODO: in the fill printer the pushed comment docs land in the fill's
    /// content/separator alternation as items of their own, so the fill neither measures
    /// nor breaks on a trailing block comment (a 100+ col numbers array ending
    /// `n /* c */` renders over-width instead of wrapping).
    ///
    /// Takes the slot INDEX rather than the element, and resolves it here: the comment
    /// lookups are keyed on `(arr, i)` while the doc comes from the element, so handing
    /// in both would let a caller pair them wrongly and bind an element's comments to its
    /// neighbour. A hole (elision) resolves to nothing and pushes nothing — it has no
    /// span to anchor a gap on.
    ///
    /// `has_comments` is the array-wide zero-comment fast gate: with no comment anywhere
    /// in the array none can lie in this element's leading/trailing gap, so both the
    /// block-comment collection and its comma scan are skipped. Blank-line detection is
    /// comment-independent and stays with the callers, outside the gate.
    fn push_array_element_with_inline_comments(
        &self,
        arr: &internal::ArrayExpression<'_>,
        i: usize,
        has_comments: bool,
        parts: &mut DocBuf,
    ) {
        let Some(expr) = arr.elements[i].as_ref() else {
            return;
        };

        if has_comments {
            let elem_start = expr.span().start;
            let search_start = self.leading_comment_search_start_for(arr, i, elem_start);
            self.add_inline_leading_block_comments(search_start, elem_start, parts);
        }

        parts.push(self.build_arg_expression_doc(expr));

        if has_comments {
            // Trailing block comments — the ones `block_comment_trails_prev_element`
            // binds to this element (before its comma; on the last slot the whole
            // same-line run, its comma never being emitted).
            self.add_trailing_array_comments(arr, expr.span().end, i, parts);
        }
    }

    /// Build group doc for non-numeric arrays (one per line when broken)
    ///
    /// Includes inline block comments between elements.
    /// Uses binary search to find comments: O(log n + k)
    ///
    /// Note: Arrays with expanding comments use build_array_doc_with_expanding_comments instead.
    fn build_array_group_doc(
        &self,
        arr: &internal::ArrayExpression<'_>,
        has_comments: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = d.pooled_docbuf();

        // Check if last element is an elision (requires mandatory trailing comma)
        let has_trailing_elision = arr.elements.last().is_some_and(Option::is_none);

        // Check Prettier's shouldBreak heuristic for nested arrays/objects
        let mut should_break = self.should_break_nested_array(arr);

        // The array's share of a spread's stripped-paren interior, carried until every
        // elision comma between the spread and the next real element is out — the same
        // forward slide the hole region's own comments take
        // ([`Self::leading_comment_search_start_for`]). Emitting it against the spread's own
        // comma is what the reprint disagrees with: by then the parens are gone and the
        // comment is an ordinary hole-region one, so it slides — two fixed points for one
        // document. It always drains, at the next real slot or in the trailing-elision arm.
        let mut pending_spread_comments: CommentVec<'_> = smallvec![];

        for (i, elem) in arr.elements.iter().enumerate() {
            // Elements and their glued comments (a hole pushes nothing).
            self.push_array_element_with_inline_comments(arr, i, has_comments, &mut parts);

            let is_last = i == arr.elements.len() - 1;
            if !is_last {
                let has_blank_after = self.has_blank_line_after_slot(arr, i, None);

                // Separator comma between elements
                parts.push(d.text(","));

                // Own-line block comments from spread with stripped parens: siblings in the
                // array, past this comma and past any elision commas that follow it. Only a
                // spread slot fills this, and a real slot always drains it first, so the
                // assignment never overwrites a live run.
                if let Some(expr) = elem {
                    pending_spread_comments = self.spread_own_line_comments(expr);
                }
                if !matches!(arr.elements.get(i + 1), Some(None)) {
                    for comment in &pending_spread_comments {
                        parts.push(d.line());
                        parts.push(self.build_comment_doc(comment));
                        should_break = true;
                    }
                    pending_spread_comments.clear();
                }

                if has_blank_after {
                    // Blank line preservation: empty line (no indent) then content line (with indent)
                    // Flat mode: just a space (blank line collapses)
                    // Break mode: literalline (empty) + hardline (indented)
                    parts.push(d.if_break(d.concat(&[d.literalline(), d.hardline()]), d.text(" ")));
                } else {
                    parts.push(d.line());
                }
            } else if has_trailing_elision {
                // Trailing comma for elision - MUST be preserved (semantically significant)
                parts.push(d.text(","));

                // The carried spread share drains here when only elisions followed it.
                for comment in &pending_spread_comments {
                    parts.push(d.line());
                    parts.push(self.build_comment_doc(comment));
                    should_break = true;
                }
                pending_spread_comments.clear();

                // The trailing-hole region: everything past the last real element's own
                // claim (e.g. `[, , ,/* c */]`, `[x, /* c */ , ]`). No hole slot prints an
                // element, so add_leading/add_trailing never run for it and this is the
                // region's only emitter — it takes the SPLIT ([`Self::element_gap_split`],
                // via [`Self::last_element_trailing_split`]) so the two sides partition the
                // gap. Anchoring on the last comma instead dropped everything the author
                // wrote *before* one of the elision commas, which no other emitter sees.
                //
                // With no real element at all the region opens at `[`. The split is at or
                // past the element's end either way, so a LAST spread's stripped-paren
                // interior stays the spread doc's share and is never re-emitted here.
                let scan_start = self
                    .last_element_trailing_split(arr)
                    .map_or(arr.span.start + 1, |(_, split)| split);
                for comment in
                    comments_to_emit_in_range(self.comments, scan_start, arr.span.end - 1)
                {
                    if comment.is_block {
                        parts.push(self.build_comment_doc(comment));
                    }
                }
            }
        }

        // Own-line block comments before the closing bracket, emitted as siblings after the
        // last element and forcing the array to break. Only a spread's stripped-paren
        // comments (which `build_spread_doc` skips) actually reach here: any *other*
        // own-line block comment past the last element lies outside every element span, so
        // `has_own_line_block_comments_in_array` sees it and `build_array_doc` routes the
        // array to the expanding printer before this path runs. The collection stays
        // general — it costs nothing and the spread case shares its shape.
        let mut trailing_own_line_comments: CommentVec<'_> = smallvec![];
        // Zero-comment fast gate: the scan collects nothing but comments, so with none
        // anywhere in the array it is a no-op.
        //
        // ⚠️ Gated on the last SLOT, not the last real element: past a trailing elision the
        // region belongs to the `has_trailing_elision` branch above, which already emitted
        // it. Keying this on `end_scan_start`'s last-real-element alone DOUBLE-PRINTS a
        // spread's own-line interior there (`[...(b⏎/* i */⏎), , ]`). The expanding printer
        // has no such branch and instead tracks what its trailing-hole iteration emitted.
        let last_slot_real = arr.elements.last().is_some_and(Option::is_some);
        if let Some(search_start) = (has_comments && last_slot_real)
            .then(|| self.end_scan_start(arr))
            .flatten()
        {
            let last_real_split = self.last_element_trailing_split(arr);
            for comment in comments_to_emit_in_range(self.comments, search_start, arr.span.end - 1)
            {
                // Only what no one closer prints — see `end_scan_emits_comment`
                // for the two-region rule; a comment the element's claim covers would
                // double-print.
                if comment.is_block
                    && self.end_scan_emits_comment(comment, search_start, last_real_split)
                {
                    trailing_own_line_comments.push(comment);
                }
            }
        }

        let mut inner_parts: DocBuf = smallvec![d.softline(), d.concat(&parts)];
        if !trailing_own_line_comments.is_empty() {
            let mut prev_comment: Option<&internal::Comment> = None;
            for comment in &trailing_own_line_comments {
                // A pair the author GLUED onto one line keeps that line
                // ([`Printer::trailing_run_hugs_previous`], the rule every comment run
                // reads); everything else takes the group's break, which `should_break`
                // below makes unconditional.
                if self.trailing_run_hugs_previous(prev_comment, comment.span.start) {
                    inner_parts.push(d.text(" "));
                } else {
                    inner_parts.push(d.line());
                }
                inner_parts.push(self.build_comment_doc(comment));
                prev_comment = Some(comment);
            }
            should_break = true;
        }

        let inner = d.concat(&inner_parts);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.softline());

        // Build group contents
        let group_contents = d.concat(&[d.text("["), indented_content, closing_line, d.text("]")]);

        // Use group_break() when shouldBreak heuristic matched or spread comments force it.
        // This sets shouldBreak on the GROUP ITSELF rather than using break_parent().
        // The difference: shouldBreak is local to this group, while break_parent()
        // propagates up and forces enclosing groups to break.
        // Prettier uses shouldBreak for this heuristic (array.js lines 89-106, 143).
        if should_break {
            d.group_break(group_contents)
        } else {
            d.group(group_contents)
        }
    }

    /// Build group doc for arrays with multiline content (forced expansion with hardlines)
    ///
    /// The hardline twin of [`Self::build_array_group_doc`], taking the same array-wide
    /// `has_comments` gate from the shared dispatch in [`Self::build_array_doc`] and
    /// sharing its [`Self::push_array_element_with_inline_comments`] seam: a *glued* block
    /// comment is not an expansion trigger, so `build_array_doc` does not divert a
    /// commented array to the expanding printer, and a glued comment reaches this path
    /// whenever some element also holds multiline content.
    fn build_array_group_doc_forced(
        &self,
        arr: &internal::ArrayExpression<'_>,
        has_comments: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

        for i in 0..arr.elements.len() {
            if i > 0 {
                parts.push(d.text(","));

                // Check for blank line before this element (preserved when wrapped) — the
                // gap after the previous slot. Use literalline() for the blank line (no
                // trailing whitespace) then hardline() for the indented content line —
                // matches the non-forced path.
                if self.has_blank_line_after_slot(arr, i - 1, None) {
                    parts.push(d.literalline());
                }

                parts.push(d.hardline());
            }

            // Elements and their glued comments (a hole pushes nothing).
            self.push_array_element_with_inline_comments(arr, i, has_comments, &mut parts);
        }

        // No trailing comma after the last element under `trailingComma: 'none'`, and no
        // trailing comment left to place: the last element's same-line run — including a
        // block past its source trailing comma — is claimed by the element seam above, and
        // an own-line block comment before the closing bracket can't reach this path.
        // `build_array_doc` routes an array to `build_array_doc_with_expanding_comments`
        // whenever one is present — a single-line one via
        // `has_own_line_block_comments_in_array` (which returns true for any own-line
        // comment past the last element), a multi-line one via the on-page check.
        let inner = d.concat(&[d.hardline(), d.concat(&parts)]);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.hardline());
        d.concat(&[d.text("["), indented_content, closing_line, d.text("]")])
    }

    /// Build a Doc for an array with comments that force expansion.
    ///
    /// Used for arrays containing line comments (can't be inline) or multi-line
    /// block comments (hardlines must propagate). Always expands to multiline.
    fn build_array_doc_with_expanding_comments(
        &self,
        arr: &internal::ArrayExpression<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts = d.pooled_docbuf();

        // A comment trailing the opening `[` on its own line is kept on the `[`
        // line (divergence from prettier, which relocates it to its own line as the
        // first element's leading comment). See conformance_prettier_ts_comments.md
        // §Comment relocation (Array literal `[`).
        let first_elem_start = arr
            .elements
            .iter()
            .flatten()
            .next()
            .map_or(arr.span.end - 1, |e| e.span().start);
        let (bracket_line_prefix, bracket_pull_pos) =
            self.delimiter_line_comment_prefix(arr.span.start, first_elem_start);

        // End of the most recently emitted REAL element, and its slot. Holes don't advance
        // either; this lets the next real element's leading-comment range walk back across
        // any intervening hole commas to claim the comments between them and the
        // previous real element. The slot is what `element_gap_split` derives `is_last`
        // from, so the leading side asks the same question the trailing side answered.
        let mut last_real_emit_end = arr.span.start + 1;
        let mut last_real_slot: Option<usize> = None;

        // End position of the last trailing-on-array comment emitted by the
        // trailing-hole iteration (when present). The post-loop scan starts here
        // to avoid re-emitting those comments.
        let mut trailing_hole_comments_end: Option<u32> = None;

        // A spread element whose stripped-paren interior the array still owes a line to,
        // held across the elisions that follow it — see the emission site below.
        let mut pending_spread_share: Option<&Expression<'_>> = None;

        for (i, elem) in arr.elements.iter().enumerate() {
            // O(remaining elements) — compute once and reuse below.
            let next_boundary = array_gap_end(arr, i);
            let (elem_start, elem_end) = match elem {
                Some(e) => (e.span().start, e.span().end),
                None => (next_boundary, next_boundary),
            };

            // Hole at the LAST element index: its leading comments are trailing
            // on the array as a whole (no future real element to attach to).
            // Collect them and emit inline after the hole's comma below.
            let is_trailing_hole = elem.is_none() && i + 1 == arr.elements.len();

            // For real elements: comments in (last_real_emit_end, elem_start).
            // For a trailing hole: same range, but extended to the closing `]`.
            // Other holes contribute nothing — their comments belong to the next
            // real element's leading-comment range.
            //
            // Filter rule: comments same-line with the previous real element are
            // trailing on it, EXCEPT block comments past its comma, which are
            // leading on this element.
            let leading_upper = if elem.is_some() {
                Some(elem_start)
            } else if is_trailing_hole {
                Some(arr.span.end - 1)
            } else {
                None
            };
            let leading_comments: CommentVec<'_> = if let Some(upper) = leading_upper {
                // Resume at the previous element's split point — the complement of its
                // trailing claim, stated once rather than re-derived as a filter here.
                let scan_start = match last_real_slot {
                    Some(prev) => self.element_gap_split(arr, prev, last_real_emit_end, upper),
                    None => last_real_emit_end,
                };
                comments_to_emit_in_range(self.comments, scan_start, upper)
                    .filter(|c| {
                        // Bracket-line comments pulled onto the `[` line above are
                        // emitted as the prefix, not as leading on the first element.
                        // (Only the first element's gap can be same-line as `[`.)
                        !bracket_pull_pos
                            .is_some_and(|dpos| self.comment_on_delimiter_line(dpos, c))
                    })
                    .collect()
            } else {
                smallvec![]
            };

            // The separator for the gap BEFORE this slot. It is emitted here, by the slot
            // that follows the gap, because the ONE thing only this slot knows is which
            // comments its gap actually emits — and the blank-line scan must stop at the
            // first of them, so that a blank line the author left ahead of it is inside the
            // measured range. Everything else about that bound (the owned-comment case, the
            // hole case, the plain element case) is `has_blank_line_after_slot`'s, since
            // those reach printers that emit no leading run at all.
            //
            // ⚠️ A **hole** hands over nothing, whatever this slot's list holds. Its printed
            // content is the comma alone, and the trailing-hole branch below fills
            // `leading_comments` with the ARRAY's trailing run, which sits *past* that
            // comma: bounding the scan there reads the hole's own line break as an author's
            // blank line. That fabricated blank was an F1 break, since the reprint puts the
            // comment on its own line where the next pass measures a real blank
            // (`[x⏎/* c */, , ]` → `[x,⏎,⏎/* c */]` → `[x,⏎⏎,⏎/* c */]`).
            if i > 0 {
                let emitted_leading_start = elem
                    .as_ref()
                    .and_then(|_| leading_comments.first())
                    .map(|c| c.span.start);
                if self.has_blank_line_after_slot(arr, i - 1, emitted_leading_start) {
                    parts.push(d.literalline());
                }
                parts.push(d.hardline());
            }

            // The element's leading run and the element form one group — see
            // `build_list_element_group` for why. A hole takes neither. An own-line
            // format-ignore directive in the element's gap freezes it verbatim (Rule A);
            // this is the only element printer a directive reaches, since either spelling
            // of an own-line comment routes the whole array here.
            if let Some(e) = elem {
                let element_doc =
                    match self.element_frozen_span(arr.span.start + 1, arr.elements, i) {
                        Some(frozen) => self.build_frozen_arg_doc(e, frozen),
                        None => self.build_arg_expression_doc(e),
                    };
                parts.push(self.build_list_element_group_from_comments(
                    leading_comments.iter().copied(),
                    elem_start,
                    element_doc,
                ));
            }

            let is_last = i + 1 == arr.elements.len();

            // Same-line trailing comments (real elements only).
            let trailing_comma_pos = elem
                .is_some()
                .then(|| self.find_comma_in_range(elem_end, next_boundary))
                .flatten();
            let trailing: CommentVec<'_> = if elem.is_some() {
                // The claimed prefix of this gap; the next slot's leading scan resumes at
                // the same split, so every comment lands on exactly one side.
                let split = self.element_gap_split(arr, i, elem_end, next_boundary);
                comments_to_emit_in_range(self.comments, elem_end, split).collect()
            } else {
                smallvec![]
            };
            // Which side of the comma each trailing block keeps — the author's side. A
            // block past a non-last element's comma is here only because a line comment
            // follows it (see `block_comment_trails_prev_element`), and it must render in
            // front of that deferred suffix, so the run comes out in source order. On the
            // LAST element the comma below is never emitted, so its after-comma blocks
            // render straight against the element — the only position left once the
            // separator the author wrote them against is gone (prettier agrees).
            let past_comma =
                |c: &tsv_lang::Comment| trailing_comma_pos.is_some_and(|pos| c.span.start > pos);

            for comment in trailing.iter().filter(|c| c.is_block && !past_comma(c)) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }

            // Separator comma between elements; under `trailingComma: 'none'` the last
            // REAL element gets no trailing comma, but a trailing-elision hole keeps its
            // (syntactically significant) comma.
            if !is_last || elem.is_none() {
                parts.push(d.text(","));
            }

            for comment in trailing.iter().filter(|c| c.is_block && past_comma(c)) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }

            // A spread whose stripped parens held a `//` already ends its line in one, so a
            // line comment written after the `)` takes its own line instead of welding onto
            // it — the array's spelling of `TrailingComments::demote_line_after_deferred`
            // (the demotion trigger this loop owns, per the note at the top of
            // `element_comma.rs`; the rendering is the shared helper).
            let defers_line = elem
                .as_ref()
                .is_some_and(|e| self.defers_trailing_line_comment(e));
            for comment in trailing.iter().filter(|c| !c.is_block) {
                self.push_trailing_line_comment_demotion_aware(&mut parts, comment, defers_line);
            }

            // The array's share of a spread's stripped-paren interior, past the comma —
            // the own-line comments the spread's own doc leaves behind. The LAST element has
            // no comma to emit against, so its share is the final scan's second anchor
            // instead (see `end_scan_emits_comment` and `final_scan_start` below).
            //
            // Carried across any elisions that follow the spread, so it lands past their
            // commas too — the same forward slide every other hole-region comment takes,
            // and the one the reprint performs once the parens are gone (the share is an
            // ordinary own-line comment there). Emitting it against the spread's own comma
            // left our output disagreeing with our reprint of it. A non-spread element
            // carries nothing, so the push is a no-op for it.
            //
            // The drain slot says whether it was in fact carried: `elem` is the spread
            // itself when the next slot is real, and a HOLE when elisions intervened —
            // which is exactly when the authored blank inside the parens must be dropped,
            // the elision's line break being structure rather than authorship.
            if !is_last && elem.is_some() {
                pending_spread_share = elem.as_ref();
            }
            if !matches!(arr.elements.get(i + 1), Some(None))
                && let Some(e) = pending_spread_share.take()
            {
                self.push_spread_own_line_comments_with_blanks(&mut parts, e, elem.is_some());
            }

            // Trailing-hole iter: emit collected trailing-on-array comments inline
            // after this hole's comma. First same-line block comment hugs the comma
            // (no separator); subsequent or own-line comments use hardline.
            if is_trailing_hole && !leading_comments.is_empty() {
                // Source position of the LAST comma before `]` (the comma we just
                // emitted for this hole). Used as the same-line anchor for the
                // first comment.
                let last_comma = self.find_last_comma_before(last_real_emit_end, arr.span.end - 1);

                for (ci, comment) in leading_comments.iter().enumerate() {
                    let same_line_inline = if ci == 0 {
                        comment.is_block
                            && last_comma.is_some_and(|c| self.is_same_line(c, comment.span.start))
                    } else {
                        let prev_comment_end = leading_comments[ci - 1].span.end;
                        comment.is_block
                            && self.is_same_line(prev_comment_end, comment.span.start)
                            && !self.has_blank_line_between(prev_comment_end, comment.span.start)
                    };
                    if same_line_inline {
                        parts.push(self.build_comment_doc(comment));
                    } else {
                        if ci > 0 {
                            let prev_comment_end = leading_comments[ci - 1].span.end;
                            if self.has_blank_line_between(prev_comment_end, comment.span.start) {
                                parts.push(d.literalline());
                            }
                        }
                        parts.push(d.hardline());
                        parts.push(self.build_comment_doc(comment));
                    }
                }
                trailing_hole_comments_end = leading_comments.last().map(|c| c.span.end);
            }

            // No separator is emitted here: the gap after this slot belongs to the slot that
            // follows it, which emits it on the way in. That slot is the only one that knows
            // where its own printed content begins — a fact this end-of-iteration position
            // could only *predict*, by re-deriving the next slot's leading comments before
            // they are collected. Two derivations of one fact drift: the prediction read the
            // **in source** axis while the collection read **to emit**, so a glued (owned)
            // comment bounded one and not the other, and the separator went missing on
            // exactly the arrays that had one.
            if elem.is_some() {
                last_real_emit_end = elem_end;
                last_real_slot = Some(i);
            }
        }

        // Final comments before closing bracket. Skip what trailing-hole emission
        // already handled; past that, only what no one closer prints — see
        // `end_scan_emits_comment` for the two exclusions it composes.
        //
        // `end_scan_start` is what reaches inside a LAST spread's stripped parens, which
        // is how that element's own-line share gets to the array: there is no comma past
        // it to emit against (a non-last spread's share is pushed by the loop above).
        // Only this scan may see that region — pulling `last_real_emit_end` back too
        // would hand it to the NEXT element's leading scan, which the element's own
        // trailing claim has already emitted from: the anchor shift `docs/comments.md`
        // names, and it DOUBLE-PRINTS every `//` written in the gap after the `)`.
        let final_scan_start = trailing_hole_comments_end
            .or_else(|| self.end_scan_start(arr))
            .unwrap_or(arr.span.start + 1);
        let last_real_split = self.last_element_trailing_split(arr);
        let mut prev_end = final_scan_start;
        // The comment this scan emitted last — `None` after one it skipped, which another
        // emitter put on a line of its own, so nothing here has it to glue to.
        let mut prev_comment: Option<&internal::Comment> = None;
        for comment in comments_to_emit_in_range(self.comments, final_scan_start, arr.span.end - 1)
        {
            if self.end_scan_emits_comment(comment, final_scan_start, last_real_split) {
                // A pair the author GLUED onto one line keeps that line
                // ([`Printer::trailing_run_hugs_previous`], the rule every end-of-container
                // run reads); otherwise the comment takes its own line, preserving an
                // author blank before it.
                self.push_trailing_run_separator(
                    &mut parts,
                    prev_comment,
                    prev_end,
                    comment.span.start,
                );
                parts.push(self.build_comment_doc(comment));
                prev_end = comment.span.end;
                prev_comment = Some(comment);
            } else {
                prev_comment = None;
            }
        }

        let inner = d.concat(&[d.hardline(), d.concat(&parts)]);
        let (indented_content, closing_line) = self.wrap_with_decl_indent(inner, d.hardline());

        d.concat(&[
            d.text("["),
            d.concat(&bracket_line_prefix),
            indented_content,
            closing_line,
            d.text("]"),
        ])
    }

    /// Build a Doc for an array expression with forced expansion (hardlines).
    ///
    /// Used by chain arg formatting when we need the array to expand internally
    /// with hardlines so fits() can correctly measure the first line.
    /// Produces: `[\n  elem,\n]` with actual hardlines.
    pub(in crate::printer) fn build_array_doc_expanded(
        &self,
        arr: &internal::ArrayExpression<'_>,
    ) -> DocId {
        let d = self.d();
        // A commented array hands off to `build_array_doc` wholesale — the element-doc-only
        // loop below would DROP every structural comment, the empty-`[]` dangling one
        // included. The object twin (`build_object_doc_expanded`) carries the same gate for
        // the same reason; the rationale lives there in full.
        if self.has_comments_on_page_between(arr.span.start, arr.span.end) {
            return self.build_array_doc(arr);
        }
        if arr.elements.is_empty() {
            return d.text("[]");
        }

        let mut parts = DocBuf::new();
        for (i, elem) in arr.elements.iter().enumerate() {
            // Elements are Option<Expression> where None = hole/elision
            if let Some(expr) = elem {
                parts.push(self.build_arg_expression_doc(expr));
            }
            // Holes are represented by just a comma (no element content)

            if i < arr.elements.len() - 1 {
                parts.push(d.text(","));
                parts.push(d.hardline());
            } else if elem.is_none() {
                // Trailing-elision hole keeps its (syntactically significant) comma;
                // a real last element gets no trailing comma under `trailingComma: 'none'`.
                parts.push(d.text(","));
            }
        }

        d.concat(&[
            d.text("["),
            d.indent_hardline(d.concat(&parts)),
            d.hardline(),
            d.text("]"),
        ])
    }
}
