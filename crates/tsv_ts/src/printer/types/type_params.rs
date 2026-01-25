// Type parameter printing for TypeScript
//
// Handles:
// - Type parameter declarations: `<T, U extends V = W>`
// - Type parameter instantiation (type arguments): `<T, U>`

use super::{CommentFilter, CommentSpacing, Printer};
use crate::ast::internal::{self, TSType, TSTypeParameter, TSTypeParameterDeclaration};
use tsv_lang::SymbolToU32;
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    //
    // Type Parameter Declarations
    //

    /// Build doc for type parameter declaration: `<T, U extends V = W>`
    /// Non-wrapping version - always inline
    pub(in crate::printer) fn build_type_parameter_declaration_doc(
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
    pub(in crate::printer) fn build_type_parameter_declaration_doc_wrapping(
        &self,
        decl: &TSTypeParameterDeclaration,
    ) -> Doc {
        doc::group(self.build_type_parameter_declaration_doc_inner(decl))
    }

    /// Build doc for type parameter declaration - inner version without group wrapper
    /// Used when caller wants to control the group (e.g., interface header)
    pub(in crate::printer) fn build_type_parameter_declaration_doc_inner(
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

        let inner_parts = doc::join_trailing(
            decl.params
                .iter()
                .map(|param| self.build_type_parameter_doc(param)),
            doc::comma_line(),
        );

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
    pub(in crate::printer) fn build_type_parameter_declaration_doc_inline_group(
        &self,
        decl: &TSTypeParameterDeclaration,
    ) -> Doc {
        self.build_type_parameter_declaration_doc_inner(decl)
    }

    /// Build doc for a single type parameter
    /// With optional modifiers: `const T`, `in T`, `out T`, `in out T`
    pub(in crate::printer) fn build_type_parameter_doc(&self, param: &TSTypeParameter) -> Doc {
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
    ) -> Doc {
        if inst.params.is_empty() {
            return doc::text("<>");
        }

        // Check for line comments between params or after last param (force multiline)
        if self.has_line_comments_in_delimited_list(&inst.params, TSType::span, inst.span.end - 1) {
            return self.build_type_parameter_instantiation_doc_with_line_comments(inst);
        }

        // Special case: single curly-brace type argument hugs the opening bracket
        // Prettier keeps `<{` together when there's a single multiline object/mapped type
        if inst.params.len() == 1
            && let Some(type_doc) = self.try_build_hugging_curly_type_doc(&inst.params[0])
        {
            return doc::concat(vec![doc::text("<"), type_doc, doc::text(">")]);
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

    /// Try to build a hugging doc for curly-brace types (object literals, mapped types).
    ///
    /// Returns `Some(doc)` if the type is a curly-brace type that should hug `<{`,
    /// `None` otherwise. Used for single type arguments where Prettier keeps
    /// the opening angle bracket hugged with the opening curly brace.
    fn try_build_hugging_curly_type_doc(&self, ty: &TSType) -> Option<Doc> {
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
