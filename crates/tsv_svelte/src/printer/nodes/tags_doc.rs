// Doc builders for Svelte template tags
//
// {@html}, {@const}, {@debug}, and {@render} — tag layout and the
// {@const} initializer break rules.

use crate::ast::internal;
use crate::printer::Printer;
use tsv_lang::doc::GroupId;
use tsv_lang::doc::arena::DocId;

// Opening-tag literals whose `.len()` locates the embedded expression past the
// tag; sharing the literal keeps the emitted text and the scan offset in sync.
const HTML_TAG_OPEN: &str = "{@html ";
const RENDER_TAG_OPEN: &str = "{@render ";
// No trailing space — the space (when content follows) is emitted separately,
// and the `.len()` derives the offset past the keyword for comment scanning.
const DEBUG_TAG_OPEN: &str = "{@debug";
const AT_CONST_TAG_OPEN: &str = "{@const ";
const CONST_TAG_OPEN: &str = "{const ";
const LET_TAG_OPEN: &str = "{let ";

impl<'a> Printer<'a> {
    /// Build a doc for {@html expr}
    pub(crate) fn build_html_tag_doc(&self, tag: &internal::HtmlTag) -> DocId {
        let d = self.d();
        // Build expression doc with surrounding comments
        // Span range: after "{@html " to before "}"
        let expr_doc = self.build_expression_with_comments_doc(
            &tag.expression,
            tag.span.start + HTML_TAG_OPEN.len() as u32,
            tag.span.end - 1,
        );

        // Assignment expressions need parens: {@html (a = b)}
        let expr_doc = if matches!(tag.expression, tsv_ts::Expression::AssignmentExpression(_)) {
            d.parens(expr_doc)
        } else {
            expr_doc
        };

        d.concat(&[d.text(HTML_TAG_OPEN), expr_doc, d.text("}")])
    }

    /// Build a doc for {@const declaration}
    pub(crate) fn build_const_tag_doc(&self, tag: &internal::ConstTag) -> DocId {
        self.build_assignment_tag_doc(AT_CONST_TAG_OPEN, &tag.id, &tag.init, tag.span)
    }

    /// Build a doc for `{const id = init}` / `{let id = init}` / `{let id}`.
    ///
    /// The with-init form reuses the shared `{@const}` assignment layout (just a
    /// different opening keyword). The binding-less `let` form (`{let id}`) has
    /// no initializer — prettier renders it as a bare declaration with a trailing
    /// `;`.
    pub(crate) fn build_declaration_tag_doc(&self, tag: &internal::DeclarationTag) -> DocId {
        let prefix = match tag.kind {
            internal::DeclarationKind::Const => CONST_TAG_OPEN,
            internal::DeclarationKind::Let => LET_TAG_OPEN,
        };
        match &tag.init {
            Some(init) => self.build_assignment_tag_doc(prefix, &tag.id, init, tag.span),
            None => {
                let d = self.d();
                let id_doc = self.build_ts_expression_doc_no_comments(&tag.id);
                d.concat(&[d.text(prefix), id_doc, d.text(";}")])
            }
        }
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
        id: &tsv_ts::Expression,
        init: &tsv_ts::Expression,
        span: tsv_lang::Span,
    ) -> DocId {
        let d = self.d();
        let id_doc = self.build_ts_expression_doc_no_comments(id);
        // Build init with LayoutMode::Standalone so binary chains use Grouped style
        // (not ContinuationIndent). The assignment layout handles indentation —
        // ContinuationIndent would double-indent continuation lines.
        let init_doc = self.build_const_init_doc(
            init,
            id.span().end, // scan from after the id so a comment between `=` and init survives
            span.end - 1,  // before "}"
        );

        // Choose layout matching prettier's assignment layout selection.
        if Self::const_should_break_after_op(init) {
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
            let assignment = d.group(d.concat(&[d.text(" ="), rhs_indented, d.text("}")]));

            d.concat(&[d.text(prefix), id_doc, assignment])
        } else if d.will_break(init_doc) {
            // Init has forced breaks (object/array/template, etc.) that aren't
            // break-after-operator — keep "= init" together, init's own breaks
            // handle formatting.
            d.concat(&[d.text(prefix), id_doc, d.text(" = "), init_doc, d.text("}")])
        } else {
            // Fluid layout: break at `=` only when the full line exceeds
            // print width. Uses indentIfBreak so the RHS is evaluated
            // independently — e.g., a ternary with identifier test stays
            // on the same line as `=` while its branches break below.
            // Prettier ref: "fluid" layout (assignment.js:59-67)
            d.concat(&[
                d.text(prefix),
                id_doc,
                d.text(" ="),
                d.group_with_id(d.indent(d.line()), GroupId::Assignment),
                d.line_suffix_boundary(),
                d.indent_if_break(init_doc, GroupId::Assignment, false),
                d.text("}"),
            ])
        }
    }

    /// Check if a @const init expression needs break-after-operator layout.
    ///
    /// Matches prettier's `shouldBreakAfterOperator` for the expression types
    /// that appear in @const tags, delegating to tsv_ts's predicates so the
    /// rules can't drift from our own assignment printer.
    /// Prettier ref: assignment.js:196-226
    fn const_should_break_after_op(expr: &tsv_ts::Expression) -> bool {
        match expr {
            // Binary expressions break after `=`, UNLESS it's a logical expression
            // with a self-expanding RHS (non-empty object/array). In that case, the
            // RHS handles its own expansion: `= item || { ... }` not `=\n  item || {}`
            // Prettier ref: assignment.js:199 `isBinaryish && !shouldInlineLogicalExpression`
            tsv_ts::Expression::BinaryExpression(bin) => {
                !tsv_ts::should_inline_logical_expression(bin)
            }
            tsv_ts::Expression::SequenceExpression(_) => true,
            // Conditionals break only when the test is binary (and not inline
            // logical); simple identifier tests (e.g., `cond ? a : b`) use fluid
            // layout. False for every other expression type.
            // Prettier ref: assignment.js:216-219
            _ => tsv_ts::conditional_should_break_after_op(expr),
        }
    }

    /// Build init expression doc for @const with assignment-appropriate config.
    ///
    /// Like `build_expression_with_comments_doc` but uses `first_line_offset = 0`
    /// so binary chains use Grouped style (not ContinuationIndent). The @const
    /// assignment layout handles indentation; ContinuationIndent would stack.
    fn build_const_init_doc(
        &self,
        expr: &tsv_ts::Expression,
        span_start: u32,
        span_end: u32,
    ) -> DocId {
        let d = self.d();
        let expr_start = expr.span().start;
        let expr_end = expr.span().end;

        let leading_docs: Vec<DocId> =
            tsv_lang::comments_in_range(self.comments, span_start, expr_start)
                .map(|c| self.build_leading_js_comment_doc(c))
                .collect();

        // mode defaults to Standalone: binary chains use Grouped style, not ContinuationIndent
        let embed = tsv_lang::EmbedContext {
            first_line_offset: 0,
            ..self.embed
        };

        let expr_doc =
            tsv_ts::build_expression_doc_with_comments(d, expr, &self.ts_inputs(), &embed);

        let trailing_docs: Vec<DocId> =
            tsv_lang::comments_in_range(self.comments, expr_end, span_end)
                .map(|c| self.build_trailing_js_comment_doc(c))
                .collect();

        self.concat_with_surrounding_comments(leading_docs, expr_doc, trailing_docs)
    }

    /// Build a doc for {@debug vars}
    ///
    /// Unlike Prettier (which strips them), tsv preserves embedded TS comments —
    /// a cataloged divergence (`tags/debug/debug_comment_prettier_divergence`).
    /// Comments are looked up from `Root.comments` by span and interleaved with
    /// the identifiers, matching the (former) buffer printer's placement.
    pub(crate) fn build_debug_tag_doc(&self, tag: &internal::DebugTag) -> DocId {
        let d = self.d();

        // Comments within the tag's content (after "{@debug" and before "}").
        let tag_comments: Vec<&tsv_lang::Comment> =
            tsv_lang::comments_in_range(self.comments, tag.span.start, tag.span.end).collect();

        if tag.identifiers.is_empty() && tag_comments.is_empty() {
            return d.text("{@debug}");
        }

        let mut parts: Vec<DocId> = vec![d.text("{@debug ")];
        // Track position as we emit content, starting after the "{@debug" keyword.
        let mut last_end = tag.span.start + DEBUG_TAG_OPEN.len() as u32;

        for (i, id) in tag.identifiers.iter().enumerate() {
            if i > 0 {
                parts.push(d.text(", "));
                last_end += 2; // ", "
            }
            // Comments appearing before this identifier.
            for comment in &tag_comments {
                if comment.span.start >= last_end && comment.span.end <= id.span().start {
                    parts.push(self.build_leading_js_comment_doc(comment));
                    last_end = comment.span.end;
                }
            }
            let name = self.extract_source_range(id.span().start_usize(), id.span().end_usize());
            parts.push(d.text_owned(name.to_string()));
            last_end = id.span().end;
        }

        // Trailing comments (after the last identifier).
        for comment in &tag_comments {
            if comment.span.start >= last_end {
                parts.push(self.build_trailing_js_comment_doc(comment));
            }
        }

        parts.push(d.text("}"));
        d.concat(&parts)
    }

    /// Build a doc for {@render snippet(args)}
    pub(crate) fn build_render_tag_doc(&self, tag: &internal::RenderTag) -> DocId {
        let d = self.d();
        // Build expression doc with surrounding comments
        // Span range: after "{@render " to before "}"
        let expr_doc = self.build_expression_with_comments_doc(
            &tag.expression,
            tag.span.start + RENDER_TAG_OPEN.len() as u32,
            tag.span.end - 1,
        );

        d.concat(&[d.text(RENDER_TAG_OPEN), expr_doc, d.text("}")])
    }
}
