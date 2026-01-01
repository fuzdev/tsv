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

// Re-export for submodules to use `super::Printer` instead of `super::super::Printer`
pub(super) use super::{Printer, build_entity_name_doc};

use super::{ParenContext, needs_parens};
use crate::ast::internal::{self, Statement};
use tsv_lang::doc;

impl<'a> Printer<'a> {
    /// Build a Doc for a statement
    pub(super) fn build_statement_doc(&self, statement: &Statement) -> doc::Doc {
        match statement {
            Statement::ExpressionStatement(stmt) => self.build_expression_statement_doc(stmt),
            Statement::VariableDeclaration(decl) => self.build_variable_declaration_doc(decl),
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

    /// Print trailing comments on the same line after a position
    pub(super) fn print_trailing_same_line_comments(&mut self, after_pos: u32) {
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

    /// Build a Doc for an expression statement
    ///
    /// Handles parentheses for object patterns and comments before semicolon.
    fn build_expression_statement_doc(&self, stmt: &internal::ExpressionStatement) -> doc::Doc {
        let needs_parens = needs_parens(&stmt.expression, ParenContext::ExpressionStatement);

        let mut parts = Vec::new();

        if needs_parens {
            parts.push(doc::text("("));
        }
        parts.push(self.build_expression_doc(&stmt.expression));
        if needs_parens {
            parts.push(doc::text(")"));
        }

        // Handle comments before semicolon
        // Prettier keeps comments BEFORE the semicolon in expression statements
        let expr_end = stmt.expression.span().end;
        let semicolon_pos = stmt.span.end.saturating_sub(1);
        if self.has_comments_between(expr_end, semicolon_pos) {
            parts.push(self.build_inline_comments_between_doc(expr_end, semicolon_pos));
        }

        parts.push(doc::text(";"));
        doc::concat(parts)
    }
}
