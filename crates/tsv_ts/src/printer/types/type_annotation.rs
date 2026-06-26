// Type annotation printing for TypeScript
//
// Handles printing of type annotations (`: Type`) with various contexts:
// - Simple type annotations
// - Width-aware wrapping for type arguments
// - Return type annotations

use super::helpers::{
    find_separator_position, immediate_union_paren, intersection_has_expanding_first_type,
    intersection_has_huggable_last_type, should_hug_union_type,
    type_args_should_wrap_for_return_type, type_needs_parens_in_union_or_intersection,
    unwrap_parenthesized,
};
use super::{CommentFilter, CommentSpacing, Printer};
use crate::ast::internal::{self, TSType};
use crate::printer::layout::hang_after_operator;
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build a Doc for a type annotation (e.g., `: number`)
    ///
    /// Handles comments between the colon and the type. For a line comment
    /// between `:` and the type, the comment stays inline after `:` with a
    /// hardline before the type (`: // c\n T`) so the line comment doesn't
    /// swallow what follows. Union types additionally INDENT the type; other
    /// types are not indented.
    pub(in crate::printer) fn build_type_annotation_doc(
        &self,
        annotation: &internal::TSTypeAnnotation<'_>,
    ) -> DocId {
        let d = self.d();
        // Check for comments between `:` and the type
        let colon_end = annotation.span.start + 1; // After the `:`
        let type_start = annotation.type_annotation.span().start;

        // Check if there's a line comment between : and the type
        if self.has_line_comments_between(colon_end, type_start) {
            // Uniform forced-continuation indent (`build_continuation_indent`): the
            // first comment trails `:` on its line, then the remaining comments and the
            // type drop one indent level so the continuation reads as part of this
            // member, not a sibling. Each line comment terminates at end-of-line —
            // otherwise a following comment (or the type) is swallowed into its text
            // (`// a // b` reparses as one comment: a content loss). Uniform across
            // union, intersection, and simple types — see conformance_prettier.md
            // §Uniform forced-continuation indent. (Prettier indents only the union
            // here and leaves intersection/simple flush, so this diverges for those.)
            let type_doc = self.build_type_doc(annotation.type_annotation);
            d.concat(&[
                d.text(":"),
                self.build_continuation_indent(colon_end, type_start, type_doc),
            ])
        } else {
            // Handle unions/intersections with width-based breaking
            // Short: `param: Type1 | Type2`
            // Long: `param:\n\t| Type1\n\t| Type2`
            //
            // This pattern matches index signature type annotation handling.
            // For unions/intersections, wrap in group + indent + line so they break after `:`
            // and inherit breaking from this context's group. Redundant comment-free
            // parens are stripped first so `(A | B)` / `(A & B)` get the bare layout
            // (prettier strips them too); other parens keep the `_` fall-through.
            match self.unwrap_redundant_parens(annotation.type_annotation) {
                TSType::Union(u) => {
                    let type_doc = self.build_union_type_doc(u, false);
                    // Extract comments between `:` and the union type (e.g., `: /* c */ A | B`)
                    let comments_doc = self.build_comments_between(
                        colon_end,
                        type_start,
                        CommentSpacing::Trailing,
                    );
                    d.concat(&[
                        d.text(":"),
                        hang_after_operator(d, d.concat(&[comments_doc, type_doc])),
                    ])
                }
                TSType::Intersection(i) => {
                    // Build intersection with proper indentation for type annotation context:
                    // `: FirstType &` stays on the same line, continuation types are indented
                    // Extract comments between `:` and the intersection first
                    self.build_intersection_type_annotation_doc(i, colon_end)
                }
                _ => {
                    // Block comments stay inline: `: /* comment */ Type`
                    let mut parts: DocBuf = smallvec![d.text(": ")];
                    parts.push(self.build_comments_between(
                        colon_end,
                        type_start,
                        CommentSpacing::Trailing,
                    ));
                    parts.push(self.build_type_doc(annotation.type_annotation));
                    d.concat(&parts)
                }
            }
        }
    }

    /// Build type annotation doc with width-aware type argument wrapping.
    ///
    /// For `TypeReference<Args>`, uses `build_type_arguments_doc_wrapping` so
    /// type arguments wrap at width boundary.
    ///
    /// For Union types, uses break-after-colon layout:
    /// ```text
    /// property:
    ///     | string
    ///     | number;
    /// ```
    ///
    /// For other types, delegates to `build_type_annotation_doc`.
    ///
    /// Returns doc starting with `: ` (the annotation prefix).
    pub(in crate::printer) fn build_type_annotation_doc_wrapping(
        &self,
        annotation: &internal::TSTypeAnnotation<'_>,
    ) -> DocId {
        self.build_type_annotation_doc_with_wrapping(annotation, true)
    }

    /// Build type annotation doc for function return types.
    ///
    /// For return types, we only use wrapping when type arguments would benefit from breaking:
    /// - Multiple type args (like `Result<A, B>`) - can break between args
    /// - Unions/intersections (like `Promise<A | B>`) - can break internally
    ///
    /// Simple cases like `Promise<void>` should NOT wrap - we want params to break first.
    pub(in crate::printer) fn build_type_annotation_doc_for_return_type(
        &self,
        annotation: &internal::TSTypeAnnotation<'_>,
    ) -> DocId {
        self.build_type_annotation_doc_with_wrapping(annotation, false)
    }

    /// Inner implementation for type annotation with wrapping support.
    ///
    /// When `always_wrap` is true, wraps any TypeReference with type args.
    /// When false, only wraps if type args would benefit from breaking.
    fn build_type_annotation_doc_with_wrapping(
        &self,
        annotation: &internal::TSTypeAnnotation<'_>,
        always_wrap: bool,
    ) -> DocId {
        let d = self.d();
        // First check for line comments between `:` and the type.
        // If there are comments, fall back to build_type_annotation_doc which handles them properly.
        let colon_end = annotation.span.start + 1; // After the `:`
        let type_start = annotation.type_annotation.span().start;
        if self.has_line_comments_between(colon_end, type_start) {
            return self.build_type_annotation_doc(annotation);
        }

        // Handle TypeReference with type arguments - use wrapping version when appropriate
        if let TSType::TypeReference(r) = annotation.type_annotation
            && let Some(type_args) = &r.type_arguments
            && (always_wrap || type_args_should_wrap_for_return_type(type_args))
        {
            // Extract comments between `:` and the type (e.g., `: /* c */ Promise<string>`)
            let comments_doc =
                self.build_comments_between(colon_end, type_start, CommentSpacing::Trailing);
            // Preserve comments between type name and type args: `Promise/* c */ <string>`
            let name_end = r.type_name.span().end;
            let ta_start = type_args.span.start;
            let name_ta_comments = self
                .build_name_to_type_params_comments_opt(
                    name_end,
                    ta_start,
                    CommentSpacing::Trailing,
                )
                .unwrap_or_else(|| d.empty());
            return d.concat(&[
                d.text(": "),
                comments_doc,
                self.build_entity_name_doc(&r.type_name),
                name_ta_comments,
                self.build_type_arguments_doc_wrapping(type_args),
            ]);
        }

        // Strip redundant comment-free parens around a union / intersection so a
        // `(A | B)` / `(A & B)` return type or member type gets the same break
        // layout as the bare form (prettier strips them too). Other parenthesized
        // types keep the existing fall-through below.
        let value_type = self.unwrap_redundant_parens(annotation.type_annotation);
        let value_type_start = value_type.span().start;

        // Handle Union types - break after colon with indent when long
        if let TSType::Union(u) = value_type {
            let type_doc = self.build_union_type_doc(u, false);

            // Extract comments between `:` and the union type (e.g., `: /* c */ A | B`)
            let comments_doc =
                self.build_comments_between(colon_end, value_type_start, CommentSpacing::Trailing);

            if should_hug_union_type(u) {
                // Hugged unions (e.g., `null | { ... }`) use conditional_group to bypass
                // the renderer's will_break check. The object type inside the union has
                // hardlines for multiline members, but those shouldn't force the type
                // annotation to break after `:`. The conditional_group calls fits()
                // directly, which correctly handles nested hardlines (returns true).
                // State 0: `: null | { ... }` (inline, no break after colon)
                // State 1: `:\n  null | { ... }` (break after colon, for very long names)
                let flat_state = d.concat(&[d.text(": "), comments_doc, type_doc]);
                let break_state = d.concat(&[
                    d.text(":"),
                    d.indent_line(d.concat(&[comments_doc, type_doc])),
                ]);
                return d.conditional_group(&[flat_state, break_state]);
            }

            let union_group = hang_after_operator(d, d.concat(&[comments_doc, type_doc]));
            return d.concat(&[d.text(":"), union_group]);
        }

        // Handle Intersection types - first member hugs `:`, continuations indented.
        if let TSType::Intersection(i) = value_type {
            return self.build_intersection_type_annotation_doc(i, colon_end);
        }

        self.build_type_annotation_doc(annotation)
    }

    /// Build intersection type annotation with proper indentation.
    ///
    /// Structure for class properties:
    /// ```text
    /// property: FirstType &
    ///     SecondType &
    ///     ThirdType;
    /// ```
    ///
    /// The first type stays on the same line as `:`, continuation types are indented.
    /// This differs from `build_intersection_type_doc` (in union_intersection.rs) which
    /// doesn't add internal indentation (expecting the parent context to provide it).
    /// Both functions share the same grouping rule: 2-type with a huggable/expanding
    /// boundary (TypeLiteral/MappedType at first or last position) skips the group;
    /// all other cases need one.
    ///
    /// Line comments between members are delegated to `build_intersection_type_doc`
    /// (which owns the multiline-with-comments layout) — the continuation loop here
    /// has no line-comment handling and would otherwise drop them.
    fn build_intersection_type_annotation_doc(
        &self,
        intersection: &internal::TSIntersectionType<'_>,
        colon_end: u32,
    ) -> DocId {
        let d = self.d();
        if intersection.types.is_empty() {
            return d.text(": ");
        }

        // Single type - just use the normal intersection doc
        // Extract comments between `:` and the type (e.g., `: & /* c */ A`)
        if intersection.types.len() == 1 {
            let first_type_start = intersection.types[0].span().start;
            let comments_doc =
                self.build_comments_between(colon_end, first_type_start, CommentSpacing::Trailing);
            return d.concat(&[
                d.text(": "),
                comments_doc,
                self.build_type_doc_with_wrapping_type_args(&intersection.types[0]),
            ]);
        }

        // Line comments between members force the multiline layout. Delegate to the
        // shared bare-intersection path (which handles them) instead of the
        // continuation loop below, which has no line-comment handling and would
        // silently drop the comments. Mirrors the type-alias layout: `: ` + the
        // huggable-aware `group(indent(...))` wrapper.
        let has_line_comments_between_members = intersection
            .types
            .windows(2)
            .any(|pair| self.has_line_comments_between(pair[0].span().end, pair[1].span().start));
        if has_line_comments_between_members {
            let first_type_start = intersection.types[0].span().start;
            let comments_doc =
                self.build_comments_between(colon_end, first_type_start, CommentSpacing::Trailing);
            let wrapped = self.intersection_hanging_with_indent(intersection);
            return d.concat(&[d.text(": "), comments_doc, wrapped]);
        }

        // Check for huggable boundary types (TypeLiteral/MappedType at first or last position)
        let last_is_huggable = intersection_has_huggable_last_type(intersection);
        let first_is_expanding = intersection_has_expanding_first_type(intersection);
        let is_huggable_pair =
            intersection.types.len() == 2 && (last_is_huggable || first_is_expanding);
        let last_idx = intersection.types.len() - 1;

        // Build first type (stays on same line as `:`)
        // Use wrapping type args so GenericType<...> can break at print width
        let first_type = &intersection.types[0];
        let first_type_doc = self.build_intersection_member_type_doc(first_type);

        // Extract comments between `:` and the first type (e.g., `: /* c */ A & B`)
        let first_type_start = first_type.span().start;
        let comments_doc =
            self.build_comments_between(colon_end, first_type_start, CommentSpacing::Trailing);
        let mut first_parts: DocBuf = smallvec![d.text(": "), comments_doc, first_type_doc];

        // Add trailing block comments after first type (before the `&`)
        let first_type_end = first_type.span().end;
        let second_type_start = intersection.types[1].span().start;
        if let Some(amp_pos) =
            find_separator_position(self.source, first_type_end, second_type_start, b'&')
        {
            first_parts.push(self.build_comments_between_filtered(
                first_type_end,
                amp_pos,
                CommentSpacing::Leading,
                CommentFilter::BlockOnly,
            ));
        }
        first_parts.push(d.text(" &"));

        // Build continuation types (indented when breaking)
        let mut continuation_parts: DocBuf = DocBuf::new();
        for (i, t) in intersection.types.iter().enumerate().skip(1) {
            let type_start = t.span().start;
            let type_end = t.span().end;
            let is_last = i == last_idx;

            // Space/line before this type
            // Huggable pair: always space (TypeLiteral handles its own expansion)
            // Multi-type with huggable last: space only for the last type
            if is_huggable_pair || (is_last && last_is_huggable) {
                continuation_parts.push(d.text(" "));
            } else {
                continuation_parts.push(d.line());
            }

            // Add leading block comments (after `&`)
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

            // The type itself (with wrapping type args so generics can break)
            continuation_parts.push(self.build_intersection_member_type_doc(t));

            // Trailing block comments and `&` separator (except for last type)
            if !is_last {
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
                // Last type - trailing comments
                continuation_parts.push(self.build_comments_between_filtered(
                    type_end,
                    intersection.span.end,
                    CommentSpacing::Leading,
                    CommentFilter::BlockOnly,
                ));
            }
        }

        // Combine: first_parts + indented continuation
        let mut parts = first_parts;
        if !continuation_parts.is_empty() {
            // Huggable pair: no indent, TypeLiteral handles its own expansion
            // All other cases: wrap continuation in indent
            if is_huggable_pair {
                parts.extend(continuation_parts);
            } else {
                parts.push(d.indent(d.concat(&continuation_parts)));
            }
        }

        // Huggable pair: no group needed, TypeLiteral expands itself.
        // All other cases: group controls line() flat/break behavior.
        if is_huggable_pair {
            d.concat(&parts)
        } else {
            d.group(d.concat(&parts))
        }
    }

    /// Build intersection member type with optional parens and wrapping type args.
    fn build_intersection_member_type_doc(&self, t: &TSType<'_>) -> DocId {
        let d = self.d();
        if type_needs_parens_in_union_or_intersection(t) {
            // Special case: parenthesized union type
            if let TSType::Union(union) = unwrap_parenthesized(t) {
                return self.build_parenthesized_union_doc(union, immediate_union_paren(t), false);
            }

            d.concat(&[
                d.text("("),
                self.build_type_doc_with_wrapping_type_args(t),
                d.text(")"),
            ])
        } else {
            self.build_type_doc_with_wrapping_type_args(t)
        }
    }
}
