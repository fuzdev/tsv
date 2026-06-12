// Conditional (ternary) expression printing for TypeScript
//
// Handles: a ? b : c, nested ternaries, comments in ternaries

use crate::ast::internal;
use crate::printer::{Printer, template_literal_has_newlines};
use tsv_lang::doc::arena::DocId;

/// Check if an expression is a nullish coalescing expression (`??`)
///
/// Prettier wraps `??` in parens when inside a ternary for clarity.
pub(in crate::printer) fn is_nullish_coalescing(expr: &internal::Expression) -> bool {
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
    pub(in crate::printer) fn build_conditional_doc_with_wrapping(
        &self,
        cond: &internal::ConditionalExpression,
    ) -> DocId {
        self.build_conditional_doc_impl(cond, false, false)
    }

    /// Build a Doc for a conditional expression in return/throw/call/new context.
    ///
    /// When the ternary's parent is ReturnStatement, ThrowStatement, CallExpression,
    /// or NewExpression, binary expressions in the test position use continuation
    /// indent. This matches Prettier's shouldNotIndent (binaryish.js:109-113) which
    /// exempts binaries from indent only when the grandparent is NOT one of these types.
    pub(in crate::printer) fn build_conditional_doc_with_binary_test_indent(
        &self,
        cond: &internal::ConditionalExpression,
    ) -> DocId {
        self.build_conditional_doc_impl(cond, false, true)
    }

    /// Implementation of conditional doc building
    ///
    /// `is_chained` indicates this conditional is nested within a parent conditional
    /// (either in consequent when broken, or in alternate). When chained, we don't
    /// wrap in a new group, so the parent's break decision cascades to this one.
    ///
    /// `indent_binary_test` indicates the ternary is inside a return/throw/call/new
    /// statement, so binary expressions in the test position should use continuation
    /// indent (matching Prettier's shouldNotIndent = false for these grandparents).
    fn build_conditional_doc_impl(
        &self,
        cond: &internal::ConditionalExpression,
        is_chained: bool,
        indent_binary_test: bool,
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

        // Prettier's shouldNotIndent (binaryish.js:109-113) exempts binaries whose
        // parent is ConditionalExpression from continuation indent, UNLESS the
        // grandparent is ReturnStatement, ThrowStatement, CallExpression, or
        // NewExpression. In those cases, shouldNotIndent = false and the binary
        // gets indent(rest) for its continuation lines.
        //
        // In embedded contexts (Svelte attributes), the grandparent is a template
        // node (none of the above), so shouldNotIndent = true → no indent.
        let test = if let internal::Expression::BinaryExpression(binary) = &*cond.test {
            if self.embed.is_embedded() {
                // Embedded: shouldNotIndent = true (grandparent is Svelte template node)
                self.build_binary_chain_doc(binary)
            } else if indent_binary_test {
                // Grandparent is return/throw/call/new: shouldNotIndent = false
                self.build_binary_chain_doc_with_continuation_indent(binary)
            } else {
                // Default: shouldNotIndent = true (grandparent is assignment, variable, etc.)
                self.build_binary_chain_doc(binary)
            }
        } else {
            self.build_expression_doc(&cond.test)
        };
        // Several test-position expressions get parens (Prettier: needs-parentheses.js).
        // For arrow/yield it is semantic: without parens the body absorbs the ternary
        // (`() => 1 ? x : y` parses as `() => (1 ? x : y)`; `yield 1 ? x : y` as
        // `yield (1 ? x : y)`). For `as`/`satisfies` it is clarity parens (same AST —
        // they bind tighter than `?:`), matching the consequent/alternate arms below.
        let test = if is_nullish_coalescing(&cond.test)
            || matches!(
                &*cond.test,
                internal::Expression::AssignmentExpression(_)
                    | internal::Expression::AwaitExpression(_)
                    | internal::Expression::ArrowFunctionExpression(_)
                    | internal::Expression::YieldExpression(_)
                    | internal::Expression::TSAsExpression(_)
                    | internal::Expression::TSSatisfiesExpression(_)
            ) {
            d.parens(test)
        } else {
            test
        };
        // Prettier's shouldNotIndent (binaryish.js:109-113) also applies to binaries
        // in consequent/alternate positions: when parent is ConditionalExpression and
        // grandparent is ReturnStatement/ThrowStatement/CallExpression/NewExpression,
        // shouldNotIndent = false → binary gets indent(rest) for continuation lines.
        // In assignment/variable contexts, shouldNotIndent = true → flat (no indent).
        // Bound the consequent's own paren-comment scan at its end — the
        // consequent-to-`:` comment is emitted by `comments_before_colon` below, so a
        // wider boundary would double-emit it.
        let consequent = self.build_ternary_branch_expr_doc(
            &cond.consequent,
            indent_binary_test,
            consequent_end,
        );

        // Split comments around ? and : operators.
        // Comments before ? go after test, comments after ? go before consequent,
        // comments after : go before alternate.
        let question_pos = self.find_char_outside_comments(test_end, consequent_start, b'?');
        let colon_pos = self.find_char_outside_comments(consequent_end, alternate_start, b':');

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

        // Comments between consequent and : (e.g., `b ? c /* comment */ : d`)
        let comments_before_colon = if let Some(c) = colon_pos {
            self.build_inline_comments_between_doc(consequent_end, c)
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
                let broken_consequent =
                    self.build_conditional_doc_impl(nested, true, indent_binary_test);
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
                self.build_conditional_doc_impl(nested, true, indent_binary_test)
            } else {
                let alternate = self.build_ternary_branch_expr_doc(
                    &cond.alternate,
                    indent_binary_test,
                    cond.span.end,
                );
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
                comments_before_colon,
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
        let question_pos = self.find_char_outside_comments(test_end, consequent_start, b'?');
        let colon_pos = self.find_char_outside_comments(consequent_end, alternate_start, b':');

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
        // When multiple comments exist, each subsequent one goes on its own line
        let mut has_line_comment_before_consequent = false;
        let mut has_prev_comment_after_q = false;
        if let Some(q_pos) = question_pos {
            for comment in tsv_lang::comments_in_range(self.comments, q_pos + 1, consequent_start) {
                if has_prev_comment_after_q {
                    // Subsequent comments go on their own line
                    q_parts.push(d.hardline());
                    q_parts.push(d.text(tsv_lang::INDENT));
                } else {
                    q_parts.push(d.text(" "));
                }
                q_parts.push(self.build_comment_doc(comment));
                has_prev_comment_after_q = true;
                if !comment.is_block {
                    has_line_comment_before_consequent = true;
                }
            }
        }

        // Consequent expression — when the outer ternary enters breaking layout
        // (line comments or multiline templates), nested conditionals in the
        // consequent must also break. Without group_break, the inner ternary's
        // group stays flat (content fits on one line), but Prettier cascades
        // the break from the parent to the entire ternary chain.
        let (consequent, is_nested_cond) =
            if let internal::Expression::ConditionalExpression(nested) = &*cond.consequent {
                let chained = self.build_conditional_doc_impl(nested, true, false);
                (d.group_break(chained), true)
            } else {
                (self.build_expression_doc(&cond.consequent), false)
            };
        if has_line_comment_before_consequent {
            // Line comment — consequent on new line
            q_parts.push(d.hardline());
            q_parts.push(d.text(tsv_lang::INDENT));
            q_parts.push(consequent);
        } else {
            // Single block comment or no comment - space then consequent
            q_parts.push(d.text(" "));
            if is_nested_cond {
                // Nested conditional handles its own indent via chained structure
                q_parts.push(consequent);
            } else {
                q_parts.push(d.indent(consequent));
            }
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
        // When multiple comments exist, each subsequent one goes on its own line
        let mut has_line_comment_before_alternate = false;
        let mut has_prev_comment_after_colon = false;
        if let Some(c_pos) = colon_pos {
            for comment in tsv_lang::comments_in_range(self.comments, c_pos + 1, alternate_start) {
                if has_prev_comment_after_colon {
                    // Subsequent comments go on their own line
                    q_parts.push(d.hardline());
                    q_parts.push(d.text(tsv_lang::INDENT));
                } else {
                    q_parts.push(d.text(" "));
                }
                q_parts.push(self.build_comment_doc(comment));
                has_prev_comment_after_colon = true;
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
            q_parts.push(d.text(tsv_lang::INDENT));
        } else {
            q_parts.push(d.text(" "));
        }
        q_parts.push(alternate_doc);

        parts.push(d.indent(d.concat(&q_parts)));

        d.concat(&parts)
    }

    /// Build expression doc for a ternary branch (consequent/alternate).
    ///
    /// When `indent_binary` is true (grandparent is return/throw/call/new),
    /// binary expressions use continuation indent matching Prettier's
    /// shouldNotIndent=false for these contexts (binaryish.js:109-113).
    fn build_ternary_branch_expr_doc(
        &self,
        expr: &internal::Expression,
        indent_binary: bool,
        boundary_end: u32,
    ) -> DocId {
        if indent_binary && let internal::Expression::BinaryExpression(binary) = expr {
            return self.build_binary_chain_doc_with_continuation_indent(binary);
        }
        self.build_expression_doc_with_paren_comments(expr, boundary_end)
    }
}
