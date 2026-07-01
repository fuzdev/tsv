// Type parameter printing for TypeScript
//
// Handles:
// - Type parameter declarations: `<T, U extends V = W>`
// - Type parameter instantiation (type arguments): `<T, U>`

use super::{CommentFilter, CommentSpacing, Printer};
use crate::ast::internal::{self, TSType, TSTypeParameter, TSTypeParameterDeclaration};
use crate::printer::layout::fluid_after_operator;
use smallvec::smallvec;
use tsv_lang::SymbolToU32;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::GroupId;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

impl<'a> Printer<'a> {
    //
    // Type Parameter Declarations
    //

    /// Build doc for type parameter declaration: `<T, U extends V = W>`
    /// Non-wrapping version - always inline, unless expanding comments force multiline
    pub(in crate::printer) fn build_type_parameter_declaration_doc(
        &self,
        decl: &TSTypeParameterDeclaration<'_>,
    ) -> DocId {
        if self.has_expanding_comments_in_type_param_declaration(decl) {
            return self.build_type_parameter_declaration_doc_with_line_comments(decl);
        }

        let d = self.d();
        let (param_docs, deferred_after) = self.build_type_parameter_docs_with_comments(decl);
        d.concat(&[
            d.text("<"),
            d.join(param_docs, ", "),
            deferred_after,
            d.text(">"),
        ])
    }

    /// Build doc for type parameter declaration with wrapping support
    /// When the group breaks, each param goes on its own line with trailing comma
    pub(crate) fn build_type_parameter_declaration_doc_wrapping(
        &self,
        decl: &TSTypeParameterDeclaration<'_>,
    ) -> DocId {
        self.d()
            .group(self.build_type_parameter_declaration_doc_inner(decl))
    }

    /// Build doc for type parameter declaration - inner version without group wrapper
    /// Used when caller wants to control the group (e.g., interface header)
    pub(in crate::printer) fn build_type_parameter_declaration_doc_inner(
        &self,
        decl: &TSTypeParameterDeclaration<'_>,
    ) -> DocId {
        let d = self.d();
        if decl.params.is_empty() {
            return d.text("<>");
        }

        if self.has_expanding_comments_in_type_param_declaration(decl) {
            return self.build_type_parameter_declaration_doc_with_line_comments(decl);
        }

        let (param_docs, deferred_after) = self.build_type_parameter_docs_with_comments(decl);
        let inner = d.concat(&[d.join_doc(param_docs, d.comma_line()), deferred_after]);
        d.concat(&[
            d.text("<"),
            d.indent_softline(inner),
            d.softline(),
            d.text(">"),
        ])
    }

    /// Build doc for type parameter declaration with expanding comments
    pub(in crate::printer) fn build_type_parameter_declaration_doc_with_line_comments(
        &self,
        decl: &TSTypeParameterDeclaration<'_>,
    ) -> DocId {
        let d = self.d();
        let mut inner_parts = DocBuf::new();
        let mut prev_end = decl.span.start + 1; // After the opening `<`

        // A line comment trailing the opening `<` is kept on the `<` line (divergence
        // from prettier, which relocates it to its own line as the first param's
        // leading comment). See conformance_prettier.md §Comment relocation
        // (Type-parameter `<` trailing). Same mechanism as the object/array/block
        // and call-`(` open-delimiter family.
        let first_start = decl.params[0].span.start; // caller guarantees non-empty
        let (angle_prefix, angle_pull_pos) =
            self.delimiter_line_comment_prefix(decl.span.start, first_start);

        for (i, param) in decl.params.iter().enumerate() {
            let param_start = param.span.start;
            let param_end = param.span.end;
            let is_last = i == decl.params.len() - 1;

            // Leading comments (after previous comma or `<`); for the first param,
            // exclude comments already pulled onto the `<` line.
            let skip_delim = if i == 0 { angle_pull_pos } else { None };
            inner_parts.extend(self.build_leading_comments_multiline_opt(
                prev_end,
                param_start,
                skip_delim,
            ));

            inner_parts.push(self.build_type_parameter_doc(param));

            if !is_last {
                let next_start = decl.params[i + 1].span.start;
                prev_end = self.emit_multiline_comma_with_comments(
                    &mut inner_parts,
                    param_end,
                    next_start,
                    false,
                );
            } else {
                // Last param: no trailing comma under `trailingComma: 'none'`, then
                // comments before `>`.
                let before_close = decl.span.end - 1;
                inner_parts.extend(self.build_trailing_comments_multiline(param_end, before_close));
                prev_end = before_close;
            }
        }

        d.concat(&[
            d.text("<"),
            d.concat(&angle_prefix),
            d.indent(d.concat(&[d.hardline(), d.concat(&inner_parts)])),
            d.hardline(),
            d.text(">"),
        ])
    }

    /// Check for expanding comments in type param declarations: line comments,
    /// own-line block comments, or line comments inside param spans (e.g.,
    /// `T extends // comment\n  A`). Used by both wrapping and non-wrapping paths.
    pub(in crate::printer) fn has_expanding_comments_in_type_param_declaration(
        &self,
        decl: &TSTypeParameterDeclaration<'_>,
    ) -> bool {
        let Some(first) = decl.params.first() else {
            return false;
        };
        // A line comment trailing the opening `<` (`<// c\n T>`) forces expansion;
        // `has_line_comments_in_delimited_list` only covers between/after params,
        // not the `<`→first-param gap, so check it explicitly. Without this the
        // inline path runs and emits block-only comments, dropping the line comment
        // entirely (content loss). Own-line block comments in this gap are already
        // handled by `has_own_line_block_comments_in_bracket_list`.
        self.has_line_comments_between(decl.span.start + 1, first.span.start)
            || self.has_line_comments_in_delimited_list(decl.params, |p| p.span, decl.span.end - 1)
            || self.has_own_line_block_comments_in_bracket_list(decl.span, decl.params, |p| p.span)
            || decl
                .params
                .iter()
                // A line comment or multiline block in a param's constraint/default gap
                // (`<T extends⏎// c⏎U>`) forces the whole `<…>` to expand, so the hang
                // renders inside the broken list; a single-line block comment collapses
                // inline and keeps `<…>` collapsed.
                .any(|p| self.comments_force_own_line_between(p.span.start, p.span.end))
    }

    /// Build enriched param docs with surrounding block comments from the declaration.
    /// Comments outside param spans (e.g., `</* c */ T /* c */>`) are captured here.
    /// Uses comma position to split: comments before comma = trailing, after = leading.
    /// Returns the per-param docs and a deferred doc for a block comment trailing
    /// the last param after its source comma — emitted by the caller past where the
    /// comma was (no trailing comma; trailingComma: 'none') so the comment is preserved
    /// after it rather than relocated before (prettier relocates; see conformance_prettier.md).
    fn build_type_parameter_docs_with_comments(
        &self,
        decl: &TSTypeParameterDeclaration<'_>,
    ) -> (DocBuf, DocId) {
        let d = self.d();
        let mut prev_end = decl.span.start + 1; // After `<`
        let mut deferred_after = d.empty();
        let param_docs = decl
            .params
            .iter()
            .enumerate()
            .map(|(i, param)| {
                let mut parts = DocBuf::new();
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
                    // Last param: split trailing block comments around a source comma.
                    // Before-comma stay with the param; after-comma are deferred past
                    // where the comma was by the caller (no trailing comma).
                    let before_close = decl.span.end - 1;
                    match self
                        .find_comma_after(param.span.end)
                        .filter(|cp| *cp < before_close)
                    {
                        Some(comma_pos) => {
                            parts.push(self.build_comments_between_filtered(
                                param.span.end,
                                comma_pos,
                                CommentSpacing::Leading,
                                CommentFilter::BlockOnly,
                            ));
                            deferred_after = self.build_comments_between_filtered(
                                comma_pos,
                                before_close,
                                CommentSpacing::Leading,
                                CommentFilter::BlockOnly,
                            );
                        }
                        None => parts.push(self.build_comments_between_filtered(
                            param.span.end,
                            before_close,
                            CommentSpacing::Leading,
                            CommentFilter::BlockOnly,
                        )),
                    }
                }
                d.concat(&parts)
            })
            .collect();
        (param_docs, deferred_after)
    }

    /// Build doc for a single type parameter
    /// With optional modifiers: `const T`, `in T`, `out T`, `in out T`
    ///
    /// A *conditional* type in `extends` constraint position keeps its parens
    /// (`<T extends (A extends B ? C : D)>`) — prettier keeps them for clarity,
    /// and for an `infer`'s conditional constraint they're required (without them
    /// the enclosing `? :` rebinds and the result fails to parse). The `=` default
    /// position strips redundant parens. See `append_keyword_value_with_comments`.
    pub(in crate::printer) fn build_type_parameter_doc(
        &self,
        param: &TSTypeParameter<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

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
            // Find `extends` keyword between name and constraint. A `TSTypeParameter`
            // constraint is always spelled `extends` — mapped-type `[K in T]` keys use
            // `in`, but those take a separate `TSMappedTypeParameter`/`build_mapped_type_doc`
            // path and never reach here, so the keyword is guaranteed present.
            #[allow(clippy::expect_used)] // extends always present for a type-parameter constraint
            let extends_pos = self
                .find_keyword_in_range(prev_end, constraint.span().start, "extends")
                .expect("extends keyword must exist when constraint is present");
            let extends_end = extends_pos + "extends".len() as u32;

            // Comments between name and `extends`: <T /* c */ extends A>
            parts.push(self.build_comments_between(prev_end, extends_pos, CommentSpacing::Leading));

            parts.push(d.text(" extends"));
            // If the constraint is `(// leading\n T)`, treat the leading line
            // comment inside the parens as if it were between `extends` and the
            // constraint so it forces the indent-and-break layout (matching
            // prettier's paren stripping).
            let (value_search_end, value_type): (u32, &TSType<'_>) = if let TSType::Parenthesized(p) =
                constraint
                && self.paren_has_leading_line_comment(p)
            {
                (p.type_annotation.span().start, p.type_annotation)
            } else {
                (constraint.span().start, constraint)
            };
            self.append_keyword_value_with_comments(
                &mut parts,
                extends_end,
                value_search_end,
                value_type,
                GroupId::TypeParameterConstraint,
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
                default,
                GroupId::TypeParameterDefault,
            );
            prev_end = default.span().end;
        }

        // Trailing comments after last part: <T /* c */> or <T extends A /* c */>
        parts.push(self.build_comments_between(prev_end, param.span.end, CommentSpacing::Leading));

        d.concat(&parts)
    }

    /// Append a constraint/default value after its keyword (`extends` / `=`),
    /// handling comments in between.
    /// Block comments are inlined: `extends /* c */ A`
    /// Line comments force break+indent: `extends // c\n  A`
    /// No comments, non-hugging union: hanging indent (`extends\n  | A\n  | B`)
    /// No comments, otherwise: break after the keyword and indent when the value
    /// overflows (`extends\n  Long`), hugging object-like types (`extends {`).
    ///
    /// `group_id` ties the after-keyword line break to `indent_if_break` so the
    /// value is indented exactly when that break fires — Prettier's
    /// `printTypeParameter` pattern.
    fn append_keyword_value_with_comments(
        &self,
        parts: &mut DocBuf,
        keyword_end: u32,
        value_start: u32,
        value_type: &TSType<'_>,
        group_id: GroupId,
    ) {
        let d = self.d();
        // Strip redundant comment-free parens so `(A | B)` / `(A & B)` constraints
        // and defaults get the bare hanging layout (prettier strips them too).
        let value_type = self.unwrap_redundant_parens(value_type);
        // A *conditional* type used as a constraint keeps its parens: prettier keeps
        // them for clarity, and for an `infer`'s conditional constraint they're
        // outright required (the enclosing conditional's `? :` rebinds without them —
        // prettier drops them there, producing unparseable output, a documented
        // divergence). The `=` default position strips them.
        if matches!(value_type, TSType::Conditional(_))
            && group_id == GroupId::TypeParameterConstraint
        {
            let comments =
                self.build_comments_between(keyword_end, value_start, CommentSpacing::Leading);
            parts.push(d.concat(&[
                d.text(" ("),
                comments,
                self.build_type_doc(value_type),
                d.text(")"),
            ]));
            return;
        }
        if self.comments_force_own_line_between(keyword_end, value_start) {
            // A line comment or multiline block after the keyword hangs the bound type
            // on its own line (and expands the `<…>` via the gate at :163). A
            // single-line block comment (own-line, trailing, or glued) collapses inline
            // and keeps `<…>` collapsed (the fall-through below).
            let value_doc = self.build_type_doc(value_type);
            self.append_keyword_value_line_comments(parts, keyword_end, value_start, value_doc);
            return;
        }
        let comments = self.build_comments_between_filtered_opt(
            keyword_end,
            value_start,
            CommentSpacing::Leading,
            CommentFilter::All,
        );
        if let Some(c) = comments {
            parts.push(c);
            // Block comment present: keep the value inline after it.
            parts.push(d.text(" "));
            parts.push(self.build_type_doc(value_type));
            return;
        }
        // No comments: a non-hugging union breaks after the keyword with a
        // hanging indent (Prettier's shouldIndentUnionType — true for type
        // parameter constraints and defaults).
        if let Some(hanging) = self.build_union_hanging_indent_doc(value_type) {
            parts.push(hanging);
            return;
        }
        // Intersection: first member hugs the keyword, continuations indented.
        if let TSType::Intersection(i) = value_type {
            parts.push(d.text(" "));
            parts.push(self.intersection_hanging_with_indent(i));
            return;
        }
        // Other types: break after the keyword and indent when the value would
        // overflow. The group holds only the line, so an object-like type still
        // hugs the keyword (`extends {`) while a plain type wraps and indents.
        parts.push(fluid_after_operator(
            d,
            self.build_type_doc(value_type),
            group_id,
        ));
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
        inst: &internal::TSTypeParameterInstantiation<'_>,
    ) -> DocId {
        let d = self.d();
        if inst.params.is_empty() {
            return d.text("<>");
        }

        // Check for comments that force expansion: line comments or own-line block
        // comments. Also check for a line comment BETWEEN `<` and the first argument
        // (e.g. `foo<// c\n A>(x)`); without this the comment falls through to the
        // block-comment-only group path below and is dropped (content loss).
        let has_leading_line_comment = inst.params.first().is_some_and(|first| {
            self.has_line_comments_between(inst.span.start + 1, first.span().start)
        });
        if has_leading_line_comment
            || self.has_line_comments_in_delimited_list(
                inst.params,
                TSType::span,
                inst.span.end - 1,
            )
            || self.has_own_line_block_comments_in_bracket_list(
                inst.span,
                inst.params,
                TSType::span,
            )
        {
            return self.build_type_parameter_instantiation_doc_with_line_comments(inst);
        }

        // Special case: a single curly-brace type argument hugs the opening
        // bracket. tsv keeps `<{` together for a single object/mapped type even
        // when it carries an interior comment; the type carries its own group so
        // it still breaks block-style when too wide. (This is the same layout the
        // type-reference type-argument path uses. Prettier instead breaks the
        // `<…>` onto its own lines for a comment-bearing mapped/empty type — a
        // deliberate divergence; see docs/conformance_prettier.md.)
        if inst.params.len() == 1
            && let Some(type_doc) = self.try_build_hugging_curly_type_doc(&inst.params[0])
        {
            return d.concat(&[d.text("<"), type_doc, d.text(">")]);
        }

        // Build params with commas and line breaks
        // The doc printer's look-ahead (fits_with_lookahead) handles the decision
        // of whether to break based on what follows the type params.
        let mut param_parts = DocBuf::new();
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
        inst: &internal::TSTypeParameterInstantiation<'_>,
    ) -> DocId {
        // Call/`new`-expression type arguments render each argument with
        // `build_type_doc`; the layout is shared with type-position arguments.
        self.build_angle_list_with_line_comments(inst, false)
    }

    /// Render a type-argument list `<…>` that breaks onto multiple lines because it
    /// carries comments — the shared body behind the call/`new`-expression
    /// ([`Self::build_type_parameter_instantiation_doc_with_line_comments`]) and type-position
    /// ([`Self::build_type_arguments_doc_with_line_comments`]) printers. `type_position`
    /// selects the per-argument doc builder (`build_type_arg_doc` vs `build_type_doc`).
    ///
    /// A single argument with a leading *line* comment hugs `<`/`>` (`foo<// c\n A>`) —
    /// a deliberate divergence (prettier expands; see
    /// `type_position_parens_leading_line_comment`). Every other comment-bearing form —
    /// a single-argument own-line *block* comment, or any multi-argument list — fully
    /// expands the list, matching prettier. The own-line block must NOT hug, or the
    /// emitted `</* c */⏎T>` re-collapses on the next pass (non-idempotent). A block
    /// trailing/glued to the argument never reaches here (it doesn't trip
    /// `has_own_line_block_comments_in_bracket_list`) and collapses inline.
    pub(in crate::printer) fn build_angle_list_with_line_comments(
        &self,
        inst: &internal::TSTypeParameterInstantiation<'_>,
        type_position: bool,
    ) -> DocId {
        let d = self.d();
        let is_multi = inst.params.len() > 1;

        // Single-arg leading *line* comment hugs `<`/`>`.
        if !is_multi {
            let param = &inst.params[0];
            let param_start = param.span().start;
            let has_line = self.has_line_comments_between(inst.span.start + 1, param_start);
            let before_close = inst.span.end - 1;
            let has_trailing =
                tsv_lang::has_comments_in_range(self.comments, param.span().end, before_close);
            if has_line && !has_trailing {
                let leading =
                    self.build_leading_comments_multiline(inst.span.start + 1, param_start);
                if !leading.is_empty() {
                    let param_doc = if type_position {
                        self.build_type_arg_doc(param, is_multi)
                    } else {
                        self.build_type_doc(param)
                    };
                    let mut parts: DocBuf = smallvec![d.text("<")];
                    parts.extend(leading);
                    parts.push(param_doc);
                    parts.push(d.text(">"));
                    return d.concat(&parts);
                }
            }
        }

        // Full multiline expansion (multi-arg, or single-arg own-line block). A
        // comment trailing `<` on its own line is kept on the `<` line (divergence —
        // prettier relocates it to lead the first argument).
        let first_param_start = inst.params[0].span().start;
        let (angle_line_prefix, delimiter_pull_pos) =
            self.delimiter_line_comment_prefix(inst.span.start, first_param_start);

        let mut inner_parts = DocBuf::new();
        let mut prev_end = inst.span.start + 1; // After the opening `<`

        for (i, param) in inst.params.iter().enumerate() {
            let param_start = param.span().start;
            let param_end = param.span().end;
            let is_last = i == inst.params.len() - 1;

            // Leading comments (after previous comma or `<`). For the first arg,
            // drop comments pulled onto the `<` line (emitted as the angle-line
            // prefix below).
            let skip_delim = if i == 0 { delimiter_pull_pos } else { None };
            inner_parts.extend(self.build_leading_comments_multiline_opt(
                prev_end,
                param_start,
                skip_delim,
            ));

            inner_parts.push(if type_position {
                self.build_type_arg_doc(param, is_multi)
            } else {
                self.build_type_doc(param)
            });

            if !is_last {
                let next_start = inst.params[i + 1].span().start;
                prev_end = self.emit_multiline_comma_with_comments(
                    &mut inner_parts,
                    param_end,
                    next_start,
                    false,
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
            d.concat(&angle_line_prefix),
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
    ///
    /// The object/mapped type carries its own width-aware group, so an inline
    /// `<{ ... }>` that overflows breaks block-style (members on their own lines)
    /// rather than spilling an inner union/intersection — matching the type-reference
    /// type-argument path (`build_type_arguments_doc_wrapping`).
    fn try_build_hugging_curly_type_doc(&self, ty: &TSType<'_>) -> Option<DocId> {
        match ty {
            // Object type literal: { a: number; b: string } or { /* comment */ }
            // Hug if it has members OR comments inside. Standard (not hugging) mode
            // so the object breaks block-style on width, the same as elsewhere.
            TSType::TypeLiteral(type_lit)
                if !type_lit.members.is_empty()
                    || self.has_comments_between(type_lit.span.start, type_lit.span.end) =>
            {
                Some(self.build_type_literal_doc(type_lit))
            }
            // Mapped type: { [K in keyof T]: V }
            TSType::Mapped(mapped) => Some(self.build_mapped_type_doc(mapped)),
            _ => None,
        }
    }
}
