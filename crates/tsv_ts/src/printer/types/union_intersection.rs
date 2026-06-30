// Union and intersection type printing for TypeScript
//
// Handles:
// - Union types: `A | B | C`
// - Intersection types: `A & B & C`
// - Comment handling between type members

use super::super::comments_in_range;
use super::helpers::{
    find_separator_position, intersection_has_expanding_first_type,
    intersection_has_huggable_last_type, should_hug_union_type,
    type_needs_parens_in_union_or_intersection, type_never_needs_parens, unwrap_parenthesized,
};
use super::{CommentFilter, CommentSpacing, Printer};
use crate::ast::internal::{TSIntersectionType, TSParenthesizedType, TSType, TSUnionType};
use crate::printer::CommentVec;
use crate::printer::analysis::has_newline_after_position;
use crate::printer::layout::hang_after_operator;
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// Member-parens predicate for a union/intersection with `member_count` members.
/// A single-member union/intersection collapses to its member (Prettier
/// postprocess), so the lone member needs no precedence parens of its own;
/// 2+ members use the normal `|`/`&` precedence rule.
fn union_member_parens(member_count: usize) -> fn(&TSType<'_>) -> bool {
    if member_count == 1 {
        type_never_needs_parens
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

impl<'a> Printer<'a> {
    //
    // Union Types
    //

    /// Emit the leading block comments in `[start, end)` before the FIRST union
    /// member, choosing the separator after each comment per Prettier's
    /// `printLeadingComment`: a `line` when the source has a newline after the
    /// comment's `*/`, a `hardline` when there is a newline both before its `/*`
    /// and after its `*/`, otherwise a space. The `line` lets the member stay
    /// glued inline when the union fits and break onto its own line when the
    /// union expands (the leading-pipe form), matching Prettier — tsv previously
    /// hardcoded a space, gluing a member onto a multi-line comment's `*/` line.
    /// Returns an empty doc when the range has no block comment.
    ///
    /// Scoped to the first member on purpose: a block comment between two members
    /// is a distinct divergence (Prettier relocates it to trail the previous
    /// member), handled by the hardcoded-space path in `build_union_type_doc`.
    fn build_member_leading_block_comments(&self, start: u32, end: u32) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        for comment in comments_in_range(self.comments, start, end) {
            if !comment.is_block {
                continue;
            }
            parts.push(self.build_comment_doc(comment));
            if has_newline_after_position(self.source, comment.span.end) {
                if self.is_own_line_comment(comment) {
                    parts.push(d.hardline());
                } else {
                    parts.push(d.line());
                }
            } else {
                parts.push(d.text(" "));
            }
        }
        d.concat(&parts)
    }

    /// Build a union member's type doc with Prettier's per-member `align(2, …)`
    /// offset (`union-type.js`) rendered as one indent level (a whole tab,
    /// tabs-only — see `docs/conformance_prettier.md`).
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
        let member_doc = self.build_type_doc_maybe_parens(t, member_parens);
        if member_parens(t) && !is_paren_union_member(t) {
            member_doc
        } else {
            d.indent(member_doc)
        }
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
    pub(in crate::printer) fn build_union_type_doc(&self, union: &TSUnionType<'_>) -> DocId {
        let d = self.d();
        if union.types.is_empty() {
            return d.empty();
        }

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
        if !self.union_has_comments_between_members(union) && should_hug_union_type(union) {
            let mut parts = DocBuf::new();
            // Extract leading block comments before the first type
            // (e.g., `| /* c */ A` — comment between leading `|` and first member)
            if let Some(first) = union.types.first() {
                parts.push(self.build_comments_between_filtered(
                    union.span.start,
                    first.span().start,
                    CommentSpacing::Trailing,
                    CommentFilter::BlockOnly,
                ));
            }
            for (i, t) in union.types.iter().enumerate() {
                if i > 0 {
                    parts.push(d.text(" | "));
                }
                parts.push(self.build_type_doc_maybe_parens(t, member_parens));
            }
            return d.concat(&parts);
        }

        // Check for line comments that force the multiline layout:
        // - Between union members (`A | B // c\n  | C`)
        // - Before the first member (`| // c\n  A | B`)
        // - Inside a member's stripped paren (`A | (// c\n  B)`) — these are
        //   relocated to trail the previous member in the multiline path.
        let first_type_start = union.types.first().map(|t| t.span().start);
        let has_leading_line_comments = first_type_start
            .is_some_and(|start| self.has_line_comments_between(union.span.start, start));
        let has_paren_inner_leading_line_comments = union.types.iter().any(
            |t| matches!(t, TSType::Parenthesized(p) if self.paren_has_leading_line_comment(p)),
        );
        if has_leading_line_comments
            || self.union_has_own_line_member_comment(union)
            || has_paren_inner_leading_line_comments
        {
            return self.build_union_type_doc_with_line_comments(union);
        }

        // Build parts: each type prefixed conditionally with `| ` or nothing
        // Flat: T1 | T2 | T3
        // Break: | T1
        //        | T2
        //        | T3
        let mut parts = DocBuf::new();

        for (i, t) in union.types.iter().enumerate() {
            let type_start = t.span().start;
            let type_end = t.span().end;

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
                let prev_type_end = union.types[i - 1].span().end;
                if let Some(pipe_pos) =
                    find_separator_position(self.source, prev_type_end, type_start, b'|')
                {
                    parts.push(self.build_comments_between_filtered(
                        pipe_pos + 1,
                        type_start,
                        CommentSpacing::Trailing,
                        CommentFilter::BlockOnly,
                    ));
                }
            } else {
                // First type: "| " when broken, nothing when flat
                parts.push(d.if_break(d.text("| "), d.empty()));

                // Extract leading block comments before the first type
                // (e.g., `| /* c */ A | B` — comment between leading `|` and first member)
                parts.push(self.build_member_leading_block_comments(union.span.start, type_start));
            }

            // Apply Prettier's per-member `align(2, …)` offset (rendered as one
            // whole tab) — see `build_union_member_offset_doc`.
            parts.push(self.build_union_member_offset_doc(t, member_parens));

            // Add trailing block comments after this type (before the next `|` separator)
            if i + 1 < union.types.len() {
                let next_type_start = union.types[i + 1].span().start;
                if let Some(pipe_pos) =
                    find_separator_position(self.source, type_end, next_type_start, b'|')
                {
                    parts.push(self.build_comments_between_filtered(
                        type_end,
                        pipe_pos,
                        CommentSpacing::Leading,
                        CommentFilter::BlockOnly,
                    ));
                }
            } else {
                // Last type - include all trailing comments up to union span end
                parts.push(self.build_comments_between_filtered(
                    type_end,
                    union.span.end,
                    CommentSpacing::Leading,
                    CommentFilter::BlockOnly,
                ));
            }
        }

        // Always group the broken-member doc (Prettier's `printed = group(members)`).
        // The group makes the union's own flat/broken decision independently of the
        // parent's break, so it re-fits on the continuation line before exploding.
        d.group(d.concat(&parts))
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
        if should_hug_union_type(union) {
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
    /// intersection arms. The `group(indent(...))` wrapper is skipped when a
    /// boundary type owns its own expansion (TypeLiteral/Mapped at the first or
    /// last position) — indenting it again would double-indent the object body.
    pub(in crate::printer) fn intersection_hanging_with_indent(
        &self,
        intersection: &TSIntersectionType<'_>,
    ) -> DocId {
        let d = self.d();
        let type_doc = self.build_intersection_type_doc(intersection, false);
        if intersection_has_huggable_last_type(intersection)
            || intersection_has_expanding_first_type(intersection)
        {
            type_doc
        } else {
            d.group(d.indent(type_doc))
        }
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

        for (i, t) in union.types.iter().enumerate() {
            let type_start = t.span().start;
            let type_end = t.span().end;

            // For non-first members, detect leading line comments inside the
            // parens of a TSParenthesizedType wrapper. Prettier relocates these
            // to trail the previous member (e.g., `a | (// c\n b)` becomes
            // `| a // c\n | b`). We extract them so they can be emitted before
            // the `| ` separator and skipped when building the member's type doc.
            let relocated_paren_leading: CommentVec<'_> = if i > 0
                && let TSType::Parenthesized(p) = t
            {
                self.paren_leading_line_comments(p)
            } else {
                smallvec![]
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
                    parts.extend(self.build_trailing_comments_multiline_ext(
                        prev_type_end,
                        pipe_pos,
                        true,
                    ));

                    // Relocated paren leading line comments: trail prev member
                    for comment in &relocated_paren_leading {
                        parts.push(self.build_trailing_line_comment_doc(comment));
                    }

                    // Comments after the pipe lead this member. Line comments (and
                    // own-line block comments) go on their own line BEFORE the `| `
                    // separator so the pipe stays attached to the type
                    // (`| A\n// c\n| B`). Inline block comments stay after `| `
                    // (`| /* c */ B`). Prettier instead relocates such comments to
                    // trail the previous member — see
                    // union_infix_pipe_line_comment_prettier_divergence.
                    let after_pipe = pipe_pos + 1;
                    let own_line: CommentVec<'_> =
                        comments_in_range(self.comments, after_pipe, type_start)
                            .filter(|c| !(c.is_block && self.is_same_line(c.span.end, type_start)))
                            .collect();
                    // A blank line the author left *before* the first own-line comment
                    // (`A |⏎⏎/* c */⏎B`) and *between* two own-line comments is preserved,
                    // matching prettier — but NOT one after the last comment before the
                    // member (prettier emits none there). This mirrors the intersection
                    // own-line path with the axes swapped: prettier's union and
                    // intersection printers preserve blanks in opposite member-gap
                    // positions.
                    if let Some(first) = own_line.first()
                        && self.has_blank_line_between(prev_type_end, first.span.start)
                    {
                        parts.push(d.literalline());
                    }
                    parts.push(d.hardline());
                    for (j, comment) in own_line.iter().enumerate() {
                        parts.push(self.build_comment_doc(comment));
                        if let Some(next) = own_line.get(j + 1)
                            && self.has_blank_line_between(comment.span.end, next.span.start)
                        {
                            parts.push(d.literalline());
                        }
                        parts.push(d.hardline());
                    }
                    parts.push(d.text("| "));
                    for comment in comments_in_range(self.comments, after_pipe, type_start) {
                        if comment.is_block && self.is_same_line(comment.span.end, type_start) {
                            parts.push(self.build_comment_doc(comment));
                            parts.push(d.text(" "));
                        }
                    }
                } else {
                    // No pipe found, just add separator
                    parts.push(d.hardline());
                    parts.push(d.text("| "));
                }
            } else {
                // First type: always has `| ` prefix when multiline
                parts.push(d.text("| "));

                // Extract leading comments before the first type. Both block and
                // line comments are emitted here — line comments require multiline
                // and place the type on the next line (e.g., `| // c\n   A`).
                parts.extend(self.build_leading_comments_multiline(union.span.start, type_start));
            }

            // The per-member offset shifts a member's *internal* break lines past
            // the `| ` prefix; it must only apply when the member's first line is
            // glued to `| `. The first member is dropped onto its own line by a
            // leading own-line comment — either before it (`| // c\n a`) or inside
            // its stripped parens (`(// c\n a)`) — where the offset would wrongly
            // indent the member's own first line. Non-first members keep their
            // leading line comments before the pipe, so they stay glued.
            let member_on_own_line = i == 0
                && (comments_in_range(self.comments, union.span.start, type_start)
                    .any(|c| !(c.is_block && self.is_same_line(c.span.end, type_start)))
                    || matches!(t, TSType::Parenthesized(p) if self.paren_has_leading_line_comment(p)));

            // Add the type with the same per-member offset as the main path
            // (`build_union_member_offset_doc`). When we relocated leading line
            // comments from inside a `TSParenthesizedType` wrapper, build the inner
            // type directly so the relocated comments aren't emitted again, re-wrap
            // with the proper parenthesized layout when precedence demands it, and
            // apply the offset so it lines up like any other member.
            if !relocated_paren_leading.is_empty()
                && let TSType::Parenthesized(p) = t
            {
                let inner = p.type_annotation;
                if let TSType::Union(union) = inner {
                    // `build_parenthesized_union_doc` lays out `(`/`)` on their own
                    // lines and only emits block comments in the paren gaps (the
                    // leading line comment was already relocated above, so pass
                    // `false`). A paren-union takes the per-member offset (see
                    // `build_union_member_offset_doc`).
                    parts.push(d.indent(self.build_parenthesized_union_doc(union, Some(p), false)));
                } else if type_needs_parens_in_union_or_intersection(inner) {
                    // Default-paren (function / conditional / intersection) supplies
                    // its own indent — no extra offset.
                    parts.push(d.concat(&[
                        d.text("("),
                        d.indent(self.build_type_doc(inner)),
                        d.text(")"),
                    ]));
                } else {
                    parts.push(d.indent(self.build_type_doc(inner)));
                }
            } else if i == 0
                && let TSType::Parenthesized(p) = t
                && let TSType::Union(inner_union) = p.type_annotation
                && self.paren_has_leading_line_comment(p)
            {
                // First union member is a parenthesized union with a leading line
                // comment inside the parens. Unlike a later member (relocated to trail
                // the previous member above), there is no previous member, so keep the
                // comment inside the parens leading the inner union — passing `true`
                // to `build_parenthesized_union_doc` so it is not dropped. Take the
                // per-member offset like any other paren-union member (matching the
                // `is_paren_union_member` arm of `build_union_member_offset_doc`).
                parts.push(d.indent(self.build_parenthesized_union_doc(
                    inner_union,
                    Some(p),
                    true,
                )));
            } else if member_on_own_line {
                // Dropped onto its own line by a leading comment — emit without the
                // offset (it would indent the member's own first line). Objects
                // still self-indent via `build_union_member_object_literal_doc`.
                if let TSType::TypeLiteral(obj) = t {
                    parts.push(self.build_union_member_object_literal_doc(obj));
                } else {
                    parts.push(self.build_type_doc_maybe_parens(t, member_parens));
                }
            } else {
                parts.push(self.build_union_member_offset_doc(t, member_parens));
            }

            // Trailing comments on last type
            if i == union.types.len() - 1 {
                for comment in comments_in_range(self.comments, type_end, union.span.end) {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
            }
        }

        d.concat(&parts)
    }

    /// Check if a union type has any comments between consecutive members.
    ///
    /// Matches prettier's `hasComment(node)` for the detached comment model:
    /// comments between member spans correspond to attached trailing/leading
    /// comments in prettier's AST.
    fn union_has_comments_between_members(&self, union: &TSUnionType<'_>) -> bool {
        union
            .types
            .windows(2)
            .any(|pair| self.has_comments_between(pair[0].span().end, pair[1].span().start))
    }

    /// Check if a union type has line comments between any consecutive members.
    ///
    /// Used by callers (e.g., mapped types) to decide whether the union needs
    /// extra indentation wrapping, and internally to force multiline formatting.
    pub(crate) fn union_has_line_comments_between_members(&self, union: &TSUnionType<'_>) -> bool {
        union
            .types
            .windows(2)
            .any(|pair| self.has_line_comments_between(pair[0].span().end, pair[1].span().start))
    }

    /// True when an **own-line comment** sits between two consecutive members —
    /// a line comment (which can never be inline), or a block comment with a
    /// newline before it (`| 'x'⏎/* c */⏎| 'y'`), on either side of the `|`.
    ///
    /// Prettier emits such a comment via `printComments` with a hardline
    /// (`union-type.js`), forcing the whole union group to break
    /// one-member-per-line. A *same-line* block comment (`a /* c */ | b`) does
    /// not count — it stays inline, matching `union_intersection_parens_comment`.
    /// Subsumes the between-members case of `union_has_line_comments_between_members`
    /// (a line comment is always own-line here) and additionally catches own-line
    /// *block* comments, which the default (groupable) path would otherwise keep flat.
    fn union_has_own_line_member_comment(&self, union: &TSUnionType<'_>) -> bool {
        union.types.windows(2).any(|pair| {
            let (prev_end, next_start) = (pair[0].span().end, pair[1].span().start);
            comments_in_range(self.comments, prev_end, next_start)
                .any(|c| self.is_own_line_comment(c))
        })
    }

    /// True when a comment between two consecutive intersection members forces the
    /// whole intersection one-member-per-line.
    ///
    /// A **line** comment always forces it. A **block** comment forces it only when
    /// it sits on its OWN line between the members — *not* inline-adjacent to the
    /// previous member (`A /* c */⏎& B`) nor to the following one (`A &⏎/* c */ B`),
    /// both of which prettier keeps inline (`A /* c */ & B`). Only a block isolated
    /// from both neighbors (`A &⏎/* c */⏎B`) breaks (`intersection-type.js`).
    ///
    /// This deliberately differs from the union's `union_has_own_line_member_comment`
    /// (which keys on `is_own_line_comment` — the preceding newline alone): prettier's
    /// **union** printer expands a block adjacent to its member, but the
    /// **intersection** printer collapses it, so keying on the preceding newline here
    /// would over-expand the `A &⏎/* c */ B` case.
    fn intersection_has_isolated_member_comment(
        &self,
        intersection: &TSIntersectionType<'_>,
    ) -> bool {
        intersection.types.windows(2).any(|pair| {
            let (prev_end, next_start) = (pair[0].span().end, pair[1].span().start);
            comments_in_range(self.comments, prev_end, next_start)
                .any(|c| self.comment_isolated_from_neighbors(prev_end, c, next_start))
        })
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
    /// See also: `build_intersection_type_annotation_doc` in type_annotation.rs
    /// for the `: Type` annotation variant (shares continuation logic).
    pub(in crate::printer) fn build_intersection_type_doc(
        &self,
        intersection: &TSIntersectionType<'_>,
        wrap_in_group: bool,
    ) -> DocId {
        let d = self.d();
        if intersection.types.is_empty() {
            return d.empty();
        }

        // A single-member intersection collapses to its member — see the matching
        // note in `build_union_type_doc`. The lone member needs no precedence
        // parens (the parent context supplies any), while comment-aware paths
        // below still preserve comments around the `&`/parens.
        let member_parens = union_member_parens(intersection.types.len());

        // Hoist leading line comments inside the first member's stripped parens
        // OUT of the intersection (e.g., `(// c\n a) & b` → `// c\n a & b`).
        // The comment goes on its own line BEFORE the intersection so the
        // intersection content itself can still fit inline.
        if let Some(TSType::Parenthesized(first_paren)) = intersection.types.first() {
            let first_paren_leading = self.paren_leading_line_comments(first_paren);
            if !first_paren_leading.is_empty() {
                let inner = self.build_intersection_type_doc_with_first_paren_leading_stripped(
                    intersection,
                    first_paren,
                );
                let mut parts = DocBuf::new();
                for comment in &first_paren_leading {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.hardline());
                }
                parts.push(inner);
                return d.concat(&parts);
            }
        }

        // Check for isolated comments between intersection members (force multiline).
        // Only the gaps between member types, not inside them. A block comment on its
        // own line between the members counts (`X &⏎/* c */⏎Y`), as does any line
        // comment — but a block inline-adjacent to either member stays inline (unlike
        // the union path, which expands an adjacent block).
        let has_isolated_comment_between_members =
            self.intersection_has_isolated_member_comment(intersection);
        // A non-first parenthesized **union** member with a leading line comment
        // inside its parens (`(a | b) & (// c⏎ a | b)`) also forces the multiline
        // layout: a line comment can't be inline, and tsv preserves it inside the
        // parens (the paren member breaks open). The *first*-member case is already
        // hoisted out above; this catches the rest, which would otherwise drop the
        // comment. Restricted to a union inner because that is the only shape the
        // multiline path renders comment-aware (`build_parenthesized_union_doc`).
        // TODO: a paren-intersection / paren-function member with a leading line
        // comment still drops it — extend when a real case appears.
        let has_nonfirst_paren_leading_line_comment = intersection.types.iter().skip(1).any(|t| {
            matches!(t, TSType::Parenthesized(p)
                if matches!(p.type_annotation, TSType::Union(_))
                    && self.paren_has_leading_line_comment(p))
        });
        if has_isolated_comment_between_members || has_nonfirst_paren_leading_line_comment {
            let doc = self.build_intersection_type_doc_with_line_comments(intersection);
            // The line-comment layout emits continuation members with a bare hardline
            // and no indent, relying on the caller to supply the hanging indent. When
            // `wrap_in_group` is set — the generic `build_type_doc` path used for type
            // arguments, tuple elements, mapped-type values, and conditional branches —
            // there is no such caller, so own the continuation indent here, mirroring
            // Prettier's `printIntersectionType` (each continuation member is wrapped in
            // `indent([" &", line, doc])`). When unset, the type-alias / annotation /
            // function-return callers already wrap the result in `indent(...)`.
            return if wrap_in_group {
                d.group(d.indent(doc))
            } else {
                doc
            };
        }

        // For intersection types, prettier uses trailing `&` when breaking,
        // with continuation types indented:
        // Flat: A & B & C
        // Break: A &
        //            B &
        //            C
        //
        // Special case: when a boundary type is huggable (TypeLiteral/MappedType at first
        // or last position in a 2-type intersection), use a space instead of line() to
        // keep `& {` or `} &` hugged. The TypeLiteral handles its own expansion.
        let last_idx = intersection.types.len() - 1;
        let last_is_huggable = intersection_has_huggable_last_type(intersection);
        let first_is_expanding = intersection_has_expanding_first_type(intersection);
        let is_huggable_pair =
            intersection.types.len() == 2 && (last_is_huggable || first_is_expanding);

        // Build first type separately (not indented)
        let mut first_parts = DocBuf::new();
        let first_type = &intersection.types[0];
        let first_type_start = first_type.span().start;
        let first_type_end = first_type.span().end;

        // Extract leading block comments before the first type
        // (e.g., `& /* c */ A & B` — comment between leading `&` and first member)
        first_parts.push(self.build_comments_between_filtered(
            intersection.span.start,
            first_type_start,
            CommentSpacing::Trailing,
            CommentFilter::BlockOnly,
        ));

        first_parts.push(self.build_type_doc_maybe_parens(first_type, member_parens));

        // Add trailing block comments after first type
        if intersection.types.len() > 1 {
            let next_type_start = intersection.types[1].span().start;
            if let Some(amp_pos) =
                find_separator_position(self.source, first_type_end, next_type_start, b'&')
            {
                first_parts.push(self.build_comments_between_filtered(
                    first_type_end,
                    amp_pos,
                    CommentSpacing::Leading,
                    CommentFilter::BlockOnly,
                ));
            }
            first_parts.push(d.text(" &"));
        } else {
            // Single type - include trailing comments
            first_parts.push(self.build_comments_between_filtered(
                first_type_end,
                intersection.span.end,
                CommentSpacing::Leading,
                CommentFilter::BlockOnly,
            ));
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
        let first_expanding_multi =
            first_is_expanding && !is_huggable_pair && intersection.types.len() > 2;
        if first_expanding_multi {
            let mut parts = first_parts;

            for (i, _) in intersection.types.iter().enumerate().skip(1) {
                let body = self.build_intersection_member_body_doc(intersection, i);
                let sep = if i == 1 { d.text(" ") } else { d.line() };
                let mut member: DocBuf = smallvec![sep];
                member.extend(body);

                if i == 1 {
                    // Hugged to first type: no indent
                    parts.extend(member);
                } else {
                    // Per-member indent
                    parts.push(d.indent(d.concat(&member)));
                }
            }

            // Always need a group for line() in index 2+ members
            return d.group(d.concat(&parts));
        }

        // Build continuation types (indented when breaking)
        let mut continuation_parts = DocBuf::new();

        for (i, _) in intersection.types.iter().enumerate().skip(1) {
            let is_last = i == last_idx;

            // Huggable pair: always space (TypeLiteral handles its own expansion)
            // Multi-type with huggable last: space only for the last type
            if is_huggable_pair || (is_last && last_is_huggable) {
                continuation_parts.push(d.text(" "));
            } else {
                continuation_parts.push(d.line());
            }

            continuation_parts.extend(self.build_intersection_member_body_doc(intersection, i));
        }

        // Combine: first_parts + continuation
        //
        // Indentation logic:
        // - If huggable is ONLY continuation (A & {b}): no indent
        //   TypeLiteral handles its own expansion, no extra indent needed
        // - If huggable with other continuations (A & B & {c}): wrap in indent
        //   When intersection breaks, TypeLiteral content needs continuation indentation
        // - No huggable at all: no indent
        //   Parent context provides indent (e.g., type alias wraps at =)
        let mut parts = first_parts;
        let has_non_huggable_continuations = last_is_huggable && intersection.types.len() > 2;
        if !continuation_parts.is_empty() {
            if has_non_huggable_continuations {
                // Multiple continuations with huggable at end - wrap in indent
                parts.push(d.indent(d.concat(&continuation_parts)));
            } else {
                // Either huggable-only or no huggable - no internal indent
                parts.extend(continuation_parts);
            }
        }

        // Need a group when:
        // - wrap_in_group requested by caller, OR
        // - non-huggable continuations exist (line() between them needs a group
        //   to go flat when content fits on one line)
        if wrap_in_group || has_non_huggable_continuations {
            d.group(d.concat(&parts))
        } else {
            d.concat(&parts)
        }
    }

    /// Build a Doc for an intersection type with line comments between members.
    ///
    /// Line comments force the intersection to be multiline because a line comment
    /// cannot be followed by content on the same line.
    ///
    /// Structure (intersection uses trailing `&`):
    /// ```text
    /// A &
    /// // comment before B
    /// B
    /// ```
    ///
    /// Note: The caller (type alias printer) handles the outer indent, so this function
    /// does not add internal indentation.
    fn build_intersection_type_doc_with_line_comments(
        &self,
        intersection: &TSIntersectionType<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        let member_parens = union_member_parens(intersection.types.len());

        for (i, t) in intersection.types.iter().enumerate() {
            let type_start = t.span().start;
            let type_end = t.span().end;

            if i > 0 {
                // Get previous type end and find the ampersand position
                let prev_type_end = intersection.types[i - 1].span().end;

                if let Some(amp_pos) =
                    find_separator_position(self.source, prev_type_end, type_start, b'&')
                {
                    // Comments before the ampersand (trailing on previous type's line or on own lines)
                    parts.extend(self.build_trailing_comments_multiline(prev_type_end, amp_pos));

                    // Comments after the ampersand - split into trailing (same line as &) and leading (own line)
                    let comments_after_amp: CommentVec<'_> =
                        comments_in_range(self.comments, amp_pos + 1, type_start).collect();

                    // Trailing comments on same line as & (come before hardline)
                    for comment in comments_after_amp
                        .iter()
                        .filter(|c| self.is_same_line(amp_pos, c.span.start))
                    {
                        parts.push(d.text(" "));
                        parts.push(self.build_comment_doc(comment));
                    }

                    // Newline for continuation
                    parts.push(d.hardline());

                    // Leading comments on their own line (come after hardline). A
                    // block comment inline-adjacent to the member it leads hugs it
                    // with a space (`/* c */ Y`); an own-line block (separated from
                    // the member by a newline) and every line comment take a hardline
                    // so the member drops to its own line — the same split the union
                    // renderer applies after `| ` (keyed on the *following* member,
                    // not on block-vs-line alone). A blank line the author left between
                    // an own-line comment and what follows it (the next own-line comment
                    // or the member) is preserved (`literalline`), matching prettier.
                    let own_line: CommentVec<'_> = comments_after_amp
                        .iter()
                        .copied()
                        .filter(|c| !self.is_same_line(amp_pos, c.span.start))
                        .collect();
                    self.emit_member_leading_comments(&mut parts, &own_line, type_start);
                } else {
                    // No ampersand found, just add newline
                    parts.push(d.hardline());
                }
            }

            // For the first type, extract leading block comments
            // (e.g., `& /* c */ A & B` — comment between leading `&` and first member)
            if i == 0 {
                parts.push(self.build_comments_between_filtered(
                    intersection.span.start,
                    type_start,
                    CommentSpacing::Trailing,
                    CommentFilter::BlockOnly,
                ));
            }

            // Add the type. A parenthesized-union member whose parens hold a
            // leading line comment (`(// c⏎ a | b)`) is built through
            // `build_parenthesized_union_doc` with the inner leading line comment
            // emitted (it breaks the paren open, keeping the comment in place);
            // `build_type_doc_maybe_parens` would re-wrap the parens but drop that
            // comment. Block-only / non-union parens fall through to the default.
            if let TSType::Parenthesized(p) = t
                && let TSType::Union(inner_union) = p.type_annotation
                && self.paren_has_leading_line_comment(p)
            {
                parts.push(self.build_parenthesized_union_doc(inner_union, Some(p), true));
            } else {
                parts.push(self.build_type_doc_maybe_parens(t, member_parens));
            }

            // Add trailing `&` for all but last type
            if i < intersection.types.len() - 1 {
                parts.push(d.text(" &"));
            } else {
                // Trailing comments on last type
                for comment in comments_in_range(self.comments, type_end, intersection.span.end) {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
            }
        }

        d.concat(&parts)
    }

    /// Build an intersection type's doc with the first member's stripped-paren
    /// leading line comments excluded from the output. Used by the hoisting
    /// path in `build_intersection_type_doc` — the caller emits the hoisted
    /// comment before this doc, and passes the first member's `TSParenthesizedType`
    /// directly so we can strip its parens without re-matching.
    fn build_intersection_type_doc_with_first_paren_leading_stripped(
        &self,
        intersection: &TSIntersectionType<'_>,
        first_paren: &TSParenthesizedType<'_>,
    ) -> DocId {
        let d = self.d();
        let member_parens = union_member_parens(intersection.types.len());
        let inner = first_paren.type_annotation;
        let first_doc = if member_parens(inner) {
            // Re-wrap inner in parens (e.g., union in intersection: `(A | B) & C`).
            if let TSType::Union(union) = inner {
                // The hoisted leading line comment is emitted by the caller, so pass
                // `false` to keep `build_parenthesized_union_doc` block-comment-only.
                self.build_parenthesized_union_doc(union, Some(first_paren), false)
            } else {
                // Matches the default parenthesization in `build_type_doc_maybe_parens`:
                // the closing `)` sits at the base indent, no sub-tab alignment.
                d.concat(&[
                    d.text("("),
                    d.indent(self.build_type_doc(inner)),
                    d.text(")"),
                ])
            }
        } else {
            self.build_type_doc(inner)
        };

        // Build the rest as `first & second & third...` inline (the hoisted
        // comment forces a hardline before; we want the intersection itself
        // to remain compact when possible).
        let mut parts: DocBuf = smallvec![first_doc];
        for t in intersection.types.iter().skip(1) {
            parts.push(d.text(" & "));
            parts.push(self.build_type_doc_maybe_parens(t, member_parens));
        }
        d.concat(&parts)
    }

    /// Build the body of an intersection continuation member (everything except separator).
    ///
    /// Returns: leading comments + type doc + trailing comments/`&` separator.
    /// Used by both the normal and expanding-first-type paths.
    fn build_intersection_member_body_doc(
        &self,
        intersection: &TSIntersectionType<'_>,
        i: usize,
    ) -> DocBuf {
        let t = &intersection.types[i];
        let type_start = t.span().start;
        let type_end = t.span().end;
        let is_last = i == intersection.types.len() - 1;
        let mut parts = DocBuf::new();

        // Leading block comments (after the `&` separator)
        let prev_type_end = intersection.types[i - 1].span().end;
        if let Some(amp_pos) = find_separator_position(self.source, prev_type_end, type_start, b'&')
        {
            parts.push(self.build_comments_between_filtered(
                amp_pos + 1,
                type_start,
                CommentSpacing::Trailing,
                CommentFilter::BlockOnly,
            ));
        }

        parts.push(self.build_type_doc_maybe_parens(t, type_needs_parens_in_union_or_intersection));

        // Trailing block comments + `&` separator (or end-of-intersection comments)
        if !is_last {
            let next_type_start = intersection.types[i + 1].span().start;
            if let Some(amp_pos) =
                find_separator_position(self.source, type_end, next_type_start, b'&')
            {
                parts.push(self.build_comments_between_filtered(
                    type_end,
                    amp_pos,
                    CommentSpacing::Leading,
                    CommentFilter::BlockOnly,
                ));
            }
            parts.push(self.d().text(" &"));
        } else {
            parts.push(self.build_comments_between_filtered(
                type_end,
                intersection.span.end,
                CommentSpacing::Leading,
                CommentFilter::BlockOnly,
            ));
        }

        parts
    }
}
