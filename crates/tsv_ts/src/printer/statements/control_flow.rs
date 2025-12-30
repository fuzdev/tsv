// Control flow statement printing for TypeScript

use super::super::Printer;
use crate::ast::internal::{self, Statement};
use tsv_lang::{SymbolToU32, doc};

/// Check if a statement can be printed inline after `if (cond)` without a newline.
///
/// Block, expression, break, continue, return, throw, and empty statements stay inline.
/// Other statements (if, for, while, etc.) go on a new line with indent.
fn is_inline_consequent(stmt: &Statement) -> bool {
    matches!(
        stmt,
        Statement::BlockStatement(_)
            | Statement::ExpressionStatement(_)
            | Statement::BreakStatement(_)
            | Statement::ContinueStatement(_)
            | Statement::ReturnStatement(_)
            | Statement::ThrowStatement(_)
            | Statement::EmptyStatement(_)
    )
}

/// Check if a statement can be printed inline after `else` without a newline.
///
/// Same as `is_inline_consequent` but also allows IfStatement for else-if chains.
fn is_inline_alternate(stmt: &Statement) -> bool {
    is_inline_consequent(stmt) || matches!(stmt, Statement::IfStatement(_))
}

impl<'a> Printer<'a> {
    // ========================================================================
    // Control Flow Statement Printers
    // ========================================================================

    pub(super) fn print_if_statement(&mut self, stmt: &internal::IfStatement) {
        // Check for comments between consequent and alternate that need special handling
        let has_if_else_comments = stmt.alternate.as_ref().is_some_and(|alt| {
            let consequent_end = stmt.consequent.span().end;
            let alternate_start = alt.span().start;
            self.has_comments_between(consequent_end, alternate_start)
        });

        if has_if_else_comments {
            // Use imperative printing for comment handling between } and else
            self.print_if_statement_imperative(stmt);
        } else {
            // Use doc-based printing for proper line-width wrapping
            let if_doc = self.build_if_statement_with_wrapping_doc(stmt);
            // Body already includes its own punctuation (semicolon), no extra margin needed
            self.write_doc(&if_doc);
        }
    }

    /// Print if statement imperatively (for cases with comments between } and else)
    fn print_if_statement_imperative(&mut self, stmt: &internal::IfStatement) {
        self.write("if (");
        self.print_expression(&stmt.test);
        self.write(")");

        // Print consequent
        let is_block = matches!(stmt.consequent.as_ref(), Statement::BlockStatement(_));
        if is_inline_consequent(&stmt.consequent) {
            if !matches!(stmt.consequent.as_ref(), Statement::EmptyStatement(_)) {
                self.write(" ");
            }
            if let Statement::BlockStatement(block) = stmt.consequent.as_ref() {
                self.print_block_statement_expand_empty(block);
            } else {
                self.print_statement(&stmt.consequent);
            }
        } else {
            self.write("\n");
            self.indent_level += 1;
            self.write_indent();
            self.print_statement(&stmt.consequent);
            self.indent_level -= 1;
        }

        if let Some(alternate) = &stmt.alternate {
            let consequent_end = stmt.consequent.span().end;
            let alternate_start = alternate.span().start;
            let has_comments_between = self.has_comments_between(consequent_end, alternate_start);

            if is_block {
                if has_comments_between {
                    self.print_if_else_comments(consequent_end, alternate_start);
                } else {
                    self.write(" else ");
                }
            } else {
                self.write("\n");
                self.write_indent();
                self.write("else ");
            }

            if is_inline_alternate(alternate) {
                if let Statement::BlockStatement(block) = alternate.as_ref() {
                    self.print_block_statement_expand_empty(block);
                } else {
                    self.print_statement(alternate);
                }
            } else {
                self.write("\n");
                self.indent_level += 1;
                self.write_indent();
                self.print_statement(alternate);
                self.indent_level -= 1;
            }
        }
    }

    /// Build a doc for an if statement with proper line-width wrapping
    ///
    /// Matches Prettier's architecture from estree.js:
    /// ```js
    /// group([
    ///   "if (",
    ///   group([indent([softline, test]), softline]),  // inner group for condition
    ///   ")",
    ///   adjustClause(consequent),  // body handling
    /// ])
    /// ```
    fn build_if_statement_with_wrapping_doc(&self, stmt: &internal::IfStatement) -> doc::Doc {
        // For binary expressions in conditions, use ungrouped version so parent group controls breaking
        // This matches Prettier's behavior where operators break together with the condition
        let test_doc = self.build_condition_doc(&stmt.test);

        let is_block = matches!(stmt.consequent.as_ref(), Statement::BlockStatement(_));
        let is_empty = matches!(stmt.consequent.as_ref(), Statement::EmptyStatement(_));

        // Condition group: group([indent([softline, test]), softline])
        // - This group decides whether the condition breaks (operators go to new lines)
        // - Binary expression is ungrouped, so its lines follow this group's decision
        // - The outer group (whole if statement) decides whether body goes to new line
        let condition_group = doc::group(doc::concat(vec![
            doc::indent(doc::concat(vec![doc::softline(), test_doc])),
            doc::softline(),
        ]));

        if is_block {
            // Block consequent: group(["if (" + condition + ") " + block])
            // Outer group controls whether the whole if statement breaks
            let mut parts = vec![doc::text("if ("), condition_group, doc::text(") ")];

            if let Statement::BlockStatement(block) = stmt.consequent.as_ref() {
                parts.push(self.build_block_statement_expand_empty_doc(block));
            }

            // Handle else clause
            if let Some(alternate) = &stmt.alternate {
                parts.push(doc::text(" else "));
                if let Statement::BlockStatement(block) = alternate.as_ref() {
                    parts.push(self.build_block_statement_expand_empty_doc(block));
                } else if is_inline_alternate(alternate) {
                    parts.push(self.build_statement_doc(alternate));
                } else {
                    parts.push(doc::hardline());
                    parts.push(doc::indent(self.build_statement_doc(alternate)));
                }
            }

            // Outer group for the whole if statement
            doc::group(doc::concat(parts))
        } else if is_empty {
            // Empty statement: `if (cond);`
            doc::group(doc::concat(vec![
                doc::text("if ("),
                condition_group,
                doc::text(");"),
            ]))
        } else {
            // Non-block consequent: use adjustClause equivalent
            // Prettier's adjustClause returns: indent([line, clause])
            // - When flat: line becomes space -> `if (cond) a;`
            // - When broken: line becomes newline + indent -> `if (cond)\n\ta;`
            let consequent_doc = self.build_statement_doc(&stmt.consequent);
            let adjust_clause = doc::indent(doc::concat(vec![doc::line(), consequent_doc]));

            let mut parts = vec![doc::group(doc::concat(vec![
                doc::text("if ("),
                condition_group,
                doc::text(")"),
                adjust_clause,
            ]))];

            // Handle else clause for non-block consequent
            if let Some(alternate) = &stmt.alternate {
                parts.push(doc::hardline());
                parts.push(doc::text("else "));
                if is_inline_alternate(alternate) {
                    if let Statement::BlockStatement(block) = alternate.as_ref() {
                        parts.push(self.build_block_statement_expand_empty_doc(block));
                    } else {
                        parts.push(self.build_statement_doc(alternate));
                    }
                } else {
                    parts.push(doc::hardline());
                    parts.push(doc::indent(self.build_statement_doc(alternate)));
                }
            }

            doc::concat(parts)
        }
    }

    /// Build a doc for a condition expression (if/while/for test)
    ///
    /// For binary expressions, uses ungrouped version so parent group controls breaking.
    /// This ensures all operands break together when the condition exceeds print width.
    fn build_condition_doc(&self, expr: &internal::Expression) -> doc::Doc {
        match expr {
            internal::Expression::BinaryExpression(binary) => {
                // Use ungrouped version so parent group controls breaking
                self.build_binary_chain_doc_ungrouped(binary)
            }
            _ => self.build_expression_doc(expr),
        }
    }

    /// Print comments between `}` of consequent and `else` keyword.
    ///
    /// Handles patterns like: `} // comment for else\nelse {`
    /// - Prints inline comments after `}`
    /// - Puts `else` on a new line if there are comments
    fn print_if_else_comments(&mut self, consequent_end: u32, alternate_start: u32) {
        let first_idx = tsv_lang::find_first_comment_from(self.comments, consequent_end);

        for comment in &self.comments[first_idx..] {
            if comment.span.start >= alternate_start {
                break;
            }
            // Print comment on same line as `}`
            self.write(" ");
            self.print_comment(comment);
        }
        // Put else on new line
        self.write("\n");
        self.write_indent();
        self.write("else ");
    }

    pub(super) fn print_for_statement(&mut self, stmt: &internal::ForStatement) {
        // Check for comments between ) and body (Prettier 3.7 #18108)
        // e.g., `for (...) /* comment */ ;`
        let header_end = self.get_for_header_end(stmt);
        let body_start = stmt.body.span().start;
        let has_comments = self.has_comments_between(header_end, body_start);

        if has_comments {
            // Handle comments imperatively
            let header_doc = self.build_for_header_doc(stmt);
            self.write_doc(&header_doc);
            self.print_inline_comments_between(header_end, body_start);
            // Prettier adds space after comment before empty statement: `/* comment */ ;`
            self.write(" ");
            self.print_statement(&stmt.body);
        } else {
            // Build complete doc including body for proper width calculation
            let full_doc = self.build_for_statement_with_body_doc(stmt);
            self.write_doc(&full_doc);
        }
    }

    /// Build a complete for statement doc including the body
    ///
    /// This includes the body in the doc so the width calculation accounts for ` {`.
    fn build_for_statement_with_body_doc(&self, stmt: &internal::ForStatement) -> doc::Doc {
        let header_doc = self.build_for_header_doc(stmt);
        let is_empty = matches!(stmt.body.as_ref(), Statement::EmptyStatement(_));
        let is_block = matches!(stmt.body.as_ref(), Statement::BlockStatement(_));

        if is_empty {
            // No space before empty statement: `for (...);`
            doc::concat(vec![header_doc, self.build_statement_doc(&stmt.body)])
        } else if is_block {
            // Block body: `for (...) { ... }`
            // Note: Unlike for-in/for-of, standard for loops keep empty blocks inline `{}`
            if let Statement::BlockStatement(block) = stmt.body.as_ref() {
                doc::concat(vec![
                    header_doc,
                    doc::text(" "),
                    self.build_block_statement_doc(block),
                ])
            } else {
                unreachable!()
            }
        } else {
            // Non-block body: `for (...) stmt;`
            doc::concat(vec![
                header_doc,
                doc::text(" "),
                self.build_statement_doc(&stmt.body),
            ])
        }
    }

    /// Get the end position of a for loop header (position after the last non-body element)
    fn get_for_header_end(&self, stmt: &internal::ForStatement) -> u32 {
        // The header ends at the update, test, init, or span start (whichever is last)
        stmt.update
            .as_ref()
            .map(|u| u.span().end)
            .or_else(|| stmt.test.as_ref().map(|t| t.span().end))
            .or_else(|| stmt.init.as_ref().map(|i| self.get_for_init_span_end(i)))
            .unwrap_or(stmt.span.start + 4) // fallback: after "for ("
    }

    /// Build a Doc for the for loop header with wrapping support
    ///
    /// Handles comments between parts (Prettier 3.7 #18099):
    /// `for (let i = 0; // comment\n i < n; i++)` expands to multi-line
    fn build_for_header_doc(&self, stmt: &internal::ForStatement) -> doc::Doc {
        // Build init, test, update parts
        let init_doc = stmt.init.as_ref().map(|init| self.build_for_init_doc(init));
        let test_doc = stmt
            .test
            .as_ref()
            .map(|test| self.build_expression_doc(test));
        let update_doc = stmt
            .update
            .as_ref()
            .map(|update| self.build_for_update_doc(update));

        // Prettier's for loop spacing rules:
        // - `for (;;)` - no content, no spaces
        // - `for (init;;)` - init only
        // - `for (;test;)` - test only
        // - `for (;;update)` - update only
        // - `for (init; test; update)` - spaces between parts
        // - `for (; cond; )` - space after last ; when update is None but test exists

        let has_any = init_doc.is_some() || test_doc.is_some() || update_doc.is_some();

        if !has_any {
            // Empty for (;;) - no wrapping needed
            return doc::text("for (;;)");
        }

        // Check for comments between parts that force expansion
        let has_comment_between_parts = self.for_header_has_comments(stmt);

        // Build the content between parens
        let mut inner_parts = Vec::new();

        let has_test = test_doc.is_some();
        let has_update = update_doc.is_some();

        // Init part
        if let Some(init) = init_doc {
            inner_parts.push(init);
        }
        inner_parts.push(doc::text(";"));

        // Check for comment between init and test
        if let Some(init) = &stmt.init {
            let init_end = self.get_for_init_span_end(init);
            let test_start = if let Some(t) = &stmt.test {
                t.span().start
            } else if let Some(u) = &stmt.update {
                u.span().start
            } else {
                stmt.span.end
            };
            if self.has_line_comments_between(init_end, test_start) {
                let comment_on_own_line = self.has_newline_before_comment(init_end, test_start);
                if comment_on_own_line {
                    inner_parts.push(doc::hardline());
                    inner_parts.push(
                        self.build_inline_comments_between_doc_no_leading_space(
                            init_end, test_start,
                        ),
                    );
                    inner_parts.push(doc::hardline());
                } else {
                    inner_parts.push(self.build_inline_comments_between_doc(init_end, test_start));
                    inner_parts.push(doc::hardline());
                }
            } else if has_test || has_update {
                inner_parts.push(doc::line());
            }
        } else if has_test || has_update {
            // Test part - add space/line before if test or update exists
            inner_parts.push(doc::line());
        }

        if let Some(test) = test_doc {
            inner_parts.push(test);
        }
        inner_parts.push(doc::text(";"));

        // Update part
        if let Some(update) = update_doc {
            inner_parts.push(doc::line());
            inner_parts.push(update);
        } else if has_test && !has_comment_between_parts {
            // Prettier adds trailing space when update is None but test exists
            inner_parts.push(doc::if_break(doc::text(""), doc::text(" ")));
        }

        doc::group(doc::concat(vec![
            doc::text("for ("),
            doc::indent_softline(doc::concat(inner_parts)),
            doc::softline(),
            doc::text(")"),
        ]))
    }

    /// Build a Doc for a for loop update expression
    fn build_for_update_doc(&self, expr: &internal::Expression) -> doc::Doc {
        if let internal::Expression::SequenceExpression(seq) = expr {
            let expr_docs: Vec<_> = seq
                .expressions
                .iter()
                .map(|e| self.build_expression_doc(e))
                .collect();
            doc::join(expr_docs, ", ")
        } else {
            self.build_expression_doc(expr)
        }
    }

    pub(super) fn print_for_in_statement(&mut self, stmt: &internal::ForInStatement) {
        // Use doc-based printing for proper width calculation of right-side expressions
        let full_doc = self.build_for_in_statement_with_body_doc(stmt);
        self.write_doc(&full_doc);
    }

    /// Build a complete for-in statement doc including the body
    fn build_for_in_statement_with_body_doc(&self, stmt: &internal::ForInStatement) -> doc::Doc {
        let mut parts = vec![
            doc::text("for ("),
            self.build_for_in_of_left_doc(&stmt.left),
            doc::text(" in "),
            self.build_expression_doc(&stmt.right),
            doc::text(") "),
        ];

        // Prettier expands empty blocks for for-in
        if let Statement::BlockStatement(block) = stmt.body.as_ref() {
            parts.push(self.build_block_statement_expand_empty_doc(block));
        } else {
            parts.push(self.build_statement_doc(&stmt.body));
        }

        doc::concat(parts)
    }

    pub(super) fn print_for_of_statement(&mut self, stmt: &internal::ForOfStatement) {
        // Use doc-based printing for proper width calculation of right-side expressions
        let full_doc = self.build_for_of_statement_with_body_doc(stmt);
        self.write_doc(&full_doc);
    }

    /// Build a complete for-of statement doc including the body
    fn build_for_of_statement_with_body_doc(&self, stmt: &internal::ForOfStatement) -> doc::Doc {
        let mut parts = vec![doc::text("for ")];
        if stmt.r#await {
            parts.push(doc::text("await "));
        }
        parts.push(doc::text("("));
        parts.push(self.build_for_in_of_left_doc(&stmt.left));
        parts.push(doc::text(" of "));
        parts.push(self.build_expression_doc(&stmt.right));
        parts.push(doc::text(") "));

        // Prettier expands empty blocks for for-of
        if let Statement::BlockStatement(block) = stmt.body.as_ref() {
            parts.push(self.build_block_statement_expand_empty_doc(block));
        } else {
            parts.push(self.build_statement_doc(&stmt.body));
        }

        doc::concat(parts)
    }

    pub(super) fn print_while_statement(&mut self, stmt: &internal::WhileStatement) {
        // Use doc-based printing for proper line-width wrapping
        let while_doc = self.build_while_statement_with_wrapping_doc(stmt);
        self.write_doc(&while_doc);
    }

    /// Build a doc for a while statement with proper line-width wrapping
    ///
    /// Matches Prettier's architecture: the condition wraps to multiple lines
    /// when the `while (condition)` line exceeds print width.
    fn build_while_statement_with_wrapping_doc(&self, stmt: &internal::WhileStatement) -> doc::Doc {
        // For binary expressions in condition, use ungrouped version so parent group controls breaking
        let test_doc = self.build_condition_doc(&stmt.test);

        // Condition group: group([indent([softline, test]), softline])
        let condition_group = doc::group(doc::concat(vec![
            doc::indent(doc::concat(vec![doc::softline(), test_doc])),
            doc::softline(),
        ]));

        let is_block = matches!(stmt.body.as_ref(), Statement::BlockStatement(_));
        let is_empty = matches!(stmt.body.as_ref(), Statement::EmptyStatement(_));

        if is_block {
            // Block body: while (cond) { ... }
            let mut parts = vec![doc::text("while ("), condition_group, doc::text(") ")];
            if let Statement::BlockStatement(block) = stmt.body.as_ref() {
                parts.push(self.build_block_statement_doc(block));
            }
            doc::group(doc::concat(parts))
        } else if is_empty {
            // Empty statement: while (cond);
            doc::group(doc::concat(vec![
                doc::text("while ("),
                condition_group,
                doc::text(");"),
            ]))
        } else {
            // Non-block body: use adjustClause equivalent
            // - When flat: line becomes space -> `while (cond) a;`
            // - When broken: line becomes newline + indent -> `while (cond)\n\ta;`
            let body_doc = self.build_statement_doc(&stmt.body);
            let adjust_clause = doc::indent(doc::concat(vec![doc::line(), body_doc]));

            doc::group(doc::concat(vec![
                doc::text("while ("),
                condition_group,
                doc::text(")"),
                adjust_clause,
            ]))
        }
    }

    pub(super) fn print_do_while_statement(&mut self, stmt: &internal::DoWhileStatement) {
        self.write("do ");
        self.print_statement(&stmt.body);
        // Block statement: `while` on same line - `do { } while (cond);`
        // Other statements: `while` on new line - `do expr;\n while (cond);`
        if matches!(stmt.body.as_ref(), Statement::BlockStatement(_)) {
            self.write(" while (");
        } else {
            self.write("\n");
            self.write_indent();
            self.write("while (");
        }
        self.print_expression(&stmt.test);
        self.write(");");
    }

    pub(super) fn print_switch_statement(&mut self, stmt: &internal::SwitchStatement) {
        // Use doc-based printing for proper line-width wrapping
        let switch_doc = self.build_switch_statement_with_wrapping_doc(stmt);
        self.write_doc(&switch_doc);
    }

    /// Build a doc for a switch statement with proper line-width wrapping
    ///
    /// Matches Prettier's architecture: the discriminant wraps to multiple lines
    /// when the `switch (discriminant) {` line exceeds print width.
    fn build_switch_statement_with_wrapping_doc(
        &self,
        stmt: &internal::SwitchStatement,
    ) -> doc::Doc {
        // For binary expressions in discriminant, use ungrouped version so parent group controls breaking
        let discriminant_doc = self.build_condition_doc(&stmt.discriminant);

        // Condition group: group([indent([softline, discriminant]), softline])
        // - This group decides whether the discriminant breaks (operators go to new lines)
        let condition_group = doc::group(doc::concat(vec![
            doc::indent(doc::concat(vec![doc::softline(), discriminant_doc])),
            doc::softline(),
        ]));

        // Build cases - they handle their own internal indentation
        // Join cases with hardlines
        let mut case_parts = Vec::new();
        for (i, case) in stmt.cases.iter().enumerate() {
            if i > 0 {
                case_parts.push(doc::hardline());
            }
            case_parts.push(self.build_switch_case_doc_inner(case));
        }

        // Structure: switch (...) { indent([hardline, cases...]) hardline }
        // The indent wraps the hardline so cases start at +1 indent level
        // For empty switch, just output {\n}
        let body_doc = if case_parts.is_empty() {
            doc::hardline()
        } else {
            doc::concat(vec![
                doc::indent(doc::concat(vec![doc::hardline(), doc::concat(case_parts)])),
                doc::hardline(),
            ])
        };

        doc::group(doc::concat(vec![
            doc::text("switch ("),
            condition_group,
            doc::text(") {"),
            body_doc,
            doc::text("}"),
        ]))
    }

    /// Build a doc for a switch case (without outer indent - that's handled by switch)
    fn build_switch_case_doc_inner(&self, case: &internal::SwitchCase) -> doc::Doc {
        let mut parts = Vec::new();

        // case X: or default:
        if let Some(test) = &case.test {
            parts.push(doc::text("case "));
            parts.push(self.build_expression_doc(test));
            parts.push(doc::text(":"));
        } else {
            parts.push(doc::text("default:"));
        }

        // Consequent statements (indented from case line)
        // Hardline must be inside the indent so it uses the +1 level
        for stmt in &case.consequent {
            parts.push(doc::indent(doc::concat(vec![
                doc::hardline(),
                self.build_statement_doc(stmt),
            ])));
        }

        doc::concat(parts)
    }

    pub(super) fn print_try_statement(&mut self, stmt: &internal::TryStatement) {
        // Use doc-based printing for the entire try statement to enable
        // width-aware wrapping of catch clause type annotations and patterns.
        // This ensures the `) {}` suffix is included in width calculations.
        let try_doc = self.build_try_statement_doc(stmt);
        self.write_doc(&try_doc);
    }

    pub(super) fn print_throw_statement(&mut self, stmt: &internal::ThrowStatement) {
        self.write("throw ");
        self.print_expression(&stmt.argument);
        self.write(";");
    }

    pub(super) fn print_break_statement(&mut self, stmt: &internal::BreakStatement) {
        self.write("break");
        if let Some(label) = &stmt.label {
            self.write(" ");
            self.print_identifier(label);
        }
        self.write(";");
    }

    pub(super) fn print_continue_statement(&mut self, stmt: &internal::ContinueStatement) {
        self.write("continue");
        if let Some(label) = &stmt.label {
            self.write(" ");
            self.print_identifier(label);
        }
        self.write(";");
    }

    pub(super) fn print_labeled_statement(&mut self, stmt: &internal::LabeledStatement) {
        self.print_identifier(&stmt.label);
        // No space before empty statement: `label:;` not `label: ;`
        if matches!(stmt.body.as_ref(), Statement::EmptyStatement(_)) {
            self.write(":");
        } else {
            self.write(": ");
        }
        self.print_statement(&stmt.body);
    }

    // ========================================================================
    // Control Flow Statement Doc Builders
    // ========================================================================

    pub(super) fn build_if_statement_doc(&self, stmt: &internal::IfStatement) -> doc::Doc {
        // No space before empty statement: `if (true);` not `if (true) ;`
        let consequent_prefix = if matches!(stmt.consequent.as_ref(), Statement::EmptyStatement(_))
        {
            ")"
        } else {
            ") "
        };
        let mut parts = vec![
            doc::text("if ("),
            self.build_expression_doc(&stmt.test),
            doc::text(consequent_prefix),
            self.build_statement_doc(&stmt.consequent),
        ];
        if let Some(alternate) = &stmt.alternate {
            if matches!(stmt.consequent.as_ref(), Statement::BlockStatement(_)) {
                parts.push(doc::text(" else "));
            } else {
                parts.push(doc::hardline());
                parts.push(doc::text("else "));
            }
            parts.push(self.build_statement_doc(alternate));
        }
        doc::concat(parts)
    }

    pub(super) fn build_for_statement_doc(&self, stmt: &internal::ForStatement) -> doc::Doc {
        let mut parts = vec![doc::text("for (")];
        if let Some(init) = &stmt.init {
            parts.push(self.build_for_init_doc(init));
        }
        parts.push(doc::text("; "));
        if let Some(test) = &stmt.test {
            parts.push(self.build_expression_doc(test));
        }
        parts.push(doc::text("; "));
        if let Some(update) = &stmt.update {
            parts.push(self.build_expression_doc(update));
        }
        parts.push(doc::text(") "));
        parts.push(self.build_statement_doc(&stmt.body));
        doc::concat(parts)
    }

    fn build_for_init_doc(&self, init: &internal::ForInit) -> doc::Doc {
        match init {
            internal::ForInit::VariableDeclaration(decl) => {
                let mut parts = vec![doc::text(decl.kind.as_str()), doc::text(" ")];
                for (i, declarator) in decl.declarations.iter().enumerate() {
                    if i > 0 {
                        parts.push(doc::text(", "));
                    }
                    parts.push(self.build_expression_doc(&declarator.id));
                    if let Some(init) = &declarator.init {
                        parts.push(doc::text(" = "));
                        parts.push(self.build_expression_doc(init));
                    }
                }
                doc::concat(parts)
            }
            internal::ForInit::Expression(expr) => self.build_expression_doc(expr),
        }
    }

    pub(super) fn build_for_in_statement_doc(&self, stmt: &internal::ForInStatement) -> doc::Doc {
        doc::concat(vec![
            doc::text("for ("),
            self.build_for_in_of_left_doc(&stmt.left),
            doc::text(" in "),
            self.build_expression_doc(&stmt.right),
            doc::text(") "),
            self.build_statement_doc(&stmt.body),
        ])
    }

    pub(super) fn build_for_of_statement_doc(&self, stmt: &internal::ForOfStatement) -> doc::Doc {
        let mut parts = vec![doc::text("for ")];
        if stmt.r#await {
            parts.push(doc::text("await "));
        }
        parts.push(doc::text("("));
        parts.push(self.build_for_in_of_left_doc(&stmt.left));
        parts.push(doc::text(" of "));
        parts.push(self.build_expression_doc(&stmt.right));
        parts.push(doc::text(") "));
        parts.push(self.build_statement_doc(&stmt.body));
        doc::concat(parts)
    }

    fn build_for_in_of_left_doc(&self, left: &internal::ForInOfLeft) -> doc::Doc {
        match left {
            internal::ForInOfLeft::VariableDeclaration(decl) => {
                let mut parts = vec![doc::text(decl.kind.as_str()), doc::text(" ")];
                if let Some(declarator) = decl.declarations.first() {
                    parts.push(self.build_expression_doc(&declarator.id));
                }
                doc::concat(parts)
            }
            internal::ForInOfLeft::Pattern(expr) => self.build_expression_doc(expr),
        }
    }

    pub(super) fn build_while_statement_doc(&self, stmt: &internal::WhileStatement) -> doc::Doc {
        let mut parts = vec![
            doc::text("while ("),
            self.build_expression_doc(&stmt.test),
            doc::text(")"),
        ];
        // Don't add space before empty statement: `while (cond);` not `while (cond) ;`
        if !matches!(stmt.body.as_ref(), Statement::EmptyStatement(_)) {
            parts.push(doc::text(" "));
        }
        parts.push(self.build_statement_doc(&stmt.body));
        doc::concat(parts)
    }

    pub(super) fn build_do_while_statement_doc(
        &self,
        stmt: &internal::DoWhileStatement,
    ) -> doc::Doc {
        let mut parts = vec![doc::text("do "), self.build_statement_doc(&stmt.body)];
        // Block statement: `while` on same line - `do { } while (cond);`
        // Other statements: `while` on new line - `do expr;\n while (cond);`
        if matches!(stmt.body.as_ref(), Statement::BlockStatement(_)) {
            parts.push(doc::text(" while ("));
        } else {
            parts.push(doc::hardline());
            parts.push(doc::text("while ("));
        }
        parts.push(self.build_expression_doc(&stmt.test));
        parts.push(doc::text(");"));
        doc::concat(parts)
    }

    pub(super) fn build_switch_statement_doc(&self, stmt: &internal::SwitchStatement) -> doc::Doc {
        let mut parts = vec![
            doc::text("switch ("),
            self.build_expression_doc(&stmt.discriminant),
            doc::text(") {"),
            doc::hardline(),
        ];
        for case in &stmt.cases {
            parts.push(doc::indent(self.build_switch_case_doc(case)));
        }
        parts.push(doc::text("}"));
        doc::concat(parts)
    }

    fn build_switch_case_doc(&self, case: &internal::SwitchCase) -> doc::Doc {
        let mut parts = vec![];
        if let Some(test) = &case.test {
            parts.push(doc::text("case "));
            parts.push(self.build_expression_doc(test));
            parts.push(doc::text(":"));
        } else {
            parts.push(doc::text("default:"));
        }
        parts.push(doc::hardline());
        for stmt in &case.consequent {
            parts.push(doc::indent(self.build_statement_doc(stmt)));
            parts.push(doc::hardline());
        }
        doc::concat(parts)
    }

    pub(super) fn build_try_statement_doc(&self, stmt: &internal::TryStatement) -> doc::Doc {
        let mut parts = vec![
            doc::text("try "),
            // Try block expands empty: `try {\n}` not `try {}`
            self.build_block_statement_expand_empty_doc(&stmt.block),
        ];
        if let Some(handler) = &stmt.handler {
            parts.push(doc::text(" catch"));
            if let Some(param) = &handler.param {
                parts.push(doc::text(" ("));
                parts.push(self.build_expression_doc(param));
                parts.push(doc::text(")"));
            }
            parts.push(doc::text(" "));
            // Catch block stays inline: `catch (e) {}`
            parts.push(self.build_block_statement_doc(&handler.body));
        }
        if let Some(finalizer) = &stmt.finalizer {
            parts.push(doc::text(" finally "));
            // Finally block expands empty: `finally {\n}` not `finally {}`
            parts.push(self.build_block_statement_expand_empty_doc(finalizer));
        }
        doc::concat(parts)
    }

    pub(super) fn build_throw_statement_doc(&self, stmt: &internal::ThrowStatement) -> doc::Doc {
        doc::concat(vec![
            doc::text("throw "),
            self.build_expression_doc(&stmt.argument),
            doc::text(";"),
        ])
    }

    pub(super) fn build_break_statement_doc(&self, stmt: &internal::BreakStatement) -> doc::Doc {
        if let Some(label) = &stmt.label {
            doc::concat(vec![
                doc::text("break "),
                doc::symbol(label.name.to_u32()),
                doc::text(";"),
            ])
        } else {
            doc::text("break;")
        }
    }

    pub(super) fn build_continue_statement_doc(
        &self,
        stmt: &internal::ContinueStatement,
    ) -> doc::Doc {
        if let Some(label) = &stmt.label {
            doc::concat(vec![
                doc::text("continue "),
                doc::symbol(label.name.to_u32()),
                doc::text(";"),
            ])
        } else {
            doc::text("continue;")
        }
    }

    pub(super) fn build_labeled_statement_doc(
        &self,
        stmt: &internal::LabeledStatement,
    ) -> doc::Doc {
        // No space before empty statement: `label:;` not `label: ;`
        let separator = if matches!(stmt.body.as_ref(), Statement::EmptyStatement(_)) {
            ":"
        } else {
            ": "
        };
        doc::concat(vec![
            doc::symbol(stmt.label.name.to_u32()),
            doc::text(separator),
            self.build_statement_doc(&stmt.body),
        ])
    }

    /// Check if a for statement header has line comments between parts
    fn for_header_has_comments(&self, stmt: &internal::ForStatement) -> bool {
        // Check between init and test
        if let Some(init) = &stmt.init {
            let init_end = self.get_for_init_span_end(init);
            let test_start = if let Some(t) = &stmt.test {
                t.span().start
            } else if let Some(u) = &stmt.update {
                u.span().start
            } else {
                stmt.span.end
            };
            if self.has_line_comments_between(init_end, test_start) {
                return true;
            }
        }
        // Check between test and update
        if let Some(test) = &stmt.test {
            let test_end = test.span().end;
            let update_start = stmt
                .update
                .as_ref()
                .map_or(stmt.span.end, |u| u.span().start);
            if self.has_line_comments_between(test_end, update_start) {
                return true;
            }
        }
        false
    }

    /// Get the end position of a ForInit
    fn get_for_init_span_end(&self, init: &internal::ForInit) -> u32 {
        match init {
            internal::ForInit::VariableDeclaration(decl) => decl.span.end,
            internal::ForInit::Expression(expr) => expr.span().end,
        }
    }
}
