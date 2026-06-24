// Type literal printing for TypeScript
//
// Handles printing of object type literals (`{ a: T; b: U }`) with:
// - Single-line and multi-line formats
// - Object alignment for union members and parenthesized intersections
// - Grouped (`Standard`) vs no-group (`NoGroup`) modes so a parent (function
//   type / type-argument list) can control breaking

use super::super::comments_in_range;
use super::Printer;
use super::helpers::{immediate_union_paren, unwrap_parenthesized};
use crate::ast::internal::{
    TSIntersectionType, TSParenthesizedType, TSType, TSTypeElement, TSTypeLiteral, TSUnionType,
};
use tsv_lang::doc::arena::DocId;

/// Mode for building type literal docs.
enum TypeLiteralMode {
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
        delimiter_pull_pos: Option<u32>,
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
            // First member: drop comments pulled onto the `{` line (emitted as
            // the brace-line prefix by the caller). No-op when `delimiter_pull_pos`
            // is `None` (the alignment caller).
            self.first_member_leading_comments(all_comments, delimiter_pull_pos)
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

    /// Build docs for block comments between the opening brace and the first
    /// member, emitted inline (`{/* c */ a: number}`). Used by the non-multiline
    /// type-literal paths, where leading comments before the first member would
    /// otherwise be dropped. Line / own-line comments don't reach here — they
    /// force the multiline path via `type_literal_force_multiline`.
    fn build_type_literal_leading_comments_inline(
        &self,
        brace_start: u32,
        first_member_start: u32,
    ) -> Vec<DocId> {
        let d = self.d();
        let mut docs = Vec::new();
        for comment in comments_in_range(self.comments, brace_start + 1, first_member_start) {
            docs.push(self.build_comment_doc(comment));
            docs.push(d.text(" "));
        }
        docs
    }

    /// Build docs for trailing comments partitioned around a semicolon.
    ///
    /// Returns docs for: `[space + comment]* ";" [space + comment]*`
    ///
    /// Comments are positioned relative to the first semicolon found in the
    /// source range `member_end..upper_bound`. If no semicolon exists in source,
    /// all comments are placed before the semicolon. Each comment is emitted via
    /// `build_trailing_comment_doc` — block inline, line through `line_suffix` so a
    /// long trailing comment never forces the member's own type (e.g. a union) to
    /// break (matches prettier and the interface-member path).
    fn build_comments_around_semicolon_doc(
        &self,
        comments: &[&tsv_lang::Comment],
        member_end: u32,
        upper_bound: u32,
    ) -> Vec<DocId> {
        let d = self.d();
        // Comment-aware so a `;` inside a comment in this gap isn't taken for the
        // member separator (which would partition the comments incorrectly).
        let semi_pos = self.find_char_outside_comments(member_end, upper_bound, b';');

        let (before_semi, after_semi): (Vec<_>, Vec<_>) =
            comments.iter().partition(|c| match semi_pos {
                Some(pos) => c.span.start < pos,
                None => true, // No semicolon in source, all comments are "before"
            });

        let mut docs = Vec::with_capacity(before_semi.len() + after_semi.len() + 1);
        for comment in before_semi {
            docs.push(self.build_trailing_comment_doc(comment));
        }
        docs.push(d.text(";"));
        for comment in after_semi {
            docs.push(self.build_trailing_comment_doc(comment));
        }
        docs
    }

    //
    // Type parenthesization with special object handling
    //

    /// Build type doc, wrapping in parentheses if the predicate returns true.
    ///
    /// Object-bearing members align via `build_aligned_object_literal_doc`:
    /// properties are double-indented and the closing `})` is single-indented.
    /// The plain default case only indents the inner type. (Prettier offsets
    /// these closings by a 2-space sub-tab alignment; tsv uses whole tabs — see
    /// `docs/conformance_prettier.md`.)
    ///
    /// Special case: intersection with trailing object type builds a custom doc
    /// so that `})` can be aligned properly (one indent level past the base).
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
                return self.build_parenthesized_union_doc(
                    union,
                    immediate_union_paren(ts_type),
                    false,
                );
            }

            // Default case: parenthesize and indent the inner type. The closing
            // `)` sits at the base indent; the object/intersection cases above
            // handle their own aligned (one-level-deeper) closings.
            d.concat(&[
                d.text("("),
                d.indent(self.build_type_doc(ts_type)),
                d.text(")"),
            ])
        } else {
            // Type-operand positions (union/intersection members, conditional
            // check/extends types, optional tuple elements) break the OUTERMOST
            // generic first, matching Prettier's `printTypeParameters`. Use the
            // wrapping type-args path so a nested non-huggable generic like
            // `Outer<Inner<...>>` wraps the outer `Outer<>` instead of force-inlining
            // the single arg and breaking only the inner `Inner<>`.
            self.build_type_doc_with_wrapping_type_args(ts_type)
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
    ///
    /// When `paren` is supplied (the union's parens are retained from source, not
    /// synthetic), block comments the user wrote inside the parens are preserved in
    /// place — a leading comment after `(` (`(/* c */ a | b)`) and a trailing comment
    /// before `)` (`(a | b /* c */)`). Prettier hoists these out of the parens; tsv
    /// keeps them with the parenthesized member. A trailing *line* comment before `)`
    /// is preserved here too (forcing the group to break). A leading *line* comment
    /// after `(` is normally pre-relocated by the union/intersection line-comment
    /// paths, so it only reaches here when `emit_inner_leading_line_comments` is set —
    /// the first-union-member case, which has no previous member to relocate onto.
    pub(super) fn build_parenthesized_union_doc(
        &self,
        union: &TSUnionType,
        paren: Option<&TSParenthesizedType>,
        emit_inner_leading_line_comments: bool,
    ) -> DocId {
        let d = self.d();
        let union_doc = self.build_union_type_doc(union, false);

        let mut needs_break = false;
        let mut indented = vec![d.softline()];
        if let Some(p) = paren {
            // Leading comments between `(` and the union. Block comments stay inline
            // (`(/* c */ a | b)`). A leading *line* comment is normally relocated by
            // the union/intersection line-comment paths (to trail the previous outer
            // member), so it reaches here only for the FIRST union member — which has
            // no previous member to relocate onto, so it is kept inside the parens
            // leading the inner union. A line comment must end its line, so it forces
            // the paren group to break.
            for comment in comments_in_range(self.comments, p.span.start + 1, union.span.start) {
                if comment.is_block {
                    indented.push(self.build_comment_doc(comment));
                    indented.push(d.text(" "));
                } else if emit_inner_leading_line_comments {
                    indented.push(self.build_comment_doc(comment));
                    indented.push(d.hardline());
                    needs_break = true;
                }
            }
        }
        indented.push(union_doc);
        if let Some(p) = paren {
            // Trailing comments between the union and `)`: a block comment stays
            // inline (`(a | b /* c */)`); a line comment defers to end-of-line and
            // forces the paren group to break — the inner union (built without its
            // own group) inherits the break, expanding to one member per line.
            for comment in comments_in_range(self.comments, union.span.end, p.span.end - 1) {
                if comment.is_block {
                    indented.push(d.text(" "));
                    indented.push(self.build_comment_doc(comment));
                } else {
                    let suffix = d.concat(&[d.text(" "), self.build_comment_doc(comment)]);
                    indented.push(d.line_suffix(suffix));
                    needs_break = true;
                }
            }
        }

        let mut inner_parts = vec![d.indent(d.concat(&indented)), d.softline()];
        if needs_break {
            inner_parts.push(d.break_parent());
        }
        let inner = d.group(d.concat(&inner_parts));
        d.parens(inner)
    }

    //
    // Object alignment helpers (for unions and parenthesized intersections)
    //

    /// Build doc for `(A & B & { members })` with proper alignment.
    ///
    /// Aligns `})` one indent level past the base when breaking:
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
    /// - Print members with double indent (4-tab alignment)
    /// - Print `})` at base indent + one level (when breaking)
    ///
    /// (Prettier offsets `})` by a 2-space sub-tab alignment; tsv uses whole
    /// tabs — see `docs/conformance_prettier.md`.)
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

        // Width-aware: the opening bracketSpacing boundary leads (a space when flat
        // `{ a }`, a newline when broken), THEN the first member's leading block
        // comments, so the padding sits before the comment (`{ /* c */ a }`).
        // The force_multiline branch handles leading comments per-member via
        // `build_multiline_member_prefix_doc`; the width-aware branch does not —
        // without this a union-member object's interior leading comment is dropped
        // (`{ /* c */ a: 1 } | B`). Mirrors the width-aware branch of
        // `build_type_literal_doc_inner`.
        if !force_multiline {
            member_parts.push(d.line());
            if let Some(first) = t.members.first() {
                member_parts.extend(
                    self.build_type_literal_leading_comments_inline(
                        t.span.start,
                        first.span().start,
                    ),
                );
            }
        }

        for (i, m) in t.members.iter().enumerate() {
            let is_first = i == 0;
            let is_last = i == t.members.len() - 1;
            // Use content_end for comment detection (before trailing separator)
            let member_content_end = m.content_end(self.source);

            if force_multiline {
                // Forced multiline: build with hardlines. `None` keeps the
                // delimiter-line comment relocating in this alignment path
                // (union-member / intersection-trailing object literals).
                member_parts.extend(self.build_multiline_member_prefix_doc(
                    prev_end,
                    m.span().start,
                    is_first,
                    None,
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
                // Width-aware: softlines, conditional semicolons. The opening
                // bracketSpacing boundary precedes the first member (emitted before
                // the loop); subsequent members keep a softline (the inter-member
                // flat space is the non-last `if_break(empty, " ")` below).
                if !is_first {
                    member_parts.push(d.softline());
                }
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
                        member_parts.push(self.build_trailing_comment_doc(comment));
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
        // Prettier breaks an object type when its first member starts on a line
        // below the opening brace. `is_brace_block_multiline` only sees a newline
        // *immediately* after `{`, so a block comment on the brace line
        // (`{ /* c */\n a: T }`) defeats it — detect the newline before the first
        // member directly here.
        let first_member_on_new_line = obj.members.first().is_some_and(|m| {
            self.source[obj.span.start as usize..m.span().start as usize].contains('\n')
        });
        let has_line_or_multiline_block =
            comments_in_range(self.comments, obj.span.start, obj.span.end)
                .any(|c| !c.is_block || c.content.contains('\n'));
        let member_spans: Vec<_> = obj.members.iter().map(TSTypeElement::span).collect();
        let has_standalone_block =
            self.has_standalone_block_comment(obj.span.start, obj.span.end, &member_spans);
        source_is_multiline
            || first_member_on_new_line
            || has_line_or_multiline_block
            || has_standalone_block
    }

    /// Build aligned object literal doc with custom opening/closing.
    ///
    /// Used for object literals in union types and parenthesized intersections.
    /// Members are double-indented (aligning with the content after `{`) and the
    /// closing delimiter is single-indented (aligning with `{`). Prettier offsets
    /// the closing by a 2-space sub-tab alignment instead; tsv renders that
    /// half-step as a whole tab — see `docs/conformance_prettier.md`.
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

        // Closing inner boundary: a hardline when forced multiline, else the
        // bracketSpacing boundary (a space when flat `{ a }`, a newline when the
        // group breaks).
        let line_doc = if force_multiline {
            d.hardline()
        } else {
            d.line()
        };

        d.group(d.concat(&[
            opening,
            d.indent(d.indent(members_doc)),
            d.indent(d.concat(&[line_doc, d.text(closing)])),
        ]))
    }

    /// Build doc for object type literal when it's a direct union member.
    ///
    /// Aligns object content with the position after `| {`:
    /// ```text
    /// type T =
    ///   | {
    ///       prop: A;  // double indent (aligns with content after "{ ")
    ///     }           // single indent (aligns with "{")
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

    /// Inner implementation for type literal doc building.
    ///
    /// `mode` controls formatting behavior:
    /// - `Standard`: Width-aware with softlines, wrapped in group
    /// - `NoGroup`: Width-aware with softlines, no group (parent controls breaking)
    fn build_type_literal_doc_inner(&self, t: &TSTypeLiteral, mode: TypeLiteralMode) -> DocId {
        let d = self.d();
        let wrap_in_group = matches!(mode, TypeLiteralMode::Standard);
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
            // A comment trailing the opening `{` on its own line is kept on the
            // `{` line (divergence from prettier, which relocates it to its own
            // line as the first member's leading comment). A line/own-line
            // comment is itself what forces this multiline branch. See
            // conformance_prettier.md §Comment relocation (Type literal `{`).
            let first_member_start = t.members[0].span().start;
            let (brace_line_prefix, delimiter_pull_pos) =
                self.delimiter_line_comment_prefix(t.span.start, first_member_start);
            parts.push(d.concat(&brace_line_prefix));

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
                    delimiter_pull_pos,
                ));
                // A preceding format-ignore directive keeps the member's source
                // verbatim. Use the content span (no trailing
                // `;`); the loop's semicolon handling below re-adds the `;`.
                let member_doc = if self.has_format_ignore_in_range(prev_end, m.span().start) {
                    self.raw_source_range(m.span().start, member_content_end)
                } else {
                    self.build_type_member_doc_inner(m)
                };
                member_parts.push(member_doc);

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
        } else {
            // Width-aware format: stays inline if fits, wraps if too long.
            // The opening bracketSpacing boundary leads (a space when flat `{ a }`,
            // a newline when broken), THEN any interior leading comments, so the
            // padding sits before the comment (`{ /* c */ a }`, not `{/* c */ a }`).
            let mut member_parts = vec![d.line()];
            if let Some(first) = t.members.first() {
                member_parts.extend(
                    self.build_type_literal_leading_comments_inline(
                        t.span.start,
                        first.span().start,
                    ),
                );
            }
            for (i, m) in t.members.iter().enumerate() {
                let is_first = i == 0;
                let is_last = i == t.members.len() - 1;
                // Use content_end for comment detection (before trailing separator)
                let member_content_end = m.content_end(self.source);

                // First member follows the opening boundary directly; subsequent
                // members keep a softline — the inter-member flat space is emitted
                // by the non-last `if_break(empty, " ")` below.
                if !is_first {
                    member_parts.push(d.softline());
                }
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
                        member_parts.push(self.build_trailing_comment_doc(comment));
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
            parts.push(d.line());
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
    /// An object type literal carries its own width-aware group, so when it
    /// overflows it breaks block-style (members on their own lines) rather than
    /// spilling an inner union/intersection — even inside a multi-argument list
    /// (`Map<K, { ...wide... }>`). Matches Prettier and the single-argument path.
    pub(in crate::printer) fn build_type_doc_for_type_arg(&self, ts_type: &TSType) -> DocId {
        let d = self.d();
        match ts_type {
            TSType::TypeLiteral(t) => self.build_type_literal_doc(t),
            TSType::Parenthesized(p) => {
                // Unwrap the parens (redundant in type-argument position — prettier
                // strips them too) but preserve any comments the user wrote inside
                // them (`Foo<(a | b /* c */)>` → `Foo<a | b /* c */>`). Without this
                // the comment was dropped — a content-loss bug. A line comment defers
                // to end-of-line and forces the type-argument list to break (via
                // `break_parent`), matching prettier's expansion.
                let inner = self.build_type_doc_for_type_arg(&p.type_annotation);
                let inner_start = p.type_annotation.span().start;
                let inner_end = p.type_annotation.span().end;
                let has_leading = self.has_comments_between(p.span.start + 1, inner_start);
                let has_trailing = self.has_comments_between(inner_end, p.span.end - 1);
                if !has_leading && !has_trailing {
                    return inner;
                }
                let leading: Vec<_> = if has_leading {
                    comments_in_range(self.comments, p.span.start + 1, inner_start).collect()
                } else {
                    Vec::new()
                };
                let trailing: Vec<_> = if has_trailing {
                    comments_in_range(self.comments, inner_end, p.span.end - 1).collect()
                } else {
                    Vec::new()
                };
                // A line comment forces the type-argument list to break. Emit
                // `break_parent` FIRST so it sits behind the inner type's group in the
                // forward `fits()` scan — otherwise it poisons that scan and needlessly
                // expands an inner union (`Foo<(a | b // c)>` keeps `a | b` inline,
                // matching prettier, rather than `| a | b`).
                let needs_break = leading
                    .iter()
                    .chain(&trailing)
                    .any(|comment| !comment.is_block);
                let mut parts = Vec::new();
                if needs_break {
                    parts.push(d.break_parent());
                }
                for comment in &leading {
                    parts.push(self.build_comment_doc(comment));
                    if comment.is_block {
                        parts.push(d.text(" "));
                    } else {
                        parts.push(d.hardline());
                    }
                }
                parts.push(inner);
                for comment in &trailing {
                    parts.push(self.build_trailing_comment_doc(comment));
                }
                d.concat(&parts)
            }
            _ => self.build_type_doc(ts_type),
        }
    }
}
