// Function type printing for TypeScript
//
// Handles:
// - Function types: `(a: T) => U`
// - Constructor types: `new () => T`
// - Signature parameters (shared with type members)
// - Return type annotations

use super::super::comments_in_range;
use super::helpers::type_args_should_wrap_for_return_type;
use super::{CommentSpacing, Printer};
use crate::ast::internal::{self, TSConstructorType, TSFunctionType, TSType};
use crate::printer::analysis::find_char_skipping_comments;
use tsv_lang::SymbolToU32;
use tsv_lang::doc::arena::{DocArena, DocId};

/// Check if an expression is an identifier with a TypeLiteral type annotation.
///
/// Used for function param hugging: `fn: (options: { a: T }) => U`
/// - The opening `{` stays on the same line as the parameter name
/// - The content expands internally
/// - The closing `}` comes on its own line when broken
///
/// Note: Only TypeLiteral is handled specially. Mapped types (`{ [K in T]: V }`)
/// also pass `is_huggable_type` but use standard param formatting.
fn get_type_literal_from_identifier(
    expr: &internal::Expression,
) -> Option<(
    &internal::Identifier,
    &internal::TSTypeAnnotation,
    &internal::TSTypeLiteral,
)> {
    match expr {
        internal::Expression::Identifier(id) => {
            id.type_annotation
                .as_ref()
                .and_then(|ann| match ann.type_annotation.as_ref() {
                    TSType::TypeLiteral(t) => Some((id, ann, t)),
                    _ => None,
                })
        }
        _ => None,
    }
}

/// Check if type parameters allow function parameter grouping.
///
/// Returns true when there are 0 type params, or exactly 1 without constraints/defaults.
/// Shared between function declarations and function/constructor types.
pub(in crate::printer) fn type_params_allow_grouping(
    type_parameters: Option<&internal::TSTypeParameterDeclaration>,
) -> bool {
    let Some(tp) = type_parameters else {
        return true;
    };
    if tp.params.len() > 1 {
        return false;
    }
    tp.params
        .first()
        .is_none_or(|p| p.constraint.is_none() && p.default.is_none())
}

/// Check if a return type qualifies for function parameter grouping.
///
/// Returns true when the return type is an object type (TypeLiteral/Mapped)
/// or the return type doc will break across lines.
pub(in crate::printer) fn return_type_triggers_grouping(
    return_type: &internal::TSTypeAnnotation,
    return_type_doc: DocId,
    d: &DocArena,
) -> bool {
    matches!(
        &*return_type.type_annotation,
        TSType::TypeLiteral(_) | TSType::Mapped(_)
    ) || d.will_break(return_type_doc)
}

impl<'a> Printer<'a> {
    //
    // Function Type Return Types
    //

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
    fn build_function_type_return_doc(&self, return_type: &internal::TSTypeAnnotation) -> DocId {
        let d = self.d();
        // Comments between `=>` and the return type (e.g., `() => /* c */ string`)
        // For function types, the annotation span starts at `=` in `=>`
        let arrow_end = return_type.span.start + 2; // after `=>`
        let type_start = return_type.type_annotation.span().start;
        // Use break-for-line variant: line comments must force a hardline before
        // the return type so they don't swallow it (`=> // c\nT`, not `=> // c T`).
        let comments_doc = self.build_trailing_comments_break_for_line(arrow_end, type_start);
        match return_type.type_annotation.as_ref() {
            TSType::Union(u) => {
                let type_doc = self.build_union_type_doc(u, false);
                d.concat(&[
                    d.text(" =>"),
                    d.group(d.indent_line(d.concat(&[comments_doc, type_doc]))),
                ])
            }
            TSType::Intersection(i) => {
                // Intersections use trailing `&` - first type NOT indented, continuations indented
                // The intersection doc handles this internally, we just need proper grouping
                let type_doc = self.build_intersection_type_doc(i, false);
                d.concat(&[d.text(" => "), comments_doc, d.group(d.indent(type_doc))])
            }
            // TypeReference with complex type args (like Promise<Result<...>>):
            // Build with wrapping type args so it can break inside the <...>
            TSType::TypeReference(r)
                if r.type_arguments
                    .as_ref()
                    .is_some_and(type_args_should_wrap_for_return_type) =>
            {
                // Use build_type_doc_inner with wrap_type_args=true to enable
                // wrapping inside the type reference's type arguments
                let type_doc = self.build_type_doc_inner(&return_type.type_annotation, true);
                d.concat(&[d.text(" => "), comments_doc, type_doc])
            }
            _ => d.concat(&[
                d.text(" => "),
                comments_doc,
                self.build_type_doc(&return_type.type_annotation),
            ]),
        }
    }

    //
    // Function and Constructor Types
    //

    /// Build a Doc for a function type: `(a: T) => U`
    ///
    /// Uses width-aware wrapping similar to arrow functions.
    /// Applies `shouldGroupFunctionParameters` when there's 1 param and the
    /// return type is an object type or will break — params are wrapped in
    /// their own group so they stay flat when the outer group breaks.
    pub(super) fn build_function_type_doc(&self, f: &TSFunctionType) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        if let Some(type_params) = &f.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        let paren_search_start = f
            .type_parameters
            .as_ref()
            .map_or(f.span.start, |tp| tp.span.end);

        // Comments between type_params and `(` go after type_params
        if let Some(tp) = &f.type_parameters
            && let Some(pp) = find_char_skipping_comments(
                self.source.as_bytes(),
                tp.span.end as usize,
                self.source.len(),
                b'(',
            )
        {
            self.append_type_params_to_paren_comments(&mut parts, tp.span.end, pp as u32);
        }

        parts.extend(self.build_grouped_params_and_return_type(
            &f.params,
            paren_search_start,
            &f.return_type,
            f.type_parameters.as_ref(),
        ));

        d.group(d.concat(&parts))
    }

    /// Build a Doc for a constructor type: `new () => T` or `abstract new <T>() => T`
    pub(super) fn build_constructor_type_doc(&self, c: &TSConstructorType) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        if c.abstract_ {
            parts.push(d.text("abstract "));
        }
        parts.push(d.text("new "));

        if let Some(type_params) = &c.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        let paren_search_start = c
            .type_parameters
            .as_ref()
            .map_or(c.span.start, |tp| tp.span.end);

        // Comments between type_params and `(` go after type_params
        if let Some(tp) = &c.type_parameters
            && let Some(pp) = find_char_skipping_comments(
                self.source.as_bytes(),
                tp.span.end as usize,
                self.source.len(),
                b'(',
            )
        {
            self.append_type_params_to_paren_comments(&mut parts, tp.span.end, pp as u32);
        }

        parts.extend(self.build_grouped_params_and_return_type(
            &c.params,
            paren_search_start,
            &c.return_type,
            c.type_parameters.as_ref(),
        ));

        d.group(d.concat(&parts))
    }

    /// Build params + return type docs with optional parameter grouping.
    ///
    /// Implements Prettier's `shouldGroupFunctionParameters` for function/constructor
    /// types: when there's 1 param and the return type is an object type or will break,
    /// wraps params in their own group so they stay flat when the outer group breaks.
    fn build_grouped_params_and_return_type(
        &self,
        params: &[internal::Expression],
        paren_search_start: u32,
        return_type: &internal::TSTypeAnnotation,
        type_parameters: Option<&internal::TSTypeParameterDeclaration>,
    ) -> [DocId; 2] {
        let d = self.d();

        // Build return type first so we can check will_break for grouping
        let return_type_doc = self.build_function_type_return_doc(return_type);

        let params_doc = d.concat(&self.build_function_params_doc(params, paren_search_start));
        let params_doc = if params.len() == 1
            && type_params_allow_grouping(type_parameters)
            && return_type_triggers_grouping(return_type, return_type_doc, d)
        {
            d.group(params_doc)
        } else {
            params_doc
        };

        [params_doc, return_type_doc]
    }

    //
    // Signature Helpers (shared with type members)
    //

    /// Find closing paren position given opening paren position
    pub(super) fn find_close_paren(&self, paren_pos: u32) -> Option<u32> {
        self.source[paren_pos as usize + 1..]
            .find(')')
            .map(|p| paren_pos + 1 + p as u32)
    }

    /// Build return type annotation with comment handling between `)` and `:`
    /// Used by MethodSignature, CallSignature, ConstructSignature
    pub(super) fn build_signature_return_type_doc(
        &self,
        paren_pos: Option<u32>,
        return_type: &internal::TSTypeAnnotation,
    ) -> DocId {
        let d = self.d();
        let mut parts = vec![];
        let mut has_comment = false;

        if let Some(paren_pos) = paren_pos
            && let Some(close_pos) = self.find_close_paren(paren_pos)
        {
            for comment in comments_in_range(self.comments, close_pos + 1, return_type.span.start) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
                has_comment = true;
            }
        }

        // Prettier adds space before `:` when there's a preceding comment
        if has_comment {
            parts.push(d.text(" "));
        }
        parts.push(self.build_type_annotation_doc(return_type));
        d.concat(&parts)
    }

    /// Build signature params doc with width-based breaking.
    ///
    /// Inline: `(param1: Type1, param2: Type2)`
    /// Broken: `(\n\tparam1: Type1,\n\tparam2: Type2,\n)`
    ///
    /// Used by MethodSignature, CallSignature, ConstructSignature in both
    /// TypeLiteral and interface contexts.
    pub(in crate::printer) fn build_signature_params_doc(
        &self,
        params: &[internal::Expression],
        paren_pos: Option<u32>,
    ) -> DocId {
        let d = self.d();
        if params.is_empty() {
            // Handle comments inside empty params (e.g., `a(/* comment */): void`)
            if let Some(paren_pos) = paren_pos
                && let Some(close_pos) = self.find_close_paren(paren_pos)
            {
                let mut parts = vec![d.text("(")];
                for comment in comments_in_range(self.comments, paren_pos + 1, close_pos) {
                    parts.push(self.build_comment_doc(comment));
                }
                parts.push(d.text(")"));
                return d.concat(&parts);
            }
            return d.text("()");
        }

        // Check for line comments or own-line block comments that force multiline
        let close_paren_pos = paren_pos.and_then(|p| self.find_close_paren(p));
        let end_boundary =
            close_paren_pos.unwrap_or_else(|| params.last().map_or(0, |p| p.span().end));
        let has_forcing_comments = self.has_line_comments_in_delimited_list(
            params,
            internal::Expression::span,
            end_boundary,
        ) || params.last().is_some_and(|last| {
            comments_in_range(self.comments, last.span().end, end_boundary)
                .any(|c| c.is_block && !self.is_same_line(last.span().end, c.span.start))
        });

        if has_forcing_comments {
            // Multiline path with hardlines (same as build_function_params_doc_with_line_comments)
            let mut inner_parts = Vec::new();
            let open_paren = paren_pos.unwrap_or(0);
            let mut prev_end = open_paren + 1;

            for (i, p) in params.iter().enumerate() {
                let param_start = p.span().start;
                let param_end = p.span().end;
                let is_last = i == params.len() - 1;
                let is_rest = matches!(p, internal::Expression::RestElement(_));

                inner_parts.extend(self.build_leading_comments_multiline(prev_end, param_start));
                inner_parts.push(self.build_function_type_param_expression_doc(p));

                if !is_last {
                    let next_start = params[i + 1].span().start;
                    prev_end = self.emit_multiline_comma_with_comments(
                        &mut inner_parts,
                        param_end,
                        next_start,
                    );
                } else {
                    let close = close_paren_pos.unwrap_or(param_end);
                    if !is_rest {
                        inner_parts.push(d.text(","));
                    }
                    inner_parts.extend(self.build_trailing_comments_multiline(param_end, close));
                }
            }

            let mut parts = vec![d.text("(")];
            parts.push(d.indent(d.concat(&[d.hardline(), d.concat(&inner_parts)])));
            parts.push(d.hardline());
            parts.push(d.text(")"));
            return d.group(d.concat(&parts));
        }

        // Build params with width-based breaking
        let mut param_parts = Vec::new();

        // Handle comments before first param (e.g., `(/* comment */ a: T)`)
        if let Some(paren_pos) = paren_pos {
            let first_param_start = params[0].span().start;
            for comment in comments_in_range(self.comments, paren_pos + 1, first_param_start) {
                param_parts.push(self.build_comment_doc(comment));
                param_parts.push(d.text(" "));
            }
        }

        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                param_parts.push(d.text(","));
                param_parts.push(d.line());
            }
            param_parts.push(self.build_function_type_param_expression_doc(param));

            // Handle trailing comments after this param
            let param_end = param.span().end;
            let next_boundary = if i + 1 < params.len() {
                params[i + 1].span().start
            } else {
                close_paren_pos.unwrap_or(param_end)
            };

            for comment in comments_in_range(self.comments, param_end, next_boundary) {
                param_parts.push(d.text(" "));
                param_parts.push(self.build_comment_doc(comment));
            }
        }

        // Check if last param is rest element (no trailing comma)
        let last_is_rest = params
            .last()
            .is_some_and(|p| matches!(p, internal::Expression::RestElement(_)));

        let mut parts = vec![d.text("(")];
        parts.push(d.indent(d.concat(&[d.softline(), d.concat(&param_parts)])));
        if !last_is_rest {
            parts.push(d.trailing_comma());
        }
        parts.push(d.softline());
        parts.push(d.text(")"));

        // Wrap in group so params break independently of outer context
        d.group(d.concat(&parts))
    }

    /// Build a Doc for a function type parameter expression with wrapping type annotations.
    ///
    /// For Identifiers, uses wrapping type annotations so generic type arguments
    /// break at print width (e.g., `param: Map<LongA, LongB>` breaks inside `<>`).
    pub(super) fn build_function_type_param_expression_doc(
        &self,
        expr: &internal::Expression,
    ) -> DocId {
        let d = self.d();
        match expr {
            internal::Expression::Identifier(id) => {
                self.build_identifier_doc_with_wrapping_type(id)
            }
            internal::Expression::RestElement(rest) => {
                // Comments between `...` and the argument (e.g., `.../* c */ args`)
                let dots_end = rest.span.start + 3; // "...".len()
                let arg_start = rest.argument.span().start;
                let comments_doc =
                    self.build_comments_between(dots_end, arg_start, CommentSpacing::Trailing);
                let mut parts = vec![
                    d.text("..."),
                    comments_doc,
                    self.build_function_type_param_expression_doc(&rest.argument),
                ];
                if let Some(ta) = &rest.type_annotation {
                    parts.push(self.build_type_annotation_doc(ta));
                }
                d.concat(&parts)
            }
            _ => self.build_expression_doc(expr),
        }
    }

    /// Build parameter list docs for function/constructor types
    /// Returns docs that should be pushed to a parts vector
    fn build_function_params_doc(
        &self,
        params: &[internal::Expression],
        paren_search_start: u32,
    ) -> Vec<DocId> {
        let d = self.d();
        let mut parts = Vec::new();

        // Find paren position for comment handling (skip comments to avoid matching `(` inside them)
        let paren_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            paren_search_start as usize,
            self.source.len(),
            b'(',
        )
        .map(|p| p as u32);

        if params.is_empty() {
            parts.push(d.text("()"));
        } else {
            // Check for line comments or own-line block comments between/after params (force multiline)
            let close_paren_pos = paren_pos.and_then(|p| self.find_close_paren(p));
            // Use last param end as fallback if close paren not found (no trailing check)
            let end_boundary =
                close_paren_pos.unwrap_or_else(|| params.last().map_or(0, |p| p.span().end));
            let has_line_comments = self.has_line_comments_in_delimited_list(
                params,
                internal::Expression::span,
                end_boundary,
            );
            // Also check for own-line block comments after the last param
            let has_own_line_block_after_last = params.last().is_some_and(|last| {
                comments_in_range(self.comments, last.span().end, end_boundary)
                    .any(|c| c.is_block && !self.is_same_line(last.span().end, c.span.start))
            });
            if has_line_comments || has_own_line_block_after_last {
                return self.build_function_params_doc_with_line_comments(params, paren_pos);
            }

            // Check for huggable single param: (options: { ... })
            // Prettier's shouldHugFunctionParameters: single param with object type annotation
            // gets hugged - no breaks added around it, the TypeLiteral handles its own expansion.
            // This keeps `(options: {` together, letting the object's content break:
            //   fn: (options: {
            //       repo: LocalRepo;
            //       log: Logger;
            //   }) => ReturnType
            // NOT:
            //   fn: (
            //       options: { repo: LocalRepo; log: Logger },
            //   ) => ReturnType
            let no_leading_comments = paren_pos
                .is_none_or(|pos| !self.has_comments_between(pos + 1, params[0].span().start));
            let no_trailing_comments = close_paren_pos
                .is_none_or(|cp| !self.has_comments_between(params[0].span().end, cp));
            let huggable_param = if params.len() == 1 && no_leading_comments && no_trailing_comments
            {
                get_type_literal_from_identifier(&params[0])
            } else {
                None
            };

            if let Some((id, type_ann, type_literal)) = huggable_param {
                // Hug mode: build identifier with TypeLiteral that doesn't have its own group.
                // This way the TypeLiteral's softlines are part of the function type group,
                // and when the function type group breaks (because line is too long),
                // those softlines become newlines, breaking the param's object type.
                //
                // Key insight: fits_with_lookahead evaluates if_break in Flat mode, which
                // can cause off-by-one errors with trailing semicolons. By removing the
                // TypeLiteral's group wrapper, its softlines directly contribute to the
                // function type group's breaking decision.
                parts.push(d.text("("));
                // Build identifier name + optional marker
                parts.push(d.symbol(id.name.to_u32()));
                if id.optional {
                    parts.push(d.text("?"));
                }
                // Build type annotation with TypeLiteral that has softlines but no group wrapper
                // Extract comments between `:` and the TypeLiteral (e.g., `x: /* c */ { a: T }`)
                let colon_end = type_ann.span.start + 1;
                let type_start = type_ann.type_annotation.span().start;
                parts.push(d.text(": "));
                parts.push(self.build_comments_between(
                    colon_end,
                    type_start,
                    CommentSpacing::Trailing,
                ));
                parts.push(self.build_type_literal_doc_for_function_param(type_literal));

                // Handle trailing comments after the param (between type literal and close paren)
                let param_end = params[0].span().end;
                let close_paren = paren_pos
                    .and_then(|p| self.find_close_paren(p))
                    .unwrap_or(param_end);
                for comment in comments_in_range(self.comments, param_end, close_paren) {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }

                parts.push(d.text(")"));
            } else {
                let mut param_parts = Vec::new();
                let mut prev_end = paren_pos.map_or(0, |p| p + 1); // After `(`
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        param_parts.push(d.text(","));
                        param_parts.push(d.line());
                    }

                    // Leading block comments (after previous comma or `(`)
                    param_parts.push(self.build_inline_comments_between_doc_trailing_space(
                        prev_end,
                        p.span().start,
                    ));

                    param_parts.push(self.build_function_type_param_expression_doc(p));

                    // Trailing block comments (before comma or `)`)
                    let param_end = p.span().end;
                    if i + 1 < params.len() {
                        let next_start = params[i + 1].span().start;
                        let comma_pos = self.find_list_comma(param_end, next_start);
                        self.append_trailing_inline_block_comments(
                            &mut param_parts,
                            param_end,
                            comma_pos,
                        );
                        prev_end = comma_pos + 1; // After comma
                    } else {
                        // Last param: trailing comments before `)`
                        let close_paren = paren_pos
                            .and_then(|p| self.find_close_paren(p))
                            .unwrap_or(param_end);
                        self.append_trailing_inline_block_comments(
                            &mut param_parts,
                            param_end,
                            close_paren,
                        );
                    }
                }
                parts.push(d.text("("));
                parts.push(d.indent(d.concat(&[d.softline(), d.concat(&param_parts)])));
                // Trailing comma when breaking, UNLESS last param is a rest element
                let last_is_rest = params
                    .last()
                    .is_some_and(|p| matches!(p, internal::Expression::RestElement(_)));
                if !last_is_rest {
                    parts.push(d.trailing_comma());
                }
                parts.push(d.softline());
                parts.push(d.text(")"));
            }
        }
        parts
    }

    /// Build function params with line comments between them (forces multiline)
    fn build_function_params_doc_with_line_comments(
        &self,
        params: &[internal::Expression],
        paren_pos: Option<u32>,
    ) -> Vec<DocId> {
        let d = self.d();
        let mut parts = Vec::new();
        let mut inner_parts = Vec::new();

        let open_paren = paren_pos.unwrap_or(0);
        let mut prev_end = open_paren + 1; // After `(`

        for (i, p) in params.iter().enumerate() {
            let param_start = p.span().start;
            let param_end = p.span().end;
            let is_last = i == params.len() - 1;

            // Leading comments (after previous comma or `(`)
            inner_parts.extend(self.build_leading_comments_multiline(prev_end, param_start));

            inner_parts.push(self.build_function_type_param_expression_doc(p));

            let is_rest = matches!(p, internal::Expression::RestElement(_));

            if !is_last {
                let next_start = params[i + 1].span().start;
                prev_end = self.emit_multiline_comma_with_comments(
                    &mut inner_parts,
                    param_end,
                    next_start,
                );
            } else {
                // Last param: trailing comma (unless rest) + comments before `)`
                let close_paren = paren_pos
                    .and_then(|p| self.find_close_paren(p))
                    .unwrap_or(param_end);
                if !is_rest {
                    inner_parts.push(d.text(","));
                }
                inner_parts.extend(self.build_trailing_comments_multiline(param_end, close_paren));
                prev_end = close_paren;
            }
        }

        parts.push(d.text("("));
        parts.push(d.indent(d.concat(&[d.hardline(), d.concat(&inner_parts)])));
        parts.push(d.hardline());
        parts.push(d.text(")"));
        parts
    }
}
