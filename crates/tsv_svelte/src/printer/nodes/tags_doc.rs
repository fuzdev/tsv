// Doc builders for Svelte template tags
//
// {@html}, {@const}, {@debug}, and {@render} — tag layout and the
// {@const} initializer break rules.

use std::rc::Rc;

use crate::ast::internal;
use crate::printer::Printer;
use tsv_lang::doc::GroupId;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build a doc for {@html expr}
    pub(crate) fn build_html_tag_doc(&self, tag: &internal::HtmlTag) -> DocId {
        let d = self.d();
        // Build expression doc with surrounding comments
        // Span range: after "{@html " (start + 7) to before "}" (end - 1)
        let expr_doc = self.build_expression_with_comments_doc(
            &tag.expression,
            tag.span.start + 7, // after "{@html "
            tag.span.end - 1,   // before "}"
        );

        // Assignment expressions need parens: {@html (a = b)}
        let expr_doc = if matches!(tag.expression, tsv_ts::Expression::AssignmentExpression(_)) {
            d.parens(expr_doc)
        } else {
            expr_doc
        };

        d.concat(&[d.text("{@html "), expr_doc, d.text("}")])
    }

    /// Build a doc for {@const declaration}
    ///
    /// Prettier formats @const as an AssignmentExpression, using its assignment
    /// layout to decide whether to break at `=`. Three layouts:
    /// - will_break: `{@const id = init}` (init has hardlines, keep together)
    /// - fluid: `{@const id = init}` or `{@const id =\n\tinit}` (marker group)
    /// - break-after-operator: `{@const id =\n\tinit}` (group with line at `=`)
    pub(crate) fn build_const_tag_doc(&self, tag: &internal::ConstTag) -> DocId {
        let d = self.d();
        let id_doc = self.build_ts_expression_doc_no_comments(&tag.id);
        // Build init with is_embedded_expression=false so binary chains use Grouped style
        // (not ContinuationIndent). The assignment layout handles indentation —
        // ContinuationIndent would double-indent continuation lines.
        let init_doc = self.build_const_init_doc(
            &tag.init,
            tag.init.span().start,
            tag.span.end - 1, // before "}"
        );

        // Choose layout matching prettier's assignment layout selection.
        if d.will_break(init_doc) {
            // Init has forced breaks (ternary, multi-line template, etc.)
            // Keep "= init" together — init's own breaks handle formatting.
            d.concat(&[
                d.text("{@const "),
                id_doc,
                d.text(" = "),
                init_doc,
                d.text("}"),
            ])
        } else if Self::const_should_break_after_op(&tag.init) {
            // Binary expressions, conditional with binary test, etc.
            // Break-after-operator: group with line at "=" so the doc printer
            // can break when the flat form exceeds print width.
            // Prettier ref: shouldBreakAfterOperator (assignment.js:196-259)
            let rhs = d.concat(&[d.line(), init_doc]);
            let rhs_indented = d.indent(rhs);
            let assignment = d.group(d.concat(&[d.text(" ="), rhs_indented, d.text("}")]));

            d.concat(&[d.text("{@const "), id_doc, assignment])
        } else {
            // Fluid layout: break at `=` only when the full line exceeds
            // print width. Uses indentIfBreak so the RHS is evaluated
            // independently — e.g., a ternary with identifier test stays
            // on the same line as `=` while its branches break below.
            // Prettier ref: "fluid" layout (assignment.js:59-67)
            d.concat(&[
                d.text("{@const "),
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

        let expr_doc = tsv_ts::build_expression_doc_with_comments(
            d,
            expr,
            self.source,
            Rc::clone(&self.interner),
            &embed,
            self.comments,
            &self.line_breaks,
            tsv_ts::TsContext::Svelte,
        );

        let trailing_docs: Vec<DocId> =
            tsv_lang::comments_in_range(self.comments, expr_end, span_end)
                .map(|c| self.build_trailing_js_comment_doc(c))
                .collect();

        if leading_docs.is_empty() && trailing_docs.is_empty() {
            expr_doc
        } else {
            let mut parts = Vec::with_capacity(leading_docs.len() + 1 + trailing_docs.len());
            parts.extend(leading_docs);
            parts.push(expr_doc);
            parts.extend(trailing_docs);
            d.concat(&parts)
        }
    }

    /// Build a doc for {@debug vars}
    pub(crate) fn build_debug_tag_doc(&self, tag: &internal::DebugTag) -> DocId {
        let d = self.d();
        if tag.identifiers.is_empty() {
            d.text("{@debug}")
        } else {
            let idents: Vec<DocId> = tag
                .identifiers
                .iter()
                .map(|id| {
                    let name =
                        self.extract_source_range(id.span().start_usize(), id.span().end_usize());
                    d.text_owned(name.to_string())
                })
                .collect();

            d.concat(&[d.text("{@debug "), d.join(idents, ", "), d.text("}")])
        }
    }

    /// Build a doc for {@render snippet(args)}
    pub(crate) fn build_render_tag_doc(&self, tag: &internal::RenderTag) -> DocId {
        let d = self.d();
        // Build expression doc with surrounding comments
        // Span range: after "{@render " (start + 9) to before "}" (end - 1)
        let expr_doc = self.build_expression_with_comments_doc(
            &tag.expression,
            tag.span.start + 9, // after "{@render "
            tag.span.end - 1,   // before "}"
        );

        d.concat(&[d.text("{@render "), expr_doc, d.text("}")])
    }
}
