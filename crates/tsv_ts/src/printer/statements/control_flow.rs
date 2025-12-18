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
        // Check if condition has line comments that require multi-line expansion
        if self.condition_has_line_comments(&stmt.test) {
            // Use doc-based printing for proper multi-line handling
            let test_doc = self.build_expression_doc(&stmt.test);
            let condition_doc = doc::group(doc::concat(vec![
                doc::text("if ("),
                doc::indent_softline(test_doc),
                doc::softline(),
                doc::text(")"),
            ]));
            self.write_doc_with_margin(&condition_doc);
        } else {
            self.write("if (");
            self.print_expression(&stmt.test);
            self.write(")");
        }

        // Print consequent - inline or newline+indent based on statement type
        if is_inline_consequent(&stmt.consequent) {
            // No space before empty statement: `if (true);` not `if (true) ;`
            if !matches!(stmt.consequent.as_ref(), Statement::EmptyStatement(_)) {
                self.write(" ");
            }
            // Empty blocks expand in if context: `if (x) {\n}` not `if (x) {}`
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
            // Check for comments between consequent and alternate
            // These are comments like: `} // comment\nelse {`
            let consequent_end = stmt.consequent.span().end;
            let alternate_start = alternate.span().start;
            let has_comments_between = self.has_comments_between(consequent_end, alternate_start);

            // If consequent is a block, else goes on same line
            // If consequent is not a block (e.g., single statement), else goes on new line
            if matches!(stmt.consequent.as_ref(), Statement::BlockStatement(_)) {
                if has_comments_between {
                    // Print inline comments between } and else
                    self.print_if_else_comments(consequent_end, alternate_start);
                } else {
                    self.write(" else ");
                }
            } else {
                self.write("\n");
                self.write_indent();
                self.write("else ");
            }
            // Print alternate - inline or newline+indent based on statement type
            if is_inline_alternate(alternate) {
                // Empty blocks expand in else context: `else {\n}` not `else {}`
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
        // Build the for header as a doc for proper line wrapping
        let doc = self.build_for_header_doc(stmt);
        self.write_doc_with_margin(&doc);

        // Check for comments between ) and body (Prettier 3.7 #18108)
        // e.g., `for (...) /* comment */ ;`
        let header_end = self.get_for_header_end(stmt);
        let body_start = stmt.body.span().start;
        let has_comments = self.has_comments_between(header_end, body_start);

        if has_comments {
            self.print_inline_comments_between(header_end, body_start);
            // Prettier adds space after comment before empty statement: `/* comment */ ;`
            self.write(" ");
        } else if !matches!(stmt.body.as_ref(), Statement::EmptyStatement(_)) {
            // Don't add space before empty statement: `for (...);` not `for (...) ;`
            self.write(" ");
        }
        self.print_statement(&stmt.body);
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
        self.write("for (");
        self.print_for_in_of_left(&stmt.left);
        self.write(" in ");
        self.print_expression(&stmt.right);
        self.write(") ");
        // Prettier expands empty blocks for for-in (unlike regular for)
        if let Statement::BlockStatement(block) = stmt.body.as_ref() {
            self.print_block_statement_expand_empty(block);
        } else {
            self.print_statement(&stmt.body);
        }
    }

    pub(super) fn print_for_of_statement(&mut self, stmt: &internal::ForOfStatement) {
        self.write("for ");
        if stmt.r#await {
            self.write("await ");
        }
        self.write("(");
        self.print_for_in_of_left(&stmt.left);
        self.write(" of ");
        self.print_expression(&stmt.right);
        self.write(") ");
        // Prettier expands empty blocks for for-of (unlike regular for)
        if let Statement::BlockStatement(block) = stmt.body.as_ref() {
            self.print_block_statement_expand_empty(block);
        } else {
            self.print_statement(&stmt.body);
        }
    }

    fn print_for_in_of_left(&mut self, left: &internal::ForInOfLeft) {
        match left {
            internal::ForInOfLeft::VariableDeclaration(decl) => {
                self.write(decl.kind.as_str());
                self.write(" ");
                if let Some(declarator) = decl.declarations.first() {
                    self.print_expression(&declarator.id);
                }
            }
            internal::ForInOfLeft::Pattern(expr) => {
                self.print_expression(expr);
            }
        }
    }

    pub(super) fn print_while_statement(&mut self, stmt: &internal::WhileStatement) {
        self.write("while (");
        self.print_expression(&stmt.test);
        self.write(")");
        // Don't add space before empty statement: `while (cond);` not `while (cond) ;`
        if !matches!(stmt.body.as_ref(), Statement::EmptyStatement(_)) {
            self.write(" ");
        }
        self.print_statement(&stmt.body);
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
        self.write("switch (");
        self.print_expression(&stmt.discriminant);
        self.write(") {\n");
        self.indent_level += 1;
        for case in &stmt.cases {
            self.print_switch_case(case);
        }
        self.indent_level -= 1;
        self.write_indent();
        self.write("}");
    }

    fn print_switch_case(&mut self, case: &internal::SwitchCase) {
        self.write_indent();
        if let Some(test) = &case.test {
            self.write("case ");
            self.print_expression(test);
            self.write(":\n");
        } else {
            self.write("default:\n");
        }
        self.indent_level += 1;
        for stmt in &case.consequent {
            self.write_indent();
            self.print_statement(stmt);
            self.write("\n");
        }
        self.indent_level -= 1;
    }

    pub(super) fn print_try_statement(&mut self, stmt: &internal::TryStatement) {
        self.write("try ");
        // Try block expands empty: `try {\n}` not `try {}`
        self.print_block_statement_expand_empty(&stmt.block);
        if let Some(handler) = &stmt.handler {
            self.write(" catch");
            if let Some(param) = &handler.param {
                self.write(" (");
                self.print_expression(param);
                self.write(")");
            }
            self.write(" ");
            // Catch block stays inline: `catch (e) {}`
            self.print_block_statement(&handler.body);
        }
        if let Some(finalizer) = &stmt.finalizer {
            self.write(" finally ");
            // Finally block expands empty: `finally {\n}` not `finally {}`
            self.print_block_statement_expand_empty(finalizer);
        }
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
            self.build_block_statement_doc(&stmt.block),
        ];
        if let Some(handler) = &stmt.handler {
            parts.push(doc::text(" catch"));
            if let Some(param) = &handler.param {
                parts.push(doc::text(" ("));
                parts.push(self.build_expression_doc(param));
                parts.push(doc::text(")"));
            }
            parts.push(doc::text(" "));
            parts.push(self.build_block_statement_doc(&handler.body));
        }
        if let Some(finalizer) = &stmt.finalizer {
            parts.push(doc::text(" finally "));
            parts.push(self.build_block_statement_doc(finalizer));
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

    /// Check if a condition expression has line comments that require multi-line expansion
    ///
    /// Returns true if the expression or any of its binary sub-expressions contain
    /// line comments between operands.
    fn condition_has_line_comments(&self, expr: &internal::Expression) -> bool {
        match expr {
            internal::Expression::BinaryExpression(binary) => {
                // Check for line comments between left and right operands
                let left_end = binary.left.span().end;
                let right_start = binary.right.span().start;
                if self.has_line_comments_between(left_end, right_start) {
                    return true;
                }
                // Recursively check sub-expressions
                self.condition_has_line_comments(&binary.left)
                    || self.condition_has_line_comments(&binary.right)
            }
            // For other expression types, no line comments to check
            _ => false,
        }
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
