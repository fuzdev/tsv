// Type annotation printing for TypeScript
//
// Handles printing of TypeScript-specific type syntax:
// - Type annotations (: Type)
// - Type keywords (number, string, boolean, etc.)
// - Complex types (unions, intersections, generics, etc.)
//
// This module coordinates type printing and delegates to specialized submodules:
// - helpers.rs: Standalone helper functions (parenthesization, unwrapping)
// - type_params.rs: Type parameter declarations and instantiation
// - type_annotation.rs: Type annotations (`: Type`)
// - type_members.rs: Type literal members (PropertySignature, MethodSignature, etc.)
// - type_literal.rs: Type literals (`{ a: T }`) and object alignment
// - function_types.rs: Function types, constructor types, signature params
// - union_intersection.rs: Union and intersection types
// - composite.rs: Conditional, mapped, tuple, array types
// - literal_types.rs: Literal types (string, number, template literal)

mod composite;
pub(in crate::printer) mod function_types;
pub(crate) mod helpers;
mod literal_types;
mod type_annotation;
mod type_literal;
mod type_members;
mod type_params;
mod union_intersection;

// Re-export public items from helpers
pub use helpers::{
    intersection_has_expanding_first_type, intersection_has_huggable_last_type,
    should_hug_union_type, unwrap_parenthesized,
};

// Re-export for submodules to use `super::X` instead of `super::super::X`
pub(super) use super::{CommentFilter, CommentSpacing, Printer};

use crate::ast::internal::{TSParenthesizedType, TSType};
use crate::printer::analysis::find_char_skipping_comments;
use helpers::type_needs_parens_for_indexed_access_object;
use helpers::type_needs_parens_for_prefix_operator;
use tsv_lang::SymbolToU32;
use tsv_lang::comments_in_range;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    //
    // Main Type Doc Builders
    //

    /// Build a Doc for a TypeScript type expression
    pub(in crate::printer) fn build_type_doc(&self, ts_type: &TSType) -> DocId {
        self.build_type_doc_inner(ts_type, false)
    }

    /// Build a Doc for a TypeScript type expression with wrapping type arguments.
    ///
    /// Used in type alias RHS where TypeReference type arguments should break
    /// internally (e.g., `Promise<LongType | null>` breaks inside `<>`).
    pub(in crate::printer) fn build_type_doc_with_wrapping_type_args(
        &self,
        ts_type: &TSType,
    ) -> DocId {
        self.build_type_doc_inner(ts_type, true)
    }

    /// Inner implementation for type doc building.
    /// When `wrap_type_args` is true, TypeReference uses wrapping type arguments.
    pub(super) fn build_type_doc_inner(&self, ts_type: &TSType, wrap_type_args: bool) -> DocId {
        let d = self.d();
        match ts_type {
            TSType::Keyword(kw) => d.text_owned(kw.kind.as_str().to_string()),
            TSType::Literal(lit) => self.build_literal_type_doc(lit),
            TSType::Array(arr) => self.build_array_type_doc(arr),
            TSType::Union(u) => self.build_union_type_doc(u, true),
            TSType::Intersection(i) => self.build_intersection_type_doc(i, true),
            TSType::TypeReference(r) => {
                let mut parts = vec![self.build_type_entity_name_doc(&r.type_name)];
                if let Some(type_args) = &r.type_arguments {
                    // Preserve comments before type args: `Map/* c */ <string, number>`
                    if let Some(doc) = self.build_name_to_type_params_comments_opt(
                        r.type_name.span().end,
                        type_args.span.start,
                        CommentSpacing::Trailing,
                    ) {
                        parts.push(doc);
                    }
                    if wrap_type_args {
                        parts.push(self.build_type_arguments_doc_wrapping(type_args));
                    } else {
                        parts.push(self.build_type_arguments_doc(type_args));
                    }
                }
                d.concat(&parts)
            }
            TSType::TypeLiteral(t) => self.build_type_literal_doc(t),
            TSType::Function(f) => self.build_function_type_doc(f),
            TSType::Constructor(c) => self.build_constructor_type_doc(c),
            TSType::Tuple(t) => self.build_tuple_type_doc(t),
            // Parenthesized types: unwrap, preserving any comments inside the parens.
            // Parent contexts (IndexedAccess, Array, TypeOperator) add parens when
            // needed based on the inner type.
            TSType::Parenthesized(p) => self.build_parenthesized_type_unwrap_doc(p),
            TSType::TypePredicate(p) => {
                let mut parts = vec![];
                if p.asserts {
                    // Comments between `asserts` and parameter name
                    let asserts_end = p.span.start + 7; // "asserts".len()
                    let param_start = p.parameter_name.span.start;
                    parts.push(d.text("asserts "));
                    parts.push(self.build_comments_between(
                        asserts_end,
                        param_start,
                        CommentSpacing::Trailing,
                    ));
                }
                parts.push(d.symbol(p.parameter_name.name.to_u32()));
                if let Some(type_ann) = &p.type_annotation {
                    // Comments between `is` keyword and the type
                    // Find `i` of `is` skipping comments (plain find("is") could match
                    // inside a comment like `/* crisis */`)
                    let param_end = p.parameter_name.span.end;
                    let type_start = type_ann.span().start;
                    let is_end = find_char_skipping_comments(
                        self.source.as_bytes(),
                        param_end as usize,
                        type_start as usize,
                        b'i',
                    )
                    .map(|i_pos| (i_pos + 2) as u32); // skip past "is"
                    parts.push(d.text(" is "));
                    if let Some(is_end) = is_end {
                        parts.push(self.build_comments_between(
                            is_end,
                            type_start,
                            CommentSpacing::Trailing,
                        ));
                    }
                    parts.push(self.build_type_doc(type_ann));
                }
                d.concat(&parts)
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
                d.group(self.build_conditional_type_doc_inner(c))
            }
            TSType::Mapped(m) => self.build_mapped_type_doc(m),
            TSType::TypeOperator(o) => {
                let needs_parens = type_needs_parens_for_prefix_operator(&o.type_annotation);
                let operand_doc = self.build_type_doc(&o.type_annotation);
                // Comments between keyword and operand type
                let keyword_end = o.span.start + o.operator.as_str().len() as u32;
                let operand_start = o.type_annotation.span().start;
                let comments_doc = self.build_comments_between(
                    keyword_end,
                    operand_start,
                    CommentSpacing::Trailing,
                );
                if needs_parens {
                    d.concat(&[
                        d.text(o.operator.as_str()),
                        d.text(" "),
                        comments_doc,
                        d.text("("),
                        operand_doc,
                        d.text(")"),
                    ])
                } else {
                    d.concat(&[
                        d.text(o.operator.as_str()),
                        d.text(" "),
                        comments_doc,
                        operand_doc,
                    ])
                }
            }
            TSType::Import(i) => {
                let mut parts = vec![d.text("import(")];
                parts.push(self.build_literal_doc(&i.argument));
                // Import type options
                if let Some(options) = &i.options {
                    parts.push(d.text(", "));
                    parts.push(self.build_expression_doc(options));
                }
                parts.push(d.text(")"));
                if let Some(qualifier) = &i.qualifier {
                    // Comments between `)` or closing paren and qualifier
                    let dot_area_start = i
                        .options
                        .as_ref()
                        .map_or(i.argument.span.end, |o| o.span().end);
                    let qualifier_start = qualifier.span().start;
                    parts.push(d.text("."));
                    parts.push(self.build_comments_between(
                        dot_area_start,
                        qualifier_start,
                        CommentSpacing::Trailing,
                    ));
                    parts.push(self.build_type_entity_name_doc(qualifier));
                }
                if let Some(type_args) = &i.type_arguments {
                    // Preserve comments before type args: `import("a").Foo/* c */ <string>`
                    let gap_start = i
                        .qualifier
                        .as_ref()
                        .map_or(i.argument.span.end + 1, |q| q.span().end);
                    if let Some(doc) = self.build_name_to_type_params_comments_opt(
                        gap_start,
                        type_args.span.start,
                        CommentSpacing::Trailing,
                    ) {
                        parts.push(doc);
                    }
                    parts.push(self.build_type_arguments_doc(type_args));
                }
                d.concat(&parts)
            }
            TSType::TypeQuery(q) => {
                let mut parts = vec![d.text("typeof ")];
                // Comments between `typeof` and the expression
                let typeof_end = q.span.start + 6; // "typeof".len()
                let expr_start = q.expr_name.span().start;
                parts.push(self.build_comments_between(
                    typeof_end,
                    expr_start,
                    CommentSpacing::Trailing,
                ));
                parts.push(self.build_type_query_expr_name_doc(&q.expr_name));
                if let Some(type_args) = &q.type_arguments {
                    // Preserve comments: `typeof fn/* c */ <string>`
                    let gap_start = q.expr_name.span().end;
                    if let Some(doc) = self.build_name_to_type_params_comments_opt(
                        gap_start,
                        type_args.span.start,
                        CommentSpacing::Trailing,
                    ) {
                        parts.push(doc);
                    }
                    parts.push(self.build_type_arguments_doc(type_args));
                }
                d.concat(&parts)
            }
            TSType::IndexedAccess(i) => {
                let object_doc = self.build_type_doc(&i.object_type);
                let needs_parens = type_needs_parens_for_indexed_access_object(&i.object_type);
                // Comments between `[` and the index type
                let index_type_start = i.index_type.span().start;
                // Find `[` position: after object type end (or after `)` for parens case)
                let bracket_area_start = i.object_type.span().end;
                let bracket_pos = self.source
                    [bracket_area_start as usize..index_type_start as usize]
                    .find('[')
                    .map(|p| bracket_area_start + p as u32 + 1); // +1 for after `[`
                let index_comments = bracket_pos.map(|bp| {
                    self.build_comments_between(bp, index_type_start, CommentSpacing::Trailing)
                });
                let index_doc = self.build_type_doc(&i.index_type);
                let mut parts = if needs_parens {
                    vec![d.text("("), object_doc, d.text(")[")]
                } else {
                    vec![object_doc, d.text("[")]
                };
                if let Some(c) = index_comments {
                    parts.push(c);
                }
                parts.extend([index_doc, d.text("]")]);
                d.concat(&parts)
            }
            TSType::Rest(r) => {
                // Comments between `...` and the type
                let dots_end = r.span.start + 3; // "...".len()
                let type_start = r.type_annotation.span().start;
                let comments_doc =
                    self.build_comments_between(dots_end, type_start, CommentSpacing::Trailing);
                d.concat(&[
                    d.text("..."),
                    comments_doc,
                    self.build_type_doc(&r.type_annotation),
                ])
            }
            TSType::Optional(o) => {
                d.concat(&[self.build_type_doc(&o.type_annotation), d.text("?")])
            }
            TSType::NamedTupleMember(n) => {
                let mut parts = vec![d.symbol(n.label.name.to_u32())];
                if n.optional {
                    parts.push(d.text("?"));
                }
                // Comments between `:` and the element type
                let label_end = n.label.span.end;
                let type_start = n.element_type.span().start;
                // Find `:` between label and type, skipping comments
                let after_colon = find_char_skipping_comments(
                    self.source.as_bytes(),
                    label_end as usize,
                    type_start as usize,
                    b':',
                )
                .map(|p| (p + 1) as u32); // +1 for after `:`
                parts.push(d.text(": "));
                if let Some(after_colon) = after_colon {
                    parts.push(self.build_comments_between(
                        after_colon,
                        type_start,
                        CommentSpacing::Trailing,
                    ));
                }
                parts.push(self.build_type_doc(&n.element_type));
                d.concat(&parts)
            }
            TSType::Infer(i) => {
                // Comments between `infer` and the type parameter name
                let infer_end = i.span.start + 5; // "infer".len()
                let name_start = i.type_parameter.name.span.start;
                let comments_doc =
                    self.build_comments_between(infer_end, name_start, CommentSpacing::Trailing);
                d.concat(&[
                    d.text("infer "),
                    comments_doc,
                    d.symbol(i.type_parameter.name.name.to_u32()),
                ])
            }
            TSType::ThisType(_) => d.text("this"),
        }
    }

    /// Returns true if there's a line comment between `(` and the inner type
    /// of a parenthesized type (e.g., `(// leading\n T)`). Used by every
    /// printer that strips parens around a type to detect when the inner
    /// line comment needs to be relocated.
    pub(in crate::printer) fn paren_has_leading_line_comment(
        &self,
        p: &TSParenthesizedType,
    ) -> bool {
        self.has_line_comments_between(p.span.start + 1, p.type_annotation.span().start)
    }

    /// Collect the line comments between `(` and the inner type of a
    /// parenthesized type. Block comments are excluded — relocation paths
    /// only apply to line comments.
    pub(in crate::printer) fn paren_leading_line_comments(
        &self,
        p: &TSParenthesizedType,
    ) -> Vec<&tsv_lang::Comment> {
        comments_in_range(
            self.comments,
            p.span.start + 1,
            p.type_annotation.span().start,
        )
        .filter(|c| !c.is_block)
        .collect()
    }

    /// Unwrap a parenthesized type, preserving any comments inside the parens.
    ///
    /// Block comments are emitted inline: `(/* c */ a)` → `/* c */ a`
    /// Line comments use `line_suffix` to defer to end of the rendered line,
    /// plus `break_parent` to force the enclosing union/intersection group to break:
    /// `(a // comment\n) | b` → `| a // comment\n| b`
    /// `(a // comment\n) & b` → `a & // comment\nb`
    fn build_parenthesized_type_unwrap_doc(&self, p: &TSParenthesizedType) -> DocId {
        let d = self.d();
        let paren_open = p.span.start;
        let inner_start = p.type_annotation.span().start;
        let inner_end = p.type_annotation.span().end;
        let paren_close = p.span.end;
        let has_leading = self.has_comments_between(paren_open, inner_start);
        let has_trailing = self.has_comments_between(inner_end, paren_close);
        if !has_leading && !has_trailing {
            return self.build_type_doc(&p.type_annotation);
        }

        let mut parts = Vec::new();
        let mut needs_break = false;

        // Leading comments: between `(` and inner type
        if has_leading {
            for comment in comments_in_range(self.comments, paren_open, inner_start) {
                if comment.is_block {
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.text(" "));
                } else {
                    // Line comment before inner type: emit inline + hardline.
                    // A line comment must terminate at end-of-line; using line_suffix
                    // here would defer it past the end of the enclosing construct
                    // and can produce invalid output (e.g., `[// leading a, b]`).
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.hardline());
                    needs_break = true;
                }
            }
        }

        parts.push(self.build_type_doc(&p.type_annotation));

        // Trailing comments: between inner type and `)`
        if has_trailing {
            for comment in comments_in_range(self.comments, inner_end, paren_close) {
                if comment.is_block {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                } else {
                    // Line comment after inner type: defer to end of line, force break
                    let suffix = d.concat(&[d.text(" "), self.build_comment_doc(comment)]);
                    parts.push(d.line_suffix(suffix));
                    needs_break = true;
                }
            }
        }

        if needs_break {
            parts.push(d.break_parent());
        }
        d.concat(&parts)
    }
}
