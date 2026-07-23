// Type literal printing for TypeScript
//
// Handles printing of object type literals (`{ a: T; b: U }`) with:
// - Single-line and multi-line formats
// - Object alignment for union members and parenthesized intersections
// - Grouped (`Standard`) vs no-group (`NoGroup`) modes so a parent (function
//   type / type-argument list) can control breaking

use super::super::CommentVec;
use super::super::comments_to_emit_in_range;
use super::Printer;
use super::helpers::{immediate_paren, unwrap_parenthesized};
use crate::ast::internal::{
    TSIntersectionType, TSParenthesizedType, TSType, TSTypeElement, TSTypeLiteral, TSUnionType,
};
use smallvec::{SmallVec, smallvec};
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// Mode for building type literal docs.
enum TypeLiteralMode {
    /// Width-aware with softlines, wrapped in group
    Standard,
    /// Width-aware with softlines, no group (parent controls breaking)
    NoGroup,
}

impl<'a> Printer<'a> {
    //
    // Comment partitioning helpers
    //

    /// Build docs for leading comments with blank line preservation in multiline format.
    ///
    /// Returns docs for: `[literalline if blank && !is_first] hardline [leading comments]`
    ///
    /// For non-first members, filters out same-line comments (they belong to the previous member).
    fn build_multiline_member_prefix_doc(
        &self,
        prev_end: u32,
        member_start: u32,
        is_first: bool,
        delimiter_pull_pos: Option<u32>,
        comments_present: bool,
    ) -> DocBuf {
        let d = self.d();
        // `comments_present` is the caller's whole-construct existence gate:
        // when false, `[prev_end, member_start]` is provably empty,
        // so skip the collect/filter machinery.
        let all_comments: CommentVec<'_> = if comments_present {
            comments_to_emit_in_range(self.comments, prev_end, member_start).collect()
        } else {
            CommentVec::new()
        };
        let leading_comments: CommentVec<'_> = if !is_first {
            all_comments
                .iter()
                .filter(|c| !self.is_same_line(prev_end, c.span.start))
                .copied()
                .collect()
        } else {
            // First member: drop comments pulled onto the `{` line (emitted as
            // the brace-line prefix by the caller). No-op when `delimiter_pull_pos`
            // is `None` (the alignment caller).
            self.first_member_leading_comments(all_comments, delimiter_pull_pos)
        };

        // Step the scan past the previous member's trailing comment(s) so a multi-line
        // block's interior newlines aren't read as an authored blank line
        // (`a: 1; /*⏎…⏎*/⏎b` has no blank line between the members).
        let check_pos = if !leading_comments.is_empty() {
            leading_comments[0].span.start
        } else {
            member_start
        };
        let has_blank =
            self.has_blank_line_between(self.blank_scan_start(prev_end, check_pos), check_pos);

        let mut docs = DocBuf::with_capacity(3);
        if has_blank && !is_first {
            docs.push(d.literalline());
        }
        docs.push(d.hardline());
        self.push_leading_comments_before(&mut docs, &leading_comments, member_start);
        docs
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
    fn build_comments_around_semicolon_doc(
        &self,
        comments: &[&tsv_lang::Comment],
        member_end: u32,
        upper_bound: u32,
    ) -> DocBuf {
        let d = self.d();
        // Comment-free gap (the common case): no separator scan needed — the
        // partition below reduces to the bare `;`.
        if comments.is_empty() {
            let mut docs = DocBuf::with_capacity(1);
            docs.push(d.text(";"));
            return docs;
        }
        // Find the source member separator — `;` OR `,` (both are valid type-member
        // separators; tsv normalizes either to `;`). Comment-aware so a separator
        // glyph inside a comment in this gap isn't mistaken for the real one. Taking
        // the earlier of the two handles whichever the author used; with neither
        // (newline/ASI-separated members) there is no anchor and all comments are
        // "before". Keying only on `;` here put a comment that followed a `,`
        // separator on the wrong side (`a: 1 /* c */;` instead of `a: 1; /* c */`).
        let semi = self.find_char_outside_comments(member_end, upper_bound, b';');
        let comma = self.find_char_outside_comments(member_end, upper_bound, b',');
        let sep_pos = [semi, comma].into_iter().flatten().min();

        let (before_semi, after_semi): (Vec<_>, Vec<_>) =
            comments.iter().partition(|c| match sep_pos {
                Some(pos) => c.span.start < pos,
                // No source separator (newline/ASI-separated members): the `;` is
                // synthesized right after the member, so every comment in the gap
                // leads the next member and goes after the `;`.
                None => false,
            });

        let mut docs = DocBuf::with_capacity(before_semi.len() + after_semi.len() + 1);
        for comment in before_semi {
            docs.push(self.build_trailing_comment_doc(comment));
        }
        docs.push(d.text(";"));
        for comment in after_semi {
            docs.push(self.build_trailing_comment_doc(comment));
        }
        docs
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
        self.build_type_doc_maybe_parens_impl(ts_type, needs_parens, true)
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
        self.build_type_doc_maybe_parens_impl(ts_type, needs_parens, false)
    }

    /// Shared implementation of `build_type_doc_maybe_parens` /
    /// `build_intersection_member_type_doc`. `indent_default_paren` gates two paren
    /// cases: (1) the default paren — `true` wraps the inner type in `d.indent`
    /// (union-member offset / conditional check-extends depth), `false` leaves it
    /// bare (intersection members); and (2) the trailing-object-intersection special
    /// case — `true` takes it (the union-member `| {` double-indent alignment),
    /// `false` skips it so the intersection member falls to the bare default arm
    /// (single indent). Both keep an intersection member one level shallower than the
    /// union-member/conditional layout.
    fn build_type_doc_maybe_parens_impl(
        &self,
        ts_type: &TSType<'_>,
        needs_parens: fn(&TSType<'_>) -> bool,
        indent_default_paren: bool,
    ) -> DocId {
        let d = self.d();
        if needs_parens(ts_type) {
            // Special case: intersection with trailing object type — build a custom
            // doc that aligns the trailing object's body + closing `})` with the
            // union member's `| {` offset (`build_aligned_object_literal_doc`'s double
            // indent). That alignment is correct only in a union-member context
            // (`indent_default_paren == true`, via `build_type_doc_maybe_parens` /
            // `build_union_member_offset_doc`); for an *intersection* member
            // (`build_intersection_member_type_doc`, `indent_default_paren == false`)
            // the trailing object hangs one level too deep. There the default arm
            // below (bare `("(", inner, ")")`, no inner indent) reproduces the array
            // path's single-indent layout, matching prettier. See
            // `intersection_paren_member_trailing_object_long`.
            // Note: unwrap_parenthesized to handle cases like `(A & {...})` where
            // the input is TSParenthesizedType wrapping TSIntersectionType
            if indent_default_paren
                && let TSType::Intersection(intersection) = unwrap_parenthesized(ts_type)
                && let Some(last) = intersection.types.last()
                && let TSType::TypeLiteral(obj) = unwrap_parenthesized(last)
            {
                return self.build_parenthesized_intersection_trailing_object_doc(
                    intersection,
                    obj,
                    immediate_paren(ts_type),
                );
            }

            // Special case: parenthesized union type
            if let TSType::Union(union) = unwrap_parenthesized(ts_type) {
                return self.build_parenthesized_union_doc(union, immediate_paren(ts_type), false);
            }

            // Default case: parenthesize the inner type. The inner `d.indent`
            // (union-member offset / conditional check-extends depth) is dropped
            // for intersection members, which take their level from the
            // intersection printer's own `& `-line indent — see
            // `build_intersection_member_type_doc`.
            let inner = self.build_type_doc(ts_type);
            d.concat(&[
                d.text("("),
                if indent_default_paren {
                    d.indent(inner)
                } else {
                    inner
                },
                d.text(")"),
            ])
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

    /// A comment-free parenthesized union in element/object position that would
    /// break EXPANDS its parens (`(⏎\t| A⏎\t| B⏎)`, prettier's `printUnionType`
    /// needs-parens branch) instead of gluing the leading `|` to the `(`. Returns
    /// `None` for any other type — the caller keeps its existing layout — and for a
    /// *commented* union, so comment placement stays untouched. `ty` is the element /
    /// object as authored (parens included): it is unwrapped to reach the union, and
    /// its own span (parens and all) is the comment window. Shared by the
    /// array-element (`build_array_type_doc`) and indexed-access-object arms; the
    /// indexed-access *index* uses bracket delimiters, not parens, so it expands
    /// inline rather than through here. See `type_param_fits_rhs_long`.
    pub(super) fn build_expanded_parenthesized_union_opt(&self, ty: &TSType<'_>) -> Option<DocId> {
        if let TSType::Union(u) = unwrap_parenthesized(ty)
            && !self.union_prints_hugged(u)
            && !self.has_comments_to_emit_between(ty.span().start, ty.span().end)
        {
            Some(self.build_parenthesized_union_doc(u, None, false))
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
    /// after `(` is only emitted here when `emit_inner_leading_line_comments` is set —
    /// the paren-union member arms of `build_union_type_doc_with_line_comments`, which
    /// keep such a comment inside the parens for EVERY member (first or later); other
    /// callers pass `false` because a leading line comment has already been handled
    /// upstream (relocated or emitted before the member).
    pub(super) fn build_parenthesized_union_doc(
        &self,
        union: &TSUnionType<'_>,
        paren: Option<&TSParenthesizedType<'_>>,
        emit_inner_leading_line_comments: bool,
    ) -> DocId {
        let d = self.d();
        let union_doc = self.build_union_type_doc(union);

        let mut needs_break = false;
        let mut indented: DocBuf = smallvec![d.softline()];
        if let Some(p) = paren {
            // Leading comments between `(` and the union. Block comments stay inline
            // (`(/* c */ a | b)`). A leading *line* comment reaches here only when
            // `emit_inner_leading_line_comments` is set — the paren-union member of an
            // outer union, whose comment tsv keeps inside the parens leading the inner
            // union (for every member, not just the first). A line comment must end its
            // line, so it forces the paren group to break.
            for comment in
                comments_to_emit_in_range(self.comments, p.span.start + 1, union.span.start)
            {
                if comment.is_block {
                    indented.push(self.build_comment_doc(comment));
                    indented.push(d.text(" "));
                } else if emit_inner_leading_line_comments {
                    indented.push(self.build_comment_doc(comment));
                    indented.push(d.hardline());
                    needs_break = true;
                }
            }
        }
        indented.push(union_doc);
        if let Some(p) = paren {
            // Trailing comments between the union and `)`: a block comment stays
            // inline (`(a | b /* c */)`); a line comment defers to end-of-line and
            // forces the paren group to break. The inner union has its own group,
            // but the line comment's `break_parent` (below) propagates, expanding it
            // to one member per line.
            for comment in comments_to_emit_in_range(self.comments, union.span.end, p.span.end - 1)
            {
                if comment.is_block {
                    indented.push(d.text(" "));
                    indented.push(self.build_comment_doc(comment));
                } else {
                    let suffix = d.concat(&[d.text(" "), self.build_comment_doc(comment)]);
                    indented.push(d.line_suffix(suffix));
                    needs_break = true;
                }
            }
        }

        let mut inner_parts: DocBuf = smallvec![d.indent(d.concat(&indented)), d.softline()];
        if needs_break {
            inner_parts.push(d.break_parent());
        }
        let inner = d.group(d.concat(&inner_parts));
        d.parens(inner)
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
            let mut lead: DocBuf = DocBuf::new();
            for comment in
                comments_to_emit_in_range(self.comments, p.span.start + 1, intersection.span.start)
            {
                lead.push(self.build_comment_doc(comment));
                if comment.is_block {
                    lead.push(d.text(" "));
                } else {
                    // A `//` runs to end-of-line — without the break it would swallow the
                    // intersection that follows it on the line.
                    lead.push(d.hardline());
                }
            }
            if !lead.is_empty() {
                // `indent`, because that break places the intersection's own first line:
                // it belongs one level in from the `(`, matching the default-paren path
                // (`d.indent(self.build_type_doc(…))` above) that every other paren-retained
                // member shape takes. A block comment carries no break, so the indent is
                // inert for it.
                opening_parts.push(d.indent(d.concat(&lead)));
            }
        }

        // Build intersection types except the last one (the object)
        let types_before_object = &intersection.types[..intersection.types.len() - 1];
        for (i, t) in types_before_object.iter().enumerate() {
            if i > 0 {
                opening_parts.push(d.text(" & "));
            }
            opening_parts.push(self.build_type_doc(t));
        }

        // Add ` & {`
        opening_parts.push(d.text(" & {"));

        self.build_aligned_object_literal_doc(trailing_obj, d.concat(&opening_parts), "})")
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
    ) -> DocId {
        let d = self.d();
        if t.members.is_empty() {
            return d.empty();
        }

        let mut member_parts = d.pooled_docbuf();
        let mut prev_end = t.span.start + 1; // after opening brace

        // Width-aware: the opening bracketSpacing boundary leads (a space when flat
        // `{ a }`, a newline when broken), THEN the first member's leading block
        // comments, so the padding sits before the comment (`{ /* c */ a }`).
        // The force_multiline branch handles leading comments per-member via
        // `build_multiline_member_prefix_doc`; the width-aware branch does not —
        // without this a union-member object's interior leading comment is dropped
        // (`{ /* c */ a: 1 } | B`). Mirrors the width-aware branch of
        // `build_type_literal_doc_inner`.
        if !force_multiline {
            self.push_width_aware_type_literal_opener(&mut member_parts, t, comments_present);
        }

        for (i, m) in t.members.iter().enumerate() {
            let is_first = i == 0;
            // Use content_end for comment detection (before trailing separator)
            let member_content_end = m.content_end(self.source);

            if force_multiline {
                // Forced multiline: build with hardlines. `None` keeps the
                // delimiter-line comment relocating in this alignment path
                // (union-member / intersection-trailing object literals).
                member_parts.extend(self.build_multiline_member_prefix_doc(
                    prev_end,
                    m.span().start,
                    is_first,
                    None,
                    comments_present,
                ));
                let mut deferred = DocBuf::new();
                member_parts.push(self.build_type_member_doc_inner(m, &mut deferred));

                // Handle trailing comments - preserve position relative to semicolon
                let upper_bound = t
                    .members
                    .get(i + 1)
                    .map_or(t.span.end, |next| next.span().start);
                let trailing: CommentVec<'_> = if comments_present {
                    comments_to_emit_in_range(self.comments, member_content_end, upper_bound)
                        .filter(|c| self.is_same_line(member_content_end, c.span.start))
                        .collect()
                } else {
                    CommentVec::new()
                };
                member_parts.extend(self.build_comments_around_semicolon_doc(
                    &trailing,
                    member_content_end,
                    upper_bound,
                ));
                // Own-line comments from the member→`;` gap, deferred past the `;`.
                member_parts.extend(deferred);
            } else {
                self.push_width_aware_type_member(&mut member_parts, t, i, m, comments_present);
            }

            prev_end = m.span().end;
        }

        if force_multiline && comments_present {
            // Trailing comments after last member
            let body_end = t.span.end.saturating_sub(1);
            member_parts.extend(self.build_trailing_body_comments_doc(prev_end, body_end));
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
        }
        let mut deferred = DocBuf::new();
        member_parts.push(self.build_type_member_doc_inner(m, &mut deferred));

        // Handle trailing comments - preserve position relative to semicolon
        let upper_bound = t
            .members
            .get(i + 1)
            .map_or(t.span.end, |next| next.span().start);
        let trailing: CommentVec<'_> = if comments_present {
            comments_to_emit_in_range(self.comments, member_content_end, upper_bound).collect()
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
            // Non-last: preserve comment position relative to semicolon
            member_parts.extend(self.build_comments_around_semicolon_doc(
                &trailing,
                member_content_end,
                upper_bound,
            ));
            // Own-line comments from the member→`;` gap, deferred past the `;`.
            member_parts.extend(deferred);
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
            // comments at all).
            || (comments_present && {
                let member_spans: SmallVec<[_; 8]> =
                    obj.members.iter().map(TSTypeElement::span).collect();
                self.has_standalone_block_comment(obj.span.start, obj.span.end, &member_spans)
            })
    }

    /// Build aligned object literal doc with custom opening/closing.
    ///
    /// Used for object literals in union types and parenthesized intersections.
    /// Members are double-indented in whole tabs (aligning with the content after
    /// `{`), and the closing delimiter takes the member's `align(2)` sub-tab offset —
    /// 2 literal spaces under `{`, tab-width independent — matching Prettier's
    /// `align(2, …)`.
    fn build_aligned_object_literal_doc(
        &self,
        obj: &TSTypeLiteral<'_>,
        opening: DocId,
        closing: &'static str,
    ) -> DocId {
        let d = self.d();
        // Empty object type: keep the braces delimiter-tight (`{}` not `{ }`) and
        // preserve any interior comment. The members-only alignment path returns an
        // empty doc for zero members, so the `d.line()` boundary below would render
        // as a spurious space and the comment would be dropped — mirror the plain
        // type-literal empty path (`build_empty_braces_inline_with_comments_doc`),
        // threading this path's (possibly prefixed) `opening`/`closing`.
        if obj.members.is_empty() {
            return self.build_empty_bracketed_with_comments_doc(
                obj.span.start,
                obj.span.end,
                opening,
                closing,
                d.line(),
            );
        }
        // Zero-comment whole-construct gate: one existence check over the literal's
        // span; every comment sub-query below is bounded within it.
        let comments_present = self.has_comments_to_emit_between(obj.span.start, obj.span.end);
        let force_multiline = self.type_literal_force_multiline(obj, comments_present);
        let members_doc = self.build_type_literal_members_only_doc_for_alignment(
            obj,
            force_multiline,
            comments_present,
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
            d.indent(d.indent(members_doc)),
            // The closing delimiter takes the union member's `align(2)` sub-tab
            // offset (2 literal spaces), so it lands under its opener at any tab
            // width — matching Prettier's `align(2, …)`. The members keep whole
            // tabs (`align(2)` + `indent` rounds up), so only the closing line's
            // representation changes.
            d.align(2, d.concat(&[line_doc, d.text(closing)])),
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
        self.build_aligned_object_literal_doc(obj, self.d().text("{"), "}")
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
            // conformance_prettier.md §Comment relocation (Type literal `{`).
            let first_member_start = t.members[0].span().start;
            let (brace_line_prefix, delimiter_pull_pos) = if comments_present {
                self.delimiter_line_comment_prefix(t.span.start, first_member_start)
            } else {
                (DocBuf::new(), None)
            };
            parts.push(d.concat(&brace_line_prefix));

            // Multi-line format (same for both modes)
            let mut member_parts = d.pooled_docbuf();
            let mut prev_end = t.span.start + 1; // after opening brace
            for (i, m) in t.members.iter().enumerate() {
                let is_first = i == 0;
                // Use content_end for comment detection (before trailing separator)
                let member_content_end = m.content_end(self.source);

                member_parts.extend(self.build_multiline_member_prefix_doc(
                    prev_end,
                    m.span().start,
                    is_first,
                    delimiter_pull_pos,
                    comments_present,
                ));
                // A preceding format-ignore directive keeps the member's source
                // verbatim. Use the content span (no trailing
                // `;`); the loop's semicolon handling below re-adds the `;`.
                // (A directive is itself a comment, so the gate is exact.)
                let mut deferred = DocBuf::new();
                let member_doc = if comments_present
                    && self.has_format_ignore_in_range(prev_end, m.span().start)
                {
                    self.raw_source_range(m.span().start, member_content_end)
                } else {
                    self.build_type_member_doc_inner(m, &mut deferred)
                };
                member_parts.push(member_doc);

                // Handle trailing comments - preserve position relative to semicolon
                let upper_bound = t
                    .members
                    .get(i + 1)
                    .map_or(t.span.end, |next| next.span().start);
                let trailing: CommentVec<'_> = if comments_present {
                    comments_to_emit_in_range(self.comments, member_content_end, upper_bound)
                        .filter(|c| self.is_same_line(member_content_end, c.span.start))
                        .collect()
                } else {
                    CommentVec::new()
                };
                member_parts.extend(self.build_comments_around_semicolon_doc(
                    &trailing,
                    member_content_end,
                    upper_bound,
                ));
                // Own-line comments from the member→`;` gap, deferred past the `;`.
                member_parts.extend(deferred);

                prev_end = m.span().end;
            }

            let body_end = t.span.end.saturating_sub(1);
            if comments_present {
                member_parts.extend(self.build_trailing_body_comments_doc(prev_end, body_end));
            }

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
                let has_leading = self.has_comments_to_emit_between(p.span.start + 1, inner_start);
                let has_trailing = self.has_comments_to_emit_between(inner_end, p.span.end - 1);
                if !has_leading && !has_trailing {
                    return inner;
                }
                let leading: CommentVec<'_> = if has_leading {
                    comments_to_emit_in_range(self.comments, p.span.start + 1, inner_start)
                        .collect()
                } else {
                    smallvec![]
                };
                let trailing: CommentVec<'_> = if has_trailing {
                    comments_to_emit_in_range(self.comments, inner_end, p.span.end - 1).collect()
                } else {
                    smallvec![]
                };
                // A line comment forces the type-argument list to break. Emit
                // `break_parent` FIRST so it sits behind the inner type's group in the
                // forward `fits()` scan — otherwise it poisons that scan and needlessly
                // expands an inner union (`Foo<(a | b // c)>` keeps `a | b` inline,
                // matching prettier, rather than `| a | b`).
                let needs_break = leading
                    .iter()
                    .chain(&trailing)
                    .any(|comment| !comment.is_block);
                let mut parts = DocBuf::new();
                if needs_break {
                    parts.push(d.break_parent());
                }
                for comment in &leading {
                    parts.push(self.build_comment_doc(comment));
                    if comment.is_block {
                        parts.push(d.text(" "));
                    } else {
                        parts.push(d.hardline());
                    }
                }
                parts.push(inner);
                for comment in &trailing {
                    parts.push(self.build_trailing_comment_doc(comment));
                }
                d.concat(&parts)
            }
            _ => self.build_type_doc(ts_type),
        }
    }
}
