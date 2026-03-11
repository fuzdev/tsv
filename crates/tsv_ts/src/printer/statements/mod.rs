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
pub(super) use super::{
    Printer, build_entity_name_doc, intersection_has_huggable_last_type, should_hug_union_type,
    unwrap_parenthesized,
};

use super::{ParenContext, needs_parens};
use crate::ast::internal::{self, Expression, LiteralValue, Statement};
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Build a Doc for a statement
    pub(super) fn build_statement_doc(&self, statement: &Statement) -> DocId {
        let d = self.d();
        match statement {
            Statement::ExpressionStatement(stmt) => self.build_expression_statement_doc(stmt),
            Statement::VariableDeclaration(decl) => self.build_variable_declaration_doc(decl),
            Statement::TSTypeAliasDeclaration(decl) => self.build_type_alias_declaration_doc(decl),
            Statement::ReturnStatement(ret) => self.build_return_statement_doc(ret),
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
            Statement::EmptyStatement(_) => d.text(";"),
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
            if self.is_same_line(after_pos, comment.span.start) {
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
    /// Preserves source parens around string literals: `('hello');` stays parenthesized.
    fn build_expression_statement_doc(&self, stmt: &internal::ExpressionStatement) -> DocId {
        let d = self.d();
        // Parens required for correctness (object expressions, object pattern assignments)
        // OR preserved from source for string literals (matches Prettier behavior)
        let needs_parens = needs_parens(&stmt.expression, ParenContext::ExpressionStatement)
            || self.has_expression_statement_source_parens(stmt);

        let mut parts = Vec::new();

        if needs_parens {
            parts.push(d.text("("));
        }

        // Set context flags for chain handling
        // is_expression_statement: allows short identifier names to merge with first call
        // in_top_level_assignment: tells assignments to use regular layout (not chain formatting)
        self.is_expression_statement.set(true);
        self.in_top_level_assignment.set(true);
        parts.push(self.build_expression_doc(&stmt.expression));
        self.in_top_level_assignment.set(false);
        self.is_expression_statement.set(false);

        if needs_parens {
            parts.push(d.text(")"));
        }

        // Handle comments before semicolon
        // Prettier keeps comments BEFORE the semicolon in expression statements
        let expr_end = stmt.expression.span().end;
        let semicolon_pos = stmt.span.end.saturating_sub(1);
        if let Some(comments_doc) =
            self.build_inline_comments_between_doc_opt(expr_end, semicolon_pos)
        {
            parts.push(comments_doc);
        }

        parts.push(d.text(";"));
        d.concat(&parts)
    }

    /// Check if an expression statement had parentheses in the source that should be preserved.
    ///
    /// Prettier preserves parens around string literal expression statements:
    /// `('hello');` stays as-is, not stripped to `'hello';`.
    /// Detected via span: if ExpressionStatement.span.start < Expression.span.start,
    /// the source had a `(` before the expression.
    fn has_expression_statement_source_parens(&self, stmt: &internal::ExpressionStatement) -> bool {
        if stmt.span.start >= stmt.expression.span().start {
            return false;
        }
        matches!(
            &stmt.expression,
            Expression::Literal(lit) if matches!(lit.value, LiteralValue::String { .. })
        )
    }

    /// Build a Doc for a return statement.
    ///
    /// Prettier's `printReturnOrThrowArgument` (function.js:231-277) wraps in parens
    /// for two cases:
    /// 1. Own-line comments in chain — unconditional: `return (\n  a\n    // comment\n    .b()\n);`
    /// 2. Binaryish arguments — conditional: `return (\n  a +\n  b\n);` (only when breaking)
    fn build_return_statement_doc(&self, ret: &internal::ReturnStatement) -> DocId {
        let d = self.d();
        let Some(arg) = &ret.argument else {
            return d.text("return;");
        };

        // Prettier wraps return args with own-line comments in unconditional parens
        // (function.js:238-239, returnArgumentHasLeadingComment)
        if self.return_argument_has_own_line_comment(ret.span.start, arg) {
            return self.build_return_or_throw_comment_paren_doc("return", arg);
        }

        // Prettier wraps all binaryish (binary + logical) return args in parens when breaking
        if let Expression::BinaryExpression(binary) = arg {
            return self.build_return_or_throw_binary_doc("return", binary);
        }

        // Assignment expressions need parentheses: return (a = b);
        // This matches Prettier's behavior for clarity (assignment vs equality)
        if matches!(arg, Expression::AssignmentExpression(_)) {
            return d.concat(&[
                d.text("return ("),
                self.build_expression_doc(arg),
                d.text(");"),
            ]);
        }

        // Standard case: no parenthesization needed
        d.concat(&[
            d.text("return "),
            self.build_expression_doc(arg),
            d.text(";"),
        ])
    }

    /// Check if a return/throw argument has own-line comments that require
    /// unconditional paren wrapping.
    ///
    /// Matches Prettier's `returnArgumentHasLeadingComment` (function.js:290-318):
    /// walks the left side of chainable expressions (calls, members, etc.)
    /// looking for own-line comments. If found, the return/throw value gets
    /// wrapped in `("(", indent([hardline, doc]), hardline, ")")`.
    fn return_argument_has_own_line_comment(&self, keyword_start: u32, arg: &Expression) -> bool {
        // Check for own-line comments before the argument itself
        // (e.g., `return // comment\n expr`)
        if self.has_leading_own_line_comment_in_range(keyword_start, arg.span().start) {
            return true;
        }

        // Walk the left side of chainable expressions checking for own-line comments
        self.chain_has_own_line_comment(arg)
    }

    /// Walk the left side of a chain looking for leading own-line comments.
    ///
    /// Mirrors Prettier's `hasNakedLeftSide` + `getLeftSide` walk with
    /// `hasLeadingOwnLineComment` check at each node. Only counts comments
    /// that are on their own line (not trailing comments on the same line
    /// as the preceding expression).
    fn chain_has_own_line_comment(&self, expr: &Expression) -> bool {
        match expr {
            Expression::CallExpression(call) => self.chain_has_own_line_comment(&call.callee),
            Expression::MemberExpression(member) => {
                // Check for leading own-line comments between object and property.
                // Must NOT be on the same line as the object — trailing comments
                // like `foo() // comment` don't trigger paren wrapping.
                let obj_end = member.object.span().end;
                let prop_start = member.property.span().start;
                if self.has_leading_own_line_comment_in_range(obj_end, prop_start) {
                    return true;
                }
                self.chain_has_own_line_comment(&member.object)
            }
            Expression::TSNonNullExpression(non_null) => {
                self.chain_has_own_line_comment(&non_null.expression)
            }
            Expression::TaggedTemplateExpression(tagged) => {
                self.chain_has_own_line_comment(&tagged.tag)
            }
            _ => false,
        }
    }

    /// Check if there are any leading own-line comments in a range.
    ///
    /// "Leading own-line" means the comment is NOT on the same line as `start`
    /// (i.e., it's on its own line, not trailing the previous expression).
    /// This matches Prettier's `hasLeadingOwnLineComment` which checks for
    /// comments with a newline after them that are leading on a node.
    fn has_leading_own_line_comment_in_range(&self, start: u32, end: u32) -> bool {
        tsv_lang::comments_in_range(self.comments, start, end)
            .any(|c| !self.is_same_line(start, c.span.start))
    }

    /// Build unconditional paren-wrapped doc for return/throw with own-line comments.
    ///
    /// Matches Prettier's `["(", indent([hardline, argumentDoc]), hardline, ")"]`
    /// (function.js:239). Unlike the binaryish case which uses `ifBreak`, this is
    /// unconditional because the comment placement makes the line break semantically necessary.
    fn build_return_or_throw_comment_paren_doc(
        &self,
        keyword: &'static str,
        arg: &Expression,
    ) -> DocId {
        let d = self.d();
        let expr_doc = self.build_expression_doc(arg);
        d.concat(&[
            d.text(keyword),
            d.text(" ("),
            d.indent(d.concat(&[d.hardline(), expr_doc])),
            d.hardline(),
            d.text(");"),
        ])
    }

    /// Shared logic for return/throw with binaryish arguments.
    ///
    /// Matches Prettier's `printReturnOrThrowArgument` (function.js:240-252):
    /// when the argument is `isBinaryish`, wraps in `ifBreak("(")...ifBreak(")")`.
    pub(super) fn build_return_or_throw_binary_doc(
        &self,
        keyword: &'static str,
        binary: &internal::BinaryExpression,
    ) -> DocId {
        let d = self.d();
        let expr_doc = self.build_binary_chain_doc_ungrouped(binary);

        // Find trailing comments between expression end and semicolon
        let expr_end = binary.span.end as usize;
        let semicolon_pos = self.source[expr_end..]
            .find(';')
            .map_or(expr_end, |i| expr_end + i);
        let trailing_comments_doc =
            self.build_inline_comments_between_doc(expr_end as u32, semicolon_pos as u32);

        // Broken: keyword (\n  expr\n);
        // Flat: keyword expr;
        let broken_doc = d.concat(&[
            d.text(" ("),
            d.indent(d.concat(&[d.softline(), d.group(expr_doc), trailing_comments_doc])),
            d.softline(),
            d.text(")"),
        ]);

        let flat_doc = d.concat(&[d.text(" "), expr_doc, trailing_comments_doc]);

        d.group(d.concat(&[
            d.text(keyword),
            d.if_break(broken_doc, flat_doc),
            d.text(";"),
        ]))
    }
}
