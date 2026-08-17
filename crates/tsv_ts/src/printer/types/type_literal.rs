// Type literal printing for TypeScript
//
// Handles printing of object type literals (`{ a: T; b: U }`) with:
// - Single-line and multi-line formats
// - Object alignment for union members and parenthesized intersections
// - Grouped (`Standard`) vs no-group (`NoGroup`) modes so a parent (function
//   type / type-argument list) can control breaking

use super::super::CommentVec;
use super::super::comments_to_emit_in_range;
use super::helpers::{outermost_paren, unwrap_parenthesized};
use super::union_intersection::union_member_parens;
use super::{Printer, StandaloneGlue};
use crate::ast::internal::{
    TSIntersectionType, TSParenthesizedType, TSType, TSTypeElement, TSTypeLiteral, TSUnionType,
};
use crate::printer::{MemberBlankScan, MemberBody, MemberFreeze, MemberSeam, ShellLeadingRun};
use smallvec::{SmallVec, smallvec};
use tsv_lang::Span;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// Mode for building type literal docs.
enum TypeLiteralMode {
    /// Width-aware with softlines, wrapped in group
    Standard,
    /// Width-aware with softlines, no group (parent controls breaking)
    NoGroup,
}

/// How [`Printer::build_type_doc_maybe_parens_impl`]'s default paren arm levels the
/// operand it wraps — and, with it, whether the trailing-object-intersection alignment
/// applies at all.
#[derive(Clone, Copy, PartialEq)]
enum DefaultParenIndent {
    /// Wrap the inner type in `d.indent`: the union-member offset / conditional
    /// check-`extends` depth. The trailing-object alignment (`| {` double indent) is
    /// correct only here.
    Nested,
    /// Leave the inner type bare — an **intersection** member takes its level from the
    /// intersection printer's own `& `-line indent, so a second one here is spurious.
    Bare,
}

impl<'a> Printer<'a> {
    //
    // Comment partitioning helpers
    //

    /// The per-family facts of a type-literal member body, for the shared member-body
    /// walk ([`Printer::build_member_list_docs_into`]): a type member's `;` is the
    /// WALK's, so its trailing run partitions around that separator
    /// ([`MemberSeam::AroundSeparator`]) and a format-ignore freeze stops at the
    /// member's content end rather than carrying a second terminator out with it.
    ///
    /// `delimiter_pull_pos` is the opening `{`'s own position where the caller pulled a
    /// delimiter-line comment onto it, so this walk drops the same comment from the first
    /// member's leading set — both callers supply it, since both keep such a comment on
    /// the `{` line. `freeze` is [`MemberFreeze::Never`] for the alignment caller — a
    /// union-member / parenthesized-intersection object type has never honored a directive
    /// in a member gap.
    fn type_literal_member_body(
        &self,
        t: &TSTypeLiteral<'_>,
        comments_present: bool,
        delimiter_pull_pos: Option<u32>,
        freeze: MemberFreeze,
    ) -> MemberBody {
        MemberBody {
            span: t.span,
            has_comments: comments_present,
            delimiter_pull_pos,
            blank_scan: MemberBlankScan::PastComments,
            freeze,
            seam: MemberSeam::AroundSeparator,
        }
    }

    /// The `{`-line comment pull a type literal's **forced-multiline** body takes: the
    /// prefix docs to emit right behind the `{`, and the position the member walk excludes
    /// them at ([`Printer::delimiter_line_comment_prefix`], the open-delimiter family).
    ///
    /// Both renderings ask it — the plain literal ([`Self::build_type_literal_doc_inner`])
    /// and the union-member / parenthesized-intersection **alignment** one
    /// ([`Self::build_aligned_object_literal_doc`], members double-indented, `}` at the
    /// member's `align(2)` offset). They differ only in geometry, and a rendering difference
    /// is no reason for a comment-position one; two spellings of that decision is how they
    /// came to answer this gap differently for as long as they did, keyed on nothing the
    /// author can see — whether the object happened to be a union member.
    ///
    /// Only the forced-multiline arm asks, and it needs no gate of its own: a `//` or an
    /// own-line comment is exactly what forces that arm, and an inline block already hugs
    /// the first member on the width-aware one. Empty of both with no members — an empty
    /// body has no first member to pull ahead of, and its own emitter owns the braces.
    fn type_literal_brace_line_pull(
        &self,
        t: &TSTypeLiteral<'_>,
        comments_present: bool,
    ) -> (DocBuf, Option<u32>) {
        match t.members.first() {
            Some(first) if comments_present => {
                self.delimiter_line_comment_prefix(t.span.start, first.span().start)
            }
            _ => (DocBuf::new(), None),
        }
    }

    /// Build docs for block comments between the opening brace and the first
    /// member, emitted inline (`{/* c */ a: number}`). Used by the non-multiline
    /// type-literal paths, where leading comments before the first member would
    /// otherwise be dropped. Line / own-line comments don't reach here — they
    /// force the multiline path via `type_literal_force_multiline`.
    fn build_type_literal_leading_comments_inline(
        &self,
        brace_start: u32,
        first_member_start: u32,
    ) -> DocBuf {
        let d = self.d();
        let mut docs = DocBuf::new();
        for comment in comments_to_emit_in_range(self.comments, brace_start + 1, first_member_start)
        {
            docs.push(self.build_comment_doc(comment));
            docs.push(d.text(" "));
        }
        docs
    }

    /// Build docs for trailing comments partitioned around a semicolon.
    ///
    /// Returns docs for: `[space + comment]* ";" [space + comment]*`
    ///
    /// Comments are positioned relative to the source member separator (`;` or
    /// `,`) found in the range `member_end..upper_bound`. With no source separator
    /// (newline/ASI-separated members) the `;` is synthesized right after the
    /// member, so every comment in the gap leads the next member (after `;`). Each
    /// comment is emitted via
    /// `build_trailing_comment_doc` — block inline, line through `line_suffix` so a
    /// long trailing comment never forces the member's own type (e.g. a union) to
    /// break (matches prettier and the interface-member path).
    ///
    /// ⚠️ `deferred` — the member's OWN member→`;` gap run, the one the caller collected
    /// while building the member doc — lands between the `;` and the after-separator
    /// comments, which is the order [`Printer::build_member_with_semicolon_doc`] gives the
    /// interface and class. Emitting it last instead put the trailing run ahead of a
    /// deferred `//`, so the two shared one `line_suffix` flush: a second `//` WELDED onto
    /// the first (`a: A // c1⏎/* c2 */; // c3` reparsing as ONE comment) and a block came
    /// out REORDERED ahead of it, where both other member containers keep them apart.
    fn build_comments_around_semicolon_doc(
        &self,
        comments: &[&tsv_lang::Comment],
        member_end: u32,
        upper_bound: u32,
        deferred: DocBuf,
    ) -> DocBuf {
        let d = self.d();
        // Comment-free gap (the common case): no separator scan needed — the
        // partition below reduces to the bare `;`.
        if comments.is_empty() {
            let mut docs = DocBuf::with_capacity(1 + deferred.len());
            docs.push(d.text(";"));
            docs.extend(deferred);
            return docs;
        }
        // Find the source member separator — `;` OR `,` (both are valid type-member
        // separators; tsv normalizes either to `;`). Comment-aware so a separator
        // glyph inside a comment in this gap isn't mistaken for the real one. Taking
        // the earlier of the two handles whichever the author used; with neither
        // (newline/ASI-separated members) there is no anchor and all comments are
        // "before". Keying only on `;` here put a comment that followed a `,`
        // separator on the wrong side (`a: 1 /* c */;` instead of `a: 1; /* c */`).
        let sep_pos = self.type_member_separator_pos(member_end, upper_bound);

        let (before_semi, after_semi): (Vec<_>, Vec<_>) =
            comments.iter().partition(|c| match sep_pos {
                Some(pos) => c.span.start < pos,
                // No source separator (newline/ASI-separated members): the `;` is
                // synthesized right after the member, so every comment in the gap
                // leads the next member and goes after the `;`.
                None => false,
            });

        let mut docs =
            DocBuf::with_capacity(before_semi.len() + after_semi.len() + 1 + deferred.len());
        for comment in before_semi {
            docs.push(self.build_trailing_comment_doc(comment));
        }
        docs.push(d.text(";"));
        // The member's own deferred run, before anything that trails the separator.
        docs.extend(deferred);
        for comment in after_semi {
            docs.push(self.build_trailing_comment_doc(comment));
        }
        docs
    }

    /// [`Printer::trailing_claim_end`] for the gap after a member ending at
    /// `member_content_end`, from its separator floor ([`Self::type_member_gap_floor`])
    /// toward the next member — unbounded (`u32::MAX`) after the last member, toward
    /// whose `}` nothing leads. The one spelling for every reader of a member gap (the
    /// shared force-multiline walk's trailing collect, the width-aware trailing collect,
    /// and the width-aware handed-over run), so the two sides cannot disagree.
    pub(in crate::printer) fn type_member_claim_end(
        &self,
        member_content_end: u32,
        next_start: Option<u32>,
    ) -> u32 {
        match next_start {
            Some(start) => self
                .trailing_claim_end(self.type_member_gap_floor(member_content_end, start), start),
            None => u32::MAX,
        }
    }

    /// Collect the comments a member's trailing run claims in the force-multiline
    /// walk — the same-line run after its content end
    /// ([`Printer::trailing_same_line_comments`], which follows a multi-line block to its
    /// closing `*/` line), stopped at the claim split
    /// ([`Self::type_member_claim_end`]): a comment hugging the next member's start
    /// leads it instead (`a: A; /* c */ b: B;`), emitted by that member's leading run
    /// (the shared walk's `leads_target` escape).
    ///
    /// `deferred_line_comment` empties the run: the member's own doc ends with a line
    /// comment its member→`;` gap deferred past the `;`, so nothing may share that output
    /// line. Claiming here anyway lands the run in the same `line_suffix` flush, where a
    /// second `//` WELDS onto the first (the pair reparsing as ONE comment) and a block
    /// comes out REORDERED ahead of it. The comments go to the next member's leading run
    /// instead, via its `claims_trailing`.
    fn collect_member_trailing_comments(
        &self,
        member_content_end: u32,
        upper_bound: u32,
        next_start: Option<u32>,
        comments_present: bool,
        deferred_line_comment: bool,
    ) -> CommentVec<'_> {
        if !comments_present || deferred_line_comment {
            return CommentVec::new();
        }
        let claim_end = self.type_member_claim_end(member_content_end, next_start);
        self.trailing_same_line_comments(member_content_end, upper_bound.min(claim_end))
    }

    /// The cursor past everything a member's own emitters printed: its span end, or the
    /// end of the last comment its trailing run CLAIMED when that reaches further.
    ///
    /// The two differ by exactly the region a re-spelled `is_same_line` filter used to
    /// miss — a run that followed a multi-line block to its closing `*/` line ends past
    /// the member's `;`. Leaving the cursor at the span end there hands the same comments
    /// to the end-of-body emitter, whose own anchor
    /// ([`Printer::comment_already_trailed`], asked of the *item's* line) cannot see that
    /// closing line and so DOUBLE-PRINTS them. The class body states the same advance as
    /// `find_end_with_trailing_comments(..).min(claim_end)`; here the claim is already in
    /// hand, so it is read off the run rather than recomputed — the two cannot then
    /// disagree about what was claimed.
    fn member_cursor_past_trailing(&self, span_end: u32, trailing: &[&tsv_lang::Comment]) -> u32 {
        trailing
            .last()
            .map_or(span_end, |c| c.span.end)
            .max(span_end)
    }

    /// The whole trailing arm of the type-literal member seam — the type-member arm of
    /// the shared member-body walk ([`MemberSeam::AroundSeparator`]), where the class and
    /// interface bodies take [`Printer::member_trailing_run`].
    ///
    /// It cannot BE that call: those two emit a run and advance a cursor, while a type
    /// member must partition its run around its own `;` and slot the member→`;` gap's
    /// deferred comments between the two halves
    /// ([`Self::build_comments_around_semicolon_doc`]). Keeping that difference
    /// expressible is why the walk names its seam rather than assuming one.
    ///
    /// Returns the walk's new `(prev_end, prev_deferred_line_comment)`. The order of the
    /// two is load-bearing: the deferred flag is read by the run collector (an emptied run
    /// is how a deferred `//` keeps its output line to itself) and again by the NEXT
    /// member's leading run and the body's end-of-run emitter, as their `claims_trailing`.
    pub(in crate::printer) fn push_type_member_trailing_seam(
        &self,
        member_parts: &mut DocBuf,
        body: MemberBody,
        member_span: Span,
        member_content_end: u32,
        next_start: Option<u32>,
        deferred: DocBuf,
    ) -> (u32, bool) {
        let upper_bound = next_start.unwrap_or(body.span.end);
        let deferred_line_comment = body.has_comments
            && self.terminator_defers_line_comment(member_span.start, member_span.end);
        let trailing = self.collect_member_trailing_comments(
            member_content_end,
            upper_bound,
            next_start,
            body.has_comments,
            deferred_line_comment,
        );
        member_parts.extend(self.build_comments_around_semicolon_doc(
            &trailing,
            member_content_end,
            upper_bound,
            deferred,
        ));
        (
            self.member_cursor_past_trailing(member_span.end, &trailing),
            deferred_line_comment,
        )
    }

    /// The member's source separator (`;` or `,` — both valid, tsv normalizes to `;`)
    /// in `[member_end, upper_bound)`, comment-aware so a separator glyph inside a
    /// comment isn't mistaken for the real one. `None` for newline/ASI-separated
    /// members.
    fn type_member_separator_pos(&self, member_end: u32, upper_bound: u32) -> Option<u32> {
        let semi = self.find_char_outside_comments(member_end, upper_bound, b';');
        let comma = self.find_char_outside_comments(member_end, upper_bound, b',');
        [semi, comma].into_iter().flatten().min()
    }

    /// Where the trailing-claim split ([`Printer::trailing_claim_end`]) opens in the
    /// gap after a type member: past the member's source separator when one exists,
    /// else at the member's content end. A comment BEFORE the separator trails its
    /// member however the lines fall (`a: A /* c */; b: B` keeps `/* c */` with `a`),
    /// so the leads-next test must not reach it — the separator is this gap's slot
    /// floor, the same role a dropped `;` plays in a statement list
    /// ([`statement_gap_floor`](crate::printer::statement_gap_floor)).
    fn type_member_gap_floor(&self, member_content_end: u32, upper_bound: u32) -> u32 {
        self.type_member_separator_pos(member_content_end, upper_bound)
            .map_or(member_content_end, |pos| pos + 1)
    }

    //
    // Type parenthesization with special object handling
    //

    /// Build type doc, wrapping in parentheses if the predicate returns true.
    ///
    /// Object-bearing members align via `build_aligned_object_literal_doc`:
    /// properties are double-indented and the closing `})` takes the member's
    /// `align(2)` sub-tab offset (2 literal spaces), matching Prettier's
    /// `align(2, …)`. The plain default case only indents the inner type.
    ///
    /// Special case: intersection with trailing object type builds a custom doc
    /// so that `})` can be aligned properly (one indent level past the base).
    pub(super) fn build_type_doc_maybe_parens(
        &self,
        ts_type: &TSType<'_>,
        needs_parens: fn(&TSType<'_>) -> bool,
    ) -> DocId {
        self.build_type_doc_maybe_parens_impl(
            ts_type,
            needs_parens,
            DefaultParenIndent::Nested,
            ShellLeadingRun::Upstream,
        )
    }

    /// Optional-tuple-element variant of [`Self::build_type_doc_maybe_parens`]: a paren
    /// shell's own leading **line** comment is emitted INSIDE the parens.
    ///
    /// The shared entry point declines that run ([`ShellLeadingRun::Upstream`]) on the
    /// argument that an upstream emitter already placed it. An optional tuple element has
    /// no upstream at all — its run was simply DROPPED (`[(// c⏎T | U)?]` → `[(T | U)?]`)
    /// — so it asks for [`ShellLeadingRun::Here`]. A licence stops where its argument
    /// stops.
    ///
    /// The flag reaches only the parenthesized-**union** arm now; the required pair around
    /// every other operand kind opens over its own leading run unconditionally, since the
    /// licence's argument fails at a LATER union / intersection member exactly as it fails
    /// here. See [`Self::build_type_doc_maybe_parens_impl`]'s ⚠️.
    pub(super) fn build_optional_element_type_doc(
        &self,
        ts_type: &TSType<'_>,
        needs_parens: fn(&TSType<'_>) -> bool,
    ) -> DocId {
        self.build_type_doc_maybe_parens_impl(
            ts_type,
            needs_parens,
            DefaultParenIndent::Nested,
            ShellLeadingRun::Here,
        )
    }

    /// Intersection-member variant of `build_type_doc_maybe_parens`: the default
    /// parenthesization emits a **bare** `("(" inner ")")` with no inner `d.indent`.
    ///
    /// Prettier never indents parenthesized-type content in the general
    /// `needsParens` case (`["(", doc, ")"]`); an intersection member's level comes
    /// entirely from `printIntersectionType`'s own `indent([" &", line, …])` wrapper
    /// (or, for the first / object-adjacent member, none). So a parenthesized
    /// function / constructor / conditional member sits at the member indent, not one
    /// level deeper. tsv's shared default `d.indent` (kept for other callers) matches
    /// the depth in a conditional check/extends paren — but for an **intersection**
    /// member it is a spurious extra level. (Union members take their `align(2)` offset
    /// from `build_union_member_offset_doc`, which uses *this* bare variant and applies
    /// the offset itself — so they don't route through the shared `d.indent` either.)
    /// Hence this dedicated entry point for the intersection member sites only. Fixes bug141
    /// §Bug 2 case 3; guarded by `intersection_paren_constructor` /
    /// `intersection_paren_conditional`.
    pub(super) fn build_intersection_member_type_doc(
        &self,
        ts_type: &TSType<'_>,
        needs_parens: fn(&TSType<'_>) -> bool,
    ) -> DocId {
        self.build_type_doc_maybe_parens_impl(
            ts_type,
            needs_parens,
            DefaultParenIndent::Bare,
            ShellLeadingRun::Upstream,
        )
    }

    /// Shared implementation of the three entry points above.
    /// [`DefaultParenIndent`] gates two paren cases at once — the default paren's own
    /// `d.indent`, and whether the trailing-object-intersection alignment applies —
    /// which together keep an intersection member one level shallower than the
    /// union-member / conditional layout. [`ShellLeadingRun`] names who emits a shell's
    /// leading line-comment run.
    fn build_type_doc_maybe_parens_impl(
        &self,
        ts_type: &TSType<'_>,
        needs_parens: fn(&TSType<'_>) -> bool,
        default_paren_indent: DefaultParenIndent,
        shell_leading_run: ShellLeadingRun,
    ) -> DocId {
        let d = self.d();
        if needs_parens(ts_type) {
            // Special case: intersection with trailing object type — build a custom
            // doc that aligns the trailing object's body + closing `})` with the
            // union member's `| {` offset (`build_aligned_object_literal_doc`'s double
            // indent). That alignment is correct only in a union-member context
            // (`DefaultParenIndent::Nested`, via `build_type_doc_maybe_parens` /
            // `build_union_member_offset_doc`); for an *intersection* member
            // (`build_intersection_member_type_doc`, `DefaultParenIndent::Bare`)
            // the trailing object hangs one level too deep. There the default arm
            // below (bare `("(", inner, ")")`, no inner indent) reproduces the array
            // path's single-indent layout, matching prettier. See
            // `intersection_paren_member_trailing_object_long`.
            // `aligned_trailing_object_shell` is the whole question — including which
            // comments the aligned opening can hold; a `//` in its member gaps declines
            // it and falls through to the default arm below.
            if default_paren_indent == DefaultParenIndent::Nested
                && let Some((intersection, obj)) = self.aligned_trailing_object_shell(ts_type)
            {
                return self.build_parenthesized_intersection_trailing_object_doc(
                    intersection,
                    obj,
                    outermost_paren(ts_type),
                );
            }

            // Special case: parenthesized union type
            if let TSType::Union(union) = unwrap_parenthesized(ts_type) {
                return self.build_parenthesized_union_doc(
                    union,
                    outermost_paren(ts_type),
                    shell_leading_run,
                );
            }

            // The union arm above renders a shell's own leading LINE run inside the
            // required pair, opening it. Every other operand kind reaches the same
            // position through the default arm below, where the run would render GLUED to
            // the `(` — a `(` with no space before the `//`, the operand never indented
            // into the shell, and the `)` welded onto its tail. That third form is what
            // [`Self::build_open_required_paren_doc`] exists to prevent, so every operand
            // kind gets the union treatment here and one position stops having two
            // spellings. Only a `//` asks: a block stays inline in both.
            //
            // ⚠️ Asked of the SHELL, never of the caller's [`ShellLeadingRun`]. That
            // licence is granted on the argument that an emitter above already placed the
            // run — true of an intersection's FIRST member (whose hoist reaches the union
            // arm above, not here) and of a union's REDUNDANT member (whose pair is not
            // required, so it never enters this branch at all), and false of every LATER
            // member, where nothing upstream claims it. Reading the flag here left those
            // members welded while their first-member and array/indexed-access siblings
            // opened — one question with two answers at sibling positions.
            //
            // The shell need not BE the operand: it can sit at the operand's leading
            // printed edge, where its run lands just inside this same pair
            // ([`Printer::required_paren_open_run`]).
            if let Some(run) = self.required_paren_open_run(ts_type) {
                return self.build_open_required_paren_doc(ts_type, run);
            }

            // Default case: parenthesize the inner type. The inner `d.indent`
            // (union-member offset / conditional check-extends depth) is dropped
            // for intersection members, which take their level from the
            // intersection printer's own `& `-line indent — see
            // `build_intersection_member_type_doc`.
            // Through the shared required-pair seam, so a shell that already prints its
            // own `(`…`)` doesn't get a second one — `[((⏎↹A extends B ? C : D // c⏎))?]`
            // where the comment-free authoring prints `[(A extends B ? C : D)?]`.
            self.build_required_paren_operand_doc(ts_type, |inner| {
                d.concat(&[
                    d.text("("),
                    match default_paren_indent {
                        DefaultParenIndent::Nested => d.indent(inner),
                        DefaultParenIndent::Bare => inner,
                    },
                    d.text(")"),
                ])
            })
        } else {
            // Type-operand positions (union/intersection members, conditional
            // check/extends types, optional tuple elements) break the OUTERMOST
            // generic first, matching Prettier's `printTypeParameters`. Use the
            // wrapping type-args path so a nested non-huggable generic like
            // `Outer<Inner<...>>` wraps the outer `Outer<>` instead of force-inlining
            // the single arg and breaking only the inner `Inner<>`.
            self.build_type_doc(ts_type)
        }
    }

    /// A parenthesized union in element/object position that would break EXPANDS its
    /// parens (`(⏎\t| A⏎\t| B⏎)`, prettier's `printUnionType` needs-parens branch)
    /// instead of gluing the leading `|` to the `(`. Returns `None` for any other type
    /// — the caller keeps its existing layout. A *commented* union takes this layout
    /// too, with the shell's own two gaps emitted by the paren builder (see the body).
    /// `ty` is the element / object as authored (parens included): it is unwrapped to
    /// reach the union, and its own span (parens and all) is the comment window. Shared
    /// by the array-element (`build_array_type_doc`) and indexed-access-object arms; the
    /// indexed-access *index* uses bracket delimiters, not parens, so it expands
    /// inline rather than through here. See `type_param_fits_rhs_long`.
    pub(super) fn build_expanded_parenthesized_union_opt(&self, ty: &TSType<'_>) -> Option<DocId> {
        if let TSType::Union(u) = unwrap_parenthesized(ty)
            && !self.union_prints_hugged(u)
        {
            // The real paren node, not `None`: with it the builder emits the shell's own
            // two gaps, so a commented shell takes this expanded layout too. Declining on
            // any comment (what this replaced) sent it to the glued-paren fall-through,
            // which welds the leading `|` to the `(` **and** leaves the shell's trailing
            // gap to an outer emitter — a member's trailing `// c` escaped past the `)`
            // and merged with whatever else flushed on that line. A comment-free shell is
            // unaffected: both gap scans find nothing.
            //
            // `outermost_paren`, whose doc carries why an inner layer would truncate the
            // gap scan (`((A | B) /* c */)[]`).
            Some(self.build_parenthesized_union_doc(u, outermost_paren(ty), ShellLeadingRun::Here))
        } else {
            None
        }
    }

    /// Build doc for a union type wrapped in parentheses.
    ///
    /// Prettier uses `group([indent(mainParts), softline])` when `pathNeedsParens`
    /// is true for a union, so that when the group breaks, `(` and `)` get their
    /// own lines with the union content indented:
    /// ```text
    /// (
    ///   | { a: string }
    ///   | { b: string }
    /// )
    /// ```
    ///
    /// When `paren` is supplied (the union's parens are retained from source, not
    /// synthetic), block comments the user wrote inside the parens are preserved in
    /// place — a leading comment after `(` (`(/* c */ a | b)`) and a trailing comment
    /// before `)` (`(a | b /* c */)`). Prettier hoists these out of the parens; tsv
    /// keeps them with the parenthesized member. A trailing *line* comment before `)`
    /// is preserved here too (forcing the group to break). A leading *line* comment
    /// after `(` is only emitted here for [`ShellLeadingRun::Here`] — the paren-union
    /// member arms of `build_union_type_doc_with_line_comments`, which keep such a
    /// comment inside the parens for EVERY member (first or later); other callers pass
    /// [`ShellLeadingRun::Upstream`] because a leading line comment has already been
    /// handled upstream (relocated or emitted before the member).
    pub(super) fn build_parenthesized_union_doc(
        &self,
        union: &TSUnionType<'_>,
        paren: Option<&TSParenthesizedType<'_>>,
        shell_leading_run: ShellLeadingRun,
    ) -> DocId {
        let d = self.d();
        let union_doc = self.build_union_type_doc(union);

        let mut needs_break = false;
        // A `//` the author glued to the `(` keeps that line, as at every other opening
        // delimiter (`split_open_delimiter_glued_run`); the `softline` below is then
        // the break that ends the comment's line. The doc and the position the rest of the
        // run resumes at travel together, so there is no resume value to read when nothing
        // was glued.
        let glued = paren
            .filter(|_| shell_leading_run == ShellLeadingRun::Here)
            .and_then(|p| {
                let (doc, resume) =
                    self.split_open_delimiter_glued_run(p.span.start + 1, union.span.start);
                doc.map(|doc| (doc, resume))
            });
        needs_break |= glued.is_some();
        let mut indented: DocBuf = smallvec![d.softline()];
        if let Some(p) = paren {
            // Leading comments between `(` and the union. Block comments stay inline
            // (`(/* c */ a | b)`). A leading *line* comment reaches here only for
            // `ShellLeadingRun::Here` — the paren-union member of an outer union, whose
            // comment tsv keeps inside the parens leading the inner union (for every
            // member, not just the first). A line comment must end its line, so it forces
            // the paren group to break.
            needs_break |= self.push_paren_shell_leading_run(
                &mut indented,
                glued.map_or(p.span.start + 1, |(_, resume)| resume),
                union.span.start,
                shell_leading_run,
            );
        }
        indented.push(union_doc);
        if let Some(p) = paren {
            // Trailing comments between the union and `)`: a block comment stays
            // inline (`(a | b /* c */)`); a line comment defers to end-of-line and
            // forces the paren group to break. The inner union has its own group,
            // but the line comment's `break_parent` (below) propagates, expanding it
            // to one member per line.
            needs_break |=
                self.push_trailing_comments_in_range(&mut indented, union.span.end, p.span.end - 1);
        }

        let mut inner_parts: DocBuf = smallvec![d.indent(d.concat(&indented)), d.softline()];
        if needs_break {
            // Unscoped, deliberately: this shell is RETAINED (`d.parens` below), so its
            // doc regenerates identically on the reparse and the force is reproducible —
            // `flush_break` is only for stripped shells
            // (`build_parenthesized_type_unwrap_doc`'s trailing arm).
            inner_parts.push(d.break_parent());
        }
        let inner = d.group(d.concat(&inner_parts));
        match glued {
            Some((doc, _)) => d.concat(&[d.text("("), doc, inner, d.text(")")]),
            None => d.parens(inner),
        }
    }

    //
    // Object alignment helpers (for unions and parenthesized intersections)
    //

    /// Build doc for `(A & B & { members })` with proper alignment.
    ///
    /// Aligns `})` one indent level past the base when breaking:
    /// ```text
    /// | (A & {
    ///         prop: T;
    ///   })
    /// ```
    ///
    /// For short objects, stays inline: `(A & {c: C})`
    ///
    /// This requires separating `{` and `}` from the TypeLiteral so we can:
    /// - Print `{` inline with `(A &`
    /// - Print members with double indent (whole tabs)
    /// - Print `})` at the base + the member's `align(2)` sub-tab offset (2 literal
    ///   spaces) when breaking, matching Prettier's `align(2, …)`.
    fn build_parenthesized_intersection_trailing_object_doc(
        &self,
        intersection: &TSIntersectionType<'_>,
        trailing_obj: &TSTypeLiteral<'_>,
        paren: Option<&TSParenthesizedType<'_>>,
    ) -> DocId {
        let d = self.d();
        // Build opening: (A & B & {
        let mut opening_parts: DocBuf = smallvec![d.text("(")];

        // Comments the author wrote inside retained parens, ahead of the intersection
        // (`(/* c */ a & { … })`, `(// c⏎a & { … })`) — kept in place, as the union
        // sibling `build_parenthesized_union_doc` keeps them. This function is handed the
        // already-unwrapped `intersection`, so the paren's own gap is invisible to every
        // other emitter and a comment there would be silently DROPPED.
        //
        // A *line* comment reaches here only for a FIRST union member: a later member's
        // is relocated to trail the previous member by
        // `build_union_type_doc_with_line_comments`, which then builds the inner type
        // directly and never routes here — so there is no double-print to guard against.
        if let Some(p) = paren {
            // The opening-delimiter rule, at the one shell whose `(` is this builder's own
            // text rather than a wrapper's: the author's glued `//` claims that line
            // (`split_open_delimiter_glued_run`) and an own-line run keeps its own,
            // so both authorings are fixed points here as at every other bracket. It used
            // to glue either way — the run rode directly behind the `(` with no break in
            // front of it.
            let (glued, resume) =
                self.split_open_delimiter_glued_run(p.span.start + 1, intersection.span.start);
            let glued_here = glued.is_some();
            opening_parts.extend(glued);
            let mut lead: DocBuf = DocBuf::new();
            let has_line = self.push_paren_shell_leading_run(
                &mut lead,
                resume,
                intersection.span.start,
                ShellLeadingRun::Here,
            );
            // A `//` on the `(` line — the author's, or the first this run emits — has to
            // end that line before anything else goes on it, and this builder supplies no
            // break of its own (the `A & {` follows immediately). A **block**-only run
            // stays inline (`(/* c */ A & {`) and takes neither.
            let mut run: DocBuf = DocBuf::new();
            if glued_here || has_line {
                run.push(d.hardline());
            }
            run.extend(lead);
            if !run.is_empty() {
                // `indent`, because that break places the intersection's own first line:
                // it belongs one level in from the `(`, matching the default-paren path
                // (`d.indent(self.build_type_doc(…))` above) that every other paren-retained
                // member shape takes.
                opening_parts.push(d.indent(d.concat(&run)));
            }
        }

        // Build intersection types except the last one (the object), each member→member
        // gap emitted around the `&` it belongs to. The opening is this builder's own
        // text, so — like the shell gaps below — these gaps are invisible to every other
        // emitter and a comment in one would be silently DROPPED. The two runs are the
        // ordinary intersection printer's (`build_intersection_member_body_doc`), so the
        // side of the `&` a comment keeps is decided in one place for both paths.
        //
        // Block comments only, and by construction rather than by filter: a `//` here
        // declines this whole layout (`aligned_trailing_object_shell`), so what reaches
        // this builder is always inline-able.
        //
        // ⚠️ Each member takes the intersection's **member-parens** rule, exactly as the
        // ordinary path's members do. A bare `build_type_doc` here asked no paren question
        // at all, so a member that needs parens as an operand of `&` lost them — silently
        // RE-MEANING the type where `&` binds tighter (`(a & (b | c) & { x: X })` printing
        // as `(a & b | c & { x: X })`, i.e. `(a & b) | (c & { x: X })`), and emitting
        // output NEITHER parser accepts for a function or constructor member. Both survive
        // every reparse-shaped audit — the meaning change is valid, idempotent and
        // comment-clean — so only the prettier differential sees them.
        let member_parens = union_member_parens(intersection.types.len());
        let types_before_object = &intersection.types[..intersection.types.len() - 1];
        for (i, t) in types_before_object.iter().enumerate() {
            if i > 0 {
                self.push_intersection_operator_gap_comments(
                    &mut opening_parts,
                    types_before_object[i - 1].span().end,
                    t.span().start,
                );
            }
            opening_parts.push(self.build_intersection_member_type_doc(t, member_parens));
        }

        // Add ` & {`, with the last gap — the one before the trailing object — taking the
        // same two runs. `build_aligned_object_literal_doc` receives the `{` fused onto
        // this opening, so a comment after the `&` has to be emitted ahead of it here.
        // `aligned_trailing_object_shell` admits only a 2+-member intersection, so this
        // slice always has a last member and the `&` always has an operand on its left.
        if let Some(prev) = types_before_object.last() {
            self.push_intersection_operator_gap_comments(
                &mut opening_parts,
                prev.span().end,
                trailing_obj.span.start,
            );
        }
        opening_parts.push(d.text("{"));

        let mut parts: DocBuf = smallvec![
            self.build_aligned_object_literal_doc(trailing_obj, d.concat(&opening_parts))
        ];

        // Comments in the shell's TRAILING gap, between the object's `}` and the `)`
        // (`(a & { x: X } /* c */)`) — kept in place, as the union sibling
        // `build_parenthesized_union_doc` keeps its own. The `)` is this builder's, so
        // like the leading gap above this one is invisible to every other emitter and a
        // comment there would be silently DROPPED — in a union member and an optional
        // tuple element alike, and whether or not the object has members.
        //
        // Emitted OUTSIDE the object literal's group: a line comment has to break the
        // line it ends, and that break belongs to the enclosing union (which must expand
        // for the comment to keep its line), not to the object, which stays free to
        // print flat.
        // A `//` runs to end-of-line, so the `)` has to leave that line — inline it
        // would be swallowed. Matches the union sibling's expanded shell, which
        // likewise drops its `)` to its own line. The `align(2)` lands it under the
        // `(` — the column the object's OWN `})` closer takes when the object breaks
        // (`build_aligned_object_literal_doc`) — so the shell closes in one place
        // whether or not the object stayed flat. The run rides in the SAME `align(2)`,
        // because it shares that column: every line this shell drops below its `(` sits
        // under it, and a run emitted outside the align lands an align-step LEFT of the
        // `)` it precedes — the un-fused closer's lesson, re-asked for the comments that
        // now travel with it. The `hardline` is an unscoped force, deliberately: this
        // shell is RETAINED, so its doc regenerates identically on the reparse —
        // `flush_break` is only for stripped shells.
        let mut tail: DocBuf = DocBuf::new();
        let broke = paren.is_some_and(|p| {
            self.push_trailing_comments_in_range(&mut tail, trailing_obj.span.end, p.span.end - 1)
        });
        if broke {
            tail.push(d.hardline());
            tail.push(d.text(")"));
            parts.push(d.align(2, d.concat(&tail)));
        } else {
            // The comment-free path stays flat and node-for-node as it was.
            parts.extend(tail);
            parts.push(d.text(")"));
        }
        d.concat(&parts)
    }

    /// Build just the member content of a TypeLiteral, without `{` or `}`.
    ///
    /// Used by `build_aligned_object_literal_doc` for union members and
    /// parenthesized intersections where braces need separate handling.
    ///
    /// When `force_multiline` is true, uses hardlines. Otherwise uses softlines
    /// for width-aware formatting.
    fn build_type_literal_members_only_doc_for_alignment(
        &self,
        t: &TSTypeLiteral<'_>,
        force_multiline: bool,
        comments_present: bool,
        delimiter_pull_pos: Option<u32>,
    ) -> DocId {
        let d = self.d();
        if t.members.is_empty() {
            return d.empty();
        }

        let mut member_parts = d.pooled_docbuf();

        if force_multiline {
            // Forced multiline: the shared member-body walk, hardline-separated. This
            // path has no freeze arm — a directive in a member gap has never been
            // honored for a union-member / parenthesized-intersection object type.
            self.build_member_list_docs_into(
                &mut member_parts,
                t.members,
                self.type_literal_member_body(
                    t,
                    comments_present,
                    delimiter_pull_pos,
                    MemberFreeze::Never,
                ),
                TSTypeElement::span,
                |m| m.content_end(self.source),
                |m, deferred| self.build_type_member_doc_inner(m, deferred),
            );
        } else {
            // Width-aware: the opening bracketSpacing boundary leads (a space when flat
            // `{ a }`, a newline when broken), THEN the first member's leading block
            // comments, so the padding sits before the comment (`{ /* c */ a }`).
            // The force_multiline branch handles leading comments per-member through the
            // shared walk; the width-aware branch does not — without this a union-member
            // object's interior leading comment is dropped (`{ /* c */ a: 1 } | B`).
            // Mirrors the width-aware branch of `build_type_literal_doc_inner`.
            self.push_width_aware_type_literal_opener(&mut member_parts, t, comments_present);
            for (i, m) in t.members.iter().enumerate() {
                self.push_width_aware_type_member(&mut member_parts, t, i, m, comments_present);
            }
        }

        d.concat(&member_parts)
    }

    /// Emit the width-aware opening boundary for a type-literal member list:
    /// the `line()` bracketSpacing boundary (a space when flat `{ a }`, a
    /// newline when broken), then the first member's interior leading block
    /// comments, so the padding sits before the comment (`{ /* c */ a }`).
    /// Shared by the width-aware branch of
    /// `build_type_literal_members_only_doc_for_alignment` and
    /// `build_type_literal_doc_inner`.
    fn push_width_aware_type_literal_opener(
        &self,
        member_parts: &mut DocBuf,
        t: &TSTypeLiteral<'_>,
        comments_present: bool,
    ) {
        let d = self.d();
        member_parts.push(d.line());
        if comments_present && let Some(first) = t.members.first() {
            member_parts.extend(
                self.build_type_literal_leading_comments_inline(t.span.start, first.span().start),
            );
        }
    }

    /// Emit one member in the width-aware (non-force-multiline) type-literal
    /// layout: a softline before non-first members, the member doc, then the
    /// member→`;` gap comments split around the conditional semicolon (present
    /// only when broken). Shared by the width-aware branch of
    /// `build_type_literal_members_only_doc_for_alignment` and
    /// `build_type_literal_doc_inner`. (The force_multiline branches differ and
    /// stay separate.)
    ///
    /// ⚠️ **The `deferred` run this threads is empty on every input that reaches here**,
    /// so its ordering against the trailing run — the thing that matters in the
    /// force-multiline twins — is inert. A member→`;` gap defers only an OWN-LINE comment,
    /// and both of its spellings (a `//`, a block on its own line) are triggers of
    /// [`Self::type_literal_force_multiline`], which routes the literal to those twins
    /// instead. Threaded rather than dropped because the arm cannot *prove* it locally (the
    /// alignment caller supplies `force_multiline` from outside), and dropping a run on an
    /// unproven emptiness argument is how a comment goes missing. Measured: an
    /// `assert!(deferred.is_empty())` here survives the fixture suite and 3.5M comment
    /// injections across 7898 files (`deno task gaps:audit`).
    fn push_width_aware_type_member(
        &self,
        member_parts: &mut DocBuf,
        t: &TSTypeLiteral<'_>,
        i: usize,
        m: &TSTypeElement<'_>,
        comments_present: bool,
    ) {
        let d = self.d();
        let is_first = i == 0;
        let is_last = i == t.members.len() - 1;
        // Use content_end for comment detection (before trailing separator)
        let member_content_end = m.content_end(self.source);
        // First member follows the opening boundary directly; subsequent
        // members keep a softline — the inter-member flat space is emitted
        // by the non-last `if_break(empty, " ")` below.
        if !is_first {
            member_parts.push(d.softline());
            // Comments the previous member's claim handed over lead this member
            // ([`Self::type_member_claim_end`] over the previous member's gap — the
            // same call its trailing collect makes, so the two sides cannot
            // disagree): on the member's own line when broken (`a: A;⏎/* c */ b: B;`),
            // and between the members when flat — where the emission is
            // byte-identical to the trailing form, so only the broken layout moves.
            if comments_present {
                let prev_content_end = t.members[i - 1].content_end(self.source);
                let claim_end = self.type_member_claim_end(prev_content_end, Some(m.span().start));
                for comment in comments_to_emit_in_range(self.comments, claim_end, m.span().start) {
                    member_parts.push(self.build_comment_doc(comment));
                    member_parts.push(d.text(" "));
                }
            }
        }
        let mut deferred = DocBuf::new();
        member_parts.push(self.build_type_member_doc_inner(m, &mut deferred));

        // Handle trailing comments - preserve position relative to semicolon.
        // The claim stops at the split: a comment hugging the next member's start
        // leads it, emitted ahead of that member in its own segment above — without
        // the bound both segments would print it.
        let upper_bound = t
            .members
            .get(i + 1)
            .map_or(t.span.end, |next| next.span().start);
        let trailing: CommentVec<'_> = if comments_present {
            let claim_end = self.type_member_claim_end(
                member_content_end,
                t.members.get(i + 1).map(|n| n.span().start),
            );
            comments_to_emit_in_range(self.comments, member_content_end, upper_bound)
                .filter(|c| c.span.start < claim_end)
                .collect()
        } else {
            CommentVec::new()
        };

        if is_last {
            // Last member: semicolon only when broken, comments after
            member_parts.push(d.if_break(d.text(";"), d.empty()));
            member_parts.extend(deferred);
            for comment in &trailing {
                member_parts.push(self.build_trailing_comment_doc(comment));
            }
        } else {
            // Non-last: preserve comment position relative to semicolon. The member's own
            // deferred run goes between the `;` and the trailing comments, the order the
            // `is_last` arm above already uses and the one the interface and class get from
            // [`Printer::build_member_with_semicolon_doc`].
            member_parts.extend(self.build_comments_around_semicolon_doc(
                &trailing,
                member_content_end,
                upper_bound,
                deferred,
            ));
            // Space before next member only when flat
            member_parts.push(d.if_break(d.empty(), d.text(" ")));
        }
    }

    /// Check if a TypeLiteral should be forced to multiline format.
    ///
    /// Returns true if:
    /// - Source has newline immediately after opening brace
    /// - Contains line comments or multi-line block comments
    /// - Contains block comments on their own line
    pub(super) fn type_literal_force_multiline(
        &self,
        obj: &TSTypeLiteral<'_>,
        comments_present: bool,
    ) -> bool {
        // Both reads below are newline-derived authoring intent (a source newline
        // after `{` / before the first member). The canonical reprint erases them
        // so an object type breaks only by width.
        let source_is_multiline =
            !self.canonical && super::super::is_brace_block_multiline(self.source, obj.span);
        // Prettier breaks an object type when its first member starts on a line
        // below the opening brace. `is_brace_block_multiline` only sees a newline
        // *immediately* after `{`, so a block comment on the brace line
        // (`{ /* c */\n a: T }`) defeats it — detect the newline before the first
        // member directly here.
        let first_member_on_new_line = !self.canonical
            && obj.members.first().is_some_and(|m| {
                self.source[obj.span.start as usize..m.span().start as usize].contains('\n')
            });
        let has_line_or_multiline_block = comments_present
            && self
                .comments_on_page_between(obj.span.start, obj.span.end)
                .any(|c| !c.is_block || c.multiline);
        source_is_multiline
            || first_member_on_new_line
            || has_line_or_multiline_block
            // Lazy: the per-member span collection only runs when the cheaper checks
            // above didn't already force multiline (and only when the construct has
            // comments at all). The glue rule is `ItemStart`, not the object literal's
            // source reading — prettier expands `{ a: A⏎/* c */,⏎b: B }` here while
            // collapsing the object literal written the same way (`StandaloneGlue`).
            || (comments_present && {
                let member_spans: SmallVec<[_; 8]> =
                    obj.members.iter().map(TSTypeElement::span).collect();
                self.has_standalone_block_comment(
                    obj.span.start,
                    obj.span.end,
                    &member_spans,
                    StandaloneGlue::ItemStart,
                )
            })
    }

    /// Build aligned object literal doc with a custom opening.
    ///
    /// Used for object literals in union types and parenthesized intersections.
    /// Members are double-indented in whole tabs (aligning with the content after
    /// `{`), and the closing `}` takes the member's `align(2)` sub-tab offset —
    /// 2 literal spaces under `{`, tab-width independent — matching Prettier's
    /// `align(2, …)`.
    ///
    /// The doc ends at the object's own `}`. A caller whose shell continues past it
    /// (the parenthesized intersection's `)`) appends that itself rather than fusing
    /// it into the closing text, so the gap between the two stays an emittable seam.
    fn build_aligned_object_literal_doc(&self, obj: &TSTypeLiteral<'_>, opening: DocId) -> DocId {
        let d = self.d();
        // Empty object type: keep the braces delimiter-tight (`{}` not `{ }`) and
        // preserve any interior comment. The members-only alignment path returns an
        // empty doc for zero members, so the `d.line()` boundary below would render
        // as a spurious space and the comment would be dropped — mirror the plain
        // type-literal empty path (`build_empty_braces_inline_with_comments_doc`),
        // threading this path's (possibly prefixed) `opening`.
        if obj.members.is_empty() {
            return self.build_empty_bracketed_with_comments_doc(
                obj.span.start,
                obj.span.end,
                opening,
                "}",
                d.line(),
            );
        }
        // Zero-comment whole-construct gate: one existence check over the literal's
        // span; every comment sub-query below is bounded within it.
        let comments_present = self.has_comments_to_emit_between(obj.span.start, obj.span.end);
        let force_multiline = self.type_literal_force_multiline(obj, comments_present);
        // A comment trailing the opening `{` on its line stays on it, through the SAME
        // decision the plain type literal makes ([`Self::type_literal_brace_line_pull`]).
        // This rendering was the open-delimiter family's last un-converted relocation: it
        // passed `None` for the pull position, so the comment fell through to the first
        // member's leading run and moved into the body.
        let (brace_line_prefix, delimiter_pull_pos) = if force_multiline {
            self.type_literal_brace_line_pull(obj, comments_present)
        } else {
            (DocBuf::new(), None)
        };
        let members_doc = self.build_type_literal_members_only_doc_for_alignment(
            obj,
            force_multiline,
            comments_present,
            delimiter_pull_pos,
        );

        // Closing inner boundary: a hardline when forced multiline, else the
        // bracketSpacing boundary (a space when flat `{ a }`, a newline when the
        // group breaks).
        let line_doc = if force_multiline {
            d.hardline()
        } else {
            d.line()
        };

        d.group(d.concat(&[
            opening,
            // The pulled comment rides on the `{`'s own line, ahead of the indent — no
            // break precedes it, so the indent has nothing to act on.
            d.concat(&brace_line_prefix),
            d.indent(d.indent(members_doc)),
            // The closing delimiter takes the union member's `align(2)` sub-tab
            // offset (2 literal spaces), so it lands under its opener at any tab
            // width — matching Prettier's `align(2, …)`. The members keep whole
            // tabs (`align(2)` + `indent` rounds up), so only the closing line's
            // representation changes.
            d.align(2, d.concat(&[line_doc, d.text("}")])),
        ]))
    }

    /// Build doc for object type literal when it's a direct union member.
    ///
    /// Aligns object content with the position after `| {`:
    /// ```text
    /// type T =
    ///   | {
    ///       prop: A;  // double indent, whole tabs (aligns with content after "{ ")
    ///     }           // align(2): base + 2 spaces (aligns under "{", any tab width)
    ///   | B;
    /// ```
    pub(super) fn build_union_member_object_literal_doc(&self, obj: &TSTypeLiteral<'_>) -> DocId {
        self.build_aligned_object_literal_doc(obj, self.d().text("{"))
    }

    //
    // Type Literal Docs
    //

    /// Build a Doc for a type literal (object type): `{ a: T; b: U }`
    ///
    /// Handles both single-line and multi-line formats:
    /// - Single-line source stays single-line if it fits: `{ a: T; b: U }`
    /// - Multi-line source (newline after `{`) stays multi-line
    /// - Comments force multi-line formatting
    pub(super) fn build_type_literal_doc(&self, t: &TSTypeLiteral<'_>) -> DocId {
        self.build_type_literal_doc_inner(t, TypeLiteralMode::Standard)
    }

    /// Inner implementation for type literal doc building.
    ///
    /// `mode` controls formatting behavior:
    /// - `Standard`: Width-aware with softlines, wrapped in group
    /// - `NoGroup`: Width-aware with softlines, no group (parent controls breaking)
    fn build_type_literal_doc_inner(&self, t: &TSTypeLiteral<'_>, mode: TypeLiteralMode) -> DocId {
        let d = self.d();
        let wrap_in_group = matches!(mode, TypeLiteralMode::Standard);
        // Zero-comment whole-construct gate: one existence check over the literal's
        // span skips every per-member comment query below on the comment-free
        // common case — each sub-range lies within [span.start, span.end].
        let comments_present = self.has_comments_to_emit_between(t.span.start, t.span.end);
        let force_multiline = self.type_literal_force_multiline(t, comments_present);

        if t.members.is_empty() {
            // Empty type literal - handle comments inside. The helper already
            // returns a self-managing group (a fitting block comment stays
            // inline as `{ /* c */ }`), so it's correct in both modes.
            return self.build_empty_braces_inline_with_comments_doc(t.span);
        }

        let mut parts: DocBuf = smallvec![d.text("{")];
        if force_multiline {
            // A comment trailing the opening `{` on its own line is kept on the
            // `{` line (divergence from prettier, which relocates it to its own
            // line as the first member's leading comment). A line/own-line
            // comment is itself what forces this multiline branch. See
            // conformance_prettier_ts_comments.md §Comment relocation (Type literal `{`).
            let (brace_line_prefix, delimiter_pull_pos) =
                self.type_literal_brace_line_pull(t, comments_present);
            parts.push(d.concat(&brace_line_prefix));

            // Multi-line format (same for both modes) — the shared member-body walk. A
            // preceding format-ignore directive keeps the member's source verbatim, up to
            // the member's CONTENT end (no trailing `;`), because the walk's own seam
            // re-adds the separator ([`MemberFreeze::Content`]).
            let mut member_parts = d.pooled_docbuf();
            self.build_member_list_docs_into(
                &mut member_parts,
                t.members,
                self.type_literal_member_body(
                    t,
                    comments_present,
                    delimiter_pull_pos,
                    MemberFreeze::Content,
                ),
                TSTypeElement::span,
                |m| m.content_end(self.source),
                |m, deferred| self.build_type_member_doc_inner(m, deferred),
            );

            parts.push(d.indent(d.concat(&member_parts)));
            parts.push(d.hardline());
        } else {
            // Width-aware format: stays inline if fits, wraps if too long.
            // The opening bracketSpacing boundary leads (a space when flat `{ a }`,
            // a newline when broken), THEN any interior leading comments, so the
            // padding sits before the comment (`{ /* c */ a }`, not `{/* c */ a }`).
            let mut member_parts = d.pooled_docbuf();
            self.push_width_aware_type_literal_opener(&mut member_parts, t, comments_present);
            for (i, m) in t.members.iter().enumerate() {
                self.push_width_aware_type_member(&mut member_parts, t, i, m, comments_present);
            }
            parts.push(d.indent(d.concat(&member_parts)));
            parts.push(d.line());
        }
        parts.push(d.text("}"));

        if wrap_in_group {
            d.group(d.concat(&parts))
        } else {
            d.concat(&parts)
        }
    }

    /// Build a Doc for a type literal in function param context (no group wrapper).
    ///
    /// Uses width-aware format with softlines (can break), but WITHOUT wrapping
    /// in its own group. This lets the parent function type group control breaking.
    ///
    /// When the function type group breaks (because line is too long), these
    /// softlines become newlines, expanding the param's object type.
    pub(super) fn build_type_literal_doc_for_function_param(&self, t: &TSTypeLiteral<'_>) -> DocId {
        self.build_type_literal_doc_inner(t, TypeLiteralMode::NoGroup)
    }

    /// Build a Doc for a type expression suitable for use as a type argument.
    ///
    /// An object type literal carries its own width-aware group, so when it
    /// overflows it breaks block-style (members on their own lines) rather than
    /// spilling an inner union/intersection — even inside a multi-argument list
    /// (`Map<K, { ...wide... }>`). Matches Prettier and the single-argument path.
    pub(in crate::printer) fn build_type_doc_for_type_arg(&self, ts_type: &TSType<'_>) -> DocId {
        let d = self.d();
        match ts_type {
            TSType::TypeLiteral(t) => self.build_type_literal_doc(t),
            TSType::Parenthesized(p) => {
                // Unwrap the parens (redundant in type-argument position — prettier
                // strips them too) but preserve any comments the user wrote inside
                // them (`Foo<(a | b /* c */)>` → `Foo<a | b /* c */>`). Without this
                // the comment was dropped — a content-loss bug. A line comment defers
                // to end-of-line and forces the type-argument list to break (via
                // `break_parent`), matching prettier's expansion.
                let inner = self.build_type_doc_for_type_arg(p.type_annotation);
                let inner_start = p.type_annotation.span().start;
                let inner_end = p.type_annotation.span().end;
                let (has_leading, has_trailing) = self.paren_inner_comment_flags(p);
                if !has_leading && !has_trailing {
                    return inner;
                }
                let leading: CommentVec<'_> = if has_leading {
                    comments_to_emit_in_range(self.comments, p.span.start, inner_start).collect()
                } else {
                    smallvec![]
                };
                // A line comment forces the type-argument list to break. Emit
                // `break_parent` FIRST so it sits behind the inner type's group in the
                // forward `fits()` scan — otherwise it poisons that scan and needlessly
                // expands an inner union (`Foo<(a | b // c)>` keeps `a | b` inline,
                // matching prettier, rather than `| a | b`).
                // Unscoped, deliberately — even though this shell is STRIPPED: the
                // trailing half's deferred run flushes inside the `<…>` list, a RETAINED
                // bracketed construct, so the reparse finds the comment in the list's own
                // trailing gap and re-forces the all-hardline expansion
                // (`type_arguments_force_expansion`), whose hardlines re-break every
                // enclosing group exactly as this unscoped force did — reproducible
                // geometry, matching prettier at every nesting (enclosing composites,
                // assignments, multi-argument lists, nested `<…>`). Contrast
                // `build_parenthesized_type_unwrap_doc`'s trailing arm, whose stripped
                // shell leaves the comment in a member gap that cannot re-force
                // intermediate groups — that strip needs `flush_break`. Pinned by
                // `type_argument_paren_union_line_comment`.
                // TODO: the LEADING half's landing is wrong for a single NON-HUGGING
                // argument: the expansion its hardline forces is prettier's fixed point,
                // not tsv's — the reparse routes the now-bare leading comment to the
                // single-argument hug (`build_angle_list_with_line_comments`), a 2-pass
                // convergence. Pinned (failing) by
                // `type_argument_paren_union_leading_line_comment`.
                // The trailing half is asked as a predicate rather than collected: the
                // emitter below re-walks that range anyway, so materializing it here
                // bought a second scan and a `SmallVec` to answer one bool.
                let needs_break = leading.iter().any(|comment| !comment.is_block)
                    || (has_trailing && self.has_line_comments_between(inner_end, p.span.end));
                let mut parts = DocBuf::new();
                if needs_break {
                    parts.push(d.break_parent());
                }
                for comment in &leading {
                    parts.push(self.build_comment_doc(comment));
                    self.push_comment_kind_separator(&mut parts, comment);
                }
                parts.push(inner);
                // The shared trailing-gap emitter, never an open-coded loop: it takes the
                // separator BEFORE each comment and asks the SOURCE for it, so a run the
                // author spread over lines stays a run. Emitting back to back welded it
                // into one comment (`// c // injected` — the second `//` becoming text of
                // the first, so that comment stops existing), and a block after a line
                // comment additionally reordered ahead of it. The weld reparses and is
                // stable, so the ledger, F1, the round-trip and the fuzzer are all blind
                // to it — `gaps:audit` is what sees it.
                self.push_trailing_comments_in_range(&mut parts, inner_end, p.span.end);
                d.concat(&parts)
            }
            _ => self.build_type_doc(ts_type),
        }
    }
}
