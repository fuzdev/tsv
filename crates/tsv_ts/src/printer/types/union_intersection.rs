// Union and intersection type printing for TypeScript
//
// Handles:
// - Union types: `A | B | C`
// - Intersection types: `A & B & C`
// - Comment handling between type members

use super::super::comments_in_range;
use super::helpers::{
    find_separator_position, intersection_has_expanding_first_type,
    intersection_has_huggable_last_type, is_huggable_type, is_hugging_union_type_arg,
    should_hug_union_type, type_needs_parens_in_union_or_intersection, type_never_needs_parens,
    unwrap_parenthesized,
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

        // A single-member union has no `|` of its own: prettier drops single-element
        // `TSUnionType` nodes in postprocess, so the lone member prints in the union's
        // position with NO leading pipe and NO per-member offset. Rendering it
        // transparently collapses a nested `| (| (| A | B))` to the innermost
        // multi-member union (`| A | B`) instead of stacking a leading `|` per level —
        // the flat form already collapses, but the loop below emits each level's
        // `if_break("| ")` + offset once a nested comment forces the union multiline.
        // Placed after the hug/line-comment paths so a leading line comment (which the
        // block-only comment helper can't carry) still routes there. A block comment
        // between the dropped `|` and the member is preserved. `member_parens` here is
        // `type_never_needs_parens`, so any required parens come from the parent one
        // level up.
        if union.types.len() == 1 {
            let member = &union.types[0];
            let leading =
                self.build_member_leading_block_comments(union.span.start, member.span().start);
            let member_doc = self.build_type_doc_maybe_parens(member, member_parens);
            return d.concat(&[leading, member_doc]);
        }

        // Build parts: each type prefixed conditionally with `| ` or nothing
        // Flat: T1 | T2 | T3
        // Break: | T1
        //        | T2
        //        | T3
        let mut parts = d.pooled_docbuf();

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
        self.build_intersection_type_doc(intersection, !boundary_owns_expansion)
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
    /// True when a return-type union hugs its brace member block-style
    /// (`{ … } | null` / `| void`) instead of breaking before it — the object owns
    /// its own expansion, the same layout the type-alias RHS / `as` cast use. The
    /// single source of truth shared by the function-type `=>` return
    /// (`build_function_type_return_doc`) and the `: Type` annotation return
    /// (`build_type_annotation_doc_with_wrapping`), so the two arms can't drift (the
    /// drift is exactly what let the hug miss those contexts before).
    ///
    /// Requires a brace member with only void siblings (`is_hugging_union_type_arg` —
    /// a `TypeLiteral`/`Mapped`; excludes the `Promise<…> | null` `TSTypeReference`
    /// print-width family). A comment prettier's `shouldHugUnionType` would bail on —
    /// between two members, or in the operator→union gap `[gap_start, gap_end]` —
    /// disqualifies the hug; an *inside-object* comment (`{ /* c */ … }`) does not.
    /// `gap_start`/`gap_end` bound the source between the `=>`/`:` and the union.
    pub(crate) fn union_return_hugs(
        &self,
        value_type: &TSType<'_>,
        union: &TSUnionType<'_>,
        gap_start: u32,
        gap_end: u32,
    ) -> bool {
        is_hugging_union_type_arg(value_type)
            && !self.union_has_comments_between_members(union)
            && !self.has_comments_between(gap_start, gap_end)
    }

    pub(crate) fn union_has_comments_between_members(&self, union: &TSUnionType<'_>) -> bool {
        // Zero-comment window gate: one binary search over the whole union span before
        // the N-1 pairwise between-member searches. Each pairwise range lies within
        // `[union.span.start, union.span.end]`, so with no comment inside the union
        // every pairwise check is provably false — skip them on the common
        // comment-free `A | B | C`.
        if !self.has_comments_between(union.span.start, union.span.end) {
            return false;
        }
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
        // Zero-comment window gate (as in `union_has_comments_between_members`): every
        // pairwise range lies within the union span, so no comment inside the union
        // means every pairwise scan below is provably false — skip them on the common
        // comment-free `A | B | C`.
        if !self.has_comments_between(union.span.start, union.span.end) {
            return false;
        }
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
        // Zero-comment window gate (see `union_has_comments_between_members`): every
        // pairwise range lies within the union span, so no comment inside the union
        // means every `comments_in_range` below is empty — skip the N-1 scans on the
        // common comment-free union.
        if !self.has_comments_between(union.span.start, union.span.end) {
            return false;
        }
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
        // Zero-comment window gate (see `union_has_comments_between_members`): every
        // pairwise range lies within the intersection span, so no comment inside it
        // means every `comments_in_range` below is empty — skip the N-1 scans.
        if !self.has_comments_between(intersection.span.start, intersection.span.end) {
            return false;
        }
        intersection.types.windows(2).any(|pair| {
            let (prev_end, next_start) = (pair[0].span().end, pair[1].span().start);
            comments_in_range(self.comments, prev_end, next_start)
                .any(|c| self.comment_isolated_from_neighbors(prev_end, c, next_start))
        })
    }

    /// True when the intersection must use the multiline, comment-aware layout
    /// (`build_intersection_type_doc_with_line_comments`) rather than the inline form —
    /// because a comment can't be inline. Two triggers:
    ///
    /// - an **isolated** comment between two members (any line comment, or an own-line
    ///   block — see `intersection_has_isolated_member_comment`); a block inline-adjacent
    ///   to either member stays inline (unlike the union path, which expands it);
    /// - a **non-first** parenthesized *union* member with a leading line comment inside
    ///   its parens (`(a | b) & (// c⏎ c | d)`) — a line comment can't be inline, and tsv
    ///   preserves it inside the parens (the member breaks open). The *first*-member case
    ///   is hoisted out by `build_intersection_type_doc`; this catches the rest, which the
    ///   inline form would otherwise drop. Restricted to a union inner — the only shape
    ///   the multiline path renders comment-aware (`build_parenthesized_union_doc`). A
    ///   paren-intersection / paren-function member with a leading line comment still
    ///   drops it — extend when a real case appears.
    fn intersection_needs_line_comment_layout(
        &self,
        intersection: &TSIntersectionType<'_>,
    ) -> bool {
        self.intersection_has_isolated_member_comment(intersection)
            || intersection.types.iter().skip(1).any(|t| {
                matches!(t, TSType::Parenthesized(p)
                    if matches!(p.type_annotation, TSType::Union(_))
                        && self.paren_has_leading_line_comment(p))
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
                // The compact inline body can't represent an *isolated* between-member
                // comment (a line/own-line comment forces multiline); route those through
                // the line-comment path with the first member's (now-hoisted) paren-leading
                // stripped, so the other comments aren't dropped. Otherwise stay compact
                // inline (block comments emitted in place).
                let line_comment_layout = self.intersection_needs_line_comment_layout(intersection);
                let inner = if line_comment_layout {
                    self.build_intersection_type_doc_with_line_comments(intersection, true)
                } else {
                    self.build_intersection_type_doc_with_first_paren_leading_stripped(
                        intersection,
                        first_paren,
                    )
                };
                let mut parts = DocBuf::new();
                for comment in &first_paren_leading {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.hardline());
                }
                parts.push(inner);
                let body = d.concat(&parts);
                // The compact inline body renders flush-left; indent the hoisted
                // comment(s) + intersection under the alias `=` so continuation lines
                // align (`type T = // c⏎⇥A & B`) and the form stays idempotent —
                // without it, pass 2 re-indents the reparsed, no-longer-parenthesized
                // body. The line-comment layout already self-indents per member.
                return if line_comment_layout {
                    body
                } else {
                    d.indent(body)
                };
            }
        }

        if self.intersection_needs_line_comment_layout(intersection) {
            let doc = self.build_intersection_type_doc_with_line_comments(intersection, false);
            // The line-comment layout self-indents per member (mirroring the no-comment
            // loop and Prettier's `printIntersectionType`), so no outer indent is added.
            // The `wrap_in_group` path (type arguments, tuple elements, mapped-type
            // values, conditional branches) still groups it; the hanging callers add the
            // group.
            return if wrap_in_group { d.group(doc) } else { doc };
        }

        // For intersection types, prettier uses trailing `&` when breaking,
        // with continuation types indented:
        // Flat: A & B & C
        // Break: A &
        //            B &
        //            C
        // The per-member separator/indent below (object-adjacency + `was_indented`)
        // keeps a huggable boundary (`& {` / `} &`) space-hugged and un-indented —
        // uniformly, whether the object is first, middle, or last.

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
        // `isObjectType` (`TSTypeLiteral`/`TSMappedType`), read on the raw member (no
        // paren-unwrap) to match Prettier's node check.
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
        let mut needs_group = wrap_in_group;
        for i in 1..intersection.types.len() {
            let prev_is_object = is_huggable_type(&intersection.types[i - 1]);
            let cur_is_object = is_huggable_type(&intersection.types[i]);
            let neither_is_object = !prev_is_object && !cur_is_object;

            let sep = if neither_is_object {
                // A breakable line is the only thing that needs the group to choose
                // between flat and broken.
                needs_group = true;
                d.line()
            } else {
                d.text(" ")
            };

            let indent_member = if neither_is_object {
                true
            } else if prev_is_object && cur_is_object {
                was_indented
            } else {
                // object↔non-object transition: indented (and opens the latch) only
                // past index 1; the index-1 transition hugs the first member at base.
                if i > 1 {
                    was_indented = true;
                    true
                } else {
                    false
                }
            };

            let mut member: DocBuf = smallvec![sep];
            member.extend(self.build_intersection_member_body_doc(intersection, i));
            if indent_member {
                parts.push(d.indent(d.concat(&member)));
            } else {
                parts.extend(member);
            }
        }

        if needs_group {
            d.group(d.concat(&parts))
        } else {
            d.concat(&parts)
        }
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
    /// conformance_prettier.md §Comment relocation.
    ///
    /// The doc self-indents per member (mirroring `build_intersection_type_doc`), so
    /// the caller adds no outer indent.
    ///
    /// `strip_first_paren_leading` is set by the hoist path: the first member's
    /// parenthesized leading line comment has already been emitted before the
    /// intersection, so the first member is built with it stripped (otherwise a
    /// paren-union first member would re-emit it inside the parens).
    fn build_intersection_type_doc_with_line_comments(
        &self,
        intersection: &TSIntersectionType<'_>,
        strip_first_paren_leading: bool,
    ) -> DocId {
        let d = self.d();
        let member_parens = union_member_parens(intersection.types.len());
        let types = &intersection.types;
        let last = types.len() - 1;

        // First member: leading block comments (`& /* c */ A`) + its type. Its
        // trailing `&` is emitted by the next member's iteration (it sits on this line).
        let mut parts = DocBuf::new();
        let first = &types[0];
        parts.push(self.build_comments_between_filtered(
            intersection.span.start,
            first.span().start,
            CommentSpacing::Trailing,
            CommentFilter::BlockOnly,
        ));
        let first_doc = match first {
            TSType::Parenthesized(fp) if strip_first_paren_leading => {
                self.build_intersection_first_member_stripped(fp, member_parens)
            }
            _ => self.build_intersection_line_comment_member_doc(first, member_parens),
        };
        parts.push(first_doc);

        let mut was_indented = false;
        for i in 1..types.len() {
            let prev = &types[i - 1];
            let cur = &types[i];
            let prev_end = prev.span().end;
            let cur_start = cur.span().start;
            let is_last = i == last;

            let amp = find_separator_position(self.source, prev_end, cur_start, b'&');

            // After-`&` comments on their own line lead the member — Prettier's
            // `hasLeadingOwnLineComment`, which forces the boundary to break. (Same-line
            // ones trail the `&` inline and are emitted below.)
            let own_line_leading: CommentVec<'_> = match amp {
                Some(amp_pos) => comments_in_range(self.comments, amp_pos + 1, cur_start)
                    .filter(|c| !self.is_same_line(amp_pos, c.span.start))
                    .collect(),
                None => smallvec![],
            };

            // Per Prettier's `printIntersectionType` per-boundary branch, on the raw
            // members' `isObjectType` (matching the no-comment loop):
            // - both objects → space-hug, indent only once `was_indented` is latched;
            // - neither object, or a leading own-line comment → break + indent;
            // - object↔non-object transition → space-hug, indent (and latch) past index 1.
            let prev_obj = is_huggable_type(prev);
            let cur_obj = is_huggable_type(cur);
            let (mut should_break, indent_member) = if prev_obj && cur_obj {
                (false, was_indented)
            } else if (!prev_obj && !cur_obj) || !own_line_leading.is_empty() {
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
            if !should_break
                && comments_in_range(self.comments, prev_end, cur_start)
                    .any(|c| self.comment_isolated_from_neighbors(prev_end, c, cur_start))
            {
                should_break = true;
            }

            // A same-line block comment authored *before* the `&` trails the previous
            // member and stays on its side of the operator (`prev /* b */ &`) — matching
            // the no-comment loop and Prettier. Emit those first, on the previous
            // member's line (indent-agnostic — no preceding newline), before the `&`.
            if let Some(amp_pos) = amp {
                for comment in comments_in_range(self.comments, prev_end, amp_pos)
                    .filter(|c| c.is_block && self.is_same_line(prev_end, c.span.start))
                {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
            }
            parts.push(d.text(" &"));

            let mut unit = DocBuf::new();
            if let Some(amp_pos) = amp {
                // The remaining before-`&` comments follow the operator: a same-line
                // *line* comment trails it inline (a `//` can't precede the `&` without
                // commenting it out — a lossless separator-trail), and an own-line
                // comment drops to its own line (blank-preserving). Mirrors
                // `build_trailing_comments_multiline` minus the same-line blocks handled
                // above. Then the same-line-after-`&` comments trail the operator inline.
                let mut run_end = prev_end;
                for comment in comments_in_range(self.comments, prev_end, amp_pos) {
                    if self.is_same_line(prev_end, comment.span.start) {
                        if !comment.is_block {
                            unit.push(d.text(" "));
                            unit.push(self.build_comment_doc(comment));
                        }
                    } else {
                        self.push_blank_preserving_hardline(&mut unit, run_end, comment.span.start);
                        unit.push(self.build_comment_doc(comment));
                    }
                    run_end = comment.span.end;
                }
                for comment in comments_in_range(self.comments, amp_pos + 1, cur_start)
                    .filter(|c| self.is_same_line(amp_pos, c.span.start))
                {
                    unit.push(d.text(" "));
                    unit.push(self.build_comment_doc(comment));
                }
            }
            if should_break {
                unit.push(d.hardline());
                self.emit_member_leading_comments(&mut unit, &own_line_leading, cur_start);
            } else {
                unit.push(d.text(" "));
            }
            unit.push(self.build_intersection_line_comment_member_doc(cur, member_parens));
            if is_last {
                for comment in
                    comments_in_range(self.comments, cur.span().end, intersection.span.end)
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
        }

        d.concat(&parts)
    }

    /// Build a single intersection member's type doc for the line-comment path. A
    /// parenthesized-**union** member whose parens hold a leading line comment
    /// (`(// c⏎ a | b)`) is built through `build_parenthesized_union_doc` with the
    /// inner leading line comment emitted (it breaks the paren open, keeping the
    /// comment in place); `build_type_doc_maybe_parens` would re-wrap the parens but
    /// drop that comment. Every other member (block-only / non-union parens) uses the
    /// default.
    fn build_intersection_line_comment_member_doc(
        &self,
        t: &TSType<'_>,
        member_parens: fn(&TSType<'_>) -> bool,
    ) -> DocId {
        if let TSType::Parenthesized(p) = t
            && let TSType::Union(inner_union) = p.type_annotation
            && self.paren_has_leading_line_comment(p)
        {
            self.build_parenthesized_union_doc(inner_union, Some(p), true)
        } else {
            self.build_type_doc_maybe_parens(t, member_parens)
        }
    }

    /// Build the first intersection member's type doc with its parenthesized leading
    /// line comment excluded (the caller — the hoist path — emits that comment before
    /// the intersection). Precedence parens are kept where the member needs them
    /// (`(A | B) & C`) and dropped where redundant (`(a) & b` → `a & b`).
    fn build_intersection_first_member_stripped(
        &self,
        first_paren: &TSParenthesizedType<'_>,
        member_parens: fn(&TSType<'_>) -> bool,
    ) -> DocId {
        let d = self.d();
        let inner = first_paren.type_annotation;
        if member_parens(inner) {
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
        }
    }

    /// Build an intersection type's doc with the first member's stripped-paren
    /// leading line comments excluded from the output — the compact **inline** form
    /// (`a & b & c`) used by the hoisting path in `build_intersection_type_doc` when the
    /// intersection has no *isolated* between-member comment. The caller emits the
    /// hoisted comment before this doc, and passes the first member's
    /// `TSParenthesizedType` directly so we can strip its parens without re-matching.
    ///
    /// Block comments between members are emitted in place — a before-`&` block trails
    /// the previous member, an after-`&` block leads the next — so they aren't dropped.
    /// This inline form is what the re-formatted (hoisted, no-longer-parenthesized)
    /// intersection settles to via the no-comment loop, so it stays idempotent; a
    /// line/own-line comment routes to `build_intersection_type_doc_with_line_comments`
    /// instead (which this compact form can't represent).
    fn build_intersection_type_doc_with_first_paren_leading_stripped(
        &self,
        intersection: &TSIntersectionType<'_>,
        first_paren: &TSParenthesizedType<'_>,
    ) -> DocId {
        let d = self.d();
        let member_parens = union_member_parens(intersection.types.len());
        let first_doc = self.build_intersection_first_member_stripped(first_paren, member_parens);

        // Build the rest as `first & second & third...` inline, preserving any block
        // comment on its authored side of each `&` (`prev /* c */ & /* c */ next`).
        let mut parts: DocBuf = smallvec![first_doc];
        for i in 1..intersection.types.len() {
            let t = &intersection.types[i];
            let prev_end = intersection.types[i - 1].span().end;
            let cur_start = t.span().start;
            let amp = find_separator_position(self.source, prev_end, cur_start, b'&');
            if let Some(amp_pos) = amp {
                parts.push(self.build_comments_between_filtered(
                    prev_end,
                    amp_pos,
                    CommentSpacing::Leading,
                    CommentFilter::BlockOnly,
                ));
            }
            parts.push(d.text(" &"));
            if let Some(amp_pos) = amp {
                parts.push(self.build_comments_between_filtered(
                    amp_pos + 1,
                    cur_start,
                    CommentSpacing::Leading,
                    CommentFilter::BlockOnly,
                ));
            }
            parts.push(d.text(" "));
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
