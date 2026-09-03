// Doc builders for Svelte template tags
//
// {@html}, {@const}, {@debug}, and {@render} — tag layout and the
// {@const} initializer break rules.

use crate::ast::internal;
use crate::printer::{HeadExpr, HeadLayout, Printer};
use tsv_lang::Span;
use tsv_lang::comments_in_source_range;
use tsv_lang::doc::arena::DocId;
use tsv_lang::doc::{DocBuf, GroupId};
use tsv_lang::source_scan::TriviaProfile;
use tsv_ts::Expression;

// Opening-tag literals whose `.len()` locates the embedded expression past the
// tag; sharing the literal keeps the emitted text and the scan offset in sync.
const HTML_TAG_OPEN: &str = "{@html ";
const RENDER_TAG_OPEN: &str = "{@render ";
// No trailing space — the space (when content follows) is emitted separately,
// and the `.len()` derives the offset past the keyword for comment scanning.
const DEBUG_TAG_OPEN: &str = "{@debug";
/// [`DEBUG_TAG_OPEN`] as the head assembler wants it — carrying the separating space every
/// other tag's opening literal carries. The space-less spelling above stays, because it also
/// **measures** the keyword (`{@debug}` with no identifiers is a valid tag), and
/// [`Printer::head_open_doc`] trims this one back for a head whose content opens on its own
/// line.
const DEBUG_TAG_OPEN_SPACED: &str = "{@debug ";
const AT_CONST_TAG_OPEN: &str = "{@const ";

impl<'a> Printer<'a> {
    /// Build a doc for {@html expr}
    ///
    /// An assignment's clarity parens (`{@html (a = b)}`) are applied inside
    /// [`Printer::build_expression_with_comments_doc`], so they land inside a frozen head's
    /// break rather than around it.
    pub(super) fn build_html_tag_doc(&self, tag: &internal::HtmlTag<'_>) -> DocId {
        // Build expression doc with surrounding comments
        // Span range: after "{@html " to before "}"
        let head = self.build_expression_with_comments_doc(
            &tag.expression,
            tag.span.start + HTML_TAG_OPEN.len() as u32,
            tag.span.end - 1,
        );

        self.build_prefixed_head_doc(HTML_TAG_OPEN, head, self.d().text("}"))
    }

    /// Build a doc for {@const declaration}
    pub(super) fn build_const_tag_doc(&self, tag: &internal::ConstTag<'_>) -> DocId {
        self.build_assignment_tag_doc(AT_CONST_TAG_OPEN, &tag.id, &tag.init, tag.span)
    }

    /// Build a doc for `{const …}` / `{let …}` — the body is a TS variable
    /// declaration, so the layout (declarator breaking, long-init break-after-`=`,
    /// comments) is delegated to `tsv_ts`. The `}` terminates the tag, so the
    /// trailing `;` is dropped — except for a bare single declarator (`{let a}` →
    /// `{let a;}`), which prettier keeps.
    pub(super) fn build_declaration_tag_doc(&self, tag: &internal::DeclarationTag<'_>) -> DocId {
        let d = self.d();
        let decl = &tag.declaration;
        let emit_semicolon = decl.declarations.len() == 1 && decl.declarations[0].init.is_none();
        let inner = tsv_ts::build_variable_declaration_doc(
            d,
            decl,
            &self.ts_inputs(),
            self.embed,
            emit_semicolon,
        );
        d.concat(&[d.text("{"), inner, d.text("}")])
    }

    /// Shared assignment-tag layout for `{@const}` and `{const}`/`{let}` (with init).
    ///
    /// Prettier formats these as an AssignmentExpression, using its assignment
    /// layout to decide whether to break at `=`. Three layouts:
    /// - will_break: `{… id = init}` (init has hardlines, keep together)
    /// - fluid: `{… id = init}` or `{… id =\n\tinit}` (marker group)
    /// - break-after-operator: `{… id =\n\tinit}` (group with line at `=`)
    fn build_assignment_tag_doc(
        &self,
        prefix: &'static str,
        id: &Expression<'_>,
        init: &Expression<'_>,
        span: Span,
    ) -> DocId {
        let d = self.d();
        // Comment-aware: a destructure id can carry comments inside
        // (`{@const { b = /* c */ 1 } = expr}`); the comment-less builder
        // silently dropped them.
        let id_doc = self.build_ts_expression_doc(id);

        // Both questions below scan the `=` gap forward from the binding, so they anchor past
        // its `: T` — which `id_doc` has already printed, and which the bare span excludes.
        let binding_end = tsv_ts::pattern_binding_end(id);

        // The layout verdict is taken BEFORE the init is built, because it is also the
        // [`Printer::trailing_comment_docs`] `closer_owns_break` answer: only the
        // break-after-operator arm below puts the init inside an `indent(…)` with `}`
        // outside it, so only there must a run-final `//`'s break be emitted one level out.
        // The other two arms leave the init on the tag's own column, where the comment's own
        // `hardline` is already the break the `}` needs.
        let break_after_op = Self::const_should_break_after_op(init)
            || self.gap_comment_hangs_value(binding_end, init.span().start);

        // Build init with LayoutMode::Standalone so a ROOT binary init uses Grouped
        // style (not ContinuationIndent). The assignment layout handles indentation —
        // ContinuationIndent would double-indent continuation lines.
        let init_doc = self.build_const_init_doc(
            init,
            binding_end, // scan from after the binding so a comment between `=` and init survives
            span.end - 1, // before "}"
            break_after_op,
        );
        let close = d.text("}");

        // Choose layout matching prettier's assignment layout selection.
        if break_after_op {
            // Binary expressions, conditional with binary test, etc.
            // Break-after-operator: group with line at "=" so the doc printer
            // can break when the flat form exceeds print width. This takes
            // precedence over the `will_break` keep-together branch below — a
            // break-after-operator RHS still breaks after `=` even when it has a
            // forced internal break (e.g. a conditional whose binary test carries
            // a trailing line comment), matching prettier and our own TS
            // assignment printer.
            // Prettier ref: shouldBreakAfterOperator (assignment.js:196-259)
            let rhs = d.concat(&[d.line(), init_doc]);
            let rhs_indented = d.indent(rhs);
            let assignment = d.group(d.concat(&[d.text(" ="), rhs_indented, close]));

            d.concat(&[d.text(prefix), id_doc, assignment])
        } else if d.will_break(init_doc) {
            // Init has forced breaks (object/array/template, etc.) that aren't
            // break-after-operator — keep "= init" together, init's own breaks
            // handle formatting.
            d.concat(&[d.text(prefix), id_doc, d.text(" = "), init_doc, close])
        } else {
            // Fluid layout: break at `=` only when the full line exceeds
            // print width. Uses indentIfBreak so the RHS is evaluated
            // independently — e.g., a ternary with identifier test stays
            // on the same line as `=` while its branches break below.
            // Prettier ref: "fluid" layout (assignment.js:59-67)
            //
            // `indent_if_break` is the one indent here, and it is CONDITIONAL, so a
            // `closer_owns_break` dedent — which is not — could not serve it. It never has
            // to: a run-final `//` ends `init_doc` with a `hardline`, so `will_break` above
            // claims that init and this arm is unreachable for it. Only a trailing BLOCK
            // comment reaches here, and a block asks the closer for no break at all.
            d.concat(&[
                d.text(prefix),
                id_doc,
                d.text(" ="),
                d.group_with_id(d.indent(d.line()), GroupId::Assignment),
                d.line_suffix_boundary(),
                d.indent_if_break(init_doc, GroupId::Assignment),
                close,
            ])
        }
    }

    /// Whether a comment in the binding→value gap hangs the value — prettier's
    /// `hasLeadingOwnLineComment` (`hasNewline` at the comment's END, the same shape
    /// `tsv_ts`'s object-property gap asks), which forces break-after-operator so the
    /// comment and the value indent together under the `=`.
    ///
    /// A comment the author did not glue to the value cannot ride on the operator's line:
    /// a `//` would swallow what follows, and an own-line block pulled flush against the
    /// `=` loses the line the author gave it — which for an honored directive makes the
    /// placement inert, so the freeze would die on the second pass. `tsv_ts`'s assignment
    /// printer states the same rule for `<script>` code; this is the `{@const}` tag's copy
    /// of it, and without it a self-expanding value (object, array) took the fluid layout
    /// and glued the run to `=`, diverging from prettier for a plain comment too.
    ///
    /// **on page**, not to-emit: hanging the value is a LAYOUT decision, and a block glued
    /// to the value is *owned* by it — emitted from inside the value's own doc, so an
    /// emit-keyed scan never sees it and the gate went blind to exactly the comments that
    /// sit closest to the value. That blindness is what let `{@const b = /**⏎ * c⏎ */ x}`
    /// stay inline where `<script>`'s `const b = …` hung it, on the same input.
    ///
    /// An **indentable** block hangs even when glued (its reprint is hard lines); a
    /// preserved multi-line block glued to the value does not (see
    /// [`tsv_lang::is_indentable_block`]) — the shared rule, so the two hosts cannot
    /// answer it differently.
    fn gap_comment_hangs_value(&self, gap_start: u32, value_start: u32) -> bool {
        tsv_lang::comments_on_page_in_range(self.comments, gap_start, value_start).any(|c| {
            !c.is_block
                || tsv_lang::is_indentable_block(self.source, c)
                || tsv_lang::source_scan::has_newline_after_position(self.source, c.span.end)
        })
    }

    /// Check if a @const init expression needs break-after-operator layout.
    ///
    /// Matches prettier's `shouldBreakAfterOperator` for the expression types
    /// that appear in @const tags, delegating to tsv_ts's predicates so the
    /// rules can't drift from our own assignment printer.
    /// Prettier ref: assignment.js:196-226
    fn const_should_break_after_op(expr: &Expression<'_>) -> bool {
        match expr {
            // Binary expressions break after `=`, UNLESS it's a logical expression
            // with a self-expanding RHS (non-empty object/array). In that case, the
            // RHS handles its own expansion: `= item || { ... }` not `=\n  item || {}`
            // Prettier ref: assignment.js:199 `isBinaryish && !shouldInlineLogicalExpression`
            Expression::BinaryExpression(bin) => !tsv_ts::should_inline_logical_expression(bin),
            Expression::SequenceExpression(_) => true,
            // Conditionals break only when the test is binary (and not inline
            // logical); simple identifier tests (e.g., `cond ? a : b`) use fluid
            // layout. False for every other expression type.
            // Prettier ref: assignment.js:216-219
            _ => tsv_ts::conditional_should_break_after_op(expr),
        }
    }

    /// Build init expression doc for @const with assignment-appropriate config.
    ///
    /// Like `build_expression_with_comments_doc` but **without** its
    /// `mode: LayoutMode::Embedded` — inheriting the host's Standalone mode is what keeps
    /// a ROOT binary init on Grouped style rather than ContinuationIndent, since the
    /// expression-root entry (`build_root_expression_doc`) reads
    /// `EmbedContext::is_embedded()`. The @const assignment layout handles indentation;
    /// ContinuationIndent would stack on top of it.
    ///
    /// The `=`→value head: an own-line directive anywhere in the binding→value gap
    /// freezes the whole value. The window is the whole gap rather than the part past
    /// the `=`, because this tag has no separate before-`=` emitter — `leading_docs`
    /// prints every comment in it directly above the value, and the directive that
    /// freezes a value is the one printed above it.
    ///
    /// `closer_owns_break` is the caller's layout verdict, forwarded to
    /// [`Printer::trailing_comment_docs`] — the init is inside an `indent(…)` with the tag's
    /// `}` outside it in exactly one of `build_assignment_tag_doc`'s layouts, and this
    /// builder runs before that choice is applied, so it cannot answer the question itself.
    fn build_const_init_doc(
        &self,
        expr: &Expression<'_>,
        span_start: u32,
        span_end: u32,
        closer_owns_break: bool,
    ) -> DocId {
        let expr_start = expr.span().start;
        let expr_end = expr.span().end;
        let frozen = self.honored_directive_in_gap(span_start, expr_start);

        let leading_docs = self.leading_comment_docs(span_start, expr_start);

        // mode defaults to Standalone: a root binary uses Grouped style, not ContinuationIndent
        let embed = tsv_lang::EmbedContext {
            first_line_offset: 0,
            ..self.embed
        };

        // No clarity parens (`wrap_value_clarity_parens`, which this site deliberately does
        // not call): here the paren is fully redundant and prettier drops it (`{@const a = (b = c)}` →
        // `{@const a = b = c}`), so the frozen arm drops it too — consistent with this
        // site's own unfrozen normalization, which is what the freeze must not contradict.
        let expr_doc = self.build_head_value_doc(expr, frozen, &embed);

        // The run's last comment supplies the break the tag's `}` reuses —
        // `build_assignment_tag_doc` places that `}` in all three of its layouts, and
        // `closer_owns_break` says which of them indented the init out from under it.
        let (trailing_docs, _) = self.trailing_comment_docs(expr_end, span_end, closer_owns_break);

        self.concat_with_surrounding_comments(leading_docs, expr_doc, trailing_docs)
    }

    /// Build a doc for {@debug vars}
    ///
    /// Unlike Prettier (which strips them), tsv preserves embedded TS comments —
    /// a cataloged divergence (`tags/debug/debug_comment_prettier_divergence`).
    /// Comments are looked up from `Root.comments` by span and interleaved with
    /// the identifiers, matching the (former) buffer printer's placement.
    ///
    /// This uses the **in-source** axis (`comments_in_source_range`), not the
    /// to-emit one: the identifiers are emitted as raw source spans
    /// (`d.source_span`), not through the ownership-aware expression printer, so
    /// this builder is the sole positional emitter of *every* comment in its
    /// content — including a block comment glued to an identifier
    /// (`{@debug /* c */ a}`), which the parser marks `owned_by_node` and the
    /// to-emit axis would skip, dropping it.
    ///
    /// Between two identifiers the gap is split at the separator comma (located
    /// trivia-aware via `find_top_level_delim`): a comment before the comma trails
    /// the previous identifier, one after it leads the next. Without the split a
    /// pre-comma comment (`{@debug x /* c */, y}`) falls through both
    /// per-identifier buckets and is dropped.
    pub(super) fn build_debug_tag_doc(&self, tag: &internal::DebugTag<'_>) -> DocId {
        let d = self.d();

        // Every gap below is read with `comments_in_source_range` — the **in-source** axis
        // named at each call site rather than filtered out of one collected run, so the
        // axis this builder stands on cannot be mistaken for the to-emit one anywhere in it.
        let tag_end = tag.span.end;
        if tag.identifiers.is_empty()
            && comments_in_source_range(self.comments, tag.span.start, tag_end)
                .next()
                .is_none()
        {
            return d.text("{@debug}");
        }

        // Track position as we emit content, starting after the "{@debug" keyword.
        let mut last_end = tag.span.start + DEBUG_TAG_OPEN.len() as u32;

        // The `{@debug`→identifiers head: an own-line directive there freezes the whole
        // identifier **list** — the run from the first identifier to the last, which is
        // what the tag normalizes (`{@debug  a ,  b }` → `{@debug a, b}`). The prefix and
        // the `}` stay parent-owned, as at every other prefixed head.
        //
        // `verbatim_source_doc`, not `build_frozen_node_doc`: this builder is the sole
        // POSITIONAL emitter of every comment in its content (the in-source axis its doc
        // comment above explains), so the head-gap run — an owned glued block included — is
        // already printed below. A second claim inside the slice would double-print it.
        if let (Some(first), Some(last)) = (tag.identifiers.first(), tag.identifiers.last())
            && self.honored_directive_in_gap(last_end, first.span().start)
        {
            let list = Span::new(first.span().start, last.span().end);
            let mut frozen_parts: DocBuf =
                comments_in_source_range(self.comments, last_end, list.start)
                    .map(|c| self.build_leading_js_comment_doc(c))
                    .collect();
            frozen_parts.push(self.verbatim_source_doc(list));
            // This builder emits its own trailing run, so it answers `ends_with_line_comment`
            // off that one run rather than from a second scan that could disagree with it.
            //
            // `closer_owns_break: true` — `indent_own_line_head` below indents this content, and
            // the freeze is the first of the four things that do, so a run-final `//` must
            // break one level out or the `}` lands at the identifier list's column.
            let (trailing_docs, ends_with_line_comment) = self.trailing_comment_run_docs(
                comments_in_source_range(self.comments, list.end, tag_end),
                true,
            );
            frozen_parts.extend(trailing_docs);
            let doc = self.indent_own_line_head(d.concat(&frozen_parts));
            return self.build_prefixed_head_doc(
                DEBUG_TAG_OPEN_SPACED,
                HeadExpr {
                    doc,
                    layout: HeadLayout::OpensOwnLine,
                    ends_with_line_comment,
                    owes_continuation_indent: false,
                },
                d.text("}"),
            );
        }

        // The shared braced-head layout ([`Printer::head_layout`]), over the
        // keyword→first-identifier gap. Taken before the parts are assembled because
        // `indents_content` is also the dedent the trailing run below owes —
        // `trailing_comment_docs`' `closer_owns_break`, spelled out by hand here since this
        // builder emits its own run.
        let layout = tag.identifiers.first().map_or(HeadLayout::Inline, |first| {
            self.head_layout(last_end, first.span().start, false)
        });

        // Content only — the prefix and the `}` are added below, so `hangs` can indent
        // exactly what sits between them (the shape `indent_own_line_head` reaches on the
        // frozen arm above, and `Printer::build_prefixed_head_doc` on every other tag).
        let mut parts = DocBuf::new();

        for (i, id) in tag.identifiers.iter().enumerate() {
            let id_start = id.span().start;
            if i > 0 {
                // Locate the separator comma at bracket depth 0 in the gap between
                // the previous identifier and this one (an identifier is a plain
                // `Identifier` or a JSDoc cast of one, whose span starts at its `(`
                // and whose comment is gap trivia here, so the gap holds only
                // trivia and the one comma). A comment before the comma trails the
                // previous identifier; one after it leads this one. Scanning
                // trivia-aware — the same scan `parse_const_tag` uses — keeps a `,`
                // inside a comment from mis-anchoring the split.
                let comma = crate::parser::find_top_level_delim(
                    self.source.as_bytes(),
                    last_end as usize,
                    id_start as usize,
                    b',',
                    TriviaProfile::JS,
                )
                .map_or(id_start, |c| c as u32);

                // Comments before the comma trail the previous identifier.
                parts.extend(
                    comments_in_source_range(self.comments, last_end, comma)
                        .map(|c| self.build_trailing_js_comment_doc(c, false)),
                );
                parts.push(d.text(", "));
                last_end = comma.saturating_add(1);
            }
            // Comments after the comma (or after the keyword, for the first
            // identifier) lead this identifier.
            parts.extend(
                comments_in_source_range(self.comments, last_end, id_start)
                    .map(|c| self.build_leading_js_comment_doc(c)),
            );
            // Not a bare `d.source_span`: a JSDoc-cast entry's span (`(a)`) can
            // hold an interior comment (`(a /* c */)`) that rides out inside the
            // slice and reaches no comment emitter — the ledger is told the
            // range is covered. Still ordinary source-slice semantics (an
            // interior newline breaks the enclosing group).
            parts.push(self.source_span_covering_comments_doc(id.span()));
            last_end = id.span().end;
        }

        // Trailing comments (after the last identifier), through the shared pairing — only
        // the run's LAST comment can supply the break the `}` reuses, so only it takes the
        // dedent. [`Printer::trailing_comment_run_docs`] rather than its lookup-bearing
        // wrapper: this builder stands on the in-source axis its doc comment above explains.
        let (trailing_docs, ends_with_line_comment) = self.trailing_comment_run_docs(
            comments_in_source_range(self.comments, last_end, tag_end),
            layout.indents_content(),
        );
        parts.extend(trailing_docs);

        let content = self.indent_head_content(d.concat(&parts), layout);
        self.build_prefixed_head_doc(
            DEBUG_TAG_OPEN_SPACED,
            HeadExpr {
                doc: content,
                layout,
                ends_with_line_comment,
                owes_continuation_indent: false,
            },
            d.text("}"),
        )
    }

    /// Build a doc for {@render snippet(args)}
    pub(super) fn build_render_tag_doc(&self, tag: &internal::RenderTag<'_>) -> DocId {
        // Build expression doc with surrounding comments
        // Span range: after "{@render " to before "}"
        let head = self.build_expression_with_comments_doc(
            &tag.expression,
            tag.span.start + RENDER_TAG_OPEN.len() as u32,
            tag.span.end - 1,
        );

        self.build_prefixed_head_doc(RENDER_TAG_OPEN, head, self.d().text("}"))
    }
}
