// Type annotation printing for TypeScript
//
// Handles printing of type annotations (`: Type`) with various contexts:
// - Simple type annotations
// - Width-aware wrapping for type arguments
// - Return type annotations

use super::helpers::{should_hug_union_type, type_args_should_wrap_for_return_type};
use super::{CommentSpacing, Printer};
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
            //
            // A *block* comment in this gap is handled by the else branch below, NOT
            // here: a newline-broken block compacts to the inline value-side position
            // (`a: /* c */ X`) rather than hanging — a deliberate, cataloged choice
            // (annotation_leading_block_prettier_divergence).
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
                    let type_doc = self.build_union_type_doc(u);
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
                _ => self.build_simple_type_annotation_doc(
                    colon_end,
                    type_start,
                    annotation.type_annotation,
                ),
            }
        }
    }

    /// Emit `: <block-comments> <type>` for a simple annotation — the fall-through
    /// shared by `build_type_annotation_doc`'s `_` match arm and
    /// `build_type_annotation_doc_with_wrapping` (once its wrapping-TypeReference /
    /// Union / Intersection branches are ruled out). Block comments in the `:`→type
    /// gap stay inline (`: /* c */ Type`). Takes the caller's already-computed
    /// `colon_end` / `type_start` so neither re-derives them, and the raw `ty` (not an
    /// unwrapped form) so redundant parens like `: (string)` are preserved.
    fn build_simple_type_annotation_doc(
        &self,
        colon_end: u32,
        type_start: u32,
        ty: &TSType<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = smallvec![d.text(": ")];
        parts.push(self.build_comments_between(colon_end, type_start, CommentSpacing::Trailing));
        parts.push(self.build_type_doc(ty));
        d.concat(&parts)
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
            let type_doc = self.build_union_type_doc(u);

            // Extract comments between `:` and the union type (e.g., `: /* c */ A | B`)
            let comments_doc =
                self.build_comments_between(colon_end, value_type_start, CommentSpacing::Trailing);

            // A brace-hugging union return (`{ … } | null` / `| void`) hugs `:`
            // block-style, like the type-alias RHS — the object owns its own expansion
            // and the void member trails the `}`. Prettier never breaks after `:` here
            // (it hugs even behind a very long method name), so there is no
            // break-after-colon fallback. `union_return_hugs` scopes it: a
            // `Promise<…> | null` `TSTypeReference` member is excluded (the sanctioned
            // `return_type_generic_union` print-width family, handled by the
            // `should_hug_union_type` branch below), and a member/gap comment
            // disqualifies the hug.
            if self.union_return_hugs(value_type, u, colon_end, value_type_start) {
                return d.concat(&[d.text(": "), comments_doc, type_doc]);
            }

            if should_hug_union_type(u) {
                // A should-hug union that didn't take the brace-hug above: a
                // `TSTypeReference` object-like member with only void siblings
                // (`Promise<…> | null`), or a brace union whose member/gap comment
                // disqualified the hug. Uses conditional_group to bypass the renderer's
                // will_break check: an inner group may break, but that shouldn't force
                // the annotation to break after `:`. The conditional_group calls fits()
                // directly, which correctly handles nested hardlines (returns true).
                // State 0: `: Promise<…> | null` (inline, no break after colon)
                // State 1: `:\n  Promise<…> | null` (break after colon, for long names)
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

        // Fall-through: reached on every simple annotation (`: string`, `: Foo`,
        // `: Foo[]`, …), the common case. Emit the shared `: <comments> <type>` path
        // directly instead of delegating to `build_type_annotation_doc`, which would
        // re-derive what we already know here: no line comments (proven false above),
        // and `unwrap_redundant_parens` + the Union/Intersection match (ruled out above).
        self.build_simple_type_annotation_doc(colon_end, type_start, annotation.type_annotation)
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

        // Multi-member: `: ` + any colon→first-member comment, then delegate the whole
        // intersection body to the shared bare-intersection printer
        // (`intersection_hanging_with_indent`). The first member hugs `:` and
        // continuations indent, exactly like the type-alias RHS — huggable boundaries,
        // the expanding-first hug, and block + line comments are all handled uniformly
        // there (a single source of truth; the bare and annotation contexts can't drift).
        // Emit only the comment between `:` and the intersection's span start. A leading
        // block comment INSIDE the intersection (`: & /* c */ A`, where the span starts at
        // the leading `&`) is emitted by the bare printer, so bounding at `types[0]` here
        // would double-emit it.
        let comments_doc = self.build_comments_between(
            colon_end,
            intersection.span.start,
            CommentSpacing::Trailing,
        );
        d.concat(&[
            d.text(": "),
            comments_doc,
            self.intersection_hanging_with_indent(intersection),
        ])
    }
}
