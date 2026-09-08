// Union and intersection type printing for TypeScript
//
// Handles:
// - Union types: `A | B | C`
// - Intersection types: `A & B & C`
// - Comment handling between type members

use super::helpers::{
    find_separator_position, intersection_has_expanding_first_type,
    intersection_has_huggable_last_type, is_huggable_type, outermost_paren, paren_shell_gaps,
    type_needs_parens_in_union_or_intersection, union_has_brace_member, union_hug_shape,
    unwrap_parenthesized,
};
use super::{CommentFilter, CommentSpacing, Printer, TrailingBlock};
use crate::ast::internal::{
    Comment, TSIntersectionType, TSParenthesizedType, TSType, TSTypeLiteral, TSUnionType,
};
use crate::printer::CommentVec;
use crate::printer::LeadingGlue;
use crate::printer::ShellLeadingRun;
use crate::printer::comments::TrailingBlank;
use crate::printer::ignore::LeadingRunFreeze;
use crate::printer::layout::hang_after_operator;
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// The union VALUE doc an operator seam prints, from
/// [`Printer::build_union_value_doc`] — the doc together with the two facts the seam's
/// own layout must agree with.
pub(in crate::printer) struct UnionValueDoc {
    pub doc: DocId,
    /// Whether the gap's glued leading run was handed INTO the union (the union prints
    /// it, after the pipe its broken layout synthesizes); the seam emits the gap's
    /// comments itself ONLY when this is `false` (docs/comments.md hazard 3).
    pub run_handed: bool,
    /// Whether `doc` prints hugged (`{ … } | null` with the member owning its
    /// expansion). The seam keeps its operator glued exactly when this holds; a handed
    /// run always declines it.
    pub hugged: bool,
}

/// What stands AHEAD of a union, for the hug question ([`Printer::union_prints_hugged_with`]):
/// a block comment glued to the first member sits in the enclosing seam's gap, outside
/// `union.span`, and whether the union may keep hugging behind it is a fact about that
/// seam, which the union cannot read off its own span.
#[derive(Clone, Copy)]
enum UnionLeadingGap {
    /// A VALUE seam — the alias `=`, the annotation `:`, the function-type `=>`, the
    /// mapped-type value — whose gap runs from `gap_start` (just past the operator) to
    /// the first member. A block comment anywhere in it declines the hug
    /// ([`Printer::union_prints_hugged_with`]): glued to the member it is prettier's own
    /// rule — the comment binds to the first member, `types.some((t) => hasComment(t))`
    /// bails and the union breaks after the operator — whether handed in from the seam's
    /// gap ([`Printer::build_union_value_doc`], `handed`) or authored after the union's
    /// leading pipe (`| /* c */ { … }`); a block the author BROKE after collapses onto
    /// the member at these seams (the single-line-block rule every keyword→value gap
    /// applies), so it must decide as the glued spelling it becomes, or the collapsed
    /// output declines on the next pass (F1). One rule, so every spelling is one fixed
    /// point (`union_hug_gap_block_comment`).
    ValueSeam { gap_start: u32, handed: Option<u32> },
    /// Any other position — a type argument's `<`, a tuple's `[`, a paren shell, an
    /// `as` / `satisfies` keyword, a conditional's keywords, the predicate's `is`: the
    /// enclosing seam prints its own gap ahead of the union, and the union answers the
    /// hug from its span alone, so a glued block keeps it hugging. Right at `is` and at
    /// a conditional's CHECK (prettier binds the comment to the parent that starts
    /// where the union does); an open divergence at the rest, where prettier binds it
    /// to the member — see [`Printer::union_prints_hugged`].
    Other,
}

/// What [`Printer::build_intersection_member_body_doc`] hands back for one continuation
/// member — three answers its caller's loop needs and cannot re-derive, named because a
/// bare tuple of them reads as three unrelated values at the call site.
struct IntersectionMemberBody {
    /// The member's doc parts: leading comments, the member, the trailing comments and `&`.
    parts: DocBuf,
    /// Whether a leading run went in carrying a **breakable** separator — a `line` only an
    /// enclosing group can decide, which an object-adjacent boundary supplies none of.
    run_breaks: bool,
    /// The member's lifted trailing run, held for the boundary that FOLLOWS it — the only
    /// place it can be queued at the indent it will flush at
    /// ([`Printer::push_hoisted_member_doc`]). `None` for a member that took no hoist, and
    /// for the LAST member, which has no boundary after it.
    held_run: Option<DocId>,
}

/// Who emits an intersection's leading-`&` gap run (`[span.start, first.start)`) — the
/// print-once seam between the leading-gap line-comment route and the body builders it
/// delegates to. Exactly one of the two prints the run.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LeadingGap {
    /// The body builder emits it (the ordinary route): its own block-only extraction,
    /// `& /* c */ A & B`.
    Emit,
    /// The leading-gap route already emitted the WHOLE run (blocks and lines, in source
    /// order) ahead of the body, so the body must not emit it again — that would
    /// double-print the gap's blocks (docs/comments.md hazard 3).
    Claimed,
}

/// An intersection's first-member leading run, and how the body must then be built — the
/// shared answer for the two routes that own the leading-`&` gap and emit it themselves
/// ([`Printer::build_intersection_leading_gap_line_comment_doc`] and the hoist inside
/// [`Printer::build_intersection_type_doc`]).
///
/// Two shapes reach it, disjoint by construction because
/// [`Printer::head_stripped_paren_shell`] never returns the node it was asked about — the
/// shell **is** the member ([`Printer::intersection_first_member_hoist_comments`]), or it
/// sits one link **inside** it (`(⏎// c⏎A)[] & B`). They differ only in WHICH span the
/// claim covers; the body answers both the same way, because "this shell's leading run is
/// already emitted" is the whole of what either has to say. The first shape's member is
/// still paren-STRIPPED — but by the ordinary hoist, which strips it for its TRAILING run
/// and holds that run for the boundary that follows, so this type carries no verdict of
/// its own about the parens.
///
/// ⚠️ Both shapes must be answered at BOTH routes. Answered at only one, the sibling left
/// the edge shell printing its run at whatever indent it was built at, welded onto the
/// block the gap had already emitted — the same defect, one route over.
struct IntersectionHeadRun<'c> {
    /// The comments this route emits ahead of the body, in source order. Empty when
    /// neither shape applies (or the first member is frozen, whose shell comments ride
    /// inside its verbatim slice).
    run: CommentVec<'c>,
    /// The shell whose own copy of `run` must stand down while the body builds — the
    /// member's own for the hoist shape, the leading-EDGE one for the other. `None`
    /// exactly when `run` is empty.
    claimed_shell: Option<Span>,
}

/// Member-parens predicate for a union/intersection with `member_count` members.
///
/// A single-member union/intersection collapses to its member (Prettier drops
/// single-element union/intersection nodes in postprocess), so the lone member prints
/// in the union's own position and needs no precedence parens of its own — any required
/// parens come from the union's parent context, applied one level up. 2+ members use the
/// normal `|`/`&` precedence rule.
pub(super) fn union_member_parens(member_count: usize) -> fn(&TSType<'_>) -> bool {
    if member_count == 1 {
        |_| false
    } else {
        type_needs_parens_in_union_or_intersection
    }
}

/// Whether a parenthesized union member is a pure paren-union (`(A | B)`), the
/// one paren shape that needs the extra per-member offset. Its layout comes from
/// `build_parenthesized_union_doc`, which puts `(`/`)` on their own lines at the
/// bare member indent — one level too shallow once the `| ` prefix is accounted
/// for. Every other parenthesized member already closes at the right level:
/// object-trailing intersections (`(A & { … })`) double-indent their body via
/// `build_parenthesized_intersection_trailing_object_doc`, and function /
/// constructor / conditional parens let their inner type supply the indent and
/// ride the closing `)` on the inner's last line (`) => void)`).
fn is_paren_union_member(ts_type: &TSType<'_>) -> bool {
    matches!(unwrap_parenthesized(ts_type), TSType::Union(_))
}

/// Which composite's member seam a lifted trailing run is being asked about. The two
/// containers differ in two ways that both bear on the hoist, so they are named once rather
/// than passed as a pair of bare flags:
///
/// - **which paren arms EXPAND** — the trailing-object alignment is gated on
///   `DefaultParenIndent::Nested` inside `Printer::build_type_doc_maybe_parens_impl`, so
///   `(A & { … })` reaches `build_parenthesized_intersection_trailing_object_doc`'s aligned
///   `})` closer from a UNION member and the bare flat wrapper from an INTERSECTION member,
///   whose level comes from the `& `-line indent ([`Printer::member_seam_trailing_shell`]);
/// - **what sits between the member and the run** — nothing in a union, where the run still
///   trails the member on its own line, but a `" &"` in an intersection, whose loop emits
///   the separator before placing the run. An INLINE comment cannot cross that: it renders
///   where it is queued. So a union hoists its run whole and an intersection SPLITS it at
///   the first `//`, handing the inline prefix back to the member
///   ([`Printer::intersection_hoisted_run_split`]).
#[derive(Clone, Copy, PartialEq)]
enum MemberSeam {
    /// A UNION member: `| ` opens the member's line and the run trails it.
    Union,
    /// An INTERSECTION member: the `&` is emitted between the member and the run.
    Intersection,
}

/// Which paren layout a union / intersection member's pair prints — the three-way routing
/// [`Printer::build_union_member_offset_doc`] takes, named once because
/// [`Printer::member_seam_trailing_shell`] has to agree with it. An EXPANDING pair owns the
/// shell's trailing gap and prints its run at its own interior indent; a flat one leaves the
/// run at the member seam. Spelled twice, this is exactly the pair that drifts — a third
/// expanding arm would have to be learned by the builder AND the hoist gate, and a gate
/// wider than the layout answering it is a DROPPED comment
/// ([`comments.md`](../../../../docs/comments.md) hazard 4).
#[derive(Clone, Copy, PartialEq)]
enum MemberPairLayout {
    /// No pair at all — `member_parens` says this member needs none, so any shell it
    /// carries is redundant and strips.
    None,
    /// The bare `("(", inner, ")")` wrapper: both delimiters ride the inner type's own
    /// first and last lines, however far the inner itself breaks.
    Flat,
    /// `build_parenthesized_union_doc` — `(` and `)` take their own lines around the inner
    /// union's members. EXPANDS.
    ParenUnion,
    /// `build_parenthesized_intersection_trailing_object_doc` — the aligned `})` closer.
    /// EXPANDS. Gated on `DefaultParenIndent::Nested`, so a UNION member only
    /// ([`MemberSeam`]).
    AlignedObject,
}

impl MemberPairLayout {
    /// Whether the pair opens over hardlines, and so OWNS the shell's trailing gap rather
    /// than leaving its run to the member seam.
    fn expands(self) -> bool {
        matches!(self, Self::ParenUnion | Self::AlignedObject)
    }
}

/// The **redundant** paren shell of a union / intersection member: the pair the comment-free
/// rule strips (`(b)` → `b`). `None` both for a member whose parens the precedence rule
/// REQUIRES (`(a | b) | c`, a function / conditional operand) and for a member that carries
/// no shell at all.
///
/// The LEADING side's opening, and only that side's. A leading `//` inside a required pair
/// stays where the author wrote it — the pair OPENS over hardlines to hold it
/// ([`Printer::build_open_required_paren_doc`]) — so only a pair that strips can hoist one
/// out ahead of the `| `. The TRAILING side asks a different question and reads
/// [`Printer::member_seam_trailing_shell`] instead: a trailing run lifts out of a REQUIRED
/// pair too, so what decides its home is the printed pair's SHAPE, not its redundancy.
fn redundant_member_shell<'t>(t: &'t TSType<'t>) -> Option<&'t TSParenthesizedType<'t>> {
    if type_needs_parens_in_union_or_intersection(t) {
        return None;
    }
    outermost_paren(t)
}

impl<'a> Printer<'a> {
    //
    // Union Types
    //

    /// The FULL leading comment run (block + line) inside a **redundant** parenthesized
    /// union member — one whose parens the comment-free rule strips (`(b)` → `b`,
    /// `!type_needs_parens_in_union_or_intersection`), so the comment cannot stay "inside"
    /// parens that don't survive — whose leading gap holds a **line** comment. Covers the
    /// pure-line (`(// c⏎ b)`), mixed (`(/* b */ // c⏎ b)`), and trailing (`(// c⏎ b /* t */)`)
    /// shells uniformly: the whole run hoists losslessly — the leading block + line each on
    /// their own line before the `| ` (this run, via [`Self::push_union_member_leading_run`]),
    /// the trailing comment appended to the member via [`Self::with_stripped_paren_trailing`].
    /// Declines a **retained**-paren member (union / intersection / function / conditional —
    /// its comment stays inside, the arms further down) and a non-paren member. Requires a
    /// **line** comment in the leading gap: a block-only (`(/* b */ b)`) or comment-free gap
    /// keeps its block inline and is already idempotent, so it returns empty (the general
    /// member arm). Peels every redundant nesting layer (`((// c⏎ b))` → `b`) to match the
    /// detection window. The narrow shared [`Self::stripped_paren_leading_line_comments`]
    /// (line-only, no block/trailing) still serves the conditional-`extends` and
    /// intersection-first-member callers.
    ///
    /// ⚠️ **The RETAIN verdict is the shell's, and this hoist strips, so it must ask it
    /// too** ([`Self::paren_retains_for_trailing_run`], the decline
    /// [`Self::intersection_first_member_hoist_comments`] already states for the
    /// intersection's own hoist). A shell the retain rule keeps holds a trailing `//` that
    /// nothing separates from the statement's tail — a LAST or sole member — so stripping
    /// it for the LEADING run defers the trailing one past the `;` exactly as stripping it
    /// for the trailing run would, which is the relocation that rule exists to prevent
    /// (`docs/comments.md`: a deferred run must not leave the construct it was written in).
    /// Asked only here, the union answered one question two ways: `| (// c⏎ a // t⏎)` and
    /// `| (a // t⏎)` are the same shell and reached different fixed points, and the second
    /// pass over the retained form fell into the first case — so the retain rule's own
    /// output was not stable under it. `type_suffix_trailing_comment_union_member`.
    fn stripped_redundant_paren_member_leading_run(&self, t: &TSType<'_>) -> CommentVec<'_> {
        let Some(shell) = redundant_member_shell(t) else {
            return smallvec![];
        };
        if self.paren_shell_retains_for_trailing_run(shell) {
            return smallvec![];
        }
        let (leading_gap, _) = paren_shell_gaps(shell);
        let leading: CommentVec<'_> = self
            .comments_to_emit_between(leading_gap.start, leading_gap.end)
            .collect();
        if leading.iter().any(|c| !c.is_block) {
            leading
        } else {
            smallvec![]
        }
    }

    /// Whether a union member's paren shell holds a `//` the multiline layout must own —
    /// the ROUTER's question, asked through the same two predicates the member loop below
    /// emits from, so the routing and the emission cannot disagree.
    ///
    /// Two shapes, one question. The shell may BE the member (retained, or redundant and
    /// hoisted), or it may sit at the member's leading printed **edge** one link down
    /// (`A | (⏎// c⏎B)[]`, `A | (⏎// c⏎B) & C` — [`Self::leading_edge_shell_claim`]). Both
    /// put a `//` in the member's own gap, and a `//` there forces the leading-pipe layout
    /// whichever shape produced it. A router that asked only the shallow shape sent the
    /// leading-edge shell to the width path, where the shell's own emitter printed the run
    /// after the `| ` — while the reparse, finding the comment in the member gap, put it
    /// above, so the two passes disagreed.
    fn union_member_paren_leading_line_comment(&self, t: &TSType<'_>) -> bool {
        // Both shapes, in order — never one OR the other. A member can be a paren shell
        // whose own gap is empty and still carry a leading-edge shell inside it
        // (`A | ((⏎// c⏎B)[])`, the author's extra layer), so returning the shallow answer
        // early routed that to the width path and left the shell printing its own run.
        self.stripped_paren_hang_has_leading_line_comment(t)
            || self.leading_edge_shell_line_comment(t)
    }

    /// Push a comment run that leads a union member from ABOVE its `| ` separator —
    /// the layout both runs at that seam take: the member gap's own-line side
    /// ([`Self::union_gap_inline_run_start`]) and a stripped-redundant-paren member's
    /// hoisted leading run ([`Self::stripped_redundant_paren_member_leading_run`]).
    ///
    /// The run's INTERNAL shape is prettier's leading-comment rule, asked per comment of
    /// its own neighbour: a comment the author glued to the next one keeps it on the same
    /// line, an author blank between two of them survives, and everything else takes a
    /// line of its own. The LAST comment always breaks — the `| ` opens the member's line,
    /// so this side of the seam cannot end inline — and no blank is emitted toward the
    /// member, which is the one place this run departs from
    /// [`Self::push_leading_comment_run`] and why it cannot simply delegate.
    ///
    /// ⚠️ **Both runs take this rule because glue is the AUTHOR's, not the seam's.** The
    /// stripped-paren run hardlining unconditionally would put `(/* b */ // c⏎ b)`'s block
    /// and line on separate lines — a pair the author wrote on one. The
    /// enclosing divergence sanctions only the run's POSITION (tsv leads the member, prettier
    /// trails the previous one); it says nothing about the run's interior, where prettier
    /// glues exactly as tsv does everywhere else.
    fn push_union_member_leading_run(&self, parts: &mut DocBuf, run: &[&Comment]) {
        let d = self.d();
        for (j, comment) in run.iter().enumerate() {
            parts.push(self.build_comment_doc(comment));
            let Some(next) = run.get(j + 1) else {
                parts.push(d.hardline());
                continue;
            };
            if self.comment_hugs_next(comment) {
                parts.push(d.text(" "));
                continue;
            }
            if self.has_blank_line_between(comment.span.end, next.span.start) {
                parts.push(d.literalline());
            }
            parts.push(d.hardline());
        }
    }

    /// Where a union member gap's comment run splits between the part printed
    /// **before** the `| ` separator (own-line) and the part printed **after** it
    /// (glued to the member) — the index of the first inline comment, `run.len()`
    /// when the whole run goes own-line.
    ///
    /// ⚠️ **The split is a PREFIX/SUFFIX partition, never a per-comment filter.** The
    /// two sides land on opposite sides of the `|`, so a filter that lets an inline
    /// comment follow an own-line one REORDERS the run:
    /// `| /* c1 */ /* c2 */⏎b` came out `/* c2 */⏎| /* c1 */ b`, printing the author's
    /// second comment first. Reordering is content-relation loss, and it is invisible
    /// to every self-oracle gate (the result is stable, reparses, and loses no bytes).
    ///
    /// The inline side is therefore the maximal suffix whose comments are each glued
    /// forward with nothing else on their line — a run that reaches the member glued
    /// (`| /* c1 */ /* c2 */ b`). Two consequences, both prettier's answer:
    ///
    /// - a run whose LAST comment breaks to the member goes own-line whole, however
    ///   many of its heads are glued (`/* c1 */ /* c2 */⏎| b`);
    /// - the split never falls inside an author-glued chain. When the comment before
    ///   the suffix is glued to it the pair would straddle the `|`, so the whole run
    ///   takes the own-line side instead (`| /* c1 */⏎/* c2 */ /* c3 */ b` →
    ///   `/* c1 */⏎/* c2 */ /* c3 */⏎| b`) — an own-line comment cannot move to the
    ///   inline side, so keeping the pair together is the only order-preserving answer.
    ///
    /// `hugs_glued` is the per-comment predicate: forward glue AND its own placement,
    /// since an own-line-authored block glued forward to the member stays own-line
    /// (see the call site's oscillation note).
    fn union_gap_inline_run_start(&self, run: &CommentVec<'_>) -> usize {
        let hugs_glued = |c: &Comment| self.comment_hugs_next(c) && !self.is_own_line_comment(c);
        let mut start = run.len();
        while start > 0 && hugs_glued(run[start - 1]) {
            start -= 1;
        }
        if start > 0 && self.comment_hugs_next(run[start - 1]) {
            return run.len();
        }
        start
    }

    /// Emit the leading block comments in `[start, end)` before the FIRST union
    /// member, choosing the separator after each comment per Prettier's
    /// `printLeadingComment`: a `line` when the source has a newline after the
    /// comment's `*/`, a `hardline` when there is a newline both before its `/*`
    /// and after its `*/`, otherwise a space. The `line` lets the member stay
    /// glued inline when the union fits and break onto its own line when the
    /// union expands (the leading-pipe form), matching Prettier; a hardcoded
    /// space would glue a member onto a multi-line comment's `*/` line.
    /// Returns an empty doc when the range has no block comment.
    ///
    /// Asked by the UNION only for its first member: a block between two union members
    /// never reaches a hardcoded separator anyway — the union's one-sided gate
    /// (`union_has_own_line_member_comment`) routes every comment that starts a line to
    /// the multiline builder, leaving that path only comments glued after the `|`, whose
    /// separator is a space either way. The INTERSECTION asks it at every boundary
    /// (`build_intersection_member_body_doc`), because its two-sided gate deliberately
    /// leaves a comment sharing the `&`'s line on the width-decided path, where the
    /// author's break after the comment is exactly what the `line` carries.
    ///
    /// Returns the doc and whether a **breakable** separator went into it — the group
    /// question the `line` implies, since only an enclosing group can decide one. Both
    /// come from the same walk on purpose: a caller that re-derived the flag from the
    /// source would be a second reading of one fact, free to drift from the emission.
    fn build_member_leading_block_comments(&self, start: u32, end: u32) -> (DocId, bool) {
        let d = self.d();
        let mut parts = DocBuf::new();
        let mut breaks = false;
        for comment in self.comments_to_emit_between(start, end) {
            if !comment.is_block {
                continue;
            }
            parts.push(self.build_comment_doc(comment));
            // The three arms are the two source facts the gates read, in the same
            // vocabulary: glued to what follows → space; the author broke after it →
            // a `line` the group decides; isolated on its line (broke after AND nothing
            // before it) → the author's own break. The last arm is what
            // `intersection_has_isolated_member_comment` routes away from this path
            // entirely, so for an intersection it is the union's first-member case only.
            if self.comment_hugs_next(comment) {
                parts.push(d.text(" "));
            } else {
                breaks = true;
                parts.push(if self.comment_isolated_on_its_line(comment) {
                    d.hardline()
                } else {
                    d.line()
                });
            }
        }
        (d.concat(&parts), breaks)
    }

    /// Append the block comments sitting *after* the `|` separator and before the union
    /// member that follows it (`A | /* c */ B`), each spaced, appending nothing when
    /// there are none.
    ///
    /// The separator's source position is needed only to bound that range — the printed
    /// `|` is static text — so the caller gates on its whole-union window first and this
    /// runs only when a comment is actually in play.
    ///
    /// Union-only, and the hardcoded space is why: the union's one-sided gate sends every
    /// comment that starts a line to the multiline builder, so what reaches here is glued
    /// after the `|` and takes a space either way. The intersection, whose gate leaves a
    /// comment sharing the `&`'s line on this path, needs the source-keyed separator
    /// instead ([`Self::build_member_leading_block_comments`]).
    fn push_post_separator_block_comments(
        &self,
        parts: &mut DocBuf,
        prev_member_end: u32,
        member_start: u32,
    ) {
        if let Some(sep_pos) =
            find_separator_position(self.source, prev_member_end, member_start, b'|')
            && let Some(comments) = self.build_comments_between_filtered_opt(
                sep_pos + 1,
                member_start,
                CommentSpacing::Trailing,
                CommentFilter::BlockOnly,
            )
        {
            parts.push(comments);
        }
    }

    /// Append the block comments sitting *before* the `|`/`&` separator that follows a
    /// member (`A /* c */ | B`), appending nothing when there are none. The separator
    /// counterpart of `push_post_separator_block_comments`; same gating contract.
    fn push_pre_separator_block_comments(
        &self,
        parts: &mut DocBuf,
        member_end: u32,
        next_member_start: u32,
        separator: u8,
    ) {
        if let Some(sep_pos) =
            find_separator_position(self.source, member_end, next_member_start, separator)
            && let Some(comments) = self.build_comments_between_filtered_opt(
                member_end,
                sep_pos,
                CommentSpacing::Leading,
                CommentFilter::BlockOnly,
            )
        {
            parts.push(comments);
        }
    }

    /// Build a union member's type doc with Prettier's per-member `align(2, …)`
    /// offset (`union-type.js`), rendered as a sub-tab alignment — literal spaces
    /// at a trailing closing delimiter, rounding up to a whole tab wherever a
    /// member's own internal indent stacks on it
    /// (`docs/conformance_prettier_ts_comments.md`).
    ///
    /// The offset applies to bare members (plain types, generics whose args wrap)
    /// and to pure paren-unions (`| (A | B)`), whose `build_parenthesized_union_doc`
    /// layout otherwise sits one level too shallow. Members that supply their own
    /// alignment opt out of the wrapper to avoid double-indenting:
    /// - object literals (`| { … }`) via `build_union_member_object_literal_doc`,
    /// - object-trailing intersections (`| (A & { … })`) and function /
    ///   constructor / conditional parens (see `is_paren_union_member`).
    fn build_union_member_offset_doc(
        &self,
        t: &TSType<'_>,
        member_parens: fn(&TSType<'_>) -> bool,
    ) -> DocId {
        let d = self.d();
        if let TSType::TypeLiteral(obj) = t {
            return self.build_union_member_object_literal_doc(obj);
        }
        match self.member_pair_layout(t, member_parens, MemberSeam::Union) {
            // `(A & { … })` supplies its own `align(2)` inside
            // `build_aligned_object_literal_doc` (its closing `})`), so it opts out of the
            // wrapper here — wrapping again double-shifts it.
            MemberPairLayout::AlignedObject => self.build_type_doc_maybe_parens(t, member_parens),
            // Function / constructor / conditional / constrained-infer paren member.
            // Prettier's needs-parens wrapping is a bare `["(", doc, ")"]` (no inner
            // indent); the member's whole `(…)` takes the `align(2)` offset, so the content
            // rounds up to a tab and the closing `) => …)` line trails at 2 spaces. Use the
            // intersection-member variant (`indent_default_paren = false`) for the bare
            // paren, then apply the offset — the old inner `d.indent` faked the offset as a
            // whole tab and stranded the closing.
            MemberPairLayout::Flat => {
                d.align(2, self.build_intersection_member_type_doc(t, member_parens))
            }
            // A pure paren-union carries its own expanding layout, and a member with no
            // pair is just the bare type; both take the offset over whatever they print.
            MemberPairLayout::ParenUnion | MemberPairLayout::None => {
                d.align(2, self.build_type_doc_maybe_parens(t, member_parens))
            }
        }
    }

    /// [`Self::build_union_member_offset_doc`] with a stripped paren shell's lifted
    /// trailing run placed OUTSIDE the per-member offset — prettier's own split, and the
    /// one the reparse agrees with. `leading_start` opens the member's leading region; see
    /// [`Self::member_hoisted_trailing_shell`] for which members hoist and why.
    fn build_union_member_doc(
        &self,
        t: &TSType<'_>,
        member_parens: fn(&TSType<'_>) -> bool,
        has_leading_comments: bool,
    ) -> DocId {
        match self.member_hoisted_trailing_shell(
            t,
            member_parens,
            MemberSeam::Union,
            has_leading_comments,
        ) {
            Some(inner) => self.with_stripped_paren_trailing_blank(
                self.build_union_member_offset_doc(inner, member_parens),
                t,
                inner,
                TrailingBlock::Inline,
                // The run stays a TRAILING run of this member — prettier binds every
                // in-union comment to the member above it — so the author's blank survives
                // at both formatters. The intersection's twin below answers `Drop`; see
                // [`TrailingBlank`] for why one question has two answers here.
                TrailingBlank::Keep,
            ),
            None => self.build_union_member_offset_doc(t, member_parens),
        }
    }

    /// The DEFERRED half of an intersection first member's lifted run, and the inline half
    /// that stays behind — the run split at its first `//`.
    ///
    /// A union hoists its run whole: nothing is emitted between the member and the run, so
    /// an inline block travels along and still renders trailing the member, which is where
    /// prettier puts it too (`| a /* c1 */⏎// c2⏎| c`). An INTERSECTION cannot — the loop
    /// has already emitted `" &"` by the time it places the run, and a block renders where
    /// it is QUEUED, so a whole-run hoist lands it on the wrong side of the operator
    /// (`a & /* c1 */` for the author's `a /* c1 */ &`, which is also prettier's). So the
    /// prefix is handed back to the member and only the deferred tail travels.
    ///
    /// Returns `(inline_prefix, deferred_run)`. ⚠️ The two PARTITION the shell's trailing
    /// gap at one position — everything before the first `//` renders inline, everything
    /// from it on is deferred by [`Printer::push_trailing_comments_in_range`] anyway — so
    /// neither a drop nor a double-print is expressible ([`comments.md`](../../../../docs/comments.md)
    /// hazard 3). BOTH must be placed: the prefix into the member's own parts, ahead of
    /// the `" &"`, and the run wherever the boundary answers — which is why the sole
    /// caller is [`Self::push_hoisted_member_doc`], where the four parts of the shell are
    /// one body.
    fn intersection_hoisted_run_split(
        &self,
        original: &TSType<'_>,
        inner: &TSType<'_>,
    ) -> (Option<DocId>, DocId) {
        let gap_start = inner.span().end;
        let gap_end = original.span().end;
        // The split is the end of the LAST leading block, not the start of the first `//` —
        // the difference is the own-line signal. `push_trailing_comments_in_range` decides
        // each comment's line by asking `comment_has_newline_between(prev_end, …)`, and
        // `prev_end` opens at the window's start; opening it AT the `//` makes the emitter
        // read zero bytes before it and print it inline after the `&`, where prettier (and
        // tsv's own line-comment-led spelling) give it its own line. Opened at the `*/`, the
        // author's newline is still in the window and the run reads as written.
        //
        // With no leading block the split is `gap_start` and this is the whole gap, which is
        // the union's spelling exactly — the partition costs a line-led run nothing.
        let mut split = gap_start;
        for c in self.comments_to_emit_between(gap_start, gap_end) {
            if !c.is_block {
                break;
            }
            split = c.span.end;
        }
        let prefix = self.build_comments_between_filtered_opt(
            gap_start,
            split,
            CommentSpacing::Leading,
            // Block-only by construction — `split` is the FIRST line comment's start — so
            // the filter refuses nothing; it states the window's shape at the call site.
            CommentFilter::BlockOnly,
        );
        let run = self.with_stripped_paren_trailing_range(
            self.d().empty(),
            split,
            gap_end,
            TrailingBlock::Inline,
            // The intersection's answer: the member below re-reads this run as its LEADING
            // one, and `printLeadingComment` asks `isNextLineEmpty`, so a blank above has
            // nowhere to land ([`TrailingBlank`]).
            TrailingBlank::Drop,
        );
        (prefix, run)
    }

    /// The paren shell a union / intersection member's lifted trailing run comes OUT of —
    /// the pair whose printed form leaves that run at the MEMBER SEAM rather than owning it.
    ///
    /// **Not the redundancy question**, which is what
    /// [`redundant_member_shell`] answers for the leading side. A trailing run lifts out of
    /// a REQUIRED pair just as readily: [`Self::paren_shell_retains_for_trailing_run`]
    /// grants the strip licence on the member SEPARATOR that follows the shell
    /// ([`Self::type_member_separator_follows`]), never on whether the parens survive — so
    /// `((a: 1) => void // c⏎)` prints its required `(`…`)` and defers the run past the `)`
    /// all the same. What decides where the run belongs is therefore the printed pair's
    /// SHAPE.
    ///
    /// Exactly two member pairs EXPAND, and both own the shell's trailing gap themselves,
    /// printing the run at their own interior indent — which is also where the reparse
    /// reads it back:
    ///
    /// - `build_parenthesized_union_doc`, whose `(` and `)` take their own lines around the
    ///   inner union's members (`union_intersection_retained_paren_line_comment`,
    ///   `array_paren_union_member_line_comment`);
    /// - `build_parenthesized_intersection_trailing_object_doc`, whose aligned `})` closer
    ///   holds the run at the `)` column — the sanctioned
    ///   `retained_paren_shell_trailing_comment_run` divergence, reachable from a union
    ///   member only ([`MemberSeam`]).
    ///
    /// Every other pair is the bare `("(", inner, ")")` wrapper, which rides the inner
    /// type's own first and last lines: the run lands past the `)` on the member's line and
    /// is the seam's to place. Both gates read `member_parens` first because
    /// `Printer::build_type_doc_maybe_parens_impl` does — a one-member composite passes
    /// `|_| false` ([`union_member_parens`]), and with no required pair to print, even a
    /// paren-union member strips like any other. ⚠️ That guard is **builder symmetry, not a
    /// pinned behaviour**: every shape reachable through it is already declined by the
    /// RETAIN gate below (a sole member has no separator after it, so its shell keeps its
    /// parens), and no probe separates the two spellings. It stays because a predicate that
    /// must agree with a builder should read what the builder reads; dropping it can only
    /// ever over-decline, and silently.
    ///
    /// A redundant shell is admitted by this same rule rather than as a separate case:
    /// both expanding shapes have a union / intersection inner, whose parens are always
    /// REQUIRED, so no redundant shell can reach either arm.
    fn member_seam_trailing_shell<'t>(
        &self,
        t: &'t TSType<'t>,
        member_parens: fn(&TSType<'_>) -> bool,
        seam: MemberSeam,
    ) -> Option<&'t TSParenthesizedType<'t>> {
        if self.member_pair_layout(t, member_parens, seam).expands() {
            return None;
        }
        outermost_paren(t)
    }

    /// Which pair a union / intersection member prints — see [`MemberPairLayout`] for why
    /// this is one function and not a branch in each of its two readers.
    fn member_pair_layout(
        &self,
        t: &TSType<'_>,
        member_parens: fn(&TSType<'_>) -> bool,
        seam: MemberSeam,
    ) -> MemberPairLayout {
        // `member_parens` first because `Printer::build_type_doc_maybe_parens_impl` gates
        // its whole paren arm on it: with no required pair to print, even a paren-union
        // member strips like any other. A one-member composite passes `|_| false`
        // ([`union_member_parens`]).
        if !member_parens(t) {
            return MemberPairLayout::None;
        }
        if is_paren_union_member(t) {
            return MemberPairLayout::ParenUnion;
        }
        if seam == MemberSeam::Union && self.aligned_trailing_object_shell(t).is_some() {
            return MemberPairLayout::AlignedObject;
        }
        MemberPairLayout::Flat
    }

    /// A union / intersection member whose paren shell lifts a trailing comment run out of
    /// itself ([`Self::member_seam_trailing_shell`]) — the shape whose lifted run must be
    /// built at the MEMBER SEAM rather than inside the member's own offset. Returns the
    /// fully-unwrapped
    /// inner for the caller to build there, the run appended via
    /// [`Printer::with_stripped_paren_trailing`]. Building the inner re-prints whatever
    /// pair the member needs, so a REQUIRED one is not lost by the unwrap — only its flat
    /// shape is relied on.
    ///
    /// One predicate, two containers, because it is one question — the lifted run is a
    /// deferred `line_suffix` whose own break renders at the indent it was QUEUED at, and
    /// each container queued it somewhere the reparse does not put it. The union built it
    /// inside the per-member `align(2)`, two columns past the `|`; the intersection built
    /// it outside the continuation `indent`, one level short of it. Pass 2 — which finds
    /// the same comment in the member GAP, the parens gone — agreed with prettier at both,
    /// so both were F1 violations against the same fixed point. What each caller then does
    /// with the run differs, because the containers' own layouts do (see
    /// [`Self::build_union_member_doc`] and the intersection's `held_trailing_run`).
    ///
    /// Prettier reaches the same two places by two different routes, and neither is a
    /// renderer rule: in a union its `handleUnionTypeComments` binds every in-union comment
    /// to the PRECEDING member, and `union-type.js` then prints a comment-carrying member
    /// as `printComments(align(2, typeDoc))` — comments OUTSIDE the offset, the source
    /// comment there saying so in as many words ("We want to align the children but without
    /// its comment") — unless the member has LEADING comments, when the whole
    /// comment-wrapped doc goes inside instead. In an intersection that handler does not
    /// fire, so an own-line comment attaches as the NEXT member's leading comment and
    /// prints inside that member's `indent`.
    ///
    /// ⚠️ **`has_leading_comments` is therefore the UNION's question, and only the
    /// union's.** Both arms of its split are observable, which is what makes the condition
    /// load-bearing there in both directions rather than mere caution
    /// ([`Self::union_member_has_leading_comments`] draws the window, and reads the
    /// member's leading printed EDGE as well as its own shell). The intersection passes
    /// `false`: with no handler to fire, prettier's oracle says nothing about the shape at
    /// that seam, and its own answer — the run at the boundary's indent — is the same
    /// whether or not something leads the member
    /// ([`Self::intersection_member_hoisted_shell`]).
    ///
    /// ⚠️ **The decline was carrying a second, unstated duty: it kept
    /// `build_parenthesized_type_unwrap_doc` in play as the shell's leading-run emitter.**
    /// Building the inner drops the shell and every comment it holds, so a caller that
    /// hoists past a non-empty leading region must emit that region itself
    /// ([`Self::push_hoisted_member_doc`]) or the run is a DROP
    /// ([`comments.md`](../../../../docs/comments.md) hazard 1), not a relocation.
    fn member_hoisted_trailing_shell<'t>(
        &self,
        t: &'t TSType<'t>,
        member_parens: fn(&TSType<'_>) -> bool,
        seam: MemberSeam,
        has_leading_comments: bool,
    ) -> Option<&'t TSType<'t>> {
        // A pair whose printed form leaves the run at the SEAM — redundant, or required
        // and flat ([`Self::member_seam_trailing_shell`]).
        let shell = self.member_seam_trailing_shell(t, member_parens, seam)?;
        // A RETAINED shell prints its own parens and keeps the run between them, where the
        // author wrote it — that run is the shell's interior, not the member seam.
        if self.paren_shell_retains_for_trailing_run(shell) {
            return None;
        }
        // Prettier's `hasComment(node, Leading)` arm — the caller's to answer, since which
        // comments actually LEAD a member is the container's layout question.
        if has_leading_comments {
            return None;
        }
        let (_, trailing) = paren_shell_gaps(shell);
        let mut run = self
            .comments_to_emit_between(trailing.start, trailing.end)
            .peekable();
        run.peek()?;
        // The run must have a DEFERRED BREAK to place, and a `//` is the only thing that
        // puts one there ([`Printer::push_trailing_comments_in_range`] defers everything
        // from the first line comment on). A run of blocks alone renders INLINE, so it has
        // no break whose indent could be wrong — the whole question this hoist answers —
        // and moving it would relocate a comment for nothing
        // (`retained_paren_intersection_member_comment`'s `(a & b /* c */) | c`, where the
        // block would escape the parens the author wrote it inside).
        if !run.any(|c| !c.is_block) {
            return None;
        }
        Some(unwrap_parenthesized(t))
    }

    /// Whether a union member carries LEADING comments — prettier's
    /// `hasComment(node, CommentCheckFlags.Leading)`, the predicate `union-type.js` splits
    /// on, asked so that it agrees with what THIS printer emits rather than only with
    /// where the source put the bytes.
    ///
    /// Two regions answer it, and they answer differently. The member's own paren **shell**
    /// prints its leading run glued whatever the authoring (`| (/* c */ a` strips to
    /// `| /* c */ a`), so any comment there counts. In a LATER member's **union gap** only
    /// the INLINE half does: the layout relocates an own-line gap comment ABOVE the `| `
    /// ([`Self::union_gap_inline_run_start`] draws exactly that split), where the reparse
    /// reads it as the PREVIOUS member's trailing run — so counting it would make this
    /// predicate contradict its own output one pass later. That is not hypothetical: it is
    /// the shape a blank injected after the `|` produces (`| z⏎|⟨blank⟩ /* c */ a // t`,
    /// found by `blanks:audit`), where the comment is past the `|` in source and above it
    /// in the output.
    fn union_member_has_leading_comments(&self, union: &TSUnionType<'_>, i: usize) -> bool {
        let member = &union.types[i];
        let member_start = member.span().start;
        if self
            .has_comments_to_emit_between(member_start, unwrap_parenthesized(member).span().start)
        {
            return true;
        }
        // The same run one link DOWN, at the member's leading printed EDGE
        // (`| ((⏎// c⏎B)[] // t⏎)`): the shells strip, so it prints ahead of the member's
        // first code token just as the member's own shell's would, and the reparse reads
        // it from the member's leading gap ([`Printer::leading_edge_printed_run`]). The
        // window above cannot see it — it closes at the UNWRAPPED member, which for an
        // edge shell is the suffixed type, whose own start is already past the run.
        //
        // Split like the union gap below and for the same reason: a LATER member's
        // own-line comment is relocated ABOVE the `| `, where pass 2 reads it as the
        // previous member's trailing run, so counting it would make this predicate
        // contradict its own output one pass later.
        if let Some(edge) = self.leading_edge_printed_run(member) {
            let run: CommentVec<'_> = self
                .comments_to_emit_between(edge.start, edge.end)
                .collect();
            if i == 0 || self.union_gap_inline_run_start(&run) < run.len() {
                return true;
            }
        }
        let gap_start = self.union_member_leading_start(union, i);
        if i == 0 {
            // The FIRST member's whole gap run is emitted after its `| `, own-line comments
            // included: there is no earlier member for one to be relocated onto, so the
            // split below has nothing to split.
            return self.has_comments_to_emit_between(gap_start, member_start);
        }
        let run: CommentVec<'_> = self
            .comments_to_emit_between(gap_start, member_start)
            .collect();
        self.union_gap_inline_run_start(&run) < run.len()
    }

    /// [`Self::member_hoisted_trailing_shell`] at the INTERSECTION seam, for a member at
    /// any position — the `false` is the seam's answer, not a caller's convenience.
    ///
    /// The leading-comment decline is prettier's UNION rule (`handleUnionTypeComments` +
    /// `union-type.js`'s leading split, spelled here by
    /// [`Self::union_member_has_leading_comments`]); no such handler fires for an
    /// intersection, so on the oracle side it does not apply at that seam at all. What the
    /// decline was ALSO doing — keeping `build_parenthesized_type_unwrap_doc` in play as
    /// the shell's leading-run emitter — is now
    /// [`Self::push_hoisted_member_doc`]'s, and the two are a PAIR: dropping the
    /// decline without it loses every block-leading shell's run outright
    /// ([`comments.md`](../../../../docs/comments.md) hazard 1).
    fn intersection_member_hoisted_shell<'t>(
        &self,
        t: &'t TSType<'t>,
        member_parens: fn(&TSType<'_>) -> bool,
    ) -> Option<&'t TSType<'t>> {
        self.member_hoisted_trailing_shell(t, member_parens, MemberSeam::Intersection, false)
    }

    /// Build a HOISTED intersection member into `parts` and hand back the trailing run held
    /// for the boundary that follows it — the WHOLE partition of the shell, in one place
    /// because it is one partition.
    ///
    /// A hoisted shell splits four ways, and every part is a comment that must print
    /// exactly once:
    ///
    /// - the shell's **leading run**, ahead of the member. The hoist builds the member from
    ///   [`unwrap_parenthesized`], which drops the shell and every comment in its leading
    ///   gap, so a site that skipped this DROPPED it
    ///   ([`comments.md`](../../../../docs/comments.md) hazard 1) — measured at 12 of a
    ///   320-case leading-comment sweep, every one a `/* c */` the author wrote inside the
    ///   parens. The window is [`paren_shell_gaps`]' deep leading half (redundant layers
    ///   strip as a unit) and the emission is
    ///   [`Printer::push_paren_shell_leading_run`] — the same one the stripped shell would
    ///   have used itself, so separators and author blanks read identically whichever side
    ///   of the hoist prints it. It stands down where an ENCLOSING gap already claimed the
    ///   run ([`Printer::shell_leading_run_claimed`]), which is also why the two halves of
    ///   the leading region need no separate rules: a `//`-leading shell is claimed
    ///   upstream (a type alias's `=` gap) and a block run never is;
    /// - the **member**, `member_doc`, built by the caller because the compact and
    ///   forced-multiline layouts build it differently;
    /// - the trailing run's **inline prefix**, which stays on the member's own side of the
    ///   `&` the caller emits next ([`Self::intersection_hoisted_run_split`]);
    /// - the **deferred tail**, returned rather than pushed, because only the boundary
    ///   FOLLOWING the member can queue it at the indent it will flush at.
    ///
    /// ⚠️ **Three sites take this hoist** — the compact path's first member, the
    /// forced-multiline path's first member, and every later member's body — and a site
    /// that forgets a part is a DROP while one that repeats a part is a DOUBLE-PRINT
    /// (hazards 1 and 3). Spelled once, neither is expressible.
    fn push_hoisted_member_doc(
        &self,
        parts: &mut DocBuf,
        member: &TSType<'_>,
        inner: &TSType<'_>,
        member_doc: DocId,
    ) -> DocId {
        if let Some(shell) = outermost_paren(member) {
            let (leading, _) = paren_shell_gaps(shell);
            if !self.shell_leading_run_claimed(shell.span.start, leading.end) {
                self.push_paren_shell_leading_run(
                    parts,
                    leading.start,
                    leading.end,
                    ShellLeadingRun::Here,
                );
            }
        }
        parts.push(member_doc);
        let (prefix, run) = self.intersection_hoisted_run_split(member, inner);
        parts.extend(prefix);
        run
    }

    /// [`Self::intersection_member_hoisted_shell`] for an intersection's FIRST member,
    /// skipped outright on a comment-free intersection — the caller's own window search
    /// (`has_comments`) has already proved no gap holds anything, and this would otherwise
    /// be two comment searches on every intersection printed. The forced-multiline path
    /// passes `true`: it is reached only because a comment is in play.
    fn intersection_first_hoisted_shell<'t>(
        &self,
        intersection: &'t TSIntersectionType<'t>,
        member_parens: fn(&TSType<'_>) -> bool,
        has_comments: bool,
    ) -> Option<&'t TSType<'t>> {
        if !has_comments {
            return None;
        }
        self.intersection_member_hoisted_shell(&intersection.types[0], member_parens)
    }

    /// Prettier's `hasLeadingOwnLineComment(originalText, node)` disjunct
    /// (`printIntersectionType`'s "no object is involved" arm), asked for the INLINE loop
    /// over the one region its router cannot see: the PREVIOUS member's paren shell.
    ///
    /// ⚠️ **The blind spot is a fact about tsv's AST, not about the rule.** The router's
    /// window runs between the member SPANS
    /// ([`Self::intersection_has_isolated_member_comment`]), and tsv KEEPS the
    /// `TSParenthesizedType` node — so a run the author wrote inside the shell falls before
    /// the paren's span end, the between-members window is empty, and the whole
    /// forced-multiline path (which spells this same rule as its own `leading_run_ends_line`)
    /// is never routed to. Prettier's parser drops the paren before printing, so the same
    /// comments simply lead the next member and its existing disjunct sees them. Widening
    /// the router's window to the member's PRINTED end would answer it there instead; asked
    /// here, the rule stays where prettier asks it and no other gap scan's window moves.
    ///
    /// **This is the question prettier asks of the SOURCE, not of what tsv did with the
    /// run.** A member whose pair EXPANDS keeps the run between its own `(`…`)`
    /// (`a & (⏎| b⏎| c // c1⏎// c2⏎) & { x: X }`) and the boundary still opens, because
    /// prettier — which has no pair there at all — opens it too. Only a **retained** shell
    /// declines: it ends no line at the boundary because nothing follows it that the run
    /// could lead ([`Self::paren_shell_retains_for_trailing_run`], the same predicate the
    /// strip itself is decided by). The window is the DEEP gap ([`paren_shell_gaps`]),
    /// redundant layers stripping as a unit.
    ///
    /// ⚠️ **`hasLeadingOwnLineComment` is asked of a LEADING run, so a comment on the
    /// member's OWN line is not in it.** That comment is the member's trailing one — it
    /// binds above, not below — and prettier's arm chain never sees it here, so an
    /// object-adjacent boundary hugs and the run rides out to the statement's tail, which
    /// is prettier's answer at every position (`({ x: X } // c⏎) & C` → `{ x: X } & C; // c`).
    /// Only a comment that also STARTS a line can lead the member below
    /// ([`Printer::comment_follows_content_on_its_line`]) — which is exactly the filter the
    /// forced-multiline path's own `leading_run_ends_line` already gets for free, its
    /// `own_line_leading` collection having dropped the `&`-line prefix before the
    /// predicate runs. Asked without it, the two paths spelled one rule two ways and the
    /// single-comment run opened a boundary prettier hugs.
    fn intersection_boundary_leading_run_ends_line(&self, prev: &TSType<'_>) -> bool {
        let Some(shell) = self.boundary_relocating_shell(prev) else {
            return false;
        };
        let (_, trailing) = paren_shell_gaps(shell);
        self.comments_to_emit_between(trailing.start, trailing.end)
            .any(|c| !self.comment_follows_content_on_its_line(c) && !self.comment_hugs_next(c))
    }

    /// The paren shell a boundary may have to open for: one the trailing-run rule
    /// STRIPS, so whatever it holds is RELOCATED by the printer rather than kept between
    /// the author's own parens.
    ///
    /// The shared decline of the two predicates around it, because it is one sentence for
    /// both regions — a bare member has no shell to relocate out of, and a RETAINED shell
    /// prints its own `(`…`)` with the run still inside, so nothing moves and no boundary
    /// has to make room. Spelled twice it would be two chances to disagree about which
    /// shells relocate, which is exactly the question both predicates exist to answer.
    fn boundary_relocating_shell<'t>(
        &self,
        member: &'t TSType<'t>,
    ) -> Option<&'t TSParenthesizedType<'t>> {
        outermost_paren(member).filter(|shell| !self.paren_shell_retains_for_trailing_run(shell))
    }

    /// The **other** region prettier's `hasLeadingOwnLineComment(originalText, node)`
    /// covers and tsv's router cannot see: the member's OWN paren shell's leading gap.
    ///
    /// Prettier's parser drops the paren, so a run the author wrote just inside it simply
    /// leads the member and its existing disjunct sees it. tsv keeps the
    /// `TSParenthesizedType`, so that run falls INSIDE the member's span and the
    /// between-members window ([`Self::intersection_has_isolated_member_comment`]) is
    /// empty — the sibling blind spot to
    /// [`Self::intersection_boundary_leading_run_ends_line`]'s, one member over. The
    /// printer relocates the run ahead of the member all the same, so a boundary that
    /// hugs past it drops the member onto a line one level shallower than every other
    /// continuation, and the reparse — which now reads the run out of the operator gap,
    /// where its own isolation forces the break — puts it back. Two passes, two forms
    /// (`Z & (// L⏎↹{ x: X } // c1⏎↹// c2⏎) & C`).
    ///
    /// ⚠️ **No `comment_follows_content_on_its_line` filter here, unlike the sibling.**
    /// That filter is what keeps a `//` on the PREVIOUS member's own line read as that
    /// member's trailing comment; everything in this gap leads the member by
    /// construction, which is what makes it prettier's question directly.
    ///
    /// ⚠️ **And no run-indent constraint.** The sibling carries one because a lifted
    /// TRAILING run is a deferred `line_suffix` that renders at the indent it was QUEUED
    /// at. A leading run is real hardlines inside the member's own parts, so it moves with
    /// whatever indent the boundary chooses and cannot be opened at the wrong one.
    ///
    /// A shell the trailing-run rule RETAINS declines for the sibling's reason, which is
    /// why the two share it ([`Self::boundary_relocating_shell`]): its run prints between
    /// the author's own parens, so nothing is relocated.
    fn intersection_member_shell_leading_run_ends_line(&self, cur: &TSType<'_>) -> bool {
        let Some(shell) = self.boundary_relocating_shell(cur) else {
            return false;
        };
        let (leading, _) = paren_shell_gaps(shell);
        self.comments_to_emit_between(leading.start, leading.end)
            .any(|c| !self.comment_hugs_next(c))
    }

    /// Prettier's `hasLeadingOwnLineComment(originalText, node)` disjunct on
    /// `printIntersectionType`'s "no object is involved" arm — **re-applied to whatever the
    /// arm chain hugged**, and asked over both paren-shell regions tsv's AST hides from the
    /// router, as the one question it is: *must this boundary open for a relocated shell
    /// run?*
    ///
    /// Prettier's chain answers a both-objects boundary BEFORE this predicate is ever asked,
    /// so the same question has to be put again to whatever hugged; and both regions fall
    /// INSIDE a member's span (tsv keeps the `TSParenthesizedType`), so
    /// [`Self::intersection_has_isolated_member_comment`]'s between-members window sees
    /// neither. A run that ends a line has no line to end on a boundary that hugs, so it
    /// rides out to the statement's own tail — past a `;` the reparse cannot re-break.
    ///
    /// The two regions carry different qualifiers, which is the whole reason they are one
    /// function rather than two call sites:
    ///
    /// - the PREVIOUS member's TRAILING gap ([`Self::intersection_boundary_leading_run_ends_line`])
    ///   is a lifted run — a deferred `line_suffix` rendering its break at the indent it was
    ///   QUEUED at — so it opens the boundary only where that indent already matches
    ///   (`run_indent_follows_boundary`), or one non-idempotency is traded for another;
    /// - this member's own LEADING gap ([`Self::intersection_member_shell_leading_run_ends_line`])
    ///   is real hardlines inside the member's own parts, so it moves with whatever indent
    ///   the boundary chooses and carries no such qualifier.
    ///
    /// ⚠️ **Both loops ask this, and that is what the one function is for.** The compact and
    /// forced-multiline paths are twins, and a boundary rule held at one of them lets a
    /// single authoring reach two fixed points depending on whether some unrelated gap
    /// happens to carry an isolated comment — the standing hazard this module keeps hitting,
    /// and exactly how the second region went ungraded on the multiline route.
    fn intersection_boundary_opens_for_shell_run(
        &self,
        prev: &TSType<'_>,
        cur: &TSType<'_>,
        run_indent_follows_boundary: bool,
    ) -> bool {
        (run_indent_follows_boundary && self.intersection_boundary_leading_run_ends_line(prev))
            || self.intersection_member_shell_leading_run_ends_line(cur)
    }

    /// Where a union member's own leading region opens: the union's span start for the
    /// FIRST member, just past the `|` separator for every later one.
    ///
    /// The separator is the boundary because prettier's `handleUnionTypeComments` binds
    /// every comment in a union to its PRECEDING member as a trailing comment, so what
    /// sits before the `|` belongs to the member above and must not decide anything about
    /// this one (`z /* p */ | (a // c⏎// c2) | c` hoists, and prettier agrees). With no
    /// separator between them — the leading-`|`-less first member, or a malformed gap —
    /// the previous member's end is the conservative fallback.
    fn union_member_leading_start(&self, union: &TSUnionType<'_>, i: usize) -> u32 {
        if i == 0 {
            return union.span.start;
        }
        let prev_end = union.types[i - 1].span().end;
        find_separator_position(self.source, prev_end, union.types[i].span().start, b'|')
            .map_or(prev_end, |sep| sep + 1)
    }

    /// Build a Doc for a union type: `A | B | C` or `| A\n| B\n| C`
    ///
    /// When flat: `A | B | C`
    /// When broken: each type on its own line with leading `| `
    ///
    /// The broken-member doc is **always its own group** — Prettier's
    /// `printed = group(members)` (`union-type.js`). The caller owns the outer
    /// wrapper (the hang/`indent([softline, …])` that supplies the break after
    /// `=` / `:` / `as` / `extends` / a conditional branch) and nests this group
    /// inside it. Because the members form their own group, a union broken from
    /// its parent first re-fits on the indented continuation line
    /// (`type X =\n\tA | B | C`) and only explodes to leading-pipe members
    /// (`| A\n| B`) when that continuation line *also* overflows — Prettier 3.9's
    /// "don't break union type when it can fit" (#18827). Before 3.9 a parent
    /// break dragged the members straight to the leading-pipe form.
    ///
    /// The hug path (`{ … } | null`) and the line-comment path return bare,
    /// ungrouped docs — they have no flat/broken choice to make (the object owns
    /// its own expansion; line comments force multiline).
    /// A redundant paren shell around a member the next `|` separator follows can
    /// strip and let its trailing line comment trail that member — the per-member
    /// break ends the line right after it, the lossless carve-out of the preserve
    /// rule (§Comment Position Philosophy). The shell answers that structurally
    /// (`Printer::type_member_separator_follows`), so the LAST member — whose line
    /// ends only at the statement's tail — retains its shell instead.
    pub(in crate::printer) fn build_union_type_doc(&self, union: &TSUnionType<'_>) -> DocId {
        self.build_union_type_doc_inner(union, UnionLeadingGap::Other)
    }

    /// Build a union VALUE doc for an operator seam (`=` / `:` / `=>` at `gap_start`),
    /// handing the gap's glued leading block run into the union when
    /// [`Self::union_external_leading_run_start`] accepts — the value-seam analog of the
    /// intersection's [`LeadingGap`] claim. With no authored leading `|` the union's span
    /// starts at its first member, so a run between the operator and the member is
    /// invisible to the in-span first-member gap — printed by the caller, it lands AHEAD
    /// of the `|` the broken layout synthesizes (`/* c */ | A`), a position prettier never
    /// produces (it binds the comment to the first member: `| /* c */ A`), while the same
    /// program with the `|` authored already prints prettier's form — two fixed points
    /// keyed on pure layout. Handed in, the first-member arm places the run after its
    /// `if_break` pipe: the flat render keeps the caller's bytes (`/* c */ A | B`), the
    /// broken one matches prettier. The paths that never synthesize a first `| ` on a
    /// fresh line (single-member collapse, the line-comment layout) emit the run glued
    /// ahead of the union — the caller's legacy position, byte-identical.
    ///
    /// The result carries whether the run was handed in — the caller emits the gap's
    /// comments itself ONLY when it was not: exactly one of the two prints the run
    /// (docs/comments.md hazard 3) — and
    /// whether the doc prints **hugged**, the one answer the seam's own layout must
    /// agree with — read here, beside the doc, so a seam cannot pair the doc with a
    /// re-derivation: at a value seam a glued block ahead of the first member declines
    /// the hug ([`UnionLeadingGap::ValueSeam`]), handed in or authored after the pipe,
    /// which the bare [`Self::union_prints_hugged`] does not see.
    pub(in crate::printer) fn build_union_value_doc(
        &self,
        gap_start: u32,
        union: &TSUnionType<'_>,
    ) -> UnionValueDoc {
        let handed = self.union_external_leading_run_start(gap_start, union);
        let gap = UnionLeadingGap::ValueSeam { gap_start, handed };
        UnionValueDoc {
            doc: self.build_union_type_doc_inner(union, gap),
            run_handed: handed.is_some(),
            hugged: self.union_prints_hugged_with(union, gap),
        }
    }

    /// The caller-side gate for [`Self::build_union_value_doc`]:
    /// `Some(run start)` when `[gap_start, first member)` holds a to-emit run the
    /// union should print instead of the caller. Declines — the caller keeps its
    /// own seam — when:
    ///
    /// - a leading `|` is authored (`union.span.start != first.start`): the gap up
    ///   to the pipe is the caller's, the gap after it the in-span machinery's;
    /// - the run holds a line comment (those routes own mandatory-break layouts);
    /// - the run is not GLUED to what follows it (the member, or an owned comment
    ///   leading it — the physical next, not the emit-set's view): a broke-after
    ///   run is the value seam's own break-materialization question, not this
    ///   binding one;
    /// - a leading-run freeze is active: the directive's placement belongs to the
    ///   freeze machinery, and moving it past a synthesized `|` would flip it
    ///   trailing and lose the freeze on the next pass.
    fn union_external_leading_run_start(
        &self,
        gap_start: u32,
        union: &TSUnionType<'_>,
    ) -> Option<u32> {
        let first_start = union.types.first()?.span().start;
        if union.span.start != first_start {
            return None;
        }
        let comments: CommentVec<'_> = self
            .comments_to_emit_between(gap_start, first_start)
            .collect();
        let (first, last) = (comments.first()?, comments.last()?);
        if !comments.iter().all(|c| c.is_block)
            || !self.is_same_line(last.span.end, self.blank_scan_end_after(last, first_start))
            || self
                .composite_leading_run_freeze(union.span.start, union.types)
                .is_some()
        {
            return None;
        }
        Some(first.span.start)
    }

    /// Prepend an external leading run (see
    /// [`Self::build_union_value_doc`]) in the caller's legacy
    /// glued position, for the union paths whose layout has no synthesized-pipe
    /// seam to bind it to.
    fn with_union_external_run_prefix(
        &self,
        run_start: Option<u32>,
        first_member_start: u32,
        doc: DocId,
    ) -> DocId {
        let Some(start) = run_start else {
            return doc;
        };
        let (run, _) = self.build_member_leading_block_comments(start, first_member_start);
        self.d().concat(&[run, doc])
    }

    fn build_union_type_doc_inner(&self, union: &TSUnionType<'_>, gap: UnionLeadingGap) -> DocId {
        let d = self.d();
        let external_run_start = match gap {
            UnionLeadingGap::ValueSeam { handed, .. } => handed,
            UnionLeadingGap::Other => None,
        };
        if union.types.is_empty() {
            return d.empty();
        }
        let first_member_start = union.types[0].span().start;

        // Format-ignore leading run (Rule A): an alone-on-line directive in the
        // out-of-span region before the union freezes its first member. Gated on the
        // document-level flag — the in-span `has_comments` gate below cannot see an
        // out-of-span directive. `freeze_first` is applied in the main loop and the
        // single-member branch.
        let leading_freeze = self.composite_leading_run_freeze(union.span.start, union.types);
        let (freeze_first, freeze_first_multiline) =
            LeadingRunFreeze::first_member_flags(leading_freeze);

        // Single-member-union collapse under a freeze. A 1-element union drops its `|`
        // when reformatted, so a member-only freeze is non-idempotent — pass 2 sees a
        // bare member no longer routed through the union (`| {a:1}` → `{a:1}` → `{ a: 1 }`).
        // The 1-element union is TRANSPARENT for directive binding: if the sole member is
        // itself a Union/Intersection, fall through and build it normally so its own Rule A
        // applies inside (`| a1&a2` → `a1 & a2`, `a1` frozen, idempotent, a design_choice
        // divergence from prettier's `| a1&a2`); a leaf/object sole member freezes the
        // WHOLE union span verbatim (keeps the `|` → idempotent AND matches prettier's
        // whole-freeze). Handles the hug path too (a lone object hugs), which is why this
        // precedes it.
        if union.types.len() == 1
            && self.list_member_frozen(union.span.start, union.types, 0, freeze_first)
            && !matches!(
                unwrap_parenthesized(&union.types[0]),
                TSType::Union(_) | TSType::Intersection(_)
            )
        {
            return self.raw_source_range(union.span.start, union.span.end);
        }

        // One window search over the union gates every comment query below. All of them
        // — the leading `|`→first-member gap, the gaps either side of each separator, the
        // trailing gap, and the line-comment probes (including those that look inside a
        // parenthesized member) — are bounded inside `union.span`, and a comment only
        // counts when it lies fully inside the queried range. So a comment-free union
        // provably has none in any of them: the searches are skipped, the `empty()`
        // children they would feed into the member list are never pushed, and — the
        // larger cost — the per-separator `find_separator_position` byte scans never run.
        // Those scans exist only to bound the comment ranges; the printed `|` is static
        // text. Byte-identical, and unions are the most common non-trivial TS type.
        let has_comments = self.has_comments_on_page_between(union.span.start, union.span.end);

        // A single-member union collapses to its member — Prettier drops
        // single-element `TSUnionType`/`TSIntersectionType` nodes in postprocess
        // (`parse/postprocess/index.js`). The member prints in the union's own
        // position, so any precedence parens around a nested union/intersection
        // member fall away (`| (A | B)` → `A | B`); required parens come from the
        // union's parent context one level up. The member still flows through the
        // normal comment-aware paths so comments clinging to the `|`/parens are
        // preserved.
        let member_parens = union_member_parens(union.types.len());

        // Prettier's shouldHugUnionType: when one member is object-like and the
        // rest are void types (null, void), format as inline `A | B | C` where
        // the object type handles its own expansion.
        // Example: `{ name: string; value: number } | null` stays hugged.
        //
        // Comments disqualify the hug only when attached to a *member node* —
        // prettier bails on `types.some((t) => hasComment(t))`. In our detached
        // model those live in the gap between consecutive members. A comment
        // nested *inside* a member (e.g. `{ /* c */ a: 1 }`) attaches to a child
        // node, not the member, so it must not block the hug — the member's own
        // doc renders it.
        //
        // At a VALUE seam a block AHEAD of the first member (`: /* c */ { … } | null`,
        // handed in from the seam's gap, or `: | /* c */ { … }` after an authored pipe)
        // is such a member comment too (`gap`): the union then does not hug — it breaks
        // after the operator and, once the member overflows, takes the one-per-line form
        // with the run after the synthesized pipe (`| /* c */ {`), which is where a
        // handed run lands. The seam reads the same answer off `UnionValueDoc::hugged`.
        if self.union_prints_hugged_with(union, gap) {
            let mut parts = DocBuf::new();
            // Extract leading block comments before the first type
            // (e.g., `| /* c */ A` — comment between leading `|` and first member)
            if has_comments
                && let Some(first) = union.types.first()
                && let Some(leading) = self.build_comments_between_filtered_opt(
                    union.span.start,
                    first.span().start,
                    CommentSpacing::Trailing,
                    CommentFilter::BlockOnly,
                )
            {
                parts.push(leading);
            }
            for (i, t) in union.types.iter().enumerate() {
                if i > 0 {
                    parts.push(d.text(" | "));
                }
                // A hugged member (object / void) frozen by a directive — first member via
                // `freeze_first` or the in-span leading gap, a later member via its gap.
                // The hug path can't hold a composite member (only object-like huggables),
                // so `build_frozen_member_doc` freezes it verbatim without the collapse
                // question the len==1 branch answers below.
                let frozen =
                    self.list_member_frozen(union.span.start, union.types, i, freeze_first);
                if frozen {
                    parts.push(self.build_frozen_member_doc(t, member_parens));
                } else {
                    parts.push(self.build_type_doc_maybe_parens(t, member_parens));
                }
            }
            return self.with_union_external_run_prefix(
                external_run_start,
                first_member_start,
                d.concat(&parts),
            );
        }

        // Check for line comments that force the multiline layout:
        // - Between union members (`A | B // c\n  | C`)
        // - Before the first member (`| // c\n  A | B`)
        // - Inside a member's parens (`A | (// c\n  B)`) — a retained paren keeps the
        //   comment inside; a redundant one leads its member on its own line. Either way
        //   the comment is a line comment, so the multiline layout is required.

        // An enclosing gap may own this node's whole transparent head region — a ONE-member
        // union prints as its member, so its leading-`|` gap is that gap's, not this
        // builder's (see [`Printer::composite_head_region_claimed`]). Both the multiline
        // route below and the collapse's own leading emitter stand down under it.
        let head_region_claimed = self.composite_head_region_claimed(union.span.start, union.types);

        if has_comments && !head_region_claimed {
            let first_type_start = union.types.first().map(|t| t.span().start);
            let has_leading_line_comments = first_type_start
                .is_some_and(|start| self.has_line_comments_between(union.span.start, start));
            // A line comment inside a member's parens (before the — possibly nested —
            // inner type), matching the window `build_union_type_doc_with_line_comments`
            // reads for its retained-paren, stripped-redundant-paren and leading-edge arms.
            //
            // ⚠️ Asked only of a MULTI-member union. With one member the union collapses to
            // that member below, so a `//` in the member's own shell is a question about
            // the MEMBER — which parens it keeps and where its run lands — and the answer
            // is the enclosing gap's, reached through the leading-edge seam. Routed here
            // instead, the multiline layout emitted a `| ` the collapse would have dropped
            // and asked the retained-paren rule as if the union had two members, so the
            // pair came back retained and the pipe fabricated purely because of the
            // comment (`single_member_union_shell_line_comment`).
            let has_paren_inner_leading_line_comments = union.types.len() > 1
                && union
                    .types
                    .iter()
                    .any(|t| self.union_member_paren_leading_line_comment(t));
            if has_leading_line_comments
                || self.union_has_own_line_member_comment(union)
                || has_paren_inner_leading_line_comments
            {
                return self.with_union_external_run_prefix(
                    external_run_start,
                    first_member_start,
                    self.build_union_type_doc_with_line_comments(union),
                );
            }
        }

        // A single-member union has no `|` of its own: prettier drops single-element
        // `TSUnionType` nodes in postprocess, so the lone member prints in the union's
        // position with NO leading pipe and NO per-member offset. Rendering it
        // transparently collapses a nested `| (| (| A | B))` to the innermost
        // multi-member union (`| A | B`) instead of stacking a leading `|` per level —
        // the flat form already collapses, but the loop below emits each level's
        // `if_break("| ")` + offset once a nested comment forces the union multiline.
        // Placed after the hug/line-comment paths so a leading line comment (which the
        // block-only comment helper can't carry) still routes there. A block comment
        // between the dropped `|` and the member is preserved. `member_parens` is the
        // single-member predicate here, so any required parens come from the parent one
        // level up.
        if union.types.len() == 1 {
            let member = &union.types[0];
            // A single-member union collapses to its member. When frozen, the resolution
            // is handled above, at the leaf/object arm of the `len() == 1` branch (whole
            // union frozen, `|` kept); a composite sole member falls through here and
            // builds normally so its OWN leading-run walk applies Rule A inside — the
            // transparency doctrine.
            if !has_comments {
                return self.with_union_external_run_prefix(
                    external_run_start,
                    first_member_start,
                    self.build_type_doc_maybe_parens(member, member_parens),
                );
            }
            // Block comments only, and that is complete: a `//` in this gap routed to
            // the multiline layout above. The run's own break flag is dropped — the
            // union's own group decides its `line`; the flag is the caller's only where
            // no group is guaranteed (the intersection's boundaries). Under an enclosing
            // gap's head-region claim the run is that gap's, so this branch adds nothing.
            let leading = if head_region_claimed {
                d.empty()
            } else {
                self.build_member_leading_block_comments(union.span.start, member.span().start)
                    .0
            };
            let member_doc = self.build_type_doc_maybe_parens(member, member_parens);
            return self.with_union_external_run_prefix(
                external_run_start,
                first_member_start,
                d.concat(&[leading, member_doc]),
            );
        }

        // Build parts: each type prefixed conditionally with `| ` or nothing
        // Flat: T1 | T2 | T3
        // Break: | T1
        //        | T2
        //        | T3
        let mut parts = d.pooled_docbuf();

        // A multi-line frozen member forces the broken one-member-per-line layout (Rule A
        // must-break): a frozen slice is `will_break`-opaque, so the force is explicit.
        // Seeded by the leading-run freeze; the loop ORs in any other frozen multi-line
        // member.
        let mut freeze_multiline = freeze_first_multiline;

        for (i, t) in union.types.iter().enumerate() {
            let type_start = t.span().start;
            let type_end = t.span().end;

            // Rule A member freeze: the first member via a leading-run directive
            // (`freeze_first`, before `span.start`) or an alone-on-line directive in the
            // in-span leading gap after the `|` (`|⏎// prettier-ignore⏎member`). A later
            // member's alone-on-line directive routes the union to the line-comment path
            // before this loop (the one-sided `union_has_own_line_member_comment` gate —
            // broader than the placement floor), so the i>0 half of this ask is expected
            // never to fire here; it stays for the one-predicate discipline.
            let frozen = self.list_member_frozen(union.span.start, union.types, i, freeze_first);
            if self.frozen_member_forces_break(frozen, t, member_parens) {
                freeze_multiline = true;
            }

            if i > 0 {
                // Between types: newline + "| " when broken, " | " when flat
                // Use if_break with line() instead of hardline() to avoid triggering will_break
                parts.push(d.if_break(d.concat(&[d.line(), d.text("| ")]), d.text(" | ")));

                // Add leading block comments for this type (after the `|` separator).
                // Uses a hardcoded trailing space (not the source-aware separator
                // used for the FIRST member): a block comment between two members
                // is a separate, pre-existing divergence — Prettier relocates it to
                // trail the previous member (`| a /* c */`), the block-comment analog
                // of `union_infix_pipe_line_comment`. tsv keeps it leading this
                // member; changing the separator here would only reshape that
                // already-divergent form, not match Prettier.
                if has_comments {
                    let prev_type_end = union.types[i - 1].span().end;
                    self.push_post_separator_block_comments(&mut parts, prev_type_end, type_start);
                }
            } else {
                // A FROZEN first member emits its leading block comments (the own-line
                // directive) BEFORE the `| ` so the directive stays own-line and
                // re-recognizes on pass 2; emitting it after `| ` (the general path below)
                // relocates it to trail the pipe (`| /* prettier-ignore */`), flipping it
                // trailing and losing the freeze next pass. The own-line block's own
                // hardline forces the group broken, so the `if_break` `| ` appears.
                if has_comments && frozen {
                    let (run, _) =
                        self.build_member_leading_block_comments(union.span.start, type_start);
                    parts.push(run);
                }

                // First type: "| " when broken, nothing when flat
                parts.push(d.if_break(d.text("| "), d.empty()));

                // Extract leading block comments before the first type
                // (e.g., `| /* c */ A | B` — comment between leading `|` and first member).
                //
                // `align(2)` for the same reason as the line-comment path's run: when
                // this run ends in a break — an own-line multi-line block, or its soft
                // `line` breaking as the union expands — it is the run that places the
                // member's own first line, which then belongs at the per-member offset
                // rather than flush under the `|`. It takes the SAME `align(2)` sub-tab
                // offset as the member (below) so the run's lines and the member's align
                // consistently; splitting the offset across the two siblings is sound
                // because `align` is a per-line property. Unconditional because it binds
                // only the breaks inside it, so a run that hugs its member is unaffected.
                // A frozen first member emitted its run before the `| ` above.
                if has_comments && !frozen {
                    let (run, _) =
                        self.build_member_leading_block_comments(union.span.start, type_start);
                    parts.push(d.align(2, run));
                }

                // An EXTERNAL glued run handed in from the value seam takes the same
                // position — after the `if_break` pipe, bound to the member it leads
                // (`| /* c */ A` when broken, `/* c */ A | B` flat). Mutually
                // exclusive with the in-span run above by construction: the seam only
                // hands a run in when no leading `|` is authored, which is exactly
                // when the in-span gap `[span.start, first.start)` is empty.
                if let Some(start) = external_run_start {
                    let (run, _) = self.build_member_leading_block_comments(start, type_start);
                    parts.push(d.align(2, run));
                }
            }

            // Apply Prettier's per-member `align(2, …)` offset (a sub-tab alignment —
            // see `build_union_member_offset_doc`). The first member's leading run takes
            // that offset separately, above: the run is aligned, never this call's
            // result, so the object-literal and default-paren members that supply their
            // own indent keep declining it.
            //
            // Rule A member freeze: emit the member verbatim (paren-transparent)
            // instead of reformatting it. Same offset shape, so it aligns with the
            // reformatted siblings in the broken layout.
            if frozen {
                parts.push(self.build_frozen_union_member_offset_doc(t, member_parens));
            } else {
                // A comment-free union pays none of the hoist's questions: the one window
                // search above (`has_comments`) already proved no gap in this union holds
                // anything, and a union is the most common non-trivial TS type — the same
                // argument every other comment query in this builder rides on.
                parts.push(if has_comments {
                    // An EXTERNAL run handed in from the value seam leads this member as
                    // surely as an in-span one (it exists only for the first member, where
                    // it is emitted above), and it is always glued after the `| `, so it
                    // counts whatever the source spelling.
                    let has_leading = (i == 0 && external_run_start.is_some())
                        || self.union_member_has_leading_comments(union, i);
                    self.build_union_member_doc(t, member_parens, has_leading)
                } else {
                    self.build_union_member_offset_doc(t, member_parens)
                });
            }

            // Add trailing block comments after this type (before the next `|` separator)
            if has_comments {
                if i + 1 < union.types.len() {
                    let next_type_start = union.types[i + 1].span().start;
                    self.push_pre_separator_block_comments(
                        &mut parts,
                        type_end,
                        next_type_start,
                        b'|',
                    );
                } else if let Some(trailing) = self.build_comments_between_filtered_opt(
                    // Last type - include all trailing comments up to union span end
                    type_end,
                    union.span.end,
                    CommentSpacing::Leading,
                    CommentFilter::BlockOnly,
                ) {
                    parts.push(trailing);
                }
            }
        }

        // Always group the broken-member doc (Prettier's `printed = group(members)`).
        // The group makes the union's own flat/broken decision independently of the
        // parent's break, so it re-fits on the continuation line before exploding.
        //
        // A multi-line frozen member forces the broken one-member-per-line layout
        // (Rule A must-break): the frozen slice is a `will_break`-opaque verbatim span,
        // so the break is forced explicitly here rather than propagating from the slice.
        // A single-line frozen member keeps the width-decided layout.
        if freeze_multiline {
            d.group_break(d.concat(&parts))
        } else {
            d.group(d.concat(&parts))
        }
    }

    /// Hanging-indent layout for a union used in a position where Prettier's
    /// `printUnionType` applies `shouldIndentUnionType` — an `as`/`satisfies`
    /// cast type, or a type-parameter `extends` constraint / `=` default. The
    /// union breaks after the keyword with leading-pipe members indented one
    /// level:
    ///
    /// ```text
    /// value as
    ///     | A
    ///     | B
    /// ```
    ///
    /// Returns `None` when the type is not an indentable union — hugging unions
    /// (`{ ... } | null`, which expand the object member inline) and non-union
    /// types use the caller's default inline layout.
    pub(in crate::printer) fn build_union_hanging_indent_doc(
        &self,
        ty: &TSType<'_>,
    ) -> Option<DocId> {
        let TSType::Union(union) = ty else {
            return None;
        };
        // `union_prints_hugged`, not the bare syntactic `union_hug_shape`: this
        // must agree with the layout `build_union_type_doc` will actually take. A comment
        // can make it decline the hug and expand, and then the keyword has to break like
        // any other non-hugging union — asking the syntactic form alone keeps `as ` glued
        // while the members explode below it.
        if self.union_prints_hugged(union) {
            return None;
        }
        // The union members form their own group (`build_union_type_doc`), nested
        // inside the hang's `group(indent([line, …]))`. When the hang breaks after
        // the keyword, the member group still re-fits on the indented continuation
        // line (`as\n\tA | B | C`) before exploding to leading-pipe members.
        let union_doc = self.build_union_type_doc(union);
        Some(hang_after_operator(self.d(), union_doc))
    }

    /// The intersection counterpart to a hanging operator layout: the first member
    /// hugs the operator and continuation members wrap one level in
    /// (`A &\n\tB &\n\tC`). Shared by the type-alias RHS and `as`/`satisfies` cast
    /// intersection arms. The bare printer owns both the continuation indent (the
    /// first member stays at base — a first member that breaks internally is not
    /// double-indented) and, via its `needs_group` flag, the group. A boundary type
    /// that owns its own expansion (TypeLiteral/Mapped at the first or last position)
    /// opts out of the group (`wrap_in_group = false`) so it isn't re-wrapped.
    pub(in crate::printer) fn intersection_hanging_with_indent(
        &self,
        intersection: &TSIntersectionType<'_>,
    ) -> DocId {
        let boundary_owns_expansion = intersection_has_huggable_last_type(intersection)
            || intersection_has_expanding_first_type(intersection);
        // A hanging caller trails a prefix (`= …`), so the hoist keeps its continuation indent.
        self.build_intersection_type_doc(intersection, !boundary_owns_expansion, false)
    }

    /// Build a Doc for a union type with line comments between members.
    ///
    /// Line comments force the union to be multiline because a line comment
    /// cannot be followed by content on the same line.
    ///
    /// Structure:
    /// ```text
    /// | A
    /// // comment before B
    /// | B
    /// ```
    fn build_union_type_doc_with_line_comments(&self, union: &TSUnionType<'_>) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        let member_parens = union_member_parens(union.types.len());

        // Rule A first-member freeze in the forced-multiline path (an in-span comment
        // routed here, plus a leading-run alone-on-line directive before the union).
        // Recomputed rather than threaded — gated on `has_format_ignore`, so it costs
        // nothing in the common case.
        let freeze_first = self
            .composite_leading_run_freeze(union.span.start, union.types)
            .is_some();

        for (i, t) in union.types.iter().enumerate() {
            let type_start = t.span().start;
            let type_end = t.span().end;

            // A LATER member that is a REDUNDANT parenthesized type (`a | (// c⏎ b)`): its
            // leading line comment can't stay "inside" parens the comment-free rule strips,
            // so it leads the member on its own line before the `| ` (emitted below,
            // rendered from the stripped inner). The wide collector also hoists a mixed
            // (`(/* b */ // c⏎ b)`) or trailing (`(// c⏎ b /* t */)`) run losslessly: the
            // leading block + line each on their own line here, the trailing comment
            // appended to the member below. Empty for a retained-paren member (whose comment
            // stays inside, the arms further down) or a block-only leading gap (stays
            // inline).
            // Rule A member freeze (paren-transparent): the first member via a leading-run
            // directive or the in-span leading gap, a later member via its own gap. Hoisted
            // here so the redundant-paren leading run below is suppressed for a frozen
            // member — its comments ride out INSIDE the frozen verbatim slice, so emitting
            // the run separately too would DOUBLE-PRINT them (`| (// c⏎ b)` frozen).
            let frozen_member =
                self.list_member_frozen(union.span.start, union.types, i, freeze_first);

            let stripped_paren_leading = if i > 0 && !frozen_member {
                self.stripped_redundant_paren_member_leading_run(t)
            } else {
                smallvec![]
            };

            // A shell at the member's leading printed EDGE rather than being the member —
            // an array suffix over it (`(⏎// c⏎B)[]`), an indexed access, an intersection
            // whose first member it is. The shell strips, so its run is physically in THIS
            // gap and the reparse finds it here; left to its own emitter it printed with a
            // bare `hardline` after the `| `, where this gap puts its own run above the
            // `| `. The window widens over the shell and the member declines its copy —
            // one emitter for one gap, on both passes.
            //
            // The union owns this even for its FIRST member, and no enclosing gap may take
            // it: the leading region here is read by four emitters across three layout
            // paths, so an outer claim over any part of it double-printed whatever the
            // author wrote between the `|` and the shell's `(`. See the ⚠️ on
            // [`Printer::head_stripped_paren_shell`], which is why a union's first member is
            // not one of its descent links.
            // Declined for a FROZEN member (whose verbatim slice already carries the
            // shell) and for one whose redundant-paren run was hoisted above — either
            // way another emitter owns those bytes.
            let run_owned_above = frozen_member || !stripped_paren_leading.is_empty();
            let (edge_claim, gap_end) = self.leading_edge_claim_and_start(run_owned_above, t);

            // The **first** member's leading-comment run, built here rather than at the
            // `| ` prefix below so the arm that emits it decides whether it takes the
            // per-member offset — the paren-union arm declines it, the general arm takes
            // it. Both block and line comments are emitted from here; a line comment
            // requires multiline and places the member on the next line (`| // c⏎  A`).
            //
            // ⚠️ Empty for every later member, and the arm chain below consumes it by
            // move — an arm that neither extends nor inspects it is a **dropped comment**
            // (`comments:audit` is the corpus-wide guard).
            //
            // `None` for `skip_delim`: the union's leading `|` is not run through
            // `delimiter_line_comment_prefix`, unlike the bracket/angle/paren lists, so
            // no comment was pulled onto a delimiter line to exclude here.
            let mut first_leading = if i == 0 {
                self.build_leading_comments_multiline(union.span.start, gap_end, None)
            } else {
                DocBuf::new()
            };

            if i > 0 {
                // Get previous type end and find the pipe position
                let prev_type_end = union.types[i - 1].span().end;

                // Collect comments between previous type and this type's pipe
                if let Some(pipe_pos) =
                    find_separator_position(self.source, prev_type_end, type_start, b'|')
                {
                    // Comments before the pipe (trailing on previous type's line or on
                    // own lines). A same-line line comment is line_suffix'd (zero width)
                    // so it can't force the previous member to break — the leading-`|`
                    // form puts the next separator on a new line, where it flushes.
                    //
                    // The run takes the PREVIOUS member's per-member `align(2)` when that
                    // member carries leading comments, and sits flush under the `|`
                    // otherwise — prettier's `hasComment(node, Leading)` split
                    // (`union-type.js`), asked here of the member the run trails because
                    // that is the member prettier's `handleUnionTypeComments` binds it to.
                    // The two arms are one program's two authorings of the same comment:
                    // the flush form is what the lifted-shell run settles to
                    // ([`Self::member_hoisted_trailing_shell`]), and the aligned form is
                    // what a shell with a leading comment settles to — so a gap emitter
                    // that answered only one of them made the other non-idempotent.
                    let gap_run = self.build_trailing_gap_comments(prev_type_end, pipe_pos);
                    if self.union_member_has_leading_comments(union, i - 1) {
                        parts.push(d.align(2, d.concat(&gap_run)));
                    } else {
                        parts.extend(gap_run);
                    }

                    // Comments after the pipe lead this member. Line comments (and
                    // own-line block comments) go on their own line BEFORE the `| `
                    // separator so the pipe stays attached to the type
                    // (`| A\n// c\n| B`). Inline block comments stay after `| `
                    // (`| /* c */ B`). Prettier instead relocates such comments to
                    // trail the previous member — see
                    // union_infix_pipe_line_comment_prettier_divergence.
                    //
                    // The partition keys on the comment's OWN placement, not only its
                    // forward glue: an own-line-authored block glued forward to the
                    // member (`a |⏎/* c */ {x:1}`) stays own-line — filtering it out on
                    // `comment_hugs_next` alone emitted it glued after `| `, and the
                    // next pass (seeing no own-line comment) collapsed the union flat —
                    // a 2-pass oscillation (the `|⟨⟩␣` blank-audit shape; prettier's
                    // fixed point keeps it own-line, and that form is stable here).
                    // A comment glued on BOTH sides still takes the post-`| ` path.
                    //
                    // ⚠️ The two sides bracket the `| `, so the split is a PREFIX/SUFFIX
                    // partition of the run — see [`Self::union_gap_inline_run_start`].
                    let after_pipe = pipe_pos + 1;
                    let run: CommentVec<'_> =
                        self.comments_to_emit_between(after_pipe, gap_end).collect();
                    let inline_start = self.union_gap_inline_run_start(&run);
                    let (own_line, inline) = run.split_at(inline_start);
                    // A blank line the author left *before* the first own-line comment
                    // (`A |⏎⏎/* c */⏎B`) and *between* two own-line comments is preserved,
                    // matching prettier — but NOT one after the last comment before the
                    // member (prettier emits none there). This mirrors the intersection
                    // own-line path with the axes swapped: prettier's union and
                    // intersection printers preserve blanks in opposite member-gap
                    // positions.
                    //
                    // ⚠️ **The question is prettier's `isPreviousLineEmpty`, asked OF THE
                    // COMMENT: is the line directly above it blank?** A gap-wide scan
                    // anchored at the previous member's end read every intervening line
                    // break as the author's blank — a `|` the author gave a line of its
                    // own (`A⏎|⏎// c⏎B`) fabricated a blank prettier doesn't write, the
                    // comma-on-its-own-line failure `previous_line_is_empty`'s own doc
                    // names. Asking the comment's own neighbour also covers the
                    // earlier-comment shape (`A⏎// x⏎| // c⏎B`) the old scan needed a
                    // `blank_scan_start` compensation for: the line above the run's first
                    // comment holds that comment (or the pipe), so it is non-empty and
                    // nothing is measured across it.
                    if let Some(first) = own_line.first()
                        && self.previous_line_is_empty(prev_type_end, first.span.start)
                    {
                        parts.push(d.literalline());
                    }
                    parts.push(d.hardline());
                    // Both runs at this seam take one rule — see
                    // [`Self::push_union_member_leading_run`]. The last comment of either
                    // breaks, so the two compose without a separator of their own.
                    self.push_union_member_leading_run(&mut parts, own_line);
                    self.push_union_member_leading_run(&mut parts, &stripped_paren_leading);
                    parts.push(d.text("| "));
                    for comment in inline {
                        parts.push(self.build_comment_doc(comment));
                        parts.push(d.text(" "));
                    }
                } else {
                    // No pipe found, just add separator
                    parts.push(d.hardline());
                    self.push_union_member_leading_run(&mut parts, &stripped_paren_leading);
                    parts.push(d.text("| "));
                }
            } else {
                // First type: always has `| ` prefix when multiline. A FROZEN first member
                // emits its leading run — the own-line directive — BEFORE the `| ` so the
                // directive stays own-line and re-recognizes on pass 2; emitting it after
                // `| ` (the general path, below) would relocate it to trail the pipe
                // (`| // prettier-ignore`), flipping it trailing and losing the freeze next
                // pass. `mem::take` hands the run over here so the frozen arm doesn't
                // re-emit it.
                if frozen_member {
                    parts.extend(std::mem::take(&mut first_leading));
                }
                parts.push(d.text("| "));
            }

            // Add the type with the same per-member offset as the main path
            // (`build_union_member_offset_doc`). A parenthesized union member with a
            // leading line comment inside the parens keeps the comment there — for
            // EVERY member, not just the first (`| (⏎ // c⏎ inner⏎)`). Per the comment
            // position philosophy tsv associates the comment with the member it
            // documents rather than hoisting it out; prettier hoists it onto its own
            // line above the member. `true` to `build_parenthesized_union_doc` emits
            // the leading line comment inside so it is not dropped; the per-member
            // `align(2)` offset lines it up like any other paren-union member (the
            // `is_paren_union_member` arm of `build_union_member_offset_doc`). See
            // union_intersection_retained_paren_leading_line_comment_prettier_divergence.
            // Rule A member freeze (paren-transparent), the `frozen_member` hoisted above:
            // the first member via a leading-run directive or the in-span gap, a later
            // member via its own gap. The directive is emitted by the separator /
            // leading-comment machinery above; only the member DOC is replaced, and the
            // frozen member takes the same `align(2)` offset as a reformatted one. A frozen
            // first member's own-line leading run (`first_leading`) was emitted before the
            // `| ` above (and taken by `mem::take`), so extending it here is a no-op that
            // preserves the arm chain's consume-by-move invariant.
            if frozen_member {
                parts.extend(first_leading);
                parts.push(self.build_frozen_union_member_offset_doc(t, member_parens));
            } else if !stripped_paren_leading.is_empty() {
                // Redundant-paren member: its leading run was already emitted before the
                // `| ` above, so render the member as its fully STRIPPED inner — building
                // `t` (the parens) instead would emit the comment a second time.
                // `unwrap_parenthesized` peels every redundant layer (`((// c⏎ b))` → `b`),
                // matching the detection window. `first_leading` is empty here (later
                // member), extended only to keep the consume-by-move invariant the arm
                // chain relies on.
                parts.extend(first_leading);
                let inner = unwrap_parenthesized(t);
                let member_doc = self.build_union_member_offset_doc(inner, member_parens);
                // A trailing comment lifted from the shell (`(// c⏎ b /* t */)`) trails the
                // member inline (`| b /* t */`) — a type position. A no-op for the
                // pure-line / mixed cases (no comment in the trailing gap).
                parts.push(self.with_stripped_paren_trailing(
                    member_doc,
                    t,
                    inner,
                    TrailingBlock::Inline,
                ));
            } else if let TSType::Parenthesized(p) = t
                // ⚠️ Both halves read THROUGH the author's extra layers, because the
                // ROUTER above does ([`Self::union_member_paren_leading_line_comment`])
                // and a member it routed here must find an emitter. A direct-child match
                // plus the shallow one-paren window missed `X | ((// c⏎A | B))`: the run
                // fell to the default arm, whose `ShellLeadingRun::Upstream` licence is
                // granted on an upstream emitter EXISTING — this arm — so nothing printed
                // it and the comment was DROPPED. The pair the shells collapse into is
                // the outermost, and `build_parenthesized_union_doc`'s window already
                // spans from it to the union, so any nesting depth emits the whole run.
                && let TSType::Union(inner_union) = unwrap_parenthesized(t)
                && self.stripped_paren_hang_has_leading_line_comment(t)
            {
                // `first_leading` is non-empty only for the first member (see its
                // declaration); a later member's leading comments were emitted on their
                // own line above, so this extends nothing there.
                parts.extend(first_leading);
                parts.push(d.align(
                    2,
                    self.build_parenthesized_union_doc(inner_union, Some(p), ShellLeadingRun::Here),
                ));
            } else {
                // The leading run takes the member's per-member offset. Whenever the run
                // ends in a break it is the run — not the `| ` prefix — that places the
                // member's own first line, so an unindented run would strand that line
                // one level shallower than the member's internal breaks. Prettier has the
                // same shape: `align(2, print())`, whose `print()` carries the leading
                // comments.
                //
                // The wrapper is applied whenever there IS a run, never keyed on whether
                // the run breaks: `indent` binds only the line breaks *inside* it, so a
                // run whose comments all hug the member is pure text and the wrapper is
                // inert. "Does this run drop the member onto its own line?" is a question
                // the doc structure already answers — asking it again with a predicate
                // would be a second gate that can drift from this one.
                //
                // Align the RUN, never `build_union_member_offset_doc`'s result: that
                // function owns the opt-outs (an object literal and a default-paren
                // member supply their own indent and decline the offset), so wrapping
                // its result would double-offset exactly those two — the member's body
                // two columns past prettier, its closing delimiter out of line with its
                // opener. Sound because `align` is a per-line property, so
                // `align(concat([run, member]))` and `concat([align(run), member])`
                // agree wherever the member does take the offset.
                if !first_leading.is_empty() {
                    parts.push(d.align(2, d.concat(&first_leading)));
                }
                // The lifted trailing run of a stripped shell leaves the offset here too
                // ([`Self::build_union_member_doc`]): this layout is reached whenever
                // ANY member's gap holds a `//`, so a clean shell on a different member
                // must not be indented by a neighbour's comment.
                let has_leading = self.union_member_has_leading_comments(union, i);
                parts.push(self.with_claimed_shell_leading_run(edge_claim, || {
                    self.build_union_member_doc(t, member_parens, has_leading)
                }));
            }

            // Trailing comments on last type
            if i == union.types.len() - 1 {
                for comment in self.comments_to_emit_between(type_end, union.span.end) {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
            }
        }

        d.concat(&parts)
    }

    /// Whether a **type argument** actually prints hugged — [`Self::union_prints_hugged`]
    /// (which owns the whole hug question, shape *and* comments) narrowed by
    /// [`union_has_brace_member`] (the type-argument-only extra clause).
    ///
    /// The single gate every type-argument position asks, so none of them has to
    /// remember that a shape predicate is only half the question. Asking a bare shape
    /// instead inlines the argument atomically while the printer expands its members,
    /// gluing the `<` to a dangling `|` (`Foo<| {…} /* c */⏎| null>`) — which is exactly
    /// what `type_arguments.rs` and `type_params.rs` both did.
    pub(crate) fn type_arg_union_prints_hugged(&self, ty: &TSType<'_>) -> bool {
        // `union_prints_hugged` subsumes `union_hug_shape`, so the shape is not re-tested
        // here — the brace clause is the only thing this position adds.
        matches!(unwrap_parenthesized(ty), TSType::Union(u)
            if self.union_prints_hugged(u) && union_has_brace_member(u))
    }

    /// Whether [`Self::build_union_type_doc`] will actually take its **hug** path —
    /// the inline `{ … } | null` form where the object member owns its own expansion.
    ///
    /// The single source of truth for that question, because two places must agree on
    /// it: the union printer (which lays the members out) and the type-alias RHS
    /// (`build_type_alias_eq_value_doc`, which decides whether to break after `=`). Asking the
    /// bare syntactic [`union_hug_shape`] at the alias while the printer declines
    /// the hug for a comment splits them — the alias keeps `= ` while the union expands,
    /// yielding `type A = | // c⏎{ a: 1 }⏎| null` where a non-hugging union of the same
    /// shape correctly breaks after the `=`.
    ///
    /// Beyond the syntactic shape, a comment prettier's `shouldHugUnionType` would bail
    /// on disqualifies the hug:
    ///
    /// - **between two members** — prettier's `types.some((t) => hasComment(t))`, which
    ///   in the detached model lives in the inter-member gap;
    /// - a **line** comment in the leading `|`→first-member gap — the hug emits that gap
    ///   block-only, so a line comment there would be silently DROPPED, and it could not
    ///   be inlined regardless (a `//` runs to end-of-line and would swallow the member).
    ///   `union_has_comments_between_members` cannot answer this: the gap is *before* the
    ///   first member, not *between* two. A **block** glued ahead of the first member —
    ///   `| /* c */ { … } | null` in that gap, or `/* c */ { … } | null` in the enclosing
    ///   seam's gap, outside `union.span` (for the plain spelling the span starts AT the
    ///   first member) — is prettier's `hasComment` on the member too: it binds such a
    ///   comment to the outermost node starting right after it, the member unless a
    ///   parent starts there (the predicate's annotation at `is`, a conditional's check),
    ///   and the union expands one-member-per-line with the comment after the pipe.
    ///   ⚠️ tsv applies that at the VALUE seams alone ([`UnionLeadingGap::ValueSeam`]:
    ///   the alias `=`, the annotation `:`, the function-type `=>`, the mapped value —
    ///   `union_hug_gap_block_comment`), where the seam hands the run into the union.
    ///   At every other position ([`UnionLeadingGap::Other`] — a type argument's `<`, a
    ///   tuple's `[`, a paren shell, `as` / `satisfies`, a type parameter's bound, a
    ///   conditional's `extends` / `?` / `:`, an indexed access, a mapped `in`) the
    ///   enclosing seam prints the run ahead of the union and the union hugs behind it,
    ///   both spellings — an **open divergence**, stable, one class: each such seam would
    ///   have to hand its run in (or hang) the way the value seams do, or its gap
    ///   emitter welds `/* c */ | {` to the operator once the union declines;
    /// - inside a member's own redundant **paren shell** (`({ … } /* c */) | null`) —
    ///   [`Self::union_member_shell_holds_comment`], the other half of
    ///   `types.some((t) => hasComment(t))` that the between-member scan cannot reach.
    ///
    /// A comment nested *inside* a member (`{ /* c */ a: 1 }`) attaches to a child node,
    /// not the member, so it never blocks the hug — the member's own doc renders it.
    ///
    /// **Axis.** This is a layout gate, so it asks the **on-page** question — an owned
    /// comment occupies the page and must block the hug like any other. The delegates it
    /// guards read the **to-emit** axis, which is sound here only because ownership is
    /// set exclusively in expression position (`parser/expression.rs`): no comment in a
    /// *type*'s gaps is ever owned, so within a union the two axes coincide. Should
    /// ownership ever reach type position, `union_has_comments_between_members` becomes
    /// the weak link — the on-page fast path would fall through and the emit-keyed
    /// pairwise scan would report "no comments" for an owned one, hugging a union whose
    /// members the printer expands.
    pub(crate) fn union_prints_hugged(&self, union: &TSUnionType<'_>) -> bool {
        self.union_prints_hugged_with(union, UnionLeadingGap::Other)
    }

    /// [`Self::union_prints_hugged`] with the enclosing seam's word on a block glued
    /// ahead of the first member ([`UnionLeadingGap`]) — the one clause that lives
    /// outside the union's own span, or ahead of its authored pipe, and so cannot be read
    /// off the union alone. The value seams ask it through
    /// [`Self::build_union_value_doc`], beside the doc it governs; everyone else asks
    /// [`Self::union_prints_hugged`] (`Other`).
    fn union_prints_hugged_with(&self, union: &TSUnionType<'_>, gap: UnionLeadingGap) -> bool {
        if !union_hug_shape(union) {
            return false;
        }
        let Some(first) = union.types.first() else {
            return false;
        };
        let first_start = first.span().start;
        // At a value seam, a block anywhere between the operator and the first member —
        // in the seam's own gap, outside the span, or after an authored pipe — declines
        // (see the doc above): asked before the in-span fast path, which cannot see the
        // seam's gap. One range from the operator, so a handed run, a broke-after block
        // and the in-span spelling are one question.
        if let UnionLeadingGap::ValueSeam { gap_start, .. } = gap
            && self
                .comments_on_page_between(gap_start, first_start)
                .any(|c| c.is_block)
        {
            return false;
        }
        // Zero-comment fast path — an **on-page** question, since it short-circuits the
        // comment gates below.
        if !self.has_comments_on_page_between(union.span.start, union.span.end) {
            return true;
        }
        !self.union_has_comments_between_members(union)
            && !self.union_member_shell_holds_comment(union)
            // A line comment in the leading `|`→first-member gap declines everywhere
            // (the hug path could not print it); a block there is the value seam's
            // question above, and hugs at every other position.
            && !self.has_line_comments_between(union.span.start, first_start)
    }

    /// Whether any member carries a comment in its own redundant **paren shell** — either
    /// of [`paren_shell_gaps`]' two windows, so a doubly-nested `((/* c */ { … }))` counts
    /// like the single layer it strips to.
    ///
    /// The detached-model half of prettier's `types.some((t) => hasComment(t))` that the
    /// between-member scan cannot see: prettier's TS AST drops the paren node, so a comment
    /// the author wrote in a member's shell attaches to that member as an ordinary leading
    /// or trailing comment and `shouldHugUnionType` bails.
    ///
    /// ⚠️ **This is a paren-INDEPENDENCE clause, not a paren-sensitivity one.** Prettier's
    /// output for `({ … } /* c */) | null` is byte-identical to `{ … } /* c */ | null`'s, and
    /// likewise at the other two placements (`(/* c */ { … })`, `| (/* c */ null)`) — it
    /// expands all six. Without this clause the unwrap in `helpers::is_object_like_type`
    /// would let the paren'd spellings hug while their bare twins expand, which is the
    /// divergence, not the parity. The bare twins are already declined by
    /// [`Self::union_has_comments_between_members`] — except the leading placement, which
    /// that scan cannot reach: a value seam declines it by handing the run into the union
    /// (see [`Self::union_prints_hugged`]), and the positions with no handoff still hug it.
    ///
    /// A comment nested deeper inside the member (`({ /* c */ a: 1 })`) sits within the
    /// unwrapped inner's own span and never counts, matching the rule
    /// [`Self::union_prints_hugged`] states for the paren-free spelling.
    ///
    /// **Axis**: on-page, like every other clause of that layout gate.
    fn union_member_shell_holds_comment(&self, union: &TSUnionType<'_>) -> bool {
        union.types.iter().filter_map(outermost_paren).any(|shell| {
            let (leading, trailing) = paren_shell_gaps(shell);
            self.has_comments_on_page_between(leading.start, leading.end)
                || self.has_comments_on_page_between(trailing.start, trailing.end)
        })
    }

    /// Whether any comment sits in a gap *between* two consecutive members — the
    /// detached-model spelling of prettier's `types.some((t) => hasComment(t))`
    /// (`shouldHugType`'s bail).
    ///
    /// Private, and deliberately so: it answers one clause of "does this union hug",
    /// never that question itself. [`Self::union_prints_hugged`] owns the whole answer,
    /// and is the only caller — a layout gate that reaches past it to this clause is
    /// re-deriving the layout with a subset of the rule, which is exactly how the
    /// leading-`|` line comment was missed.
    fn union_has_comments_between_members(&self, union: &TSUnionType<'_>) -> bool {
        // Zero-comment window gate: one binary search over the whole union span before
        // the N-1 pairwise between-member searches. Each pairwise range lies within
        // `[union.span.start, union.span.end]`, so with no comment inside the union
        // every pairwise check is provably false — skip them on the common
        // comment-free `A | B | C`.
        if !self.has_comments_to_emit_between(union.span.start, union.span.end) {
            return false;
        }
        union
            .types
            .windows(2)
            .any(|pair| self.has_comments_to_emit_between(pair[0].span().end, pair[1].span().start))
    }

    /// True when an **own-line comment** sits between two consecutive members —
    /// a line comment (which can never be inline), or a block comment with a
    /// newline before it (`| 'x'⏎/* c */⏎| 'y'`), on either side of the `|`.
    ///
    /// Prettier emits such a comment via `printComments` with a hardline
    /// (`union-type.js`), forcing the whole union group to break
    /// one-member-per-line. A *same-line* block comment (`a /* c */ | b`) does
    /// not count — it stays inline, matching `union_intersection_parens_comment`.
    /// Catches own-line *block* comments too, which the default (groupable) path would
    /// otherwise keep flat.
    fn union_has_own_line_member_comment(&self, union: &TSUnionType<'_>) -> bool {
        // Zero-comment window gate (see `union_has_comments_between_members`): every
        // pairwise range lies within the union span, so no comment inside the union
        // means every `comments_to_emit_in_range` below is empty — skip the N-1 scans on the
        // common comment-free union.
        if !self.has_comments_to_emit_between(union.span.start, union.span.end) {
            return false;
        }
        union.types.windows(2).any(|pair| {
            let (prev_end, next_start) = (pair[0].span().end, pair[1].span().start);
            self.comments_on_page_between(prev_end, next_start)
                .any(|c| self.is_own_line_comment(c))
        })
    }

    /// True when a comment between two consecutive intersection members forces the
    /// whole intersection one-member-per-line.
    ///
    /// A **line** comment always forces it. A **block** comment forces it only when it
    /// OWNS its line in source — a newline both before and after it
    /// ([`Printer::comment_isolated_on_its_line`], prettier's `printLeadingComment`
    /// hardline condition). Anything sharing its line keeps it inline, whichever side:
    /// the previous member (`A /* c */⏎& B`), the following one (`A &⏎/* c */ B`), the
    /// `&` itself (`A⏎& /* c */⏎B` and `A⏎/* c */ &⏎B`), or another comment
    /// (`A &⏎/* c1 */ /* c2 */⏎B`). Only the isolated block (`A &⏎/* c */⏎B`) breaks
    /// (`intersection-type.js`).
    ///
    /// ⚠️ The `&` spellings are why this reads the SOURCE rather than the member spans:
    /// the operator is re-emitted structure no member span covers, so an item-boundary
    /// anchor calls a comment sharing the `&`'s line isolated and breaks an intersection
    /// that fits — output tsv's own second pass then collapsed, the break having put the
    /// comment back on the previous member's line.
    ///
    /// This deliberately differs from the union's `union_has_own_line_member_comment`
    /// (which keys on `is_own_line_comment` — the preceding newline alone, no glue half):
    /// prettier's **union** printer expands a block adjacent to its member, but the
    /// **intersection** printer collapses it, so asking the glue half there would
    /// under-expand the `A |⏎/* c */ B` case the union breaks.
    fn intersection_has_isolated_member_comment(
        &self,
        intersection: &TSIntersectionType<'_>,
    ) -> bool {
        // Zero-comment window gate (see `union_has_comments_between_members`): every
        // pairwise range lies within the intersection span, so no comment inside it
        // means every `comments_to_emit_in_range` below is empty — skip the N-1 scans.
        if !self.has_comments_to_emit_between(intersection.span.start, intersection.span.end) {
            return false;
        }
        intersection.types.windows(2).any(|pair| {
            let (prev_end, next_start) = (pair[0].span().end, pair[1].span().start);
            self.comments_on_page_between(prev_end, next_start)
                .any(|c| self.comment_isolated_on_its_line(c))
        })
    }

    /// True when the intersection must use the multiline, comment-aware layout
    /// (`build_intersection_type_doc_with_line_comments`) rather than the inline form —
    /// because a comment can't be inline. Two triggers:
    ///
    /// - an **isolated** comment between two members (any line comment, or an own-line
    ///   block — see `intersection_has_isolated_member_comment`); a block inline-adjacent
    ///   to either member stays inline (unlike the union path, which expands it);
    /// - a parenthesized *union* member with a leading line comment inside its parens
    ///   (`(a | b) & (// c⏎ c | d)`) — a line comment can't be inline, and tsv preserves it
    ///   inside the parens (the member breaks open), which the inline form would otherwise
    ///   drop. Asked of **every** member including the first — the pair SURVIVES into the
    ///   output and can host the run ([`Self::paren_union_line_comment_member`] is what
    ///   checks that, and its ⚠️ is why), and
    ///   `intersection_first_member_hoist_comments` declines that shape for exactly this
    ///   arm to answer it — save where an enclosing seam has already claimed the first
    ///   member's run ([`Self::first_member_shell_run_claimed`]). Restricted to a union
    ///   inner — the only shape the multiline path renders comment-aware
    ///   (`build_parenthesized_union_doc`). A paren-intersection / paren-function member
    ///   with a leading line comment still drops it — extend when a real case appears.
    ///
    /// ⚠️ **Not context-free: this reads the `claimed_shell_leading_run` cell**, through
    /// [`Self::first_member_shell_run_claimed`]. So every caller must ask it in the SAME
    /// [`Printer::with_claimed_shell_leading_run`] scope the layout it routes to will build
    /// in — otherwise the router and [`Self::build_intersection_line_comment_member_doc`]
    /// answer different questions, and the member's run is dropped or printed twice. The
    /// three callers inside `build_intersection_type_doc` /
    /// `build_intersection_leading_gap_line_comment_doc` satisfy it by construction (no
    /// claim is set between the ask and the build). The fourth,
    /// [`Self::aligned_trailing_object_shell`], routes from OUTSIDE the intersection
    /// builder — and its own union-member call site sits inside that member's `edge_claim`
    /// scope, which is exactly what keeps the two in step there.
    fn intersection_needs_line_comment_layout(
        &self,
        intersection: &TSIntersectionType<'_>,
    ) -> bool {
        let member_parens = union_member_parens(intersection.types.len());
        self.intersection_has_isolated_member_comment(intersection)
            || intersection.types.iter().enumerate().any(|(i, t)| {
                self.paren_union_line_comment_member(t, member_parens)
                    .is_some()
                    && (i > 0 || !self.first_member_shell_run_claimed(t))
            })
    }

    /// The paren-union member shape a leading `//` is answered in FOUR times: the routing
    /// gate above, [`Self::build_intersection_line_comment_member_doc`] (which emits the
    /// run inside the pair), [`Self::intersection_first_member_hoist_comments`] (which
    /// declines to hoist it out of one), and — one level out —
    /// [`Printer::intersection_member_run_owned_downstream`], which is the leading-edge
    /// seam declining to claim a run this shape already owns. ONE accessor, so the four
    /// cannot drift: a decline whose shape is wider than the layout answering it is a
    /// DROPPED comment, and one that is narrower is a DOUBLE-PRINTED one.
    ///
    /// ⚠️ The pair must actually SURVIVE, and being a paren-union does not say that —
    /// `member_parens` does (the intersection's own rule, [`union_member_parens`]). The
    /// whole keep-inside answer rests on the run having a pair to sit in, and **a comment
    /// never changes which parens are retained**, so a member the comment-free rule strips
    /// must relocate the run instead. Two shapes strip, and each falsifies the premise a
    /// different way: a **single-member** union (`a & (// c⏎ | b)`, the leading-`|`
    /// spelling) is semantically just its member, so
    /// `type_needs_parens_in_union_or_intersection` sees through it; and in a **one-member
    /// intersection** (`& (// c⏎ | a | b)`) `member_parens` is `|_| false` outright, so
    /// even a real union's pair goes. Kept inside anyway, both retained a paren the
    /// reparse then strips — pass 1 printing `a &⏎ ( // c⏎ b⏎ )` and pass 2 `a &⏎ // c⏎ b`,
    /// an F1 violation rather than a divergence. Pinned by
    /// `types/intersection_redundant_paren_member_line_comment_prettier_divergence`.
    ///
    /// ⚠️ And the window is the **whole shell**, not the outermost paren's own gap: the
    /// printer emits ONE pair however many layers the author nested, and the run renders
    /// inside it (`build_parenthesized_union_doc` scans `[outermost (, union.start)`). So
    /// `((// c⏎ a | b)) & d` and `(// c⏎ a | b) & d` are one authoring with one answer —
    /// which is what the union family already gave at every member, and what prettier gives
    /// too (it converges all three spellings, hoisting the run in each). Asked of the
    /// paren's DIRECT child instead, the nested spelling matched nothing: at a LATER member
    /// the router did not fire, the default builder reached
    /// `build_type_doc_maybe_parens_impl`'s union arm with `ShellLeadingRun::Upstream` on
    /// the strength of an upstream emitter that only a FIRST member has, and the comment
    /// was silently **DROPPED** ([`comments.md`](../../../../docs/comments.md) hazard 1).
    /// A layer count is not authorship, and a rule keyed on one answers two ways for one
    /// authored comment.
    ///
    /// The survival question is also what [`Printer::head_stripped_paren_shell`]'s
    /// intersection link asks this about: where no pair survives, an enclosing gap owns the
    /// run instead of the member.
    pub(super) fn paren_union_line_comment_member<'t>(
        &self,
        t: &'t TSType<'t>,
        member_parens: fn(&TSType<'_>) -> bool,
    ) -> Option<(&'t TSParenthesizedType<'t>, &'t TSUnionType<'t>)> {
        if !member_parens(t) {
            return None;
        }
        let p = outermost_paren(t)?;
        let TSType::Union(union) = unwrap_parenthesized(t) else {
            return None;
        };
        self.stripped_paren_hang_has_leading_line_comment(t)
            .then_some((p, union))
    }

    /// Whether an ENCLOSING leading-edge seam has already claimed the run inside the
    /// intersection's **first** member's paren shell
    /// ([`Self::shell_leading_run_claimed`]). Only the first member can be claimed — it
    /// is the descent link of `Printer::head_stripped_paren_shell`, and a later member
    /// is never on that path.
    ///
    /// ⚠️ Asked by all THREE readers of the paren-union member shape — the routing gate
    /// above, [`Self::build_intersection_line_comment_member_doc`], and
    /// [`Self::intersection_first_member_hoist_comments`] — which is the whole point:
    /// where the seam has claimed, the keep-inside layout is a SECOND emitter for one
    /// comment. Asking it at the router alone is not enough, since the isolated
    /// member-comment trigger reaches that layout without consulting the router's arm
    /// (`(// c⏎ | A) // x⏎ & B` routes on the `// x` in the operator gap).
    ///
    /// It is load-bearing at the **hoist**, which every enclosing seam outranks — a claimed
    /// run hoisted here is a SECOND emitter for one comment
    /// ([`comments.md`](../../../../docs/comments.md) hazard 3), and removing the read
    /// alone breaks ten fixtures. At the two **paren-union** readers it is the belt to
    /// [`Printer::head_stripped_paren_shell`]'s braces: that seam declines the descent for
    /// exactly the shape they answer ([`Self::paren_union_line_comment_member`], its fourth
    /// reader), so no gap should be able to claim a surviving pair's run in the first place
    /// — but the claim is matched by CONTAINMENT rather than equality, and a claim over a
    /// wider region is the residue that argument does not cover.
    fn first_member_shell_run_claimed(&self, first_member: &TSType<'_>) -> bool {
        self.head_run_claimed(first_member.span().start, first_member)
    }

    /// Emit one intersection member→member gap — the comments before the `&`, the `&`
    /// itself, and the comments after it leading the next member — for a caller that
    /// prints the operator as its own text rather than through
    /// [`Self::build_intersection_member_body_doc`] (the aligned trailing-object shell).
    /// The two runs are that method's, so both paths answer "which side of the `&` does
    /// this comment keep?" the same way.
    pub(in crate::printer) fn push_intersection_operator_gap_comments(
        &self,
        parts: &mut DocBuf,
        prev_member_end: u32,
        next_member_start: u32,
    ) {
        self.push_pre_separator_block_comments(parts, prev_member_end, next_member_start, b'&');
        parts.push(self.d().text(" & "));
        if let Some(sep_pos) =
            find_separator_position(self.source, prev_member_end, next_member_start, b'&')
        {
            // The run's own `breaks` flag is dropped, not ignored: a comment isolated on
            // its line is exactly what `intersection_needs_line_comment_layout` routes
            // away from this layout, so every comment reaching here is inline-able and
            // the flag is always false.
            let (run, _) = self.build_member_leading_block_comments(sep_pos + 1, next_member_start);
            parts.push(run);
        }
    }

    /// The object-trailing intersection shell (`(A & { … })`) when it takes the
    /// **aligned** layout — [`Self::build_parenthesized_intersection_trailing_object_doc`],
    /// which prints its own `(`…`)` and supplies the member's `align(2)` offset itself
    /// (on its own closing `})`), so such a member must not be wrapped in that offset
    /// again or its body and closing double-shift.
    ///
    /// One predicate for one question, asked by both readers — the builder selection in
    /// `build_type_doc_maybe_parens_impl` and the offset opt-out in
    /// [`Self::build_union_member_offset_doc`]. They were two spellings of the shape test
    /// before, and the comment condition below is exactly the kind of clause that can only
    /// be added to one of two spellings.
    ///
    /// A **line** comment in the opening's member gaps declines the layout: that opening is
    /// fused text (`" & "`, `" & {"`) whose `{` cannot leave the `&`'s line, and a `//` runs
    /// to end-of-line. Declining hands the shell to the general retained-paren path, which
    /// already lays out exactly this — the same shape the no-trailing-object sibling
    /// (`(a & // c⏎b)`) takes, matching prettier where prettier preserves. The gate is the
    /// ordinary intersection printer's own ([`Self::intersection_needs_line_comment_layout`]),
    /// so the two paths cannot disagree about which comments can be inline. ⚠️ That gate
    /// reads the `claimed_shell_leading_run` cell, so this predicate must be asked in the
    /// scope the builder will run in — see its own ⚠️.
    ///
    /// The third return is the **leading-edge shell claim** this shell's own `(` gap owns.
    /// The builder prints that `(` and emits the gap behind it, so it is the enclosing gap
    /// for a redundant shell at the intersection's head — and it widens over that shell's
    /// run and claims it, exactly as every other gap that can hold one does
    /// ([`Printer::leading_edge_claim_and_start`]). Resolved HERE rather than in the
    /// builder because it is also what lets the first-member decline below stand down: a
    /// run this gap claims has an emitter after all. Filtered on
    /// [`Printer::shell_leading_run_claimed`] for the licence every EDGE claim carries —
    /// where a gap ABOVE already owns the run (a union member's `|` gap does, for this very
    /// shape), claiming it again prints it twice.
    pub(in crate::printer) fn aligned_trailing_object_shell<'t>(
        &self,
        ts_type: &'t TSType<'t>,
    ) -> Option<(
        &'t TSIntersectionType<'t>,
        &'t TSTypeLiteral<'t>,
        Option<Span>,
    )> {
        let unwrapped = unwrap_parenthesized(ts_type);
        let TSType::Intersection(intersection) = unwrapped else {
            return None;
        };
        // A **one-member** intersection (`& { x: X }`, the leading-`&` form) declines too:
        // there is no `&` in the output — prettier drops a single-element intersection node
        // in postprocess and so does the ordinary path, which reaches the same collapse in
        // every other context. Taking it here instead printed the operator as pure text with
        // nothing on its left (`[( & { x: X })?]`, reachable as an OPTIONAL tuple element)
        // and dropped any comment after that `&`, since the arm had no member gap to scan.
        if intersection.types.len() < 2 {
            return None;
        }
        let TSType::TypeLiteral(obj) = unwrap_parenthesized(intersection.types.last()?) else {
            return None;
        };
        // The shell's own `(` gap owns a redundant head shell's run (see the doc above);
        // resolved only where there IS such a gap, since the builder emits one only for a
        // paren it was handed.
        let claim = outermost_paren(ts_type).and_then(|_| {
            self.leading_edge_shell_claim(unwrapped)
                .filter(|c| !self.shell_leading_run_claimed(c.start, c.end))
        });
        // A first-member paren holding a leading `//` that this gap does NOT claim declines
        // the layout. The aligned builder reassembles the intersection from its members'
        // docs, so a hoist run `build_intersection_type_doc` would have lifted out has no
        // emitter here and the comment was DROPPED (`docs/comments.md` hazard 4) —
        // invisibly, because the hoist's own shape is a REQUIRED pair
        // (`((⟨⟩b | c) & { x: X }) | e`), which the redundant-shell claims the union's
        // routing gate asks about cannot see.
        //
        // ⚠️ The two arms partition rather than race: where the claim exists it covers
        // exactly the hoist's window (the claim opens at the shell's `(`, the hoist one
        // byte in, and both end at the fully-unwrapped inner's start), and where the shell
        // is deeper than the first member the hoist is empty by construction — so a claimed
        // run is never also hoisted, and an unclaimed one is never lost.
        (!self.intersection_needs_line_comment_layout(intersection)
            && (claim.is_some()
                || self
                    .intersection_first_member_hoist_comments(intersection)
                    .is_empty()))
        .then_some((intersection, obj, claim))
    }

    //
    // Intersection Types
    //

    /// Build a Doc for an intersection type: `A & B & C` or `A &\n\tB &\n\tC`
    ///
    /// Prettier formatting for intersection types differs from union types:
    /// - Flat: `A & B & C`
    /// - Break: `A &\n\tB &\n\tC` (trailing `&`, continuation indented)
    ///
    /// When `wrap_in_group` is true (default), wraps in its own group for
    /// independent breaking decisions. When false, inherits from parent.
    ///
    /// `own_line` tells the first-member comment hoist that the caller has already
    /// placed this intersection on its own indented line (a tuple element), rather
    /// than trailing a prefix on the enclosing line (`type T = …`, `[K in …]: …`).
    /// The hoist's continuation indent hangs the run one level under a *trailing*
    /// prefix; on an own-line placement the caller's line indent already supplies
    /// that level, so a second one over-indents the reparsed bare form (the tuple
    /// non-idempotency). Almost every caller is a trailing-prefix context and passes
    /// `false`; only own-line element callers pass `true`. See
    /// `intersection_first_member_hoist_comments`.
    ///
    /// See also: `build_intersection_type_annotation_doc` in type_annotation.rs
    /// for the `: Type` annotation variant (shares continuation logic).
    pub(in crate::printer) fn build_intersection_type_doc(
        &self,
        intersection: &TSIntersectionType<'_>,
        wrap_in_group: bool,
        own_line: bool,
    ) -> DocId {
        let d = self.d();
        if intersection.types.is_empty() {
            return d.empty();
        }

        // Format-ignore leading run (Rule A), symmetric with `build_union_type_doc`: an
        // alone-on-line directive freezes only the first member. (There is no
        // whole-intersection arm — the intersection first member behaves like every
        // other honored list position.) Gated on the document-level flag; the in-span `has_comments`
        // gate below cannot see an out-of-span directive.
        let leading_freeze =
            self.composite_leading_run_freeze(intersection.span.start, intersection.types);
        let freeze_first = leading_freeze.is_some();

        // Single-member-intersection collapse under a freeze — the union's rule, mirrored
        // (see the matching branch in `build_union_type_doc`): a 1-element intersection
        // drops its `&` when reformatted, so a member-only freeze is non-idempotent —
        // pass 2 sees a bare member no longer routed through the intersection. A
        // leaf/object sole member freezes the WHOLE intersection span verbatim (keeps the
        // `&` → idempotent); a composite sole member is TRANSPARENT for directive binding
        // — fall through and build it normally so its own Rule A applies inside.
        if intersection.types.len() == 1
            && self.list_member_frozen(intersection.span.start, intersection.types, 0, freeze_first)
            && !matches!(
                unwrap_parenthesized(&intersection.types[0]),
                TSType::Union(_) | TSType::Intersection(_)
            )
        {
            return self.raw_source_range(intersection.span.start, intersection.span.end);
        }

        // One window search over the intersection, exactly as `build_union_type_doc`
        // does: every comment query below (the leading `&` gap, the gaps either side of
        // each separator, the trailing gap, the paren-hoist probe, and the line-comment
        // layout check) is bounded inside `intersection.span`, so a comment-free `A & B`
        // provably has none — no search, no empty child, and no `find_separator_position`
        // byte scan (the printed `&` is static text).
        //
        // On-page (not to-emit): a zero-comment fast gate is an on-page question per
        // docs/comments.md, matching `build_union_type_doc`. Byte-identical to the old
        // to-emit spelling today — no comment in a *type* gap is ever owned (ownership is
        // set only in expression position) — but on-page is the correct axis, so a future
        // owned comment in type position can't blind the layout gates this guards.
        let has_comments =
            self.has_comments_on_page_between(intersection.span.start, intersection.span.end);

        // An enclosing gap may own this node's whole transparent head region — a ONE-member
        // intersection prints as its member, so its leading-`&` gap is that gap's, not
        // this builder's (see `Printer::composite_head_region_claimed`). Every route below
        // that would emit the gap stands down; the member's own shell run is already
        // declined by the hoist's `first_member_shell_run_claimed`.
        let head_region_claimed =
            self.composite_head_region_claimed(intersection.span.start, intersection.types);

        // A LINE comment in the leading-`&` gap `[span.start, first.start)` can't be
        // carried by the compact path's block-only extraction (a `//` runs to EOL), and
        // no other route scans this gap for line comments — it was silently dropped
        // (docs/comments.md hazard 1), and a dropped alone-on-line directive left a
        // phantom one-pass freeze (member frozen on pass 1, freeze lost on pass 2).
        // Route through the leading-gap run emitter, which owns the WHOLE gap run.
        if has_comments
            && !head_region_claimed
            && let Some(first_member) = intersection.types.first()
            && self.has_line_comments_between(intersection.span.start, first_member.span().start)
        {
            return self.build_intersection_leading_gap_line_comment_doc(
                intersection,
                wrap_in_group,
                own_line,
                leading_freeze,
            );
        }

        // Hoist leading line comments inside the first member's stripped parens
        // OUT of the intersection (e.g., `(// c\n a) & b` → `// c\n a & b`, and the
        // double-nested `((// c\n a)) & b` the same way). The comment goes on its own
        // line BEFORE the intersection so the intersection content itself can still fit
        // inline. The deep window scans the whole stripped shell, not just the outer
        // paren's own gap, so a comment nested one paren deeper still hoists.
        //
        // ⚠️ Suppressed for a FROZEN first member, whose shell comments ride inside its
        // verbatim slice — hoisting them too is a DOUBLE-PRINT, and hoisting them
        // *instead* strips the shell the freeze was meant to preserve. The rule this
        // module states (`ignore.rs` header) is that comment preservation outranks
        // redundant-paren removal under a freeze, and the union's member loop already
        // suppresses its own stripped-paren run the same way; this path was the one place
        // the hoist still won, so a `//` in the shell lost the freeze outright while the
        // block spelling kept it. `build_intersection_leading_gap_line_comment_doc` states
        // the identical suppression for the leading-gap route.
        //
        // A shell one link INSIDE the first member (`& /* c */ (// c2⏎A)[] & B`) is the
        // second shape [`IntersectionHeadRun`] resolves, reached only where the ENCLOSING
        // gap declined the claim — which for a multi-member intersection is exactly a
        // leading-`&` gap holding a comment of its own
        // ([`Printer::head_stripped_paren_shell`]'s ⚠️), leaving this the innermost gap
        // that can own the run.
        if has_comments && let Some(first_member) = intersection.types.first() {
            let first_frozen = self.list_member_frozen(
                intersection.span.start,
                intersection.types,
                0,
                freeze_first,
            );
            let head = self.intersection_first_member_head_run(intersection, first_frozen);
            if !head.run.is_empty() {
                // The leading-`&` gap rides the SAME run, ahead of the hoisted shell comments
                // — the two windows are contiguous in source (`& /* c */ (// c2⏎ A)`), so one
                // emission in source order is the only way to keep it. Emitted by the body
                // instead, the hoisted run rendered FIRST and the pair came back REVERSED;
                // left to the body while an enclosing gap had widened over the shell, the
                // gap's own comments printed TWICE. `build_intersection_leading_gap_line_comment_doc`
                // states the identical composition for the route a `//` in that gap takes.
                let mut run: CommentVec<'_> = self
                    .comments_to_emit_between(intersection.span.start, first_member.span().start)
                    .collect();
                run.extend(head.run.iter().copied());
                // The compact inline body can't represent an *isolated* between-member
                // comment (a line/own-line comment forces multiline); route those through
                // the line-comment path with the first member's (now-hoisted) paren-leading
                // stripped, so the other comments aren't dropped. Otherwise stay compact
                // inline (block comments emitted in place). The shared builder answers all
                // of that once, for this route and the leading-gap one alike.
                let inner = self.build_intersection_claimed_body_doc(
                    intersection,
                    wrap_in_group,
                    &head,
                    leading_freeze,
                );
                let mut parts = DocBuf::new();
                for comment in &run {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.hardline());
                }
                parts.push(inner);
                let body = d.concat(&parts);
                // The body renders flush-left; indent the hoisted comment(s) +
                // intersection under the alias `=` so continuation lines align
                // (`type T = // c⏎⇥A & B`) and the form stays idempotent — without it,
                // pass 2 re-indents the reparsed, no-longer-parenthesized body, whose
                // run now sits in the `=` gap and hangs the value there. An `own_line`
                // caller (a tuple element) already indents the whole element, so it is
                // the one skip.
                //
                // ⚠️ **The forced-multiline layout is NOT a second skip**, though it once
                // was, on the reading that it "self-indents per member". It indents
                // CONTINUATION members; the first member sits at the body's own base,
                // which is the line the hoisted run drops onto — so the skip left that
                // line one level short of every other and of the reparse
                // (`(// L⏎↹(p: 1) => void // c1⏎↹// c2⏎) &⏎// x⏎C`). One authoring reached
                // two fixed points depending on whether some unrelated gap happened to
                // carry an isolated comment, which is the standing twin-loop hazard read
                // one level out.
                return if own_line { body } else { d.indent(body) };
            }
        }

        let leading_gap = if head_region_claimed {
            LeadingGap::Claimed
        } else {
            LeadingGap::Emit
        };

        if has_comments && self.intersection_needs_line_comment_layout(intersection) {
            let doc =
                self.build_intersection_type_doc_with_line_comments(intersection, leading_gap);
            // The line-comment layout self-indents per member (mirroring the no-comment
            // loop and Prettier's `printIntersectionType`), so no outer indent is added.
            // The `wrap_in_group` path (type arguments, tuple elements, mapped-type
            // values, conditional branches) still groups it; the hanging callers add the
            // group.
            return if wrap_in_group { d.group(doc) } else { doc };
        }

        self.build_intersection_compact_doc(
            intersection,
            wrap_in_group,
            has_comments,
            leading_freeze,
            leading_gap,
        )
    }

    /// The compact (width-decided) intersection layout — the tail of
    /// `build_intersection_type_doc` once the leading-gap / hoist / line-comment routes
    /// have declined. Prettier uses a trailing `&` when breaking, with continuation
    /// types indented:
    /// - Flat: `A & B & C`
    /// - Break: `A &\n\tB &\n\tC`
    ///
    /// The per-member separator/indent (object-adjacency + `was_indented`) keeps a
    /// huggable boundary (`& {` / `} &`) space-hugged and un-indented — uniformly,
    /// whether the object is first, middle, or last.
    ///
    /// `leading_gap` says who emits the leading-gap block comments
    /// (`[span.start, first.start)`, e.g. `& /* c */ A & B`) — see [`LeadingGap`].
    fn build_intersection_compact_doc(
        &self,
        intersection: &TSIntersectionType<'_>,
        wrap_in_group: bool,
        has_comments: bool,
        leading_freeze: Option<LeadingRunFreeze>,
        leading_gap: LeadingGap,
    ) -> DocId {
        let d = self.d();
        let (freeze_first, freeze_first_multiline) =
            LeadingRunFreeze::first_member_flags(leading_freeze);

        // A single-member intersection collapses to its member — see the matching
        // note in `build_union_type_doc`. The lone member needs no precedence
        // parens (the parent context supplies any), while comment-aware paths
        // below still preserve comments around the `&`/parens.
        let member_parens = union_member_parens(intersection.types.len());

        // Build first type separately (not indented)
        let mut first_parts = DocBuf::new();
        let first_type = &intersection.types[0];
        let first_type_start = first_type.span().start;
        let first_type_end = first_type.span().end;

        // Extract leading block comments before the first type
        // (e.g., `& /* c */ A & B` — comment between leading `&` and first member).
        // Skipped when the leading-gap run emitter already claimed the whole gap.
        if leading_gap == LeadingGap::Emit
            && has_comments
            && let Some(leading) = self.build_comments_between_filtered_opt(
                intersection.span.start,
                first_type_start,
                CommentSpacing::Trailing,
                CommentFilter::BlockOnly,
            )
        {
            first_parts.push(leading);
        }

        // Rule A first-member freeze: emit `types[0]` verbatim (paren-transparent). Frozen
        // via a leading-run directive (`freeze_first`) or an alone-on-line directive
        // in the in-span leading gap after a leading `&`.
        let first_frozen =
            self.list_member_frozen(intersection.span.start, intersection.types, 0, freeze_first);
        // A stripped shell's trailing run lifted off the FIRST member, held back from its
        // doc so the loop can build it inside the continuation `indent` when the boundary
        // takes one ([`Self::member_hoisted_trailing_shell`]). Held unconditionally and
        // placed by the loop, which is the same shape the forced-multiline path uses — the
        // boundary's own answer is the one thing that decides it, and re-deriving it here
        // would be that rule spelled twice.
        let mut held_trailing_run = None;
        if first_frozen {
            first_parts.push(self.build_frozen_member_doc(first_type, member_parens));
        } else if let Some(inner) =
            self.intersection_first_hoisted_shell(intersection, member_parens, has_comments)
        {
            held_trailing_run = Some(self.push_hoisted_member_doc(
                &mut first_parts,
                first_type,
                inner,
                self.build_intersection_member_type_doc(inner, member_parens),
            ));
        } else {
            first_parts.push(self.build_intersection_member_type_doc(first_type, member_parens));
        }

        // Add trailing block comments after first type
        if intersection.types.len() > 1 {
            if has_comments {
                let next_type_start = intersection.types[1].span().start;
                self.push_pre_separator_block_comments(
                    &mut first_parts,
                    first_type_end,
                    next_type_start,
                    b'&',
                );
            }
            first_parts.push(d.text(" &"));
        } else if has_comments
            && let Some(trailing) = self.build_comments_between_filtered_opt(
                // Single type - include trailing comments
                first_type_end,
                intersection.span.end,
                CommentSpacing::Leading,
                CommentFilter::BlockOnly,
            )
        {
            first_parts.push(trailing);
        }

        // Special case: expanding first type with 3+ members
        //
        // Matches prettier's per-member indent logic for object-to-non-object transitions:
        // - First successor (i=1) is hugged: `} & B` (space, no indent)
        // - Further successors (i>=2) get per-member indent: `indent(" &" line C)`
        //
        // Example: `type T = { a: A } & B & C` formats as:
        //   type T = {
        //       a: A;
        //   } & B &
        //       C;
        // Continuation members follow Prettier's `printIntersectionType`
        // (`intersection-type.js`) per boundary between member `i - 1` and `i`:
        //
        // - **neither is an object** — a breakable `line` and the member indented
        //   (`indent([" &", line, doc])`); this is the only spot the intersection
        //   itself breaks.
        // - **object-adjacent** (a transition object↔non-object, or object↔object)
        //   — a hard `& ` (never breaks; the object owns its own expansion), and the
        //   member indented only once the `was_indented` latch is set.
        //
        // `was_indented` mirrors Prettier's flag: it flips on the first *transition
        // past index 1*, and gates the indent of every object-adjacent member. So an
        // object hugging the first member (`A & { … }`) — or a run of objects
        // starting at index 1 (`A & { … } & { … }`) — stays at base and its body
        // indents just one level, while a `}`→non-object tail and every later member
        // carry the continuation indent. `is_huggable_type` is Prettier's
        // `isObjectType` (`TSTypeLiteral`/`TSMappedType`), and it reads through the
        // member's redundant parens — asking it on the RAW member instead hands
        // `({ … }) & B` the breakable `line` of the neither-object arm, so pass 1 breaks
        // the `&` and pass 2 (no paren left) hugs it. The rule and its reason live on
        // `is_huggable_type`; ask it directly and never re-derive the check here.
        //
        // This subsumes the old huggable-pair / last-huggable separator special-cases
        // and the blanket `indent(continuations)`; the first member always stays at
        // base (built into `first_parts`), so a first member that breaks internally
        // is never double-indented.
        //
        // `build_intersection_type_doc_with_line_comments` ports the same per-boundary
        // object-adjacency + `was_indented` rule for the forced-multiline line-comment
        // case — keep the two in sync. They stay separate because the comment path
        // forces `hardline` (not the group-decided `line`) and emits the `&` *before*
        // the gap comments (this loop emits it after, which would swallow a line
        // comment), so a merge is not byte-identical.
        let mut parts = first_parts;
        let mut was_indented = false;
        // Whether the PREVIOUS boundary took a continuation `indent` — read only by the
        // leading-own-line-comment disjunct below, for the indent a later member's lifted
        // run was queued at.
        let mut prev_indent_member = false;
        let mut needs_group = wrap_in_group;
        // Rule A must-break, as in `build_union_type_doc`: a frozen slice is
        // `will_break`-opaque, so a multi-line frozen member forces the broken layout
        // explicitly. Seeded by the leading-run/first-gap freeze; the loop ORs in any
        // other frozen multi-line member.
        let mut freeze_multiline = freeze_first_multiline
            || self.frozen_member_forces_break(first_frozen, first_type, member_parens);
        for i in 1..intersection.types.len() {
            let prev_type = &intersection.types[i - 1];
            let prev_is_object = is_huggable_type(prev_type);
            let cur_is_object = is_huggable_type(&intersection.types[i]);
            let neither_is_object = !prev_is_object && !cur_is_object;
            let frozen =
                self.list_member_frozen(intersection.span.start, intersection.types, i, false);
            if self.frozen_member_forces_break(frozen, &intersection.types[i], member_parens) {
                freeze_multiline = true;
            }

            let (mut breaks, mut indent_member) = if neither_is_object {
                (true, true)
            } else if prev_is_object && cur_is_object {
                (false, was_indented)
            } else {
                // object↔non-object transition: indented (and opens the latch) only
                // past index 1; the index-1 transition hugs the first member at base.
                if i > 1 {
                    was_indented = true;
                    (false, true)
                } else {
                    (false, false)
                }
            };
            // The boundary must open for a relocated shell run, whichever of the two
            // hidden regions holds it ([`Self::intersection_boundary_opens_for_shell_run`]).
            //
            // A HELD run always satisfies the previous member's indent qualifier — the loop
            // queues it at this very boundary — and every member's run is now held
            // ([`Self::build_intersection_member_body_doc`]), so what remains for
            // `prev_indent_member` is the run this loop never took: a member whose pair
            // EXPANDS keeps it between its own `(`…`)`, at the level the PREVIOUS boundary
            // set. That is the whole of that flag's job now; it used to carry a later
            // member's own lifted run too, and turning that half away is what left
            // `Z & ({ x: X } // c1⏎// c2⏎) & C` carrying its run out past the `;` and
            // `{ x: X } & (a // c1⏎// c2⏎) & b` non-idempotent. Holding the run answers
            // both, which is why the decline's shapes are gone rather than sanctioned.
            if !breaks
                && self.intersection_boundary_opens_for_shell_run(
                    prev_type,
                    &intersection.types[i],
                    held_trailing_run.is_some() || (i > 1 && prev_indent_member),
                )
            {
                breaks = true;
                indent_member = true;
            }
            prev_indent_member = indent_member;
            let sep = if breaks {
                // A breakable line is the only thing that needs the group to choose
                // between flat and broken.
                needs_group = true;
                d.line()
            } else {
                d.text(" ")
            };

            let mut member: DocBuf = DocBuf::new();
            // The PREVIOUS member's held run, now that the boundary has answered. Inside
            // the indent it enters AHEAD of `sep`, because the run is a `line_suffix` and
            // must be queued before the `line` it flushes at — queued after it, it would
            // ride to the next break instead. Outside, it goes straight back beside the
            // member. Every comment in it is deferred (the predicate requires a leading
            // `//`), so neither placement can reorder it against the `&` already emitted
            // above. The `take` empties the slot before this iteration's own member fills
            // it below, so each run is placed exactly once, at the boundary that FOLLOWS
            // the member it came off.
            if let Some(run) = held_trailing_run.take() {
                if indent_member {
                    member.push(run);
                } else {
                    parts.push(run);
                }
            }
            member.push(sep);
            let body = self.build_intersection_member_body_doc(
                intersection,
                i,
                has_comments,
                frozen,
                member_parens,
            );
            // This member's own lifted run, held for the NEXT boundary — the slot the
            // `take` above just emptied. A last member holds nothing (there is no boundary
            // after it), so the slot is empty when the loop ends.
            held_trailing_run = body.held_run;
            // A leading run the author broke after carries a breakable `line`, which only
            // a group can decide — an object-adjacent boundary supplies no `line` of its
            // own, so the group has to be asked for here.
            needs_group |= body.run_breaks;
            member.extend(body.parts);
            if indent_member {
                parts.push(d.indent(d.concat(&member)));
            } else {
                parts.extend(member);
            }
        }

        // A one-member intersection runs no loop, so nothing above placed the run — and a
        // lifted run nobody prints is a DROPPED comment (docs/comments.md hazard 1).
        parts.extend(held_trailing_run);

        // A multi-line frozen member forces the broken layout (Rule A must-break):
        // the frozen slice is a `will_break`-opaque verbatim span, so force it here. No
        // fixture reaches this for an intersection (every frozen member is single-line),
        // but it keeps the union / intersection must-break rule symmetric.
        if freeze_multiline {
            d.group_break(d.concat(&parts))
        } else if needs_group {
            d.group(d.concat(&parts))
        } else {
            d.concat(&parts)
        }
    }

    /// The leading-`&` gap holds a LINE comment (`type T =⏎\t&⏎\t// c⏎\t{x: 1} & b;`):
    /// emit the WHOLE gap run (blocks and lines, in source order) here, then the body
    /// with the gap claimed — the compact path's block-only extraction can't carry a
    /// line comment, so it was dropped (docs/comments.md hazard 1). Two emission modes,
    /// both the hosts' own fixed points (pass 2 re-reads these comments OUTSIDE the
    /// intersection span — the bare leading `&` never survives formatting — via the
    /// alias `=`-gap force-break loop / the annotation continuation indent):
    ///
    /// - **plain** — trailing-prefix: the first comment trails the caller's `=`/`:`
    ///   prefix and the rest go own-line (`type T = // c⏎⇥{ x: 1 } & b;`);
    /// - **frozen first member** (an alone-on-line directive in the gap, Rule A) — the
    ///   run is emitted OWN-LINE, a hardline BEFORE it, so the directive never trails
    ///   the prefix: a trailing placement is inert, and the relocated form would lose
    ///   the freeze on pass 2 (`type T =⏎⇥// prettier-ignore⏎⇥{x: 1} & b;`). At an
    ///   `own_line` caller the run already starts its line, so no extra break.
    ///
    /// The run uses the paren-hoist path's hand-rolled `[comment, hardline]` shape, not
    /// `push_leading_comment_run`: the run always ends in a line comment's forced
    /// break, and the plain-hardline shape (author blanks between run comments
    /// collapsed) is the one-pass fixed point at BOTH hosts — blank preservation
    /// converges only at the alias (`build_trailing_comments_hang_next`, the
    /// annotation's pass-2 emitter, drops the blank: a 2-pass transient).
    ///
    /// A parenthesized first member's stripped-shell run
    /// (`intersection_first_member_hoist_comments`) rides the same emission — the shell
    /// window is contiguous with the gap, so source order holds — suppressed for a
    /// frozen first member, whose shell comments ride inside the verbatim slice.
    ///
    /// The whole doc is indented one level under the trailing prefix so continuation
    /// lines align and pass 2 reproduces the bytes; an `own_line` caller (tuple
    /// element) already supplies that level and skips it.
    fn build_intersection_leading_gap_line_comment_doc(
        &self,
        intersection: &TSIntersectionType<'_>,
        wrap_in_group: bool,
        own_line: bool,
        leading_freeze: Option<LeadingRunFreeze>,
    ) -> DocId {
        let d = self.d();
        let freeze_first = leading_freeze.is_some();
        let first_member = &intersection.types[0];
        let first_frozen =
            self.list_member_frozen(intersection.span.start, intersection.types, 0, freeze_first);
        let head = self.intersection_first_member_head_run(intersection, first_frozen);
        // To-emit axis: this is the gap's emitter (docs/comments.md).
        let mut run: CommentVec<'_> = self
            .comments_to_emit_between(intersection.span.start, first_member.span().start)
            .collect();
        run.extend(head.run.iter().copied());

        // The body, with the leading gap (and the first member's own head run) claimed
        // by the run above — the print-once seam: exactly one of the two emits it.
        let body = self.build_intersection_claimed_body_doc(
            intersection,
            wrap_in_group,
            &head,
            leading_freeze,
        );

        let mut parts = DocBuf::new();
        if first_frozen && !own_line {
            parts.push(d.hardline());
        }
        for comment in &run {
            parts.push(self.build_comment_doc(comment));
            parts.push(d.hardline());
        }
        parts.push(body);
        let doc = d.concat(&parts);
        if own_line { doc } else { d.indent(doc) }
    }

    /// Build a Doc for an intersection type with line comments between members.
    ///
    /// A line comment (or an own-line block comment) between two members forces the
    /// intersection multiline. Within that forced-multiline layout the per-member
    /// separator and indent follow Prettier's `printIntersectionType`
    /// (`intersection-type.js`), exactly as the no-comment loop in
    /// `build_intersection_type_doc` does: object-adjacent members stay space-hugged
    /// (`} & B` / `A & {`) and only a neither-object boundary — or a member with a
    /// leading own-line comment (`hasLeadingOwnLineComment`) — breaks, with
    /// continuation members indented once the `was_indented` latch is set.
    ///
    /// The one intersection-specific rule is comment **preservation**: a line comment
    /// in a member gap can never share a line with the next member, so its boundary
    /// always breaks — even where object-adjacency would otherwise glue
    /// (`{ c } // c⏎& D`, `B // c⏎& { c }`). Prettier instead `lineSuffix`-relocates
    /// that comment past the glued members to the end of the visual line (across a
    /// member boundary, even past `;`); tsv keeps it on its member's line. See
    /// conformance_prettier_ts_comments.md §Comment relocation.
    ///
    /// The doc self-indents per member (mirroring `build_intersection_type_doc`), so
    /// the caller adds no outer indent.
    ///
    /// `leading_gap` says who emits the `[span.start, first.start)` run — see
    /// [`LeadingGap`]. The first member's own shell run is answered the same way, by the
    /// claim its emitter sets ([`Self::build_intersection_claimed_body_doc`]), so this
    /// builder needs no separate "was the leading run hoisted?" input: the shell simply
    /// declines its copy and the ordinary hoist strips it for its trailing run.
    fn build_intersection_type_doc_with_line_comments(
        &self,
        intersection: &TSIntersectionType<'_>,
        leading_gap: LeadingGap,
    ) -> DocId {
        let d = self.d();
        let member_parens = union_member_parens(intersection.types.len());
        let types = &intersection.types;
        let last = types.len() - 1;

        // Rule A first-member freeze in the forced-multiline path (recomputed, gated on
        // `has_format_ignore`; the `Whole` case is returned early upstream). A frozen
        // first member and the hoist are already mutually exclusive:
        // `intersection_first_member_head_run` returns an empty run for one, so no route
        // can arrive here with both — and a freeze that lost to the hoist would have
        // double-printed the run riding inside its verbatim slice.
        let freeze_first = self
            .composite_leading_run_freeze(intersection.span.start, types)
            .is_some();
        let frozen_first = self.list_member_frozen(intersection.span.start, types, 0, freeze_first);

        // First member: leading block comments (`& /* c */ A`) + its type. Its
        // trailing `&` is emitted by the next member's iteration (it sits on this line).
        // The leading-gap route claims the whole gap run, so nothing is emitted here.
        let mut parts = DocBuf::new();
        let first = &types[0];
        if leading_gap == LeadingGap::Emit {
            parts.push(self.build_comments_between_filtered(
                intersection.span.start,
                first.span().start,
                CommentSpacing::Trailing,
                CommentFilter::BlockOnly,
            ));
        }
        // The first member's lifted paren-shell trailing run, held back from its doc so
        // the loop can build it at the indent the flush lands on — the same hold the
        // no-comment path takes, and for the same reason
        // ([`Self::member_hoisted_trailing_shell`]). Unlike there, this path cannot decide
        // the boundary ahead of the member (a comment in the gap can force it open), so
        // the run is always held and the loop puts it back where the member's own doc had
        // it — byte for byte — when the boundary turns out not to indent.
        let mut held_trailing_run = None;
        let first_doc = if frozen_first {
            self.build_frozen_member_doc(first, member_parens)
        } else if let Some(inner) =
            self.intersection_first_hoisted_shell(intersection, member_parens, true)
        {
            let mut member_parts = DocBuf::new();
            held_trailing_run = Some(self.push_hoisted_member_doc(
                &mut member_parts,
                first,
                inner,
                self.build_intersection_line_comment_member_doc(inner, member_parens),
            ));
            d.concat(&member_parts)
        } else {
            self.build_intersection_line_comment_member_doc(first, member_parens)
        };
        parts.push(first_doc);

        let mut was_indented = false;
        // Whether the PREVIOUS boundary took a continuation `indent` — read only by the
        // shell disjunct below, for the indent a run left inside an expanding pair was
        // queued at (the compact loop's own `prev_indent_member`).
        let mut prev_indent_member = false;
        for i in 1..types.len() {
            let prev = &types[i - 1];
            let cur = &types[i];
            let prev_end = prev.span().end;
            let cur_start = cur.span().start;
            let is_last = i == last;

            let amp = find_separator_position(self.source, prev_end, cur_start, b'&');

            // The after-`&` comments the author did NOT put on the operator's line: they
            // lead the member, and are emitted by the shared leading-run emitter below.
            // (Same-line ones trail the `&` inline and are emitted further down; the split
            // is by LINE, which is monotonic in source position, so the two emissions can
            // never reorder the gap the way a glue-keyed one can —
            // see [`Self::union_gap_inline_run_start`] for the sibling that could.)
            //
            // ⚠️ This is a newline BEFORE the comment, which is **not** prettier's
            // `hasLeadingOwnLineComment` (a newline AFTER) though it was once labelled so —
            // and the second arm of the chain below was gated on it under that label, which
            // was a live divergence. This collection now decides only WHICH comments lead
            // the member; whether they force the boundary open is `leading_run_ends_line`.
            let own_line_leading: CommentVec<'_> = match amp {
                Some(amp_pos) => self
                    .comments_to_emit_between(amp_pos + 1, cur_start)
                    .filter(|c| !self.is_same_line(amp_pos, c.span.start))
                    .collect(),
                None => smallvec![],
            };

            // Prettier's `hasLeadingOwnLineComment`: does any comment in the member's
            // leading run carry a newline AFTER its `*/`, so that the run ENDS A LINE?
            // Asked in two places below, so it is resolved once.
            //
            // ⚠️ **Not `!own_line_leading.is_empty()`**, which was what the second arm used
            // and is the newline BEFORE — whether the author started a fresh line for the
            // comment. The two part on a comment glued FORWARD to its member from a line of
            // its own (`&⏎/* c */ cc`): tsv broke the boundary, prettier hugs, and since
            // the glued spelling on the operator's line already hugged in both, tsv held
            // two fixed points for one program keyed on a newline that stops meaning
            // anything once the comment is glued to what follows it.
            let leading_run_ends_line = own_line_leading.iter().any(|c| !self.comment_hugs_next(c));

            // Per Prettier's `printIntersectionType` per-boundary branch, on the members'
            // `isObjectType` (matching the no-comment loop — `is_huggable_type` reads
            // through a member's redundant parens, and its doc says why):
            // - both objects → space-hug, indent only once `was_indented` is latched;
            // - neither object, or a leading own-line comment → break + indent;
            // - object↔non-object transition → space-hug, indent (and latch) past index 1.
            //
            // ⚠️ The order matters beyond which arm prints: only the THIRD arm latches
            // `was_indented`, so moving the own-line-comment test out of the second arm
            // would change every later boundary. It stays where prettier asks it.
            let prev_obj = is_huggable_type(prev);
            let cur_obj = is_huggable_type(cur);
            let (mut should_break, mut indent_member) = if prev_obj && cur_obj {
                (false, was_indented)
            } else if (!prev_obj && !cur_obj) || leading_run_ends_line {
                (true, true)
            } else {
                let ind = i > 1;
                if ind {
                    was_indented = true;
                }
                (false, ind)
            };
            // Preserve: an isolated comment (any line comment, or an own-line block) in
            // the gap can't be inline, so its boundary breaks even where object-adjacency
            // would glue — the tsv/Prettier divergence this path exists for.
            //
            // ⚠️ The break carries the INDENT with it. The two are one decision in every
            // arm above — a hug arm answers `false` to both — so forcing only the break
            // here left the member at the enclosing indent, i.e. column 0 for a type-alias
            // RHS (`type T = a & // c⏎{ x: X };`). It reads as an un-indented continuation
            // only at the boundaries a hug arm chose (`i == 1`, or object-adjacency before
            // `was_indented` latches); one member later the same shape indents, which is
            // what made it look like an object-member quirk rather than this pairing.
            //
            // ⚠️ **A leading run that ENDS A LINE cannot ride the FIRST arm either**, which
            // the arm chain cannot express: prettier's own chain answers a both-objects
            // boundary before `hasLeadingOwnLineComment` is ever asked, so the same
            // predicate has to be re-applied to whatever the chain hugged.
            //
            // A run whose last comment carries a newline (or a blank) after its `*/` makes
            // [`Printer::push_leading_comment_run`] end the line, and a hug arm that keeps
            // the member on the operator's line then reads its OWN output back as a
            // same-line-after-`&` run on pass 2 — where the separator is an unconditional
            // space, so the break (and any author blank inside it) is eaten. Two passes,
            // two forms. The isolation guard above cannot answer it: a run's second half
            // has a comment before it on its line, so it is not isolated, yet the run still
            // ends the line.
            //
            // ⚠️ Prettier reaches the OPPOSITE answer here and does not converge on it —
            // its first arm hugs a both-objects boundary before this question is asked, so
            // the run re-binds across the `&` on the next pass. tsv's break is the stable
            // form, pinned by `intersection_object_adjacent_own_line_run_prettier_divergence`.
            if !should_break
                && (leading_run_ends_line
                    || self
                        .comments_on_page_between(prev_end, cur_start)
                        .any(|c| self.comment_isolated_on_its_line(c)))
            {
                should_break = true;
                indent_member = true;
            }
            // The same question over the two regions `own_line_leading` cannot see — the
            // members' own paren shells, whose runs are inside those members' spans
            // ([`Self::intersection_boundary_opens_for_shell_run`], which the compact loop
            // asks too, and which carries the previous member's indent qualifier).
            if !should_break
                && self.intersection_boundary_opens_for_shell_run(
                    prev,
                    cur,
                    held_trailing_run.is_some() || (i > 1 && prev_indent_member),
                )
            {
                should_break = true;
                indent_member = true;
            }
            prev_indent_member = indent_member;

            // The previous member's TRAILING RUN in the `prev_end`→`&` gap, claimed once
            // by the shared last-item→closer walk ([`Printer::closer_trailing_comment_run`],
            // via its `_end` boundary): the prefix of comments that FOLLOW CONTENT on their
            // line, whether that content is the member itself or the `*/` of a comment the
            // run already claimed. A **block** in it stays on the previous member's side of
            // the operator (`prev /* b */ &`) — matching the no-comment loop and Prettier;
            // a **line** comment trails the `&` instead (below), the only lossless place
            // for it. Everything past the run is on a line the author gave it and keeps it.
            //
            // ⚠️ **One walk, because the two emissions must PARTITION the gap.** Asking
            // `is_same_line(prev_end, …)` on each side of the operator — the two spellings
            // this replaces — is blind to every byte no member span covers, so a comment
            // glued to a preceding comment's `*/` read as own-line to *both* arms and was
            // MOVED across the `&` (`docs/comments.md` §Own-line-ness is a SOURCE
            // question). The union's member gap never had the bug: it routes through
            // `build_trailing_gap_comments`, which is the same claim.
            // The FIRST member's held run, now that the boundary has answered. It heads
            // `unit` when this boundary indents, so its deferred break renders at the
            // continuation level the flush lands on; otherwise it goes straight back
            // beside the member, ahead of the before-`&` blocks — its own source position,
            // and the concat the member's doc used to hold. `take` is a no-op past `i ==
            // 1`, so the run is placed exactly once.
            let mut indented_first_run = None;
            if let Some(run) = held_trailing_run.take() {
                if indent_member {
                    indented_first_run = Some(run);
                } else {
                    parts.push(run);
                }
            }

            let trailing_run_end = amp.map_or(prev_end, |amp_pos| {
                self.closer_trailing_run_end(prev_end, amp_pos)
            });
            if let Some(amp_pos) = amp {
                for comment in self
                    .comments_to_emit_between(prev_end, amp_pos)
                    .filter(|c| c.is_block && c.span.end <= trailing_run_end)
                {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
            }
            parts.push(d.text(" &"));

            let mut unit = DocBuf::new();
            unit.extend(indented_first_run);
            if let Some(amp_pos) = amp {
                // The rest of the before-`&` gap follows the operator: the run's *line*
                // comment trails it inline (a `//` can't precede the `&` without commenting
                // it out — a lossless separator-trail), and everything past the run drops to
                // its own line, keeping a glued pair together and preserving an author blank
                // ([`Printer::push_trailing_run_separator`], the rule every end-of-container
                // run reads). Then the same-line-after-`&` comments trail the operator
                // inline.
                //
                // ⚠️ **The FIRST of those own-line comments takes a plain `hardline`,
                // because it is the one that CROSSES the `&`.** An author blank ahead of it
                // was written between the previous member and the comment — a gap the
                // printer does not reproduce, since the operator is pulled back onto the
                // member's line — so re-emitting it *after* the `&` fabricates a blank the
                // author never wrote there, and prettier writes none. It is fabrication
                // rather than a stable divergence: the emitted blank lands after the `&`,
                // where the next pass reads it as the after-operator gap and drops it, so
                // the form never reaches a fixed point (F1). Every LATER separator keeps the
                // blank — a blank *between* two own-line comments never crosses anything and
                // is the author's (`& // c1⏎⏎// c2`, which both formatters hold stable).
                let mut scan_from = prev_end;
                let mut prev_comment: Option<&Comment> = None;
                let mut crossed_operator = false;
                for comment in self.comments_to_emit_between(prev_end, amp_pos) {
                    if comment.span.end <= trailing_run_end {
                        if !comment.is_block {
                            // Via `line_suffix`: the layout below breaks after a trailing
                            // `//` regardless, so the suffix flushes there byte-identically
                            // — and a run the FIRST member's stripped shell already
                            // deferred flushes ahead of it in source order instead of
                            // welding behind an inline emission
                            // (`(a // x⏎) // inj⏎& b` welded as `& // inj // x`); the
                            // flush's run separator breaks between the two
                            // (`doc/arena_render_suffix.rs`).
                            unit.push(self.build_trailing_comment_doc(comment));
                        }
                        // A block in the run went before the `&` and is not on this line at
                        // all, so nothing can be glued to it. A `//` IS on this line, and
                        // the next comment must not land behind it — it is tracked for the
                        // separator below, where `trailing_run_hugs_previous` answers "a
                        // line comment never hugs" and gives that comment its own line.
                        prev_comment = (!comment.is_block).then_some(comment);
                    } else {
                        if crossed_operator {
                            self.push_trailing_run_separator(
                                &mut unit,
                                prev_comment,
                                scan_from,
                                comment.span.start,
                            );
                        } else {
                            unit.push(d.hardline());
                        }
                        unit.push(self.build_comment_doc(comment));
                        prev_comment = Some(comment);
                        crossed_operator = true;
                    }
                    scan_from = comment.span.end;
                }
                // The comments the author wrote on the `&`'s own line, trailing it inline.
                //
                // ⚠️ **The separator asks what this unit last emitted, not the operator.**
                // A space is right only while the line still ends in the `&` — once
                // anything above emitted a comment, appending with a space WELDS this one
                // onto it, and where that predecessor is a `//` the weld is CONTENT LOSS:
                // `A⏎// x⏎& // c⏎B` printed `// x // c`, one comment whose text contains
                // the second. It reparses, it is idempotent, and the ledger still counts
                // both as printed once, so F1, round-trip, the census and the fuzzer are
                // all blind — only a prettier differential sees it. Routed through the
                // shared run rule, which hugs only a block the author glued.
                for comment in self
                    .comments_to_emit_between(amp_pos + 1, cur_start)
                    .filter(|c| self.is_same_line(amp_pos, c.span.start))
                {
                    if prev_comment.is_none()
                        || self.trailing_run_hugs_previous(prev_comment, comment.span.start)
                    {
                        // Glued at end of line (or opening it): a block inline with its
                        // space, a `//` via `line_suffix` — deferred so the FIRST
                        // member's stripped-shell run, already pending in the buffer,
                        // flushes ahead of it in source order instead of welding behind
                        // an inline emission (`(a // x⏎) & // inj⏎b` welded as
                        // `& // inj // x`); the flush's run separator breaks between the
                        // two (`doc/arena_render_suffix.rs`).
                        unit.push(self.build_trailing_comment_doc(comment));
                    } else {
                        // The run separator's non-glue arm is a real break, which flushes
                        // any pending run before this comment prints — no weld exposure.
                        self.push_trailing_run_separator(
                            &mut unit,
                            prev_comment,
                            scan_from,
                            comment.span.start,
                        );
                        unit.push(self.build_comment_doc(comment));
                    }
                    prev_comment = Some(comment);
                    scan_from = comment.span.end;
                }
            }
            // ⚠️ **The leading run is emitted OUTSIDE the break/hug choice**, because it
            // is a comment question and the choice above is a layout one. Prettier's arm
            // chain (`intersection-type.js`) asks `hasLeadingOwnLineComment` only in its
            // second arm, so an object-adjacent boundary HUGS past a leading comment —
            // and its `print()` still carries that comment, since the arms pick the
            // separator, never whether the member's own comments exist. tsv's hug arm had
            // the run inside the `else`, so every comment in it was DROPPED
            // (`{ x: 1 } &⏎/* c */ { y: 2 }` printed `{ x: 1 } & { y: 2 }`) — hazard 4,
            // an alternate-layout arm that emits only the member. The isolation guard
            // above masked the common spellings: a comment on a line of its own forces
            // the break and so reaches the other arm, leaving exactly the run GLUED
            // forward to the member to fall through — the one shape a line comment
            // cannot take, so no `//` repro exists and the ledger stayed green.
            unit.push(if should_break {
                d.hardline()
            } else {
                d.text(" ")
            });
            self.push_leading_comment_run(
                &mut unit,
                own_line_leading.iter().copied(),
                cur_start,
                LeadingGlue::Adjacent,
            );
            // Rule A between-members freeze: an own-line directive in this member's gap
            // freezes the member (paren-transparent). The directive is emitted by the
            // separator / leading-comment machinery above; only the member DOC is
            // replaced. Intersection members take no `align(2)` offset, so the bare
            // paren-transparent doc matches a reformatted sibling.
            //
            // A non-last member's lifted trailing run is HELD for the boundary that follows
            // it, exactly as the compact loop holds one — the run is a deferred
            // `line_suffix` and only that boundary can queue it at the indent it will flush
            // at ([`Self::push_hoisted_member_doc`]). This loop is the compact one's twin
            // and the two must answer one question one way; held at only one of them, the
            // same authoring reached two fixed points depending on whether some OTHER gap
            // happened to carry an isolated comment.
            let mut member_held = None;
            if self.list_member_frozen(intersection.span.start, types, i, freeze_first) {
                unit.push(self.build_frozen_member_doc(cur, member_parens));
            } else if let Some(inner) = (!is_last)
                .then(|| self.intersection_member_hoisted_shell(cur, member_parens))
                .flatten()
            {
                member_held = Some(self.push_hoisted_member_doc(
                    &mut unit,
                    cur,
                    inner,
                    self.build_intersection_line_comment_member_doc(inner, member_parens),
                ));
            } else {
                unit.push(self.build_intersection_line_comment_member_doc(cur, member_parens));
            }
            if is_last {
                for comment in self.comments_to_emit_between(cur.span().end, intersection.span.end)
                {
                    unit.push(d.text(" "));
                    unit.push(self.build_comment_doc(comment));
                }
            }

            if indent_member {
                parts.push(d.indent(d.concat(&unit)));
            } else {
                parts.extend(unit);
            }
            // This member's own held run, for the NEXT boundary — the slot the `take`
            // above emptied.
            held_trailing_run = member_held;
        }

        // A one-member intersection runs no loop, so nothing above placed the run — and a
        // lifted run nobody prints is a DROPPED comment (docs/comments.md hazard 1).
        parts.extend(held_trailing_run);

        d.concat(&parts)
    }

    /// Build a single intersection member's type doc for the line-comment path. A
    /// parenthesized-**union** member whose parens hold a leading line comment
    /// (`(// c⏎ a | b)`) is built through `build_parenthesized_union_doc` with the
    /// inner leading line comment emitted (it breaks the paren open, keeping the
    /// comment in place); `build_type_doc_maybe_parens` would re-wrap the parens but
    /// drop that comment. Every other member (block-only / non-union parens) uses the
    /// default.
    ///
    /// ⚠️ The claim question ([`Self::first_member_shell_run_claimed`]) is asked HERE and
    /// not only at the router: this layout is reached by TWO triggers, and the isolated
    /// member-comment one never asks it (`(// c⏎ | A) // x⏎ & B` routes on the `// x` in
    /// the operator gap). A claimed run built here is a SECOND emitter for one comment —
    /// the enclosing seam prints it, then the pair prints it again
    /// ([`comments.md`](../../../../docs/comments.md) hazard 3). Declining hands the
    /// member to the default builder, which is the shape the seam already assumed.
    fn build_intersection_line_comment_member_doc(
        &self,
        t: &TSType<'_>,
        member_parens: fn(&TSType<'_>) -> bool,
    ) -> DocId {
        if let Some((p, inner_union)) = self.paren_union_line_comment_member(t, member_parens)
            && !self.first_member_shell_run_claimed(t)
        {
            self.build_parenthesized_union_doc(inner_union, Some(p), ShellLeadingRun::Here)
        } else {
            self.build_intersection_member_type_doc(t, member_parens)
        }
    }

    /// The leading line-comment run the intersection hoist relocates out of the first
    /// member's stripped paren shell. Its render safety differs from the shared
    /// [`Self::stripped_paren_leading_line_comments`] by inner shape:
    ///
    /// - a **union** inner re-wraps through `build_parenthesized_union_doc`, which
    ///   re-emits the shell's leading block comments and trailing comments in place — so
    ///   the shell can hold anything and only the leading line run needs hoisting (no
    ///   block/trailing decline);
    /// - a **bare** inner strips its parens entirely, so the whole leading run (block +
    ///   line) hoists here and the stripped inner is built via `build_hang_value_doc`
    ///   (which re-attaches any trailing comment) — mirroring the keyword→value
    ///   seam. Gated on a leading **line** comment (the hang trigger): a mixed
    ///   (`(/* b */ // c⏎ A) & B`) or trailing (`(// c⏎ A /* t */) & B`) shell hoists and
    ///   settles on the same fixed point the bare authoring does; a block-only or
    ///   trailing-block-only shell has no line comment, so it stays on the idempotent
    ///   no-hoist path.
    ///
    /// Without the union carve-out, a mixed shell (`(/* b */ // c⏎ a | b) & d`) declined
    /// and dropped the line comment its inner union would have kept.
    fn intersection_first_member_hoist_comments(
        &self,
        intersection: &TSIntersectionType<'_>,
    ) -> CommentVec<'_> {
        // Both facts derive from the one node, so a caller cannot pair a first member with
        // another intersection's member-parens rule.
        let member_parens = union_member_parens(intersection.types.len());
        let Some(first_member) = intersection.types.first() else {
            return smallvec![];
        };
        let Some(shell) = outermost_paren(first_member) else {
            return smallvec![];
        };
        let inner = unwrap_parenthesized(first_member);
        // The hoist's window is [`paren_shell_gaps`]' deep leading half — the one spelling
        // of "the author's shell", so this collector cannot drift from the predicates that
        // decide the strip below.
        let (leading, _) = paren_shell_gaps(shell);
        // An ENCLOSING gap may already own this run: the intersection's first member is a
        // descent link of the leading-edge seam (`Printer::head_stripped_paren_shell`), so
        // at a gap that claimed it the hoist here would be a second emitter for one comment.
        // The hoist's own relocation is the fallback — correct only while nothing above it
        // has a better answer about the indent
        // ([`comments.md`](../../../../docs/comments.md) hazard 3).
        if self.first_member_shell_run_claimed(first_member) {
            return smallvec![];
        }
        // The pair the run sits in is RETAINED by the trailing-run rule, so it emits the
        // run inside itself ([`Printer::paren_retains_for_trailing_run`]). Hoisting would
        // strip a shell that survives, and its trailing `//` — deferred by the strip —
        // would then escape past the construct's own terminator, which is the relocation
        // that rule exists to prevent (`docs/comments.md`: a deferred run must not leave
        // the construct it was written in).
        if self.paren_retains_for_trailing_run(first_member) {
            return smallvec![];
        }
        // The pair that DIRECTLY holds the run SURVIVES, so
        // `build_intersection_line_comment_member_doc` emits the run inside it — declining
        // here is what keeps the comment where the author wrote it, the answer every LATER
        // member already gives. Hoisting it instead was also non-idempotent: it relocated
        // the run into an enclosing retained shell's `(`→member gap, which the next pass
        // renders through that gap's own emitter.
        //
        // Asked through `paren_union_line_comment_member`, the same accessor the routing
        // gate and the member emitter ask — the decline and the layout that answers it are
        // one shape by construction, and its two ⚠️s are why a redundant outer layer
        // (`(// c⏎ (a | b)) & c`, `((// c⏎ a | b)) & c`) and a pair the member rule strips
        // (`(// c⏎ | b) & c`) both still hoist below.
        if self
            .paren_union_line_comment_member(first_member, member_parens)
            .is_some()
        {
            return smallvec![];
        }
        if matches!(inner, TSType::Union(_)) {
            return self
                .comments_to_emit_between(leading.start, leading.end)
                .filter(|c| !c.is_block)
                .collect();
        }
        // Bare inner: hoist the full leading run (block + line), but only when a leading
        // line comment forces the hang — a block-only leading gap keeps its block inline
        // and is already idempotent. Collect the run once and gate on it directly (the
        // hang trigger is a line comment in the run). The trailing comment is not dropped:
        // the member is built by the ordinary hoist under this run's claim, which lifts it
        // to the boundary that follows ([`Self::push_hoisted_member_doc`]).
        let lead: CommentVec<'_> = self
            .comments_to_emit_between(leading.start, leading.end)
            .collect();
        if lead.iter().any(|c| !c.is_block) {
            return lead;
        }
        smallvec![]
    }

    /// Resolve [`IntersectionHeadRun`] for `intersection` — the first member's own leading
    /// run and the edge shell (if any) it was taken from. Empty for a frozen first member.
    fn intersection_first_member_head_run(
        &self,
        intersection: &TSIntersectionType<'_>,
        first_frozen: bool,
    ) -> IntersectionHeadRun<'_> {
        let empty = IntersectionHeadRun {
            run: smallvec![],
            claimed_shell: None,
        };
        if first_frozen {
            return empty;
        }
        let hoist = self.intersection_first_member_hoist_comments(intersection);
        if !hoist.is_empty() {
            return IntersectionHeadRun {
                run: hoist,
                // The member's OWN shell — the only shape the hoist returns a run for.
                claimed_shell: self.intersection_first_member_shell_claim(intersection),
            };
        }
        let Some(first_member) = intersection.types.first() else {
            return empty;
        };
        // An ENCLOSING gap may already own the edge shell's run — the whole intersection
        // can itself sit at some outer gap's leading edge — in which case this route would
        // be a second emitter for one run (docs/comments.md hazard 3), exactly as the
        // hoist's own `first_member_shell_run_claimed` decline is.
        let Some(claim) = self
            .leading_edge_shell_line_comment_claim(first_member)
            .filter(|claim| !self.shell_leading_run_claimed(claim.start, claim.end))
        else {
            return empty;
        };
        IntersectionHeadRun {
            run: self
                .comments_to_emit_between(claim.start, claim.end)
                .collect(),
            claimed_shell: Some(claim),
        }
    }

    /// The intersection body for a route that has already emitted the leading-`&` gap and
    /// `head`'s run — the print-once seam: the body takes [`LeadingGap::Claimed`], and the
    /// first member is either rebuilt paren-stripped (the hoist shape) or built whole with
    /// its edge shell silenced. Shared by both such routes so they cannot answer the shape
    /// question differently.
    ///
    /// ⚠️ **The hoist shape is a CLAIM, not a third layout.** It once had a body builder of
    /// its own, whose separator was the literal `" & "` — so no boundary in it could ever
    /// open, and the first member's lifted trailing run had nowhere to flush but past the
    /// `;` (`(// L⏎↹(p: 1) => void // c1⏎↹// c2⏎) & C`, and the same for every other inner
    /// the member position parenthesizes). Saying instead that the shell's leading run is
    /// already claimed hands the whole shape to [`Self::build_intersection_compact_doc`],
    /// which strips the shell for its trailing run
    /// ([`Self::member_hoisted_trailing_shell`]), holds that run for the boundary that
    /// follows, and answers the per-boundary object-adjacency rule — three answers that
    /// were being spelled a second time, or not at all. `intersection_shell_leading_run_boundary`.
    fn build_intersection_claimed_body_doc(
        &self,
        intersection: &TSIntersectionType<'_>,
        wrap_in_group: bool,
        head: &IntersectionHeadRun<'_>,
        leading_freeze: Option<LeadingRunFreeze>,
    ) -> DocId {
        // Set around the ROUTING too, not only around the body —
        // `intersection_needs_line_comment_layout` reads the claim
        // ([`Self::first_member_shell_run_claimed`]), so one set later would route on one
        // state and build in another.
        self.with_claimed_shell_leading_run(head.claimed_shell, || {
            if self.intersection_needs_line_comment_layout(intersection) {
                self.build_intersection_type_doc_with_line_comments(
                    intersection,
                    LeadingGap::Claimed,
                )
            } else {
                self.build_intersection_compact_doc(
                    intersection,
                    wrap_in_group,
                    true,
                    leading_freeze,
                    LeadingGap::Claimed,
                )
            }
        })
    }

    /// The first member's paren-shell leading region, as a claim for the route that has
    /// already emitted that run ([`Self::build_intersection_claimed_body_doc`]). The
    /// window is [`paren_shell_gaps`]' deep leading half, the one spelling of "the
    /// author's shell", so it covers exactly what the hoist collected.
    fn intersection_first_member_shell_claim(
        &self,
        intersection: &TSIntersectionType<'_>,
    ) -> Option<Span> {
        let shell = outermost_paren(intersection.types.first()?)?;
        let (leading, _) = paren_shell_gaps(shell);
        Some(Span::new(shell.span.start, leading.end))
    }

    /// Build the body of an intersection continuation member (everything except
    /// separator) for `build_intersection_type_doc`'s single per-boundary loop: leading
    /// comments + the member doc (frozen verbatim when `frozen` — Rule A) + trailing
    /// comments/`&` separator.
    ///
    /// `has_comments` is the caller's whole-intersection window answer: `false` proves
    /// both gaps around this member are bare, so neither is searched and the `&` byte
    /// scan that would bound them never runs. `frozen` is the caller's
    /// `list_member_frozen` answer (also feeding its must-break tracking), threaded so
    /// the question is asked once per member.
    ///
    /// The third return is this member's **held trailing run**, and the reason the loop
    /// carries one at all. A member's lifted run is a deferred `line_suffix` that renders
    /// its break at the indent it was QUEUED at, so the only place it can be queued is the
    /// boundary that FOLLOWS the member — which this builder has not reached. Emitting it
    /// inside the member doc queued it one level short and left the two passes disagreeing
    /// (`{ x: X } & (a // c1⏎// c2⏎) & b`), or, at a boundary that hugs, carried it out
    /// past the `;` (`Z & ({ x: X } // c1⏎// c2⏎) & C`). The FIRST member was already held
    /// this way; every later one now is, which is the same rule at every position rather
    /// than at one ([`Self::intersection_member_hoisted_shell`]).
    ///
    /// A **last** member holds nothing: no boundary follows it, so there is nowhere better
    /// to put the run than where the member's own doc has it.
    fn build_intersection_member_body_doc(
        &self,
        intersection: &TSIntersectionType<'_>,
        i: usize,
        has_comments: bool,
        frozen: bool,
        member_parens: fn(&TSType<'_>) -> bool,
    ) -> IntersectionMemberBody {
        let t = &intersection.types[i];
        let type_start = t.span().start;
        let type_end = t.span().end;
        let is_last = i == intersection.types.len() - 1;
        let mut parts = DocBuf::new();

        // Leading block comments (after the `&` separator), each followed by prettier's
        // `printLeadingComment` separator — a space where the author glued the member to
        // the comment, a breakable `line` where they broke after it. That `line` is what
        // keeps a run the author gave its own line ON that line once the intersection
        // breaks for width, while collapsing it when the intersection fits; a hardcoded
        // space glued a member onto the run's `*/` line. Only the *hardline* case (a
        // comment isolated on both sides) forces the break, and that one never reaches
        // here — `intersection_needs_line_comment_layout` routes it to the multiline
        // builder. The union's between-member path keeps its hardcoded space because its
        // one-sided gate leaves it only comments glued after the `|`; the intersection's
        // two-sided gate deliberately leaves the broke-after ones here.
        let mut run_breaks = false;
        if has_comments {
            let prev_type_end = intersection.types[i - 1].span().end;
            if let Some(sep_pos) =
                find_separator_position(self.source, prev_type_end, type_start, b'&')
            {
                let (run, breaks) =
                    self.build_member_leading_block_comments(sep_pos + 1, type_start);
                parts.push(run);
                run_breaks = breaks;
            }
        }

        // Rule A member freeze (paren-transparent). The directive itself was emitted by
        // the post-separator run above. Which placements reach this width-decided path
        // is gated by `intersection_has_isolated_member_comment` — a directive isolated
        // on BOTH sides routes to the line-comment path first (a narrower routing than
        // the union's one-sided `is_own_line_comment` gate); the freeze itself is
        // placement-keyed identically in both families (alone-on-line only).
        let mut held_run = None;
        if frozen {
            parts.push(self.build_frozen_member_doc(t, member_parens));
        } else if let Some(inner) = (!is_last)
            .then(|| self.intersection_member_hoisted_shell(t, member_parens))
            .flatten()
        {
            held_run = Some(self.push_hoisted_member_doc(
                &mut parts,
                t,
                inner,
                self.build_intersection_member_type_doc(inner, member_parens),
            ));
        } else {
            parts.push(self.build_intersection_member_type_doc(t, member_parens));
        }

        // Trailing block comments + `&` separator (or end-of-intersection comments)
        if !is_last {
            if has_comments {
                let next_type_start = intersection.types[i + 1].span().start;
                self.push_pre_separator_block_comments(&mut parts, type_end, next_type_start, b'&');
            }
            parts.push(self.d().text(" &"));
        } else if has_comments
            && let Some(trailing) = self.build_comments_between_filtered_opt(
                type_end,
                intersection.span.end,
                CommentSpacing::Leading,
                CommentFilter::BlockOnly,
            )
        {
            parts.push(trailing);
        }

        IntersectionMemberBody {
            parts,
            run_breaks,
            held_run,
        }
    }
}
