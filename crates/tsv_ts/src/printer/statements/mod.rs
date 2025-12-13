// Statement printing for TypeScript
//
// Handles printing of different statement types:
// - Expression statements (expression followed by semicolon)
// - Variable declarations (const, let, var)
// - Type-related statements (type alias, return)
// - Function and class declarations
// - Import/export statements
// - Control flow (if, for, while, switch, try, etc.)

mod class;
mod control_flow;
mod function;
mod modules;
mod types;
mod variable;

use super::Printer;
use crate::ast::internal::{self, Statement};
use tsv_lang::doc;

impl<'a> Printer<'a> {
    /// Print a statement
    pub(super) fn print_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::ExpressionStatement(stmt) => self.print_expression_statement(stmt),
            Statement::VariableDeclaration(decl) => self.print_variable_declaration(decl),
            Statement::TSTypeAliasDeclaration(decl) => self.print_type_alias_declaration(decl),
            Statement::ReturnStatement(ret) => self.print_return_statement(ret),
            Statement::BlockStatement(block) => self.print_block_statement(block),
            Statement::FunctionDeclaration(decl) => self.print_function_declaration(decl),
            Statement::ClassDeclaration(decl) => self.print_class_declaration(decl),
            Statement::ExportNamedDeclaration(decl) => self.print_export_named_declaration(decl),
            Statement::ExportDefaultDeclaration(decl) => {
                self.print_export_default_declaration(decl);
            }
            Statement::ExportAllDeclaration(decl) => self.print_export_all_declaration(decl),
            Statement::TSExportAssignment(decl) => self.print_export_assignment(decl),
            Statement::ImportDeclaration(decl) => self.print_import_declaration(decl),
            Statement::TSImportEqualsDeclaration(decl) => {
                self.print_import_equals_declaration(decl);
            }
            // Control flow statements
            Statement::IfStatement(stmt) => self.print_if_statement(stmt),
            Statement::ForStatement(stmt) => self.print_for_statement(stmt),
            Statement::ForInStatement(stmt) => self.print_for_in_statement(stmt),
            Statement::ForOfStatement(stmt) => self.print_for_of_statement(stmt),
            Statement::WhileStatement(stmt) => self.print_while_statement(stmt),
            Statement::DoWhileStatement(stmt) => self.print_do_while_statement(stmt),
            Statement::SwitchStatement(stmt) => self.print_switch_statement(stmt),
            Statement::TryStatement(stmt) => self.print_try_statement(stmt),
            Statement::ThrowStatement(stmt) => self.print_throw_statement(stmt),
            Statement::BreakStatement(stmt) => self.print_break_statement(stmt),
            Statement::ContinueStatement(stmt) => self.print_continue_statement(stmt),
            Statement::LabeledStatement(stmt) => self.print_labeled_statement(stmt),
            Statement::EmptyStatement(_) => self.write(";"),
            Statement::TSInterfaceDeclaration(decl) => self.print_interface_declaration(decl),
            Statement::TSDeclareFunction(decl) => self.print_declare_function(decl),
            Statement::TSEnumDeclaration(decl) => self.print_enum_declaration(decl),
            Statement::TSModuleDeclaration(decl) => self.print_module_declaration(decl),
        }
    }

    /// Build a Doc for a statement
    pub(super) fn build_statement_doc(&self, statement: &Statement) -> doc::Doc {
        match statement {
            Statement::ExpressionStatement(stmt) => {
                let expr_doc = self.build_expression_doc(&stmt.expression);
                doc::concat(vec![expr_doc, doc::text(";")])
            }
            Statement::VariableDeclaration(decl) => {
                // Simple doc build for variable declarations
                let keyword = decl.kind.as_str();
                let mut parts = vec![doc::text(keyword), doc::text(" ")];

                for (i, declarator) in decl.declarations.iter().enumerate() {
                    if i > 0 {
                        parts.push(doc::text(","));
                        parts.push(doc::hardline());
                        parts.push(doc::text(self.config.indent));
                    }
                    // id can be Identifier, ArrayPattern, or ObjectPattern
                    parts.push(self.build_expression_doc(&declarator.id));
                    if let Some(init) = &declarator.init {
                        parts.push(doc::text(" = "));
                        parts.push(self.build_expression_doc(init));
                    }
                }

                parts.push(doc::text(";"));
                doc::concat(parts)
            }
            Statement::TSTypeAliasDeclaration(decl) => self.build_type_alias_declaration_doc(decl),
            Statement::ReturnStatement(ret) => {
                let mut parts = vec![doc::text("return")];
                if let Some(arg) = &ret.argument {
                    parts.push(doc::text(" "));
                    parts.push(self.build_expression_doc(arg));
                }
                parts.push(doc::text(";"));
                doc::concat(parts)
            }
            Statement::BlockStatement(block) => self.build_block_statement_doc(block),
            Statement::FunctionDeclaration(decl) => self.build_function_declaration_doc(decl),
            Statement::ClassDeclaration(decl) => self.build_class_declaration_doc(decl),
            Statement::ExportNamedDeclaration(decl) => {
                self.build_export_named_declaration_doc(decl)
            }
            Statement::ExportDefaultDeclaration(decl) => {
                self.build_export_default_declaration_doc(decl)
            }
            Statement::ExportAllDeclaration(decl) => self.build_export_all_declaration_doc(decl),
            Statement::TSExportAssignment(decl) => self.build_export_assignment_doc(decl),
            Statement::ImportDeclaration(decl) => self.build_import_declaration_doc(decl),
            Statement::TSImportEqualsDeclaration(decl) => {
                self.build_import_equals_declaration_doc(decl)
            }
            // Control flow statements - use simple doc building
            Statement::IfStatement(stmt) => self.build_if_statement_doc(stmt),
            Statement::ForStatement(stmt) => self.build_for_statement_doc(stmt),
            Statement::ForInStatement(stmt) => self.build_for_in_statement_doc(stmt),
            Statement::ForOfStatement(stmt) => self.build_for_of_statement_doc(stmt),
            Statement::WhileStatement(stmt) => self.build_while_statement_doc(stmt),
            Statement::DoWhileStatement(stmt) => self.build_do_while_statement_doc(stmt),
            Statement::SwitchStatement(stmt) => self.build_switch_statement_doc(stmt),
            Statement::TryStatement(stmt) => self.build_try_statement_doc(stmt),
            Statement::ThrowStatement(stmt) => self.build_throw_statement_doc(stmt),
            Statement::BreakStatement(stmt) => self.build_break_statement_doc(stmt),
            Statement::ContinueStatement(stmt) => self.build_continue_statement_doc(stmt),
            Statement::LabeledStatement(stmt) => self.build_labeled_statement_doc(stmt),
            Statement::EmptyStatement(_) => doc::text(";"),
            Statement::TSInterfaceDeclaration(decl) => self.build_interface_declaration_doc(decl),
            Statement::TSDeclareFunction(decl) => self.build_declare_function_doc(decl),
            Statement::TSEnumDeclaration(decl) => self.build_enum_declaration_doc(decl),
            Statement::TSModuleDeclaration(decl) => self.build_module_declaration_doc(decl),
        }
    }

    /// Print an expression statement (expression followed by semicolon)
    fn print_expression_statement(&mut self, stmt: &internal::ExpressionStatement) {
        // Object pattern assignments need parentheses to avoid ambiguity with block statements
        // e.g., `({a, b} = obj);` not `{a, b} = obj;`
        let needs_parens = self.expression_statement_needs_parens(&stmt.expression);

        if needs_parens {
            self.write("(");
        }
        self.print_expression(&stmt.expression);
        if needs_parens {
            self.write(")");
        }

        // For expression statements, prettier keeps comments BEFORE the semicolon.
        // This differs from variable declarations which move comments after the semicolon.
        // Example: `1 as const /* comment */;` stays as `1 as const /* comment */;`
        let expr_end = stmt.expression.span().end;
        // Find where the semicolon is (it's the last character of the statement)
        // Comments between expression end and statement end (excluding semicolon) go before
        let semicolon_pos = stmt.span.end.saturating_sub(1);
        if self.has_comments_between(expr_end, semicolon_pos) {
            self.print_inline_comments_between(expr_end, semicolon_pos);
        }

        self.write(";");

        // Print trailing comments after the semicolon (on same line only)
        self.print_trailing_same_line_comments(stmt.span.end);
    }

    /// Print trailing comments on the same line after a position
    fn print_trailing_same_line_comments(&mut self, after_pos: u32) {
        let first_idx = tsv_lang::find_first_comment_from(self.comments, after_pos);
        for comment in &self.comments[first_idx..] {
            if tsv_lang::printing::is_same_line(self.source, after_pos, comment.span.start) {
                self.write(" ");
                self.print_comment(comment);
            } else {
                break;
            }
        }
    }

    /// Check if an expression statement needs parentheses
    ///
    /// Object pattern assignments need parens to avoid ambiguity with block statements.
    /// Array pattern assignments don't need parens (no ambiguity with array literal).
    fn expression_statement_needs_parens(&self, expr: &internal::Expression) -> bool {
        expression_needs_parens_at_statement_level(expr)
    }
}

/// Check if an expression needs parentheses at statement level
///
/// Separate function to avoid clippy warning about &self only used in recursion.
fn expression_needs_parens_at_statement_level(expr: &internal::Expression) -> bool {
    match expr {
        internal::Expression::AssignmentExpression(assign) => {
            matches!(assign.left.as_ref(), internal::Expression::ObjectPattern(_))
        }
        // Sequence expressions with object pattern assignment in first position also need parens
        internal::Expression::SequenceExpression(seq) => {
            if let Some(first) = seq.expressions.first() {
                expression_needs_parens_at_statement_level(first)
            } else {
                false
            }
        }
        _ => false,
    }
}
