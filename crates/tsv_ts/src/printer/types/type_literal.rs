// Type literal printing for TypeScript
//
// Handles printing of object type literals (`{ a: T; b: U }`) with:
// - Single-line and multi-line formats
// - Object alignment for union members and parenthesized intersections
// - "Hugging" behavior for type arguments

use super::super::comments_in_range;
use super::Printer;
use super::helpers::unwrap_parenthesized;
use crate::ast::internal::{TSIntersectionType, TSType, TSTypeElement, TSTypeLiteral, TSUnionType};
use tsv_lang::doc::arena::DocId;

/// Mode for building type literal docs.
enum TypeLiteralMode {
    /// Inline `; ` separators, no softlines, no group (for type args)
    Hugging,
    /// Width-aware with softlines, wrapped in group
    Standard,
    /// Width-aware with softlines, no group (parent controls breaking)
    NoGroup,
}

impl<'a> Printer<'a> {
    //
    // Comment partitioning helpers
    //

    /// Build docs for leading comments with blank line preservation in multiline format.
    ///
    /// Returns docs for: `[literalline if blank && !is_first] hardline [leading comments]`
    ///
    /// For non-first members, filters out same-line comments (they belong to the previous member).
    fn build_multiline_member_prefix_doc(
        &self,
        prev_end: u32,
        member_start: u32,
        is_first: bool,
    ) -> Vec<DocId> {
        let d = self.d();
        let all_comments: Vec<_> =
            comments_in_range(self.comments, prev_end, member_start).collect();
        let leading_comments: Vec<_> = if !is_first {
            all_comments
                .iter()
                .filter(|c| !self.is_same_line(prev_end, c.span.start))
                .copied()
                .collect()
        } else {
            all_comments
        };

        let has_blank = if !leading_comments.is_empty() {
            self.has_blank_line_between(prev_end, leading_comments[0].span.start)
        } else {
            self.has_blank_line_between(prev_end, member_start)
        };

        let mut docs = Vec::with_capacity(3);
        if has_blank && !is_first {
            docs.push(d.literalline());
        }
        docs.push(d.hardline());
        docs.extend(self.build_leading_comments_with_blank_lines(&leading_comments, member_start));
        docs
    }

    /// Build docs for trailing comments partitioned around a semicolon.
    ///
    /// Returns docs for: `[space + comment]* ";" [space + comment]*`
    ///
    /// Comments are positioned relative to the first semicolon found in the
    /// source range `member_end..upper_bound`. If no semicolon exists in source,
    /// all comments are placed before the semicolon.
    fn build_comments_around_semicolon_doc(
        &self,
        comments: &[&tsv_lang::Comment],
        member_end: u32,
        upper_bound: u32,
    ) -> Vec<DocId> {
        let d = self.d();
        let source_slice = &self.source[member_end as usize..upper_bound as usize];
        let semi_offset = source_slice.find(';');

        let (before_semi, after_semi): (Vec<_>, Vec<_>) =
            comments.iter().partition(|c| match semi_offset {
                Some(offset) => {
                    let semi_pos = member_end + offset as u32;
                    c.span.start < semi_pos
                }
                None => true, // No semicolon in source, all comments are "before"
            });

        let mut docs = Vec::with_capacity(before_semi.len() + after_semi.len() + 1);
        for comment in before_semi {
            docs.push(d.text(" "));
            docs.push(self.build_comment_doc(comment));
        }
        docs.push(d.text(";"));
        for comment in after_semi {
            docs.push(d.text(" "));
            docs.push(self.build_comment_doc(comment));
        }
        docs
    }

    //
    // Type parenthesization with special object handling
    //

    /// Build type doc, wrapping in parentheses if the predicate returns true.
    ///
    /// Uses `align_spaces(2, ...)` for proper Prettier-style alignment:
    /// - Object properties get pure tabs (via double indent)
    /// - Closing `})` gets 2-space alignment after tabs
    ///
    /// Special case: intersection with trailing object type builds a custom doc
    /// so that `})` can be aligned properly (at base indent + 2 spaces).
    pub(super) fn build_type_doc_maybe_parens(
        &self,
        ts_type: &TSType,
        needs_parens: fn(&TSType) -> bool,
    ) -> DocId {
        let d = self.d();
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

            // Special case: parenthesized union type
            if let TSType::Union(union) = unwrap_parenthesized(ts_type) {
                return self.build_parenthesized_union_doc(union);
            }

            // Default case: simple parenthesization
            d.concat(&[
                d.text("("),
                d.align_spaces(2, d.indent(self.build_type_doc(ts_type))),
                d.text(")"),
            ])
        } else {
            self.build_type_doc(ts_type)
        }
    }

    /// Build doc for a union type wrapped in parentheses.
    ///
    /// Prettier uses `group([indent(mainParts), softline])` when `pathNeedsParens`
    /// is true for a union, so that when the group breaks, `(` and `)` get their
    /// own lines with the union content indented:
    /// ```text
    /// (
    ///   | { a: string }
    ///   | { b: string }
    /// )
    /// ```
    pub(super) fn build_parenthesized_union_doc(&self, union: &TSUnionType) -> DocId {
        let d = self.d();
        let union_doc = self.build_union_type_doc(union, false);
        let inner =
            d.group(d.concat(&[d.indent(d.concat(&[d.softline(), union_doc])), d.softline()]));
        d.concat(&[d.text("("), inner, d.text(")")])
    }

    //
    // Object alignment helpers (for unions and parenthesized intersections)
    //

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
    ) -> DocId {
        let d = self.d();
        // Build opening: (A & B & {
        let mut opening_parts = vec![d.text("(")];

        // Build intersection types except the last one (the object)
        let types_before_object = &intersection.types[..intersection.types.len() - 1];
        for (i, t) in types_before_object.iter().enumerate() {
            if i > 0 {
                opening_parts.push(d.text(" & "));
            }
            opening_parts.push(self.build_type_doc(t));
        }

        // Add ` & {`
        opening_parts.push(d.text(" & {"));

        self.build_aligned_object_literal_doc(trailing_obj, d.concat(&opening_parts), "})")
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
    ) -> DocId {
        let d = self.d();
        if t.members.is_empty() {
            return d.empty();
        }

        let mut member_parts = vec![];
        let mut prev_end = t.span.start + 1; // after opening brace

        for (i, m) in t.members.iter().enumerate() {
            let is_first = i == 0;
            let is_last = i == t.members.len() - 1;
            // Use content_end for comment detection (before trailing separator)
            let member_content_end = m.content_end(self.source);

            if force_multiline {
                // Forced multiline: build with hardlines
                member_parts.extend(self.build_multiline_member_prefix_doc(
                    prev_end,
                    m.span().start,
                    is_first,
                ));
                member_parts.push(self.build_type_member_doc_inner(m));

                // Handle trailing comments - preserve position relative to semicolon
                let upper_bound = t
                    .members
                    .get(i + 1)
                    .map_or(t.span.end, |next| next.span().start);
                let trailing: Vec<_> =
                    comments_in_range(self.comments, member_content_end, upper_bound)
                        .filter(|c| self.is_same_line(member_content_end, c.span.start))
                        .collect();
                member_parts.extend(self.build_comments_around_semicolon_doc(
                    &trailing,
                    member_content_end,
                    upper_bound,
                ));
            } else {
                // Width-aware: softlines, conditional semicolons
                member_parts.push(d.softline());
                member_parts.push(self.build_type_member_doc_inner(m));

                // Handle trailing comments - preserve position relative to semicolon
                let upper_bound = t
                    .members
                    .get(i + 1)
                    .map_or(t.span.end, |next| next.span().start);
                let trailing: Vec<_> =
                    comments_in_range(self.comments, member_content_end, upper_bound).collect();

                if is_last {
                    // Last member: semicolon only when broken
                    // In flat mode there's no semicolon, so all comments are "after"
                    // In break mode semicolon is added, comments still come after
                    member_parts.push(d.if_break(d.text(";"), d.empty()));
                    for comment in &trailing {
                        member_parts.push(d.text(" "));
                        member_parts.push(self.build_comment_doc(comment));
                    }
                } else {
                    // Non-last: semicolon always present, preserve comment position
                    member_parts.extend(self.build_comments_around_semicolon_doc(
                        &trailing,
                        member_content_end,
                        upper_bound,
                    ));
                    // Space before next member only when flat
                    member_parts.push(d.if_break(d.empty(), d.text(" ")));
                }
            }

            prev_end = m.span().end;
        }

        if force_multiline {
            // Trailing comments after last member
            let body_end = t.span.end.saturating_sub(1);
            member_parts.extend(self.build_trailing_body_comments_doc(prev_end, body_end));
        }

        d.concat(&member_parts)
    }

    /// Check if a TypeLiteral should be forced to multiline format.
    ///
    /// Returns true if:
    /// - Source has newline immediately after opening brace
    /// - Contains line comments or multi-line block comments
    /// - Contains block comments on their own line
    pub(super) fn type_literal_force_multiline(&self, obj: &TSTypeLiteral) -> bool {
        let source_is_multiline = super::super::is_brace_block_multiline(self.source, obj.span);
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
        opening: DocId,
        closing: &'static str,
    ) -> DocId {
        let d = self.d();
        let force_multiline = self.type_literal_force_multiline(obj);
        let members_doc =
            self.build_type_literal_members_only_doc_for_alignment(obj, force_multiline);

        let line_doc = if force_multiline {
            d.hardline()
        } else {
            d.softline()
        };

        d.group(d.concat(&[
            opening,
            d.indent(d.indent(members_doc)),
            d.align_spaces(2, d.concat(&[line_doc, d.text(closing)])),
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
    pub(super) fn build_union_member_object_literal_doc(&self, obj: &TSTypeLiteral) -> DocId {
        self.build_aligned_object_literal_doc(obj, self.d().text("{"), "}")
    }

    //
    // Type Literal Docs
    //

    /// Build a Doc for a type literal (object type): `{ a: T; b: U }`
    ///
    /// Handles both single-line and multi-line formats:
    /// - Single-line source stays single-line if it fits: `{ a: T; b: U }`
    /// - Multi-line source (newline after `{`) stays multi-line
    /// - Comments force multi-line formatting
    pub(super) fn build_type_literal_doc(&self, t: &TSTypeLiteral) -> DocId {
        self.build_type_literal_doc_inner(t, TypeLiteralMode::Standard)
    }

    /// Build a Doc for a type literal without wrapping in a group ("hugging").
    ///
    /// Used for type arguments where the type literal should "hug" and let
    /// the parent `<...>` group control breaking, matching Prettier's behavior.
    pub(super) fn build_type_literal_doc_hugging(&self, t: &TSTypeLiteral) -> DocId {
        self.build_type_literal_doc_inner(t, TypeLiteralMode::Hugging)
    }

    /// Inner implementation for type literal doc building.
    ///
    /// `mode` controls formatting behavior:
    /// - `Hugging`: Inline `; ` separators, no softlines, no group (for type args)
    /// - `Standard`: Width-aware with softlines, wrapped in group
    /// - `NoGroup`: Width-aware with softlines, no group (parent controls breaking)
    fn build_type_literal_doc_inner(&self, t: &TSTypeLiteral, mode: TypeLiteralMode) -> DocId {
        let d = self.d();
        use TypeLiteralMode::{Hugging, Standard};
        let hug = matches!(mode, Hugging);
        let wrap_in_group = matches!(mode, Standard);
        let force_multiline = self.type_literal_force_multiline(t);

        if t.members.is_empty() {
            // Empty type literal - handle comments inside
            let empty_doc = self.build_empty_body_with_comments_doc(t.span);
            return if wrap_in_group {
                d.group(empty_doc)
            } else {
                empty_doc
            };
        }

        let mut parts = vec![d.text("{")];
        if force_multiline {
            // Multi-line format (same for both modes)
            let mut member_parts = vec![];
            let mut prev_end = t.span.start + 1; // after opening brace
            for (i, m) in t.members.iter().enumerate() {
                let is_first = i == 0;
                // Use content_end for comment detection (before trailing separator)
                let member_content_end = m.content_end(self.source);

                member_parts.extend(self.build_multiline_member_prefix_doc(
                    prev_end,
                    m.span().start,
                    is_first,
                ));
                member_parts.push(self.build_type_member_doc_inner(m));

                // Handle trailing comments - preserve position relative to semicolon
                let upper_bound = t
                    .members
                    .get(i + 1)
                    .map_or(t.span.end, |next| next.span().start);
                let trailing: Vec<_> =
                    comments_in_range(self.comments, member_content_end, upper_bound)
                        .filter(|c| self.is_same_line(member_content_end, c.span.start))
                        .collect();
                member_parts.extend(self.build_comments_around_semicolon_doc(
                    &trailing,
                    member_content_end,
                    upper_bound,
                ));

                prev_end = m.span().end;
            }

            let body_end = t.span.end.saturating_sub(1);
            member_parts.extend(self.build_trailing_body_comments_doc(prev_end, body_end));

            parts.push(d.indent(d.concat(&member_parts)));
            parts.push(d.hardline());
        } else if hug {
            // Hugging mode: inline content with `; ` separators
            // Preserve comment position relative to semicolon
            for (i, m) in t.members.iter().enumerate() {
                let is_last = i == t.members.len() - 1;
                // Use content_end for comment detection (before trailing separator)
                let member_content_end = m.content_end(self.source);

                parts.push(self.build_type_member_doc_inner(m));

                let upper_bound = t
                    .members
                    .get(i + 1)
                    .map_or(t.span.end, |next| next.span().start);
                let trailing: Vec<_> =
                    comments_in_range(self.comments, member_content_end, upper_bound).collect();

                if !is_last {
                    parts.extend(self.build_comments_around_semicolon_doc(
                        &trailing,
                        member_content_end,
                        upper_bound,
                    ));
                    parts.push(d.text(" "));
                } else {
                    // Last member in hugging mode: no semicolon
                    for comment in &trailing {
                        parts.push(d.text(" "));
                        parts.push(self.build_comment_doc(comment));
                    }
                }
            }
        } else {
            // Width-aware format: stays inline if fits, wraps if too long
            // Preserve comment position relative to semicolon
            let mut member_parts = vec![];
            for (i, m) in t.members.iter().enumerate() {
                let is_last = i == t.members.len() - 1;
                // Use content_end for comment detection (before trailing separator)
                let member_content_end = m.content_end(self.source);

                member_parts.push(d.softline());
                member_parts.push(self.build_type_member_doc_inner(m));

                let upper_bound = t
                    .members
                    .get(i + 1)
                    .map_or(t.span.end, |next| next.span().start);
                let trailing: Vec<_> =
                    comments_in_range(self.comments, member_content_end, upper_bound).collect();

                if is_last {
                    // Last member: semicolon only when broken, comments after
                    member_parts.push(d.if_break(d.text(";"), d.empty()));
                    for comment in &trailing {
                        member_parts.push(d.text(" "));
                        member_parts.push(self.build_comment_doc(comment));
                    }
                } else {
                    // Non-last: preserve comment position relative to semicolon
                    member_parts.extend(self.build_comments_around_semicolon_doc(
                        &trailing,
                        member_content_end,
                        upper_bound,
                    ));
                    // Space before next member only when flat
                    member_parts.push(d.if_break(d.empty(), d.text(" ")));
                }
            }
            parts.push(d.indent(d.concat(&member_parts)));
            parts.push(d.softline());
        }
        parts.push(d.text("}"));

        if wrap_in_group {
            d.group(d.concat(&parts))
        } else {
            d.concat(&parts)
        }
    }

    /// Build a Doc for a type literal in function param context (no group wrapper).
    ///
    /// Uses width-aware format with softlines (can break), but WITHOUT wrapping
    /// in its own group. This lets the parent function type group control breaking.
    ///
    /// When the function type group breaks (because line is too long), these
    /// softlines become newlines, expanding the param's object type.
    pub(super) fn build_type_literal_doc_for_function_param(&self, t: &TSTypeLiteral) -> DocId {
        self.build_type_literal_doc_inner(t, TypeLiteralMode::NoGroup)
    }

    /// Build a Doc for a type expression suitable for use as a type argument.
    ///
    /// Object type literals are built without groups ("hugging") so the parent
    /// `<...>` group controls breaking, matching Prettier's behavior.
    pub(in crate::printer) fn build_type_doc_for_type_arg(&self, ts_type: &TSType) -> DocId {
        match ts_type {
            TSType::TypeLiteral(t) => self.build_type_literal_doc_hugging(t),
            TSType::Parenthesized(p) => {
                // Unwrap parentheses and recurse
                self.build_type_doc_for_type_arg(&p.type_annotation)
            }
            _ => self.build_type_doc(ts_type),
        }
    }
}
