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

        // Print inline comments - includes both:
        // 1. Comments after semicolon (stmt.span.end)
        // 2. Comments before semicolon (between expression.end and stmt.span.end)
        // Prettier moves comments from before semicolon to after it
        self.print_inline_comments_in_statement(stmt.expression.span().end, stmt.span.end);
    }

    /// Print a variable declaration
    fn print_variable_declaration(&mut self, decl: &internal::VariableDeclaration) {
        // Write the keyword (const, let, var)
        let keyword_start = decl.span.start;
        let keyword_len = match decl.kind {
            VariableDeclarationKind::Const => {
                self.write("const");
                5
            }
            VariableDeclarationKind::Let => {
                self.write("let");
                3
            }
            VariableDeclarationKind::Var => {
                self.write("var");
                3
            }
        };

        // Print comments between keyword and first declarator (e.g., `const /* comment */ x`)
        if !decl.declarations.is_empty() {
            let first_declarator_start = decl.declarations[0].span.start;
            let keyword_end = keyword_start + keyword_len;
            self.print_inline_comments_between(keyword_end, first_declarator_start);
        }

        self.write(" ");

        // Print declarators
        // TODO: Handle comments between declarators in multi-declarator statements
        // Example: `const x = 1 /* comment */, y = 2;`
        // Currently: Comments between declarators not explicitly handled
        // Need: Check for comments after comma separator, print before next declarator
        let mut last_declarator_end = 0u32;
        for (i, declarator) in decl.declarations.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_variable_declarator(declarator);
            last_declarator_end = declarator.span.end;
        }

        self.write(";");

        // Print inline comments - includes both:
        // 1. Comments after semicolon (decl.span.end)
        // 2. Comments before semicolon (between last declarator and decl.span.end)
        // Prettier moves comments from before semicolon to after it
        self.print_inline_comments_in_statement(last_declarator_end, decl.span.end);
    }

    /// Print a variable declarator
    fn print_variable_declarator(&mut self, declarator: &internal::VariableDeclarator) {
        // Print the identifier
        self.print_identifier(&declarator.id);

        // Print the initializer if present
        if let Some(init) = &declarator.init {
            // Handle comments around the equals sign
            // Find equals position and split comments into before/after groups
            let id_end = declarator.id.span.end;
            let init_start = init.span().start;

            // Find the `=` character position in the source
            let equals_pos = self.find_equals_position(id_end, init_start);

            // Print comments before `=`
            let has_before_comments = self.print_inline_comments_between(id_end, equals_pos);

            // Print comments after `=` (need to check before writing equals to know if we need trailing space)
            let has_after_comments_temp = self.has_comments_between(equals_pos + 1, init_start);

            // Print equals with appropriate spacing
            if has_before_comments {
                if has_after_comments_temp {
                    self.write(" ="); // Comment added leading space, trailing space will be added by print_inline_comments_between
                } else {
                    self.write(" = ");
                }
            } else if has_after_comments_temp {
                self.write(" ="); // Trailing space will be added by print_inline_comments_between
            } else {
                self.write(" = ");
            }

            // Print comments after `=`
            let has_after_comments = self.print_inline_comments_between(equals_pos + 1, init_start);

            // Add trailing space after comments (before expression)
            if has_after_comments {
                self.write(" ");
            }

            self.print_expression(init);
        }
    }
}
