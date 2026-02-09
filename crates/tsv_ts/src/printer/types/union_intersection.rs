// Union and intersection type printing for TypeScript
//
// Handles:
// - Union types: `A | B | C`
// - Intersection types: `A & B & C`
// - Comment handling between type members

use super::super::comments_in_range;
use super::helpers::{
    find_separator_position, intersection_has_huggable_last_type,
    type_needs_parens_in_intersection, type_needs_parens_in_union,
};
use super::{CommentFilter, CommentSpacing, Printer};
use crate::ast::internal::{TSIntersectionType, TSType, TSUnionType};
use tsv_lang::doc::arena::DocId;

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

        // Check for line comments between union members (force multiline)
        // Only check the gaps between member types, not inside member types
        let has_line_comments_between_members = union
            .types
            .windows(2)
            .any(|pair| self.has_line_comments_between(pair[0].span().end, pair[1].span().start));
        if has_line_comments_between_members {
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
            }

            // Special handling for object type literals: use aligned indentation
            if let TSType::TypeLiteral(obj) = t {
                parts.push(self.build_union_member_object_literal_doc(obj));
            } else {
                parts.push(self.build_type_doc_maybe_parens(t, type_needs_parens_in_union));
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

        for (i, t) in union.types.iter().enumerate() {
            let type_start = t.span().start;
            let type_end = t.span().end;

            if i > 0 {
                // Get previous type end and find the pipe position
                let prev_type_end = union.types[i - 1].span().end;

                // Collect comments between previous type and this type's pipe
                if let Some(pipe_pos) =
                    find_separator_position(self.source, prev_type_end, type_start, b'|')
                {
                    // Comments before the pipe (trailing on previous type's line or on own lines)
                    parts.extend(self.build_trailing_comments_multiline(prev_type_end, pipe_pos));

                    // Newline before `| `
                    parts.push(d.hardline());
                    parts.push(d.text("| "));

                    // Comments after the pipe (leading on this type)
                    parts.extend(self.build_leading_comments_multiline(pipe_pos + 1, type_start));
                } else {
                    // No pipe found, just add separator
                    parts.push(d.hardline());
                    parts.push(d.text("| "));
                }
            } else {
                // First type: always has `| ` prefix when multiline
                parts.push(d.text("| "));
            }

            // Add the type
            if let TSType::TypeLiteral(obj) = t {
                parts.push(self.build_union_member_object_literal_doc(obj));
            } else {
                parts.push(self.build_type_doc_maybe_parens(t, type_needs_parens_in_union));
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

        // Check for line comments between intersection members (force multiline)
        // Only check the gaps between member types, not inside member types
        let has_line_comments_between_members = intersection
            .types
            .windows(2)
            .any(|pair| self.has_line_comments_between(pair[0].span().end, pair[1].span().start));
        if has_line_comments_between_members {
            return self.build_intersection_type_doc_with_line_comments(intersection);
        }

        // For intersection types, prettier uses trailing `&` when breaking,
        // with continuation types indented:
        // Flat: A & B & C
        // Break: A &
        //            B &
        //            C
        //
        // Special case: when the last type is a TypeLiteral (object type), use a space
        // instead of line() to keep `& {` hugged together. The TypeLiteral handles its
        // own expansion independently.
        let last_idx = intersection.types.len() - 1;
        let last_is_huggable = intersection_has_huggable_last_type(intersection);

        // Build first type separately (not indented)
        let mut first_parts = Vec::new();
        let first_type = &intersection.types[0];
        let first_type_end = first_type.span().end;

        first_parts
            .push(self.build_type_doc_maybe_parens(first_type, type_needs_parens_in_intersection));

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

        // Build continuation types (indented when breaking)
        let mut continuation_parts = Vec::new();

        for (i, t) in intersection.types.iter().enumerate().skip(1) {
            let type_start = t.span().start;
            let type_end = t.span().end;
            let is_last = i == last_idx;

            // After separator: line when broken, space when flat
            // But for huggable last types (TypeLiteral), always use space to keep `& {` hugged
            if is_last && last_is_huggable {
                continuation_parts.push(d.text(" "));
            } else {
                continuation_parts.push(d.line());
            }

            // Add leading block comments for this type (after the `&` separator)
            let prev_type_end = intersection.types[i - 1].span().end;
            if let Some(amp_pos) =
                find_separator_position(self.source, prev_type_end, type_start, b'&')
            {
                continuation_parts.push(self.build_comments_between_filtered(
                    amp_pos + 1,
                    type_start,
                    CommentSpacing::Trailing,
                    CommentFilter::BlockOnly,
                ));
            }

            continuation_parts
                .push(self.build_type_doc_maybe_parens(t, type_needs_parens_in_intersection));

            // Add trailing block comments after this type (before the next `&` separator)
            if i + 1 < intersection.types.len() {
                let next_type_start = intersection.types[i + 1].span().start;
                if let Some(amp_pos) =
                    find_separator_position(self.source, type_end, next_type_start, b'&')
                {
                    continuation_parts.push(self.build_comments_between_filtered(
                        type_end,
                        amp_pos,
                        CommentSpacing::Leading,
                        CommentFilter::BlockOnly,
                    ));
                }
                continuation_parts.push(d.text(" &"));
            } else {
                // Last type - include all trailing comments up to intersection span end
                continuation_parts.push(self.build_comments_between_filtered(
                    type_end,
                    intersection.span.end,
                    CommentSpacing::Leading,
                    CommentFilter::BlockOnly,
                ));
            }
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

            // Add the type
            parts.push(self.build_type_doc_maybe_parens(t, type_needs_parens_in_intersection));

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
}
