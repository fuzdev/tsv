// Conditional (ternary) expression printing for TypeScript
//
// Handles: a ? b : c, nested ternaries, comments in ternaries

use super::{Printer, template_literal_has_newlines};
use crate::ast::internal;
use tsv_lang::doc::arena::DocId;

/// Check if an expression is a nullish coalescing expression (`??`)
///
/// Prettier wraps `??` in parens when inside a ternary for clarity.
pub(super) fn is_nullish_coalescing(expr: &internal::Expression) -> bool {
    matches!(
        expr,
        internal::Expression::BinaryExpression(bin)
            if bin.operator == internal::BinaryOperator::QuestionQuestion
    )
}

/// Check if an expression is a template literal containing newlines
///
/// When a template literal contains embedded newlines in its quasi strings,
/// it should be treated as "multiline" for formatting purposes. This is used
/// to force ternaries to break when their consequent or alternate is multiline.
fn is_multiline_template_literal(expr: &internal::Expression) -> bool {
    matches!(expr, internal::Expression::TemplateLiteral(t) if template_literal_has_newlines(t))
}

impl<'a> Printer<'a> {
    /// Build a Doc for a conditional expression with wrapping support
    pub(super) fn build_conditional_doc_with_wrapping(
        &self,
        cond: &internal::ConditionalExpression,
    ) -> DocId {
        self.build_conditional_doc_impl(cond, false)
    }

    /// Implementation of conditional doc building
    ///
    /// `is_chained` indicates this conditional is nested within a parent conditional
    /// (either in consequent when broken, or in alternate). When chained, we don't
    /// wrap in a new group, so the parent's break decision cascades to this one.
    fn build_conditional_doc_impl(
        &self,
        cond: &internal::ConditionalExpression,
        is_chained: bool,
    ) -> DocId {
        let d = self.d();
        let test_end = cond.test.span().end;
        let consequent_start = cond.consequent.span().start;
        let consequent_end = cond.consequent.span().end;
        let alternate_start = cond.alternate.span().start;

        // Check for line comments that force breaking
        let has_line_comments = self.has_line_comments_between(test_end, consequent_start)
            || self.has_line_comments_between(consequent_end, alternate_start);

        // Check for multiline template literals in test, consequent, or alternate
        // Template literals with embedded newlines should force the ternary to break,
        // even though those newlines don't appear in the doc structure.
        let has_multiline_template = is_multiline_template_literal(&cond.test)
            || is_multiline_template_literal(&cond.consequent)
            || is_multiline_template_literal(&cond.alternate);

        // If there are line comments or multiline template literals, use a breaking layout.
        // Block comments after ? or : are handled inline in the non-breaking path.
        if has_line_comments || has_multiline_template {
            return self.build_conditional_doc_with_line_comments(cond, is_chained);
        }

        // Nullish coalescing, assignments, and await in test position need parens for clarity
        // Prettier: needs-parens.js — AwaitExpression needs parens when parent is
        // ConditionalExpression and key is "test" (await has higher precedence than ?:
        // but parens aid readability)
        let test = self.build_expression_doc(&cond.test);
        let test = if is_nullish_coalescing(&cond.test)
            || matches!(
                &*cond.test,
                internal::Expression::AssignmentExpression(_)
                    | internal::Expression::AwaitExpression(_)
            ) {
            d.parens(test)
        } else {
            test
        };
        let consequent = self.build_expression_doc(&cond.consequent);

        // Split comments around ? and : operators.
        // Comments before ? go after test, comments after ? go before consequent,
        // comments after : go before alternate.
        let question_pos = self.find_char_position(test_end, consequent_start, '?');
        let colon_pos = self.find_char_position(consequent_end, alternate_start, ':');

        // Comments between test and ?
        let comments_before_question = if let Some(q) = question_pos {
            self.build_inline_comments_between_doc(test_end, q)
        } else {
            self.build_inline_comments_between_doc(test_end, consequent_start)
        };

        // Comments between ? and consequent (e.g., `b ? /* comment */ c`)
        // Trailing space so the comment doesn't touch the consequent
        let comments_after_question = if let Some(q) = question_pos {
            self.build_inline_comments_between_doc_trailing_space(q + 1, consequent_start)
        } else {
            d.empty()
        };

        // Comments between : and alternate (e.g., `c : /* comment */ d`)
        let comments_after_colon = if let Some(c) = colon_pos {
            self.build_inline_comments_between_doc_trailing_space(c + 1, alternate_start)
        } else {
            d.empty()
        };

        // Handle nested conditional in consequent specially:
        // - When flat: parens for parsing `a ? (b ? c : d) : e`
        // - When broken: continue chain without parens (same as alternate)
        //
        // Prettier wraps each branch in indent() so that multiline content
        // (like arrow block bodies) gets proper nesting. Exception: nested
        // conditionals handle their own indentation, so no extra wrapper.
        let consequent_doc =
            if let internal::Expression::ConditionalExpression(nested) = &*cond.consequent {
                // Broken version: continue chain without parens
                let broken_consequent = self.build_conditional_doc_impl(nested, true);
                if d.will_break(consequent) {
                    // Consequent forces breaking (e.g., line comments produce hardlines).
                    // Skip if_break and use broken layout directly — the outer group
                    // will break because broken_consequent contains hardlines.
                    // Matches Prettier's willBreak(consequentDoc) → shouldBreak check
                    // in printTernaryOld (ternary-old.js).
                    broken_consequent
                } else {
                    // Normal if_break: parens when flat, chain when broken
                    let flat_consequent = d.parens(consequent);
                    d.if_break(broken_consequent, flat_consequent)
                }
            } else if matches!(
                &*cond.consequent,
                internal::Expression::TSAsExpression(_)
                    | internal::Expression::TSSatisfiesExpression(_)
                    | internal::Expression::AssignmentExpression(_)
            ) || is_nullish_coalescing(&cond.consequent)
            {
                d.indent(d.parens(consequent))
            } else {
                d.indent(consequent)
            };

        // Handle nested conditional in alternate: continue the chain
        // - Nested conditional does NOT need parens: `a ? b : c ? d : e`
        //   (right-associative, so naturally parsed as `a ? b : (c ? d : e)`)
        // - `as`/`satisfies` need parens to avoid `:` ambiguity: `a ? b : (c as T)`
        // - `??` needs parens for clarity: `a ? b : (c ?? d)`
        let alternate_doc =
            if let internal::Expression::ConditionalExpression(nested) = &*cond.alternate {
                // Recursively build as chained (no group wrapper, no parens)
                // No indent wrapper - nested conditional has its own structure
                self.build_conditional_doc_impl(nested, true)
            } else {
                let alternate = self.build_expression_doc(&cond.alternate);
                let alternate = if matches!(
                    &*cond.alternate,
                    internal::Expression::TSAsExpression(_)
                        | internal::Expression::TSSatisfiesExpression(_)
                        | internal::Expression::AssignmentExpression(_)
                ) || is_nullish_coalescing(&cond.alternate)
                {
                    d.parens(alternate)
                } else {
                    alternate
                };
                d.indent(alternate)
            };

        let inner = d.concat(&[
            test,
            comments_before_question,
            d.indent(d.concat(&[
                d.line(),
                d.text("? "),
                comments_after_question,
                consequent_doc,
                d.line(),
                d.text(": "),
                comments_after_colon,
                alternate_doc,
            ])),
        ]);

        // If chained (nested in another conditional), don't wrap in group
        // This allows the parent's break decision to cascade
        if is_chained { inner } else { d.group(inner) }
    }

    /// Build a conditional expression doc when there are line comments
    ///
    /// Line comments force the ternary to break because they end at newline.
    /// This produces:
    /// ```js
    /// test // comment
    ///   ? // comment
    ///     consequent // comment
    ///   : // comment
    ///     alternate
    /// ```
    fn build_conditional_doc_with_line_comments(
        &self,
        cond: &internal::ConditionalExpression,
        _is_chained: bool,
    ) -> DocId {
        let d = self.d();
        let test_end = cond.test.span().end;
        let consequent_start = cond.consequent.span().start;
        let consequent_end = cond.consequent.span().end;
        let alternate_start = cond.alternate.span().start;

        // Build test expression with parens if needed (same logic as non-breaking path)
        let test = self.build_expression_doc(&cond.test);
        let test = if is_nullish_coalescing(&cond.test)
            || matches!(
                &*cond.test,
                internal::Expression::AssignmentExpression(_)
                    | internal::Expression::AwaitExpression(_)
            ) {
            d.parens(test)
        } else {
            test
        };

        // Find the ? and : positions for proper comment categorization
        let question_pos = self.find_char_position(test_end, consequent_start, '?');
        let colon_pos = self.find_char_position(consequent_end, alternate_start, ':');

        let mut parts = vec![test];

        // Comments between test and ? (inline after test)
        let comments_before_q_end = question_pos.unwrap_or(consequent_start);
        for comment in tsv_lang::comments_in_range(self.comments, test_end, comments_before_q_end) {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }

        // Start the indented part with ? on new line
        let mut q_parts = vec![d.hardline(), d.text("?")];

        // Comments between ? and consequent
        let mut has_line_comment_before_consequent = false;
        if let Some(q_pos) = question_pos {
            for comment in tsv_lang::comments_in_range(self.comments, q_pos + 1, consequent_start) {
                q_parts.push(d.text(" "));
                q_parts.push(self.build_comment_doc(comment));
                if !comment.is_block {
                    has_line_comment_before_consequent = true;
                }
            }
        }

        // Consequent expression
        let consequent = self.build_expression_doc(&cond.consequent);
        if has_line_comment_before_consequent {
            // Line comment needs hardline before consequent
            q_parts.push(d.hardline());
            q_parts.push(d.text(self.config.indent));
            q_parts.push(consequent);
        } else {
            // Block comment or no comment - space then consequent
            q_parts.push(d.text(" "));
            q_parts.push(d.indent(consequent));
        }

        // Comments between consequent and : (inline after consequent)
        let comments_before_colon_end = colon_pos.unwrap_or(alternate_start);
        for comment in
            tsv_lang::comments_in_range(self.comments, consequent_end, comments_before_colon_end)
        {
            if comment.is_block {
                // Block comments count toward width
                q_parts.push(d.text(" "));
                q_parts.push(self.build_comment_doc(comment));
            } else {
                // Line comments use line_suffix to exclude from width calculations
                q_parts.push(self.build_trailing_line_comment_doc(comment));
            }
        }

        // : on new line
        q_parts.push(d.hardline());
        q_parts.push(d.text(":"));

        // Comments between : and alternate
        let mut has_line_comment_before_alternate = false;
        if let Some(c_pos) = colon_pos {
            for comment in tsv_lang::comments_in_range(self.comments, c_pos + 1, alternate_start) {
                q_parts.push(d.text(" "));
                q_parts.push(self.build_comment_doc(comment));
                if !comment.is_block {
                    has_line_comment_before_alternate = true;
                }
            }
        }

        // Alternate expression - nested conditionals cascade the break without extra indent
        let alternate_doc =
            if let internal::Expression::ConditionalExpression(nested) = &*cond.alternate {
                // Recursively use breaking layout - no indent wrapper (has its own structure)
                self.build_conditional_doc_with_line_comments(nested, true)
            } else {
                // Regular expressions get indent wrapper
                d.indent(self.build_expression_doc(&cond.alternate))
            };

        if has_line_comment_before_alternate {
            q_parts.push(d.hardline());
            q_parts.push(d.text(self.config.indent));
        } else {
            q_parts.push(d.text(" "));
        }
        q_parts.push(alternate_doc);

        parts.push(d.indent(d.concat(&q_parts)));

        d.concat(&parts)
    }

    /// Find the position of a character in source, skipping over comments
    fn find_char_position(&self, start: u32, end: u32, target: char) -> Option<u32> {
        super::analysis::find_char_skipping_comments(
            self.source.as_bytes(),
            start as usize,
            end as usize,
            target as u8,
        )
        .map(|pos| pos as u32)
    }
}
