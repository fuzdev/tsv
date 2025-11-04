// Statement printing for TypeScript
//
// Handles printing of different statement types:
// - Expression statements (expression followed by semicolon)
// - Variable declarations (const, let, var)
// - Future: Function declarations, class declarations, import/export, etc.

use super::Printer;
use crate::ast::internal::{self, Statement, VariableDeclarationKind};

impl<'a> Printer<'a> {
    /// Print a statement
    pub(super) fn print_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::ExpressionStatement(stmt) => self.print_expression_statement(stmt),
            Statement::VariableDeclaration(decl) => self.print_variable_declaration(decl),
        }
    }

    /// Print an expression statement (expression followed by semicolon)
    fn print_expression_statement(&mut self, stmt: &internal::ExpressionStatement) {
        self.print_expression(&stmt.expression);
        self.write(";");
    }

    /// Print a variable declaration
    fn print_variable_declaration(&mut self, decl: &internal::VariableDeclaration) {
        // Write the keyword (const, let, var)
        match decl.kind {
            VariableDeclarationKind::Const => self.write("const"),
            VariableDeclarationKind::Let => self.write("let"),
            VariableDeclarationKind::Var => self.write("var"),
        }

        self.write(" ");

        // Print declarators (assume single declarator for now)
        for (i, declarator) in decl.declarations.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_variable_declarator(declarator);
        }

        self.write(";");
    }

    /// Print a variable declarator
    fn print_variable_declarator(&mut self, declarator: &internal::VariableDeclarator) {
        // Print the identifier
        self.print_identifier(&declarator.id);

        // Print the initializer if present
        if let Some(init) = &declarator.init {
            self.write(" = ");
            self.print_expression(init);
        }
    }
}
