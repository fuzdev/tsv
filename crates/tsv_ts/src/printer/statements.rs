// Statement printing for TypeScript
//
// Handles printing of different statement types:
// - Expression statements (expression followed by semicolon)
// - Variable declarations (const, let, var)
// - Future: Function declarations, class declarations, import/export, etc.

use super::{Printer, is_pure_property_chain};
use crate::ast::internal::{self, Statement, VariableDeclarationKind};
use tsv_lang::doc;

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

    /// Print a variable declarator with "fluid" assignment wrapping
    ///
    /// When the declaration exceeds print_width, wraps after `=`:
    /// ```javascript
    /// const medium =
    ///     obj1.prop1.prop2.prop3...;
    /// ```
    ///
    /// Uses `group(id + " =" + indent(line + rhs))` so the group decision
    /// determines whether to break. Property chains use greedy line packing
    /// via `fill()` for long chains that need internal breaks.
    fn print_variable_declarator(&mut self, declarator: &internal::VariableDeclarator) {
        // Check if we have an initializer - if not, just print the identifier
        let Some(init) = &declarator.init else {
            self.print_identifier(&declarator.id);
            return;
        };

        // Handle comments around the equals sign
        let id_end = declarator.id.span.end;
        let init_start = init.span().start;
        let equals_pos = self.find_equals_position(id_end, init_start);
        let has_comments_before_eq = self.has_comments_between(id_end, equals_pos);
        let has_comments_after_eq = self.has_comments_between(equals_pos + 1, init_start);

        // If there are comments, use direct printing (comment handling with doc IR is complex)
        if has_comments_before_eq || has_comments_after_eq {
            self.print_identifier(&declarator.id);
            let _ = self.print_inline_comments_between(id_end, equals_pos);
            if has_comments_after_eq {
                self.write(" =");
            } else {
                self.write(" = ");
            }
            let _ = self.print_inline_comments_between(equals_pos + 1, init_start);
            if has_comments_after_eq {
                self.write(" ");
            }
            self.print_expression(init);
            return;
        }

        // Check if RHS needs "fluid" assignment wrapping
        // Fluid layout applies to property chains (member expressions without calls)
        // that don't have internal breaking points. Objects, arrays, calls, and
        // ternaries handle their own wrapping internally.
        let needs_fluid_layout = is_pure_property_chain(init);

        if needs_fluid_layout {
            // Build doc for "fluid" assignment layout:
            // - If RHS fits after `= `, stay on one line: `id = value`
            // - If RHS doesn't fit, break after `=` and indent: `id =\n\tvalue`
            //
            // Structure: group(id + " =" + indent(line + rhs))
            // When the group decides to break, line() becomes newline + indent
            let id_str = declarator.id.span.extract(self.source);
            let id_doc = doc::text(id_str);
            let init_doc = self.build_expression_doc(init);

            let assignment_doc = doc::group(doc::concat(vec![
                id_doc,
                doc::text(" ="),
                doc::indent(doc::concat(vec![doc::line(), init_doc])),
            ]));

            let base_offset = self.config.base_indent_offset * self.config.tab_width;
            let current_col = self.current_column() + base_offset;
            let output = doc::print_doc_at_column(&assignment_doc, &self.config, current_col);
            self.write(&output);
        } else {
            // Direct printing for expressions that handle their own wrapping
            self.print_identifier(&declarator.id);
            self.write(" = ");
            self.print_expression(init);
        }
    }
}
