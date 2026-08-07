// Function type printing for TypeScript
//
// Handles:
// - Function types: `(a: T) => U`
// - Constructor types: `new () => T`
// - Signature parameters (shared with type members)
// - Return type annotations

use super::super::comments_to_emit_in_range;
use super::super::expressions::functions::{has_huggable_type_annotation, is_huggable_pattern};
use super::helpers::type_args_should_wrap_for_return_type;
use super::{BlankRule, CommentSpacing, Printer};
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

/// Prettier's `shouldGroupFunctionParameters`: wrap params in their own group
/// when there's 1 param and the return type is an object type or will break, so
/// the params stay flat even when the outer signature group breaks.
///
/// Takes decomposed fields (return type + its already-built doc, both `Option`)
/// so it serves every signature shape: `FunctionDeclaration` / `FunctionExpression`
/// (function declarations, class methods), function/constructor TYPES, and the
/// type-member / bodyless (`declare`, overload) signatures.
pub(in crate::printer) fn should_group_function_parameters(
    params: &[internal::Expression<'_>],
    type_parameters: Option<&internal::TSTypeParameterDeclaration<'_>>,
    return_type: Option<&internal::TSTypeAnnotation<'_>>,
    return_type_doc: Option<DocId>,
    d: &DocArena,
) -> bool {
    if params.len() != 1 {
        return false;
    }
    let Some(rt_doc) = return_type_doc else {
        return false;
    };
    if !type_params_allow_grouping(type_parameters) {
        return false;
    }
    return_type.is_some_and(|rt| return_type_triggers_grouping(rt, rt_doc, d))
}

/// Wrap `params_doc` in its own group when `should_group_function_parameters` holds,
/// else return it unchanged. The four signature builders (function declarations /
/// class methods, function expressions / object methods, type-member / bodyless
/// signatures, and function/constructor types) share this guard so a single param
/// hugs (stays flat) while a will-break return type breaks.
pub(in crate::printer) fn group_params_if_should(
    params_doc: DocId,
    params: &[internal::Expression<'_>],
    type_parameters: Option<&internal::TSTypeParameterDeclaration<'_>>,
    return_type: Option<&internal::TSTypeAnnotation<'_>>,
    return_type_doc: Option<DocId>,
    d: &DocArena,
) -> DocId {
    if should_group_function_parameters(params, type_parameters, return_type, return_type_doc, d) {
        d.group(params_doc)
    } else {
        params_doc
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
        // The **effective** return type: a redundant paren shell holding only leading
        // comments is stripped here, so those comments belong to the `=>` gap below and
        // are emitted by it (see `leading_paren_unwrapped`) — that is what makes
        // `() => // c⏎T` and `() => (// c⏎T)` reach the same fixed point instead of the
        // shell's own flush hang.
        let return_ty = self.leading_paren_unwrapped(return_type.type_annotation);
        let type_start = return_ty.span().start;
        // An alone-on-line format-ignore directive in the `=>`→return gap stays
        // OWN-LINE — the trailing-hang emitter below would relocate it to trail the
        // `=>` (`=> // prettier-ignore`), an inert placement that loses the freeze on
        // the second pass — and freezes a non-composite return type verbatim
        // (`single_child_frozen`; a composite return declines and freezes via its own
        // leading-run walk, which reaches the directive across the gap's whitespace).
        // Covers function, constructor, and abstract-constructor types (all route here).
        if self.member_gap_frozen(arrow_end, type_start) {
            let value_doc = self.build_routed_child_doc(return_ty);
            let mut parts: DocBuf = smallvec![d.text(arrow)];
            self.append_keyword_value_line_comments(&mut parts, arrow_end, type_start, value_doc);
            return d.concat(&parts);
        }
        // Use break-for-line variant: line comments must force a hardline before
        // the return type so they don't swallow it (`=> // c\nT`, not `=> // c T`).
        // `None` on the comment-free path so none of the five layouts below carries an
        // empty child — every function type (`() => void`, every callback parameter)
        // reaches one of them.
        let comments_doc = self
            .has_comments_to_emit_between(arrow_end, type_start)
            .then(|| self.build_trailing_comments_hang_next(arrow_end, type_start));
        // A comment that hangs the return type takes the continuation indent every
        // other keyword→value gap takes (§Uniform Forced-Continuation Indent) — the
        // `member_gap_frozen` arm above already gets it from
        // `append_keyword_value_line_comments`, and this is the same seam without a
        // directive. The indent wraps the comment run *and* the type, so the hardline
        // inside the run is what carries it; an inline block hangs nothing, so the
        // wrapper is gated rather than unconditional (it would be inert, but the gate
        // states which case it is for).
        let hangs =
            comments_doc.is_some() && self.comments_force_own_line_between(arrow_end, type_start);
        // `<lead><comments><type>`, skipping the comment slot when the gap is bare.
        let joined = |lead: DocId, ty: DocId| match comments_doc {
            Some(c) if hangs => d.concat(&[lead, d.indent(d.concat(&[c, ty]))]),
            Some(c) => d.concat(&[lead, c, ty]),
            None => d.concat(&[lead, ty]),
        };
        // Strip redundant comment-free parens so `($A | $B)` / `($A & $B)` return
        // types get the same hanging layout as the bare form (prettier strips them
        // too). Only union/intersection are unwrapped; other parenthesized types
        // keep the match-on-original fall-through below.
        let value_type = self.unwrap_redundant_parens(return_ty);
        if let TSType::Union(u) = value_type {
            let type_doc = self.build_union_type_doc(u);
            // A brace-hugging union return (`{ … } | null` / `| void`) hugs `=>`
            // block-style: the object owns its own expansion and the void member
            // trails the `}`, the same layout the type-alias RHS / `as` cast use
            // (`build_union_type_doc`'s hug path). See `union_return_hugs` for the
            // scope: a `Promise<…> | null` `TSTypeReference` member is deliberately
            // NOT hugged (the sanctioned `return_type_generic_union` print-width
            // family), and a member/gap comment disqualifies the hug — those fall
            // through to the break-after-operator layout that matches prettier there.
            if self.union_return_hugs(value_type, arrow_end, type_start) {
                return joined(d.text(arrow_sp), type_doc);
            }
            let hung = match comments_doc {
                Some(c) => d.concat(&[c, type_doc]),
                None => type_doc,
            };
            return d.concat(&[d.text(arrow), hang_after_operator(d, hung)]);
        }
        if let TSType::Intersection(i) = value_type {
            // Delegate to the shared bare hanging printer (same layout the type-alias
            // RHS / `as` cast use): a huggable/expanding boundary returns bare
            // `type_doc` (the object owns its own expansion — no extra indent), and a
            // pure-non-object intersection gets `group(type_doc)` with the bare printer
            // owning its continuation indent (a first member that breaks internally
            // isn't double-indented). The old inline `group(indent(type_doc))` for the
            // huggable branch double-indented the object body.
            let wrapped = self.intersection_hanging_with_indent(i);
            return joined(d.text(arrow_sp), wrapped);
        }
        match return_ty {
            // TypeReference with complex type args (like Promise<Result<...>>):
            // Build with wrapping type args so it can break inside the <...>
            TSType::TypeReference(r)
                if r.type_arguments
                    .as_ref()
                    .is_some_and(type_args_should_wrap_for_return_type) =>
            {
                // The type reference's own type arguments wrap internally when too wide.
                let type_doc = self.build_type_doc(return_ty);
                joined(d.text(arrow_sp), type_doc)
            }
            _ => joined(d.text(arrow_sp), self.build_type_doc(return_ty)),
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
        //
        // The two source scans below serve only that gap, and the `(` scan runs to the
        // end of the source in the worst case — so ask the cheap question first. Any
        // `after_close` they could find lies at or past `paren_search_start`, so a
        // comment-free `[paren_search_start, arrow_start]` window means the `filter`
        // would reject whatever they returned. Skipping them is byte-identical.
        let arrow_start = return_type.span.start;
        let after_close = self
            .has_comments_to_emit_between(paren_search_start, arrow_start)
            .then(|| {
                find_char_skipping_comments(
                    self.source.as_bytes(),
                    paren_search_start as usize,
                    self.source.len(),
                    b'(',
                )
                .and_then(|open| self.find_closing_paren(open as u32, arrow_start))
                .filter(|&after_close| self.has_comments_to_emit_between(after_close, arrow_start))
            })
            .flatten();

        // A line comment in the `)`→`=>` gap can't stay inline — it would swallow
        // `=> void` (`() // c => void`). Keep it trailing `)` and force `=>` onto
        // the next line flush (`() // c\n=> void`). With no params (as here) prettier
        // agrees — there is no param to move it onto; with params it relocates the
        // comment onto the last param and breaks the list (a divergence — see
        // pre_arrow_param_line_comment_prettier_divergence, mirrored by the `)`→`:`
        // return-type gap). A block comment stays inline (`() /* c */ => void`).
        let pre_arrow_line_close =
            after_close.filter(|&ac| self.has_line_comments_between(ac, arrow_start));
        // An alone-on-line format-ignore directive in the `)`→`=>` gap freezes the
        // WHOLE return annotation verbatim — `=> T` is the node the directive
        // precedes (`build_frozen_span_doc`, the span analog of the single-child
        // freeze; comments inside the slice ride out verbatim). Emission preserves
        // each gap comment's own-line-ness: a `)`-trailing comment keeps trailing,
        // and the directive keeps its own line — the pull-to-trailing hang below
        // would leave it trailing `)`, an inert placement that loses the freeze on
        // the second pass.
        //
        // Asked of `after_close`, NOT the line-comment-filtered `pre_arrow_line_close`:
        // PLACEMENT, not spelling, keys honoring, so a BLOCK-spelled directive alone on
        // its line freezes identically. The line-comment filter answers a LAYOUT
        // question (must `=>` start a fresh line?), and routing the honoring question
        // through it silently dropped the block spelling onto the inline path. The
        // sibling gaps route the same way — the `=>`→return gap above, the `as` /
        // `satisfies` keyword gap, and the type-predicate `is` gap all OR the freeze
        // verdict into their routing rather than gating it on the spelling.
        let frozen_pre_arrow = after_close.filter(|&ac| self.member_gap_frozen(ac, arrow_start));
        let return_type_doc = if let Some(ac) = frozen_pre_arrow {
            // Own-line-preserving, the same emitter the routed conditional branches
            // use: a `)`-trailing comment keeps trailing and the directive keeps its
            // own line, in source order (the trailing run is the gap's prefix).
            let (trailing, own_line) = self.build_own_line_preserving_run(ac, arrow_start);
            let mut pre_parts: DocBuf = smallvec![trailing];
            pre_parts.extend(own_line);
            pre_parts.push(d.hardline());
            pre_parts.push(self.build_frozen_span_doc(return_type.span));
            d.concat(&pre_parts)
        } else {
            let return_type_doc =
                self.build_function_type_return_doc(return_type, pre_arrow_line_close.is_none());
            if let Some(ac) = pre_arrow_line_close {
                let pre = self.build_trailing_comments_hang_next(ac, arrow_start);
                d.concat(&[d.text(" "), pre, return_type_doc])
            } else {
                match after_close {
                    Some(ac) => d.concat(&[
                        self.build_comments_between(ac, arrow_start, CommentSpacing::Leading),
                        return_type_doc,
                    ]),
                    None => return_type_doc,
                }
            }
        };

        let params_doc = d.concat(&self.build_function_params_doc(params, paren_search_start));
        let params_doc = group_params_if_should(
            params_doc,
            params,
            type_parameters,
            Some(return_type),
            Some(return_type_doc),
            d,
        );

        [params_doc, return_type_doc]
    }

    //
    // Signature Helpers (shared with type members)
    //

    /// The params' close paren (the position just past the `)`) bounding a `)`→return-type
    /// gap, resolved ONCE for the two consumers keyed on it — the frozen route
    /// ([`Self::build_frozen_return_type_doc`]) and the gap's comment prefix
    /// ([`Self::build_close_paren_to_return_type_comments`]) — so neither walks the
    /// parameter list the other just walked. A caller that already located its close paren
    /// for other boundaries (a function declaration, whose scan also bounds the params'
    /// trailing comments) passes it to those two directly and skips this.
    ///
    /// Depth-tracked (skips nested parens / comments) — the naive first-`)` scan mis-fires
    /// on complex params and pulls real param-trailing comments into the gap
    /// (duplication).
    ///
    /// That scan walks the whole parameter list byte by byte, and it exists ONLY to bound
    /// the two comment-keyed questions above, so it is skipped when the WIDER
    /// `(`→return-type window holds no comment TO EMIT: the `)`→`:` window is contained in
    /// it, so the emitter's loop could not have run, and no *honorable* directive can sit
    /// in the gap either — the freeze needs the directive alone on its line, so a newline
    /// follows it, so it is not glued, so it is never `owned_by_node` and always in the
    /// to-emit set (`member_gap_frozen`'s in-source axis and this to-emit gate agree for
    /// exactly that reason). Either way a `None` close paren yields the same empty doc and
    /// the same declined freeze. One binary search replaces a per-signature byte walk on
    /// the common (comment-free) path.
    pub(in crate::printer) fn return_type_close_paren(
        &self,
        paren_pos: Option<u32>,
        return_type_start: u32,
    ) -> Option<u32> {
        paren_pos
            .filter(|&p| self.has_comments_to_emit_between(p, return_type_start))
            .and_then(|p| self.find_closing_paren(p, return_type_start))
    }

    /// The frozen return-type route for a `)`→`:` gap: an alone-on-line format-ignore
    /// directive there freezes the whole `: type` annotation and keeps its own line
    /// ([`Self::build_frozen_annotation_head_doc`]), replacing the ordinary
    /// prefix-plus-annotation emission of all three hosts — a function/method
    /// declaration, an arrow, and a type-member signature. Without it the gap's first
    /// comment trails `)` (`build_close_paren_to_return_type_comments`), which is inert
    /// under the placement classification and would lose the freeze on the second pass.
    ///
    /// Takes the already-located `close_paren_after` (via
    /// [`Self::return_type_close_paren`] where the caller doesn't have one), so it shares
    /// that scan with the comment emitter below rather than running a second one.
    pub(in crate::printer) fn build_frozen_return_type_doc(
        &self,
        close_paren_after: Option<u32>,
        return_type: &internal::TSTypeAnnotation<'_>,
    ) -> Option<DocId> {
        self.build_frozen_annotation_head_doc(close_paren_after?, return_type)
    }

    /// Emit any comments in the `)`→return-type gap, so the caller can append the
    /// `: type` after this prefix (`close_paren_after` = the position just past the `)`).
    ///
    /// A **block** comment stays inline with a trailing space, and prettier adds a
    /// space before `:` when one precedes it (`m(a) /* c */ : void`). A **line**
    /// comment forces the return-type `:` onto the next line (`m(a) // c⏎: void`)
    /// so it isn't swallowed. Returns an empty doc when there is no such comment.
    pub(in crate::printer) fn build_close_paren_to_return_type_comments(
        &self,
        close_paren_after: Option<u32>,
        return_type_start: u32,
    ) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = smallvec![];
        let mut last_is_line = false;
        if let Some(close_after) = close_paren_after {
            for comment in comments_to_emit_in_range(self.comments, close_after, return_type_start)
            {
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
                // A `//` line comment can't stay inline — it would swallow the
                // return type (`) // c : T`). Force the following `:` onto the next
                // line, keeping the comment trailing `)` (preserve-position;
                // prettier relocates it onto the last param). Mirrors the
                // function-type `)`→`=>` gap's line-comment handling.
                if !comment.is_block {
                    parts.push(d.hardline());
                }
                last_is_line = !comment.is_block;
            }
        }
        // Prettier adds a space before `:` when a block comment precedes it; after a
        // line comment the hardline already placed `:` flush on the next line.
        if !parts.is_empty() && !last_is_line {
            parts.push(d.text(" "));
        }
        d.concat(&parts)
    }

    /// [`Self::build_function_return_type_doc`] for a caller that has only the params'
    /// `(` — it locates the close paren once ([`Self::return_type_close_paren`]) and
    /// hands it on. Used by MethodSignature, CallSignature, ConstructSignature
    /// (type-literal and interface members) and the declare-function signature.
    pub(in crate::printer) fn build_signature_return_type_doc(
        &self,
        paren_pos: Option<u32>,
        return_type: &internal::TSTypeAnnotation<'_>,
    ) -> DocId {
        self.build_function_return_type_doc(
            self.return_type_close_paren(paren_pos, return_type.span.start),
            return_type,
        )
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
        let params_doc = group_params_if_should(
            params_doc,
            params,
            type_parameters,
            return_type,
            return_type_doc,
            d,
        );

        let mut sig_parts: DocBuf = smallvec![params_doc];
        if let Some(rt_doc) = return_type_doc {
            sig_parts.push(rt_doc);
        }
        d.group(d.concat(&sig_parts))
    }

    /// Build a return type (`: T`) with `)`→`:` comment handling, using the return-type
    /// type variant (wraps unions/intersections so params break first). The caller
    /// supplies the already-located close paren (position just past the `)`); the
    /// `(`-only twin is [`Self::build_signature_return_type_doc`].
    pub(in crate::printer) fn build_function_return_type_doc(
        &self,
        close_paren_after: Option<u32>,
        return_type: &internal::TSTypeAnnotation<'_>,
    ) -> DocId {
        let d = self.d();
        // The frozen `)`→`:` route, on the close paren this caller already located.
        if let Some(frozen) = self.build_frozen_return_type_doc(close_paren_after, return_type) {
            return frozen;
        }
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
            return self.build_empty_params_with_comments_doc(paren_pos, self.source.len() as u32);
        }

        // Check for line comments or own-line block comments that force multiline
        let close_paren_pos = paren_pos.and_then(|p| self.matching_close_paren(p));

        // Prettier's shouldHugFunctionParameters: a single param that's an object/array
        // pattern (or carries an object/type-literal annotation) hugs — `(o: {` and
        // `}: T)` stay together while the object's own group breaks, instead of the
        // param LIST breaking around it. Mirrors `build_params_doc_with_comments` for
        // value params; this is the signature-context path (bodyless declare/overload
        // functions, method/call/construct signatures). Skipped when a comment sits in
        // the `(`→param or param→`)` gap — the breakable path below places those.
        let no_hug_comments = paren_pos
            .is_none_or(|p| !self.has_comments_to_emit_between(p + 1, params[0].span().start))
            && close_paren_pos
                .is_none_or(|c| !self.has_comments_to_emit_between(params[0].span().end, c));
        if params.len() == 1
            && (is_huggable_pattern(&params[0]) || has_huggable_type_annotation(&params[0]))
            && no_hug_comments
            && !self.param_has_own_line_decorators(&params[0])
        {
            return d.parens(self.build_function_type_param_expression_doc(&params[0]));
        }
        let end_boundary =
            close_paren_pos.unwrap_or_else(|| params.last().map_or(0, |p| p.span().end));
        // Zero-comment window gate: one binary search over the whole params window.
        // Every comment sub-query here (leading-gap / delimited-list / last-param) and
        // in the fast-path build loop below is bounded within `[window_start,
        // end_boundary]`, so with no comment inside the window all are provably
        // empty/false — skip them on the common comment-free signature. The blank-line
        // check is a source blank-line test independent of comments and stays outside
        // the gate. Mirrors `build_params_doc_with_comments`'s fast gate.
        let window_start = paren_pos.map_or_else(|| params[0].span().start, |p| p + 1);
        let comments_present = self.has_comments_on_page_between(window_start, end_boundary);
        // A line comment trailing `(` (`(// c\n p`), or an own-line block comment in
        // the `(`→first-param gap, forces multiline (else the inline path below lets a
        // line comment swallow the following tokens, `(// c p: T)`; mirrors
        // `build_function_params_doc`'s leading-gap check). A blank line the author
        // left between two params also forces multiline (preserved by the separator
        // emission below) — same as regular function params; prettier keeps the blank
        // in every parameter-list position.
        let force_multiline =
            self.type_params_force_multiline(params, paren_pos, end_boundary, comments_present);

        if force_multiline {
            // Hardline params layout, shared with the function/constructor-type path.
            // Wrapped in this signature's own group (a method signature keeps a
            // trailing-`(` line comment in place, a call/construct signature relocates
            // it — that divergence lives in `delimiter_line_comment_prefix`, inside the
            // shared helper).
            return d.group(d.concat(&self.build_type_params_multiline_parts(params, paren_pos)));
        }

        // Build params with width-based breaking
        let mut param_parts = DocBuf::new();

        // Handle comments before first param (e.g., `(/* comment */ a: T)`) — gated by
        // the zero-comment window check above (the range is inside the window).
        if comments_present && let Some(paren_pos) = paren_pos {
            let first_param_start = params[0].span().start;
            for comment in
                comments_to_emit_in_range(self.comments, paren_pos + 1, first_param_start)
            {
                param_parts.push(self.build_comment_doc(comment));
                param_parts.push(d.text(" "));
            }
        }

        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                param_parts.push(d.text(","));
                param_parts.push(d.line());
            }
            param_parts.push(self.build_function_type_param_item_doc(paren_pos, params, i));

            // Handle trailing comments after this param — gated by the window check
            // (each param→next-boundary gap is inside the window).
            if comments_present {
                let param_end = param.span().end;
                let next_boundary = if i + 1 < params.len() {
                    params[i + 1].span().start
                } else {
                    close_paren_pos.unwrap_or(param_end)
                };

                for comment in comments_to_emit_in_range(self.comments, param_end, next_boundary) {
                    param_parts.push(d.text(" "));
                    param_parts.push(self.build_comment_doc(comment));
                }
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

    /// Whether a type-side parameter list must take the hardline layout
    /// ([`Self::build_type_params_multiline_parts`]) rather than a width-decided one.
    /// The single statement of that decision for both type-side hosts — the type-member
    /// signature path ([`Self::build_signature_params_doc`]) and the
    /// function/constructor-type path ([`Self::build_function_params_doc`]) — which
    /// share the layout emitter and so must share the question that routes to it.
    ///
    /// The legs, in the order they can fire: an author blank line between two params
    /// (comment-independent, so it is asked first and outside the gate); then, only in a
    /// comment-bearing window, an own-line comment leading any param, a line comment
    /// anywhere in the delimited list, a line comment or own-line block in the
    /// `(`→first-param gap, and an own-line block after the last param. Each of these
    /// would otherwise be swallowed or collapsed by an inline layout.
    ///
    /// `comments_present` is the caller's window gate (its window is a superset of every
    /// range asked here), so a comment-free list pays one blank-line scan and nothing else.
    fn type_params_force_multiline(
        &self,
        params: &[internal::Expression<'_>],
        paren_pos: Option<u32>,
        end_boundary: u32,
        comments_present: bool,
    ) -> bool {
        if self.has_blank_line_between_params(params) {
            return true;
        }
        if !comments_present {
            return false;
        }
        self.has_leading_own_line_comment_in_params(params, paren_pos.map(|p| p + 1))
            || self.has_line_comments_in_delimited_list(
                params,
                internal::Expression::span,
                end_boundary,
            )
            || paren_pos.is_some_and(|p| {
                params.first().is_some_and(|first| {
                    let first_start = first.span().start;
                    self.has_line_comments_between(p + 1, first_start)
                        || self.has_own_line_block_comment_after(p, p + 1, first_start)
                })
            })
            || params.last().is_some_and(|last| {
                self.has_own_line_block_comment_after(
                    last.span().end,
                    last.span().end,
                    end_boundary,
                )
            })
    }

    /// A type-side parameter list item: the freeze-aware layer over
    /// [`Self::build_function_type_param_expression_doc`]. An alone-on-line
    /// format-ignore directive leading parameter `i` freezes it verbatim (Rule A).
    /// Shared by all three type-side loops — the signature path, the
    /// function/constructor-type path, and their common multiline layout — so the
    /// placement question is asked once per position. The value-side twin is
    /// `build_function_parameter_item_doc` (type-side parameters carry no decorators,
    /// so they have only the one freeze position).
    fn build_function_type_param_item_doc(
        &self,
        paren_pos: Option<u32>,
        params: &[internal::Expression<'_>],
        i: usize,
    ) -> DocId {
        self.param_frozen_span(paren_pos, params, i).map_or_else(
            || self.build_function_type_param_expression_doc(&params[i]),
            |frozen| self.build_frozen_span_doc(frozen),
        )
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
                let comments_doc = self.build_trailing_comments_hang_next(dots_end, arg_start);
                let mut parts: DocBuf = smallvec![
                    d.text("..."),
                    comments_doc,
                    self.build_function_type_param_expression_doc(rest.argument),
                ];
                // The optional `?` marker and the `: type` annotation, with the comment
                // landings both of their gaps need — `wrap` is true to match the
                // `Identifier` arm above, which breaks its annotation's generic arguments
                // at print width. See `push_rest_element_tail_doc`, shared with the
                // value-side `build_rest_element_doc`.
                self.push_rest_element_tail_doc(&mut parts, rest, true);
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
            parts.push(
                self.build_empty_params_with_comments_doc(paren_pos, self.source.len() as u32),
            );
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
                self.has_comments_on_page_between(window_start, end_boundary)
            };

            // The hardline layout and the decision that routes to it are both shared with
            // the signature path: a line comment anywhere in the list or the `(` gap would
            // be swallowed by the inline path below, an own-line block would collapse into
            // it, and an author blank line must survive.
            if self.type_params_force_multiline(
                params,
                paren_pos,
                end_boundary,
                window_has_comments,
            ) {
                return self.build_type_params_multiline_parts(params, paren_pos);
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
                || paren_pos.is_none_or(|pos| {
                    !self.has_comments_to_emit_between(pos + 1, params[0].span().start)
                });
            let no_trailing_comments = !window_has_comments
                || close_paren_pos
                    .is_none_or(|cp| !self.has_comments_to_emit_between(params[0].span().end, cp));
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
                if window_has_comments
                    && let Some(comments) = self
                        .build_inline_comments_between_doc_trailing_space_opt(colon_end, type_start)
                {
                    parts.push(comments);
                }
                parts.push(self.build_type_literal_doc_for_function_param(type_literal));

                // Handle trailing comments after the param (between type literal and
                // close paren); `end_boundary` is that close paren (or the param end
                // fallback — identical for this single-param path).
                let param_end = params[0].span().end;
                for comment in comments_to_emit_in_range(self.comments, param_end, end_boundary) {
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
                // see conformance_prettier_ts_comments.md §Comment relocation).
                let mut last_after_comma = DocBuf::new();
                let mut prev_end = paren_pos.map_or(0, |p| p + 1); // After `(`
                for (i, p) in params.iter().enumerate() {
                    // Leading comments start after the previous comma (`prev_end`); a
                    // stranded after-comma block (on the comma's line, newline before
                    // this param) trails the comma instead of leading this param —
                    // matching function params / call args (prettier relocates it before
                    // the comma). See conformance_prettier_ts_comments.md §Comment relocation.
                    let mut leading_start = prev_end;
                    if i > 0 {
                        param_parts.push(d.text(","));
                        let comma = prev_end - 1;
                        for comment in
                            comments_to_emit_in_range(self.comments, comma, p.span().start)
                        {
                            if !self.is_stranded_after_comma_block(comment, comma, p.span().start) {
                                break; // stranded blocks are a contiguous prefix on the comma line
                            }
                            param_parts.push(d.text(" "));
                            param_parts.push(self.build_comment_doc(comment));
                            leading_start = comment.span.end;
                        }
                        param_parts.push(d.line());
                    }

                    // Leading block comments (after the previous comma / stranded blocks, or `(`)
                    param_parts.push(self.build_inline_comments_between_doc_trailing_space(
                        leading_start,
                        p.span().start,
                    ));

                    param_parts.push(self.build_function_type_param_item_doc(paren_pos, params, i));

                    // Trailing block comments (before comma or `)`)
                    let param_end = p.span().end;
                    if i + 1 < params.len() {
                        let next_start = params[i + 1].span().start;
                        let comma_pos = self.find_list_comma(param_end, next_start);
                        // Only the run that follows content on its line trails this param;
                        // the rest leads the next one, so the leading scan resumes at the
                        // run's end rather than past the comma
                        // (`Printer::inline_trailing_run_end`).
                        let run_end = self.inline_trailing_run_end(param_end, comma_pos);
                        self.append_trailing_inline_block_comments(
                            &mut param_parts,
                            param_end,
                            run_end,
                        );
                        prev_end = run_end;
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

    /// Emit the multiline (hardline) parameter layout shared by the two type-side
    /// param paths: the type-member signature path (`build_signature_params_doc`, when
    /// a comment / blank line forces multiline) and the function/constructor-type path
    /// (`build_function_params_doc`). Renders `(⏎\t<param>,⏎\t…⏎)` — each param via
    /// `build_function_type_param_expression_doc`, with comments preserved in every gap
    /// (the `(`-line trailing-comment pull, inter-param commas, trailing before `)`).
    ///
    /// Returns the assembled parts **ungrouped**: the signature caller wraps them in
    /// its own group; the function-type caller lets the outer function-type group
    /// govern (a group around hardline-only content is a no-op, so both agree).
    fn build_type_params_multiline_parts(
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
        // leading comment). See conformance_prettier_ts_comments.md §Comment relocation
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
            inner_parts.extend(self.build_leading_comments_multiline(
                prev_end,
                param_start,
                skip_delim,
            ));

            inner_parts.push(self.build_function_type_param_item_doc(paren_pos, params, i));

            if !is_last {
                let next_start = params[i + 1].span().start;
                prev_end = self.emit_multiline_comma_with_comments(
                    &mut inner_parts,
                    param_end,
                    next_start,
                    BlankRule::NextLineEmpty,
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
