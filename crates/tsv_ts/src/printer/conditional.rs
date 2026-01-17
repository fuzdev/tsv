// Conditional (ternary) expression printing for TypeScript
//
// Handles: a ? b : c, nested ternaries, comments in ternaries

use super::{Printer, template_literal_has_newlines};
use crate::ast::internal;
use tsv_lang::doc::{self, Doc};

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
    ) -> Doc {
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
    ) -> Doc {
        let test_end = cond.test.span().end;
        let consequent_start = cond.consequent.span().start;
        let consequent_end = cond.consequent.span().end;
        let alternate_start = cond.alternate.span().start;

        // Check for line comments that force breaking
        let has_line_comments = self.has_line_comments_between(test_end, consequent_start)
            || self.has_line_comments_between(consequent_end, alternate_start);

        // Check for comments between ? and consequent, or : and alternate
        // These are comments that appear after the operator, which forces breaking
        // e.g., `cond ? /* comment */ a : b` → breaks
        let has_comments_after_operators = self.has_comments_after_ternary_operator(
            test_end,
            consequent_start,
            consequent_end,
            alternate_start,
        );

        // Check for multiline template literals in test, consequent, or alternate
        // Template literals with embedded newlines should force the ternary to break,
        // even though those newlines don't appear in the doc structure.
        let has_multiline_template = is_multiline_template_literal(&cond.test)
            || is_multiline_template_literal(&cond.consequent)
            || is_multiline_template_literal(&cond.alternate);

        // If there are line comments, operator comments, or multiline template literals,
        // use a breaking layout
        if has_line_comments || has_comments_after_operators || has_multiline_template {
            return self.build_conditional_doc_with_line_comments(cond, is_chained);
        }

        // Nullish coalescing and assignments in test position need parens for clarity
        let test = self.build_expression_doc(&cond.test);
        let test = if is_nullish_coalescing(&cond.test)
            || matches!(&*cond.test, internal::Expression::AssignmentExpression(_))
        {
            doc::parens(test)
        } else {
            test
        };
        let consequent = self.build_expression_doc(&cond.consequent);

        // Check for comments between test and ? (before consequent)
        let comments_after_test =
            self.build_inline_comments_between_doc(test_end, consequent_start);

        // Handle nested conditional in consequent specially:
        // - When flat: parens for parsing `a ? (b ? c : d) : e`
        // - When broken: continue chain without parens (same as alternate)
        //
        // Prettier wraps each branch in indent() so that multiline content
        // (like arrow block bodies) gets proper nesting. Exception: nested
        // conditionals handle their own indentation, so no extra wrapper.
        let consequent_doc =
            if let internal::Expression::ConditionalExpression(nested) = &*cond.consequent {
                // Flat version: parens around the nested conditional
                let flat_consequent = doc::parens(consequent);
                // Broken version: continue chain without parens
                let broken_consequent = self.build_conditional_doc_impl(nested, true);
                // No indent wrapper - nested conditional has its own structure
                doc::if_break(broken_consequent, flat_consequent)
            } else if matches!(
                &*cond.consequent,
                internal::Expression::TSAsExpression(_)
                    | internal::Expression::TSSatisfiesExpression(_)
                    | internal::Expression::AssignmentExpression(_)
            ) || is_nullish_coalescing(&cond.consequent)
            {
                doc::indent(doc::parens(consequent))
            } else {
                doc::indent(consequent)
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
                    doc::parens(alternate)
                } else {
                    alternate
                };
                doc::indent(alternate)
            };

        let inner = doc::concat(vec![
            test,
            comments_after_test,
            doc::indent(doc::concat(vec![
                doc::line(),
                doc::text("? "),
                consequent_doc,
                doc::line(),
                doc::text(": "),
                alternate_doc,
            ])),
        ]);

        // If chained (nested in another conditional), don't wrap in group
        // This allows the parent's break decision to cascade
        if is_chained { inner } else { doc::group(inner) }
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
    ) -> Doc {
        let test_end = cond.test.span().end;
        let consequent_start = cond.consequent.span().start;
        let consequent_end = cond.consequent.span().end;
        let alternate_start = cond.alternate.span().start;

        // Build test expression with parens if needed
        let test = self.build_expression_doc(&cond.test);
        let test = if is_nullish_coalescing(&cond.test)
            || matches!(&*cond.test, internal::Expression::AssignmentExpression(_))
        {
            doc::parens(test)
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
            parts.push(doc::text(" "));
            parts.push(self.build_comment_doc(comment));
        }

        // Start the indented part with ? on new line
        let mut q_parts = vec![doc::hardline(), doc::text("?")];

        // Comments between ? and consequent
        let mut has_line_comment_before_consequent = false;
        if let Some(q_pos) = question_pos {
            for comment in tsv_lang::comments_in_range(self.comments, q_pos + 1, consequent_start) {
                q_parts.push(doc::text(" "));
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
            q_parts.push(doc::hardline());
            q_parts.push(doc::text(self.config.indent));
            q_parts.push(consequent);
        } else {
            // Block comment or no comment - space then consequent
            q_parts.push(doc::text(" "));
            q_parts.push(doc::indent(consequent));
        }

        // Comments between consequent and : (inline after consequent)
        let comments_before_colon_end = colon_pos.unwrap_or(alternate_start);
        for comment in
            tsv_lang::comments_in_range(self.comments, consequent_end, comments_before_colon_end)
        {
            q_parts.push(doc::text(" "));
            q_parts.push(self.build_comment_doc(comment));
        }

        // : on new line
        q_parts.push(doc::hardline());
        q_parts.push(doc::text(":"));

        // Comments between : and alternate
        let mut has_line_comment_before_alternate = false;
        if let Some(c_pos) = colon_pos {
            for comment in tsv_lang::comments_in_range(self.comments, c_pos + 1, alternate_start) {
                q_parts.push(doc::text(" "));
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
                doc::indent(self.build_expression_doc(&cond.alternate))
            };

        if has_line_comment_before_alternate {
            q_parts.push(doc::hardline());
            q_parts.push(doc::text(self.config.indent));
        } else {
            q_parts.push(doc::text(" "));
        }
        q_parts.push(alternate_doc);

        parts.push(doc::indent(doc::concat(q_parts)));

        doc::concat(parts)
    }

    /// Check if there are comments between ternary operators and their operands
    ///
    /// Returns true if:
    /// - There's a comment between `?` and the consequent expression
    /// - There's a comment between `:` and the alternate expression
    ///
    /// These comments force the ternary to break, even if it would otherwise fit on one line.
    fn has_comments_after_ternary_operator(
        &self,
        test_end: u32,
        consequent_start: u32,
        consequent_end: u32,
        alternate_start: u32,
    ) -> bool {
        // Find ? position between test and consequent
        let question_pos = self.find_char_position(test_end, consequent_start, '?');

        // Check for comments between ? and consequent
        if let Some(q_pos) = question_pos
            && tsv_lang::comments_in_range(self.comments, q_pos + 1, consequent_start)
                .next()
                .is_some()
        {
            return true;
        }

        // Find : position between consequent and alternate
        let colon_pos = self.find_char_position(consequent_end, alternate_start, ':');

        // Check for comments between : and alternate
        if let Some(c_pos) = colon_pos
            && tsv_lang::comments_in_range(self.comments, c_pos + 1, alternate_start)
                .next()
                .is_some()
        {
            return true;
        }

        false
    }

    /// Find the position of a character in source, skipping over comments
    fn find_char_position(&self, start: u32, end: u32, target: char) -> Option<u32> {
        let search_range = &self.source[start as usize..end as usize];
        let bytes = search_range.as_bytes();
        let target_byte = target as u8;
        let mut i = 0;

        while i < bytes.len() {
            // Skip over block comments
            if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
                continue;
            }

            if bytes[i] == target_byte {
                return Some(start + i as u32);
            }
            i += 1;
        }

        None
    }
}
