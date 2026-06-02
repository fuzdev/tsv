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
    type_needs_parens_in_union_or_intersection, type_never_needs_parens,
};
use super::{CommentFilter, CommentSpacing, Printer};
use crate::ast::internal::{self, TSIntersectionType, TSParenthesizedType, TSType, TSUnionType};
use crate::printer::layout::hang_after_operator;
use tsv_lang::doc::arena::DocId;

/// Member-parens predicate for a union/intersection with `member_count` members.
/// A single-member union/intersection collapses to its member (Prettier
/// postprocess), so the lone member needs no precedence parens of its own;
/// 2+ members use the normal `|`/`&` precedence rule.
fn union_member_parens(member_count: usize) -> fn(&TSType) -> bool {
    if member_count == 1 {
        type_never_needs_parens
    } else {
        type_needs_parens_in_union_or_intersection
    }
}

impl<'a> Printer<'a> {
    //
    // Union Types
    //

    /// Build a Doc for a union type: `A | B | C` or `| A\n| B\n| C`
    ///
    /// When flat: `A | B | C`
    /// When broken: each type on its own line with leading `| `
    ///
    /// When `wrap_in_group` is true (default), wraps the union in its own group
    /// so that it makes independent breaking decisions. This is appropriate for
    /// most contexts (e.g., type parameter constraints should stay flat even
    /// when the parameter list breaks).
    ///
    /// When `wrap_in_group` is false, the union inherits breaking from its parent
    /// group. Use this for type alias values and index signature values where
    /// the union should break together with the parent context.
    pub(in crate::printer) fn build_union_type_doc(
        &self,
        union: &TSUnionType,
        wrap_in_group: bool,
    ) -> DocId {
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

        // Check for any comments on or between union members (disqualifies hugging).
        // Prettier's `hasComment(node)` includes attached trailing comments, which in
        // our detached model appear between consecutive member spans.
        let has_comments_on_or_between_members = union
            .types
            .iter()
            .any(|t| self.has_comments_between(t.span().start, t.span().end))
            || self.union_has_comments_between_members(union);

        // Prettier's shouldHugUnionType: when one member is object-like and the
        // rest are void types (null, void), format as inline `A | B | C` where
        // the object type handles its own expansion.
        // Example: `{ name: string; value: number } | null` stays hugged.
        if !has_comments_on_or_between_members && should_hug_union_type(union) {
            let mut parts = Vec::new();
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
            || self.union_has_line_comments_between_members(union)
            || has_paren_inner_leading_line_comments
        {
            return self.build_union_type_doc_with_line_comments(union);
        }

        // Build parts: each type prefixed conditionally with `| ` or nothing
        // Flat: T1 | T2 | T3
        // Break: | T1
        //        | T2
        //        | T3
        let mut parts = Vec::new();

        for (i, t) in union.types.iter().enumerate() {
            let type_start = t.span().start;
            let type_end = t.span().end;

            if i > 0 {
                // Between types: newline + "| " when broken, " | " when flat
                // Use if_break with line() instead of hardline() to avoid triggering will_break
                parts.push(d.if_break(d.concat(&[d.line(), d.text("| ")]), d.text(" | ")));

                // Add leading block comments for this type (after the `|` separator)
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
                parts.push(self.build_comments_between_filtered(
                    union.span.start,
                    type_start,
                    CommentSpacing::Trailing,
                    CommentFilter::BlockOnly,
                ));
            }

            // Special handling for object type literals: use aligned indentation
            if let TSType::TypeLiteral(obj) = t {
                parts.push(self.build_union_member_object_literal_doc(obj));
            } else {
                parts.push(self.build_type_doc_maybe_parens(t, member_parens));
            }

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

        if wrap_in_group {
            // Independent breaking decision
            d.group(d.concat(&parts))
        } else {
            // Inherit breaking from parent group
            d.concat(&parts)
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
    pub(in crate::printer) fn build_union_hanging_indent_doc(&self, ty: &TSType) -> Option<DocId> {
        let TSType::Union(union) = ty else {
            return None;
        };
        if should_hug_union_type(union) {
            return None;
        }
        // `wrap_in_group = false`: the union inherits breaking from the hanging
        // group so the after-keyword line and the member separators break together.
        let union_doc = self.build_union_type_doc(union, false);
        Some(hang_after_operator(self.d(), union_doc))
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
    fn build_union_type_doc_with_line_comments(&self, union: &TSUnionType) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        let member_parens = union_member_parens(union.types.len());

        for (i, t) in union.types.iter().enumerate() {
            let type_start = t.span().start;
            let type_end = t.span().end;

            // For non-first members, detect leading line comments inside the
            // parens of a TSParenthesizedType wrapper. Prettier relocates these
            // to trail the previous member (e.g., `a | (// c\n b)` becomes
            // `| a // c\n | b`). We extract them so they can be emitted before
            // the `| ` separator and skipped when building the member's type doc.
            let relocated_paren_leading: Vec<&internal::Comment> = if i > 0
                && let TSType::Parenthesized(p) = t
            {
                self.paren_leading_line_comments(p)
            } else {
                Vec::new()
            };

            if i > 0 {
                // Get previous type end and find the pipe position
                let prev_type_end = union.types[i - 1].span().end;

                // Collect comments between previous type and this type's pipe
                if let Some(pipe_pos) =
                    find_separator_position(self.source, prev_type_end, type_start, b'|')
                {
                    // Comments before the pipe (trailing on previous type's line or on own lines)
                    parts.extend(self.build_trailing_comments_multiline(prev_type_end, pipe_pos));

                    // Relocated paren leading line comments: trail prev member
                    for comment in &relocated_paren_leading {
                        parts.push(self.build_trailing_line_comment_doc(comment));
                    }

                    parts.push(d.hardline());

                    // Comments after the pipe lead this member. Line comments (and
                    // own-line block comments) go on their own line BEFORE the `| `
                    // separator so the pipe stays attached to the type
                    // (`| A\n// c\n| B`). Inline block comments stay after `| `
                    // (`| /* c */ B`). Prettier instead relocates such comments to
                    // trail the previous member — see
                    // union_infix_pipe_line_comment_prettier_divergence.
                    let after_pipe = pipe_pos + 1;
                    for comment in comments_in_range(self.comments, after_pipe, type_start) {
                        if !(comment.is_block && self.is_same_line(comment.span.end, type_start)) {
                            parts.push(self.build_comment_doc(comment));
                            parts.push(d.hardline());
                        }
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

            // Add the type. When we relocated leading line comments from inside
            // a TSParenthesizedType wrapper, build the inner type directly so
            // the relocated comments aren't emitted again. Re-wrap in parens
            // when precedence demands it.
            if !relocated_paren_leading.is_empty()
                && let TSType::Parenthesized(p) = t
            {
                if type_needs_parens_in_union_or_intersection(&p.type_annotation) {
                    parts.push(d.text("("));
                    parts.push(self.build_type_doc(&p.type_annotation));
                    parts.push(d.text(")"));
                } else {
                    parts.push(self.build_type_doc(&p.type_annotation));
                }
            } else if let TSType::TypeLiteral(obj) = t {
                parts.push(self.build_union_member_object_literal_doc(obj));
            } else {
                parts.push(self.build_type_doc_maybe_parens(t, member_parens));
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
    fn union_has_comments_between_members(&self, union: &TSUnionType) -> bool {
        union
            .types
            .windows(2)
            .any(|pair| self.has_comments_between(pair[0].span().end, pair[1].span().start))
    }

    /// Check if a union type has line comments between any consecutive members.
    ///
    /// Used by callers (e.g., mapped types) to decide whether the union needs
    /// extra indentation wrapping, and internally to force multiline formatting.
    pub(crate) fn union_has_line_comments_between_members(&self, union: &TSUnionType) -> bool {
        union
            .types
            .windows(2)
            .any(|pair| self.has_line_comments_between(pair[0].span().end, pair[1].span().start))
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
        intersection: &TSIntersectionType,
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
                let mut parts = Vec::new();
                for comment in &first_paren_leading {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.hardline());
                }
                parts.push(inner);
                return d.concat(&parts);
            }
        }

        // Check for line comments between intersection members (force multiline)
        // Only check the gaps between member types, not inside member types
        let has_line_comments_between_members = intersection
            .types
            .windows(2)
            .any(|pair| self.has_line_comments_between(pair[0].span().end, pair[1].span().start));
        if has_line_comments_between_members {
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
        let mut first_parts = Vec::new();
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
                let mut member = vec![sep];
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
        let mut continuation_parts = Vec::new();

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
        intersection: &TSIntersectionType,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
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
                    let comments_after_amp: Vec<_> =
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

                    // Leading comments on their own line (come after hardline)
                    for comment in comments_after_amp
                        .iter()
                        .filter(|c| !self.is_same_line(amp_pos, c.span.start))
                    {
                        parts.push(self.build_comment_doc(comment));
                        if comment.is_block {
                            parts.push(d.text(" "));
                        } else {
                            parts.push(d.hardline());
                        }
                    }
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

            // Add the type
            parts.push(self.build_type_doc_maybe_parens(t, member_parens));

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
        intersection: &TSIntersectionType,
        first_paren: &TSParenthesizedType,
    ) -> DocId {
        let d = self.d();
        let member_parens = union_member_parens(intersection.types.len());
        let inner = first_paren.type_annotation.as_ref();
        let first_doc = if member_parens(inner) {
            // Re-wrap inner in parens (e.g., union in intersection: `(A | B) & C`).
            if let TSType::Union(union) = inner {
                self.build_parenthesized_union_doc(union)
            } else {
                d.concat(&[
                    d.text("("),
                    d.align_spaces(2, d.indent(self.build_type_doc(inner))),
                    d.text(")"),
                ])
            }
        } else {
            self.build_type_doc(inner)
        };

        // Build the rest as `first & second & third...` inline (the hoisted
        // comment forces a hardline before; we want the intersection itself
        // to remain compact when possible).
        let mut parts = vec![first_doc];
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
        intersection: &TSIntersectionType,
        i: usize,
    ) -> Vec<DocId> {
        let t = &intersection.types[i];
        let type_start = t.span().start;
        let type_end = t.span().end;
        let is_last = i == intersection.types.len() - 1;
        let mut parts = Vec::new();

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
