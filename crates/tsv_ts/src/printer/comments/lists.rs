// List- and body-level comment emitters.
//
// These handle comments across a member/element list or a body: leading/trailing
// comments with blank-line preservation, the open-delimiter trailing-comment
// divergence (delimiter-line prefix), empty-container comments, signature/body
// comment splitting, inline-block comment runs, and comma emission in forced-
// multiline lists.

use super::{CommentVec, LeadingGlue, Printer};
use crate::ast::internal;
use crate::printer::{next_printed_stmt, next_printed_stmt_start, statement_gap_floor};
use tsv_lang::Span;
use tsv_lang::comments_in_source_range;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// Which blank-line rule a comma-separated list's separator follows.
///
/// Prettier asks two different questions here, and a list belongs to exactly one of them —
/// so the caller names its kind rather than passing a bare "preserve?" bool that cannot say
/// which rule it meant. The split is not cosmetic: the two disagree precisely when the comma
/// and the blank line sit on different lines.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BlankRule {
    /// The list has no blank-preservation rule; the separator is a plain hardline.
    None,
    /// Prettier's `isNextLineEmpty` — measured from the **element's end**, so the blank must
    /// begin on the line that element ends on. Params, call arguments, object properties:
    /// prettier emits a `hardline` there, so a blank also forces the list to break.
    NextLineEmpty,
    /// Prettier's `isLineAfterElementEmpty` — advance to the **comma** first, then measure.
    /// Arrays and tuples: prettier emits a `softline`, so a blank never forces a break, and a
    /// blank before the comma is not preserved.
    AfterComma,
}

/// What follows a member in its body, for [`Printer::member_trailing_run`].
///
/// The `floor` is the gap's slot floor — where the leads-next test
/// ([`Printer::comment_leads_next_item`]) opens — and it is a **per-family fact**, not a
/// preference, so the caller states it rather than the shared walk guessing: a class body
/// steps past the stray `;`s that print nothing (`class_member_gap_floor`), while an
/// interface member's own span already covers its separator. Same shape and same reason as
/// [`BlankRule`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MemberGap {
    /// A next member starts at `start`; the leads-next test opens at `floor`.
    Next { floor: u32, start: u32 },
    /// Nothing prints after this member in the body, so nothing can be led and the claim
    /// runs to `list_end` — the body's end, which only this arm has any use for, hence its
    /// place here rather than in the call's signature.
    Last { list_end: u32 },
}

/// How a brace container decides a standalone block comment is **glued to what follows**
/// ([`Printer::has_standalone_block_comment`]).
///
/// The leading half of that question is one rule everywhere — the source
/// ([`Printer::comment_follows_content_on_its_line`]). The trailing half is not, because
/// prettier genuinely answers it differently for the two families, so the caller names its
/// rule rather than inheriting one. Same shape and same reason as [`BlankRule`], and as the
/// union-vs-intersection split [`Printer::is_own_line_comment`] carries.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum StandaloneGlue {
    /// The SOURCE reading (the glue half is [`Printer::comment_hugs_next`], reached through
    /// the shared [`Printer::block_comment_owns_its_line`]): anything after the `*/` on
    /// that line glues the comment, the container's own comma included.
    /// The **object literal** and **object pattern**, whose separator is re-emitted
    /// structure outside every property span — so `{ a: 1⏎/* c */,⏎b: 2 }` collapses
    /// inline, as prettier collapses it, instead of expanding on pass 1 and collapsing on
    /// pass 2. A **dangling** comment — none of `item_spans` starts after it — is exempt:
    /// the closer pulled onto its line is not glue, the same carve-out the bracket list's
    /// no-element-follows arm makes.
    Source,
    /// Only an ITEM starting on the comment's line glues it; the separator does not. The
    /// **type literal**, and this is prettier's own answer rather than a residual: it
    /// expands `type T = { a: A⏎/* c */,⏎b: B }` where it collapses the object literal
    /// written the same way. The mechanism is that a TS member's range swallows its own
    /// `;`/`,` terminator, so a comment before it is *inside* the member and never reaches
    /// prettier's container gate at all — an item-start reading is what reproduces that
    /// from the outside.
    ItemStart,
}

impl<'a> Printer<'a> {
    /// Build the leading-comment run over `[start, end)` for a list whose comments have
    /// forced it multiline (tuples, type params/args, function-type params, unions, the
    /// bracket-break shell, the broken cast).
    ///
    /// A thin adapter over the shared leading-comment emitter
    /// ([`Printer::push_leading_comment_run`]), so the separator after each comment
    /// follows prettier's `printLeadingComment` (space / soft `line` / hardline, keyed on
    /// the source around *that* comment, never on where `end` is).
    ///
    /// `skip_delim` drops the comments sharing `pos`'s source line: they were already
    /// emitted as a trailing prefix on the opening delimiter's line (see
    /// [`Self::delimiter_line_comment_prefix`]), so emitting them here too would
    /// **duplicate** them. Pass the `Option<u32>` that helper returns — gated to the list's
    /// first element, `None` for the rest, and `None` where no delimiter is involved.
    pub(in crate::printer) fn build_leading_comments_multiline(
        &self,
        start: u32,
        end: u32,
        skip_delim: Option<u32>,
    ) -> DocBuf {
        let d = self.d();
        let mut parts = DocBuf::new();
        self.push_leading_comment_run(
            &mut parts,
            comments_to_emit_in_range(self.comments, start, end)
                .filter(|c| !skip_delim.is_some_and(|pos| self.comment_on_delimiter_line(pos, c))),
            end,
            LeadingGlue::Adjacent,
            d.empty(),
        );
        parts
    }

    /// Assemble one **element** of an array-like list: its leading-comment run plus the
    /// element, wrapped in a single `group`.
    ///
    /// The group is what lets a leading comment's soft `line` (prettier's
    /// `printLeadingComment`) be measured against *this element alone* and collapse
    /// (`/* c1 */ /* c2 */ a`) even though the list itself is broken. Prettier's
    /// `printArrayElements` (`src/language-js/print/array.js`) does the same — it pushes
    /// `group(print())` per element and `print()` carries the element's leading comments —
    /// and array literals, array patterns, and tuple types all route through it. A line
    /// comment (or an author blank line) in the run puts a `hardline` inside the group, so
    /// it breaks and the element drops below, also matching prettier.
    ///
    /// This grouping is what separates the array family from the params family: function /
    /// type-parameter / type-argument / call-argument lists use a bare `join([",", line])`
    /// with **no** per-element group, so the identical soft `line` rides the broken outer
    /// group and breaks. That one fact predicts the layout at every one of those sites —
    /// don't re-derive it. See conformance_prettier_ts_comments.md §Comment relocation.
    ///
    /// A list with holes passes only its real elements here; a hole carries no comments and
    /// takes no group.
    pub(crate) fn build_list_element_group(&self, mut leading: DocBuf, element: DocId) -> DocId {
        let d = self.d();
        leading.push(element);
        d.group(d.concat(&leading))
    }

    /// [`Self::build_list_element_group`] for a caller holding the element's leading
    /// comments as a list rather than a range — the array literal and array pattern, whose
    /// per-element filter (which same-line block comments trail the *previous* element
    /// across its comma) is too specific to express as a range.
    ///
    /// Builds the run here so the separator policy every array-family element shares
    /// (`LeadingGlue::Adjacent`, no continuation indent) is stated once rather than at each
    /// call. The range-holding sibling is
    /// [`Self::build_leading_comments_multiline`].
    pub(crate) fn build_list_element_group_from_comments<'c>(
        &self,
        comments: impl Iterator<Item = &'c internal::Comment>,
        element_start: u32,
        element: DocId,
    ) -> DocId {
        let mut leading = DocBuf::new();
        self.push_leading_comment_run(
            &mut leading,
            comments,
            element_start,
            LeadingGlue::Adjacent,
            self.d().empty(),
        );
        self.build_list_element_group(leading, element)
    }

    /// Build the docs for a gap that FOLLOWS a printed item and runs to the next printed
    /// token: the trailing run inline behind the item, everything after it on its own line.
    ///
    /// **One ordered pass**, because the two outcomes interleave: `f(a⏎, /* c1 */⏎/* c2 */)`
    /// trails `c1` on the item's line and gives `c2` its own, and two buffers appended in a
    /// fixed order print them REVERSED.
    ///
    /// Which comments TRAIL is [`Printer::closer_trailing_comment_run`], taken as a PREFIX
    /// ([`Printer::closer_trailing_run_end`] is its boundary), and the run stops at the
    /// first `//` since nothing may share a line comment's line. A **block** takes the
    /// SOURCE reading, because an `is_same_line(start, …)` one is blind to every byte no
    /// item span covers — above all the list's own comma, which the author can push onto
    /// its own line (`[T⏎, /* c */]`), leaving the comment on a line it never had
    /// (`docs/comments.md` §Own-line-ness is a SOURCE question). A **line** comment keeps
    /// the item anchor, the sanctioned divergence that walk carries.
    ///
    /// ⚠️ **Three gap SHAPES share this walk, and the reading is sound at all three even
    /// though it was chosen for one.** `start` is never an opening delimiter — it is the
    /// end of something printed — so "does content precede this comment on its line" is the
    /// same question at a **closer** gap (a last item→`]`/`>`/`)`), at the **delimiter** gap
    /// after a cast's `>`, and at the **operator** gap before a union's `|`. Only the first
    /// can OBSERVE the difference: the two readings part exactly where text no item span
    /// covers sits on the comment's line, and a closer gap holds the list's own comma
    /// (deleted under `trailingComma: 'none'`) while the other two hold nothing but trivia.
    /// The one byte that can reach them is a preceding comment's `*/` — and a comment the
    /// source reading claims for *that* reason is GLUED to its predecessor, so the trailing
    /// arm and the own-line arm's glue branch emit the identical ` ` + comment. **The two
    /// arms agree on exactly the set the readings disagree about**, which is what makes one
    /// walk sound at all three shapes; either mechanism alone already prints
    /// `| A /* a⏎b */ /* c */⏎// d⏎| B` the way prettier does (the delimiter reading with no
    /// glue arm — the state before this seam was unified — split a pair the author wrote on
    /// one line, and that was a real divergence). A gap whose anchor genuinely IS the
    /// delimiter keeps the delimiter reading and does not belong here
    /// ([`Self::delimiter_line_comment_prefix`]'s question, not this walk's).
    ///
    /// Used wherever such a gap is already broken across lines: the tuple, type
    /// parameters and arguments, both parameter lists, and the angle-bracket cast.
    pub(crate) fn build_trailing_gap_comments(&self, start: u32, end: u32) -> DocBuf {
        self.build_trailing_gap_comments_ext(start, end, false)
    }

    /// As [`Self::build_trailing_gap_comments`], but when `suffix_trailing_lines` is set a
    /// **line** comment in the trailing run is routed through `line_suffix` (zero width) so
    /// it can't force the preceding element to break. Only safe where the following
    /// separator lands on a *new* line (so the suffix flushes at that hardline without
    /// crossing the separator) — true for the union's leading-`|` form and for a parameter
    /// list this comment has already forced open, but NOT the intersection's trailing-`&`
    /// form (a same-line `//` there would otherwise comment out the `&`; that case is
    /// handled as a comment-position divergence instead).
    pub(crate) fn build_trailing_gap_comments_ext(
        &self,
        start: u32,
        end: u32,
        suffix_trailing_lines: bool,
    ) -> DocBuf {
        let d = self.d();
        let mut parts = DocBuf::new();
        let run_end = self.closer_trailing_run_end(start, end);
        let mut prev_end = start;
        // The comment emitted last, for the glue question in the own-line arm.
        let mut prev_comment: Option<&internal::Comment> = None;
        for comment in comments_to_emit_in_range(self.comments, start, end) {
            if comment.span.end <= run_end {
                if suffix_trailing_lines {
                    // Block → inline (width counted); line → line_suffix (zero width).
                    parts.push(self.build_trailing_comment_doc(comment));
                } else {
                    // Trailing: inline, behind the item.
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
            } else {
                // Own line comment (block or line) — unless the author GLUED it to the
                // previous one, which keeps that line. Otherwise an author blank line
                // before it (`elem⏎⏎/* c */` before the closing delimiter) is preserved:
                // prettier keeps one blank in every list position (tuple, function/-type
                // params, signatures, type args/params).
                self.push_trailing_run_separator(
                    &mut parts,
                    prev_comment,
                    prev_end,
                    comment.span.start,
                );
                parts.push(self.build_comment_doc(comment));
            }
            prev_end = comment.span.end;
            prev_comment = Some(comment);
        }
        parts
    }
    /// Filter block comments between two positions based on whether they're on the same line as start
    ///
    /// # Arguments
    /// * `start` - Start position (e.g., end of previous chain element)
    /// * `end` - End position (e.g., start of next chain element)
    ///
    /// Returns the block comments on the same source line as `start`.
    pub(crate) fn filter_block_comments(&self, start: u32, end: u32) -> CommentVec<'_> {
        comments_to_emit_in_range(self.comments, start, end)
            .filter(|c| c.is_block)
            .filter(|c| self.is_same_line(start, c.span.start))
            .collect()
    }

    /// True when a block comment in the LAST item→closer gap `(start, end)` owns its line,
    /// so the list must open around it — the trailing counterpart of the leading-run walk
    /// ([`Printer::has_leading_own_line_comment_in_params`]), asked by the value-level
    /// parameter list ([`Printer::has_trailing_line_comment_in_params`]) and the type-level
    /// one (`type_params_force_multiline`).
    ///
    /// The classification is the shared one ([`Printer::block_comment_owns_its_line`]) with
    /// `item_follows: false`: no item is left to lead, so the closer sharing the comment's
    /// line is not glue and the predicate reduces to its leading half — nothing before the
    /// `/*` on that line. That half is what the gap's own comma defeats: under
    /// `trailingComma: 'none'` the comma is never re-emitted, and an `is_same_line(item_end,
    /// …)` reading calls a comment glued to it own-line and opens a list that fits
    /// (`docs/comments.md` §Own-line-ness is a SOURCE question).
    ///
    /// Line comments in the same position are detected separately — they always force the
    /// break, wherever on the line they sit.
    pub(crate) fn has_own_line_block_comment_before_closer(&self, start: u32, end: u32) -> bool {
        self.comments_on_page_between(start, end)
            .any(|c| c.is_block && self.block_comment_owns_its_line(c, false))
    }

    /// Check if there's a block comment on its own line within a container.
    ///
    /// A "standalone" block comment starts a line of its own in SOURCE
    /// ([`Printer::comment_follows_content_on_its_line`]) and is not glued to what follows
    /// it, under the caller's [`StandaloneGlue`] rule. Used to force multiline formatting
    /// for objects/type literals.
    ///
    /// The [`StandaloneGlue::Source`] arm is the shared classification
    /// ([`Printer::block_comment_owns_its_line`], which carries the argument), the same one
    /// the bracketed-list gate ([`Printer::has_own_line_block_comments_in_bracket_list`])
    /// and the element→comma seam ask, so the three cannot disagree about one gap. The
    /// [`StandaloneGlue::ItemStart`] arm shares only the leading half — the enum's doc
    /// carries why the type literal parts ways on the other.
    pub(crate) fn has_standalone_block_comment(
        &self,
        container_start: u32,
        container_end: u32,
        item_spans: &[Span],
        glue: StandaloneGlue,
    ) -> bool {
        self.comments_on_page_between(container_start, container_end)
            .any(|c| {
                if !c.is_block {
                    return false; // Line comments handled separately
                }
                match glue {
                    StandaloneGlue::Source => {
                        // `>=` for the same reason the bracket list takes it: an item
                        // glued to the comment starts exactly where it ends.
                        let item_follows = item_spans.iter().any(|s| s.start >= c.span.end);
                        self.block_comment_owns_its_line(c, item_follows)
                    }
                    // The leading half is shared; only the glue half differs. An item
                    // *after* the comment shares its line when the comment's end and the
                    // item's start match (`/* c */ item`); such a comment leads that item
                    // inline and forces nothing. `is_same_line` must take its earlier
                    // position first — the helper returns false for out-of-order args.
                    StandaloneGlue::ItemStart => {
                        !self.comment_follows_content_on_its_line(c)
                            && !item_spans
                                .iter()
                                .any(|s| self.is_same_line(c.span.end, s.start))
                    }
                }
            })
    }

    /// Whether the construct spanning `[span_start, span_end)` ends its doc with a **line
    /// comment** rather than with its `;` — the case where its terminator gap held one and
    /// [`Self::push_semicolon_with_gap_comments`] deferred it past the `;`, onto a line of its
    /// own.
    ///
    /// A caller that trails a same-line comment on this construct has to ask, because the
    /// anchor for "same line" is the `;`'s position in **source** while what the comment
    /// would actually land next to is the doc's **last line**. When the two differ, an
    /// appended block is welded onto the line comment (`// c /* b */`) — the block becomes
    /// text of the comment and is lost.
    ///
    /// Keyed on a **terminator the construct's own doc re-prints**, which is what puts the
    /// deferred comment behind it: the statement / class-member `;`, and a type member's
    /// separator — `;` **or** `,`, since tsv normalizes either to the `;` the member doc
    /// emits ([`Printer::build_comments_around_semicolon_doc`]). Reading only `;` here left
    /// the comma authoring welding (`a: A // c1⏎, // c2` → `a: A; // c1 // c2`, the second
    /// `//` becoming text of the first) at the interface and type-literal walks — the very
    /// merge this predicate exists to prevent. A `}` / element-list `,` is NOT this: there
    /// the separator is re-emitted by the CONTAINER's seam, outside the item's doc, so the
    /// item ends where its source does and its same-line run still trails — and no such
    /// caller asks this question anyway (every call site is a statement or member-body
    /// walk). Keyed on a **line** comment because only a `//` runs to end of line; a
    /// deferred block leaves the line open.
    pub(crate) fn terminator_defers_line_comment(&self, span_start: u32, span_end: u32) -> bool {
        let bytes = self.source.as_bytes();
        if span_end == 0 || !matches!(bytes.get(span_end as usize - 1), Some(b';' | b',')) {
            return false;
        }
        let separator = span_end - 1;
        let Some(last) = comments_to_emit_in_range(self.comments, span_start, separator).last()
        else {
            return false;
        };
        // Only whitespace may sit between the deferred comment and the separator — anything
        // else means the comment is interior to the construct, not in its terminator gap.
        !last.is_block
            && bytes[last.span.end as usize..separator as usize]
                .iter()
                .all(u8::is_ascii_whitespace)
    }

    /// Whether a gap comment LEADS the next printed item rather than trailing the
    /// previous one: the **glued run** it starts reaches `next_start`'s line — code on
    /// both sides of the run binds it forward (prettier's remaining-comment placement),
    /// and per-item line breaks put the whole run on the next item's line, glued
    /// (`a(); /* c */ b();` → `a();⏎/* c */ b();`).
    ///
    /// The reach is a CHAIN, not one `is_same_line` (prettier's `isEndOfLineComment`
    /// extends the comment's end through every comment glued to it before testing for
    /// the newline): only a newline **outside comment bytes** breaks it, so a glued run
    /// whose tail is a multi-line block still leads
    /// (`a(); /* c */ /* x⏎y */ b();` → both lead `b`), while a run whose closing
    /// comment ends its line trails whole (`a(); /* c1 */ /* c2 */⏎b();`). The scan is
    /// the **in-source** axis — an owned comment's bytes glue the chain like any
    /// other's. Erased structure in the gap (a dropped `;`, a member separator) is
    /// invisible to the line test, which is why the CLAIM scan opens at its slot floor
    /// ([`statement_gap_floor`] and kin): a comment bound inside its own slot is
    /// claimed as trailing and stays behind the cursor, so this test is never asked
    /// of it.
    ///
    /// The one statement of the statement/member-gap split, asked from both sides:
    /// [`Self::trailing_claim_end`] bounds the trailing emitter's claim with it, and
    /// [`Self::comment_already_trailed`]'s `leads_target` is its escape on the leading
    /// side — the two must partition the gap, so neither may re-spell it.
    ///
    /// A line comment can never satisfy the test (its own line ends the chain before
    /// anything follows it), so the deferred-`;` machinery never interacts with it. A
    /// multi-line block reads its CLOSING line, so `a(); /* x⏎y */ b();` leads too.
    pub(crate) fn comment_leads_next_item(
        &self,
        comment: &tsv_lang::Comment,
        next_start: u32,
    ) -> bool {
        let mut pos = comment.span.end;
        for c in comments_in_source_range(self.comments, pos, next_start) {
            if !self.is_same_line(pos, c.span.start) {
                return false;
            }
            pos = c.span.end;
        }
        self.is_same_line(pos, next_start)
    }

    /// Where the previous item's same-line trailing claim ENDS in the gap
    /// `[gap_start, next_start)`: the start of the first comment (to emit) that
    /// [`Self::comment_leads_next_item`] hands to the next printed item, or `next_start`
    /// when every comment in the gap ends its own line — the claim may then run the
    /// whole gap.
    ///
    /// The claim stays a PREFIX by construction: comments are disjoint and ordered, so
    /// a later comment's glue chain to `next_start` is a suffix of an earlier one's —
    /// once one comment leads, every later one in the gap leads too.
    ///
    /// `gap_start` is the slot floor, not always the item's end: a comment before a
    /// dropped `EmptyStatement` binds inside its own slot and trails
    /// (`a(); /* c */ ; b();` — prettier reads it the same way), so statement-list
    /// callers open the scan past the last dropped `;`
    /// ([`statement_gap_floor`]); the type-literal
    /// member gap opens it past the member's source separator for the same reason (a
    /// comment before the `;` trails its member however the lines fall).
    ///
    /// Callers pass the result as the trailing emitter's upper bound and clamp their
    /// cursor with it (`find_end_with_trailing_comments(..).min(claim_end)`), so the
    /// leading side finds the handed-over comments still ahead of the cursor.
    pub(crate) fn trailing_claim_end(&self, gap_start: u32, next_start: u32) -> u32 {
        comments_to_emit_in_range(self.comments, gap_start, next_start)
            .find(|c| self.comment_leads_next_item(c, next_start))
            .map_or(next_start, |c| c.span.start)
    }

    /// [`Self::trailing_claim_end`] for `body[index]`'s trailing gap: from the gap's
    /// slot floor ([`statement_gap_floor`]) toward the next printed statement — or
    /// toward `tail_target` when only dropped `;`s (or nothing) follow: the construct
    /// past the list's end that gap comments could still lead (a switch consequent's
    /// next `case` label), `None` where nothing prints there (a body's `}`). With no
    /// target at all nothing leads and the claim is unbounded (`u32::MAX`), so a
    /// caller's own scan bound stands unchanged.
    ///
    /// The one spelling for every statement-list walk (program, block/namespace body,
    /// switch consequent — both its printing and orphan arms) so the trailing claim,
    /// the cursor clamp, and the orphan scan bound cannot drift apart.
    pub(crate) fn statement_claim_end(
        &self,
        body: &[internal::Statement<'_>],
        index: usize,
        tail_target: Option<u32>,
    ) -> u32 {
        match next_printed_stmt(body, index)
            .map(|s| s.span().start)
            .or(tail_target)
        {
            Some(target) => self.trailing_claim_end(statement_gap_floor(body, index), target),
            None => u32::MAX,
        }
    }

    /// The trailing arm of a gap seam, once: emit the item's same-line trailing run and
    /// return it with the advanced cursor.
    ///
    /// The two families differ only in how they DERIVE `upper_bound` and `claim_end`
    /// ([`Self::member_trailing_run`], [`Self::statement_trailing_run`]); what they do with
    /// them is this, and it was spelled twice. Both ends matter and they are not the same
    /// end: the emitted run stops at whichever of the two comes first, while the cursor is
    /// clamped to the **claim split alone** — a handed-over comment must stay ahead of it
    /// for the next item's leading run to find, and `upper_bound` (the next item's start)
    /// would clamp it past nothing at all.
    fn trailing_run(&self, item_end: u32, upper_bound: u32, claim_end: u32) -> (DocBuf, u32) {
        let docs = self.build_trailing_same_line_comment_docs(item_end, upper_bound.min(claim_end));
        let prev_end = self
            .find_end_with_trailing_comments(item_end)
            .min(claim_end);
        (docs, prev_end)
    }

    /// The whole trailing arm of the **member**-gap seam, shared by the class-body and
    /// interface-body walks — the member sibling of [`Self::statement_trailing_run`], and
    /// for the same reason: the two walks held byte-identical copies of it, and the copy is
    /// what let them drift (the interface's re-spelled `is_same_line` filter missed a
    /// multi-line block's closing line, so a comment glued past that `*/` was torn onto its
    /// own line and, on the last member, double-printed by the end-of-body run). An enum
    /// body is NOT one of these: its members are comma-separated, so it takes the
    /// element-comma seam (`collect_trailing_comments` / `push_element_comma_trailing`)
    /// like every other comma list.
    ///
    /// Emits the member's same-line trailing run — bounded at the next member and stopped
    /// at the claim split ([`Self::trailing_claim_end`]) so a comment whose glue chain
    /// reaches that member leads it instead (`a: A; /* c */ b: B;`), emitted by its leading
    /// run — and returns the docs plus the advanced cursor, clamped to the same split so a
    /// handed-over comment stays ahead of it for that leading run to find.
    ///
    /// The caller states the gap's slot FLOOR, a per-family fact rather than a preference
    /// ([`MemberGap`]) — the same role `statement_gap_floor` plays for a statement list.
    pub(crate) fn member_trailing_run(&self, member_end: u32, gap: MemberGap) -> (DocBuf, u32) {
        let (upper_bound, claim_end) = match gap {
            MemberGap::Next { floor, start } => (start, self.trailing_claim_end(floor, start)),
            // Nothing prints after this member, so nothing can be led and the claim runs
            // to the body's end.
            MemberGap::Last { list_end } => (list_end, u32::MAX),
        };
        self.trailing_run(member_end, upper_bound, claim_end)
    }

    /// The whole trailing arm of the statement-gap seam for `body[index]`, shared by
    /// the program and block walks: emit the statement's same-line trailing run —
    /// bounded at the next *printed* statement, skipping dropped `;`s, so a comment
    /// trailing a dropped `;` (`a();; // c`) attaches here rather than being stranded,
    /// and stopped at the claim split ([`Self::statement_claim_end`]) so a comment
    /// whose glue chain reaches the next statement leads it instead
    /// (`a(); /* c */ let b = 1;`), emitted by its leading run. Returns the docs and
    /// the advanced cursor, clamped to the same split so the handed-over comments
    /// stay ahead of it for that leading run to find.
    pub(crate) fn statement_trailing_run(
        &self,
        body: &[internal::Statement<'_>],
        index: usize,
        list_end: u32,
    ) -> (DocBuf, u32) {
        let stmt_end = body[index].span().end;
        let bound = next_printed_stmt_start(body, index, list_end);
        self.trailing_run(stmt_end, bound, self.statement_claim_end(body, index, None))
    }

    /// Whether `comment` was already emitted as the PREVIOUS item's trailing run — it
    /// shares `anchor`'s source line — so this leading / end-of-body run must skip it.
    ///
    /// The single home for a question **eight** emitters used to answer with their own
    /// `is_same_line` call: the two statement-list leading runs (block body, switch
    /// consequent), the class-member, type-literal-member and enum-member leading runs, and
    /// the three end-of-body runs (program, block, the shared
    /// [`Self::build_trailing_body_comments_doc`] — which the object literal now reaches
    /// too). Answering it independently is exactly what let them drift — see the
    /// one-question-one-predicate rule the printer keeps re-learning.
    ///
    /// `anchor` is `None` when there is no previous item at all: nothing trailed, so
    /// nothing is claimed and the run keeps everything.
    ///
    /// ⚠️ **The two families spell `anchor` differently, and the two are EQUIVALENT — do
    /// not "fix" one into the other.** The statement walks (program, block, switch) pass
    /// the previous statement's **span end**; the member walks (class, interface, type
    /// literal) pass the **cursor already advanced past the trailing run**
    /// (`find_end_with_trailing_comments(..).min(claim_end)`). Those hold different values
    /// whenever a multi-line block trailed the item — the cursor then sits on that block's
    /// closing line, a line below the span end — yet the predicate cannot tell them apart,
    /// because the cursor only ever advances to one of two places: **inside** the trailing
    /// run, whose comments are behind it and never reach this filter, or to the **claim
    /// split**, where every comment from there on leads the next item (the claim is a
    /// prefix) and `leads_target` rescues it. A comment that does reach the filter
    /// therefore sits on a later line than *both* anchors. Measured as well as argued: the
    /// two spellings produce byte-identical output over 9196 real files and over the
    /// targeted shapes that provably move the cursor off the item's line.
    ///
    /// `claims_trailing` forces `false`. The previous item deferred a line comment past its
    /// own `;` ([`Self::terminator_defers_line_comment`]) and therefore trailed **nothing**
    /// on that line, which leaves this run the only emitter left for it. Skipping the run
    /// in BOTH places is a dropped comment; trailing it in both is a double-print.
    ///
    /// `leads_target` is the printed item this gap's comments could lead — the escape
    /// matching [`Self::trailing_claim_end`]'s cut on the trailing side: a comment
    /// [`Self::comment_leads_next_item`] hands forward shares the anchor's line yet was
    /// NOT claimed, so it must not be skipped here. Pass `None` where nothing prints at
    /// the range's end (an orphaned/end-of-body run) — there the trailing claim ran the
    /// whole gap and the line test alone is the answer.
    pub(crate) fn comment_already_trailed(
        &self,
        anchor: Option<u32>,
        comment: &tsv_lang::Comment,
        claims_trailing: bool,
        leads_target: Option<u32>,
    ) -> bool {
        !claims_trailing
            && !leads_target.is_some_and(|t| self.comment_leads_next_item(comment, t))
            && anchor.is_some_and(|a| self.is_same_line(a, comment.span.start))
    }

    /// The comments that TRAIL a node ending at `after_pos` on its own output line — the
    /// same-line run in `[after_pos, upper_bound)`.
    ///
    /// The run **follows a multi-line block comment to its closing `*/` line**, so a
    /// comment the author glued past that `*/` trails the node too rather than being torn
    /// onto a line of its own (`a; /* x⏎y */ /* c */`). That is the same `line_ref` walk
    /// [`Self::find_end_with_trailing_comments`] makes, and the two are deliberately on
    /// different axes and must stay that way: this one is **to emit** (an owned comment is
    /// printed by its own node), while the cursor is **in source** (it steps over comment
    /// bytes so a blank-line scan can't read a comment's own newlines as an author's
    /// blank).
    ///
    /// The one statement of "what trails here", for the emitter that renders it
    /// ([`Self::build_trailing_same_line_comment_docs`]) and for the caller that needs the
    /// run as a LIST first — a type-literal member partitions it around its own `;`. A
    /// re-spelled `is_same_line(after_pos, c.span.start)` filter answers the multi-line
    /// case wrong, which is what put the interface and type-literal member walks at odds
    /// with the class body and with prettier.
    pub(crate) fn trailing_same_line_comments(
        &self,
        after_pos: u32,
        upper_bound: u32,
    ) -> CommentVec<'_> {
        let mut line_ref = after_pos;
        let mut run = CommentVec::new();
        for comment in comments_to_emit_in_range(self.comments, after_pos, upper_bound) {
            if !self.is_same_line(line_ref, comment.span.start) {
                break; // Only same-line comments
            }
            // Follow multi-line block comments to their closing line
            if comment.is_block && !self.is_same_line(comment.span.start, comment.span.end) {
                line_ref = comment.span.end;
            }
            run.push(comment);
        }
        run
    }

    /// Build docs for trailing same-line comments after a node
    ///
    /// Line comments are wrapped in `line_suffix` so they don't affect width
    /// calculations for preceding groups (matches Prettier behavior).
    /// Block comments are inline and do affect width.
    ///
    /// Returns a Vec of docs to append to the current parts.
    pub(crate) fn build_trailing_same_line_comment_docs(
        &self,
        after_pos: u32,
        upper_bound: u32,
    ) -> DocBuf {
        let mut docs = DocBuf::new();
        for comment in self.trailing_same_line_comments(after_pos, upper_bound) {
            docs.push(self.build_trailing_comment_doc(comment));
        }
        docs
    }

    /// Build the leading-comment run before `target_start` — the member, statement, or
    /// element the comments lead.
    ///
    /// A thin adapter over the shared leading-comment emitter
    /// ([`Printer::push_leading_comment_run`]), for the callers holding their run as a
    /// slice rather than a range (each applies a per-site filter — dropping the previous
    /// statement's same-line trailing comments, or the ones pulled onto the delimiter
    /// line). The separator after each comment is prettier's `printLeadingComment`
    /// (space / soft `line` / blank-preserving hardline), keyed on the source around
    /// *that* comment.
    ///
    /// Every caller is an already-broken context — an expanded pattern, a multiline
    /// member prefix, a class/interface body, a statement list — so the soft `line`
    /// renders as a break here. That is why this reads as "hardline between own-line
    /// comments" at every site; it is the general rule landing in a broken group, not a
    /// policy of its own. A caller that ever measures this run inside a group that can
    /// *fit* gets prettier's collapse for free, which is the point of routing here.
    ///
    /// Used by: class body members, interface/enum members, block statement bodies,
    /// type literals, expanded object patterns. The orphaned-comment sibling is
    /// [`Self::push_orphaned_comment_run`]. Pushes into the caller's buffer
    /// (usually pooled) rather than returning a fresh `DocBuf` — a long run
    /// would spill the intermediate on every call just to be `extend`ed away.
    pub(crate) fn push_leading_comments_before(
        &self,
        parts: &mut DocBuf,
        comments: &[&internal::Comment],
        target_start: u32,
    ) {
        self.push_leading_comment_run(
            parts,
            comments.iter().copied(),
            target_start,
            LeadingGlue::Adjacent,
            self.d().empty(),
        );
    }

    /// Build the run for comments **orphaned by a dropped statement** — a bare `;`
    /// (`EmptyStatement`) never prints in a body list, so the comments in its gap have
    /// no node to lead. `gap_end` is a source position, not something that will be
    /// emitted.
    ///
    /// So the last comment must not glue to `gap_end`, and takes no separator at all —
    /// the caller's own next emission supplies it. Only an author blank line is recorded
    /// (a bare `literalline`, which the caller's hardline completes), because the
    /// caller's gap check starts later in the source and cannot rediscover it.
    ///
    /// Every *other* comment in the run leads the next comment, which is an ordinary
    /// leading run — so it routes through the shared emitter unchanged, and only the
    /// last comment is special-cased here.
    ///
    /// A sibling of [`Self::push_leading_comments_before`] rather than a flag on it:
    /// what differs is not a separator policy but whether the run has a target at all.
    pub(crate) fn push_orphaned_comment_run(
        &self,
        parts: &mut DocBuf,
        comments: &[&internal::Comment],
        gap_end: u32,
    ) {
        let Some((last, leading)) = comments.split_last() else {
            return;
        };
        self.push_leading_comment_run(
            parts,
            leading.iter().copied(),
            last.span.start,
            LeadingGlue::Adjacent,
            self.d().empty(),
        );
        parts.push(self.build_comment_doc(last));
        if self.has_blank_line_between(last.span.end, gap_end) {
            parts.push(self.d().literalline());
        }
    }

    /// Build docs for trailing comments at the end of a body (before closing `}`).
    ///
    /// Handles comments that appear after the last member/statement in a body,
    /// with blank line preservation between them. Returns a Vec of docs to append.
    ///
    /// The separator is emitted **before** every comment (`hardline`, preceded by a
    /// `literalline` when the author left a blank line) rather than after it. Keying it on
    /// the *following* comment's existence is what makes the rule uniform: the closing `}`
    /// supplies its own break. The mirror-image formulation — emit the separator *after*
    /// each non-last comment — has to ask what KIND the comment was, and answering "a block
    /// needs no break, the `}` follows immediately" welds whatever comes next onto its line
    /// (`/* c1 *//* c2 */`, `/* c1 *///  c2` — the second comment becomes text of the
    /// first).
    ///
    /// The one thing the separator asks is whether the author **glued** this comment to the
    /// previous one ([`Printer::comment_hugs_next`]), in which case the pair keeps the line
    /// it was written on. That is not the weld above — a space keeps both comments distinct,
    /// and a line comment never hugs, so nothing lands behind a `//`. Prettier keeps the
    /// glue here and splits it in an EMPTY body, which is why
    /// [`Self::push_dangling_comment_run`]'s separator stays unconditional.
    ///
    /// Used by every end-of-body run: class body, interface body, enum body, type literal,
    /// namespace body, block-statement bodies (function and bare blocks, via
    /// [`Self::build_block_body_doc`]), the object literal (via
    /// [`Self::build_trailing_closer_comments_doc`], the only caller whose container may
    /// still collapse), and the `}`-less one — the end of the **program**, where `body_end`
    /// is the source length.
    ///
    /// `claims_trailing` says this run owns the comments sharing `prev_end`'s source line.
    /// Normally the last item's trailing emitter took them, so they are skipped here; a
    /// caller whose last item deferred a line comment past its own `;`
    /// ([`Self::terminator_defers_line_comment`]) trailed nothing, so it passes `true` and
    /// this run claims them. Skipping them in BOTH places is a dropped comment — and with
    /// no further item in the list, this emitter is the last chance to print them.
    pub(crate) fn build_trailing_body_comments_doc(
        &self,
        prev_end: u32,
        body_end: u32,
        claims_trailing: bool,
    ) -> DocBuf {
        self.build_trailing_closer_comments_doc(
            prev_end,
            body_end,
            claims_trailing,
            self.d().hardline(),
        )
    }

    /// [`Self::build_trailing_body_comments_doc`] with the run's separator supplied, for
    /// the one container that may still COLLAPSE around its trailing run — the object
    /// literal's inline form, whose separator is a soft `line` its group decides.
    ///
    /// It exists so that container does not keep a SECOND copy of this walk: the copy is
    /// what let the two drift, and the stripped-shell blank scan below then had to be
    /// fixed twice. Every other end-of-body run is already hard-broken, hence the
    /// `hardline` default.
    pub(crate) fn build_trailing_closer_comments_doc(
        &self,
        prev_end: u32,
        body_end: u32,
        claims_trailing: bool,
        separator: DocId,
    ) -> DocBuf {
        let d = self.d();
        let mut docs = DocBuf::new();
        // The blank scan measures a DISTANCE, so it opens past the closers of a stripped
        // paren shell the last item's doc consumed but did not print
        // ([`Self::element_shell_end`]) — an enum member's `A = (⏎1⏎)⏎/* c */` otherwise
        // reads the shell's own line breaks as an author blank and FABRICATES one. Inert
        // for every caller whose `prev_end` is a `;`/`}`: the walk commits only when it
        // actually steps over a `)`, and `body_end` keeps it inside the body.
        //
        // ⚠️ **The `anchor` below keeps the unshifted `prev_end`**, which is the opposite
        // question — "did the last item's own emitter already trail this comment?" — asked
        // of the item's line. Shifting it past the `)` would call a comment on the closer's
        // line already-trailed and DROP it, since this run is the last chance to print one.
        let mut last_pos = self.element_shell_end(prev_end, body_end);
        // `prev_end == 0` is the one caller with no previous item at all: a comments-only
        // file, where the run IS the output. Nothing trailed, so nothing is claimed
        // ([`Self::comment_already_trailed`]'s `None` anchor), and the first comment opens
        // the document rather than breaking away from content above it. A real `{}` body's
        // cursor is at least its `{` plus one, so the two cases can't collide.
        let anchor = (prev_end > 0).then_some(prev_end);
        let mut needs_separator = anchor.is_some();
        // The comment this run emitted last, for the glue question below. `None` after a
        // skipped one: that comment is on the page but not on THIS run's line, so a
        // comment glued to it in source has nothing here to glue to.
        let mut prev_emitted: Option<&internal::Comment> = None;

        for comment in comments_to_emit_in_range(self.comments, prev_end, body_end) {
            if self.comment_already_trailed(anchor, comment, claims_trailing, None) {
                // Already emitted as the last item's trailing run — but the cursor still
                // steps over its BYTES, since the blank-line scan below reads raw source
                // and would otherwise count a multi-line block's own newlines as a blank.
                last_pos = comment.span.end;
                prev_emitted = None;
                continue;
            }
            if needs_separator {
                // A pair the author GLUED onto one line keeps that line
                // ([`Self::trailing_run_hugs_previous`], the run's shared question).
                // The non-glue arm is this walk's own — the separator is the CALLER's, and
                // its blank scan reads the loose newline count from the already-peeled
                // `last_pos` — so it asks the predicate rather than routing through
                // [`Self::push_trailing_run_separator`].
                if self.trailing_run_hugs_previous(prev_emitted, comment.span.start) {
                    docs.push(d.text(" "));
                } else {
                    if self.has_blank_line_between(last_pos, comment.span.start) {
                        docs.push(d.literalline());
                    }
                    docs.push(separator);
                }
            }
            docs.push(self.build_comment_doc(comment));
            last_pos = comment.span.end;
            prev_emitted = Some(comment);
            needs_separator = true;
        }

        docs
    }

    /// Append a **dangling** comment run — the comments alone inside an otherwise empty
    /// delimiter pair, with no node to lead or trail — joined by a `hardline` (prettier's
    /// `printDanglingComments`).
    ///
    /// The separator sits strictly BETWEEN comments: the delimiter pair supplies the break
    /// before the first and after the last. It is emitted before each comment but the
    /// first, never after each comment but the last, for the same reason
    /// [`Self::build_trailing_body_comments_doc`] is — the "after" formulation has to ask
    /// what KIND the comment was, and "a block needs no break, the closer follows
    /// immediately" is false the moment another comment follows, welding the two together
    /// (`/* c1 *//* c2 */`).
    ///
    /// ⚠️ **Unconditional, unlike the trailing run's**, which lets an author-glued pair keep
    /// its line. Prettier draws that line between the two positions: it keeps the glue after
    /// a last item and splits the pair in an EMPTY container (`class A { /* c1 */ /* c2 */
    /// }`), so this emitter has no glue question to ask.
    ///
    /// The caller owns whether the run can stay inline: it picks the open/close separator
    /// (a collapsible `line`/`softline`, or a `hardline` for the always-exploded bodies).
    pub(crate) fn push_dangling_comment_run<'c>(
        &self,
        parts: &mut DocBuf,
        comments: impl IntoIterator<Item = &'c internal::Comment>,
    ) {
        let d = self.d();
        for (i, comment) in comments.into_iter().enumerate() {
            if i > 0 {
                parts.push(d.hardline());
            }
            parts.push(self.build_comment_doc(comment));
        }
    }

    /// Emit a **trailing** comment run: every comment preceded by its separator
    /// ([`Self::push_trailing_run_separator`]), so an author-glued pair keeps its line and
    /// everything else takes the blank-preserving break.
    ///
    /// The third member of the run-emitter family, beside
    /// [`Self::push_leading_comment_run`] and [`Self::push_dangling_comment_run`] — it is
    /// the *separator-before-each* rule, and the `(prev_pos, prev_comment)` bookkeeping it
    /// carries is exactly what every hand-rolled copy of this walk gets to restate. `scan_from`
    /// is the caller's cursor, which bounds the first comment's blank scan.
    pub(crate) fn push_trailing_comment_run<'c>(
        &self,
        parts: &mut DocBuf,
        comments: impl IntoIterator<Item = &'c internal::Comment>,
        scan_from: u32,
    ) {
        let mut prev_pos = scan_from;
        let mut prev_comment: Option<&internal::Comment> = None;
        for comment in comments {
            self.push_trailing_run_separator(parts, prev_comment, prev_pos, comment.span.start);
            parts.push(self.build_comment_doc(comment));
            prev_pos = comment.span.end;
            prev_comment = Some(comment);
        }
    }

    /// Compute the "delimiter-line prefix" for the open-delimiter trailing-comment
    /// divergence (object literals, array literals, and block bodies).
    ///
    /// A comment on the same source line as the opening delimiter at `delim_pos`
    /// is kept on that line — instead of being relocated to its own line as the
    /// first element's leading comment (prettier's behavior). Returns the emitted
    /// prefix docs (` /* c */` / ` // c`, leading-space convention) and, when the
    /// pull fired, `Some(delim_pos)` — the position the caller passes back to
    /// exclude those same-line comments from the first element's leading set
    /// (`None` when nothing was pulled, so the prefix is empty).
    ///
    /// Gated on `should_force_expansion_for_comments`, so an inline block comment
    /// hugging the first element (`{ /* c */ a: 1 }`, `[/* c */ x]`) is left in
    /// place and the result is `(empty, None)`. See conformance_prettier_ts_comments.md
    /// §Comment relocation.
    ///
    /// The call family asks the same question about its `(` and reaches the same rule
    /// from the other side — see `docs/comments.md` §The delimiter-line question. Its
    /// force-expanded builders spell this predicate identically
    /// (`PartitionedComments::has_trailing_comments` conjoined with
    /// `should_force_expansion_for_comments`); its collapse-capable path
    /// (`emit_first_arg_leading_comments`) asks the narrower
    /// `PartitionedComments::has_trailing_line` instead, which is load-bearing, not drift.
    pub(in crate::printer) fn delimiter_line_comment_prefix(
        &self,
        delim_pos: u32,
        first_elem_start: u32,
    ) -> (DocBuf, Option<u32>) {
        self.delimiter_line_comment_prefix_impl(delim_pos, first_elem_start, false)
    }

    /// Object-literal variant of `delimiter_line_comment_prefix` that *also* pulls
    /// a block comment sharing the opening `{` line onto that line when the first
    /// property is on a later line (the object spans multiple lines). An object
    /// literal preserves its authored multi-line-ness, so a source newline before
    /// the first property means it will break, and the block trails `{` (like a
    /// line comment does) instead of dropping to the property's leading line.
    /// Collapsing containers (arrays, arg lists) keep the base behavior and call
    /// the plain form. The caller must treat a fired pull as forcing must-break
    /// (the prefix is only emitted on the break path).
    pub(in crate::printer) fn delimiter_line_comment_prefix_object(
        &self,
        delim_pos: u32,
        first_elem_start: u32,
    ) -> (DocBuf, Option<u32>) {
        self.delimiter_line_comment_prefix_impl(delim_pos, first_elem_start, true)
    }

    /// Assemble a computed `[…]` / `?.[…]` (or mapped-type `[K in T]`) that must BREAK
    /// because a line comment sits in the open→body gap: pull a `[`-line comment onto the
    /// open line (own-line ones keep their line, blank-preserving), emit `body`, and drop
    /// `]`. `body` is pre-built by the caller (key/index/interior plus any body→`]`
    /// trailing comments, per each printer's own rule), so this owns only the shared
    /// shell. `open` is the emitted bracket text (`[` or `?.[`); `bracket_char` is the
    /// source position of the `[` glyph (the scan/pull anchor), decoupled from `open` for
    /// the `?.[` form (`bracket_char + 1` is the first inside-bracket position). Shared by
    /// the computed-key, computed-member-access, and mapped-type break paths.
    pub(in crate::printer) fn build_bracket_line_comment_break(
        &self,
        open: &'static str,
        bracket_char: u32,
        body_start: u32,
        body: DocId,
    ) -> DocId {
        let d = self.d();
        let (line_prefix, pull_pos) = self.delimiter_line_comment_prefix(bracket_char, body_start);
        let mut inner =
            self.build_leading_comments_multiline(bracket_char + 1, body_start, pull_pos);
        inner.push(body);
        d.group_break(d.concat(&[
            d.text(open),
            d.concat(&line_prefix),
            d.indent_softline(d.concat(&inner)),
            d.softline(),
            d.text("]"),
        ]))
    }

    fn delimiter_line_comment_prefix_impl(
        &self,
        delim_pos: u32,
        first_elem_start: u32,
        pull_expanding_block: bool,
    ) -> (DocBuf, Option<u32>) {
        let pc = super::calls::PartitionedComments::new(
            self.comments,
            self.comment_line_breaks,
            delim_pos,
            first_elem_start,
        );
        // The base rule gates the pull on forced expansion (a line comment, or a
        // block standalone on its own line) — the call family's own predicate, so the
        // two families cannot drift. `pull_expanding_block` adds the object case: a
        // block on the delimiter line with the first element on a later line — the
        // object will break, so the block trails the `{`. (Its `has_trailing_block`
        // implies the base rule's `has_trailing_comments`, which is why the added arm
        // sits beside the predicate rather than inside its conjunction.)
        let pull = pc.pulls_to_delimiter_line(self)
            || (pull_expanding_block
                && pc.has_trailing_block()
                && !self.is_same_line(delim_pos, first_elem_start));
        let mut prefix = DocBuf::new();
        if pull {
            // The run, plus the author blank below it — one emitter for every
            // delimiter-line pull in both families.
            pc.emit_delimiter_line_pull(&mut prefix, self);
        }
        (prefix, pull.then_some(delim_pos))
    }

    /// Whether `comment` was pulled onto the opening delimiter's line by
    /// `delimiter_line_comment_prefix` — i.e. it shares a source line with the
    /// delimiter at `delim_pos`.
    ///
    /// The prefix helper emits these comments on the delimiter's line; every
    /// consumer must then drop the same comments from the first element's
    /// leading-comment set so they aren't emitted twice. Centralizing the test
    /// keeps that exclusion in lockstep with what the prefix actually pulls.
    pub(in crate::printer) fn comment_on_delimiter_line(
        &self,
        delim_pos: u32,
        comment: &internal::Comment,
    ) -> bool {
        self.is_same_line(delim_pos, comment.span.start)
    }

    /// A first element/member's leading comments with the delimiter-line
    /// comments removed.
    ///
    /// `delimiter_line_comment_prefix` emits the comments sharing the opening
    /// delimiter's line as a prefix on that line, so every member-loop consumer
    /// must drop the same comments from the first element's leading set to avoid
    /// emitting them twice (see `comment_on_delimiter_line`). Returns `comments`
    /// unchanged when `delimiter_pull_pos` is `None` (nothing was pulled).
    pub(in crate::printer) fn first_member_leading_comments<'c>(
        &self,
        comments: CommentVec<'c>,
        delimiter_pull_pos: Option<u32>,
    ) -> CommentVec<'c> {
        match delimiter_pull_pos {
            Some(dpos) => comments
                .into_iter()
                .filter(|c| !self.comment_on_delimiter_line(dpos, c))
                .collect(),
            None => comments,
        }
    }

    /// A list item's **leading** run: the comments in `[gap_start, item_start)`, minus any
    /// the opening delimiter pulled onto its own line ([`Self::first_member_leading_comments`]).
    ///
    /// ⚠️ `gap_start` is where the PREVIOUS item's **trailing run ended**
    /// (`TrailingComments::end_pos`) — never a position past the separator. The two runs
    /// partition one gap, so a scan that starts past the comma leaves the comments the
    /// author wrote on the other side of it (`a: 1⏎// c⏎, b`) with no emitter at all; that
    /// was a live DROP at four sites. The parameter is named for the cursor it wants.
    /// See [docs/comments.md](../../../../../docs/comments.md) §The element-comma seam.
    ///
    /// `delimiter_pull` is `Some(pos)` **only for the first item** — a later item's gap can
    /// still be on the delimiter's line (a one-line list), and dropping there would delete
    /// a comment nothing else prints. Callers pass `if is_first { pull } else { None }`.
    pub(in crate::printer) fn collect_item_leading_comments(
        &self,
        gap_start: u32,
        item_start: u32,
        delimiter_pull: Option<u32>,
    ) -> CommentVec<'_> {
        let comments: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, gap_start, item_start).collect();
        self.first_member_leading_comments(comments, delimiter_pull)
    }

    /// Build a line_suffix doc for all comments between two positions
    ///
    /// Used for trailing comments on call arguments, where comments should stay
    /// on the same line but not affect width calculations for breaking decisions.
    /// Returns None if no comments exist in the range.
    ///
    /// Example: `fn(arg // comment)` - the comment becomes a line_suffix
    pub(crate) fn build_trailing_comments_line_suffix(
        &self,
        start: u32,
        end: u32,
    ) -> Option<DocId> {
        let d = self.d();
        let mut in_range = comments_to_emit_in_range(self.comments, start, end).peekable();
        in_range.peek()?;

        let mut parts = DocBuf::new();
        for comment in in_range {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }

        Some(d.line_suffix(d.concat(&parts)))
    }

    /// Build a Doc for an empty body (`{}`) that may contain comments.
    ///
    /// If comments exist between the braces, formats as:
    /// ```text
    /// {
    ///     // comment
    /// }
    /// ```
    ///
    /// If no comments, returns `{}`.
    ///
    /// Always breaks when a comment is present — used by the containers prettier
    /// keeps exploded (class body, interface body, namespace body). The
    /// containers that keep a fitting block comment inline (object literals and
    /// patterns, enum bodies, type literals) pass a collapsible `sep` to
    /// [`Self::build_empty_bracketed_with_comments_doc`] instead; "always breaks" is
    /// nothing more than that same emitter with a `hardline` separator, so both live on
    /// one dangling-comment rule.
    pub(crate) fn build_empty_body_with_comments_doc(&self, body_span: Span) -> DocId {
        let d = self.d();
        self.build_empty_bracketed_with_comments_doc(
            body_span.start,
            body_span.end,
            d.text("{"),
            "}",
            d.hardline(),
        )
    }

    /// Build a Doc for an empty `{}` body whose only content is a dangling
    /// comment, keeping a fitting block comment inline with bracket spacing
    /// (`{ /* c */ }`).
    ///
    /// tsv applies bracket spacing uniformly: object literals, destructuring
    /// patterns, enum bodies, and type literals all print a comment-only empty
    /// body as `{ /* c */ }`. Prettier tightens every one of these to
    /// `{/* c */}`, so this is a divergence — see conformance_prettier_ts.md
    /// §Empty-object comment bracket spacing. A truly empty `{}` (no comment)
    /// has no content to space and stays tight in both. See
    /// [`Self::build_empty_inline_with_comments_doc`].
    pub(crate) fn build_empty_braces_inline_with_comments_doc(&self, body_span: Span) -> DocId {
        let d = self.d();
        let sep = d.line();
        self.build_empty_inline_with_comments_doc(body_span.start, body_span.end, "{}", sep)
    }

    /// Build a Doc for an empty bracket `[]` body whose only content is a
    /// dangling comment, keeping a fitting block comment inline (`[/* c */]`).
    ///
    /// Used by array literals/patterns and tuple types. See
    /// [`Self::build_empty_inline_with_comments_doc`].
    pub(crate) fn build_empty_brackets_inline_with_comments_doc(&self, span: Span) -> DocId {
        self.build_empty_brackets_inline_with_comments_doc_range(span.start, span.end)
    }

    /// Build a Doc for an empty bracket `[]` body with explicit bounds (e.g. an
    /// array pattern with a type annotation). See
    /// [`Self::build_empty_brackets_inline_with_comments_doc`].
    pub(crate) fn build_empty_brackets_inline_with_comments_doc_range(
        &self,
        body_start: u32,
        body_end: u32,
    ) -> DocId {
        let d = self.d();
        let sep = d.softline();
        self.build_empty_inline_with_comments_doc(body_start, body_end, "[]", sep)
    }

    /// Build a Doc for an empty paren list whose only content is a dangling
    /// comment, keeping a fitting block comment inline (`fn(/* c */)`).
    ///
    /// The paren counterpart of [`Self::build_empty_brackets_inline_with_comments_doc`],
    /// shared by every empty paren list: call and `new` arguments (including the
    /// member-chain and optional-call `?.(` forms, hence the `opening` prefix),
    /// value parameter lists (function, method, arrow), and signature-level type
    /// params. A line comment inside `()` cannot stay inline — `//` runs to the end
    /// of the line and would swallow the `)` — so it forces the break; this is the
    /// one delimiter pair where inlining is a correctness bug rather than a layout
    /// choice.
    ///
    /// The sibling swallow in CALLEE position — a line comment between the callee and
    /// its `(` (`call // c⏎()`, and the optional-call `call?. // c⏎()`) — is a different
    /// mechanism (callee-position trivia, not a dangling comment inside a delimiter
    /// pair) and is handled by this emitter's caller, `push_empty_args`, which drops the
    /// whole list to an indented continuation line.
    ///
    /// `paren_open` is the `(` position and `paren_close_after` the position past
    /// the `)` (as returned by `find_closing_paren`).
    pub(crate) fn build_empty_parens_inline_with_comments_doc(
        &self,
        paren_open: u32,
        paren_close_after: u32,
        opening: &'static str,
    ) -> DocId {
        let d = self.d();
        let sep = d.softline();
        self.build_empty_bracketed_with_comments_doc(
            paren_open,
            paren_close_after,
            d.text(opening),
            ")",
            sep,
        )
    }

    /// Build a Doc for an empty parameter list, preserving any dangling comments
    /// inside the parens (`fn(/* c */)`). Shared by every empty parameter list —
    /// value params (function, method, arrow) and signature-level type params — so
    /// the dangling rule of
    /// [`Self::build_empty_parens_inline_with_comments_doc`] reaches all of them.
    ///
    /// `search_limit` bounds the depth-tracked `)` search, which skips comment and
    /// string content so a `)` inside a comment can't be mistaken for the closer.
    /// Callers that know a tighter bound (an arrow's body start) pass it; the rest
    /// pass the source length. Yields a bare `()` when there is no `(` to anchor to.
    pub(crate) fn build_empty_params_with_comments_doc(
        &self,
        params_start: Option<u32>,
        search_limit: u32,
    ) -> DocId {
        if let Some(open) = params_start
            && let Some(close_after) = self.find_closing_paren(open, search_limit)
        {
            return self.build_empty_parens_inline_with_comments_doc(open, close_after, "(");
        }
        self.d().text("()")
    }

    /// Build a Doc for an empty delimited container whose only content is a
    /// dangling comment, matching prettier 3.9's `printDanglingCommentsInList`
    /// (prettier PRs #18617 / #18615): a block comment that fits stays inline
    /// (`[/* c */]`, `{/* c */}`); a line comment can't be inlined and forces
    /// the break, and an overflowing or multi-line block comment breaks via the
    /// enclosing group. `sep` is the open/close separator — `softline` (no
    /// space) for brackets, object literals/patterns, and enum bodies, `line`
    /// (bracket spacing) for type literals.
    ///
    /// Containers that always break with a dangling comment (class, interface,
    /// and namespace bodies) reach the same emitter through
    /// [`Self::build_empty_body_with_comments_doc`], with `sep` a `hardline`.
    ///
    /// The empty import-attribute clause (`with {}`) calls it directly, from
    /// `statements/modules/import_attributes.rs` — hence `pub(in crate::printer)`
    /// rather than module-private.
    pub(in crate::printer) fn build_empty_inline_with_comments_doc(
        &self,
        span_start: u32,
        span_end: u32,
        pair: &'static str,
        sep: DocId,
    ) -> DocId {
        let opening = self.d().text(&pair[..1]);
        self.build_empty_bracketed_with_comments_doc(span_start, span_end, opening, &pair[1..], sep)
    }

    /// Like [`Self::build_empty_inline_with_comments_doc`] but with an arbitrary
    /// `opening` doc (which may carry a prefix, e.g. a parenthesized-intersection
    /// `(A & {`) and a static `closing` string (`}`, `]`, `)`). The empty body
    /// stays delimiter-tight when comment-free (`{}` not `{ }`), so a union-member
    /// or paren-intersection object type that reaches the alignment path prints
    /// with no spurious bracket space and preserves any interior comment.
    pub(crate) fn build_empty_bracketed_with_comments_doc(
        &self,
        span_start: u32,
        span_end: u32,
        opening: DocId,
        closing: &'static str,
        sep: DocId,
    ) -> DocId {
        let d = self.d();
        let body_start = span_start + 1; // After opening delimiter
        let body_end = span_end.saturating_sub(1); // Before closing delimiter

        let comments: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, body_start, body_end).collect();

        if comments.is_empty() {
            return d.concat(&[opening, d.text(closing)]);
        }

        let mut comment_parts = DocBuf::new();
        self.push_dangling_comment_run(&mut comment_parts, comments.iter().copied());

        // A line comment can't be inlined, so it forces the break; a fitting
        // block comment stays inline (the group breaks on overflow / a multi-line
        // block comment's own hardlines).
        let has_line = comments.iter().any(|c| !c.is_block);
        let close_sep = if has_line { d.hardline() } else { sep };

        d.group(d.concat(&[
            opening,
            d.indent(d.concat(&[sep, d.concat(&comment_parts)])),
            close_sep,
            d.text(closing),
        ]))
    }

    /// Append the comments between a signature's last content token and the
    /// member's end (typically right before the printed `;`): after the return
    /// type, or after the params' closing `)` when there is no return type.
    /// Same-line comments stay with the member (a block inline, a line via
    /// `line_suffix`); an **own-line** comment is deferred to `deferred` (own line,
    /// blank preserved) for the caller to emit **after** the `;`, matching prettier
    /// (the member doc doesn't own the `;`). `deferred` is empty on the common
    /// no-comment path.
    ///
    /// Shared by method/call/construct signatures in interfaces and type literals
    /// and by declare functions (all use the type-member `;` binding —
    /// `split_member_terminator_gap_comments`).
    pub(crate) fn append_signature_end_comments(
        &self,
        parts: &mut DocBuf,
        return_type: Option<&internal::TSTypeAnnotation<'_>>,
        paren_pos: Option<u32>,
        span_end: u32,
        deferred: &mut DocBuf,
    ) {
        let content_end = return_type.map_or_else(
            || {
                paren_pos
                    .and_then(|p| self.find_closing_paren(p, span_end))
                    .unwrap_or(span_end)
            },
            |rt| rt.span.end,
        );
        deferred.extend(self.split_member_terminator_gap_comments(parts, content_end, span_end));
    }

    /// Partition the comments in a content→separator gap `[start, sep_pos)`, binding
    /// the separator (`,` / `;`) to the content the way prettier does:
    ///
    /// - a **same-line** comment is pushed to `parts` (before the separator) — a block
    ///   inline (`X /* c */<sep>`, preserved), a line via `line_suffix` (zero width, so
    ///   it floats past the separator to the next hardline → `X<sep> // c`) — *except*
    ///   that when `block_after_separator` is set a same-line **block** is instead
    ///   *returned* (deferred), so it trails **after** the separator (`X<sep> /* c */`);
    /// - an **own-line** comment is *returned* (not pushed), each on its own line
    ///   (`hardline` + comment), for the caller to emit **after** the separator so the
    ///   author's line break is kept and a `//` can't swallow the separator; when
    ///   `block_after_separator` (the `;`-terminator case), a single blank line before it
    ///   (relative to the content, then the previous comment) is also preserved
    ///   (`literalline`), matching prettier — the `,`-separator case keeps no blank
    ///   (prettier emits none in a list element→comma gap).
    ///
    /// `block_after_separator` is the prettier-3.9 behavior for the statement/member
    /// **`;` terminator** (the `;` is pure structure, so trailing a block past it is
    /// lossless — `expr; /* c */`); the list **`,` separator** passes `false` and keeps
    /// a same-line block before the comma (`X /* c */,`) — prettier did not change that.
    ///
    /// Caller idiom: `let after = self.split_comma_gap_comments(parts, elem_end,
    /// comma_pos); parts.push(","); parts.extend(after);`. Emitting an own-line comment
    /// *before* the separator would put the separator on the comment's line — a `//`
    /// swallows it (content loss), a block just diverges from prettier.
    ///
    /// The **comma's** binding of both axes: a same-line block stays before it, and a
    /// blank line above a deferred own-line comment does NOT survive it. Neither is a
    /// caller preference — the `;` terminators state their own through
    /// [`Self::push_semicolon_with_gap_comments`] (blank preserved at either block
    /// binding, since a `;` ends its line) and the type-member one through
    /// [`Self::split_member_terminator_gap_comments`].
    pub(crate) fn split_comma_gap_comments(
        &self,
        parts: &mut DocBuf,
        start: u32,
        sep_pos: u32,
    ) -> DocBuf {
        self.push_gap_comments(parts, start, sep_pos, false, false, self.d().hardline())
    }

    /// The **for-header `;`** variant of
    /// [`split_comma_gap_comments`](Self::split_comma_gap_comments): the same
    /// binding (a same-line block stays before the `;`, no blank preserved), but the
    /// break onto the deferred run's **first** line is the header's own `line`.
    ///
    /// A statement or member terminator ends its line outright, so a `hardline` there
    /// states a fact about the construct. A `for` header does not: it is a group that
    /// decides its own width, and prettier reaches this run through the very `line` that
    /// follows the `;` in `printForStatement` — so a `hardline` answers a width question
    /// with a comment answer and forces open a header prettier keeps flat
    /// (`docs/comments.md` §Own-line-ness is a SOURCE question). Only the FIRST break is
    /// the caller's; every break *within* the run stays the shared site's, so two
    /// comments the author gave two lines still take two.
    ///
    /// A `//` in this gap is still safe from the flat rendering — it would swallow the
    /// clauses behind it — because any line comment inside the parens forces the header
    /// open through `build_for_header_doc`'s own `has_line_comment_in_header`, which
    /// covers the whole paren interior and so strictly contains this gap.
    pub(crate) fn split_for_header_gap_comments(
        &self,
        parts: &mut DocBuf,
        start: u32,
        sep_pos: u32,
    ) -> DocBuf {
        self.push_gap_comments(parts, start, sep_pos, false, false, self.d().line())
    }

    /// The gap-split caller idiom for a **`;` terminator**, in one call: the gap's pre-`;` comments, the `;`,
    /// then the comments that belong after it.
    ///
    /// The ordering is the reason this exists rather than three lines at each site: the
    /// returned docs must be pushed *after* the `;` text, and a site that inlines the
    /// idiom can invert it silently — an own-line comment emitted before the separator
    /// puts the `;` on the comment's line, where a `//` swallows it outright.
    ///
    /// `block_after_separator` is the terminator's own axis, not a caller preference:
    /// `true` for a statement/member `;` (prettier trails a same-line block past it —
    /// `expr; /* c */`), `false` where the operand keeps it (`import =` / `export =` /
    /// `export as namespace`, the ambient module shorthand). A caller whose split is
    /// *conditional* keeps the raw idiom.
    ///
    /// ⚠️ **The blank rule is the SEPARATOR's, not that axis's.** A `;` terminator ends
    /// its line, so an author blank above a deferred own-line comment survives it in
    /// prettier — at *both* block bindings, since where the same-line block sits says
    /// nothing about what an own-line comment two lines down is worth. Reading the blank
    /// off `block_after_separator` — which is the **comma's** rule
    /// ([`Self::split_comma_gap_comments`]), not a `;`'s — silently dropped it at every
    /// `false` site;
    /// this passes `preserve_blank` unconditionally, which is also what makes this and
    /// [`Self::split_member_terminator_gap_comments`] one answer rather than two.
    ///
    /// The gap's far end is derived from `span_end` — the node's own end — rather than
    /// taken from the caller, because "where is my `;`" has one answer and every
    /// terminator here is **optional under ASI**. Where the author wrote one the span
    /// runs through it, so the `;` is the byte before `span_end`; where ASI ended the
    /// statement the span stops at its content, and the subtraction every caller used to
    /// spell inline lands *inside* that content instead — the assumed-delimiter-position
    /// hazard, harmless only because the resulting range happens to be empty. Stating it
    /// once keeps it that way by construction.
    pub(crate) fn push_semicolon_with_gap_comments(
        &self,
        parts: &mut DocBuf,
        content_end: u32,
        span_end: u32,
        block_after_separator: bool,
    ) {
        let semicolon_pos = if span_end > content_end {
            span_end - 1
        } else {
            content_end
        };
        let after = self.push_gap_comments(
            parts,
            content_end,
            semicolon_pos,
            block_after_separator,
            true,
            self.d().hardline(),
        );
        parts.push(self.d().text(";"));
        parts.extend(after);
    }

    /// The **type-member `;`** variant of the gap split: a same-line
    /// block stays *before* the `;` (`a: A /* c */;`, like a list separator) **but** a
    /// blank line before an own-line comment IS preserved (like a statement terminator).
    /// This mixed binding is what prettier does for a type-literal / interface member
    /// terminator, which neither of its two siblings' bindings expresses. Same
    /// caller idiom (the returned own-line docs are emitted by the type-element *joiner*
    /// after its `;`, since the member doc doesn't own the `;`).
    pub(crate) fn split_member_terminator_gap_comments(
        &self,
        parts: &mut DocBuf,
        start: u32,
        sep_pos: u32,
    ) -> DocBuf {
        self.push_gap_comments(parts, start, sep_pos, false, true, self.d().hardline())
    }

    /// Where a separator gap's ANCHOR-LINE run ENDS — the split
    /// [`Self::push_gap_comments`] partitions on, and the one every caller that has to
    /// reason about the run it produced must re-ask.
    ///
    /// Stated as a POSITION rather than a per-comment predicate, the way the array
    /// literal's element seam states its own ([`Printer::element_gap_split`]): the run is a
    /// **prefix** of the gap by construction — positions increase, so once a comment opens a
    /// later line every comment behind it does too — and one split shared by its three
    /// readers (the partition itself, the deferred run's end, the blank-line scan's
    /// terminal) cannot drift into handing a caller a run the emitter never produced.
    ///
    /// The run **follows a multi-line block to its closing `*/` line**, the same walk
    /// [`Self::trailing_same_line_comments`] and the element seam make: a comment the author
    /// glued past that `*/` trails the gap the block trails (`a: A /* x⏎y */ /* c */;`)
    /// rather than reading as own-line and deferring past the separator. Asking a bare
    /// `is_same_line(anchor, …)` instead put every gap that routes here — the type-member
    /// and class-member terminators, a statement's own `;`, and the comma seam's parameter
    /// lists — at odds with the array literal and with prettier, and the resulting form is
    /// not even a fixed point (the reprint has the comment leading the next item, where its
    /// own line reads as authored).
    ///
    /// The walk is the **in-source** axis: an owned comment's bytes sit on the page and
    /// carry the line reference like any other's, exactly as
    /// [`Self::comment_leads_next_item`]'s glue chain does.
    ///
    /// ⚠️ **Scoped to the region BEFORE the comma.** Past the comma the anchor is the
    /// comma itself ([`Printer::comment_on_comma_line`]) — the author may have pushed the
    /// separator onto a line of its own, and the printer pulls it back, so an
    /// element-anchored reading there re-binds the comment to the next item.
    ///
    /// ⚠️ **An anchor reading, deliberately** — not the source reading
    /// ([`Printer::comment_follows_content_on_its_line`]) the element→comma SEAM asks.
    /// This gap's anchor is the element's own end and the question is which side of the
    /// separator the comment renders on, not which element it binds to.
    pub(in crate::printer) fn gap_anchor_line_end(&self, anchor: u32, upper_bound: u32) -> u32 {
        let mut line_ref = anchor;
        let mut end = anchor;
        for comment in comments_in_source_range(self.comments, anchor, upper_bound) {
            if !self.is_same_line(line_ref, comment.span.start) {
                break;
            }
            // Follow multi-line block comments to their closing line
            if comment.is_block && !self.is_same_line(comment.span.start, comment.span.end) {
                line_ref = comment.span.end;
            }
            end = comment.span.end;
        }
        end
    }

    /// Core of the gap-comment partition, with the two policy axes decoupled:
    /// `block_after` moves a **same-line block** past the separator (deferred), and
    /// `preserve_blank` keeps a single blank line before a deferred **own-line** comment
    /// (`literalline`). A same-line line comment always uses `line_suffix` (zero width,
    /// floats past the separator); an own-line comment is deferred on its own
    /// `hardline`. `prev` tracks the content/prior-comment end for blank detection.
    ///
    /// ⚠️ **"Own-line" is per comment, so the deferred run asks the glue question**
    /// ([`Self::trailing_run_hugs_previous`]) like every other trailing run — a bare
    /// `hardline` between two comments the author wrote on ONE line splits the pair
    /// (`docs/comments.md` §Trailing and dangling runs: the separator is one question
    /// wherever a run is emitted). This gap is a run at five constructs at once — a
    /// statement's own `;`, a class member's, a type member's, a `for` head's, a
    /// declarator's — so the drift was five divergences from one line. The site keeps
    /// its own non-glue arm because its blank rule is the caller's (`preserve_blank`),
    /// which is the sanctioned shape for asking the predicate directly.
    ///
    /// The hug can only ever bind two comments the anchor-line split already put on the
    /// same side: a deferred comment sharing a source line with an anchor-line one is
    /// impossible, since [`Self::gap_anchor_line_end`] follows that line (a multi-line
    /// block included) to its end.
    ///
    /// `first_break` is the third axis, and the narrowest: the break onto the deferred
    /// run's **first** line, which is the one break this gap does not own — it belongs to
    /// whatever precedes the separator. A statement or member terminator ends its line
    /// outright and passes `hardline`; a `for` header is a group that decides its own
    /// width and passes its own `line`
    /// ([`Self::split_for_header_gap_comments`]). Every break *within* the run is this
    /// site's and stays a `hardline`, so the caller can only move the run's first line,
    /// never merge two lines the author wrote.
    fn push_gap_comments(
        &self,
        parts: &mut DocBuf,
        start: u32,
        sep_pos: u32,
        block_after: bool,
        preserve_blank: bool,
        first_break: DocId,
    ) -> DocBuf {
        let d = self.d();
        let mut deferred = DocBuf::new();
        let mut prev = start;
        let mut gap = comments_to_emit_in_range(self.comments, start, sep_pos).peekable();
        // Zero-comment fast gate: the split is a search of its own, and every `;`-gap
        // caller (a `const`'s terminator, a `for` head) asks on documents that mostly have
        // no comment here at all. Safe to skip when the to-emit range is empty even though
        // the split reads the IN-SOURCE axis: an all-owned gap classifies nothing, so the
        // value would go unused.
        let anchor_line_end = if gap.peek().is_some() {
            self.gap_anchor_line_end(start, sep_pos)
        } else {
            start
        };
        let mut prev_comment: Option<&internal::Comment> = None;
        // Whether the deferred run has been opened — the `first_break` axis is about the
        // break INTO the run, so it is spent once and never on a later comment. Tracked
        // rather than read off `prev_comment`, which the anchor-line arm also sets, or off
        // `deferred`, which `block_after` may already have pushed a trailing block into.
        let mut deferred_open = false;
        for comment in gap {
            if comment.span.start < anchor_line_end {
                if block_after && comment.is_block {
                    deferred.push(self.build_trailing_comment_doc(comment));
                } else {
                    parts.push(self.build_trailing_comment_doc(comment));
                }
            } else {
                // The separator, BEFORE the comment — a space where the author glued the
                // pair, otherwise a break: the caller's onto the run's first line, this
                // site's own after that (the caller owns the blank rule).
                if self.trailing_run_hugs_previous(prev_comment, comment.span.start) {
                    deferred.push(d.text(" "));
                } else {
                    if preserve_blank && self.has_blank_line_between(prev, comment.span.start) {
                        deferred.push(d.literalline());
                    }
                    deferred.push(if deferred_open {
                        d.hardline()
                    } else {
                        first_break
                    });
                }
                deferred_open = true;
                deferred.push(self.build_comment_doc(comment));
            }
            prev = comment.span.end;
            prev_comment = Some(comment);
        }
        deferred
    }

    /// Append leading inline block comments (`/*content*/ ` format) between two positions.
    ///
    /// Only emits block comments; line comments are skipped (they would have been
    /// detected earlier and routed to the multiline path). Counterpart of
    /// [`Self::append_trailing_inline_block_comments`].
    pub(crate) fn append_leading_inline_block_comments(
        &self,
        parts: &mut DocBuf,
        start: u32,
        end: u32,
    ) {
        let d = self.d();
        for comment in comments_to_emit_in_range(self.comments, start, end) {
            if comment.is_block {
                // One text node (`/*content*/ `) — callers may pass `parts` as
                // fill items, so the space can't split into its own node. The
                // full span is the verbatim `/*content*/` (delimiters included).
                let mut w = d.pool_writer();
                w.push_str(comment.span.extract(self.source));
                w.push(' ');
                let doc = w.finish_text();
                // A comment emission that can't route through `build_comment_doc` (the
                // trailing space must share the node), so it tags its own ledger node.
                #[cfg(feature = "comment_check")]
                d.tag_comment_doc(doc, comment.span, self.source);
                parts.push(doc);
            }
        }
    }

    /// Append trailing inline block comments (` /*content*/` format) between two positions.
    ///
    /// Only emits block comments; line comments are skipped (they would have been
    /// detected earlier and routed to the multiline path).
    pub(crate) fn append_trailing_inline_block_comments(
        &self,
        parts: &mut DocBuf,
        start: u32,
        end: u32,
    ) {
        let d = self.d();
        for comment in comments_to_emit_in_range(self.comments, start, end) {
            if comment.is_block {
                // One text node (` /*content*/`) — callers may pass `parts` as
                // fill items, so the space can't split into its own node. The
                // full span is the verbatim `/*content*/` (delimiters included).
                let mut w = d.pool_writer();
                w.push(' ');
                w.push_str(comment.span.extract(self.source));
                let doc = w.finish_text();
                // A comment emission that can't route through `build_comment_doc` (the
                // leading space must share the node), so it tags its own ledger node.
                #[cfg(feature = "comment_check")]
                d.tag_comment_doc(doc, comment.span, self.source);
                parts.push(doc);
            }
        }
    }

    /// Split the last list-member's trailing inline block comments around a source
    /// comma (in `elem_end..end_boundary`): comments before the comma go to `before`,
    /// comments after it to `after`. Callers emit `after` past where the comma was
    /// (no trailing comma; trailingComma: 'none') so the comment is preserved after it
    /// rather than relocated before (see conformance_prettier_ts_comments.md §Comment relocation).
    pub(crate) fn append_last_trailing_block_comments_split(
        &self,
        before: &mut DocBuf,
        after: &mut DocBuf,
        elem_end: u32,
        end_boundary: u32,
    ) {
        // Zero-comment fast gate: with no comment in the window, both splits emit
        // nothing wherever the comma is — skip the comma scan entirely.
        if !self.has_comments_to_emit_between(elem_end, end_boundary) {
            return;
        }
        match self.find_comma_in_range(elem_end, end_boundary) {
            Some(comma_pos) => {
                self.append_trailing_inline_block_comments(before, elem_end, comma_pos);
                self.append_trailing_inline_block_comments(after, comma_pos, end_boundary);
            }
            None => self.append_trailing_inline_block_comments(before, elem_end, end_boundary),
        }
    }

    /// Emit comma with surrounding comments for a non-last element in a forced-multiline list.
    ///
    /// Handles comment positioning around the comma between `elem_end` and `next_start`:
    /// 1. Trailing comments before comma (multiline layout)
    /// 2. Comma text
    /// 3. Same-line trailing comments after comma (line comments)
    /// 4. Hardline separator
    ///
    /// Returns the new `prev_end` position.
    ///
    /// `blank_rule` keeps a blank line the author left *before* the next element (or its
    /// own-line leading comment, `A,⏎⏎/* c */⏎B`), and says from WHERE it is measured — a
    /// per-family fact, not a preference (see [`BlankRule`]). Prettier preserves the blank
    /// for **tuples** ([`BlankRule::AfterComma`], measured past the comma) and
    /// **function-type param lists** (function/constructor types, method/call/construct
    /// signatures — [`BlankRule::NextLineEmpty`], measured from the element's end, same as
    /// regular function params), and collapses it for type-parameter / type-argument lists
    /// ([`BlankRule::None`]).
    pub(crate) fn emit_multiline_comma_with_comments(
        &self,
        parts: &mut DocBuf,
        elem_end: u32,
        next_start: u32,
        blank_rule: BlankRule,
    ) -> u32 {
        let d = self.d();
        let comma_pos = self.find_list_comma(elem_end, next_start);

        // The comma binds to the element; same-line gap comments stay before it
        // (block inline, line via `line_suffix`), own-line ones defer to after it
        // (leading the next element). A same-line block stays *before* the comma
        // — prettier 3.9 only moved the `;` case. See `split_comma_gap_comments`.
        let deferred_own_line = self.split_comma_gap_comments(parts, elem_end, comma_pos);

        // This gap's anchor-line split ([`Self::gap_anchor_line_end`]), resolved ONCE for
        // the three readers below — the deferred run's last comment, and each of the two
        // blank-scan arms. It is the same question the emitter above partitioned on, and a
        // reader that re-asks it is a reader that can be given a different answer.
        //
        // The bound is the WIDER `next_start`: that is what the blank scan needs, and it
        // costs the before-comma readers nothing, since the run is a prefix — over
        // `[elem_end, comma_pos)` the two bounds classify identically.
        let anchor_line_end = self.gap_anchor_line_end(elem_end, next_start);
        let last_deferred = comments_to_emit_in_range(self.comments, elem_end, comma_pos)
            .filter(|c| c.span.start >= anchor_line_end)
            .last();

        // A deferred run LEADS the next element, so its last comment takes the
        // leading-comment separator, not this emitter's element hardline: a space when the
        // author glued it to what follows on its line ([`Printer::comment_hugs_next`],
        // prettier's `printLeadingComment`), which for a list is the comma the comment sits
        // in front of (`[a⏎/* c */, b]`). Answering it with the element separator instead
        // drops such a comment onto a line of its own — a third form, and one that puts
        // this family at odds with the array literal, whose per-element group collapses
        // the same soft `line` (`docs/comments.md` §Array family vs params family).
        let deferred_hugs = !deferred_own_line.is_empty()
            && last_deferred.is_some_and(|c| self.comment_hugs_next(c));
        parts.push(d.text(","));
        if deferred_hugs {
            // An author blank line belongs AHEAD of a hugging run, where it was written —
            // between the element and the comment. The element separator below can only
            // put it after, which for a glued run would split the comment from the item it
            // leads.
            //
            // ⚠️ **WHICH blank counts is still the caller's list kind**, exactly as it is
            // in the tail below: a hugging run moves where the `literalline` goes, not
            // which region is measured. Answering `NextLineEmpty` for every family
            // emitted a blank in the TUPLE (`[aaaa⏎⏎/* c */, bbbb]`) that prettier
            // collapses — and the fabricated form is stable, so F1, the ledger and
            // `blanks:audit` are all blind to it.
            if self.separator_gap_has_blank(
                blank_rule,
                elem_end,
                comma_pos + 1,
                next_start,
                anchor_line_end,
            ) {
                parts.push(d.literalline());
            }
            parts.extend(deferred_own_line);
            parts.push(d.text(" "));
            return comma_pos + 1;
        }
        parts.extend(deferred_own_line);

        // Trailing comments after the comma, claimed by kind. A line comment goes through
        // `line_suffix` (zero width) so it never forces the preceding element to break;
        // it flushes at the hardline below (prettier's `lineSuffix`). A block stays
        // inline, width counted.
        //
        // ⚠️ **The block arm asks the HUG question, not a trailing one.** A block the
        // author glued to the next item (`A, /* c */ B`) is that item's leading comment at
        // every other site and in prettier, so claiming it here tears it off the item it
        // was written against — which an anchor-line test does, since the next item sits
        // on a later line in this forced-multiline layout. `is_stranded_after_comma_block`
        // is the single spelling of the split (§Comment relocation): stranded stays
        // trailing the comma, hugging leads the next item. A block ahead of a same-line
        // `//` is stranded by that predicate and stays claimed — the line comment defers
        // through `line_suffix`, so a block left to lead the next item would render after
        // it and the authored pair would come back reversed.
        //
        // A **line** comment on the comma's line is claimed: nothing can follow a `//` on
        // its line, so it has no hug question to ask. Its anchor is the comma, not the
        // element — an author who pushed the comma onto its own line (`a⏎, // c⏎ b`) wrote
        // the comment against the comma, and the printer pulls the comma back onto the
        // element's line, so an element-anchored reading led it onto the NEXT item.
        //
        // ⚠️ **The claimed run ENDS at the first `//` this gap has already emitted** —
        // including one from *before* the comma (`a // c1⏎, // c2⏎ b` folds two source
        // lines onto one output line), which is why the scan opens at `elem_end` rather
        // than at the comma. See [`Printer::gap_emitted_line_comment_before`] for what a
        // claim past that point welds or reorders.
        let mut after_comma_end = comma_pos + 1;
        for comment in comments_to_emit_in_range(self.comments, comma_pos + 1, next_start) {
            if self.gap_emitted_line_comment_before(elem_end, comment.span.start) {
                break;
            }
            let claimed = if comment.is_block {
                self.is_stranded_after_comma_block(comment, comma_pos, next_start)
            } else {
                self.comment_on_comma_line(comma_pos, comment)
            };
            if claimed {
                parts.push(self.build_trailing_comment_doc(comment));
                after_comma_end = comment.span.end;
            }
        }

        // Hardline to separate from next element, optionally preserving an author blank line
        // before the next own-line leading comment. WHICH blank counts is the caller's list
        // kind, not this emitter's business — see [`BlankRule`].
        if self.separator_gap_has_blank(
            blank_rule,
            elem_end,
            after_comma_end,
            next_start,
            anchor_line_end,
        ) {
            parts.push(d.literalline());
        }
        parts.push(d.hardline());

        after_comma_end
    }

    /// Whether this element→element gap holds an author blank line the list **preserves**,
    /// under the caller's family rule ([`BlankRule`]).
    ///
    /// Both of [`Self::emit_multiline_comma_with_comments`]'s separator arms ask it — the
    /// hugging one, which emits the `literalline` ahead of the deferred run, and the tail,
    /// which emits it after the comma. They differ only in where the region past the comma
    /// opens (`from`); the RULE is the list kind and must not change with the arm. Asking
    /// one arm's question in the other's spelling gave the TUPLE a blank prettier collapses
    /// — stable output, so F1, the ledger and `blanks:audit` were all blind to it.
    ///
    /// **in source**: `next_lead` bounds a raw blank-line scan, which cannot tell a
    /// comment's own newlines from an author's blank line — so it must stop at every
    /// comment in the gap, not just the ones this caller emits.
    ///
    /// `anchor_line_end` is the gap's anchor-line split ([`Self::gap_anchor_line_end`]),
    /// passed in rather than re-derived: the caller already resolved it for the deferred
    /// run's separator, and the comment this scan stops at must be the first one that
    /// emitter DEFERRED — a second derivation is a second chance to disagree about which
    /// that is.
    fn separator_gap_has_blank(
        &self,
        blank_rule: BlankRule,
        elem_end: u32,
        from: u32,
        next_start: u32,
        anchor_line_end: u32,
    ) -> bool {
        if blank_rule == BlankRule::None {
            return false;
        }
        let next_lead = self
            .comments_in_source_between(from, next_start)
            .find(|c| c.span.start >= anchor_line_end)
            .map_or(next_start, |c| c.span.start);
        match blank_rule {
            // Measured from the ELEMENT's end, so a blank the author put before the
            // comma still counts — and one after a comma pushed onto its own line
            // does not.
            BlankRule::NextLineEmpty => self.is_next_line_empty(elem_end, next_lead),
            // Measured from past the comma, prettier's array/tuple rule. **Strict**:
            // the next element's span can begin inside a paren shell the printer
            // strips (a tuple element's `[a,⏎(⏎/* c */⏎b)]`), and the newline before
            // that `(` plus the one after it read as an author blank line to the
            // table lookup — emitting a blank the reparse then reads back as real.
            //
            // Strict removes the FABRICATED blank, not every blank the shell touches:
            // a blank the author actually typed inside the shell (`[a,⏎(⏎⏎/* c */⏎b)]`)
            // still has a whitespace-only line to find, so it survives the strip and
            // leads the element, where prettier drops it. Defensible — the author did
            // write the blank, and the result is stable — but it is a divergence, not
            // an equivalence.
            BlankRule::AfterComma => self.has_blank_line_between_strict(from, next_lead),
            // Bailed out above; spelled here so the match stays total.
            BlankRule::None => false,
        }
    }
}
