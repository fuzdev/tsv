// Type annotation printing for TypeScript
//
// Handles printing of TypeScript-specific type syntax:
// - Type annotations (: Type)
// - Type keywords (number, string, boolean, etc.)
// - Future: Complex types (unions, intersections, generics, etc.)

use super::{CommentFilter, CommentSpacing, Printer};
use crate::ast::internal::{
    self, TSArrayType, TSConstructorType, TSFunctionType, TSIntersectionType, TSLiteralType,
    TSTupleType, TSType, TSTypeElement, TSTypeLiteral, TSTypeParameter, TSTypeParameterDeclaration,
    TSUnionType, TemplateLiteralType,
};
use tsv_lang::SymbolToU32;
use tsv_lang::comments_in_range;
use tsv_lang::doc::{self, Doc};
use tsv_lang::printing::is_same_line;

// =============================================================================
// Helper functions
// =============================================================================

/// Check if type arguments warrant wrapping in return types.
///
/// Returns true when type args can benefit from breaking:
/// - Multiple type args (like `Result<A, B>`) - can break between args
/// - Unions or intersections (like `Promise<A | B>`) - can break internally
///
/// Returns false for single simple type args (like `Promise<void>`) - these should
/// let function params break first rather than breaking the return type.
fn type_args_should_wrap_for_return_type(args: &internal::TSTypeParameterInstantiation) -> bool {
    // Multiple type args can break between them
    if args.params.len() > 1 {
        return true;
    }
    // Unions/intersections can break internally
    args.params
        .iter()
        .any(|param| matches!(param, TSType::Union(_) | TSType::Intersection(_)))
}

/// Find the position of a separator character in the source between start and end,
/// skipping over comments. Returns Some(position) if found, None otherwise.
fn find_separator_position(source: &str, start: u32, end: u32, separator: u8) -> Option<u32> {
    let bytes = source.as_bytes();
    let mut pos = start as usize;
    let end = end as usize;

    while pos < end {
        let b = bytes[pos];
        if b == separator {
            return Some(pos as u32);
        }
        // Skip block comments: /* ... */
        if b == b'/' && pos + 1 < end && bytes[pos + 1] == b'*' {
            pos += 2;
            while pos + 1 < end {
                if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                    pos += 2;
                    break;
                }
                pos += 1;
            }
            continue;
        }
        // Skip line comments: // ...
        if b == b'/' && pos + 1 < end && bytes[pos + 1] == b'/' {
            pos += 2;
            while pos < end && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        pos += 1;
    }
    None
}

// =============================================================================
// Type parenthesization helpers
// =============================================================================

/// Recursively unwrap TSParenthesizedType to get the inner type.
pub(crate) fn unwrap_parenthesized(ts_type: &TSType) -> &TSType {
    match ts_type {
        TSType::Parenthesized(p) => unwrap_parenthesized(&p.type_annotation),
        _ => ts_type,
    }
}

/// Check if the last type in an intersection is "huggable" (like TypeLiteral).
///
/// Huggable types expand independently and should not have breaks/indent applied
/// around them in the parent context. This keeps patterns like `& {` hugged together.
#[inline]
pub(crate) fn intersection_has_huggable_last_type(intersection: &TSIntersectionType) -> bool {
    intersection
        .types
        .last()
        .is_some_and(|t| matches!(unwrap_parenthesized(t), TSType::TypeLiteral(_)))
}

/// Check if a type needs parentheses when used as the object in indexed access (`T[K]`).
/// Without parens: `A | B[K]` parses as `A | (B[K])`, not `(A | B)[K]`
fn type_needs_parens_for_indexed_access_object(ts_type: &TSType) -> bool {
    let inner = unwrap_parenthesized(ts_type);
    // TypeOperator included: `(keyof T)[K]` is valid and different from `keyof T[K]`
    matches!(
        inner,
        TSType::Union(_)
            | TSType::Intersection(_)
            | TSType::TypeQuery(_)
            | TSType::TypeOperator(_)
            | TSType::Conditional(_)
            | TSType::Infer(_)
            | TSType::Function(_)
            | TSType::Constructor(_)
    )
}

/// Check if a type needs parentheses when used as the element type in an array (`T[]`).
/// Without parens: `A | B[]` parses as `A | (B[])`, not `(A | B)[]`
fn type_needs_parens_for_array_element(ts_type: &TSType) -> bool {
    let inner = unwrap_parenthesized(ts_type);
    // TypeOperator excluded: `(readonly T)[]` is invalid TypeScript
    matches!(
        inner,
        TSType::Union(_)
            | TSType::Intersection(_)
            | TSType::TypeQuery(_)
            | TSType::Conditional(_)
            | TSType::Infer(_)
            | TSType::Function(_)
            | TSType::Constructor(_)
    )
}

/// Check if a type needs parentheses when used as the operand of a prefix type operator
/// (keyof, readonly, unique). Without parens: `keyof A | B` parses as `(keyof A) | B`
fn type_needs_parens_for_prefix_operator(ts_type: &TSType) -> bool {
    let inner = unwrap_parenthesized(ts_type);
    matches!(inner, TSType::Union(_) | TSType::Intersection(_))
}

/// Check if a type needs parentheses when used as a member of an intersection.
/// Union, function, constructor, and conditional types have lower precedence than `&`.
fn type_needs_parens_in_intersection(ts_type: &TSType) -> bool {
    let inner = unwrap_parenthesized(ts_type);
    matches!(
        inner,
        TSType::Union(_) | TSType::Function(_) | TSType::Constructor(_) | TSType::Conditional(_)
    )
}

/// Check if a type needs parentheses when used as a member of a union.
/// Function, constructor, and conditional types have lower precedence than `|`.
fn type_needs_parens_in_union(ts_type: &TSType) -> bool {
    let inner = unwrap_parenthesized(ts_type);
    matches!(
        inner,
        TSType::Function(_)
            | TSType::Constructor(_)
            | TSType::Conditional(_)
            | TSType::Intersection(_)
    )
}

impl<'a> Printer<'a> {
    /// Build type doc, wrapping in parentheses if the predicate returns true.
    ///
    /// Uses `align_spaces(2, ...)` for proper Prettier-style alignment:
    /// - Object properties get pure tabs (via double indent)
    /// - Closing `})` gets 2-space alignment after tabs
    ///
    /// Special case: intersection with trailing object type builds a custom doc
    /// so that `})` can be aligned properly (at base indent + 2 spaces).
    fn build_type_doc_maybe_parens(
        &self,
        ts_type: &TSType,
        needs_parens: fn(&TSType) -> bool,
    ) -> Doc {
        if needs_parens(ts_type) {
            // Special case: intersection with trailing object type
            // Build custom doc for proper alignment of closing `})`
            // Note: unwrap_parenthesized to handle cases like `(A & {...})` where
            // the input is TSParenthesizedType wrapping TSIntersectionType
            if let TSType::Intersection(intersection) = unwrap_parenthesized(ts_type)
                && let Some(last) = intersection.types.last()
                && let TSType::TypeLiteral(obj) = unwrap_parenthesized(last)
            {
                return self
                    .build_parenthesized_intersection_trailing_object_doc(intersection, obj);
            }

            // Default case: simple parenthesization
            doc::concat(vec![
                doc::text("("),
                doc::align_spaces(2, doc::indent(self.build_type_doc(ts_type))),
                doc::text(")"),
            ])
        } else {
            self.build_type_doc(ts_type)
        }
    }

    /// Build doc for `(A & B & { members })` with proper alignment.
    ///
    /// Prettier aligns `})` with the opening `(` using tabs + spaces when breaking:
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
    /// - Print members with double indent (for proper 4-tab alignment)
    /// - Print `})` at base indent + 2-space alignment (when breaking)
    fn build_parenthesized_intersection_trailing_object_doc(
        &self,
        intersection: &TSIntersectionType,
        trailing_obj: &TSTypeLiteral,
    ) -> Doc {
        // Build opening: (A & B & {
        let mut opening_parts = vec![doc::text("(")];

        // Build intersection types except the last one (the object)
        let types_before_object = &intersection.types[..intersection.types.len() - 1];
        for (i, t) in types_before_object.iter().enumerate() {
            if i > 0 {
                opening_parts.push(doc::text(" & "));
            }
            opening_parts.push(self.build_type_doc(t));
        }

        // Add ` & {`
        opening_parts.push(doc::text(" & {"));

        self.build_aligned_object_literal_doc(trailing_obj, doc::concat(opening_parts), "})")
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
        t: &TSTypeLiteral,
        force_multiline: bool,
    ) -> Doc {
        if t.members.is_empty() {
            return doc::text("");
        }

        let mut member_parts = vec![];
        let mut prev_end = t.span.start + 1; // after opening brace

        for (i, m) in t.members.iter().enumerate() {
            let is_first = i == 0;
            let is_last = i == t.members.len() - 1;
            let member_end = m.span().end;

            if force_multiline {
                // Forced multiline: build with hardlines
                let all_comments: Vec<_> =
                    comments_in_range(self.comments, prev_end, m.span().start).collect();
                let leading_comments: Vec<_> = if !is_first {
                    all_comments
                        .iter()
                        .filter(|c| !is_same_line(self.source, prev_end, c.span.start))
                        .copied()
                        .collect()
                } else {
                    all_comments
                };

                let has_blank = if !leading_comments.is_empty() {
                    tsv_lang::printing::has_blank_line_between(
                        self.source,
                        prev_end,
                        leading_comments[0].span.start,
                    )
                } else {
                    tsv_lang::printing::has_blank_line_between(
                        self.source,
                        prev_end,
                        m.span().start,
                    )
                };

                if has_blank && !is_first {
                    member_parts.push(doc::literalline());
                }
                member_parts.push(doc::hardline());

                member_parts.extend(
                    self.build_leading_comments_with_blank_lines(&leading_comments, m.span().start),
                );
                member_parts.push(self.build_type_member_doc_inner(m, false));

                // Handle trailing comments
                let upper_bound = t
                    .members
                    .get(i + 1)
                    .map_or(t.span.end, |next| next.span().start);
                let trailing: Vec<_> = comments_in_range(self.comments, member_end, upper_bound)
                    .filter(|c| is_same_line(self.source, member_end, c.span.start))
                    .collect();

                for comment in trailing.iter().filter(|c| c.is_block) {
                    member_parts.push(doc::text(" "));
                    member_parts.push(self.build_comment_doc(comment));
                }
                member_parts.push(doc::text(";"));
                for comment in trailing.iter().filter(|c| !c.is_block) {
                    member_parts.push(doc::text(" "));
                    member_parts.push(self.build_comment_doc(comment));
                }
            } else {
                // Width-aware: softlines, conditional semicolons
                member_parts.push(doc::softline());
                member_parts.push(self.build_type_member_doc_inner(m, false));

                // Handle trailing comments
                let upper_bound = t
                    .members
                    .get(i + 1)
                    .map_or(t.span.end, |next| next.span().start);
                for comment in comments_in_range(self.comments, member_end, upper_bound) {
                    member_parts.push(doc::text(" "));
                    member_parts.push(self.build_comment_doc(comment));
                }

                if is_last {
                    // Last member: semicolon only when broken
                    member_parts.push(doc::if_break(doc::text(";"), doc::text("")));
                } else {
                    // Non-last: semicolon always, space only when flat
                    member_parts.push(doc::if_break(doc::text(";"), doc::text("; ")));
                }
            }

            prev_end = member_end;
        }

        if force_multiline {
            // Trailing comments after last member
            let body_end = t.span.end.saturating_sub(1);
            member_parts.extend(self.build_trailing_body_comments_doc(prev_end, body_end));
        }

        doc::concat(member_parts)
    }

    /// Check if a TypeLiteral should be forced to multiline format.
    ///
    /// Returns true if:
    /// - Source has newline immediately after opening brace
    /// - Contains line comments or multi-line block comments
    /// - Contains block comments on their own line
    fn type_literal_force_multiline(&self, obj: &TSTypeLiteral) -> bool {
        let source_is_multiline = super::is_brace_block_multiline(self.source, obj.span);
        let has_line_or_multiline_block =
            comments_in_range(self.comments, obj.span.start, obj.span.end)
                .any(|c| !c.is_block || c.content.contains('\n'));
        let member_spans: Vec<_> = obj.members.iter().map(TSTypeElement::span).collect();
        let has_standalone_block =
            self.has_standalone_block_comment(obj.span.start, obj.span.end, &member_spans);
        source_is_multiline || has_line_or_multiline_block || has_standalone_block
    }

    /// Build aligned object literal doc with custom opening/closing.
    ///
    /// Used for object literals in union types and parenthesized intersections
    /// where Prettier uses:
    /// - Double indent for members (aligns with content after `{`)
    /// - 2-space alignment for closing (aligns with `{`)
    fn build_aligned_object_literal_doc(
        &self,
        obj: &TSTypeLiteral,
        opening: Doc,
        closing: &'static str,
    ) -> Doc {
        let force_multiline = self.type_literal_force_multiline(obj);
        let members_doc =
            self.build_type_literal_members_only_doc_for_alignment(obj, force_multiline);

        let line_doc = if force_multiline {
            doc::hardline()
        } else {
            doc::softline()
        };

        doc::group(doc::concat(vec![
            opening,
            doc::indent(doc::indent(members_doc)),
            doc::align_spaces(2, doc::concat(vec![line_doc, doc::text(closing)])),
        ]))
    }

    /// Build doc for object type literal when it's a direct union member.
    ///
    /// Prettier aligns object content with the position after `| {`:
    /// ```text
    /// type T =
    ///   | {
    ///       prop: A;  // double indent (aligns with content after "{ ")
    ///     }           // base indent + 2 spaces (aligns with "{")
    ///   | B;
    /// ```
    fn build_union_member_object_literal_doc(&self, obj: &TSTypeLiteral) -> Doc {
        self.build_aligned_object_literal_doc(obj, doc::text("{"), "}")
    }

    /// Build doc for type parameter declaration: `<T, U extends V = W>`
    /// Non-wrapping version - always inline
    pub(super) fn build_type_parameter_declaration_doc(
        &self,
        decl: &TSTypeParameterDeclaration,
    ) -> Doc {
        let param_docs: Vec<_> = decl
            .params
            .iter()
            .map(|param| self.build_type_parameter_doc(param))
            .collect();
        doc::concat(vec![
            doc::text("<"),
            doc::join(param_docs, ", "),
            doc::text(">"),
        ])
    }

    /// Build doc for type parameter declaration with wrapping support
    /// When the group breaks, each param goes on its own line with trailing comma
    pub(super) fn build_type_parameter_declaration_doc_wrapping(
        &self,
        decl: &TSTypeParameterDeclaration,
    ) -> Doc {
        doc::group(self.build_type_parameter_declaration_doc_inner(decl))
    }

    /// Build doc for type parameter declaration - inner version without group wrapper
    /// Used when caller wants to control the group (e.g., interface header)
    pub(super) fn build_type_parameter_declaration_doc_inner(
        &self,
        decl: &TSTypeParameterDeclaration,
    ) -> Doc {
        if decl.params.is_empty() {
            return doc::text("<>");
        }

        // Check for line comments between parameters or after last parameter (force multiline)
        if self.has_line_comments_in_delimited_list(&decl.params, |p| p.span, decl.span.end - 1) {
            return self.build_type_parameter_declaration_doc_with_line_comments(decl);
        }

        let docs: Vec<_> = decl
            .params
            .iter()
            .map(|param| self.build_type_parameter_doc(param))
            .collect();
        let inner_parts = doc::join_trailing(docs, doc::comma_line());

        doc::concat(vec![
            doc::text("<"),
            doc::indent_softline(inner_parts),
            doc::softline(),
            doc::text(">"),
        ])
    }

    /// Build doc for type parameter declaration with line comments between params
    fn build_type_parameter_declaration_doc_with_line_comments(
        &self,
        decl: &TSTypeParameterDeclaration,
    ) -> Doc {
        let mut inner_parts = Vec::new();
        let mut prev_end = decl.span.start + 1; // After the opening `<`

        for (i, param) in decl.params.iter().enumerate() {
            let param_start = param.span.start;
            let param_end = param.span.end;
            let is_last = i == decl.params.len() - 1;

            // Leading comments
            inner_parts.extend(self.build_leading_comments_multiline(prev_end, param_start));

            inner_parts.push(self.build_type_parameter_doc(param));

            let next_boundary = if i + 1 < decl.params.len() {
                decl.params[i + 1].span.start
            } else {
                decl.span.end - 1 // Before the closing `>`
            };

            // Trailing comma for all params
            inner_parts.push(doc::text(","));

            // Trailing comments
            inner_parts.extend(self.build_trailing_comments_multiline(param_end, next_boundary));

            // Hardline to separate from next element
            if !is_last {
                inner_parts.push(doc::hardline());
            }

            prev_end = next_boundary;
        }

        doc::concat(vec![
            doc::text("<"),
            doc::indent(doc::concat(vec![doc::hardline(), doc::concat(inner_parts)])),
            doc::hardline(),
            doc::text(">"),
        ])
    }

    /// Build doc for type parameter declaration without independent group
    /// Used when type params should break with the parent group (e.g., class header group mode)
    #[inline]
    pub(super) fn build_type_parameter_declaration_doc_inline_group(
        &self,
        decl: &TSTypeParameterDeclaration,
    ) -> Doc {
        self.build_type_parameter_declaration_doc_inner(decl)
    }

    /// Build doc for a single type parameter
    /// With optional modifiers: `const T`, `in T`, `out T`, `in out T`
    pub(super) fn build_type_parameter_doc(&self, param: &TSTypeParameter) -> Doc {
        let mut parts = Vec::new();

        // Add modifiers in order: const, in, out
        if param.is_const {
            parts.push(doc::text("const "));
        }
        if param.is_in {
            parts.push(doc::text("in "));
        }
        if param.is_out {
            parts.push(doc::text("out "));
        }

        parts.push(doc::symbol(param.name.name.to_u32()));

        if let Some(constraint) = &param.constraint {
            parts.push(doc::text(" extends "));
            parts.push(self.build_type_doc(constraint));
        }

        if let Some(default) = &param.default {
            parts.push(doc::text(" = "));
            parts.push(self.build_type_doc(default));
        }

        doc::concat(parts)
    }

    /// Build doc for type parameter instantiation (type arguments): `<T, U>`
    ///
    /// Supports breaking to multiple lines when content is too long:
    /// ```typescript
    /// new Map<
    ///     VeryLongKeyType,
    ///     VeryLongValueType,
    /// >();
    /// ```
    ///
    /// Also preserves comments: `</* a */ T /* b */, U>`
    pub(super) fn build_type_parameter_instantiation_doc(
        &self,
        inst: &internal::TSTypeParameterInstantiation,
    ) -> Doc {
        if inst.params.is_empty() {
            return doc::text("<>");
        }

        // Check for line comments between params or after last param (force multiline)
        if self.has_line_comments_in_delimited_list(&inst.params, TSType::span, inst.span.end - 1) {
            return self.build_type_parameter_instantiation_doc_with_line_comments(inst);
        }

        // Build params with commas and line breaks
        // The doc printer's look-ahead (fits_with_lookahead) handles the decision
        // of whether to break based on what follows the type params.
        let mut param_parts = Vec::new();
        let mut prev_end = inst.span.start + 1; // After the opening `<`

        for (i, param) in inst.params.iter().enumerate() {
            let param_start = param.span().start;

            if i > 0 {
                param_parts.push(doc::text(","));
                param_parts.push(doc::line());
            }

            // Add leading block comments before this type argument
            param_parts.push(self.build_comments_between_filtered(
                prev_end,
                param_start,
                CommentSpacing::Trailing,
                CommentFilter::BlockOnly,
            ));

            param_parts.push(self.build_type_doc(param));

            // Add trailing block comments after this type argument
            let param_end = param.span().end;
            let next_boundary = if i + 1 < inst.params.len() {
                inst.params[i + 1].span().start
            } else {
                inst.span.end - 1 // Before the closing `>`
            };
            param_parts.push(self.build_comments_between_filtered(
                param_end,
                next_boundary,
                CommentSpacing::Leading,
                CommentFilter::BlockOnly,
            ));

            // Update prev_end to next_boundary to avoid double-counting comments
            prev_end = next_boundary;
        }

        // Wrap in group with angle brackets and optional breaks
        doc::group(doc::concat(vec![
            doc::text("<"),
            doc::indent_softline(doc::concat(param_parts)),
            doc::softline(),
            doc::text(">"),
        ]))
    }

    /// Build type parameter instantiation with line comments
    fn build_type_parameter_instantiation_doc_with_line_comments(
        &self,
        inst: &internal::TSTypeParameterInstantiation,
    ) -> Doc {
        let mut inner_parts = Vec::new();
        let mut prev_end = inst.span.start + 1; // After the opening `<`

        for (i, param) in inst.params.iter().enumerate() {
            let param_start = param.span().start;
            let param_end = param.span().end;
            let is_last = i == inst.params.len() - 1;

            // Leading comments
            inner_parts.extend(self.build_leading_comments_multiline(prev_end, param_start));

            inner_parts.push(self.build_type_doc(param));

            let next_boundary = if i + 1 < inst.params.len() {
                inst.params[i + 1].span().start
            } else {
                inst.span.end - 1 // Before the closing `>`
            };

            // Comma (not on last element for type arguments - no trailing comma)
            if !is_last {
                inner_parts.push(doc::text(","));
            }

            // Trailing comments
            inner_parts.extend(self.build_trailing_comments_multiline(param_end, next_boundary));

            // Hardline to separate from next element
            if !is_last {
                inner_parts.push(doc::hardline());
            }

            prev_end = next_boundary;
        }

        doc::concat(vec![
            doc::text("<"),
            doc::indent(doc::concat(vec![doc::hardline(), doc::concat(inner_parts)])),
            doc::hardline(),
            doc::text(">"),
        ])
    }

    /// Build a Doc for a type annotation (e.g., `: number`)
    ///
    /// Handles comments between the colon and the type.
    /// For simple types with line comments, the comment is moved to after the type.
    /// For union types, the comment stays before and the type is INDENTED.
    /// For intersection types, the comment stays before but the type is NOT indented.
    ///
    /// NOTE: For simple types with trailing comments, the caller must NOT add a semicolon
    /// since this function includes it. Use `type_annotation_has_trailing_comment_doc` to check.
    pub(super) fn build_type_annotation_doc(&self, annotation: &internal::TSTypeAnnotation) -> Doc {
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
    pub(super) fn build_type_annotation_doc_wrapping(
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
    pub(super) fn build_type_annotation_doc_for_return_type(
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
                super::build_entity_name_doc(&r.type_name),
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
    pub(super) fn type_annotation_has_trailing_comment_doc(
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

    /// Build a Doc for a TypeScript type expression
    pub(super) fn build_type_doc(&self, ts_type: &TSType) -> Doc {
        self.build_type_doc_inner(ts_type, false)
    }

    /// Build a Doc for a TypeScript type expression with wrapping type arguments.
    ///
    /// Used in type alias RHS where TypeReference type arguments should break
    /// internally (e.g., `Promise<LongType | null>` breaks inside `<>`).
    pub(super) fn build_type_doc_with_wrapping_type_args(&self, ts_type: &TSType) -> Doc {
        self.build_type_doc_inner(ts_type, true)
    }

    /// Inner implementation for type doc building.
    /// When `wrap_type_args` is true, TypeReference uses wrapping type arguments.
    fn build_type_doc_inner(&self, ts_type: &TSType, wrap_type_args: bool) -> Doc {
        match ts_type {
            TSType::Keyword(kw) => doc::text_owned(kw.kind.as_str().to_string()),
            TSType::Literal(lit) => self.build_literal_type_doc(lit),
            TSType::Array(arr) => self.build_array_type_doc(arr),
            TSType::Union(u) => self.build_union_type_doc(u, true),
            TSType::Intersection(i) => self.build_intersection_type_doc(i, true),
            TSType::TypeReference(r) => {
                let mut parts = vec![self.build_type_entity_name_doc(&r.type_name)];
                if let Some(type_args) = &r.type_arguments {
                    if wrap_type_args {
                        parts.push(self.build_type_arguments_doc_wrapping(type_args));
                    } else {
                        parts.push(self.build_type_arguments_doc(type_args));
                    }
                }
                doc::concat(parts)
            }
            TSType::TypeLiteral(t) => self.build_type_literal_doc(t),
            TSType::Function(f) => self.build_function_type_doc(f),
            TSType::Constructor(c) => self.build_constructor_type_doc(c),
            TSType::Tuple(t) => self.build_tuple_type_doc(t),
            // Parenthesized types: just unwrap. Parent contexts (IndexedAccess, Array,
            // TypeOperator) add parens when needed based on the inner type.
            TSType::Parenthesized(p) => self.build_type_doc(&p.type_annotation),
            TSType::TypePredicate(p) => {
                let mut parts = vec![];
                if p.asserts {
                    parts.push(doc::text("asserts "));
                }
                parts.push(doc::symbol(p.parameter_name.name.to_u32()));
                if let Some(type_ann) = &p.type_annotation {
                    parts.push(doc::text(" is "));
                    parts.push(self.build_type_doc(type_ann));
                }
                doc::concat(parts)
            }
            TSType::Conditional(c) => {
                // Conditional types use width-aware wrapping:
                // When broken, ternary arms are indented:
                //   check extends extends_type
                //     ? true_type
                //     : false_type
                //
                // The outer-most conditional is wrapped in a group. Nested conditionals
                // (in true_type or false_type) are NOT wrapped in their own group - they
                // inherit breaking from the parent. This matches prettier's behavior.
                doc::group(self.build_conditional_type_doc_inner(c))
            }
            TSType::Mapped(m) => self.build_mapped_type_doc(m),
            TSType::TypeOperator(o) => {
                let needs_parens = type_needs_parens_for_prefix_operator(&o.type_annotation);
                let operand_doc = self.build_type_doc(&o.type_annotation);
                if needs_parens {
                    doc::concat(vec![
                        doc::text(o.operator.as_str()),
                        doc::text(" ("),
                        operand_doc,
                        doc::text(")"),
                    ])
                } else {
                    doc::concat(vec![
                        doc::text(o.operator.as_str()),
                        doc::text(" "),
                        operand_doc,
                    ])
                }
            }
            TSType::Import(i) => {
                let mut parts = vec![doc::text("import(")];
                parts.push(self.build_literal_doc(&i.argument));
                // Import type options
                if let Some(options) = &i.options {
                    parts.push(doc::text(", "));
                    parts.push(self.build_expression_doc(options));
                }
                parts.push(doc::text(")"));
                if let Some(qualifier) = &i.qualifier {
                    parts.push(doc::text("."));
                    parts.push(self.build_type_entity_name_doc(qualifier));
                }
                if let Some(type_args) = &i.type_arguments {
                    parts.push(self.build_type_arguments_doc(type_args));
                }
                doc::concat(parts)
            }
            TSType::TypeQuery(q) => {
                let mut parts = vec![doc::text("typeof ")];
                parts.push(self.build_type_query_expr_name_doc(&q.expr_name));
                if let Some(type_args) = &q.type_arguments {
                    parts.push(self.build_type_arguments_doc(type_args));
                }
                doc::concat(parts)
            }
            TSType::IndexedAccess(i) => {
                let object_doc = self.build_type_doc(&i.object_type);
                let needs_parens = type_needs_parens_for_indexed_access_object(&i.object_type);
                if needs_parens {
                    doc::concat(vec![
                        doc::text("("),
                        object_doc,
                        doc::text(")["),
                        self.build_type_doc(&i.index_type),
                        doc::text("]"),
                    ])
                } else {
                    doc::concat(vec![
                        object_doc,
                        doc::text("["),
                        self.build_type_doc(&i.index_type),
                        doc::text("]"),
                    ])
                }
            }
            TSType::Rest(r) => doc::concat(vec![
                doc::text("..."),
                self.build_type_doc(&r.type_annotation),
            ]),
            TSType::Optional(o) => doc::concat(vec![
                self.build_type_doc(&o.type_annotation),
                doc::text("?"),
            ]),
            TSType::NamedTupleMember(n) => {
                let mut parts = vec![doc::symbol(n.label.name.to_u32())];
                if n.optional {
                    parts.push(doc::text("?"));
                }
                parts.push(doc::text(": "));
                parts.push(self.build_type_doc(&n.element_type));
                doc::concat(parts)
            }
            TSType::Infer(i) => doc::concat(vec![
                doc::text("infer "),
                doc::symbol(i.type_parameter.name.name.to_u32()),
            ]),
        }
    }

    /// Build doc for conditional type WITHOUT the outer group wrapper.
    /// This is used for nested conditionals which should inherit breaking from their parent.
    ///
    /// Structure: `check extends extends_type [indent: line, "? ", true_type, line, ": ", false_type]`
    fn build_conditional_type_doc_inner(&self, c: &internal::TSConditionalType) -> Doc {
        // Build true_type doc: if it's a conditional (possibly wrapped in parens), don't wrap in group
        // Add parens for readability only when flat (single-line), not when broken (multi-line)
        let true_type_doc =
            if let TSType::Conditional(inner) = unwrap_parenthesized(c.true_type.as_ref()) {
                // Nested conditional in true position:
                // - Flat: add parens for readability: `T extends A ? (T extends B ? C : D) : E`
                // - Broken: no parens (the line breaks provide clarity)
                let inner_doc = self.build_conditional_type_doc_inner(inner);
                doc::if_break(
                    inner_doc.clone(),
                    doc::concat(vec![doc::text("("), inner_doc, doc::text(")")]),
                )
            } else {
                self.build_type_doc(&c.true_type)
            };

        // Build false_type doc: if it's a conditional, don't wrap in group
        // No parens needed for nested conditionals in false position (right-associative)
        let false_type_doc =
            if let TSType::Conditional(inner) = unwrap_parenthesized(c.false_type.as_ref()) {
                self.build_conditional_type_doc_inner(inner)
            } else {
                self.build_type_doc(&c.false_type)
            };

        // Build extends_type doc - unions need special handling to avoid trailing space
        // after "extends" when the union breaks (e.g., `T extends\n\t| A\n\t| B`)
        let extends_type_doc = if let TSType::Union(union) = c.extends_type.as_ref() {
            if union.types.is_empty() {
                doc::text(" ")
            } else {
                let mut parts = Vec::new();
                for (i, t) in union.types.iter().enumerate() {
                    if i > 0 {
                        parts.push(doc::if_break(
                            doc::concat(vec![doc::line(), doc::text("| ")]),
                            doc::text(" | "),
                        ));
                    } else {
                        // First type: line() + "| " when broken, space when flat
                        parts.push(doc::if_break(
                            doc::concat(vec![doc::line(), doc::text("| ")]),
                            doc::text(" "),
                        ));
                    }
                    parts.push(self.build_type_doc(t));
                }
                doc::group(doc::indent(doc::concat(parts)))
            }
        } else {
            doc::concat(vec![doc::text(" "), self.build_type_doc(&c.extends_type)])
        };

        doc::concat(vec![
            self.build_type_doc(&c.check_type),
            doc::text(" extends"),
            extends_type_doc,
            doc::indent(doc::concat(vec![
                doc::line(),
                doc::text("? "),
                true_type_doc,
                doc::line(),
                doc::text(": "),
                false_type_doc,
            ])),
        ])
    }

    /// Build doc for type query expression name
    fn build_type_query_expr_name_doc(&self, expr_name: &internal::TSTypeQueryExprName) -> Doc {
        match expr_name {
            internal::TSTypeQueryExprName::EntityName(entity) => {
                self.build_type_entity_name_doc(entity)
            }
            internal::TSTypeQueryExprName::Import(i) => {
                let mut parts = vec![doc::text("import(")];
                parts.push(self.build_literal_doc(&i.argument));
                parts.push(doc::text(")"));
                if let Some(qualifier) = &i.qualifier {
                    parts.push(doc::text("."));
                    parts.push(self.build_type_entity_name_doc(qualifier));
                }
                if let Some(type_args) = &i.type_arguments {
                    parts.push(self.build_type_arguments_doc(type_args));
                }
                doc::concat(parts)
            }
        }
    }

    /// Build doc for mapped type: `{ [K in T]: V }`
    ///
    /// Source-fidelity aware: preserves multi-line formatting when source is multi-line.
    /// - Source one-line, fits: `{[K in keyof T]: T[K]}`
    /// - Source one-line, long: `{\n\t[K in keyof T]: T[K];\n}`
    /// - Source multi-line: `{\n\t[K in keyof T]: T[K];\n}` (always)
    fn build_mapped_type_doc(&self, m: &internal::TSMappedType) -> Doc {
        // Check if source was multi-line (preserve author's formatting choice)
        let source_is_multiline = super::is_brace_block_multiline(self.source, m.span);

        // Build the mapping body
        let mut body_parts = vec![];

        // readonly modifier: `readonly` or `-readonly`
        if let Some(readonly) = m.readonly {
            body_parts.push(doc::text(if readonly { "readonly " } else { "-readonly " }));
        }

        // [K in constraint]
        body_parts.push(doc::text("["));
        body_parts.push(doc::text_owned(m.type_parameter.name.clone()));
        body_parts.push(doc::text(" in "));
        body_parts.push(self.build_type_doc(&m.type_parameter.constraint));

        // as clause: `as NewKeyType`
        if let Some(name_type) = &m.name_type {
            body_parts.push(doc::text(" as "));
            body_parts.push(self.build_type_doc(name_type));
        }

        body_parts.push(doc::text("]"));

        // optional modifier: `?` or `-?`
        if let Some(optional) = m.optional {
            body_parts.push(doc::text(if optional { "?" } else { "-?" }));
        }

        body_parts.push(doc::text(": "));

        // value type
        if let Some(type_ann) = &m.type_annotation {
            body_parts.push(self.build_type_doc(type_ann));
        }

        if source_is_multiline {
            // Multi-line source: preserve multi-line format with hardlines
            doc::concat(vec![
                doc::text("{"),
                doc::indent(doc::concat(vec![
                    doc::hardline(),
                    doc::concat(body_parts),
                    doc::text(";"),
                ])),
                doc::hardline(),
                doc::text("}"),
            ])
        } else {
            // One-line source: width-aware (stays inline if fits, wraps if too long)
            body_parts.insert(0, doc::softline());
            body_parts.push(doc::if_break(doc::text(";"), doc::text("")));
            doc::group(doc::concat(vec![
                doc::text("{"),
                doc::indent(doc::concat(body_parts)),
                doc::softline(),
                doc::text("}"),
            ]))
        }
    }

    /// Build doc for type entity name
    fn build_type_entity_name_doc(&self, name: &internal::TSEntityName) -> Doc {
        // Delegate to standalone function - doesn't need printer state
        super::build_entity_name_doc(name)
    }

    /// Build doc for type member with optional trailing semicolon
    fn build_type_member_doc_inner(&self, member: &TSTypeElement, with_semicolon: bool) -> Doc {
        match member {
            TSTypeElement::PropertySignature(prop) => {
                let mut parts = vec![];
                if prop.readonly {
                    parts.push(doc::text("readonly "));
                }
                if prop.computed {
                    parts.push(doc::text("["));
                    parts.push(self.build_expression_doc(&prop.key));
                    parts.push(doc::text("]"));
                } else {
                    parts.push(self.build_expression_doc(&prop.key));
                }

                // Handle comments between key and colon (e.g., `b /* comment */: B`)
                let key_end = prop.key.span().end;
                if let Some(type_ann) = &prop.type_annotation {
                    let type_ann_start = type_ann.span.start;
                    for comment in comments_in_range(self.comments, key_end, type_ann_start) {
                        parts.push(doc::text(" "));
                        parts.push(self.build_comment_doc(comment));
                    }
                }

                if prop.optional {
                    parts.push(doc::text("?"));
                }
                if let Some(type_ann) = &prop.type_annotation {
                    // Use width-aware wrapping for TypeReference with type arguments
                    parts.push(self.build_type_annotation_doc_wrapping(type_ann));

                    // Handle comments between type and semicolon (e.g., `a: A /* block */;`)
                    let type_end = type_ann.span.end;
                    let prop_end = prop.span.end;
                    for comment in comments_in_range(self.comments, type_end, prop_end) {
                        parts.push(doc::text(" "));
                        parts.push(self.build_comment_doc(comment));
                    }

                    // For simple types with trailing comments, the doc already includes semicolon
                    let has_trailing_comment =
                        self.type_annotation_has_trailing_comment_doc(type_ann);
                    if with_semicolon && !has_trailing_comment {
                        parts.push(doc::text(";"));
                    }
                } else if with_semicolon {
                    parts.push(doc::text(";"));
                }
                doc::concat(parts)
            }
            TSTypeElement::MethodSignature(method) => {
                let mut parts = vec![];
                // Print accessor keyword for get/set signatures
                match method.kind {
                    internal::MethodKind::Get => parts.push(doc::text("get ")),
                    internal::MethodKind::Set => parts.push(doc::text("set ")),
                    _ => {}
                }
                if method.computed {
                    parts.push(doc::text("["));
                    parts.push(self.build_expression_doc(&method.key));
                    parts.push(doc::text("]"));
                } else {
                    parts.push(self.build_expression_doc(&method.key));
                }

                // Handle comments around method signature parts
                // Comments between key and type_params/`(` go before `?`
                // Comments between type_params and `(` go after type_params
                let key_end = method.key.span().end;
                let type_params_end = method.type_parameters.as_ref().map(|tp| tp.span.end);

                // Find the position of `(` in source (search after type_params if present)
                let paren_search_start = type_params_end.unwrap_or(key_end);
                let paren_pos = self.source[paren_search_start as usize..]
                    .find('(')
                    .map(|p| paren_search_start + p as u32);

                // Comments between key and type_params (or `(` if no type_params) go before `?`
                let comments_before_boundary = type_params_end.or(paren_pos).unwrap_or(key_end);
                for comment in comments_in_range(self.comments, key_end, comments_before_boundary) {
                    parts.push(doc::text(" "));
                    parts.push(self.build_comment_doc(comment));
                }

                if method.optional {
                    parts.push(doc::text("?"));
                }
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &method.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }

                // Comments between type_params and `(` go after type_params
                if let (Some(tp_end), Some(paren_pos)) = (type_params_end, paren_pos) {
                    for comment in comments_in_range(self.comments, tp_end, paren_pos) {
                        parts.push(doc::text(" "));
                        parts.push(self.build_comment_doc(comment));
                    }
                }

                parts.push(self.build_signature_params_doc(&method.params, paren_pos));
                if let Some(return_type) = &method.return_type {
                    parts.push(self.build_signature_return_type_doc(paren_pos, return_type));
                }
                if with_semicolon {
                    parts.push(doc::text(";"));
                }
                doc::group(doc::concat(parts))
            }
            TSTypeElement::CallSignature(call) => {
                let mut parts = vec![];
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &call.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }

                // Find paren position for comment handling
                let paren_search_start = call
                    .type_parameters
                    .as_ref()
                    .map_or(call.span.start, |tp| tp.span.end);
                let paren_pos = self.source[paren_search_start as usize..]
                    .find('(')
                    .map(|p| paren_search_start + p as u32);

                parts.push(self.build_signature_params_doc(&call.params, paren_pos));
                if let Some(return_type) = &call.return_type {
                    parts.push(self.build_signature_return_type_doc(paren_pos, return_type));
                }
                if with_semicolon {
                    parts.push(doc::text(";"));
                }
                doc::group(doc::concat(parts))
            }
            TSTypeElement::ConstructSignature(ctor) => {
                let mut parts = vec![doc::text("new ")];
                // Print type parameters if present: `<T>` or `<T, U>`
                if let Some(type_params) = &ctor.type_parameters {
                    parts.push(self.build_type_parameter_declaration_doc(type_params));
                }

                // Find paren position for comment handling
                let paren_search_start = ctor
                    .type_parameters
                    .as_ref()
                    .map_or(ctor.span.start, |tp| tp.span.end);
                let paren_pos = self.source[paren_search_start as usize..]
                    .find('(')
                    .map(|p| paren_search_start + p as u32);

                parts.push(self.build_signature_params_doc(&ctor.params, paren_pos));
                if let Some(return_type) = &ctor.return_type {
                    parts.push(self.build_signature_return_type_doc(paren_pos, return_type));
                }
                if with_semicolon {
                    parts.push(doc::text(";"));
                }
                doc::group(doc::concat(parts))
            }
            TSTypeElement::IndexSignature(idx) => {
                let mut parts = vec![];
                if idx.readonly {
                    parts.push(doc::text("readonly "));
                }

                // Build the key parameter docs
                // For key type annotations with unions/intersections, use special handling
                // so they break properly with leading |/trailing & style.
                let param_docs: Vec<_> =
                    idx.parameters
                        .iter()
                        .map(|param| {
                            let mut param_parts = vec![doc::symbol(param.name.to_u32())];
                            if let Some(type_ann) = &param.type_annotation {
                                match type_ann.type_annotation.as_ref() {
                                    TSType::Union(u) => {
                                        // Union key type: break after `:` with leading `|`
                                        let type_doc = self.build_union_type_doc(u, false);
                                        param_parts.push(doc::text(":"));
                                        param_parts.push(doc::group(doc::indent(doc::concat(
                                            vec![doc::line(), type_doc],
                                        ))));
                                    }
                                    TSType::Intersection(i) => {
                                        // Intersection key type: break after `:` with trailing `&`
                                        let type_doc = self.build_intersection_type_doc(i, false);
                                        param_parts.push(doc::text(":"));
                                        param_parts.push(doc::group(doc::indent(doc::concat(
                                            vec![doc::line(), type_doc],
                                        ))));
                                    }
                                    _ => {
                                        // Regular key type: use standard annotation
                                        param_parts.push(self.build_type_annotation_doc(type_ann));
                                    }
                                }
                            }
                            doc::concat(param_parts)
                        })
                        .collect();

                // Build `[key: type]` as a group that can break when key type is long
                // Flat: [key: type]
                // Break: [\n\tkey: type\n]
                let bracket_contents = doc::join(param_docs, ", ");
                let bracket_group = doc::group(doc::concat(vec![
                    doc::text("["),
                    doc::indent_softline(bracket_contents),
                    doc::softline(),
                    doc::text("]"),
                ]));
                parts.push(bracket_group);

                // Handle comments between `]` and type annotation
                // Search for `]` from the last parameter's end position
                let search_start = idx.parameters.last().map_or(idx.span.start, |p| p.span.end);
                let bracket_close_pos = self.source[search_start as usize..]
                    .find(']')
                    .map(|p| search_start + p as u32);
                let type_start = idx.type_annotation.type_annotation.span().start;
                let mut has_comment = false;
                if let Some(close_pos) = bracket_close_pos {
                    // Search up to the type's span start, not the annotation's
                    for comment in comments_in_range(self.comments, close_pos + 1, type_start) {
                        parts.push(doc::text(" "));
                        parts.push(self.build_comment_doc(comment));
                        has_comment = true;
                    }
                }

                // Build value type annotation with proper breaking for long unions/intersections
                // When we have a comment before `:`, we handle `:` ourselves to avoid
                // build_type_annotation_doc outputting the comment again.
                if has_comment {
                    // We already output the comment, now just add ` : Type`
                    parts.push(doc::text(" : "));
                    parts.push(self.build_type_doc(&idx.type_annotation.type_annotation));
                } else {
                    // No comment before `:`, use normal type annotation handling
                    match idx.type_annotation.type_annotation.as_ref() {
                        TSType::Union(u) => {
                            let type_doc = self.build_union_type_doc(u, false);
                            parts.push(doc::text(":"));
                            parts.push(doc::group(doc::indent(doc::concat(vec![
                                doc::line(), // space when flat, newline when broken
                                type_doc,
                            ]))));
                        }
                        TSType::Intersection(i) => {
                            let type_doc = self.build_intersection_type_doc(i, false);
                            if intersection_has_huggable_last_type(i) {
                                // No indent/line - keep `: Type & {` hugged
                                parts.push(doc::text(": "));
                                parts.push(type_doc);
                            } else {
                                parts.push(doc::text(":"));
                                parts.push(doc::group(doc::indent(doc::concat(vec![
                                    doc::line(),
                                    type_doc,
                                ]))));
                            }
                        }
                        _ => {
                            parts.push(self.build_type_annotation_doc(&idx.type_annotation));
                        }
                    }
                }

                if with_semicolon {
                    parts.push(doc::text(";"));
                }
                doc::concat(parts)
            }
        }
    }

    /// Build a Doc for a type literal (object type): `{ a: T; b: U }`
    ///
    /// Handles both single-line and multi-line formats:
    /// - Single-line source stays single-line if it fits: `{ a: T; b: U }`
    /// - Multi-line source (newline after `{`) stays multi-line
    /// - Comments force multi-line formatting
    fn build_type_literal_doc(&self, t: &TSTypeLiteral) -> Doc {
        let force_multiline = self.type_literal_force_multiline(t);

        let mut parts = vec![doc::text("{")];
        if !t.members.is_empty() {
            if force_multiline {
                // Multi-line format with leading comment handling (forced by source)
                let mut member_parts = vec![];
                let mut prev_end = t.span.start + 1; // after opening brace
                for (i, m) in t.members.iter().enumerate() {
                    let is_first = i == 0;
                    let member_end = m.span().end;

                    // Build leading comments (excluding same-line trailing comments from prev member)
                    let all_comments: Vec<_> =
                        comments_in_range(self.comments, prev_end, m.span().start).collect();
                    let leading_comments: Vec<_> = if !is_first {
                        all_comments
                            .iter()
                            .filter(|c| !is_same_line(self.source, prev_end, c.span.start))
                            .copied()
                            .collect()
                    } else {
                        all_comments
                    };

                    // Check for blank lines before this member
                    let has_blank = if !leading_comments.is_empty() {
                        tsv_lang::printing::has_blank_line_between(
                            self.source,
                            prev_end,
                            leading_comments[0].span.start,
                        )
                    } else {
                        tsv_lang::printing::has_blank_line_between(
                            self.source,
                            prev_end,
                            m.span().start,
                        )
                    };

                    // Add separator before this member
                    // For first member: just hardline
                    // For other members: literalline + hardline if blank line, just hardline otherwise
                    if has_blank && !is_first {
                        member_parts.push(doc::literalline());
                    }
                    // Always add hardline before member (or its leading comments)
                    member_parts.push(doc::hardline());

                    // Print leading comments with blank line preservation
                    member_parts.extend(self.build_leading_comments_with_blank_lines(
                        &leading_comments,
                        m.span().start,
                    ));
                    // Print member WITHOUT semicolon so we can add trailing comments first
                    member_parts.push(self.build_type_member_doc_inner(m, false));

                    // Handle trailing same-line comments after member
                    // Block comments go before semicolon: `a: A /* block */;`
                    // Line comments go after semicolon: `a: A; // line`
                    let upper_bound = t
                        .members
                        .get(i + 1)
                        .map_or(t.span.end, |next| next.span().start);
                    let trailing: Vec<_> =
                        comments_in_range(self.comments, member_end, upper_bound)
                            .filter(|c| is_same_line(self.source, member_end, c.span.start))
                            .collect();

                    // Block comments before semicolon
                    for comment in trailing.iter().filter(|c| c.is_block) {
                        member_parts.push(doc::text(" "));
                        member_parts.push(self.build_comment_doc(comment));
                    }

                    // Semicolon
                    member_parts.push(doc::text(";"));

                    // Line comments after semicolon
                    for comment in trailing.iter().filter(|c| !c.is_block) {
                        member_parts.push(doc::text(" "));
                        member_parts.push(self.build_comment_doc(comment));
                    }

                    prev_end = member_end;
                }

                // Handle trailing comments after the last member (before closing `}`)
                let body_end = t.span.end.saturating_sub(1);
                member_parts.extend(self.build_trailing_body_comments_doc(prev_end, body_end));

                parts.push(doc::indent(doc::concat(member_parts)));
                parts.push(doc::hardline());
            } else {
                // Width-aware format: stays inline if fits, wraps if too long
                // Flat: {prop: string; prop2: number}
                // Broken: {\n\tprop: string;\n\tprop2: number;\n}
                let mut member_parts = vec![];
                for (i, m) in t.members.iter().enumerate() {
                    let is_last = i == t.members.len() - 1;
                    let member_end = m.span().end;

                    // Add line break before each member (softline for width-aware)
                    member_parts.push(doc::softline());
                    member_parts.push(self.build_type_member_doc_inner(m, false));

                    // Handle trailing comments between member and semicolon
                    let upper_bound = t
                        .members
                        .get(i + 1)
                        .map_or(t.span.end, |next| next.span().start);
                    for comment in comments_in_range(self.comments, member_end, upper_bound) {
                        member_parts.push(doc::text(" "));
                        member_parts.push(self.build_comment_doc(comment));
                    }

                    if is_last {
                        // Last member: semicolon only when broken
                        member_parts.push(doc::if_break(doc::text(";"), doc::text("")));
                    } else {
                        // Non-last: semicolon always, space only when flat
                        member_parts.push(doc::if_break(doc::text(";"), doc::text("; ")));
                    }
                }
                parts.push(doc::indent(doc::concat(member_parts)));
                parts.push(doc::softline());
            }
        }
        parts.push(doc::text("}"));
        // Wrap in group for width-aware breaking
        doc::group(doc::concat(parts))
    }

    /// Build ` => ReturnType` doc for function/constructor types.
    ///
    /// For union return types, uses break-after-arrow layout:
    /// ```text
    /// =>
    ///     | Type1
    ///     | Type2
    /// ```
    ///
    /// For intersection return types, uses trailing `&` with indented continuations:
    /// ```text
    /// => Type1 &
    ///     Type2
    /// ```
    fn build_function_type_return_doc(&self, return_type: &internal::TSTypeAnnotation) -> Doc {
        match return_type.type_annotation.as_ref() {
            TSType::Union(u) => {
                let type_doc = self.build_union_type_doc(u, false);
                doc::concat(vec![
                    doc::text(" =>"),
                    doc::group(doc::indent(doc::concat(vec![doc::line(), type_doc]))),
                ])
            }
            TSType::Intersection(i) => {
                // Intersections use trailing `&` - first type NOT indented, continuations indented
                // The intersection doc handles this internally, we just need proper grouping
                let type_doc = self.build_intersection_type_doc(i, false);
                doc::concat(vec![doc::text(" => "), doc::group(doc::indent(type_doc))])
            }
            _ => doc::concat(vec![
                doc::text(" => "),
                self.build_type_doc(&return_type.type_annotation),
            ]),
        }
    }

    /// Build a Doc for a function type: `(a: T) => U`
    ///
    /// Uses width-aware wrapping similar to arrow functions.
    fn build_function_type_doc(&self, f: &TSFunctionType) -> Doc {
        let mut parts = Vec::new();

        // Type parameters wrapped in their own group (can break independently)
        if let Some(type_params) = &f.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Function parameters - search for `(` from type_params end or function start
        let paren_search_start = f
            .type_parameters
            .as_ref()
            .map_or(f.span.start, |tp| tp.span.end);
        parts.extend(self.build_function_params_doc(&f.params, paren_search_start));
        parts.push(self.build_function_type_return_doc(&f.return_type));

        // Wrap entire function type in a group for width-aware breaking
        doc::group(doc::concat(parts))
    }

    /// Build a Doc for a constructor type: `new () => T` or `abstract new <T>() => T`
    fn build_constructor_type_doc(&self, c: &TSConstructorType) -> Doc {
        let mut parts = Vec::new();

        // Add 'abstract' keyword if present
        if c.abstract_ {
            parts.push(doc::text("abstract "));
        }

        // Add 'new' keyword
        parts.push(doc::text("new "));

        // Type parameters wrapped in their own group (can break independently)
        if let Some(type_params) = &c.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Constructor parameters - search for `(` from type_params end or constructor start
        let paren_search_start = c
            .type_parameters
            .as_ref()
            .map_or(c.span.start, |tp| tp.span.end);
        parts.extend(self.build_function_params_doc(&c.params, paren_search_start));
        parts.push(self.build_function_type_return_doc(&c.return_type));

        // Wrap entire constructor type in a group for width-aware breaking
        doc::group(doc::concat(parts))
    }

    /// Build a Doc for a tuple type: `[A, B, C]`
    ///
    /// Uses width-aware breaking: inline if fits, one element per line if not.
    fn build_tuple_type_doc(&self, t: &TSTupleType) -> Doc {
        if t.element_types.is_empty() {
            return doc::text("[]");
        }

        // Check for line comments between elements or after last element (force multiline)
        if self.has_line_comments_in_delimited_list(&t.element_types, TSType::span, t.span.end - 1)
        {
            return self.build_tuple_type_doc_with_line_comments(t);
        }

        // Build element docs with commas and line breaks
        let mut parts = Vec::new();
        for (i, elem) in t.element_types.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(","));
                parts.push(doc::line());
            }
            parts.push(self.build_type_doc(elem));
        }

        // Width-aware breaking: inline if fits, one-per-line if not
        let inner = doc::concat(vec![
            doc::softline(),
            doc::concat(parts),
            doc::trailing_comma(),
        ]);

        doc::group(doc::concat(vec![
            doc::text("["),
            doc::indent(inner),
            doc::softline(),
            doc::text("]"),
        ]))
    }

    /// Build tuple type with line comments between elements
    fn build_tuple_type_doc_with_line_comments(&self, t: &TSTupleType) -> Doc {
        let mut inner_parts = Vec::new();
        let mut prev_end = t.span.start + 1; // After the opening `[`

        for (i, elem) in t.element_types.iter().enumerate() {
            let elem_start = elem.span().start;
            let elem_end = elem.span().end;
            let is_last = i == t.element_types.len() - 1;

            // Leading comments
            inner_parts.extend(self.build_leading_comments_multiline(prev_end, elem_start));

            inner_parts.push(self.build_type_doc(elem));

            let next_boundary = if i + 1 < t.element_types.len() {
                t.element_types[i + 1].span().start
            } else {
                t.span.end - 1 // Before the closing `]`
            };

            // Trailing comma for all elements
            inner_parts.push(doc::text(","));

            // Trailing comments
            inner_parts.extend(self.build_trailing_comments_multiline(elem_end, next_boundary));

            // Hardline to separate from next element
            if !is_last {
                inner_parts.push(doc::hardline());
            }

            prev_end = next_boundary;
        }

        doc::concat(vec![
            doc::text("["),
            doc::indent(doc::concat(vec![doc::hardline(), doc::concat(inner_parts)])),
            doc::hardline(),
            doc::text("]"),
        ])
    }

    /// Build a Doc for an array type (e.g., `number[]`)
    fn build_array_type_doc(&self, arr: &TSArrayType) -> Doc {
        let needs_parens = type_needs_parens_for_array_element(&arr.element_type);
        let element_doc = self.build_type_doc(&arr.element_type);
        if needs_parens {
            doc::concat(vec![doc::text("("), element_doc, doc::text(")[]")])
        } else {
            doc::concat(vec![element_doc, doc::text("[]")])
        }
    }

    /// Find closing paren position given opening paren position
    fn find_close_paren(&self, paren_pos: u32) -> Option<u32> {
        self.source[paren_pos as usize + 1..]
            .find(')')
            .map(|p| paren_pos + 1 + p as u32)
    }

    /// Build return type annotation with comment handling between `)` and `:`
    /// Used by MethodSignature, CallSignature, ConstructSignature
    fn build_signature_return_type_doc(
        &self,
        paren_pos: Option<u32>,
        return_type: &internal::TSTypeAnnotation,
    ) -> Doc {
        let mut parts = vec![];
        let mut has_comment = false;

        if let Some(paren_pos) = paren_pos
            && let Some(close_pos) = self.find_close_paren(paren_pos)
        {
            for comment in comments_in_range(self.comments, close_pos + 1, return_type.span.start) {
                parts.push(doc::text(" "));
                parts.push(self.build_comment_doc(comment));
                has_comment = true;
            }
        }

        // Prettier adds space before `:` when there's a preceding comment
        if has_comment {
            parts.push(doc::text(" "));
        }
        parts.push(self.build_type_annotation_doc(return_type));
        doc::concat(parts)
    }

    /// Build signature params doc with width-based breaking.
    ///
    /// Inline: `(param1: Type1, param2: Type2)`
    /// Broken: `(\n\tparam1: Type1,\n\tparam2: Type2,\n)`
    ///
    /// Used by MethodSignature, CallSignature, ConstructSignature in both
    /// TypeLiteral and interface contexts.
    pub(super) fn build_signature_params_doc(
        &self,
        params: &[internal::Expression],
        paren_pos: Option<u32>,
    ) -> Doc {
        if params.is_empty() {
            // Handle comments inside empty params (e.g., `a(/* comment */): void`)
            if let Some(paren_pos) = paren_pos
                && let Some(close_pos) = self.find_close_paren(paren_pos)
            {
                let mut parts = vec![doc::text("(")];
                for comment in comments_in_range(self.comments, paren_pos + 1, close_pos) {
                    parts.push(self.build_comment_doc(comment));
                }
                parts.push(doc::text(")"));
                return doc::concat(parts);
            }
            return doc::text("()");
        }

        // Build params with width-based breaking
        let mut param_parts = Vec::new();

        // Handle comments before first param (e.g., `(/* comment */ a: T)`)
        if let Some(paren_pos) = paren_pos {
            let first_param_start = params[0].span().start;
            for comment in comments_in_range(self.comments, paren_pos + 1, first_param_start) {
                param_parts.push(self.build_comment_doc(comment));
                param_parts.push(doc::text(" "));
            }
        }

        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                param_parts.push(doc::text(","));
                param_parts.push(doc::line());
            }
            param_parts.push(self.build_function_type_param_expression_doc(param));

            // Handle trailing comments after this param
            let param_end = param.span().end;
            let next_boundary = if i + 1 < params.len() {
                params[i + 1].span().start
            } else {
                paren_pos
                    .and_then(|p| self.find_close_paren(p))
                    .unwrap_or(param_end)
            };

            for comment in comments_in_range(self.comments, param_end, next_boundary) {
                param_parts.push(doc::text(" "));
                param_parts.push(self.build_comment_doc(comment));
            }
        }

        // Check if last param is rest element (no trailing comma)
        let last_is_rest = params
            .last()
            .is_some_and(|p| matches!(p, internal::Expression::RestElement(_)));

        let mut parts = vec![doc::text("(")];
        parts.push(doc::indent(doc::concat(vec![
            doc::softline(),
            doc::concat(param_parts),
        ])));
        if !last_is_rest {
            parts.push(doc::trailing_comma());
        }
        parts.push(doc::softline());
        parts.push(doc::text(")"));

        // Wrap in group so params break independently of outer context
        doc::group(doc::concat(parts))
    }

    /// Build a Doc for a function type parameter expression with wrapping type annotations.
    ///
    /// For Identifiers, uses wrapping type annotations so generic type arguments
    /// break at print width (e.g., `param: Map<LongA, LongB>` breaks inside `<>`).
    pub(super) fn build_function_type_param_expression_doc(&self, expr: &internal::Expression) -> Doc {
        match expr {
            internal::Expression::Identifier(id) => self.build_identifier_doc_with_wrapping_type(id),
            internal::Expression::RestElement(rest) => doc::concat(vec![
                doc::text("..."),
                self.build_function_type_param_expression_doc(&rest.argument),
            ]),
            _ => self.build_expression_doc(expr),
        }
    }

    /// Build parameter list docs for function/constructor types
    /// Returns docs that should be pushed to a parts vector
    fn build_function_params_doc(
        &self,
        params: &[internal::Expression],
        paren_search_start: u32,
    ) -> Vec<Doc> {
        let mut parts = Vec::new();

        // Find paren position for comment handling
        let paren_pos = self.source[paren_search_start as usize..]
            .find('(')
            .map(|p| paren_search_start + p as u32);

        if params.is_empty() {
            parts.push(doc::text("()"));
        } else {
            // Check for line comments between parameters or after last parameter (force multiline)
            let close_paren_pos = paren_pos.and_then(|p| self.find_close_paren(p));
            // Use last param end as fallback if close paren not found (no trailing check)
            let end_boundary = close_paren_pos
                .unwrap_or_else(|| params.last().map_or(0, |p| p.span().end));
            if self.has_line_comments_in_delimited_list(params, internal::Expression::span, end_boundary) {
                return self.build_function_params_doc_with_line_comments(params, paren_pos);
            }

            let mut param_parts = Vec::new();
            for (i, p) in params.iter().enumerate() {
                if i > 0 {
                    param_parts.push(doc::text(","));
                    param_parts.push(doc::line());
                }
                param_parts.push(self.build_function_type_param_expression_doc(p));

                // Handle trailing comments after this param
                let param_end = p.span().end;
                let next_boundary = if i + 1 < params.len() {
                    params[i + 1].span().start
                } else {
                    paren_pos
                        .and_then(|p| self.find_close_paren(p))
                        .unwrap_or(param_end)
                };

                for comment in comments_in_range(self.comments, param_end, next_boundary) {
                    param_parts.push(doc::text(" "));
                    param_parts.push(self.build_comment_doc(comment));
                }
            }
            parts.push(doc::text("("));
            parts.push(doc::indent(doc::concat(vec![
                doc::softline(),
                doc::concat(param_parts),
            ])));
            // Trailing comma when breaking, UNLESS last param is a rest element
            let last_is_rest = params
                .last()
                .is_some_and(|p| matches!(p, internal::Expression::RestElement(_)));
            if !last_is_rest {
                parts.push(doc::trailing_comma());
            }
            parts.push(doc::softline());
            parts.push(doc::text(")"));
        }
        parts
    }

    /// Build function params with line comments between them (forces multiline)
    fn build_function_params_doc_with_line_comments(
        &self,
        params: &[internal::Expression],
        paren_pos: Option<u32>,
    ) -> Vec<Doc> {
        let mut parts = Vec::new();
        let mut inner_parts = Vec::new();

        let open_paren = paren_pos.unwrap_or(0);
        let mut prev_end = open_paren + 1; // After `(`

        for (i, p) in params.iter().enumerate() {
            let param_start = p.span().start;
            let param_end = p.span().end;
            let is_last = i == params.len() - 1;

            // Leading comments
            inner_parts.extend(self.build_leading_comments_multiline(prev_end, param_start));

            inner_parts.push(self.build_function_type_param_expression_doc(p));

            let next_boundary = if i + 1 < params.len() {
                params[i + 1].span().start
            } else {
                paren_pos
                    .and_then(|p| self.find_close_paren(p))
                    .unwrap_or(param_end)
            };

            // Comma (trailing comma for all, unless last param is rest element)
            let is_rest = matches!(p, internal::Expression::RestElement(_));
            if !is_last || !is_rest {
                inner_parts.push(doc::text(","));
            }

            // Trailing comments
            inner_parts.extend(self.build_trailing_comments_multiline(param_end, next_boundary));

            // Hardline to separate from next element
            if !is_last {
                inner_parts.push(doc::hardline());
            }

            prev_end = next_boundary;
        }

        parts.push(doc::text("("));
        parts.push(doc::indent(doc::concat(vec![
            doc::hardline(),
            doc::concat(inner_parts),
        ])));
        parts.push(doc::hardline());
        parts.push(doc::text(")"));
        parts
    }

    /// Build a Doc for a literal type
    fn build_literal_type_doc(&self, lit: &TSLiteralType) -> Doc {
        match lit {
            TSLiteralType::TemplateLiteral(template) => {
                self.build_template_literal_type_doc(template)
            }
            TSLiteralType::String(literal) => self.build_literal_doc(literal),
            TSLiteralType::Number(literal) => self.build_literal_doc(literal),
            TSLiteralType::BigInt(literal) => self.build_literal_doc(literal),
            TSLiteralType::UnaryExpression(unary) => {
                // For negative number types like `-1`
                let op = doc::text(unary.operator.as_str());
                let arg = self.build_expression_doc(&unary.argument);
                doc::concat(vec![op, arg])
            }
        }
    }

    /// Build a Doc for a template literal type
    ///
    /// Divergence: When the type exceeds print width, we break after `${` with `}` on its own
    /// line. Prettier keeps template literal types inline regardless of length. For conditional
    /// types, we break at `?/:` operators instead (the conditional's natural break points).
    ///
    /// Breaking decision is based on flat-layout positions: we pre-compute which interpolations
    /// would exceed print width in flat mode, then break all of those. This ensures consistent
    /// formatting - types that would exceed at their original positions break, even if earlier
    /// breaks would have given them more room.
    fn build_template_literal_type_doc(&self, template: &TemplateLiteralType) -> Doc {
        let print_width = self.config.print_width;

        // First pass: analyze types and determine which exceed print width at their flat positions
        // Store as (doc, flat_str, is_conditional, exceeds_width)
        let mut type_data: Vec<(Doc, String, bool, bool)> =
            Vec::with_capacity(template.types.len());
        let mut pos: usize = 1; // Start after backtick

        for (i, quasi) in template.quasis.iter().enumerate() {
            pos += quasi.raw.len();
            if i < template.types.len() {
                let t = &template.types[i];
                let is_conditional = matches!(t, TSType::Conditional(_));
                let type_doc = self.build_type_doc(t);
                let flat_str = self.render_doc_flat(&type_doc);

                // Position includes: current pos + "${" (2) + type + "}" (1)
                let interp_end = pos + 2 + flat_str.len() + 1;
                let exceeds_width = interp_end > print_width;

                type_data.push((type_doc, flat_str, is_conditional, exceeds_width));
                pos = interp_end;
            }
        }

        // Second pass: build doc with breaking decisions already made
        let mut parts = vec![doc::text("`")];
        let mut type_iter = type_data.into_iter();

        for quasi in &template.quasis {
            parts.push(doc::text_owned(quasi.raw.clone()));
            if let Some((type_doc, flat_str, is_conditional, exceeds_width)) = type_iter.next() {
                // Use relative indent() for positioning within the current context.
                // The template is already at some indent level (e.g., after = break in type alias).
                // Content gets +1 indent, closing stays at current level.
                let interp_doc = if is_conditional {
                    // Conditional types: wrap in group - breaks happen at ?/: operators
                    // Don't add extra indent - conditional type's own formatting handles branch indentation
                    doc::concat(vec![doc::text("${"), doc::group(type_doc), doc::text("}")])
                } else if exceeds_width {
                    // Exceeds print width at flat position - always break
                    doc::concat(vec![
                        doc::text("${"),
                        doc::indent(doc::concat(vec![doc::hardline(), type_doc])),
                        doc::hardline(),
                        doc::text("}"),
                    ])
                } else {
                    // Short enough - try flat first, break if doesn't fit at actual position
                    doc::conditional_group(vec![
                        doc::concat(vec![
                            doc::text("${"),
                            doc::text_owned(flat_str),
                            doc::text("}"),
                        ]),
                        doc::concat(vec![
                            doc::text("${"),
                            doc::indent(doc::concat(vec![doc::line(), type_doc])),
                            doc::line(),
                            doc::text("}"),
                        ]),
                    ])
                };
                parts.push(interp_doc);
            }
        }
        parts.push(doc::text("`"));
        doc::concat(parts)
    }

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
    pub(super) fn build_union_type_doc(&self, union: &TSUnionType, wrap_in_group: bool) -> Doc {
        if union.types.is_empty() {
            return doc::text("");
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
                parts.push(doc::if_break(
                    doc::concat(vec![doc::line(), doc::text("| ")]),
                    doc::text(" | "),
                ));

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
                parts.push(doc::if_break(doc::text("| "), doc::text("")));
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
            doc::group(doc::concat(parts))
        } else {
            // Inherit breaking from parent group
            doc::concat(parts)
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
    fn build_union_type_doc_with_line_comments(&self, union: &TSUnionType) -> Doc {
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
                    parts.push(doc::hardline());
                    parts.push(doc::text("| "));

                    // Comments after the pipe (leading on this type)
                    parts.extend(self.build_leading_comments_multiline(pipe_pos + 1, type_start));
                } else {
                    // No pipe found, just add separator
                    parts.push(doc::hardline());
                    parts.push(doc::text("| "));
                }
            } else {
                // First type: always has `| ` prefix when multiline
                parts.push(doc::text("| "));
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
                    parts.push(doc::text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
            }
        }

        doc::concat(parts)
    }

    /// Build a Doc for an intersection type: `A & B & C` or `A &\n\tB &\n\tC`
    ///
    /// Prettier formatting for intersection types differs from union types:
    /// - Flat: `A & B & C`
    /// - Break: `A &\n\tB &\n\tC` (trailing `&`, continuation indented)
    ///
    /// When `wrap_in_group` is true (default), wraps in its own group for
    /// independent breaking decisions. When false, inherits from parent.
    pub(super) fn build_intersection_type_doc(
        &self,
        intersection: &TSIntersectionType,
        wrap_in_group: bool,
    ) -> Doc {
        if intersection.types.is_empty() {
            return doc::text("");
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
            first_parts.push(doc::text(" &"));
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
                continuation_parts.push(doc::text(" "));
            } else {
                continuation_parts.push(doc::line());
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
                continuation_parts.push(doc::text(" &"));
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
        if !continuation_parts.is_empty() {
            let has_non_huggable_continuations = last_is_huggable && intersection.types.len() > 2;
            if has_non_huggable_continuations {
                // Multiple continuations with huggable at end - wrap in indent
                parts.push(doc::indent(doc::concat(continuation_parts)));
            } else {
                // Either huggable-only or no huggable - no internal indent
                parts.extend(continuation_parts);
            }
        }

        if wrap_in_group {
            // Independent breaking decision
            doc::group(doc::concat(parts))
        } else {
            // Inherit breaking from parent group
            doc::concat(parts)
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
    ) -> Doc {
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
                        .filter(|c| is_same_line(self.source, amp_pos, c.span.start))
                    {
                        parts.push(doc::text(" "));
                        parts.push(self.build_comment_doc(comment));
                    }

                    // Newline for continuation
                    parts.push(doc::hardline());

                    // Leading comments on their own line (come after hardline)
                    for comment in comments_after_amp
                        .iter()
                        .filter(|c| !is_same_line(self.source, amp_pos, c.span.start))
                    {
                        parts.push(self.build_comment_doc(comment));
                        if comment.is_block {
                            parts.push(doc::text(" "));
                        } else {
                            parts.push(doc::hardline());
                        }
                    }
                } else {
                    // No ampersand found, just add newline
                    parts.push(doc::hardline());
                }
            }

            // Add the type
            parts.push(self.build_type_doc_maybe_parens(t, type_needs_parens_in_intersection));

            // Add trailing `&` for all but last type
            if i < intersection.types.len() - 1 {
                parts.push(doc::text(" &"));
            } else {
                // Trailing comments on last type
                for comment in comments_in_range(self.comments, type_end, intersection.span.end) {
                    parts.push(doc::text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
            }
        }

        doc::concat(parts)
    }
}
