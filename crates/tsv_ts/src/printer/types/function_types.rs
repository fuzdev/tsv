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
use crate::printer::layout::hang_after_operator;
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::source_scan::find_char_skipping_comments;

/// Check if an expression is an identifier with a TypeLiteral type annotation.
///
/// Used for function param hugging: `fn: (options: { a: T }) => U`
/// - The opening `{` stays on the same line as the parameter name
/// - The content expands internally
/// - The closing `}` comes on its own line when broken
///
/// Note: Only TypeLiteral is handled specially. Mapped types (`{ [K in T]: V }`)
/// also pass `is_huggable_type` but use standard param formatting.
fn get_type_literal_from_identifier<'a>(
    expr: &'a internal::Expression<'a>,
) -> Option<(
    &'a internal::Identifier<'a>,
    &'a internal::TSTypeAnnotation<'a>,
    &'a internal::TSTypeLiteral<'a>,
)> {
    match expr {
        internal::Expression::Identifier(id) => {
            id.type_annotation()
                .and_then(|ann| match ann.type_annotation {
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
    type_parameters: Option<&internal::TSTypeParameterDeclaration<'_>>,
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
    return_type: &internal::TSTypeAnnotation<'_>,
    return_type_doc: DocId,
    d: &DocArena,
) -> bool {
    matches!(
        return_type.type_annotation,
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
    /// Build the ` => ReturnType` tail. `leading_space` controls the space before
    /// `=>`: normally `true` (` => T`), but `false` when a line comment in the
    /// `)`→`=>` gap has forced a hardline before `=>` (the caller emits the comment
    /// + hardline, so `=>` starts the next line flush — `() // c\n=> void`).
    fn build_function_type_return_doc(
        &self,
        return_type: &internal::TSTypeAnnotation<'_>,
        leading_space: bool,
    ) -> DocId {
        let d = self.d();
        // `=>` with the optional leading space, as static text — the leading-space
        // flag selects among four fixed strings, so no per-call String alloc.
        let arrow = if leading_space { " =>" } else { "=>" };
        let arrow_sp = if leading_space { " => " } else { "=> " };
        // Comments between `=>` and the return type (e.g., `() => /* c */ string`)
        // For function types, the annotation span starts at `=` in `=>`
        let arrow_end = return_type.span.start + "=>".len() as u32;
        let type_start = return_type.type_annotation.span().start;
        // Use break-for-line variant: line comments must force a hardline before
        // the return type so they don't swallow it (`=> // c\nT`, not `=> // c T`).
        let comments_doc = self.build_trailing_comments_break_for_line(arrow_end, type_start);
        // Strip redundant comment-free parens so `($A | $B)` / `($A & $B)` return
        // types get the same hanging layout as the bare form (prettier strips them
        // too). Only union/intersection are unwrapped; other parenthesized types
        // keep the match-on-original fall-through below.
        let value_type = self.unwrap_redundant_parens(return_type.type_annotation);
        if let TSType::Union(u) = value_type {
            let type_doc = self.build_union_type_doc(u);
            return d.concat(&[
                d.text(arrow),
                hang_after_operator(d, d.concat(&[comments_doc, type_doc])),
            ]);
        }
        if let TSType::Intersection(i) = value_type {
            // Intersections use trailing `&` - first type NOT indented, continuations indented
            let type_doc = self.build_intersection_type_doc(i, false);
            return d.concat(&[d.text(arrow_sp), comments_doc, d.group(d.indent(type_doc))]);
        }
        match return_type.type_annotation {
            // TypeReference with complex type args (like Promise<Result<...>>):
            // Build with wrapping type args so it can break inside the <...>
            TSType::TypeReference(r)
                if r.type_arguments
                    .as_ref()
                    .is_some_and(type_args_should_wrap_for_return_type) =>
            {
                // Use build_type_doc_inner with wrap_type_args=true to enable
                // wrapping inside the type reference's type arguments
                let type_doc = self.build_type_doc_inner(return_type.type_annotation, true);
                d.concat(&[d.text(arrow_sp), comments_doc, type_doc])
            }
            _ => d.concat(&[
                d.text(arrow_sp),
                comments_doc,
                self.build_type_doc(return_type.type_annotation),
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
    pub(super) fn build_function_type_doc(&self, f: &TSFunctionType<'_>) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        self.append_type_params_and_signature(
            &mut parts,
            f.type_parameters.as_ref(),
            f.params,
            &f.return_type,
            f.span.start,
        );
        d.group(d.concat(&parts))
    }

    /// Append the shared tail of a function/constructor type to `parts`: the type
    /// parameters, any comments between them and `(`, the parameter list, and the
    /// ` => ReturnType`. `span_start` locates the `(` when there are no type params.
    fn append_type_params_and_signature(
        &self,
        parts: &mut DocBuf,
        type_parameters: Option<&internal::TSTypeParameterDeclaration<'_>>,
        params: &[internal::Expression<'_>],
        return_type: &internal::TSTypeAnnotation<'_>,
        span_start: u32,
    ) {
        if let Some(type_params) = type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        let paren_search_start = type_parameters.map_or(span_start, |tp| tp.span.end);

        // Comments between type_params and `(` go after type_params
        if let Some(tp) = type_parameters
            && let Some(pp) = find_char_skipping_comments(
                self.source.as_bytes(),
                tp.span.end as usize,
                self.source.len(),
                b'(',
            )
        {
            self.append_type_params_to_paren_comments(parts, tp.span.end, pp as u32);
        }

        parts.extend(self.build_grouped_params_and_return_type(
            params,
            paren_search_start,
            return_type,
            type_parameters,
        ));
    }

    /// Build a Doc for a constructor type: `new () => T` or `abstract new <T>() => T`
    pub(super) fn build_constructor_type_doc(&self, c: &TSConstructorType<'_>) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();

        if c.abstract_ {
            // Preserve a comment in the `abstract`→`new` keyword gap
            // (`abstract /* c */ new`). Prettier relocates it after `new`; per
            // Comment Position Philosophy we keep it in place (block inline, line
            // comment floated via `line_suffix` — same treatment as the `new`→`(`
            // gap below). Without this it was dropped (content loss).
            let abstract_end = self
                .find_keyword_in_range(c.span.start, c.return_type.span.start, "abstract")
                .map_or(c.span.start, |p| p + "abstract".len() as u32);
            let new_start = self
                .find_keyword_in_range(abstract_end, c.return_type.span.start, "new")
                .unwrap_or(abstract_end);
            parts.push(d.text("abstract"));
            self.append_type_params_to_paren_comments(&mut parts, abstract_end, new_start);
            parts.push(d.text(" "));
        }

        // Comments between `new` and the type params / `(` (e.g. `new /* c */ ()`).
        // Prettier relocates these (after `)`, before the first param, or — with
        // type params — keeps them in place); per Comment Position Philosophy we
        // preserve the user's position after `new`. Without this they were dropped.
        parts.push(d.text("new"));
        let new_end = self
            .find_keyword_in_range(c.span.start, c.return_type.span.start, "new")
            .map_or(c.span.start, |p| p + "new".len() as u32);
        let next_token_start = c
            .type_parameters
            .as_ref()
            .map(|tp| tp.span.start)
            .or_else(|| {
                find_char_skipping_comments(
                    self.source.as_bytes(),
                    new_end as usize,
                    self.source.len(),
                    b'(',
                )
                .map(|p| p as u32)
            });
        if let Some(next_start) = next_token_start {
            self.append_type_params_to_paren_comments(&mut parts, new_end, next_start);
        }
        parts.push(d.text(" "));

        self.append_type_params_and_signature(
            &mut parts,
            c.type_parameters.as_ref(),
            c.params,
            &c.return_type,
            c.span.start,
        );

        d.group(d.concat(&parts))
    }

    /// Build params + return type docs with optional parameter grouping.
    ///
    /// Implements Prettier's `shouldGroupFunctionParameters` for function/constructor
    /// types: when there's 1 param and the return type is an object type or will break,
    /// wraps params in their own group so they stay flat when the outer group breaks.
    fn build_grouped_params_and_return_type(
        &self,
        params: &[internal::Expression<'_>],
        paren_search_start: u32,
        return_type: &internal::TSTypeAnnotation<'_>,
        type_parameters: Option<&internal::TSTypeParameterDeclaration<'_>>,
    ) -> [DocId; 2] {
        let d = self.d();

        // Comments between the close paren and `=>` (e.g. `() /* c */ => void`).
        // Without this they are dropped — the params doc ends at `)` and the
        // return doc begins at `=>`, so nothing else covers the gap.
        let arrow_start = return_type.span.start;
        let after_close = find_char_skipping_comments(
            self.source.as_bytes(),
            paren_search_start as usize,
            self.source.len(),
            b'(',
        )
        .and_then(|open| self.find_closing_paren(open as u32, arrow_start))
        .filter(|&after_close| self.has_comments_between(after_close, arrow_start));

        // A line comment in the `)`→`=>` gap can't stay inline — it would swallow
        // `=> void` (`() // c => void`). Keep it trailing `)` and force `=>` onto
        // the next line flush (`() // c\n=> void`), matching prettier. A block
        // comment stays inline (`() /* c */ => void`).
        let pre_arrow_line_close =
            after_close.filter(|&ac| self.has_line_comments_between(ac, arrow_start));
        let return_type_doc =
            self.build_function_type_return_doc(return_type, pre_arrow_line_close.is_none());
        let return_type_doc = if let Some(ac) = pre_arrow_line_close {
            let pre = self.build_trailing_comments_break_for_line(ac, arrow_start);
            d.concat(&[d.text(" "), pre, return_type_doc])
        } else {
            let pre_arrow_doc = after_close.map_or_else(
                || d.empty(),
                |ac| self.build_comments_between(ac, arrow_start, CommentSpacing::Leading),
            );
            d.concat(&[pre_arrow_doc, return_type_doc])
        };

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

    /// Build the doc for an empty parameter list, preserving any dangling block
    /// comments inside the parens (`(/* c */)`). Returns `()` when there are none.
    ///
    /// Shared by function/constructor types (`build_function_params_doc`) and
    /// the type-member signatures (`build_signature_params_doc`) — without this
    /// the dangling comment is dropped (content loss).
    fn build_empty_params_doc(&self, paren_pos: Option<u32>) -> DocId {
        let d = self.d();
        if let Some(paren_pos) = paren_pos
            && let Some(close_pos) = self.matching_close_paren(paren_pos)
            && self.has_comments_between(paren_pos + 1, close_pos)
        {
            // A line comment can't stay inline inside `()` — it would swallow the
            // `)`. With no parameter to lead, prettier 3.9 (#18623) drops the
            // comment to its own indented line and breaks the `()`; tsv matches.
            // (Contrast a *non-empty* list, where a line comment trailing `(`
            // stays on the `(` line via the open-delimiter divergence
            // `delimiter_line_comment_prefix` — that path doesn't reach here.)
            // Block comments stay inline (`(/* c */)`), matching prettier.
            if self.has_line_comments_between(paren_pos + 1, close_pos) {
                let mut inner = DocBuf::new();
                for comment in comments_in_range(self.comments, paren_pos + 1, close_pos) {
                    inner.push(d.hardline());
                    inner.push(self.build_comment_doc(comment));
                }
                let mut parts: DocBuf = smallvec![d.text("(")];
                parts.push(d.indent(d.concat(&inner)));
                parts.push(d.hardline());
                parts.push(d.text(")"));
                return d.concat(&parts);
            }
            let mut parts: DocBuf = smallvec![d.text("(")];
            for comment in comments_in_range(self.comments, paren_pos + 1, close_pos) {
                parts.push(self.build_comment_doc(comment));
            }
            parts.push(d.text(")"));
            d.concat(&parts)
        } else {
            d.text("()")
        }
    }

    /// Emit any block comments in the `)`→return-type gap, with a trailing space.
    ///
    /// Prettier adds a space before `:` when a comment precedes it
    /// (`m(a) /* c */ : void`), so the caller appends the `: type` after this prefix.
    /// Returns an empty doc when there is no such comment.
    pub(in crate::printer) fn build_paren_to_return_type_comments(
        &self,
        paren_pos: Option<u32>,
        return_type_start: u32,
    ) -> DocId {
        // Depth-tracked close paren (skips nested parens / comments) — the naive
        // first-`)` scan mis-fires on complex params and pulls real param-trailing
        // comments into this range (duplication).
        let close_paren_after =
            paren_pos.and_then(|p| self.find_closing_paren(p, return_type_start));
        self.build_close_paren_to_return_type_comments(close_paren_after, return_type_start)
    }

    /// `build_paren_to_return_type_comments` for callers that already located the
    /// params' close paren (`close_paren_after` = position just past the `)`) —
    /// reuses that scan instead of re-running it.
    pub(in crate::printer) fn build_close_paren_to_return_type_comments(
        &self,
        close_paren_after: Option<u32>,
        return_type_start: u32,
    ) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = smallvec![];
        if let Some(close_after) = close_paren_after {
            for comment in comments_in_range(self.comments, close_after, return_type_start) {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
            }
        }
        // Prettier adds space before `:` when there's a preceding comment
        if !parts.is_empty() {
            parts.push(d.text(" "));
        }
        d.concat(&parts)
    }

    /// Build return type annotation with comment handling between `)` and `:`
    /// Used by MethodSignature, CallSignature, ConstructSignature (type-literal and
    /// interface members) and the declare-function signature.
    pub(in crate::printer) fn build_signature_return_type_doc(
        &self,
        paren_pos: Option<u32>,
        return_type: &internal::TSTypeAnnotation<'_>,
    ) -> DocId {
        let d = self.d();
        let prefix = self.build_paren_to_return_type_comments(paren_pos, return_type.span.start);
        d.concat(&[
            prefix,
            self.build_type_annotation_doc_for_return_type(return_type),
        ])
    }

    /// Wrap the parameter list and return-type annotation of a type-member
    /// signature (`MethodSignature` / `CallSignature` / `ConstructSignature`) or a
    /// bodyless function signature (overload / `declare`) in one **signature
    /// group**, so a too-long signature breaks the PARAMS before the return-type
    /// generic breaks — params-break-priority, matching `build_callable_signature_doc`
    /// for class/function signatures and prettier.
    ///
    /// Three pieces cooperate: `build_signature_params_doc` leaves the params
    /// **ungrouped** (softlines this group controls); `build_signature_return_type_doc`
    /// uses the return-type type variant (which keeps a union / multi-arg generic
    /// inline until the params have broken); and prettier's
    /// `shouldGroupFunctionParameters` (1 param + `type_params_allow_grouping` +
    /// `return_type_triggers_grouping` — an object/mapped return, or one whose doc
    /// will-breaks) re-wraps the params in their OWN group so they HUG (stay flat)
    /// while the return type breaks — e.g. `create_context<T>(fallback: () => T): {⏎…⏎}`.
    /// The signature group is scoped to just params+return — NOT the member key or
    /// its comments — so a comment-forced hardline elsewhere in the member (e.g.
    /// `new // c⏎(a): A`) doesn't drag the params open.
    pub(in crate::printer) fn build_signature_params_return_group(
        &self,
        params: &[internal::Expression<'_>],
        type_parameters: Option<&internal::TSTypeParameterDeclaration<'_>>,
        return_type: Option<&internal::TSTypeAnnotation<'_>>,
        paren_pos: Option<u32>,
    ) -> DocId {
        let d = self.d();
        let params_doc = self.build_signature_params_doc(params, paren_pos);
        let return_type_doc =
            return_type.map(|rt| self.build_signature_return_type_doc(paren_pos, rt));

        // shouldGroupFunctionParameters: a single param whose return type is an
        // object/mapped type (or otherwise will-breaks) hugs — the params stay flat
        // and the return type breaks, instead of the params breaking.
        let params_doc = if params.len() == 1
            && type_params_allow_grouping(type_parameters)
            && return_type
                .zip(return_type_doc)
                .is_some_and(|(rt, rt_doc)| return_type_triggers_grouping(rt, rt_doc, d))
        {
            d.group(params_doc)
        } else {
            params_doc
        };

        let mut sig_parts: DocBuf = smallvec![params_doc];
        if let Some(rt_doc) = return_type_doc {
            sig_parts.push(rt_doc);
        }
        d.group(d.concat(&sig_parts))
    }

    /// Build a function-declaration return type (`: T`) with `)`→`:` comment
    /// handling, using the return-type type variant (wraps unions/intersections so
    /// params break first). Sibling of `build_signature_return_type_doc`, which
    /// serves type-member signatures and uses the plain type variant. The caller
    /// supplies the already-located close paren (position just past the `)`).
    pub(in crate::printer) fn build_function_return_type_doc(
        &self,
        close_paren_after: Option<u32>,
        return_type: &internal::TSTypeAnnotation<'_>,
    ) -> DocId {
        let d = self.d();
        let prefix = self
            .build_close_paren_to_return_type_comments(close_paren_after, return_type.span.start);
        d.concat(&[
            prefix,
            self.build_type_annotation_doc_for_return_type(return_type),
        ])
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
        params: &[internal::Expression<'_>],
        paren_pos: Option<u32>,
    ) -> DocId {
        let d = self.d();
        if params.is_empty() {
            // Handle comments inside empty params (e.g., `a(/* comment */): void`)
            return self.build_empty_params_doc(paren_pos);
        }

        // Check for line comments or own-line block comments that force multiline
        let close_paren_pos = paren_pos.and_then(|p| self.matching_close_paren(p));
        let end_boundary =
            close_paren_pos.unwrap_or_else(|| params.last().map_or(0, |p| p.span().end));
        // A line comment trailing `(` (`(// c\n p`), or an own-line block comment
        // in the `(`→first-param gap, forces multiline. Without this it falls to
        // the inline path below, where a line comment swallows the following tokens
        // (`(// c p: T)`). Mirrors `build_function_params_doc`'s leading-gap check.
        let has_leading_gap_forcing = paren_pos.is_some_and(|p| {
            let first_start = params[0].span().start;
            self.has_line_comments_between(p + 1, first_start)
                || self.has_own_line_block_comment_after(p, p + 1, first_start)
        });
        // A blank line the author left between two params also forces multiline (and
        // is preserved by the separator emission below) — same as regular function
        // params; prettier keeps the blank in every parameter-list position.
        let force_multiline = self.has_line_comments_in_delimited_list(
            params,
            internal::Expression::span,
            end_boundary,
        ) || has_leading_gap_forcing
            || self.has_blank_line_between_params(params)
            || params.last().is_some_and(|last| {
                self.has_own_line_block_comment_after(
                    last.span().end,
                    last.span().end,
                    end_boundary,
                )
            });

        if force_multiline {
            // Multiline path with hardlines (same as build_function_params_doc_with_line_comments)
            let mut inner_parts = DocBuf::new();
            let open_paren = paren_pos.unwrap_or(0);
            let mut prev_end = open_paren + 1;

            // A line comment trailing `(` is kept on the `(` line. For a method
            // signature prettier also keeps it there (match); for call/construct
            // signatures prettier relocates it to its own line (divergence). tsv
            // applies the open-delimiter rule uniformly. Same mechanism as the
            // function/constructor-type params path.
            let (paren_prefix, paren_pull_pos) = paren_pos.map_or_else(
                || (DocBuf::new(), None),
                |open| self.delimiter_line_comment_prefix(open, params[0].span().start),
            );

            for (i, p) in params.iter().enumerate() {
                let param_start = p.span().start;
                let param_end = p.span().end;
                let is_last = i == params.len() - 1;

                let skip_delim = if i == 0 { paren_pull_pos } else { None };
                inner_parts.extend(self.build_leading_comments_multiline_opt(
                    prev_end,
                    param_start,
                    skip_delim,
                ));
                inner_parts.push(self.build_function_type_param_expression_doc(p));

                if !is_last {
                    let next_start = params[i + 1].span().start;
                    prev_end = self.emit_multiline_comma_with_comments(
                        &mut inner_parts,
                        param_end,
                        next_start,
                        true,
                    );
                } else {
                    let close = close_paren_pos.unwrap_or(param_end);
                    // No trailing comma after the last param (trailingComma: 'none').
                    inner_parts.extend(self.build_trailing_comments_multiline(param_end, close));
                }
            }

            let mut parts: DocBuf = smallvec![d.text("(")];
            parts.push(d.concat(&paren_prefix));
            parts.push(d.indent(d.concat(&[d.hardline(), d.concat(&inner_parts)])));
            parts.push(d.hardline());
            parts.push(d.text(")"));
            return d.group(d.concat(&parts));
        }

        // Build params with width-based breaking
        let mut param_parts = DocBuf::new();

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

        let mut parts: DocBuf = smallvec![d.text("(")];
        parts.push(d.indent(d.concat(&[d.softline(), d.concat(&param_parts)])));
        // No trailing comma on the last param (trailingComma: 'none').
        parts.push(d.softline());
        parts.push(d.text(")"));

        // No group — the outer signature group (build_method_signature_member_doc /
        // build_call_or_construct_signature_doc) controls these softlines, so a too-long
        // signature breaks the PARAMS before the return-type generic breaks (matching
        // build_params_doc_with_comments for class/function signatures, and prettier).
        d.concat(&parts)
    }

    /// Build a Doc for a function type parameter expression with wrapping type annotations.
    ///
    /// For Identifiers, uses wrapping type annotations so generic type arguments
    /// break at print width (e.g., `param: Map<LongA, LongB>` breaks inside `<>`).
    pub(super) fn build_function_type_param_expression_doc(
        &self,
        expr: &internal::Expression<'_>,
    ) -> DocId {
        let d = self.d();
        match expr {
            internal::Expression::Identifier(id) => {
                self.build_identifier_doc_with_wrapping_type(id)
            }
            internal::Expression::RestElement(rest) => {
                // Comments between `...` and the argument (e.g., `.../* c */ args`); a
                // line comment breaks so it can't swallow the rest parameter.
                let dots_end = rest.span.start + "...".len() as u32;
                let arg_start = rest.argument.span().start;
                let comments_doc = self.build_trailing_comments_break_for_line(dots_end, arg_start);
                let mut parts: DocBuf = smallvec![
                    d.text("..."),
                    comments_doc,
                    self.build_function_type_param_expression_doc(rest.argument),
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
        params: &[internal::Expression<'_>],
        paren_search_start: u32,
    ) -> DocBuf {
        let d = self.d();
        let mut parts = DocBuf::new();

        // Find paren position for comment handling (skip comments to avoid matching `(` inside them)
        let paren_pos = find_char_skipping_comments(
            self.source.as_bytes(),
            paren_search_start as usize,
            self.source.len(),
            b'(',
        )
        .map(|p| p as u32);

        if params.is_empty() {
            parts.push(self.build_empty_params_doc(paren_pos));
        } else {
            // Check for line comments or own-line block comments between/after params (force multiline)
            let close_paren_pos = paren_pos.and_then(|p| self.matching_close_paren(p));
            // Use last param end as fallback if close paren not found (no trailing check)
            let end_boundary =
                close_paren_pos.unwrap_or_else(|| params.last().map_or(0, |p| p.span().end));

            // Zero-comment fast gate (see `build_params_doc_with_comments`): every
            // comment sub-query below is bounded within `[paren, end_boundary]`
            // (with no located paren, the leading queries anchor at 0, so the
            // window widens to stay a superset), so when no comment lies there
            // each is provably empty/false.
            let window_has_comments = {
                let window_start = paren_pos.unwrap_or(0);
                self.has_comments_between(window_start, end_boundary)
            };

            let has_line_comments = window_has_comments
                && self.has_line_comments_in_delimited_list(
                    params,
                    internal::Expression::span,
                    end_boundary,
                );
            // Also check for own-line block comments after the last param
            let has_own_line_block_after_last = window_has_comments
                && params.last().is_some_and(|last| {
                    self.has_own_line_block_comment_after(
                        last.span().end,
                        last.span().end,
                        end_boundary,
                    )
                });
            // A line comment trailing `(` (`(// c\n p`), or an own-line block comment
            // in the `(`→first-param gap (`(\n/* c */\n p`), forces multiline.
            // `has_line_comments_in_delimited_list` skips this leading gap, and the
            // inline path below emits these with trailing spacing — a line comment
            // swallows the following tokens, a block comment collapses inline. Route
            // to the hardline path so they land on their own line (matches prettier).
            let has_leading_gap_forcing = window_has_comments
                && paren_pos.is_some_and(|p| {
                    let first_start = params[0].span().start;
                    self.has_line_comments_between(p + 1, first_start)
                        || self.has_own_line_block_comment_after(p, p + 1, first_start)
                });
            // A blank line between two params also forces multiline (preserved by the
            // hardline path) — same as regular function params; prettier keeps it.
            if has_line_comments
                || has_own_line_block_after_last
                || has_leading_gap_forcing
                || self.has_blank_line_between_params(params)
            {
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
            let no_leading_comments = !window_has_comments
                || paren_pos
                    .is_none_or(|pos| !self.has_comments_between(pos + 1, params[0].span().start));
            let no_trailing_comments = !window_has_comments
                || close_paren_pos
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
                parts.push(self.identifier_name_doc(id));
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

                // Handle trailing comments after the param (between type literal and
                // close paren); `end_boundary` is that close paren (or the param end
                // fallback — identical for this single-param path).
                let param_end = params[0].span().end;
                for comment in comments_in_range(self.comments, param_end, end_boundary) {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }

                parts.push(d.text(")"));
            } else if !window_has_comments {
                // Zero-comment fast path: plain params joined by `,` + line — no
                // per-gap comma scans or comment lookups. Renders identically (the
                // skipped pushes are empty comment docs and the empty after-comma
                // buffer).
                let mut param_parts = DocBuf::new();
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        param_parts.push(d.text(","));
                        param_parts.push(d.line());
                    }
                    param_parts.push(self.build_function_type_param_expression_doc(p));
                }
                parts.push(d.text("("));
                parts.push(d.indent(d.concat(&[d.softline(), d.concat(&param_parts)])));
                parts.push(d.softline());
                parts.push(d.text(")"));
            } else {
                let mut param_parts = DocBuf::new();
                // Block comment trailing the last param after its source comma — preserved
                // past where the comma was (no trailing comma; prettier relocates before;
                // see conformance_prettier.md §Comment relocation).
                let mut last_after_comma = DocBuf::new();
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
                        // Last param: trailing comments before `)` (`end_boundary` is
                        // the close paren, or the last param end fallback).
                        self.append_last_trailing_block_comments_split(
                            &mut param_parts,
                            &mut last_after_comma,
                            param_end,
                            end_boundary,
                        );
                    }
                }
                parts.push(d.text("("));
                parts.push(d.indent(d.concat(&[d.softline(), d.concat(&param_parts)])));
                // No trailing comma on the last param (trailingComma: 'none').
                // Preserved after-comma block comment(s) on the last param
                parts.extend(last_after_comma);
                parts.push(d.softline());
                parts.push(d.text(")"));
            }
        }
        parts
    }

    /// Build function params with line comments between them (forces multiline)
    fn build_function_params_doc_with_line_comments(
        &self,
        params: &[internal::Expression<'_>],
        paren_pos: Option<u32>,
    ) -> DocBuf {
        let d = self.d();
        let mut parts = DocBuf::new();
        let mut inner_parts = DocBuf::new();

        let open_paren = paren_pos.unwrap_or(0);
        let mut prev_end = open_paren + 1; // After `(`

        // A line comment trailing the opening `(` is kept on the `(` line (divergence
        // from prettier, which relocates it to its own line as the first param's
        // leading comment). See conformance_prettier.md §Comment relocation
        // (Function/constructor-type `(` trailing). Same mechanism as the call-`(`
        // and object/array/block open-delimiter family.
        let (paren_prefix, paren_pull_pos) = paren_pos.map_or_else(
            || (DocBuf::new(), None),
            |open| self.delimiter_line_comment_prefix(open, params[0].span().start),
        );

        for (i, p) in params.iter().enumerate() {
            let param_start = p.span().start;
            let param_end = p.span().end;
            let is_last = i == params.len() - 1;

            // Leading comments (after previous comma or `(`); for the first param,
            // exclude comments already pulled onto the `(` line.
            let skip_delim = if i == 0 { paren_pull_pos } else { None };
            inner_parts.extend(self.build_leading_comments_multiline_opt(
                prev_end,
                param_start,
                skip_delim,
            ));

            inner_parts.push(self.build_function_type_param_expression_doc(p));

            if !is_last {
                let next_start = params[i + 1].span().start;
                prev_end = self.emit_multiline_comma_with_comments(
                    &mut inner_parts,
                    param_end,
                    next_start,
                    true,
                );
            } else {
                // Last param: no trailing comma (trailingComma: 'none') + comments before `)`
                let close_paren = paren_pos
                    .and_then(|p| self.matching_close_paren(p))
                    .unwrap_or(param_end);
                inner_parts.extend(self.build_trailing_comments_multiline(param_end, close_paren));
                prev_end = close_paren;
            }
        }

        parts.push(d.text("("));
        parts.push(d.concat(&paren_prefix));
        parts.push(d.indent(d.concat(&[d.hardline(), d.concat(&inner_parts)])));
        parts.push(d.hardline());
        parts.push(d.text(")"));
        parts
    }
}
