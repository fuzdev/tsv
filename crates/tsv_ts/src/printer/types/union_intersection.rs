// Union and intersection type printing for TypeScript
//
// Handles:
// - Union types: `A | B | C`
// - Intersection types: `A & B & C`
// - Comment handling between type members

use super::super::comments_to_emit_in_range;
use super::helpers::{
    find_separator_position, intersection_has_expanding_first_type,
    intersection_has_huggable_last_type, is_huggable_type,
    type_needs_parens_in_union_or_intersection, union_has_brace_member, union_hug_shape,
    unwrap_parenthesized,
};
use super::{CommentFilter, CommentSpacing, Printer};
use crate::ast::internal::{TSIntersectionType, TSType, TSUnionType};
use crate::printer::CommentVec;
use crate::printer::LeadingGlue;
use crate::printer::analysis::has_newline_after_position;
use crate::printer::ignore::LeadingRunFreeze;
use crate::printer::layout::hang_after_operator;
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// Member-parens predicate for a union/intersection with `member_count` members.
///
/// A single-member union/intersection collapses to its member (Prettier drops
/// single-element union/intersection nodes in postprocess), so the lone member prints
/// in the union's own position and needs no precedence parens of its own — any required
/// parens come from the union's parent context, applied one level up. 2+ members use the
/// normal `|`/`&` precedence rule.
fn union_member_parens(member_count: usize) -> fn(&TSType<'_>) -> bool {
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

/// Whether a parenthesized member is an object-trailing intersection
/// (`(A & { … })`) — the shape `build_parenthesized_intersection_trailing_object_doc`
/// builds via `build_aligned_object_literal_doc`, which supplies the member's
/// `align(2)` offset itself (on its own closing `})`). Such a member must NOT be
/// wrapped in the offset again, or its body and closing double-shift. Mirrors the
/// detection in `build_type_doc_maybe_parens_impl`.
fn is_object_trailing_intersection_member(ts_type: &TSType<'_>) -> bool {
    if let TSType::Intersection(intersection) = unwrap_parenthesized(ts_type)
        && let Some(last) = intersection.types.last()
    {
        matches!(unwrap_parenthesized(last), TSType::TypeLiteral(_))
    } else {
        false
    }
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
    /// their own line before the `| ` (this run, via [`Self::push_own_line_comment_run`]),
    /// the trailing comment appended to the member via [`Self::with_stripped_paren_trailing`].
    /// Declines a **retained**-paren member (union / intersection / function / conditional —
    /// its comment stays inside, the arms further down) and a non-paren member. Requires a
    /// **line** comment in the leading gap: a block-only (`(/* b */ b)`) or comment-free gap
    /// keeps its block inline and is already idempotent, so it returns empty (the general
    /// member arm). Peels every redundant nesting layer (`((// c⏎ b))` → `b`) to match the
    /// detection window. The narrow shared [`Self::stripped_paren_leading_line_comments`]
    /// (line-only, no block/trailing) still serves the conditional-`extends` and
    /// intersection-first-member callers.
    fn stripped_redundant_paren_member_leading_run(&self, t: &TSType<'_>) -> CommentVec<'_> {
        if type_needs_parens_in_union_or_intersection(t) || !matches!(t, TSType::Parenthesized(_)) {
            return smallvec![];
        }
        let inner = unwrap_parenthesized(t);
        let leading: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, t.span().start, inner.span().start).collect();
        if leading.iter().any(|c| !c.is_block) {
            leading
        } else {
            smallvec![]
        }
    }

    /// Push each comment on its own line (comment + `hardline`), the layout a
    /// stripped-redundant-paren member's leading line run takes before its `| `
    /// separator ([`Self::stripped_redundant_paren_leading_line_comments`]).
    fn push_own_line_comment_run(&self, parts: &mut DocBuf, comments: &CommentVec<'_>) {
        let d = self.d();
        for comment in comments {
            parts.push(self.build_comment_doc(comment));
            parts.push(d.hardline());
        }
    }

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
        for comment in comments_to_emit_in_range(self.comments, start, end) {
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

    /// Append the block comments sitting *after* the `|`/`&` separator and before the
    /// member that follows it (`A | /* c */ B`), appending nothing when there are none.
    ///
    /// The separator's source position is needed only to bound that range — the printed
    /// `|`/`&` is static text — so the caller gates on its whole-union/intersection
    /// window first and this runs only when a comment is actually in play.
    fn push_post_separator_block_comments(
        &self,
        parts: &mut DocBuf,
        prev_member_end: u32,
        member_start: u32,
        separator: u8,
    ) {
        if let Some(sep_pos) =
            find_separator_position(self.source, prev_member_end, member_start, separator)
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
    /// member's own internal indent stacks on it (`docs/conformance_prettier.md`).
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
        if member_parens(t) && !is_paren_union_member(t) {
            if is_object_trailing_intersection_member(t) {
                // `(A & { … })` supplies its own `align(2)` inside
                // `build_aligned_object_literal_doc` (its closing `})`), so it opts
                // out of the wrapper here — wrapping again double-shifts it.
                self.build_type_doc_maybe_parens(t, member_parens)
            } else {
                // Function / constructor / conditional paren member. Prettier's
                // needs-parens wrapping is a bare `["(", doc, ")"]` (no inner
                // indent); the member's whole `(…)` takes the `align(2)` offset, so
                // the content rounds up to a tab and the closing `) => …)` line
                // trails at 2 spaces. Use the intersection-member variant
                // (`indent_default_paren = false`) for the bare paren, then apply
                // the offset — the old inner `d.indent` faked the offset as a whole
                // tab and stranded the closing.
                d.align(2, self.build_intersection_member_type_doc(t, member_parens))
            }
        } else {
            d.align(2, self.build_type_doc_maybe_parens(t, member_parens))
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

        // Format-ignore leading run (Rule A): a directive in the out-of-span region
        // before the union freezes the whole node (same-line glued) or its first member
        // (own-line). Gated on the document-level flag — the in-span `has_comments` gate
        // below cannot see an out-of-span directive. The `Whole` return must precede the
        // hug / line-comment / single-member paths; `freeze_first` is applied in the main
        // loop and the single-member branch.
        let leading_freeze = self.union_leading_run_freeze(union);
        if let Some(LeadingRunFreeze::Whole) = leading_freeze {
            return self.raw_source_range(union.span.start, union.span.end);
        }
        let (freeze_first, freeze_first_multiline) = match leading_freeze {
            Some(LeadingRunFreeze::FirstMember { multiline }) => (true, multiline),
            _ => (false, false),
        };

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
            && (freeze_first
                || self.member_gap_frozen(union.span.start, union.types[0].span().start))
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
        if self.union_prints_hugged(union) {
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
                let frozen = if i == 0 {
                    freeze_first || self.member_gap_frozen(union.span.start, t.span().start)
                } else {
                    self.member_gap_frozen(union.types[i - 1].span().end, t.span().start)
                };
                if frozen {
                    parts.push(self.build_frozen_member_doc(t, member_parens));
                } else {
                    parts.push(self.build_type_doc_maybe_parens(t, member_parens));
                }
            }
            return d.concat(&parts);
        }

        // Check for line comments that force the multiline layout:
        // - Between union members (`A | B // c\n  | C`)
        // - Before the first member (`| // c\n  A | B`)
        // - Inside a member's parens (`A | (// c\n  B)`) — a retained paren keeps the
        //   comment inside; a redundant one leads its member on its own line. Either way
        //   the comment is a line comment, so the multiline layout is required.
        if has_comments {
            let first_type_start = union.types.first().map(|t| t.span().start);
            let has_leading_line_comments = first_type_start
                .is_some_and(|start| self.has_line_comments_between(union.span.start, start));
            // A line comment inside a member's parens (before the — possibly nested —
            // inner type), matching the window `build_union_type_doc_with_line_comments`
            // reads for both the retained-paren and stripped-redundant-paren arms.
            let has_paren_inner_leading_line_comments = union.types.iter().any(|t| {
                matches!(t, TSType::Parenthesized(_))
                    && self.has_line_comments_between(
                        t.span().start + 1,
                        unwrap_parenthesized(t).span().start,
                    )
            });
            if has_leading_line_comments
                || self.union_has_own_line_member_comment(union)
                || has_paren_inner_leading_line_comments
            {
                return self.build_union_type_doc_with_line_comments(union);
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
            // is handled above (`single_member_union_leaf_freeze` for a leaf/object sole
            // member; a composite sole member falls through here and builds normally so
            // its OWN leading-run walk applies Rule A inside — the transparency doctrine).
            if !has_comments {
                return self.build_type_doc_maybe_parens(member, member_parens);
            }
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
                if has_comments {
                    let prev_type_end = union.types[i - 1].span().end;
                    self.push_post_separator_block_comments(
                        &mut parts,
                        prev_type_end,
                        type_start,
                        b'|',
                    );
                }
            } else {
                // A FROZEN first member emits its leading block comments (the own-line
                // directive) BEFORE the `| ` so the directive stays own-line and
                // re-recognizes on pass 2; emitting it after `| ` (the general path below)
                // relocates it to trail the pipe (`| /* prettier-ignore */`), flipping it
                // trailing and losing the freeze next pass. The own-line block's own
                // hardline forces the group broken, so the `if_break` `| ` appears.
                let first_frozen =
                    freeze_first || self.member_gap_frozen(union.span.start, type_start);
                if has_comments && first_frozen {
                    parts.push(
                        self.build_member_leading_block_comments(union.span.start, type_start),
                    );
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
                if has_comments && !first_frozen {
                    parts.push(d.align(
                        2,
                        self.build_member_leading_block_comments(union.span.start, type_start),
                    ));
                }
            }

            // Apply Prettier's per-member `align(2, …)` offset (a sub-tab alignment —
            // see `build_union_member_offset_doc`). The first member's leading run takes
            // that offset separately, above: the run is aligned, never this call's
            // result, so the object-literal and default-paren members that supply their
            // own indent keep declining it.
            //
            // Rule A first-member freeze: emit `types[0]` verbatim (paren-transparent)
            // instead of reformatting it. Same offset shape, so it aligns with the
            // reformatted siblings in the broken layout.
            // First member frozen via a leading-run directive (`freeze_first`, before
            // `span.start`) OR an own-line directive in the in-span leading gap after the
            // `|` (`| /* c */⏎// prettier-ignore⏎member`); later members were already
            // handled by the offset builder's callers.
            let frozen_first_member =
                i == 0 && (freeze_first || self.member_gap_frozen(union.span.start, type_start));
            if frozen_first_member {
                parts.push(self.build_frozen_union_member_offset_doc(t, member_parens));
            } else {
                parts.push(self.build_union_member_offset_doc(t, member_parens));
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
        // A multi-line frozen first member forces the broken one-member-per-line layout
        // (Rule A must-break): the frozen slice is a `will_break`-opaque verbatim span,
        // so the break is forced explicitly here rather than propagating from the slice.
        // A single-line frozen member keeps the width-decided layout.
        if freeze_first_multiline {
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
        // routed here, plus a leading-run own-line directive before the union). Recomputed
        // rather than threaded — gated on `has_format_ignore`, so it costs nothing in the
        // common case. The `Whole` case never reaches here (returned early in
        // `build_union_type_doc`).
        let freeze_first = matches!(
            self.union_leading_run_freeze(union),
            Some(LeadingRunFreeze::FirstMember { .. })
        );

        for (i, t) in union.types.iter().enumerate() {
            let type_start = t.span().start;
            let type_end = t.span().end;

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
                self.build_leading_comments_multiline(union.span.start, type_start, None)
            } else {
                DocBuf::new()
            };

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
            let frozen_member = if i == 0 {
                freeze_first || self.member_gap_frozen(union.span.start, type_start)
            } else {
                self.member_gap_frozen(union.types[i - 1].span().end, type_start)
            };

            let stripped_paren_leading = if i > 0 && !frozen_member {
                self.stripped_redundant_paren_member_leading_run(t)
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

                    // Comments after the pipe lead this member. Line comments (and
                    // own-line block comments) go on their own line BEFORE the `| `
                    // separator so the pipe stays attached to the type
                    // (`| A\n// c\n| B`). Inline block comments stay after `| `
                    // (`| /* c */ B`). Prettier instead relocates such comments to
                    // trail the previous member — see
                    // union_infix_pipe_line_comment_prettier_divergence.
                    let after_pipe = pipe_pos + 1;
                    let own_line: CommentVec<'_> =
                        comments_to_emit_in_range(self.comments, after_pipe, type_start)
                            .filter(|c| !self.comment_hugs_next(c, type_start))
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
                        let Some(next) = own_line.get(j + 1) else {
                            // The last comment always breaks: the filter above routed
                            // every member-hugging block onto the post-`| ` path, so
                            // whatever is left cannot hug. No blank line is emitted
                            // toward the member (see the blank-line note above).
                            parts.push(d.hardline());
                            continue;
                        };
                        // A block the author glued to the next comment leads it inline,
                        // matching prettier's leading-comment rule. This run brackets the
                        // `| ` separator and has its own blank-line policy, so it can't use
                        // `push_leading_comment_run` — but it shares the rule.
                        if self.comment_hugs_next(comment, next.span.start) {
                            parts.push(d.text(" "));
                            continue;
                        }
                        if self.has_blank_line_between(comment.span.end, next.span.start) {
                            parts.push(d.literalline());
                        }
                        parts.push(d.hardline());
                    }
                    self.push_own_line_comment_run(&mut parts, &stripped_paren_leading);
                    parts.push(d.text("| "));
                    for comment in comments_to_emit_in_range(self.comments, after_pipe, type_start)
                    {
                        if self.comment_hugs_next(comment, type_start) {
                            parts.push(self.build_comment_doc(comment));
                            parts.push(d.text(" "));
                        }
                    }
                } else {
                    // No pipe found, just add separator
                    parts.push(d.hardline());
                    self.push_own_line_comment_run(&mut parts, &stripped_paren_leading);
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
                // member inline (`| b /* t */`) — a type position, so `defer = false`. A
                // no-op for the pure-line / mixed cases (no comment in the trailing gap).
                parts.push(self.with_stripped_paren_trailing(member_doc, t, inner, false));
            } else if let TSType::Parenthesized(p) = t
                && let TSType::Union(inner_union) = p.type_annotation
                && self.paren_has_leading_line_comment(p)
            {
                // `first_leading` is non-empty only for the first member (see its
                // declaration); a later member's leading comments were emitted on their
                // own line above, so this extends nothing there.
                parts.extend(first_leading);
                parts.push(d.align(
                    2,
                    self.build_parenthesized_union_doc(inner_union, Some(p), true),
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
                parts.push(self.build_union_member_offset_doc(t, member_parens));
            }

            // Trailing comments on last type
            if i == union.types.len() - 1 {
                for comment in comments_to_emit_in_range(self.comments, type_end, union.span.end) {
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
    /// Requires the brace-member shape (`union_has_brace_member` — a
    /// `TypeLiteral`/`Mapped`; excludes the `Promise<…> | null` `TSTypeReference`
    /// print-width family). A comment prettier's `shouldHugUnionType` would bail on —
    /// between two members, or in the operator→union gap `[gap_start, gap_end]` —
    /// disqualifies the hug; an *inside-object* comment (`{ /* c */ … }`) does not.
    /// `gap_start`/`gap_end` bound the source between the `=>`/`:` and the union.
    pub(crate) fn union_return_hugs(
        &self,
        value_type: &TSType<'_>,
        gap_start: u32,
        gap_end: u32,
    ) -> bool {
        // The `:`→union gap is this site's own question; the hug itself is not
        // re-derived here but delegated (via `type_arg_union_prints_hugged`) to
        // [`Self::union_prints_hugged`]. This gate used to spell out its own subset of
        // that predicate's comment checks and missed the leading `|`→first-member line
        // comment, so a comment that made the printer decline the hug still read as
        // "hug" here and kept `: ` glued while the members exploded below it.
        self.type_arg_union_prints_hugged(value_type)
            && !self.has_comments_to_emit_between(gap_start, gap_end)
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
    /// (`build_type_alias_doc`, which decides whether to break after `=`). Asking the
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
    ///   first member, not *between* two. A **block** there stays hugged and inline
    ///   (`/* c */ { a: 1 } | null`), matching prettier.
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
        if !union_hug_shape(union) {
            return false;
        }
        // Zero-comment fast path — an **on-page** question, since it short-circuits the
        // comment gates below.
        if !self.has_comments_on_page_between(union.span.start, union.span.end) {
            return true;
        }
        !self.union_has_comments_between_members(union)
            && !union.types.first().is_some_and(|first| {
                self.has_line_comments_between(union.span.start, first.span().start)
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
        // means every `comments_to_emit_in_range` below is empty — skip the N-1 scans.
        if !self.has_comments_to_emit_between(intersection.span.start, intersection.span.end) {
            return false;
        }
        intersection.types.windows(2).any(|pair| {
            let (prev_end, next_start) = (pair[0].span().end, pair[1].span().start);
            self.comments_on_page_between(prev_end, next_start)
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

        // Format-ignore leading run (Rule A), symmetric with `build_union_type_doc`: a
        // same-line glued directive freezes the whole intersection; an own-line directive
        // freezes only the first member. (There is no whole-intersection arm for the
        // own-line case — the intersection first member behaves like every other honored
        // list position.) Gated on the document-level flag; the in-span `has_comments`
        // gate below cannot see an out-of-span directive.
        let first_inner = intersection
            .types
            .first()
            .map(|t| unwrap_parenthesized(t).span());
        let leading_freeze = self.leading_run_freeze(intersection.span.start, first_inner);
        if let Some(LeadingRunFreeze::Whole) = leading_freeze {
            return self.raw_source_range(intersection.span.start, intersection.span.end);
        }
        let (freeze_first, freeze_first_multiline) = match leading_freeze {
            Some(LeadingRunFreeze::FirstMember { multiline }) => (true, multiline),
            _ => (false, false),
        };

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

        // A single-member intersection collapses to its member — see the matching
        // note in `build_union_type_doc`. The lone member needs no precedence
        // parens (the parent context supplies any), while comment-aware paths
        // below still preserve comments around the `&`/parens.
        let member_parens = union_member_parens(intersection.types.len());

        // Hoist leading line comments inside the first member's stripped parens
        // OUT of the intersection (e.g., `(// c\n a) & b` → `// c\n a & b`, and the
        // double-nested `((// c\n a)) & b` the same way). The comment goes on its own
        // line BEFORE the intersection so the intersection content itself can still fit
        // inline. The deep window scans the whole stripped shell, not just the outer
        // paren's own gap, so a comment nested one paren deeper still hoists.
        if has_comments && let Some(first_member) = intersection.types.first() {
            let first_paren_leading = self.intersection_first_member_hoist_comments(first_member);
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
                    self.build_intersection_type_doc_with_first_paren_leading_stripped(intersection)
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
                // body. The line-comment layout already self-indents per member, and an
                // `own_line` caller (tuple element) already indents the whole element, so
                // both skip the extra level — adding it there over-indents the
                // continuation past the reparsed bare form (a non-idempotency).
                return if line_comment_layout || own_line {
                    body
                } else {
                    d.indent(body)
                };
            }
        }

        if has_comments && self.intersection_needs_line_comment_layout(intersection) {
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
        if has_comments
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
        // via a leading-run directive (`freeze_first`) OR an own-line directive in the
        // in-span leading gap after a leading `&`.
        if freeze_first || self.member_gap_frozen(intersection.span.start, first_type_start) {
            first_parts.push(self.build_frozen_member_doc(first_type, member_parens));
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
            member.extend(self.build_intersection_member_body_doc(intersection, i, has_comments));
            if indent_member {
                parts.push(d.indent(d.concat(&member)));
            } else {
                parts.extend(member);
            }
        }

        // A multi-line frozen first member forces the broken layout (Rule A must-break):
        // the frozen slice is a `will_break`-opaque verbatim span, so force it here. No
        // fixture reaches this for an intersection (every frozen first member is
        // single-line), but it keeps the union / intersection must-break rule symmetric.
        if freeze_first_multiline {
            d.group_break(d.concat(&parts))
        } else if needs_group {
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

        // Rule A first-member freeze in the forced-multiline path (recomputed, gated on
        // `has_format_ignore`; the `Whole` case is returned early upstream). The hoist
        // path (`strip_first_paren_leading`) already relocates the first member's inner
        // line comment, so a freeze there would double-print it — the hoist wins.
        let freeze_first = !strip_first_paren_leading
            && matches!(
                self.leading_run_freeze(
                    intersection.span.start,
                    intersection
                        .types
                        .first()
                        .map(|t| unwrap_parenthesized(t).span()),
                ),
                Some(LeadingRunFreeze::FirstMember { .. })
            );

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
        let first_doc = if freeze_first
            || (!strip_first_paren_leading
                && self.member_gap_frozen(intersection.span.start, first.span().start))
        {
            self.build_frozen_member_doc(first, member_parens)
        } else if strip_first_paren_leading && matches!(first, TSType::Parenthesized(_)) {
            self.build_intersection_first_member_stripped(first, member_parens)
        } else {
            self.build_intersection_line_comment_member_doc(first, member_parens)
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
                Some(amp_pos) => comments_to_emit_in_range(self.comments, amp_pos + 1, cur_start)
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
                && self
                    .comments_on_page_between(prev_end, cur_start)
                    .any(|c| self.comment_isolated_from_neighbors(prev_end, c, cur_start))
            {
                should_break = true;
            }

            // A same-line block comment authored *before* the `&` trails the previous
            // member and stays on its side of the operator (`prev /* b */ &`) — matching
            // the no-comment loop and Prettier. Emit those first, on the previous
            // member's line (indent-agnostic — no preceding newline), before the `&`.
            if let Some(amp_pos) = amp {
                for comment in comments_to_emit_in_range(self.comments, prev_end, amp_pos)
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
                for comment in comments_to_emit_in_range(self.comments, prev_end, amp_pos) {
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
                for comment in comments_to_emit_in_range(self.comments, amp_pos + 1, cur_start)
                    .filter(|c| self.is_same_line(amp_pos, c.span.start))
                {
                    unit.push(d.text(" "));
                    unit.push(self.build_comment_doc(comment));
                }
            }
            if should_break {
                unit.push(d.hardline());
                self.push_leading_comment_run(
                    &mut unit,
                    own_line_leading.iter().copied(),
                    cur_start,
                    LeadingGlue::Adjacent,
                    d.empty(),
                );
            } else {
                unit.push(d.text(" "));
            }
            // Rule A between-members freeze: an own-line directive in this member's gap
            // freezes the member (paren-transparent). The directive is emitted by the
            // separator / leading-comment machinery above; only the member DOC is
            // replaced. Intersection members take no `align(2)` offset, so the bare
            // paren-transparent doc matches a reformatted sibling.
            if self.member_gap_frozen(prev_end, cur_start) {
                unit.push(self.build_frozen_member_doc(cur, member_parens));
            } else {
                unit.push(self.build_intersection_line_comment_member_doc(cur, member_parens));
            }
            if is_last {
                for comment in
                    comments_to_emit_in_range(self.comments, cur.span().end, intersection.span.end)
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
    ///   (which re-attaches any trailing comment) — mirroring the bug188 keyword→value
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
        first_member: &TSType<'_>,
    ) -> CommentVec<'_> {
        if !matches!(first_member, TSType::Parenthesized(_)) {
            return smallvec![];
        }
        let inner = unwrap_parenthesized(first_member);
        if matches!(inner, TSType::Union(_)) {
            return comments_to_emit_in_range(
                self.comments,
                first_member.span().start + 1,
                inner.span().start,
            )
            .filter(|c| !c.is_block)
            .collect();
        }
        // Bare inner: hoist the full leading run (block + line), but only when a leading
        // line comment forces the hang — a block-only leading gap keeps its block inline
        // and is already idempotent. Collect the run once and gate on it directly (the
        // hang trigger is a line comment in the run). The trailing comment is re-attached
        // by `build_hang_value_doc` in `build_intersection_first_member_stripped`, so
        // nothing is dropped.
        let lead: CommentVec<'_> = comments_to_emit_in_range(
            self.comments,
            first_member.span().start + 1,
            inner.span().start,
        )
        .collect();
        if lead.iter().any(|c| !c.is_block) {
            return lead;
        }
        smallvec![]
    }

    /// Build the first intersection member's type doc with its parenthesized leading
    /// line comment(s) excluded (the caller — the hoist path — emits them before the
    /// intersection). Every redundant paren layer is stripped (`unwrap_parenthesized`,
    /// so the double-nested `((a)) & b` reduces the same as `(a) & b`); precedence
    /// parens are then re-applied where the bare inner needs them (`(A | B) & C`) and
    /// dropped where redundant (`(a) & b` → `a & b`).
    fn build_intersection_first_member_stripped(
        &self,
        first_member: &TSType<'_>,
        member_parens: fn(&TSType<'_>) -> bool,
    ) -> DocId {
        let d = self.d();
        let inner = unwrap_parenthesized(first_member);
        if member_parens(inner) {
            // Re-wrap inner in parens (e.g., union in intersection: `(A | B) & C`).
            if let TSType::Union(union) = inner {
                // The hoisted leading line comment is emitted by the caller, so pass
                // `false` to keep `build_parenthesized_union_doc` block-comment-only.
                // The outermost paren bounds the block-comment scan, so a block comment
                // authored in the stripped shell (before the union) is still preserved.
                let paren = match first_member {
                    TSType::Parenthesized(p) => Some(p),
                    _ => None,
                };
                self.build_parenthesized_union_doc(union, paren, false)
            } else {
                // Matches the bare intersection-member parenthesization in
                // `build_intersection_member_type_doc`: no inner `d.indent`, the
                // level comes from the intersection printer's own `& `-line indent.
                // Thread any trailing comment lifted from the stripped shell through the
                // precedence re-wrap (an object-trailing intersection inner,
                // `(// c⏎ X & { … } /* t */) & B`) so it isn't dropped — the `)` the
                // re-wrap adds is not the stripped shell's, so the trailing gap comment
                // still needs re-attaching.
                let rewrapped = d.concat(&[d.text("("), self.build_type_doc(inner), d.text(")")]);
                self.with_stripped_paren_trailing(rewrapped, first_member, inner, false)
            }
        } else {
            // Re-attach any trailing comment lifted from a stripped shell (`(A /* t */)`);
            // type position, so a trailing block trails the member inline (defer = false).
            // A no-op when `first_member` was not a stripped shell or held no trailing
            // comment — leaving the bare-inner layout unchanged.
            self.build_hang_value_doc(first_member, inner, false)
        }
    }

    /// Build an intersection type's doc with the first member's stripped-paren
    /// leading line comments excluded from the output — the compact **inline** form
    /// (`a & b & c`) used by the hoisting path in `build_intersection_type_doc` when the
    /// intersection has no *isolated* between-member comment. The caller emits the
    /// hoisted comment before this doc; the first member's parens are stripped from
    /// `intersection.types[0]`.
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
    ) -> DocId {
        let d = self.d();
        let member_parens = union_member_parens(intersection.types.len());
        let first_doc =
            self.build_intersection_first_member_stripped(&intersection.types[0], member_parens);

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
            parts.push(self.build_intersection_member_type_doc(t, member_parens));
        }
        d.concat(&parts)
    }

    /// Build the body of an intersection continuation member (everything except separator).
    ///
    /// Returns: leading comments + type doc + trailing comments/`&` separator.
    /// Used by both the normal and expanding-first-type paths.
    ///
    /// `has_comments` is the caller's whole-intersection window answer: `false` proves
    /// both gaps around this member are bare, so neither is searched and the `&` byte
    /// scan that would bound them never runs.
    fn build_intersection_member_body_doc(
        &self,
        intersection: &TSIntersectionType<'_>,
        i: usize,
        has_comments: bool,
    ) -> DocBuf {
        let t = &intersection.types[i];
        let type_start = t.span().start;
        let type_end = t.span().end;
        let is_last = i == intersection.types.len() - 1;
        let mut parts = DocBuf::new();

        // Leading block comments (after the `&` separator)
        if has_comments {
            let prev_type_end = intersection.types[i - 1].span().end;
            self.push_post_separator_block_comments(&mut parts, prev_type_end, type_start, b'&');
        }

        parts.push(
            self.build_intersection_member_type_doc(t, type_needs_parens_in_union_or_intersection),
        );

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

        parts
    }
}
