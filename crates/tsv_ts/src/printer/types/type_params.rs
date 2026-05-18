// Type parameter printing for TypeScript
//
// Handles:
// - Type parameter declarations: `<T, U extends V = W>`
// - Type parameter instantiation (type arguments): `<T, U>`

use super::{CommentFilter, CommentSpacing, Printer};
use crate::ast::internal::{self, TSType, TSTypeParameter, TSTypeParameterDeclaration};
use crate::printer::analysis::find_char_skipping_comments;
use tsv_lang::SymbolToU32;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    //
    // Type Parameter Declarations
    //

    /// Build doc for type parameter declaration: `<T, U extends V = W>`
    /// Non-wrapping version - always inline, unless expanding comments force multiline
    pub(in crate::printer) fn build_type_parameter_declaration_doc(
        &self,
        decl: &TSTypeParameterDeclaration,
    ) -> DocId {
        if self.has_expanding_comments_in_type_param_declaration(decl) {
            return self.build_type_parameter_declaration_doc_with_line_comments(decl);
        }

        let d = self.d();
        let param_docs = self.build_type_parameter_docs_with_comments(decl);
        d.concat(&[d.text("<"), d.join(param_docs, ", "), d.text(">")])
    }

    /// Build doc for type parameter declaration with wrapping support
    /// When the group breaks, each param goes on its own line with trailing comma
    pub(in crate::printer) fn build_type_parameter_declaration_doc_wrapping(
        &self,
        decl: &TSTypeParameterDeclaration,
    ) -> DocId {
        self.d()
            .group(self.build_type_parameter_declaration_doc_inner(decl))
    }

    /// Build doc for type parameter declaration - inner version without group wrapper
    /// Used when caller wants to control the group (e.g., interface header)
    pub(in crate::printer) fn build_type_parameter_declaration_doc_inner(
        &self,
        decl: &TSTypeParameterDeclaration,
    ) -> DocId {
        let d = self.d();
        if decl.params.is_empty() {
            return d.text("<>");
        }

        if self.has_expanding_comments_in_type_param_declaration(decl) {
            return self.build_type_parameter_declaration_doc_with_line_comments(decl);
        }

        let param_docs = self.build_type_parameter_docs_with_comments(decl);
        let inner = d.join_trailing(param_docs, d.comma_line());
        d.concat(&[
            d.text("<"),
            d.indent_softline(inner),
            d.softline(),
            d.text(">"),
        ])
    }

    /// Build doc for type parameter declaration with expanding comments
    fn build_type_parameter_declaration_doc_with_line_comments(
        &self,
        decl: &TSTypeParameterDeclaration,
    ) -> DocId {
        let d = self.d();
        let mut inner_parts = Vec::new();
        let mut prev_end = decl.span.start + 1; // After the opening `<`

        for (i, param) in decl.params.iter().enumerate() {
            let param_start = param.span.start;
            let param_end = param.span.end;
            let is_last = i == decl.params.len() - 1;

            // Leading comments (after previous comma or `<`)
            inner_parts.extend(self.build_leading_comments_multiline(prev_end, param_start));

            inner_parts.push(self.build_type_parameter_doc(param));

            if !is_last {
                let next_start = decl.params[i + 1].span.start;
                prev_end = self.emit_multiline_comma_with_comments(
                    &mut inner_parts,
                    param_end,
                    next_start,
                );
            } else {
                // Last param: trailing comma + comments before `>`
                let before_close = decl.span.end - 1;
                inner_parts.push(d.text(","));
                inner_parts.extend(self.build_trailing_comments_multiline(param_end, before_close));
                prev_end = before_close;
            }
        }

        d.concat(&[
            d.text("<"),
            d.indent(d.concat(&[d.hardline(), d.concat(&inner_parts)])),
            d.hardline(),
            d.text(">"),
        ])
    }

    /// Check for expanding comments in type param declarations: line comments,
    /// own-line block comments, or line comments inside param spans (e.g.,
    /// `T extends // comment\n  A`). Used by both wrapping and non-wrapping paths.
    fn has_expanding_comments_in_type_param_declaration(
        &self,
        decl: &TSTypeParameterDeclaration,
    ) -> bool {
        !decl.params.is_empty()
            && (self.has_line_comments_in_delimited_list(
                &decl.params,
                |p| p.span,
                decl.span.end - 1,
            ) || self.has_own_line_block_comments_in_bracket_list(
                decl.span,
                &decl.params,
                |p| p.span,
            ) || decl
                .params
                .iter()
                .any(|p| self.has_line_comments_between(p.span.start, p.span.end)))
    }

    /// Build enriched param docs with surrounding block comments from the declaration.
    /// Comments outside param spans (e.g., `</* c */ T /* c */>`) are captured here.
    /// Uses comma position to split: comments before comma = trailing, after = leading.
    fn build_type_parameter_docs_with_comments(
        &self,
        decl: &TSTypeParameterDeclaration,
    ) -> Vec<DocId> {
        let d = self.d();
        let mut prev_end = decl.span.start + 1; // After `<`
        decl.params
            .iter()
            .enumerate()
            .map(|(i, param)| {
                let mut parts = Vec::new();
                // Leading block comments (after previous comma or `<`)
                parts.push(self.build_comments_between_filtered(
                    prev_end,
                    param.span.start,
                    CommentSpacing::Trailing,
                    CommentFilter::BlockOnly,
                ));
                parts.push(self.build_type_parameter_doc(param));

                if i + 1 < decl.params.len() {
                    // Find comma between this param and next
                    let next_start = decl.params[i + 1].span.start;
                    let comma_pos = self.find_list_comma(param.span.end, next_start);
                    // Trailing block comments (before comma)
                    parts.push(self.build_comments_between_filtered(
                        param.span.end,
                        comma_pos,
                        CommentSpacing::Leading,
                        CommentFilter::BlockOnly,
                    ));
                    prev_end = comma_pos + 1; // After comma
                } else {
                    // Last param: trailing comments before `>`
                    parts.push(self.build_comments_between_filtered(
                        param.span.end,
                        decl.span.end - 1,
                        CommentSpacing::Leading,
                        CommentFilter::BlockOnly,
                    ));
                }
                d.concat(&parts)
            })
            .collect()
    }

    /// Build doc for a single type parameter
    /// With optional modifiers: `const T`, `in T`, `out T`, `in out T`
    pub(in crate::printer) fn build_type_parameter_doc(&self, param: &TSTypeParameter) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // Add modifiers in order: const, in, out
        if param.is_const {
            parts.push(d.text("const "));
        }
        if param.is_in {
            parts.push(d.text("in "));
        }
        if param.is_out {
            parts.push(d.text("out "));
        }

        // Comments before name: </* c */ T>
        parts.push(self.build_comments_between(
            param.span.start,
            param.name.span.start,
            CommentSpacing::Trailing,
        ));

        parts.push(d.symbol(param.name.name.to_u32()));

        // Track where we are for finding comments after the name
        let mut prev_end = param.name.span.end;

        if let Some(constraint) = &param.constraint {
            // Find `extends` keyword between name and constraint
            #[allow(clippy::expect_used)]
            // extends must exist when constraint is present in a valid AST
            let extends_pos = self
                .find_keyword_in_range(prev_end, constraint.span().start, "extends")
                .expect("extends keyword must exist when constraint is present");
            let extends_end = extends_pos + 7; // "extends".len()

            // Comments between name and `extends`: <T /* c */ extends A>
            parts.push(self.build_comments_between(prev_end, extends_pos, CommentSpacing::Leading));

            parts.push(d.text(" extends"));
            // If the constraint is `(// leading\n T)`, treat the leading line
            // comment inside the parens as if it were between `extends` and the
            // constraint so it forces the indent-and-break layout (matching
            // prettier's paren stripping).
            let (value_search_end, value_doc) = if let TSType::Parenthesized(p) =
                constraint.as_ref()
                && self.paren_has_leading_line_comment(p)
            {
                (
                    p.type_annotation.span().start,
                    self.build_type_doc(&p.type_annotation),
                )
            } else {
                (constraint.span().start, self.build_type_doc(constraint))
            };
            self.append_keyword_value_with_comments(
                &mut parts,
                extends_end,
                value_search_end,
                value_doc,
            );
            prev_end = constraint.span().end;
        }

        if let Some(default) = &param.default {
            // Find `=` between previous end and default
            #[allow(clippy::expect_used)] // = must exist when default is present in a valid AST
            let eq_pos = find_char_skipping_comments(
                self.source.as_bytes(),
                prev_end as usize,
                default.span().start as usize,
                b'=',
            )
            .expect("= must exist when default is present");
            let eq_end = (eq_pos + 1) as u32;
            let eq_pos = eq_pos as u32;

            // Comments before `=`: <T extends B /* c */ = C>
            parts.push(self.build_comments_between(prev_end, eq_pos, CommentSpacing::Leading));

            parts.push(d.text(" ="));
            self.append_keyword_value_with_comments(
                &mut parts,
                eq_end,
                default.span().start,
                self.build_type_doc(default),
            );
            prev_end = default.span().end;
        }

        // Trailing comments after last part: <T /* c */> or <T extends A /* c */>
        parts.push(self.build_comments_between(prev_end, param.span.end, CommentSpacing::Leading));

        d.concat(&parts)
    }

    /// Append a value doc after a keyword, handling comments in between.
    /// Block comments are inlined: `extends /* c */ A`
    /// Line comments force break+indent: `extends // c\n  A`
    /// No comments: `extends A` (space + value)
    fn append_keyword_value_with_comments(
        &self,
        parts: &mut Vec<DocId>,
        keyword_end: u32,
        value_start: u32,
        value_doc: DocId,
    ) {
        let d = self.d();
        let comments = self.build_comments_between_filtered_opt(
            keyword_end,
            value_start,
            CommentSpacing::Leading,
            CommentFilter::All,
        );
        if let Some(c) = comments {
            parts.push(c);
            if self.has_line_comments_between(keyword_end, value_start) {
                parts.push(d.indent(d.concat(&[d.hardline(), value_doc])));
                return;
            }
        }
        parts.push(d.text(" "));
        parts.push(value_doc);
    }

    //
    // Type Parameter Instantiation (Type Arguments)
    //

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
    ///
    /// Special case: single object type hugs the opening bracket:
    /// ```typescript
    /// fn<{
    ///     a: number;
    ///     b: string;
    /// }>();
    /// ```
    pub(in crate::printer) fn build_type_parameter_instantiation_doc(
        &self,
        inst: &internal::TSTypeParameterInstantiation,
    ) -> DocId {
        let d = self.d();
        if inst.params.is_empty() {
            return d.text("<>");
        }

        // Check for comments that force expansion: line comments or own-line block comments
        if self.has_line_comments_in_delimited_list(&inst.params, TSType::span, inst.span.end - 1)
            || self.has_own_line_block_comments_in_bracket_list(
                inst.span,
                &inst.params,
                TSType::span,
            )
        {
            return self.build_type_parameter_instantiation_doc_with_line_comments(inst);
        }

        // Special case: single curly-brace type argument hugs the opening bracket
        // Prettier keeps `<{` together when there's a single multiline object/mapped type
        if inst.params.len() == 1
            && let Some(type_doc) = self.try_build_hugging_curly_type_doc(&inst.params[0])
        {
            return d.concat(&[d.text("<"), type_doc, d.text(">")]);
        }

        // Build params with commas and line breaks
        // The doc printer's look-ahead (fits_with_lookahead) handles the decision
        // of whether to break based on what follows the type params.
        let mut param_parts = Vec::new();
        let mut prev_end = inst.span.start + 1; // After the opening `<`

        for (i, param) in inst.params.iter().enumerate() {
            let param_start = param.span().start;

            if i > 0 {
                param_parts.push(d.text(","));
                param_parts.push(d.line());
            }

            // Add leading block comments (after previous comma or `<`)
            param_parts.push(self.build_comments_between_filtered(
                prev_end,
                param_start,
                CommentSpacing::Trailing,
                CommentFilter::BlockOnly,
            ));

            param_parts.push(self.build_type_doc(param));

            let param_end = param.span().end;
            if i + 1 < inst.params.len() {
                // Find comma between this param and next
                let next_start = inst.params[i + 1].span().start;
                let comma_pos = self.find_list_comma(param_end, next_start);
                // Trailing block comments (before comma)
                param_parts.push(self.build_comments_between_filtered(
                    param_end,
                    comma_pos,
                    CommentSpacing::Leading,
                    CommentFilter::BlockOnly,
                ));
                prev_end = comma_pos + 1; // After comma
            } else {
                // Last param: trailing comments before `>`
                param_parts.push(self.build_comments_between_filtered(
                    param_end,
                    inst.span.end - 1,
                    CommentSpacing::Leading,
                    CommentFilter::BlockOnly,
                ));
            }
        }

        // Wrap in group with angle brackets and optional breaks
        d.group(d.concat(&[
            d.text("<"),
            d.indent_softline(d.concat(&param_parts)),
            d.softline(),
            d.text(">"),
        ]))
    }

    /// Build type parameter instantiation with line comments
    fn build_type_parameter_instantiation_doc_with_line_comments(
        &self,
        inst: &internal::TSTypeParameterInstantiation,
    ) -> DocId {
        let d = self.d();
        let mut inner_parts = Vec::new();
        let mut prev_end = inst.span.start + 1; // After the opening `<`

        for (i, param) in inst.params.iter().enumerate() {
            let param_start = param.span().start;
            let param_end = param.span().end;
            let is_last = i == inst.params.len() - 1;

            // Leading comments (after previous comma or `<`)
            inner_parts.extend(self.build_leading_comments_multiline(prev_end, param_start));

            inner_parts.push(self.build_type_doc(param));

            if !is_last {
                let next_start = inst.params[i + 1].span().start;
                prev_end = self.emit_multiline_comma_with_comments(
                    &mut inner_parts,
                    param_end,
                    next_start,
                );
            } else {
                // Last param: trailing comments before `>`
                let before_close = inst.span.end - 1;
                inner_parts.extend(self.build_trailing_comments_multiline(param_end, before_close));
                prev_end = before_close;
            }
        }

        d.concat(&[
            d.text("<"),
            d.indent(d.concat(&[d.hardline(), d.concat(&inner_parts)])),
            d.hardline(),
            d.text(">"),
        ])
    }

    /// Try to build a hugging doc for curly-brace types (object literals, mapped types).
    ///
    /// Returns `Some(doc)` if the type is a curly-brace type that should hug `<{`,
    /// `None` otherwise. Used for single type arguments where Prettier keeps
    /// the opening angle bracket hugged with the opening curly brace.
    fn try_build_hugging_curly_type_doc(&self, ty: &TSType) -> Option<DocId> {
        match ty {
            // Object type literal: { a: number; b: string } or { /* comment */ }
            // Hug if it has members OR comments inside (will be multiline)
            TSType::TypeLiteral(type_lit)
                if !type_lit.members.is_empty()
                    || self.has_comments_between(type_lit.span.start, type_lit.span.end) =>
            {
                Some(self.build_type_literal_doc_hugging(type_lit))
            }
            // Mapped type: { [K in keyof T]: V }
            TSType::Mapped(mapped) => Some(self.build_mapped_type_doc(mapped)),
            _ => None,
        }
    }
}
