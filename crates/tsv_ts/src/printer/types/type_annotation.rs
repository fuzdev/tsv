// Type annotation printing for TypeScript
//
// Handles printing of type annotations (`: Type`) with various contexts:
// - Simple type annotations
// - Width-aware wrapping for type arguments
// - Return type annotations

use super::helpers::{intersection_has_huggable_last_type, type_args_should_wrap_for_return_type};
use super::{CommentSpacing, Printer};
use crate::ast::internal::{self, TSType};
use tsv_lang::doc::{self, Doc};

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
    ) -> Doc {
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
                doc::concat(vec![
                    doc::text(":"),
                    comments_doc,
                    // Wrap hardline inside indent so the line break is at the indented level
                    doc::indent(doc::concat(vec![
                        doc::hardline(),
                        self.build_type_doc(&annotation.type_annotation),
                    ])),
                ])
            } else if is_intersection {
                // Line comment stays before intersection, type on new line (NOT indented)
                let comments_doc = self.build_inline_comments_between_doc(colon_end, type_start);
                doc::concat(vec![
                    doc::text(":"),
                    comments_doc,
                    doc::hardline(),
                    self.build_type_doc(&annotation.type_annotation),
                ])
            } else {
                // Simple type: comment moves to after the type (prettier 3.7 behavior)
                // Include semicolon here to ensure comment is before it
                let comments_doc = self.build_inline_comments_between_doc(colon_end, type_start);
                doc::concat(vec![
                    doc::text(": "),
                    self.build_type_doc(&annotation.type_annotation),
                    doc::text(";"),
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
                    doc::group(doc::concat(vec![
                        doc::text(":"),
                        doc::indent(doc::concat(vec![
                            doc::line(), // space when flat, newline when broken
                            type_doc,
                        ])),
                    ]))
                }
                TSType::Intersection(i) => {
                    // Check if last type is huggable (TypeLiteral) - if so, don't add indent/line
                    // because the TypeLiteral handles its own expansion with proper indentation
                    let type_doc = self.build_intersection_type_doc(i, false);
                    if intersection_has_huggable_last_type(i) {
                        // No indent/line - keep `: Type & {` hugged
                        doc::concat(vec![doc::text(": "), type_doc])
                    } else {
                        doc::group(doc::concat(vec![
                            doc::text(":"),
                            doc::indent(doc::concat(vec![doc::line(), type_doc])),
                        ]))
                    }
                }
                _ => {
                    // Block comments stay inline for simple types
                    // Use trailing spacing so comment gets space after: `: /* comment */ Type`
                    let has_comments = self.has_comments_between(colon_end, type_start);
                    if has_comments {
                        let comments_doc = self.build_comments_between(
                            colon_end,
                            type_start,
                            CommentSpacing::Trailing,
                        );
                        doc::concat(vec![
                            doc::text(": "),
                            comments_doc,
                            self.build_type_doc(&annotation.type_annotation),
                        ])
                    } else {
                        doc::concat(vec![
                            doc::text(": "),
                            self.build_type_doc(&annotation.type_annotation),
                        ])
                    }
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
    ) -> Doc {
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
    ) -> Doc {
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
    ) -> Doc {
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
            return doc::concat(vec![
                doc::text(": "),
                super::super::build_entity_name_doc(&r.type_name),
                self.build_type_arguments_doc_wrapping(type_args),
            ]);
        }

        // Handle Union types - break after colon with indent when long
        if let TSType::Union(u) = annotation.type_annotation.as_ref() {
            let type_doc = self.build_union_type_doc(u, false);
            let union_group = doc::group(doc::indent(doc::concat(vec![doc::line(), type_doc])));
            return doc::concat(vec![doc::text(":"), union_group]);
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
}
