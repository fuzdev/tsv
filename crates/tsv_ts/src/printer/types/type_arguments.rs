// Type-argument instantiation (`<T, U>`) rendering

use super::Printer;
use super::helpers::{is_huggable_type, is_simple_type_arg, unwrap_parenthesized};
use crate::ast::internal::{self, TSType};
use smallvec::smallvec;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build doc for a type used as a type argument.
    ///
    /// For single type arg contexts, uses normal doc (allows object types to break).
    /// For multiple type arg contexts, uses hugging (objects don't break independently).
    pub(in crate::printer) fn build_type_arg_doc(
        &self,
        param: &TSType<'_>,
        is_multi_arg: bool,
    ) -> DocId {
        if is_multi_arg {
            self.build_type_doc_for_type_arg(param)
        } else {
            self.build_type_doc(param)
        }
    }

    /// Emit a single type argument inline: `<` + leading inline block comments + the
    /// type doc + trailing inline block comments + `>`. No group, no softlines — the
    /// argument is atomic, so an overflowing head breaks *around* the `<…>` (the call
    /// arguments, the assignment `=`) rather than inside it; any brace-delimited member
    /// carries its own group and still breaks block-style within the hugged `<…>`.
    ///
    /// Assumes `args.params.len() == 1` **and** that the caller has gated on the argument
    /// actually hugging — [`Self::type_arg_hugs`] for both builders here, and its split
    /// spelling in `type_params.rs` (whose object case routes through
    /// `try_build_hugging_curly_type_doc`, a documented divergence). Own-line and line
    /// comments are routed to the multiline path before this runs, so only inline block
    /// comments remain to preserve here. Shared by the type-position builder and the
    /// call/`new`/instantiation builder.
    /// `has_comments` is the caller's whole-`<…>` window answer: `false` proves both
    /// gaps below are comment-free, so neither is searched.
    pub(in crate::printer) fn build_single_type_arg_inline(
        &self,
        args: &internal::TSTypeParameterInstantiation<'_>,
        has_comments: bool,
    ) -> DocId {
        let type_doc = self.build_type_doc(&args.params[0]);
        self.build_single_type_arg_inline_with(args, has_comments, type_doc)
    }

    /// [`Self::build_single_type_arg_inline`] with a caller-built argument doc — the
    /// shared emission for the instantiation curly-hug arm, whose argument doc comes
    /// from `try_build_hugging_curly_type_doc` rather than `build_type_doc`.
    /// Applies the Rule A freeze itself (swapping `type_doc` for the frozen verbatim
    /// slice), so every single-argument hug path honors it.
    pub(in crate::printer) fn build_single_type_arg_inline_with(
        &self,
        args: &internal::TSTypeParameterInstantiation<'_>,
        has_comments: bool,
        type_doc: DocId,
    ) -> DocId {
        let d = self.d();
        let param = &args.params[0];
        // Rule A: an alone-on-line directive before the sole argument freezes it whole
        // (`//` spellings route to the expansion builder before this; an alone-on-line
        // block spelling can reach this inline path).
        let frozen =
            has_comments && self.list_item_frozen(args.span.start + 1, &|_| param.span(), 0);
        let arg_doc = if frozen {
            self.build_frozen_list_member_doc(param)
        } else {
            type_doc
        };
        let mut parts = smallvec![d.text("<")];
        if has_comments {
            let after_open = args.span.start + 1; // After the opening `<`
            let before_close = args.span.end - 1; // Before the closing `>`
            self.append_leading_inline_block_comments(&mut parts, after_open, param.span().start);
            parts.push(arg_doc);
            self.append_trailing_inline_block_comments(&mut parts, param.span().end, before_close);
        } else {
            parts.push(arg_doc);
        }
        parts.push(d.text(">"));
        d.concat(&parts)
    }

    /// Whether a **single type argument** hugs — i.e. whether `<T>` inlines atomically.
    /// Prettier's `shouldHugType` (`print/type-annotation.js`), and the whole answer, so no
    /// type-argument site re-derives it from parts. Three clauses:
    ///
    /// 1. a **simple** type ([`is_simple_type_arg`]) — atomic, never benefits from breaking;
    /// 2. an **object** type ([`is_huggable_type`] — `TypeLiteral`/`Mapped`), which carries
    ///    its own group and breaks block-style *inside* the hugged `<…>`;
    /// 3. a **hugged union** ([`Self::type_arg_union_prints_hugged`] — `{ … } | null`), whose
    ///    object member likewise owns the expansion.
    ///
    /// For TypeScript this *is* prettier's `shouldInline` at `len == 1`: its remaining
    /// disjunct, `NullableTypeAnnotation`, is Flow-only, and its `isParameterInTestCall` /
    /// `isArrowFunctionVariable` clauses gate call-site `<…>`, not a type-position one.
    ///
    /// ⚠️ A non-hugging argument (an intersection, a function type, a conditional) must
    /// **not** inline: `build_single_type_arg_inline` emits no group and no softlines, so an
    /// inlined `<…>` has no break point and an overflowing head breaks *around* the brackets
    /// (the enclosing operand, or the assignment `=`) instead of inside them.
    pub(in crate::printer) fn type_arg_hugs(&self, ty: &TSType<'_>) -> bool {
        is_simple_type_arg(ty)
            || is_huggable_type(unwrap_parenthesized(ty))
            || self.type_arg_union_prints_hugged(ty)
    }

    /// Comments that force the `<...>` list to the multiline layout: line
    /// comments anywhere (including before the first argument, e.g.
    /// `Foo<// leading\n  a>`) or own-line block comments — neither can render
    /// inline. Shared by both type-argument builders (the type-position
    /// `build_type_arguments_doc*` and the call/`new` instantiation
    /// `build_type_parameter_instantiation_doc`).
    /// `has_comments` is the caller's whole-`<…>` window answer. Every sub-query below
    /// is bounded within `[args.span.start, args.span.end]`, and `has_comments_to_emit_between`
    /// only yields comments fully inside its range — so when no comment lies inside the
    /// `<…>`, all three are provably false. Callers hold the flag rather than
    /// recomputing it here, because they gate their own per-argument comment work
    /// (and its trivia scans) on the same answer.
    pub(in crate::printer) fn type_arguments_force_expansion(
        &self,
        args: &internal::TSTypeParameterInstantiation<'_>,
        has_comments: bool,
    ) -> bool {
        if !has_comments {
            return false;
        }
        // Every clause below asks the **effective** argument span — the node left once a
        // redundant paren shell carrying only leading comments is stripped
        // ([`Self::leading_paren_unwrapped`]), the same node the expansion builder emits.
        // The shell is stripped in this position, so a comment the author wrote just
        // inside it physically lands in one of the list's own gaps (`<`→argument,
        // argument→argument, argument→`>`) and must route the list here like any other
        // comment there. Asking `TSType::span` left those gaps empty, so the shape fell
        // through to the group path and the comment rendered from the argument's own
        // doc — reaching a different fixed point than the same comment written without
        // the parens (`Foo<(⏎/* c */⏎T)>` collapsing inline where the bare authoring
        // expands; `Foo<A, (// c⏎B)>` leading `B` where the bare authoring trails the
        // comma). Both spellings are idempotent, so only a bare-vs-paren comparison
        // shows the split — the `unformatted_parens` variants are that claim.
        //
        // The strip is also what settles the second case against the element-comma seam
        // ([`comments.md`](../../../../docs/comments.md)): the two authorings have
        // byte-identical stripped forms, so the shell carries no position signal past
        // its own deletion, and the seam's partition claims the comment for the previous
        // argument's trailing run exactly as it does for the bare form.
        //
        // Own-line-ness is measured on the SOURCE for the same reason the strip happens at
        // all: the `(` occupies the comment's line whether or not the item span still
        // covers it, so a block glued to the shell (`Foo<A,⏎(/* c */⏎B)>`) collapses inline
        // — matching prettier, and matching the tuple's identical spelling. Asking the item
        // BOUNDARY instead reads that block as own-line and expands the list, a third fixed
        // point neither the bare authoring nor prettier produces;
        // `type_position_parens_glued_block_comment_prettier_divergence` is that claim. Only
        // a block the author gave a line of its own expands.
        //
        // The same reading covers a shell one link DOWN — at the argument's leading printed
        // edge (`Foo<A, (⏎// c⏎B) & C>`), where the argument node itself is not the shell:
        // [`Self::list_item_printed_span`] starts the argument past that run, so the run is
        // in this list's own gap here and at every emitter below.
        //
        // `false` for the widening's decline: a FROZEN argument's run rides inside its
        // verbatim slice rather than this list's gaps, but a freeze implies an
        // alone-on-line directive in that same gap, which is itself an expanding comment
        // — so both readings force expansion and the router cannot disagree with the
        // emitter (which does hold the per-item verdict).
        let arg_span =
            |ty: &TSType<'_>| self.list_item_printed_span(false, self.leading_paren_unwrapped(ty));
        self.has_expanding_comments_in_bracket_list(args.span, args.params, arg_span)
    }

    /// Build doc for type arguments: `<T, U>`.
    ///
    /// One builder for every type-argument position — there is no "wrapping" variant, because
    /// there is nothing for one to do differently. A single **hugging** argument inlines
    /// atomically ([`Self::type_arg_hugs`]); everything else — a non-hugging single argument
    /// and every multi-argument list alike — gets the group, so the `<…>` breaks at print
    /// width independently of its parent:
    ///
    /// ```text
    /// Array<{        Promise<
    ///     prop: T;       A & B & C
    /// }>             >
    /// ```
    ///
    /// Inlining a hugging argument matters beyond layout taste: the group/softlines it avoids
    /// would create Break-mode Line nodes in `fits()` rest_commands, causing upstream groups
    /// (like arrays in Fluid assignment layout) to incorrectly appear to "fit" — Line in Break
    /// mode returns true from `fits()`, short-circuiting the width check.
    pub(crate) fn build_type_arguments_doc(
        &self,
        args: &internal::TSTypeParameterInstantiation<'_>,
    ) -> DocId {
        let d = self.d();
        if args.params.is_empty() {
            return d.text("<>");
        }

        // One window search over the `<…>`, threaded into everything below it. **On-page**:
        // this is the builder's zero-comment fast gate, so it short-circuits the layout gates
        // below (`type_arguments_force_expansion` above all) — an emit-keyed answer would make
        // every one of them blind to an owned comment. Sound today either way (ownership is
        // set only in expression position, and a `<…>` window holds types), but the question
        // asked here is "does a comment occupy the page", so that is the axis it asks.
        let has_comments = self.has_comments_on_page_between(args.span.start, args.span.end);

        if self.type_arguments_force_expansion(args, has_comments) {
            return self.build_type_arguments_doc_with_line_comments(args);
        }

        // A single argument inlines only when it hugs; a non-hugging one (an intersection, a
        // function type, a conditional) falls through to the group below, which is what gives
        // the `<…>` a break point of its own.
        if args.params.len() == 1 && self.type_arg_hugs(&args.params[0]) {
            return self.build_single_type_arg_inline(args, has_comments);
        }

        self.build_type_arguments_group_doc(args, has_comments)
    }

    /// The width-decided `<…>` tail, shared by **both** type-argument families — the
    /// type-position list above and the call/`new` instantiation
    /// (`build_type_parameter_instantiation_doc`). Matches Prettier's
    /// `group([<, indent([softline, join([",", line], args)]), softline, >])` via the
    /// shared angle-list core; each argument renders in multi-arg (hugging) mode — also
    /// for a non-hugging single argument, preserving the pre-merge behavior of the
    /// type-position path.
    ///
    /// The two families differ **only** in which single arguments they hug before
    /// reaching here (the instantiation's curly-hug arm is a documented divergence);
    /// everything past that decision is one body, so neither can drift from the other.
    pub(in crate::printer) fn build_type_arguments_group_doc(
        &self,
        args: &internal::TSTypeParameterInstantiation<'_>,
        has_comments: bool,
    ) -> DocId {
        let d = self.d();
        d.group(self.build_angle_list_doc(
            args.span,
            args.params.len(),
            |i| args.params[i].span(),
            |i, frozen| {
                if frozen {
                    self.build_frozen_list_member_doc(&args.params[i])
                } else {
                    self.build_type_arg_doc(&args.params[i], true)
                }
            },
            |i| self.frozen_list_member_multiline(&args.params[i]),
            has_comments,
        ))
    }

    /// Build doc for type arguments with expanding comments (line or own-line block),
    /// which force multiline because they can't appear inline. Shared by **both**
    /// type-argument families, like the width-decided tail above — a call/`new`
    /// argument *is* a type argument, so both route here rather than keeping twin
    /// bodies in step by hand.
    ///
    /// Spans and docs both come from `leading_paren_unwrapped`, so this builder's gap
    /// emitters and the item docs agree on where each argument starts — see that
    /// function for why a leading-only paren shell must not reach here.
    pub(in crate::printer) fn build_type_arguments_doc_with_line_comments(
        &self,
        args: &internal::TSTypeParameterInstantiation<'_>,
    ) -> DocId {
        let is_multi = args.params.len() > 1;
        let param_at = |i: usize| self.leading_paren_unwrapped(&args.params[i]);
        // Rule A, asked on each argument's OWN span. It decides whether the printed span
        // may widen over a leading-edge shell, so it is resolved here rather than inside
        // the list core — reading it from the widened span would let a frozen argument
        // both slice its shell verbatim and hand the same run to this list's gap emitter
        // ([`Printer::leading_edge_claim_and_start`]).
        let raw_span = |i: usize| param_at(i).span();
        let frozen_at = |i: usize| self.list_item_frozen(args.span.start + 1, &raw_span, i);
        self.build_angle_list_with_line_comments(
            args.span,
            args.params.len(),
            frozen_at,
            |i| self.list_item_printed_span(frozen_at(i), param_at(i)),
            |i, frozen| {
                let param = param_at(i);
                if frozen {
                    return self.build_frozen_list_member_doc(param);
                }
                // The list emits this argument's leading-edge shell run (its span above
                // starts past it), so the shell declines its own copy.
                let claim = self.leading_edge_shell_claim(param);
                self.with_claimed_shell_leading_run(claim, || {
                    self.build_type_arg_doc(param, is_multi)
                })
            },
        )
    }
}
