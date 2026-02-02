// Function type printing for TypeScript
//
// Handles:
// - Function types: `(a: T) => U`
// - Constructor types: `new () => T`
// - Signature parameters (shared with type members)
// - Return type annotations

use super::super::comments_in_range;
use super::Printer;
use super::helpers::type_args_should_wrap_for_return_type;
use crate::ast::internal::{self, TSConstructorType, TSFunctionType, TSType};
use tsv_lang::SymbolToU32;
use tsv_lang::doc::{self, Doc};

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
) -> Option<(&internal::Identifier, &internal::TSTypeLiteral)> {
    match expr {
        internal::Expression::Identifier(id) => {
            id.type_annotation
                .as_ref()
                .and_then(|ann| match ann.type_annotation.as_ref() {
                    TSType::TypeLiteral(t) => Some((id, t)),
                    _ => None,
                })
        }
        _ => None,
    }
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
    fn build_function_type_return_doc(&self, return_type: &internal::TSTypeAnnotation) -> Doc {
        match return_type.type_annotation.as_ref() {
            TSType::Union(u) => {
                let type_doc = self.build_union_type_doc(u, false);
                doc::concat(vec![
                    doc::text(" =>"),
                    doc::group(doc::indent_line(type_doc)),
                ])
            }
            TSType::Intersection(i) => {
                // Intersections use trailing `&` - first type NOT indented, continuations indented
                // The intersection doc handles this internally, we just need proper grouping
                let type_doc = self.build_intersection_type_doc(i, false);
                doc::concat(vec![doc::text(" => "), doc::group(doc::indent(type_doc))])
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
                doc::concat(vec![doc::text(" => "), type_doc])
            }
            _ => doc::concat(vec![
                doc::text(" => "),
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
    pub(super) fn build_function_type_doc(&self, f: &TSFunctionType) -> Doc {
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
    pub(super) fn build_constructor_type_doc(&self, c: &TSConstructorType) -> Doc {
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
    pub(in crate::printer) fn build_signature_params_doc(
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
    pub(super) fn build_function_type_param_expression_doc(
        &self,
        expr: &internal::Expression,
    ) -> Doc {
        match expr {
            internal::Expression::Identifier(id) => {
                self.build_identifier_doc_with_wrapping_type(id)
            }
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
            let end_boundary =
                close_paren_pos.unwrap_or_else(|| params.last().map_or(0, |p| p.span().end));
            if self.has_line_comments_in_delimited_list(
                params,
                internal::Expression::span,
                end_boundary,
            ) {
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
            let huggable_param = if params.len() == 1 && no_leading_comments {
                get_type_literal_from_identifier(&params[0])
            } else {
                None
            };

            if let Some((id, type_literal)) = huggable_param {
                // Hug mode: build identifier with TypeLiteral that doesn't have its own group.
                // This way the TypeLiteral's softlines are part of the function type group,
                // and when the function type group breaks (because line is too long),
                // those softlines become newlines, breaking the param's object type.
                //
                // Key insight: fits_with_lookahead evaluates if_break in Flat mode, which
                // can cause off-by-one errors with trailing semicolons. By removing the
                // TypeLiteral's group wrapper, its softlines directly contribute to the
                // function type group's breaking decision.
                parts.push(doc::text("("));
                // Build identifier name + optional marker
                parts.push(doc::symbol(id.name.to_u32()));
                if id.optional {
                    parts.push(doc::text("?"));
                }
                // Build type annotation with TypeLiteral that has softlines but no group wrapper
                parts.push(doc::text(": "));
                parts.push(self.build_type_literal_doc_for_function_param(type_literal));

                // Handle trailing comments after the param (between type literal and close paren)
                let param_end = params[0].span().end;
                let close_paren = paren_pos
                    .and_then(|p| self.find_close_paren(p))
                    .unwrap_or(param_end);
                for comment in comments_in_range(self.comments, param_end, close_paren) {
                    parts.push(doc::text(" "));
                    parts.push(self.build_comment_doc(comment));
                }

                parts.push(doc::text(")"));
            } else {
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
}
