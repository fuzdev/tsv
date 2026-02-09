// Type annotation printing for TypeScript
//
// Handles printing of type annotations (`: Type`) with various contexts:
// - Simple type annotations
// - Width-aware wrapping for type arguments
// - Return type annotations

use super::helpers::{
    find_separator_position, intersection_has_huggable_last_type,
    type_args_should_wrap_for_return_type, type_needs_parens_in_intersection,
};
use super::{CommentFilter, CommentSpacing, Printer};
use crate::ast::internal::{self, TSType};
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build a Doc for a type annotation (e.g., `: number`)
    ///
    /// Handles comments between the colon and the type.
    /// For simple types with line comments, the comment is moved to after the type.
    /// For union types, the comment stays before and the type is INDENTED.
    /// For intersection types, the comment stays before but the type is NOT indented.
    ///
    /// NOTE: For simple types with trailing comments, the caller must NOT add a semicolon
    /// since this function includes it. Use `type_annotation_has_trailing_comment_doc` to check.
    pub(in crate::printer) fn build_type_annotation_doc(
        &self,
        annotation: &internal::TSTypeAnnotation,
    ) -> DocId {
        let d = self.d();
        // Check for comments between `:` and the type
        let colon_end = annotation.span.start + 1; // After the `:`
        let type_start = annotation.type_annotation.span().start;

        // Check if there's a line comment between : and the type
        if self.has_line_comments_between(colon_end, type_start) {
            // Check if type is union (gets indented) or intersection (stays at same level)
            let is_union = matches!(&*annotation.type_annotation, TSType::Union(_));
            let is_intersection = matches!(&*annotation.type_annotation, TSType::Intersection(_));

            if is_union {
                // Line comment stays before union, type on new INDENTED line
                let comments_doc = self.build_inline_comments_between_doc(colon_end, type_start);
                d.concat(&[
                    d.text(":"),
                    comments_doc,
                    // Wrap hardline inside indent so the line break is at the indented level
                    d.indent(d.concat(&[
                        d.hardline(),
                        self.build_type_doc(&annotation.type_annotation),
                    ])),
                ])
            } else if is_intersection {
                // Line comment stays before intersection, type on new line (NOT indented)
                let comments_doc = self.build_inline_comments_between_doc(colon_end, type_start);
                d.concat(&[
                    d.text(":"),
                    comments_doc,
                    d.hardline(),
                    self.build_type_doc(&annotation.type_annotation),
                ])
            } else {
                // Simple type: comment moves to after the type (prettier 3.7 behavior)
                // Include semicolon here to ensure comment is before it
                let comments_doc = self.build_inline_comments_between_doc(colon_end, type_start);
                d.concat(&[
                    d.text(": "),
                    self.build_type_doc(&annotation.type_annotation),
                    d.text(";"),
                    comments_doc,
                ])
            }
        } else {
            // Handle unions/intersections with width-based breaking
            // Short: `param: Type1 | Type2`
            // Long: `param:\n\t| Type1\n\t| Type2`
            //
            // This pattern matches index signature type annotation handling.
            // For unions/intersections, wrap in group + indent + line so they break after `:`
            // and inherit breaking from this context's group.
            match annotation.type_annotation.as_ref() {
                TSType::Union(u) => {
                    let type_doc = self.build_union_type_doc(u, false);
                    d.group(d.concat(&[
                        d.text(":"),
                        d.indent(d.concat(&[
                            d.line(), // space when flat, newline when broken
                            type_doc,
                        ])),
                    ]))
                }
                TSType::Intersection(i) => {
                    // Build intersection with proper indentation for type annotation context:
                    // `: FirstType &` stays on the same line, continuation types are indented
                    self.build_intersection_type_annotation_doc(i)
                }
                _ => {
                    // Block comments stay inline: `: /* comment */ Type`
                    let mut parts = vec![d.text(": ")];
                    parts.push(self.build_comments_between(
                        colon_end,
                        type_start,
                        CommentSpacing::Trailing,
                    ));
                    parts.push(self.build_type_doc(&annotation.type_annotation));
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
        annotation: &internal::TSTypeAnnotation,
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
        annotation: &internal::TSTypeAnnotation,
    ) -> DocId {
        self.build_type_annotation_doc_with_wrapping(annotation, false)
    }

    /// Inner implementation for type annotation with wrapping support.
    ///
    /// When `always_wrap` is true, wraps any TypeReference with type args.
    /// When false, only wraps if type args would benefit from breaking.
    fn build_type_annotation_doc_with_wrapping(
        &self,
        annotation: &internal::TSTypeAnnotation,
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
        if let TSType::TypeReference(r) = annotation.type_annotation.as_ref()
            && let Some(type_args) = &r.type_arguments
            && (always_wrap || type_args_should_wrap_for_return_type(type_args))
        {
            return d.concat(&[
                d.text(": "),
                super::super::build_entity_name_doc(self.d(), &r.type_name),
                self.build_type_arguments_doc_wrapping(type_args),
            ]);
        }

        // Handle Union types - break after colon with indent when long
        if let TSType::Union(u) = annotation.type_annotation.as_ref() {
            let type_doc = self.build_union_type_doc(u, false);
            let union_group = d.group(d.indent_line(type_doc));
            return d.concat(&[d.text(":"), union_group]);
        }

        self.build_type_annotation_doc(annotation)
    }

    /// Check if a type annotation has a trailing line comment (between : and type)
    /// that should be moved after the type for simple types.
    ///
    /// Used by callers to know when NOT to add a trailing semicolon
    /// (because build_type_annotation_doc already includes it).
    pub(in crate::printer) fn type_annotation_has_trailing_comment_doc(
        &self,
        annotation: &internal::TSTypeAnnotation,
    ) -> bool {
        let colon_end = annotation.span.start + 1;
        let type_start = annotation.type_annotation.span().start;
        self.has_line_comments_between(colon_end, type_start)
            && !matches!(
                &*annotation.type_annotation,
                TSType::Union(_) | TSType::Intersection(_)
            )
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
    /// Both functions share the same grouping rule: huggable-only (2-type with
    /// TypeLiteral last) skips the group; all other cases need one.
    fn build_intersection_type_annotation_doc(
        &self,
        intersection: &internal::TSIntersectionType,
    ) -> DocId {
        let d = self.d();
        if intersection.types.is_empty() {
            return d.text(": ");
        }

        // Single type - just use the normal intersection doc
        if intersection.types.len() == 1 {
            return d.concat(&[
                d.text(": "),
                self.build_type_doc_with_wrapping_type_args(&intersection.types[0]),
            ]);
        }

        // Check for huggable last type (TypeLiteral)
        let last_is_huggable = intersection_has_huggable_last_type(intersection);
        let last_idx = intersection.types.len() - 1;

        // Build first type (stays on same line as `:`)
        // Use wrapping type args so GenericType<...> can break at print width
        let first_type = &intersection.types[0];
        let first_type_doc = self.build_intersection_member_type_doc(first_type);

        let mut first_parts = vec![d.text(": "), first_type_doc];

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
        let mut continuation_parts = Vec::new();
        for (i, t) in intersection.types.iter().enumerate().skip(1) {
            let type_start = t.span().start;
            let type_end = t.span().end;
            let is_last = i == last_idx;

            // Space/line before this type
            if is_last && last_is_huggable {
                // Keep `& {` hugged
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
            // Huggable-only (A & {b}): no indent, TypeLiteral handles its own expansion
            // All other cases: wrap continuation in indent
            if last_is_huggable && intersection.types.len() == 2 {
                parts.extend(continuation_parts);
            } else {
                parts.push(d.indent(d.concat(&continuation_parts)));
            }
        }

        // Huggable-only (A & {b}): no group needed, TypeLiteral expands itself.
        // All other cases: group controls line() flat/break behavior.
        if last_is_huggable && intersection.types.len() == 2 {
            d.concat(&parts)
        } else {
            d.group(d.concat(&parts))
        }
    }

    /// Build intersection member type with optional parens and wrapping type args.
    fn build_intersection_member_type_doc(&self, t: &TSType) -> DocId {
        let d = self.d();
        if type_needs_parens_in_intersection(t) {
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
