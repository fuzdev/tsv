// Statement formatting for TypeScript
//
// Handles formatting of different statement types:
// - Expression statements (expression followed by semicolon)
// - Variable declarations (const, let, var)
// - Future: Function declarations, class declarations, import/export, etc.

use crate::ast::internal::{self, Statement, VariableDeclarationKind};
use crate::formatter_core::Formatter;

impl Formatter {
    /// Format a statement
    pub(super) fn format_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::ExpressionStatement(stmt) => self.format_expression_statement(stmt),
            Statement::VariableDeclaration(decl) => self.format_variable_declaration(decl),
        }
    }

    /// Format an expression statement (expression followed by semicolon)
    fn format_expression_statement(&mut self, stmt: &internal::ExpressionStatement) {
        self.format_expression(&stmt.expression);
        self.write(";");
    }

    /// Format a variable declaration
    fn format_variable_declaration(&mut self, decl: &internal::VariableDeclaration) {
        // Write the keyword (const, let, var)
        match decl.kind {
            VariableDeclarationKind::Const => self.write("const"),
            VariableDeclarationKind::Let => self.write("let"),
            VariableDeclarationKind::Var => self.write("var"),
        }

        self.write(" ");

        // Format declarators (assume single declarator for now)
        for (i, declarator) in decl.declarations.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.format_variable_declarator(declarator);
        }

        self.write(";");
    }

    /// Format a variable declarator
    fn format_variable_declarator(&mut self, declarator: &internal::VariableDeclarator) {
        // Format the identifier
        self.format_identifier(&declarator.id);

        // Format the initializer if present
        if let Some(init) = &declarator.init {
            self.write(" = ");
            self.format_expression(init);
        }
    }
}
