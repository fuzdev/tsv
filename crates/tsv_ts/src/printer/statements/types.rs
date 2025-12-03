// Type-related statement printing for TypeScript

use super::super::Printer;
use crate::ast::internal;

impl<'a> Printer<'a> {
    /// Print a type alias declaration: `type X = T`
    pub(super) fn print_type_alias_declaration(&mut self, decl: &internal::TSTypeAliasDeclaration) {
        self.write("type ");
        self.print_identifier(&decl.id);
        self.write(" = ");
        self.print_type(&decl.type_annotation);
        self.write(";");
    }

    /// Print a return statement: `return expr;` or `return;`
    pub(super) fn print_return_statement(&mut self, ret: &internal::ReturnStatement) {
        self.write("return");
        if let Some(arg) = &ret.argument {
            self.write(" ");
            self.print_expression(arg);
        }
        self.write(";");
    }
}
