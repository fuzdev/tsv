// Control flow statement printing for TypeScript

use super::super::Printer;
use crate::ast::internal::{self, Statement};
use tsv_lang::{SymbolResolver, doc};

impl<'a> Printer<'a> {
    // ========================================================================
    // Control Flow Statement Printers
    // ========================================================================

    pub(super) fn print_if_statement(&mut self, stmt: &internal::IfStatement) {
        self.write("if (");
        self.print_expression(&stmt.test);
        self.write(")");

        // Print consequent - determine if it needs newline+indent
        // Block, expression, break, continue, return, throw stay inline
        // Other statements (if, for, while, etc.) go on new line: `if (cond)\n\tstmt`
        let consequent_inline = matches!(
            stmt.consequent.as_ref(),
            Statement::BlockStatement(_)
                | Statement::ExpressionStatement(_)
                | Statement::BreakStatement(_)
                | Statement::ContinueStatement(_)
                | Statement::ReturnStatement(_)
                | Statement::ThrowStatement(_)
        );

        if consequent_inline {
            self.write(" ");
            self.print_statement(&stmt.consequent);
        } else {
            self.write("\n");
            self.indent_level += 1;
            self.write_indent();
            self.print_statement(&stmt.consequent);
            self.indent_level -= 1;
        }

        if let Some(alternate) = &stmt.alternate {
            // If consequent is a block, else goes on same line
            // If consequent is not a block (e.g., single statement), else goes on new line
            if matches!(stmt.consequent.as_ref(), Statement::BlockStatement(_)) {
                self.write(" else ");
            } else {
                self.write("\n");
                self.write_indent();
                self.write("else ");
            }
            // Print alternate - determine if it needs newline+indent
            // Block, expression, break, continue, return, throw, and if statements stay inline
            // Other statements go on new line with indent
            let alternate_inline = matches!(
                alternate.as_ref(),
                Statement::BlockStatement(_)
                    | Statement::ExpressionStatement(_)
                    | Statement::BreakStatement(_)
                    | Statement::ContinueStatement(_)
                    | Statement::ReturnStatement(_)
                    | Statement::ThrowStatement(_)
                    | Statement::IfStatement(_)
            );

            if alternate_inline {
                self.print_statement(alternate);
            } else {
                self.write("\n");
                self.indent_level += 1;
                self.write_indent();
                self.print_statement(alternate);
                self.indent_level -= 1;
            }
        }
    }

    pub(super) fn print_for_statement(&mut self, stmt: &internal::ForStatement) {
        self.write("for (");
        if let Some(init) = &stmt.init {
            self.print_for_init(init);
        }
        self.write(";");
        // Add space after semicolon if there's a test or if test is missing but update is present
        if stmt.test.is_some() || (stmt.test.is_none() && stmt.update.is_some()) {
            self.write(" ");
        }
        if let Some(test) = &stmt.test {
            self.print_expression(test);
        }
        self.write(";");
        // Add space after second semicolon if there's an update OR if there was a test
        // (prettier adds trailing space when update is None but test was present)
        if stmt.update.is_some() || stmt.test.is_some() {
            self.write(" ");
        }
        if let Some(update) = &stmt.update {
            // For update position, sequence expressions don't need parens
            self.print_for_update(update);
        }
        self.write(") ");
        self.print_statement(&stmt.body);
    }

    /// Print a for loop update expression
    /// Sequence expressions in this position don't need parentheses
    fn print_for_update(&mut self, expr: &internal::Expression) {
        if let internal::Expression::SequenceExpression(seq) = expr {
            for (i, e) in seq.expressions.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.print_expression(e);
            }
        } else {
            self.print_expression(expr);
        }
    }

    fn print_for_init(&mut self, init: &internal::ForInit) {
        match init {
            internal::ForInit::VariableDeclaration(decl) => {
                self.write(decl.kind.as_str());
                self.write(" ");
                for (i, declarator) in decl.declarations.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expression(&declarator.id);
                    if let Some(init) = &declarator.init {
                        self.write(" = ");
                        self.print_expression(init);
                    }
                }
            }
            internal::ForInit::Expression(expr) => {
                self.print_expression(expr);
            }
        }
    }

    pub(super) fn print_for_in_statement(&mut self, stmt: &internal::ForInStatement) {
        self.write("for (");
        self.print_for_in_of_left(&stmt.left);
        self.write(" in ");
        self.print_expression(&stmt.right);
        self.write(") ");
        self.print_statement(&stmt.body);
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
        self.print_statement(&stmt.body);
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
        self.print_block_statement(&stmt.block);
        if let Some(handler) = &stmt.handler {
            self.write(" catch");
            if let Some(param) = &handler.param {
                self.write(" (");
                self.print_expression(param);
                self.write(")");
            }
            self.write(" ");
            self.print_block_statement(&handler.body);
        }
        if let Some(finalizer) = &stmt.finalizer {
            self.write(" finally ");
            self.print_block_statement(finalizer);
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
        self.write(": ");
        self.print_statement(&stmt.body);
    }

    // ========================================================================
    // Control Flow Statement Doc Builders
    // ========================================================================

    pub(super) fn build_if_statement_doc(&self, stmt: &internal::IfStatement) -> doc::Doc {
        let mut parts = vec![
            doc::text("if ("),
            self.build_expression_doc(&stmt.test),
            doc::text(") "),
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
                doc::text(self.resolve_symbol(label.name)),
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
                doc::text(self.resolve_symbol(label.name)),
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
        doc::concat(vec![
            doc::text(self.resolve_symbol(stmt.label.name)),
            doc::text(": "),
            self.build_statement_doc(&stmt.body),
        ])
    }
}
