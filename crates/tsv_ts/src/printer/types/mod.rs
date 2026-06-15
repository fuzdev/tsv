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
pub use helpers::{should_hug_union_type, unwrap_parenthesized};

// Re-export for submodules to use `super::X` instead of `super::super::X`
pub(super) use super::{CommentFilter, CommentSpacing, Printer};

use crate::ast::internal::{TSImportType, TSParenthesizedType, TSType};
use crate::printer::calls::PartitionedComments;
use crate::printer::layout::hang_after_operator;
use helpers::type_needs_parens_for_indexed_access_object;
use helpers::type_needs_parens_for_optional_element;
use helpers::type_needs_parens_for_prefix_operator;
use tsv_lang::SymbolToU32;
use tsv_lang::comments_in_range;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

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
                    // A line comment after `is` stays trailing it, with the
                    // predicate type on the next line (preserve-in-place; prettier
                    // relocates the comment to trail the body `{`).
                    if let Some(is_end) = is_end
                        && self.has_line_comments_between(is_end, type_start)
                    {
                        let value_doc = self.build_type_doc(type_ann);
                        parts.push(d.text(" is"));
                        self.append_keyword_value_line_comments(
                            &mut parts, is_end, type_start, value_doc,
                        );
                    } else {
                        let comments_doc = is_end.map_or_else(
                            || d.empty(),
                            |is_end| {
                                self.build_comments_between(
                                    is_end,
                                    type_start,
                                    CommentSpacing::Trailing,
                                )
                            },
                        );
                        // A long union/intersection hangs after `is` (redundant parens
                        // stripped first); everything else stays inline after `is `.
                        match self.unwrap_redundant_parens(type_ann) {
                            TSType::Union(u) => {
                                let type_doc = self.build_union_type_doc(u, false);
                                parts.push(d.text(" is"));
                                parts.push(hang_after_operator(
                                    d,
                                    d.concat(&[comments_doc, type_doc]),
                                ));
                            }
                            TSType::Intersection(i) => {
                                parts.push(d.text(" is "));
                                parts.push(comments_doc);
                                parts.push(self.intersection_hanging_with_indent(i));
                            }
                            _ => {
                                parts.push(d.text(" is "));
                                parts.push(comments_doc);
                                parts.push(self.build_type_doc(type_ann));
                            }
                        }
                    }
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
                // Comments between keyword and operand type
                let keyword_end = o.span.start + o.operator.as_str().len() as u32;
                let operand_start = o.type_annotation.span().start;
                // A line comment after the operator stays trailing it, with the
                // operand on the next line (matches prettier).
                if self.has_line_comments_between(keyword_end, operand_start) {
                    let operand_doc = self.build_type_doc(&o.type_annotation);
                    let value_doc = if needs_parens {
                        d.concat(&[d.text("("), operand_doc, d.text(")")])
                    } else {
                        operand_doc
                    };
                    let mut parts = vec![d.text(o.operator.as_str())];
                    self.append_keyword_value_line_comments(
                        &mut parts,
                        keyword_end,
                        operand_start,
                        value_doc,
                    );
                    return d.concat(&parts);
                }
                let operand_doc = self.build_type_doc(&o.type_annotation);
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
            TSType::Import(i) => self.build_import_type_doc(i),
            TSType::TypeQuery(q) => {
                // Comments between `typeof` and the expression
                let typeof_end = q.span.start + 6; // "typeof".len()
                let expr_start = q.expr_name.span().start;
                // A line comment after `typeof` stays trailing it, with the
                // expression on the next line (matches prettier).
                if self.has_line_comments_between(typeof_end, expr_start) {
                    let mut value_parts = vec![self.build_type_query_expr_name_doc(&q.expr_name)];
                    if let Some(type_args) = &q.type_arguments {
                        let gap_start = q.expr_name.span().end;
                        if let Some(doc) = self.build_name_to_type_params_comments_opt(
                            gap_start,
                            type_args.span.start,
                            CommentSpacing::Trailing,
                        ) {
                            value_parts.push(doc);
                        }
                        value_parts.push(self.build_type_arguments_doc(type_args));
                    }
                    let value_doc = d.concat(&value_parts);
                    let mut parts = vec![d.text("typeof")];
                    self.append_keyword_value_line_comments(
                        &mut parts, typeof_end, expr_start, value_doc,
                    );
                    return d.concat(&parts);
                }
                let mut parts = vec![d.text("typeof ")];
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
                let index_type_start = i.index_type.span().start;
                let bracket_area_start = i.object_type.span().end;
                // The access `[`, located outside comments so a `[` glyph inside a
                // comment before it (`A /* [ */[K]`) isn't mistaken for the bracket.
                let bracket_open =
                    self.find_char_outside_comments(bracket_area_start, index_type_start, b'[');
                // Comments in the object→`[` gap (`A /* c */[K]`) trail the object
                // in place; comments in the `[`→index gap (`A[/* c */ K]`) lead the
                // index — both preserved where the user placed them.
                let object_comments = bracket_open.and_then(|bp| {
                    self.build_inline_comments_between_doc_opt(bracket_area_start, bp)
                });
                let index_comments = bracket_open.map(|bp| {
                    self.build_comments_between(bp + 1, index_type_start, CommentSpacing::Trailing)
                });
                let index_doc = self.build_type_doc(&i.index_type);
                let mut parts = if needs_parens {
                    vec![d.text("("), object_doc, d.text(")")]
                } else {
                    vec![object_doc]
                };
                if let Some(c) = object_comments {
                    parts.push(c);
                }
                parts.push(d.text("["));
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
                let inner = self.build_type_doc_maybe_parens(
                    &o.type_annotation,
                    type_needs_parens_for_optional_element,
                );
                d.concat(&[inner, d.text("?")])
            }
            TSType::NamedTupleMember(n) => {
                let mut parts = vec![d.symbol(n.label.name.to_u32())];
                let label_end = n.label.span.end;
                let type_start = n.element_type.span().start;
                // Comments between label and `?` (e.g., `[a /* c */?: T]`)
                let after_modifier = if n.optional {
                    self.push_modifier_marker_doc(&mut parts, label_end, b'?')
                } else {
                    label_end
                };
                // Find `:` between label/`?` and type, skipping comments
                let after_colon = find_char_skipping_comments(
                    self.source.as_bytes(),
                    after_modifier as usize,
                    type_start as usize,
                    b':',
                )
                .map(|p| (p + 1) as u32); // +1 for after `:`
                // Comments between label/`?` and `:` (e.g., `[b /* c */: T]`)
                if let Some(after_colon) = after_colon
                    && self.has_comments_between(after_modifier, after_colon - 1)
                {
                    parts.push(
                        self.build_inline_comments_between_doc(after_modifier, after_colon - 1),
                    );
                }
                let comments_doc = after_colon.map_or_else(
                    || d.empty(),
                    |after_colon| {
                        self.build_comments_between(
                            after_colon,
                            type_start,
                            CommentSpacing::Trailing,
                        )
                    },
                );
                // A long union/intersection element hangs after `:` (redundant parens
                // stripped first); everything else stays inline after `: `.
                match self.unwrap_redundant_parens(&n.element_type) {
                    TSType::Union(u) => {
                        let type_doc = self.build_union_type_doc(u, false);
                        parts.push(d.text(":"));
                        parts.push(hang_after_operator(d, d.concat(&[comments_doc, type_doc])));
                    }
                    TSType::Intersection(i) => {
                        parts.push(d.text(": "));
                        parts.push(comments_doc);
                        parts.push(self.intersection_hanging_with_indent(i));
                    }
                    _ => {
                        parts.push(d.text(": "));
                        parts.push(comments_doc);
                        parts.push(self.build_type_doc(&n.element_type));
                    }
                }
                d.concat(&parts)
            }
            TSType::Infer(i) => {
                // Comments between `infer` and the type parameter name
                let infer_end = i.span.start + 5; // "infer".len()
                let name_start = i.type_parameter.name.span.start;
                let comments_doc =
                    self.build_comments_between(infer_end, name_start, CommentSpacing::Trailing);
                // Delegate the name + optional `extends C` constraint to the shared
                // type-parameter doc builder — prettier's `printInferType` is
                // `["infer ", print("typeParameter")]`, so an infer constraint lays
                // out identically to a `<T extends C>` declaration constraint.
                d.concat(&[
                    d.text("infer "),
                    comments_doc,
                    self.build_type_parameter_doc(&i.type_parameter, true),
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

    /// Build a complete import type: the `import(<specifier>)` call plus its
    /// optional `.qualifier` and `<type args>`, preserving comments at each
    /// boundary. Shared by `TSType::Import` and the `typeof import(...)` form
    /// (`TSTypeQueryExprName::Import`), which must format identically.
    pub(in crate::printer) fn build_import_type_doc(&self, i: &TSImportType) -> DocId {
        let d = self.d();
        // Closing `)` of the `import(...)` call, skipping any inside comments.
        let after_args = i
            .options
            .as_ref()
            .map_or(i.argument.span.end, |o| o.span().end);
        let paren_close = self
            .find_char_outside_comments(after_args, i.span.end, b')')
            .unwrap_or(after_args);

        let mut parts = vec![self.build_import_type_call_doc(i, paren_close)];
        if let Some(qualifier) = &i.qualifier {
            // Comments between `)` and qualifier (e.g. `import('a') /* c */ .Foo`)
            let dot_area_start = paren_close + 1;
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
                .map_or(paren_close + 1, |q| q.span().end);
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

    /// Build the `import(<specifier>)` call portion of an import type, preserving
    /// comments between `import(` and the specifier (leading) and between the
    /// specifier and `)` (trailing). Leading comments go through the shared
    /// `build_paren_leading_value_doc` (also used by the dynamic-import expression in
    /// `calls/import_expr.rs`). Qualifier / type arguments are appended by the caller.
    ///
    /// - leading line / own-line block comment → break the parens multiline
    /// - inline block comment → stay inline (`import(/* c */ 'a')`)
    /// - trailing line comment → break multiline; trailing block → inline
    fn build_import_type_call_doc(&self, i: &TSImportType, paren_close: u32) -> DocId {
        let d = self.d();
        let open_paren_end = i.span.start + 7; // "import(".len()
        let arg_start = i.argument.span.start;
        let arg_end = i.argument.span.end;
        let literal_doc = self.build_literal_doc(&i.argument);

        // Options present: keep the inline `import('a', {...})` layout, preserving
        // any leading comments before the specifier.
        if let Some(options) = &i.options {
            let arg_doc = match self.build_rhs_comments_opt(open_paren_end, arg_start) {
                Some(lead) => d.concat(&[lead, literal_doc]),
                None => literal_doc,
            };
            return d.concat(&[
                d.text("import("),
                arg_doc,
                d.text(", "),
                self.build_expression_doc(options),
                d.text(")"),
            ]);
        }

        // Leading comments between `import(` and the specifier.
        let (arg_doc, leading_forces_break) =
            self.build_paren_leading_value_doc(open_paren_end, arg_start, literal_doc);

        // Trailing comments between the specifier and `)`.
        let has_trailing = self.has_comments_between(arg_end, paren_close);
        let has_trailing_line = self.has_line_comments_between(arg_end, paren_close);

        let mut inner = vec![arg_doc];
        if has_trailing {
            let pc =
                PartitionedComments::new(self.comments, self.line_breaks, arg_end, paren_close);
            pc.emit_trailing_comments(&mut inner, self);
        }
        let inner = d.concat(&inner);

        if leading_forces_break || has_trailing_line {
            // Line / own-line comments force the parens to break across lines.
            d.concat(&[
                d.text("import("),
                d.indent(d.concat(&[d.hardline(), inner])),
                d.hardline(),
                d.text(")"),
            ])
        } else {
            // Block comments only (or none) — stay inline.
            d.concat(&[d.text("import("), inner, d.text(")")])
        }
    }

    /// Whether a `TSParenthesizedType` carries comments inside its parens, as
    /// `(has_leading, has_trailing)` flags — leading = between `(` and the inner
    /// type, trailing = between the inner type and `)`. Used both to decide
    /// whether redundant parens can be stripped and to emit the comments in place
    /// when they can't.
    pub(in crate::printer) fn paren_inner_comment_flags(
        &self,
        p: &TSParenthesizedType,
    ) -> (bool, bool) {
        let inner = p.type_annotation.span();
        (
            self.has_comments_between(p.span.start, inner.start),
            self.has_comments_between(inner.end, p.span.end),
        )
    }

    /// Unwrap redundant, comment-free `TSParenthesizedType` layers to find the
    /// effective inner type for a layout decision. Parens around a union /
    /// intersection in type-alias-RHS, cast (`as` / `satisfies`), return-type,
    /// and type-member positions are redundant — prettier strips them — so a
    /// `(union)` / `(intersection)` should get the same break layout as the bare
    /// form (leading `| ` for unions, hanging indent for intersections) rather
    /// than hanging inline. Stops at a paren that carries comments — those are
    /// preserved in place by `build_parenthesized_type_unwrap_doc`.
    pub(in crate::printer) fn unwrap_redundant_parens<'t>(&self, ty: &'t TSType) -> &'t TSType {
        match ty {
            TSType::Parenthesized(p) if self.paren_inner_comment_flags(p) == (false, false) => {
                self.unwrap_redundant_parens(p.type_annotation.as_ref())
            }
            other => other,
        }
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
        let (has_leading, has_trailing) = self.paren_inner_comment_flags(p);
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
