// Template literal printing for TypeScript
//
// Builds docs for template literals (quasi/interpolation layout, alignment,
// interpolation comments) and tagged template expressions.

use crate::ast::internal::Expression;
use crate::printer::comments::CommentSpacing;
use crate::printer::{ParenContext, Printer};
use tsv_lang::TAB_WIDTH;
use tsv_lang::comments_in_range;
use tsv_lang::doc::arena::DocId;
use tsv_lang::printing::visual_width;

impl<'a> Printer<'a> {
    /// Build a Doc for a template literal.
    ///
    /// Two formatting strategies based on expression type:
    /// - **Qualifying types** (Identifier, MemberExpression, Conditional, etc.):
    ///   softline wrapping in a regular group — `${/}` breaks when line exceeds width.
    /// - **Non-qualifying types** (CallExpression, chains, arrows, etc.):
    ///   no softlines at `${/}` — expression breaks internally, boundaries hug.
    ///   Matches Prettier's approach where these types keep their doc structure.
    pub(super) fn build_template_literal_doc(
        &self,
        template: &crate::ast::internal::TemplateLiteral<'_>,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();
        parts.push(d.line_suffix_boundary());
        parts.push(d.text("`"));

        let mut previous_quasi_indent_size: usize = 0;

        for (i, quasi) in template.quasis.iter().enumerate() {
            // Template content — split at newlines and join with literalline
            // (matches Prettier's replaceEndOfLine(node.value.raw) for TemplateElement).
            // Using literalline makes will_break() propagate correctly through
            // containing groups, so chains/calls break when they contain multiline templates.
            parts.push(self.replace_end_of_line(quasi.raw(self.source), quasi.has_newline));

            // Interpolation
            if i < template.expressions.len() {
                let expr = &template.expressions[i];
                let next_quasi = &template.quasis[i + 1];

                // Calculate indent size from quasi text (Prettier's getIndentSize)
                let text = quasi.raw(self.source);
                let indent_size = if quasi.has_newline {
                    self.get_template_indent_size(text)
                } else {
                    previous_quasi_indent_size
                };
                previous_quasi_indent_size = indent_size;

                // Build expression doc
                let expr_doc = if self.needs_parens(expr, ParenContext::TemplateLiteralExpression) {
                    d.parens(self.build_expression_doc(expr))
                } else {
                    self.build_expression_doc(expr)
                };

                // Collect comments in the interpolation region
                let leading_comments: Vec<_> =
                    comments_in_range(self.comments, quasi.span.end, expr.span().start).collect();
                let trailing_comments: Vec<_> =
                    comments_in_range(self.comments, expr.span().end, next_quasi.span.start)
                        .collect();

                // Check if any comments are line comments (non-block)
                let has_line_comment = leading_comments.iter().any(|c| !c.is_block)
                    || trailing_comments.iter().any(|c| !c.is_block);

                // Combine expression with comments. The line-comment path builds its own
                // layout (hardlines after `//`), so the plain leading/trailing docs are only
                // built in the branch that uses them.
                let full_expr_doc = if has_line_comment {
                    self.build_template_comments_and_expr_doc(
                        &leading_comments,
                        expr_doc,
                        &trailing_comments,
                    )
                } else {
                    let leading_comments_doc =
                        self.build_template_interpolation_comments(&leading_comments, true);
                    let trailing_comments_doc =
                        self.build_template_interpolation_comments(&trailing_comments, false);
                    d.concat(&[leading_comments_doc, expr_doc, trailing_comments_doc])
                };

                let has_comments = !leading_comments.is_empty() || !trailing_comments.is_empty();
                let has_trailing_line_comment = trailing_comments.iter().any(|c| !c.is_block);

                // Qualifying types and trailing line comments use softline wrapping
                // at ${/} boundaries so the group can break there.
                // Non-qualifying types keep expression doc as-is (no ${/} softlines)
                // so ${ hugs while the expression breaks internally.
                let use_softline_wrap = has_trailing_line_comment
                    || Self::is_template_softline_expression(expr, has_comments);
                let inner = if use_softline_wrap {
                    d.concat(&[
                        d.indent(d.concat(&[d.softline(), full_expr_doc])),
                        d.softline(),
                    ])
                } else {
                    full_expr_doc
                };

                // Apply alignment based on quasi indent (Prettier's addAlignmentToDoc).
                // Three paths matching Prettier's template-literal.js:262-265:
                // 1. indent_size==0 && quasi ends with \n: expression starts at column 0
                //    on a new line — reset indent to absolute 0 (align(-∞)).
                // 2. indent_size>0: expression follows indented template content —
                //    apply full alignment (indent levels + reset).
                // 3. indent_size==0 && quasi doesn't end with \n: inline quasi (e.g. ", ")
                //    — preserve code context indent, no wrapping.
                let aligned = if indent_size == 0 && text.ends_with('\n') {
                    d.align(0, inner)
                } else if indent_size > 0 {
                    self.add_alignment_to_doc(inner, indent_size)
                } else {
                    inner
                };
                let group_doc =
                    d.concat(&[d.text("${"), aligned, d.line_suffix_boundary(), d.text("}")]);
                // Force break when trailing line comments are present — a line
                // comment on a flat line would swallow the closing `}`.
                parts.push(if has_trailing_line_comment {
                    d.group_break(group_doc)
                } else {
                    d.group(group_doc)
                });
            }
        }

        parts.push(d.text("`"));
        d.concat(&parts)
    }

    /// Get the visual indent size of the last line in template quasi text.
    /// Equivalent to Prettier's `getIndentSize`.
    fn get_template_indent_size(&self, text: &str) -> usize {
        if let Some(last_nl) = text.rfind('\n') {
            let after_nl = &text[last_nl + 1..];
            let ws_end = after_nl
                .chars()
                .take_while(|c| *c == '\t' || *c == ' ')
                .count();
            visual_width(&after_nl[..ws_end], TAB_WIDTH)
        } else {
            0
        }
    }

    /// Apply alignment to a doc based on indent size.
    /// Equivalent to Prettier's `addAlignmentToDoc(doc, size, tabWidth)`.
    ///
    /// When size > 0: wraps with `align(0)` to reset indent to absolute 0,
    /// then applies indent levels from zero. This is critical because
    /// after `literalline` in template content, the output column resets
    /// to 0 but the indent level on the command stack still carries the
    /// code context. Without the reset, softlines would break at the
    /// inherited code indent instead of the template's visual position.
    ///
    /// When size == 0: returns doc unchanged (matching Prettier's behavior).
    fn add_alignment_to_doc(&self, doc: DocId, size: usize) -> DocId {
        if size == 0 {
            return doc;
        }
        let d = self.d();
        // In prettier's useTabs renderer, align(n%tw) creates a WIDTH command
        // that adds lastTabs=1 + lastSpaces=n%tw. When followed by INDENT,
        // flushTabs() emits the pending tab and resetLast() drops the fractional
        // spaces. So align(r) + indent effectively rounds up to a full tab.
        // Match this by using ceiling division for the indent count.
        let n = size.div_ceil(TAB_WIDTH);
        let mut result = doc;
        for _ in 0..n {
            result = d.indent(result);
        }
        // Reset to absolute indent 0. Uses align(0) not dedent because
        // dedent only decrements by 1 (saturating_sub), while we need
        // to reset to 0 regardless of the current indent depth.
        d.align(0, result)
    }

    /// Convert a string with newlines into a doc with literalline between parts.
    /// Equivalent to Prettier's `replaceEndOfLine(text)` for TemplateElement nodes.
    /// `has_newline` is the caller's precomputed `text.contains('\n')` (the quasi's
    /// `has_newline` flag), so the common no-newline fast path skips re-scanning.
    fn replace_end_of_line(&self, text: &str, has_newline: bool) -> DocId {
        let d = self.d();
        if !has_newline {
            return d.text_owned(text.to_string());
        }
        let mut doc_parts = Vec::new();
        for (i, part) in text.split('\n').enumerate() {
            if i > 0 {
                doc_parts.push(d.literalline());
            }
            if !part.is_empty() {
                doc_parts.push(d.text_owned(part.to_string()));
            }
        }
        d.concat(&doc_parts)
    }

    /// Whether this expression type qualifies for softline wrapping at
    /// `${`/`}` boundaries. Matches Prettier's qualifying type list
    /// (template-literal.js:230-238).
    ///
    /// Qualifying types: simple expressions with no inherent block structure.
    /// Softline wrapping lets the `${}` group break when the line exceeds width.
    ///
    /// Non-qualifying types (CallExpression, ArrowFunctionExpression,
    /// TemplateLiteral, etc.): have internal break points or their own
    /// visual formatting. `${}` hugs while the expression breaks internally.
    fn is_template_softline_expression(expr: &Expression<'_>, has_comments: bool) -> bool {
        if has_comments {
            return true;
        }
        // Matches Prettier's qualifying types (template-literal.js:230-238):
        // Identifier, MemberExpression, ConditionalExpression, SequenceExpression,
        // isBinaryCastExpression (TSAsExpression, TSSatisfiesExpression),
        // isBinaryish (BinaryExpression — includes &&, ||, ??).
        // Plus additional simple types that have no internal break points.
        matches!(
            expr,
            Expression::Identifier(_)
                | Expression::Literal(_)
                | Expression::MemberExpression(_)
                | Expression::ConditionalExpression(_)
                | Expression::UnaryExpression(_)
                | Expression::UpdateExpression(_)
                | Expression::MetaProperty(_)
                | Expression::ThisExpression(_)
                | Expression::Super(_)
                | Expression::BinaryExpression(_)
                | Expression::SequenceExpression(_)
                | Expression::TSAsExpression(_)
                | Expression::TSSatisfiesExpression(_)
        )
    }

    /// Build comments doc for template literal interpolations
    ///
    /// Line comments get a hardline after them (required since `//` extends to EOL).
    /// Block comments get a space after (leading) or before (trailing).
    fn build_template_interpolation_comments(
        &self,
        comments: &[&crate::ast::internal::Comment],
        is_leading: bool,
    ) -> DocId {
        let d = self.d();
        if comments.is_empty() {
            return d.empty();
        }

        let mut parts = Vec::new();
        for comment in comments {
            if is_leading {
                // Leading comments: comment then separator
                parts.push(self.build_comment_doc(comment));
                if comment.is_block {
                    parts.push(d.text(" "));
                } else {
                    // Line comment requires hardline after
                    parts.push(d.hardline());
                }
            } else {
                // Trailing comments: space then comment
                parts.push(d.text(" "));
                parts.push(self.build_comment_doc(comment));
                // Line comments at the end still need hardline for proper formatting
                if !comment.is_block {
                    parts.push(d.hardline());
                }
            }
        }
        d.concat(&parts)
    }

    /// Build comments and expression doc for template interpolation with line comments.
    ///
    /// Uses `hardline` after line comments so the enclosing `indent()` wrapper handles indentation.
    /// Does NOT add hardline after the last trailing comment since the closing `}` literalline
    /// will provide that newline.
    fn build_template_comments_and_expr_doc(
        &self,
        leading_comments: &[&crate::ast::internal::Comment],
        expr_doc: DocId,
        trailing_comments: &[&crate::ast::internal::Comment],
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // Leading comments
        for comment in leading_comments {
            parts.push(self.build_comment_doc(comment));
            if comment.is_block {
                parts.push(d.text(" "));
            } else {
                parts.push(d.hardline());
            }
        }

        // Expression
        parts.push(expr_doc);

        // Trailing comments - don't add hardline after the last one since the
        // closing `}` has its own literalline that provides the newline
        let last_idx = trailing_comments.len().saturating_sub(1);
        for (i, comment) in trailing_comments.iter().enumerate() {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
            if !comment.is_block && i < last_idx {
                parts.push(d.hardline());
            }
        }

        d.concat(&parts)
    }

    /// Build a Doc for a tagged template expression
    pub(super) fn build_tagged_template_doc(
        &self,
        tagged: &crate::ast::internal::TaggedTemplateExpression<'_>,
    ) -> DocId {
        let d = self.d();

        // Wrap tag in parens if needed (e.g., ternary: `(a ? b : c)`template``, or an
        // optional chain: `` (a?.b)`x` `` — a chain can't be a tag per spec). A
        // non-null assertion that seals a parenthesized chain (`` (a?.b)!`x` ``) keeps
        // the parens via the sealed-base rendering. This must happen BEFORE adding
        // removed-paren comments so comments stay outside.
        let tag_doc = if let Some(sealed) = self.build_sealed_non_null_paren_doc(tagged.tag) {
            sealed
        } else if self.needs_parens(tagged.tag, ParenContext::TaggedTemplateTag) {
            d.parens(self.build_expression_doc(tagged.tag))
        } else {
            self.build_expression_doc(tagged.tag)
        };

        // Check for comments between removed parentheses and tag
        // e.g., (/* comment */ tag)`template` has tagged.span.start at '(' and tag.span.start at 'tag'
        let tag_doc = self.prepend_removed_paren_comments(
            tagged.span.start,
            tagged.tag.span().start,
            tag_doc,
        );

        let mut parts = vec![tag_doc];
        if let Some(type_args) = &tagged.type_arguments {
            // Preserve comments between tag and type args: `fn/* c */ <string>`template``
            let tag_end = tagged.tag.span().end;
            let ta_start = type_args.span.start;
            if let Some(doc) = self.build_name_to_type_params_comments_opt(
                tag_end,
                ta_start,
                CommentSpacing::Trailing,
            ) {
                parts.push(doc);
            }
            parts.push(self.build_type_parameter_instantiation_doc(type_args));
        }

        // Emit comments between tag (or type_args) and template literal
        // e.g., `foo /* c */ \`x\`` or `foo\n// c\n\`x\``
        let comment_start = tagged
            .type_arguments
            .as_ref()
            .map_or_else(|| tagged.tag.span().end, |ta| ta.span.end);
        let comment_end = tagged.quasi.span.start;
        let gap_comments: Vec<_> =
            comments_in_range(self.comments, comment_start, comment_end).collect();
        if !gap_comments.is_empty() {
            let mut prev_end = comment_start;
            let mut ends_with_hardline = false;
            for comment in &gap_comments {
                if comment.is_block {
                    let on_own_line = self.has_newline_between(prev_end, comment.span.start);
                    if on_own_line || ends_with_hardline {
                        parts.push(d.hardline());
                        parts.push(self.build_comment_doc(comment));
                        // Check if there's a newline after this block comment
                        ends_with_hardline =
                            self.has_newline_between(comment.span.end, comment_end);
                    } else {
                        parts.push(d.text(" "));
                        parts.push(self.build_comment_doc(comment));
                        ends_with_hardline = false;
                    }
                } else {
                    // Line comment: hardline or space before, always hardline after
                    let on_own_line = self.has_newline_between(prev_end, comment.span.start);
                    if on_own_line || ends_with_hardline {
                        parts.push(d.hardline());
                    } else {
                        parts.push(d.text(" "));
                    }
                    parts.push(self.build_comment_doc(comment));
                    ends_with_hardline = true;
                }
                prev_end = comment.span.end;
            }
            if ends_with_hardline {
                parts.push(d.hardline());
            } else {
                parts.push(d.text(" "));
            }
        }

        parts.push(self.build_template_literal_doc(&tagged.quasi));
        d.concat(&parts)
    }
}
