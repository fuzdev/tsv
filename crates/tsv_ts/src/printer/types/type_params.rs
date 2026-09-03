// Type parameter printing for TypeScript
//
// Handles:
// - Type parameter declarations: `<T, U extends V = W>`
// - Type parameter instantiation (type arguments): `<T, U>`

use super::helpers::{is_simple_type_arg, unwrap_parenthesized};
use super::{BlankRule, CommentFilter, CommentSpacing, KeywordValueHead, Printer, TrailingBlock};
use crate::ast::internal::{
    self, TSType, TSTypeParameter, TSTypeParameterDeclaration, TSTypeParameterModifier,
};
use crate::printer::layout::{bracketed_list_body, fluid_after_operator};
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::GroupId;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::find_char_skipping_comments;

impl<'a> Printer<'a> {
    //
    // Type Parameter Declarations
    //

    /// Build doc for type parameter declaration: `<T, U extends V = W>`
    /// Non-wrapping version - always inline, unless expanding comments (or a
    /// multi-line frozen parameter) force multiline
    pub(in crate::printer) fn build_type_parameter_declaration_doc(
        &self,
        decl: &TSTypeParameterDeclaration<'_>,
    ) -> DocId {
        if self.has_expanding_comments_in_type_param_declaration(decl)
            || self.has_frozen_multiline_type_param(decl)
        {
            return self.build_type_parameter_declaration_doc_with_line_comments(decl);
        }

        let d = self.d();
        let param_docs = self.build_type_parameter_docs_with_comments(decl);
        d.concat(&[d.text("<"), d.join(param_docs, ", "), d.text(">")])
    }

    /// Whether any parameter is Rule-A-frozen with a multi-line slice — the
    /// always-inline path's analog of `build_angle_list_doc`'s
    /// `frozen_forces_break`: `has_expanding_comments_in_type_param_declaration`
    /// keys on comment SHAPE, which a MULTILINE-spelled alone-on-line directive
    /// doesn't trip (`has_own_line_block_comments_in_bracket_list` skips multiline
    /// blocks), yet the frozen member's own span can still cross lines — the `<…>`
    /// must expand (a `verbatim_source_span` is `will_break`-opaque, so the forcing
    /// is explicit).
    fn has_frozen_multiline_type_param(&self, decl: &TSTypeParameterDeclaration<'_>) -> bool {
        self.has_format_ignore
            && (0..decl.params.len()).any(|i| {
                self.list_item_frozen(decl.span.start + 1, &|j| decl.params[j].span, i)
                    && !self.is_same_line(decl.params[i].span.start, decl.params[i].span.end)
            })
    }

    /// Build doc for type parameter declaration with wrapping support
    /// When the group breaks, each param goes on its own line with trailing comma
    pub(crate) fn build_type_parameter_declaration_doc_wrapping(
        &self,
        decl: &TSTypeParameterDeclaration<'_>,
    ) -> DocId {
        // A test call's callback inlines its type parameters at any width — the
        // function-expression twin of the peek in `build_type_params_doc_for_arrow`
        // (prettier's `isParameterInTestCall` asks `isTestCall` of the function's
        // parent for both kinds). PEEKED, not consumed: the value parameters are the
        // ones that spend the flag. Delegating to the always-inline builder keeps its
        // gates, so an expanding comment (or a frozen multi-line param) still opens
        // the list. See the field doc on `Printer::test_call_flat_params`.
        if self.test_call_flat_params.get() {
            return self.build_type_parameter_declaration_doc(decl);
        }
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

        // On-page: a layout builder's zero-comment fast gate (same axis note as
        // `build_type_arguments_doc`).
        let has_comments = self.has_comments_on_page_between(decl.span.start, decl.span.end);
        self.build_angle_list_doc(
            decl.span,
            decl.params.len(),
            |i| decl.params[i].span,
            |i, frozen| self.build_type_parameter_item_doc(&decl.params[i], frozen),
            |i| !self.is_same_line(decl.params[i].span.start, decl.params[i].span.end),
            has_comments,
        )
    }

    /// A type-parameter list item: the frozen verbatim slice under a Rule A
    /// format-ignore freeze (a `TSTypeParameter` has no paren shell, so the slice is
    /// its whole span), the ordinary parameter doc otherwise.
    fn build_type_parameter_item_doc(&self, param: &TSTypeParameter<'_>, frozen: bool) -> DocId {
        if frozen {
            self.raw_source_range(param.span.start, param.span.end)
        } else {
            self.build_type_parameter_doc(param)
        }
    }

    /// Build doc for type parameter declaration with expanding comments.
    ///
    pub(in crate::printer) fn build_type_parameter_declaration_doc_with_line_comments(
        &self,
        decl: &TSTypeParameterDeclaration<'_>,
    ) -> DocId {
        // Type parameters are declarations, never types, so no leading-edge paren shell
        // can widen an item span here — Rule A reads the params' own spans directly.
        let item_span = |i: usize| decl.params[i].span;
        self.build_angle_list_with_line_comments(
            decl.span,
            decl.params.len(),
            |i| self.list_item_frozen(decl.span.start + 1, &item_span, i),
            item_span,
            |i, frozen| self.build_type_parameter_item_doc(&decl.params[i], frozen),
        )
    }

    /// Check for expanding comments in type param declarations: line comments,
    /// own-line block comments, or line comments inside param spans (e.g.,
    /// `T extends // comment\n  A`). Used by both wrapping and non-wrapping paths.
    pub(in crate::printer) fn has_expanding_comments_in_type_param_declaration(
        &self,
        decl: &TSTypeParameterDeclaration<'_>,
    ) -> bool {
        // Zero-comment window gate: one binary search over the whole `<…>` span. The
        // shared clauses and the per-param one below are all bounded within
        // `[decl.span.start, decl.span.end]`, so with nothing on the page all are
        // provably false. Skips them on the common comment-free `<T, U>`.
        if !self.has_comments_on_page_between(decl.span.start, decl.span.end) {
            return false;
        }
        self.has_expanding_comments_in_bracket_list(decl.span, decl.params, |p| p.span)
            || decl
                .params
                .iter()
                // A line comment or multiline block in a param's constraint/default gap
                // (`<T extends⏎// c⏎U>`) forces the whole `<…>` to expand, so the hang
                // renders inside the broken list; a single-line block comment collapses
                // inline and keeps `<…>` collapsed. The type parameter's own question,
                // which the shared list clauses don't ask.
                .any(|p| self.comments_force_own_line_between(p.span.start, p.span.end))
    }

    /// Build enriched param docs with surrounding block comments from the declaration.
    /// Comments outside param spans (e.g., `</* c */ T /* c */>`) are captured here.
    /// Only inline block comments can reach this path (the caller routes line and
    /// own-line comments to the expansion builder first), so the last param's
    /// trailing gap is one Leading-spaced range up to `>` — a comment past a source
    /// comma stays after where the comma was (no trailing comma emitted;
    /// trailingComma: 'none'), the same shape as `build_angle_list_doc`'s last-item
    /// arm.
    fn build_type_parameter_docs_with_comments(
        &self,
        decl: &TSTypeParameterDeclaration<'_>,
    ) -> DocBuf {
        let d = self.d();
        let mut prev_end = decl.span.start + 1; // After `<`
        decl.params
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
                // Rule A: an alone-on-line directive in the gap freezes this
                // parameter (the always-inline path; most such spellings route to
                // the expansion builder via
                // `has_expanding_comments_in_type_param_declaration` first).
                let frozen =
                    self.list_item_frozen(decl.span.start + 1, &|j| decl.params[j].span, i);
                parts.push(self.build_type_parameter_item_doc(param, frozen));

                if i + 1 < decl.params.len() {
                    // Find comma between this param and next
                    let next_start = decl.params[i + 1].span.start;
                    let comma_pos = self.find_list_comma(param.span.end, next_start);
                    // Trailing block comments: only the run that follows content on its
                    // line: the rest leads the next param, so the leading scan resumes at
                    // the run's end rather than past the comma
                    // (`Printer::inline_trailing_run_end`).
                    let run_end = self.inline_trailing_run_end(param.span.end, comma_pos);
                    parts.push(self.build_comments_between_filtered(
                        param.span.end,
                        run_end,
                        CommentSpacing::Leading,
                        CommentFilter::BlockOnly,
                    ));
                    prev_end = run_end;
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
        let mut parts = d.pooled_docbuf();

        // One window search over the parameter gates every comment query below. All of
        // them — the `<`→name gap, the name→`extends` gap, the pre-`=` gap, the trailing
        // gap, and the keyword→value gaps `append_keyword_value` inspects — are bounded
        // inside `param.span`, and a comment only counts when it lies fully inside the
        // queried range. So a comment-free parameter provably has none in any of them:
        // the searches are skipped, no `empty()` child is pushed, and the `extends` /
        // `=` byte scans never run. Those scans exist only to bound the comment ranges —
        // both keywords are re-emitted as static text — which is why a comment-free
        // parameter can pass `None` for the range. Byte-identical; `<T>` is on every
        // generic function, class, interface and alias.
        let has_comments = self.has_comments_on_page_between(param.span.start, param.span.end);

        // Modifiers print in the CANONICAL order — `const in out` — however the source
        // spells them, matching prettier (`<in const T>` → `<const in T>`). The source
        // order survives on the wire only; see `write_type_parameter`.
        if param.modifiers.contains(TSTypeParameterModifier::Const) {
            parts.push(d.text("const "));
        }
        if param.modifiers.contains(TSTypeParameterModifier::In) {
            parts.push(d.text("in "));
        }
        if param.modifiers.contains(TSTypeParameterModifier::Out) {
            parts.push(d.text("out "));
        }

        // Comments before name: </* c */ T>
        if has_comments
            && let Some(leading) = self.build_inline_comments_between_doc_trailing_space_opt(
                param.span.start,
                param.name.span.start,
            )
        {
            parts.push(leading);
        }

        parts.push(self.identifier_name_doc(&param.name));

        // Track where we are for finding comments after the name
        let mut prev_end = param.name.span.end;

        if let Some(constraint) = &param.constraint {
            // The shared keyword→value head. Resolving it needs the keyword's position,
            // so the `extends` byte scan comes first — and only on the comment-bearing
            // path: a comment-free parameter takes the scan-free head, which is why the
            // keyword position is looked up here rather than unconditionally.
            //
            // A `TSTypeParameter` constraint is always spelled `extends` — mapped-type
            // `[K in T]` keys use `in`, but those take a separate
            // `TSMappedTypeParameter`/`build_mapped_type_doc` path and never reach here,
            // so the keyword is guaranteed present. The head's paren-strip arm treats a
            // leading line comment inside `(// leading⏎ T)` — and the double-nested
            // `((// leading⏎ T))` — as if it sat between `extends` and the constraint,
            // so it forces the indent-and-break layout (matching prettier's paren
            // stripping).
            let mut line_gap = None;
            let head = if has_comments {
                #[expect(clippy::expect_used)] // extends always present for a constraint
                let extends_pos = self
                    .find_keyword_in_range(prev_end, constraint.span().start, "extends")
                    .expect("extends keyword must exist when constraint is present");

                line_gap = self.route_pre_keyword_gap(&mut parts, prev_end, extends_pos);
                self.keyword_value_head(extends_pos + "extends".len() as u32, constraint)
            } else {
                KeywordValueHead::without_gap(constraint)
            };

            self.push_keyword_value_or_continuation(
                &mut parts,
                line_gap,
                "extends",
                " extends",
                &head,
                GroupId::TypeParameterConstraint,
            );
            prev_end = constraint.span().end;
        }

        if let Some(default) = &param.default {
            // The `=` mirror of the constraint above, same head and same scan gate:
            // `<T = (// c⏎ U)>` (and the double-nested form) strips to the same hang as
            // bare `<T = // c⏎ U>`. A mixed / trailing shell hoists losslessly too — the
            // trailing comment is reattached in `append_keyword_value` via
            // `build_hang_value_doc`.
            let mut line_gap = None;
            let head = if has_comments {
                #[expect(clippy::expect_used)] // = must exist when a default is present
                let eq_pos = find_char_skipping_comments(
                    self.source.as_bytes(),
                    prev_end as usize,
                    default.span().start as usize,
                    b'=',
                )
                .expect("= must exist when default is present");

                line_gap = self.route_pre_keyword_gap(&mut parts, prev_end, eq_pos as u32);
                self.keyword_value_head((eq_pos + 1) as u32, default)
            } else {
                KeywordValueHead::without_gap(default)
            };

            self.push_keyword_value_or_continuation(
                &mut parts,
                line_gap,
                "=",
                " =",
                &head,
                GroupId::TypeParameterDefault,
            );
            prev_end = default.span().end;
        }

        // Trailing comments after last part: <T /* c */> or <T extends A /* c */>
        if has_comments
            && let Some(trailing) = self.build_comments_between_filtered_opt(
                prev_end,
                param.span.end,
                CommentSpacing::Leading,
                CommentFilter::All,
            )
        {
            parts.push(trailing);
        }

        d.concat(&parts)
    }

    /// Push ` <keyword>` and its value ([`Self::append_keyword_value`]) — or, when
    /// [`Self::route_pre_keyword_gap`] deferred a line-comment gap, the gap's comment
    /// run with the whole `<keyword> <value>` tail dropped to a continuation line one
    /// indent level in (the uniform forced-continuation indent,
    /// `build_continuation_indent`). `keyword`/`spaced_keyword` are the same literal
    /// with and without the leading space, so the inline arm keeps its single text
    /// node.
    fn push_keyword_value_or_continuation(
        &self,
        parts: &mut DocBuf,
        line_gap: Option<(u32, u32)>,
        keyword: &'static str,
        spaced_keyword: &'static str,
        head: &KeywordValueHead<'_>,
        group_id: GroupId,
    ) {
        let d = self.d();
        if let Some((gap_start, keyword_pos)) = line_gap {
            let mut tail: DocBuf = smallvec![d.text(keyword)];
            self.append_keyword_value(&mut tail, head, group_id);
            parts.push(self.build_continuation_indent(gap_start, keyword_pos, d.concat(&tail)));
        } else {
            parts.push(d.text(spaced_keyword));
            self.append_keyword_value(parts, head, group_id);
        }
    }

    /// Emit the keyword→value gap and the value doc — the shared routing of the
    /// frozen and clarity-paren arms in [`Self::append_keyword_value`]: an
    /// own-line-forcing comment (line, or own-line multiline block) hangs
    /// `value_doc` on its own indented line
    /// ([`Self::append_keyword_value_line_comments`]); otherwise inline block
    /// comments trail the keyword and the value follows on the line.
    fn push_hang_or_inline_value(
        &self,
        parts: &mut DocBuf,
        keyword_end: u32,
        value_start: u32,
        value_doc: DocId,
    ) {
        let d = self.d();
        if self.comments_force_own_line_between(keyword_end, value_start) {
            self.append_keyword_value_line_comments(parts, keyword_end, value_start, value_doc);
            return;
        }
        if let Some(comments) = self.build_comments_between_filtered_opt(
            keyword_end,
            value_start,
            CommentSpacing::Leading,
            CommentFilter::All,
        ) {
            parts.push(comments);
        }
        parts.push(d.text(" "));
        parts.push(value_doc);
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
    ///
    /// `head` is the shared keyword→value resolution ([`Printer::keyword_value_head`]):
    /// the gap window both the gates and the emitters use, the freeze verdict, and the
    /// (possibly paren-stripped) value plus its pre-strip shell — so this builder can't
    /// gate on one window while claiming another. Its `gap_start` is `None` when the
    /// caller proved the parameter comment-free and never located the keyword: no gap,
    /// no comment arms, no freeze — straight to the width-decided tail.
    ///
    /// This site is the head protocol PLUS two rules of its own, which is why it keeps
    /// its own builder rather than calling `build_keyword_value_doc`: it strips a
    /// redundant comment-free paren off the value (the cast / annotation heads do not),
    /// and a conditional constraint keeps clarity parens.
    fn append_keyword_value(
        &self,
        parts: &mut DocBuf,
        head: &KeywordValueHead<'_>,
        group_id: GroupId,
    ) {
        let d = self.d();
        // An alone-on-line format-ignore directive in the keyword→value gap freezes a
        // non-composite value verbatim (the head's `frozen`; a union/intersection value
        // declines and freezes via its own leading-run walk). The head checked the
        // UNWIDENED gap — its window ends at the shell's own span start under a freeze,
        // so an in-shell directive stays on the ordinary paths below. The directive
        // keeps its own line (`append_keyword_value_line_comments` already preserves
        // own-line comments). A conditional constraint keeps its required parens
        // under the freeze (the same clarity/`infer` rule as the unfrozen arm below).
        if let Some(keyword_end) = head.gap_start
            && head.frozen
        {
            let member_parens: fn(&TSType<'_>) -> bool =
                if group_id == GroupId::TypeParameterConstraint {
                    |t| matches!(t, TSType::Conditional(_))
                } else {
                    |_| false
                };
            let frozen_doc = self.build_frozen_head_doc(head.child, member_parens);
            // Under a freeze the head's window already ends at the child's own start.
            self.push_hang_or_inline_value(parts, keyword_end, head.value_start, frozen_doc);
            return;
        }
        // Strip redundant comment-free parens so `(A | B)` / `(A & B)` constraints
        // and defaults get the bare hanging layout (prettier strips them too).
        let value_type = self.unwrap_redundant_parens(head.value_type);
        // A *conditional* type used as a constraint keeps its parens: prettier keeps
        // them for clarity, and for an `infer`'s conditional constraint they're
        // outright required (the enclosing conditional's `? :` rebinds without them —
        // prettier drops them there, producing unparseable output, a documented
        // divergence). The `=` default position strips them.
        //
        // Asked through the comment-BLIND [`unwrap_parenthesized`], because whether this
        // constraint needs its parens is a fact about the GRAMMAR — a comment the author
        // wrote in a shell around it decides nothing. `unwrap_redundant_parens` stops at
        // a commented shell, so a comment dropped the constraint out of this arm and
        // printed it bare, losing the very parens the `infer` case requires.
        //
        // A shell the trailing-run rule RETAINS is left to its own emitter
        // (`build_parenthesized_type_unwrap_doc`), which owns the retain-vs-strip question
        // for a trailing line comment: stripping here and lifting the `//` out would give
        // that one gap a third answer, neither prettier's nor tsv's own.
        //
        // ⚠️ And the shell always DOES reach it here, because the paren-strip hang seam
        // ([`Printer::keyword_value_stripped_paren_hang`]) declines exactly the shells this
        // rule retains. Asking without that guarantee was a trap: the hang strips a shell
        // whose leading gap holds a line comment, so one carrying BOTH
        // (`(// c⏎ A extends B ? C : D // t)`) satisfied the retain rule while already
        // being gone, and deferring to an emitter that never runs printed the constraint
        // bare — for the `infer` case not a layout difference but output the canonical
        // parser REJECTS (the `?` rebinds).
        let conditional_constraint = matches!(
            unwrap_parenthesized(head.value_type),
            TSType::Conditional(_)
        ) && group_id == GroupId::TypeParameterConstraint
            && !self.paren_retains_for_trailing_run(head.child);
        if conditional_constraint {
            // The author's own shell (if any) is stripped, so this arm owns its three
            // interior regions: the leading gap widens into the emitter's window below
            // (`value_start`), the inner conditional prints between the re-emitted
            // parens, and the trailing gap is lifted onto the end by the shared seam —
            // the same partition `build_hang_value_doc` performs for every other arm.
            // Building the pair by hand without that lift DROPPED the trailing comment
            // ([`comments.md`](../../../../docs/comments.md) hazard 1).
            let inner = unwrap_parenthesized(head.value_type);
            // The re-emitted pair is a THIRD emitter for a leading-edge shell's run
            // (`K extends (⏎// c⏎L) extends M ? N : O`): the run belongs to the `extends`
            // gap, which the emitter below owns, so the shell declines its copy for the
            // duration of this build and the pair closes around the bare conditional.
            let paren_doc = self.with_claimed_shell_leading_run(head.claimed_shell, || {
                d.concat(&[d.text("("), self.build_type_doc(inner), d.text(")")])
            });
            // The clarity parens are re-emitted around the conditional, so the
            // keyword→value gap follows the same protocol as the unparenthesized
            // arms below: a line comment (or own-line multiline block) hangs the
            // parenthesized value on its own indented line, an inline block trails
            // the keyword before the `(`.
            //
            // The window ends where the conditional's own printed content begins: the
            // inner's start normally — which is what covers a leading BLOCK in the
            // author's own shell, a comment this arm's hand-built pair bypasses the
            // shell's emitter for — or, where the seam claimed a leading-edge shell one
            // link inside the check type, at that claim's end.
            let value_start = head
                .claimed_shell
                .map_or_else(|| inner.span().start, |shell| shell.end);
            let mut hung: DocBuf = smallvec![];
            if let Some(keyword_end) = head.gap_start {
                self.push_hang_or_inline_value(&mut hung, keyword_end, value_start, paren_doc);
            } else {
                hung.push(d.text(" "));
                hung.push(paren_doc);
            }
            // The trailing gap holds only BLOCKS here — a `//` in it retains the shell,
            // which never reaches this arm
            // ([`Printer::keyword_value_stripped_paren_hang`] declines the hang for it)
            // — so the lift is inline and takes no break of its own.
            parts.push(self.with_stripped_paren_trailing(
                d.concat(&hung),
                head.child,
                inner,
                TrailingBlock::Inline,
            ));
            return;
        }
        if let Some(keyword_end) = head.gap_start {
            if self.comments_force_own_line_between(keyword_end, head.value_start) {
                // A line comment or multiline block after the keyword hangs the bound type
                // on its own line (and expands the `<…>` via the gate in
                // `has_expanding_comments_in_type_param_declaration`). A
                // single-line block comment (own-line, trailing, or glued) collapses inline
                // and keeps `<…>` collapsed (the fall-through below). Type position: a
                // trailing block lifted from a stripped shell trails the value inline
                // before the `,`/`>`.
                let value_doc = self.with_claimed_shell_leading_run(head.claimed_shell, || {
                    self.build_hang_value_doc(head.child, value_type, TrailingBlock::Inline)
                });
                self.append_keyword_value_line_comments(
                    parts,
                    keyword_end,
                    head.value_start,
                    value_doc,
                );
                return;
            }
            if let Some(comments) = self.build_comments_between_filtered_opt(
                keyword_end,
                head.value_start,
                CommentSpacing::Leading,
                CommentFilter::All,
            ) {
                parts.push(comments);
                // Block comment present: keep the value inline after it.
                parts.push(d.text(" "));
                parts.push(self.build_type_doc(value_type));
                return;
            }
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

        // One window search over the `<…>`, threaded into everything below it.
        let has_comments = self.has_comments_on_page_between(inst.span.start, inst.span.end);

        // Line comments (anywhere, including a leading `foo<// c\n A>(x)` — which
        // would otherwise fall through to the block-comment-only group path below and
        // be dropped) or own-line block comments force the multiline layout. Shared
        // predicate with the type-position builder.
        if self.type_arguments_force_expansion(inst, has_comments) {
            return self.build_type_arguments_doc_with_line_comments(inst);
        }

        // Special case: a single curly-brace type argument hugs the opening
        // bracket. tsv keeps `<{` together for a single object/mapped type even
        // when it carries an interior comment; the type carries its own group so
        // it still breaks block-style when too wide. (This is the same layout the
        // type-reference type-argument path uses. Prettier instead breaks the
        // `<…>` onto its own lines for a comment-bearing mapped/empty type — a
        // deliberate divergence; see docs/conformance_prettier_ts_comments.md.)
        if inst.params.len() == 1
            && let Some(type_doc) = self.try_build_hugging_curly_type_doc(&inst.params[0])
        {
            // The `<`→arg / arg→`>` gaps may hold inline block comments (a glued
            // format-ignore directive included) — the shared single-arg emission
            // preserves them (and applies the freeze) instead of dropping them.
            return self.build_single_type_arg_inline_with(inst, has_comments, type_doc);
        }

        // A single *simple* or *hugged-union* type argument inlines atomically: no
        // group, no softlines. Simple = keyword, literal, `this`, or a bare type
        // reference (`is_simple_type_arg`); hugged union = `{…} | null` / `null | {…}`
        // (`union_type_arg_hug_shape`), whose object member carries its own group and
        // breaks block-style inside the hugged `<…>` rather than breaking the `<…>` onto
        // its own lines. Matches Prettier's `shouldInline`/`shouldHugType` and tsv's own
        // type-position builder (`build_type_arguments_doc`), via the shared
        // predicates. Without it the fall-through group below gives the argument a
        // softline break point, so an overflowing call head (`callee<Ref>(`) breaks the
        // `<Ref>` instead of the arguments (and, as an assignment RHS, keeps the RHS on
        // the `=` line rather than breaking after `=`). Comment-bearing single arguments
        // are already routed to the multiline path above, so only inline block comments
        // remain — the shared `build_single_type_arg_inline` preserves them. (The single
        // brace-delimited object/mapped type is handled by the curly-hug case above.)
        if inst.params.len() == 1
            && (is_simple_type_arg(&inst.params[0])
                || self.type_arg_union_prints_hugged(&inst.params[0]))
        {
            return self.build_single_type_arg_inline(inst, has_comments);
        }

        // Multi-argument (or non-hugging single) tail: the type-argument families' shared
        // width-decided core (`build_type_arguments_group_doc`), byte-for-byte the body the
        // type-position list runs. The doc printer's look-ahead (fits_with_lookahead)
        // handles whether to break based on what follows the type params.
        //
        // Routing here — rather than rendering items with the generic `build_type_doc` —
        // is also what strips a redundant paren shell whose comments are trailing
        // (`f<(a | b // c)>(y)`): `build_type_doc` retains such a shell, because a
        // deferred run must not escape the construct it was written in, but inside a
        // `<…>` the enclosing list is itself a retained bracketed construct that the run
        // flushes safely inside of (see `build_type_doc_for_type_arg`).
        self.build_type_arguments_group_doc(inst, has_comments)
    }

    /// The shared width-decided angle-list body: `<` + softline-indented,
    /// comma-separated items with their inline block comments + `>` — the one core
    /// behind all three `<…>` families (type-parameter declarations, call/`new`
    /// instantiations, and type-position type arguments), so none hand-rolls
    /// this layout independently. Returns the UNGROUPED concat: the
    /// declaration's callers control the group themselves (e.g. the interface
    /// header); the type-argument callers wrap it in a group.
    ///
    /// `item_span`/`item_doc` select the family's item type and per-item printer;
    /// `item_doc` receives the item's Rule A `frozen` flag (an alone-on-line
    /// format-ignore directive in the item's gap — most spellings route to the
    /// expansion builder below before this runs) and emits the frozen verbatim
    /// slice when set.
    /// `frozen_forces_break` is asked only for frozen items: `true` (a multi-line
    /// frozen slice) forces the broken layout — a `verbatim_source_span` is
    /// `will_break`-opaque, so the forcing is explicit, and the emitted hardlines
    /// propagate the break to the caller's group.
    /// `has_comments` is the caller's whole-`<…>` window answer (on-page — a layout
    /// builder's zero-comment fast gate): `false` proves every gap below is
    /// comment-free, so neither the comment searches nor the `find_list_comma` byte
    /// scans that bound them run — a comment-free `<T, U>` builds with no source
    /// scanning at all (the printed `,` is static text). Line comments and blocks the
    /// author ISOLATED on a line never reach here (each family's expansion predicate
    /// routes them to [`Self::build_angle_list_with_line_comments`] first) — but a block
    /// merely written *below* the previous item does, and takes the soft `line` this
    /// list's own group then decides. That is why the leading run goes through the shared
    /// emitter rather than a spacing enum: the separator is not a property of the layout.
    pub(in crate::printer) fn build_angle_list_doc(
        &self,
        span: Span,
        count: usize,
        item_span: impl Fn(usize) -> Span,
        item_doc: impl Fn(usize, bool) -> DocId,
        frozen_forces_break: impl Fn(usize) -> bool,
        has_comments: bool,
    ) -> DocId {
        let d = self.d();
        let mut inner_parts = DocBuf::new();
        let mut prev_end = span.start + 1; // After the opening `<`
        let mut force_break = false;

        for i in 0..count {
            if i > 0 {
                inner_parts.push(d.text(","));
                inner_parts.push(d.line());
            }

            // Leading comments (after the previous comma or `<`), through the shared
            // emitter so each separator is prettier's `printLeadingComment`. The soft
            // `line` is the one that matters here: a glued run the author gave its own
            // line (`<A,⏎/* c1 */ /* c2 */⏎B>`) collapses onto the item when this list
            // fits and breaks above it when it doesn't — one fixed point for both
            // authorings. A hardcoded space would reach the second, and an expansion gate
            // ([`Printer::block_comment_owns_its_line`]) routing every own-line run to
            // the all-hardline builder would hide that.
            if has_comments {
                inner_parts.extend(self.build_leading_comments_multiline(
                    prev_end,
                    item_span(i).start,
                    None,
                ));
            }

            // Rule A: an alone-on-line directive in this item's gap freezes the item
            // (the directive itself was just emitted by the gap emitter above).
            let frozen = has_comments && self.list_item_frozen(span.start + 1, &item_span, i);
            if frozen && frozen_forces_break(i) {
                force_break = true;
            }

            inner_parts.push(item_doc(i, frozen));

            if has_comments {
                let item_end = item_span(i).end;
                if i + 1 < count {
                    // Find comma between this item and the next
                    let next_start = item_span(i + 1).start;
                    let comma_pos = self.find_list_comma(item_end, next_start);
                    // Trailing block comments: only the run that follows content on its
                    // line: the rest leads the next item, so the leading scan resumes at
                    // the run's end rather than past the comma
                    // (`Printer::inline_trailing_run_end`).
                    let run_end = self.inline_trailing_run_end(item_end, comma_pos);
                    if let Some(trailing) = self.build_comments_between_filtered_opt(
                        item_end,
                        run_end,
                        CommentSpacing::Leading,
                        CommentFilter::BlockOnly,
                    ) {
                        inner_parts.push(trailing);
                    }
                    prev_end = run_end;
                } else if let Some(trailing) = self.build_comments_between_filtered_opt(
                    // Last item: trailing comments before `>` (including past a source
                    // comma — trailingComma 'none' emits none, so the comment stays
                    // after where it was rather than relocating before it).
                    item_end,
                    span.end - 1,
                    CommentSpacing::Leading,
                    CommentFilter::BlockOnly,
                ) {
                    inner_parts.push(trailing);
                }
            }
        }

        bracketed_list_body(
            d,
            d.text("<"),
            d.text(">"),
            d.concat(&inner_parts),
            force_break,
        )
    }

    /// Render a type-argument list `<…>` that breaks onto multiple lines because it
    /// carries comments — the shared body behind the angle-list families: type arguments
    /// in both type and call/`new`-expression position (which share one caller,
    /// [`Self::build_type_arguments_doc_with_line_comments`]) and type-parameter
    /// declarations ([`Self::build_type_parameter_declaration_doc_with_line_comments`]).
    /// `item_span`/`item_doc` select the family's item type and per-item printer.
    ///
    /// **Argument count changes nothing here.** A single argument is the N=1 form of
    /// the list and takes the list's layout — delimiter-line comment on the `<` line,
    /// body indented, `>` dangling — exactly as a multi-argument list does. A
    /// single-argument leading *line* comment does NOT hug `<`/`>`
    /// (`foo<// c⏎A>`); that hug would be the one shape in the family answering the
    /// layout question by count. The divergence is the
    /// delimiter-line placement itself, which is the multi-argument entry's
    /// (`type_args_open_angle_comment`) — prettier drops the comment to its own line
    /// at every count. See `type_position_parens_leading_line_comment`.
    pub(in crate::printer) fn build_angle_list_with_line_comments(
        &self,
        span: Span,
        count: usize,
        item_frozen: impl Fn(usize) -> bool,
        item_span: impl Fn(usize) -> Span,
        item_doc: impl Fn(usize, bool) -> DocId,
    ) -> DocId {
        let d = self.d();

        // Full multiline expansion (every count). A
        // comment trailing `<` on its own line is kept on the `<` line (divergence —
        // prettier relocates it to lead the first argument).
        let first_param_start = item_span(0).start;
        let (angle_line_prefix, delimiter_pull_pos) =
            self.delimiter_line_comment_prefix(span.start, first_param_start);

        let mut inner_parts = DocBuf::new();
        let mut prev_end = span.start + 1; // After the opening `<`

        for i in 0..count {
            let param_start = item_span(i).start;
            let param_end = item_span(i).end;
            let is_last = i == count - 1;

            // Leading comments (after previous comma or `<`). For the first arg,
            // drop comments pulled onto the `<` line (emitted as the angle-line
            // prefix below).
            let skip_delim = if i == 0 { delimiter_pull_pos } else { None };
            inner_parts.extend(self.build_leading_comments_multiline(
                prev_end,
                param_start,
                skip_delim,
            ));

            // Rule A: an alone-on-line directive in this item's gap freezes the
            // item; the directive itself was just emitted by the leading run above.
            // No must-break question here — this layout is already all-hardline.
            // The verdict is the CALLER's because `item_span` may already have widened
            // over a leading-edge paren shell, and the window a freeze is read on is the
            // item's own ([`Printer::leading_edge_claim_and_start`]).
            inner_parts.push(item_doc(i, item_frozen(i)));

            if !is_last {
                let next_start = item_span(i + 1).start;
                prev_end = self.emit_multiline_comma_with_comments(
                    &mut inner_parts,
                    param_end,
                    next_start,
                    BlankRule::None,
                );
            } else {
                // Last param: trailing comments before `>`
                let before_close = span.end - 1;
                inner_parts.extend(self.build_trailing_gap_comments(param_end, before_close));
                prev_end = before_close;
            }
        }

        self.build_delimited_doc(
            d.text("<"),
            angle_line_prefix,
            d.indent_hardline(d.concat(&inner_parts)),
            d.hardline(),
            d.text(">"),
        )
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
    /// type-argument path (`build_type_arguments_doc`).
    fn try_build_hugging_curly_type_doc(&self, ty: &TSType<'_>) -> Option<DocId> {
        match ty {
            // Object type literal: { a: number; b: string } or { /* comment */ }
            // Hug if it has members OR comments inside. Standard (not hugging) mode
            // so the object breaks block-style on width, the same as elsewhere.
            TSType::TypeLiteral(type_lit)
                if !type_lit.members.is_empty()
                    || self
                        .has_comments_to_emit_between(type_lit.span.start, type_lit.span.end) =>
            {
                Some(self.build_type_literal_doc(type_lit))
            }
            // Mapped type: { [K in keyof T]: V }
            TSType::Mapped(mapped) => Some(self.build_mapped_type_doc(mapped)),
            _ => None,
        }
    }
}
