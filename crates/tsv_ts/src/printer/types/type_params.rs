// Type parameter printing for TypeScript
//
// Handles:
// - Type parameter declarations: `<T, U extends V = W>`
// - Type parameter instantiation (type arguments): `<T, U>`

use super::helpers::is_simple_type_arg;
use super::{BlankRule, CommentFilter, CommentSpacing, Printer};
use crate::ast::internal::{self, TSType, TSTypeParameter, TSTypeParameterDeclaration};
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
    /// keys on comment SHAPE, which a glued directive never trips, yet the frozen
    /// member's own span can still cross lines — the `<…>` must expand (a
    /// `verbatim_source_span` is `will_break`-opaque, so the forcing is explicit).
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
    /// A single-param leading line comment does NOT hug here (unlike the
    /// type-argument builders): `function f<// c⏎T>()` fully expands.
    pub(in crate::printer) fn build_type_parameter_declaration_doc_with_line_comments(
        &self,
        decl: &TSTypeParameterDeclaration<'_>,
    ) -> DocId {
        self.build_angle_list_with_line_comments(
            decl.span,
            decl.params.len(),
            |i| decl.params[i].span,
            |i, frozen| self.build_type_parameter_item_doc(&decl.params[i], frozen),
            false,
        )
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
        // Zero-comment window gate: one binary search over the whole `<…>` span.
        // Every sub-query below is bounded within `[decl.span.start, decl.span.end]`
        // (the `<`→first-param gap, the delimited-list scan up to `end - 1`, and each
        // per-param constraint/default gap), so with no comment inside the `<…>` all
        // are provably false. Skips them on the common comment-free `<T, U>`.
        if !self.has_comments_to_emit_between(decl.span.start, decl.span.end) {
            return false;
        }
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
                // Rule A: a glued directive in the gap freezes this parameter (the
                // always-inline path; own-line spellings route to the expansion
                // builder via `has_expanding_comments_in_type_param_declaration`).
                let frozen =
                    self.list_item_frozen(decl.span.start + 1, &|j| decl.params[j].span, i);
                parts.push(self.build_type_parameter_item_doc(param, frozen));

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
            // If the constraint is `(// leading\n T)` — or the double-nested
            // `((// leading\n T))` — treat the leading line comment inside the parens
            // as if it were between `extends` and the constraint so it forces the
            // indent-and-break layout (matching prettier's paren stripping). The deep
            // window unwraps every redundant layer; a shallow one-level window missed
            // a comment nested one paren deeper (non-idempotent).
            let (value_search_end, value_type): (u32, &TSType<'_>) = if has_comments {
                self.keyword_value_stripped_paren_hang(constraint)
            } else {
                (constraint.span().start, constraint)
            };

            // Find `extends` keyword between name and constraint. A `TSTypeParameter`
            // constraint is always spelled `extends` — mapped-type `[K in T]` keys use
            // `in`, but those take a separate `TSMappedTypeParameter`/`build_mapped_type_doc`
            // path and never reach here, so the keyword is guaranteed present.
            let comment_range = has_comments.then(|| {
                #[allow(clippy::expect_used)] // extends always present for a constraint
                let extends_pos = self
                    .find_keyword_in_range(prev_end, constraint.span().start, "extends")
                    .expect("extends keyword must exist when constraint is present");
                let extends_end = extends_pos + "extends".len() as u32;

                // Comments between name and `extends`: <T /* c */ extends A>
                if let Some(pre) = self.build_comments_between_filtered_opt(
                    prev_end,
                    extends_pos,
                    CommentSpacing::Leading,
                    CommentFilter::All,
                ) {
                    parts.push(pre);
                }
                (extends_end, value_search_end)
            });

            parts.push(d.text(" extends"));
            self.append_keyword_value(
                &mut parts,
                comment_range,
                value_type,
                GroupId::TypeParameterConstraint,
                constraint,
            );
            prev_end = constraint.span().end;
        }

        if let Some(default) = &param.default {
            // Same deep-window paren handling as the constraint above: `<T = (// c\n U)>`
            // (and the double-nested form) strips to the same hang as bare `<T = // c\n U>`,
            // so substitute the unwrapped inner and widen the gap window to its start. A
            // mixed / trailing shell hoists losslessly too — the trailing comment is
            // reattached in `append_keyword_value` via `build_hang_value_doc`.
            let (value_search_end, value_type): (u32, &TSType<'_>) = if has_comments {
                self.keyword_value_stripped_paren_hang(default)
            } else {
                (default.span().start, default)
            };

            // Find `=` between previous end and default
            let comment_range = has_comments.then(|| {
                #[allow(clippy::expect_used)] // = must exist when a default is present
                let eq_pos = find_char_skipping_comments(
                    self.source.as_bytes(),
                    prev_end as usize,
                    default.span().start as usize,
                    b'=',
                )
                .expect("= must exist when default is present");
                let eq_end = (eq_pos + 1) as u32;

                // Comments before `=`: <T extends B /* c */ = C>
                if let Some(pre) = self.build_comments_between_filtered_opt(
                    prev_end,
                    eq_pos as u32,
                    CommentSpacing::Leading,
                    CommentFilter::All,
                ) {
                    parts.push(pre);
                }
                (eq_end, value_search_end)
            });

            parts.push(d.text(" ="));
            self.append_keyword_value(
                &mut parts,
                comment_range,
                value_type,
                GroupId::TypeParameterDefault,
                default,
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
    /// `comment_range` is the `(keyword_end, value_start)` gap to search, or `None`
    /// when the caller has already proven the whole parameter comment-free — the
    /// keyword's source position is needed for nothing else, so `None` also spares the
    /// caller the byte scan that would locate it.
    fn append_keyword_value(
        &self,
        parts: &mut DocBuf,
        comment_range: Option<(u32, u32)>,
        value_type: &TSType<'_>,
        group_id: GroupId,
        // The original (pre-seam) constraint/default node, so a trailing comment lifted
        // from a stripped redundant-paren shell (`extends (// c\n T /* t */)`) can be
        // reattached in the hang branch. Equal to `value_type` when no shell was stripped.
        shell: &TSType<'_>,
    ) {
        let d = self.d();
        // A format-ignore directive in the keyword→value gap (own-line or glued)
        // freezes a non-composite value verbatim (`single_child_frozen`; a
        // union/intersection value declines and freezes via its own leading-run walk).
        // Checked against the UNWIDENED gap — the window ends at the shell's own span
        // start, so an in-shell directive stays on the ordinary paths below. An
        // own-line directive keeps its own line (`append_keyword_value_line_comments`
        // already preserves own-line comments); a glued one stays inline before the
        // slice. A conditional constraint keeps its required parens under the freeze
        // (the same clarity/`infer` rule as the unfrozen arm below).
        if let Some((keyword_end, _)) = comment_range
            && self.single_child_frozen(keyword_end, shell)
        {
            let member_parens: fn(&TSType<'_>) -> bool =
                if group_id == GroupId::TypeParameterConstraint {
                    |t| matches!(t, TSType::Conditional(_))
                } else {
                    |_| false
                };
            // The single-child must-break: a multi-line frozen slice signals the
            // enclosing width-decided groups explicitly (`build_frozen_single_child_doc`'s
            // rationale — a verbatim slice is `will_break`-opaque).
            let frozen_doc = {
                let base = self.build_frozen_member_doc(shell, member_parens);
                if self.frozen_member_forces_break(true, shell, member_parens) {
                    d.concat(&[base, d.break_parent()])
                } else {
                    base
                }
            };
            let child_start = shell.span().start;
            if self.comments_force_own_line_between(keyword_end, child_start) {
                self.append_keyword_value_line_comments(
                    parts,
                    keyword_end,
                    child_start,
                    frozen_doc,
                );
            } else {
                if let Some(comments) = self.build_comments_between_filtered_opt(
                    keyword_end,
                    child_start,
                    CommentSpacing::Leading,
                    CommentFilter::All,
                ) {
                    parts.push(comments);
                }
                parts.push(d.text(" "));
                parts.push(frozen_doc);
            }
            return;
        }
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
            let mut inner: DocBuf = smallvec![d.text(" (")];
            if let Some((keyword_end, value_start)) = comment_range
                && let Some(comments) = self.build_comments_between_filtered_opt(
                    keyword_end,
                    value_start,
                    CommentSpacing::Leading,
                    CommentFilter::All,
                )
            {
                inner.push(comments);
            }
            inner.push(self.build_type_doc(value_type));
            inner.push(d.text(")"));
            parts.push(d.concat(&inner));
            return;
        }
        if let Some((keyword_end, value_start)) = comment_range {
            if self.comments_force_own_line_between(keyword_end, value_start) {
                // A line comment or multiline block after the keyword hangs the bound type
                // on its own line (and expands the `<…>` via the gate in
                // `has_expanding_comments_in_type_param_declaration`). A
                // single-line block comment (own-line, trailing, or glued) collapses inline
                // and keeps `<…>` collapsed (the fall-through below). Type position: a
                // trailing block lifted from a stripped shell trails the value inline
                // before the `,`/`>` (`defer = false`).
                let value_doc = self.build_hang_value_doc(shell, value_type, false);
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

        // Multi-argument (or non-hugging single) tail: the shared width-decided core.
        // The doc printer's look-ahead (fits_with_lookahead) handles the decision
        // of whether to break based on what follows the type params.
        d.group(self.build_angle_list_doc(
            inst.span,
            inst.params.len(),
            |i| inst.params[i].span(),
            |i, frozen| {
                if frozen {
                    self.build_frozen_list_member_doc(&inst.params[i])
                } else {
                    self.build_type_doc(&inst.params[i])
                }
            },
            |i| self.frozen_list_member_multiline(&inst.params[i]),
            has_comments,
        ))
    }

    /// Build type parameter instantiation with line comments
    fn build_type_parameter_instantiation_doc_with_line_comments(
        &self,
        inst: &internal::TSTypeParameterInstantiation<'_>,
    ) -> DocId {
        // Call/`new`-expression type arguments render each argument with
        // `build_type_doc`; the layout is shared with type-position arguments.
        self.build_angle_list_with_line_comments(
            inst.span,
            inst.params.len(),
            |i| inst.params[i].span(),
            |i, frozen| {
                if frozen {
                    self.build_frozen_list_member_doc(&inst.params[i])
                } else {
                    self.build_type_doc(&inst.params[i])
                }
            },
            true,
        )
    }

    /// The shared width-decided angle-list body: `<` + softline-indented,
    /// comma-separated items with their inline block comments + `>` — the one core
    /// behind all three `<…>` families (type-parameter declarations, call/`new`
    /// instantiations, and type-position type arguments), which previously
    /// hand-rolled this layout independently. Returns the UNGROUPED concat: the
    /// declaration's callers control the group themselves (e.g. the interface
    /// header); the type-argument callers wrap it in a group.
    ///
    /// `item_span`/`item_doc` select the family's item type and per-item printer;
    /// `item_doc` receives the item's Rule A `frozen` flag (an in-gap glued
    /// format-ignore directive — own-line spellings route to the expansion builder
    /// below before this runs) and emits the frozen verbatim slice when set.
    /// `frozen_forces_break` is asked only for frozen items: `true` (a multi-line
    /// frozen slice) forces the broken layout — a `verbatim_source_span` is
    /// `will_break`-opaque, so the forcing is explicit, and the emitted hardlines
    /// propagate the break to the caller's group.
    /// `has_comments` is the caller's whole-`<…>` window answer (on-page — a layout
    /// builder's zero-comment fast gate): `false` proves every gap below is
    /// comment-free, so neither the comment searches nor the `find_list_comma` byte
    /// scans that bound them run — a comment-free `<T, U>` builds with no source
    /// scanning at all (the printed `,` is static text). Line comments and own-line
    /// blocks never reach here (each family's expansion predicate routes them to
    /// [`Self::build_angle_list_with_line_comments`] first), so only inline block
    /// comments remain to preserve.
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

            // Leading block comments (after the previous comma or `<`)
            if has_comments
                && let Some(leading) = self.build_comments_between_filtered_opt(
                    prev_end,
                    item_span(i).start,
                    CommentSpacing::Trailing,
                    CommentFilter::BlockOnly,
                )
            {
                inner_parts.push(leading);
            }

            // Rule A: a glued directive in this item's gap freezes the item (the
            // directive itself was just emitted by the gap emitter above).
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
                    // Trailing block comments (before the comma)
                    if let Some(trailing) = self.build_comments_between_filtered_opt(
                        item_end,
                        comma_pos,
                        CommentSpacing::Leading,
                        CommentFilter::BlockOnly,
                    ) {
                        inner_parts.push(trailing);
                    }
                    prev_end = comma_pos + 1; // After comma
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

        bracketed_list_body(d, "<", ">", d.concat(&inner_parts), force_break)
    }

    /// Render a type-argument list `<…>` that breaks onto multiple lines because it
    /// carries comments — the shared body behind all three angle-list families: the
    /// call/`new`-expression ([`Self::build_type_parameter_instantiation_doc_with_line_comments`]),
    /// type-position ([`Self::build_type_arguments_doc_with_line_comments`]), and
    /// type-parameter-declaration
    /// ([`Self::build_type_parameter_declaration_doc_with_line_comments`]) printers.
    /// `item_span`/`item_doc` select the family's item type and per-item printer.
    ///
    /// With `hug_single_leading_line`, a single argument with a leading *line*
    /// comment hugs `<`/`>` (`foo<// c\n A>`) — a deliberate divergence (prettier
    /// expands; see `type_position_parens_leading_line_comment`). The declaration
    /// family passes `false` (it fully expands that shape). Every other
    /// comment-bearing form — a single-argument own-line *block* comment, or any
    /// multi-argument list — fully expands the list, matching prettier. The
    /// own-line block must NOT hug, or the emitted `</* c */⏎T>` re-collapses on
    /// the next pass (non-idempotent). A block trailing/glued to the argument never
    /// reaches here (it doesn't trip `has_own_line_block_comments_in_bracket_list`)
    /// and collapses inline.
    pub(in crate::printer) fn build_angle_list_with_line_comments(
        &self,
        span: Span,
        count: usize,
        item_span: impl Fn(usize) -> Span,
        item_doc: impl Fn(usize, bool) -> DocId,
        hug_single_leading_line: bool,
    ) -> DocId {
        let d = self.d();

        // Single-arg leading *line* comment hugs `<`/`>`.
        if hug_single_leading_line && count == 1 {
            let param_start = item_span(0).start;
            let has_line = self.has_line_comments_between(span.start + 1, param_start);
            let before_close = span.end - 1;
            let has_trailing = tsv_lang::has_comments_to_emit_in_range(
                self.comments,
                item_span(0).end,
                before_close,
            );
            // A FROZEN argument must not hug: the hug pulls the leading run onto the
            // `<` line, which would relocate an own-line directive to a trailing-`<`
            // placement the classification reads as inert — the freeze would be lost
            // on the second pass. Fall through to the full expansion instead, where
            // an own-line directive stays own-line (the same keep-own-line rule the
            // union in-span freeze carries).
            if has_line && !has_trailing && !self.list_item_frozen(span.start + 1, &item_span, 0) {
                let leading =
                    // `None`: this hug path emits no delimiter-line prefix, so nothing
                    // was pulled onto the `<` line to exclude here.
                    self.build_leading_comments_multiline(span.start + 1, param_start, None);
                if !leading.is_empty() {
                    let mut parts: DocBuf = smallvec![d.text("<")];
                    parts.extend(leading);
                    parts.push(item_doc(0, false));
                    parts.push(d.text(">"));
                    return d.concat(&parts);
                }
            }
        }

        // Full multiline expansion (multi-arg, or single-arg own-line block). A
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

            // Rule A: an own-line (or glued) directive in this item's gap freezes the
            // item; the directive itself was just emitted by the leading run above.
            // No must-break question here — this layout is already all-hardline.
            let frozen = self.list_item_frozen(span.start + 1, &item_span, i);
            inner_parts.push(item_doc(i, frozen));

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
