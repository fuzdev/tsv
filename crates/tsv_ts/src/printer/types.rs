// Type annotation printing for TypeScript
//
// Handles printing of TypeScript-specific type syntax:
// - Type annotations (: Type)
// - Type keywords (number, string, boolean, etc.)
// - Future: Complex types (unions, intersections, generics, etc.)

use super::Printer;
use crate::ast::internal::{self, TSType};

impl<'a> Printer<'a> {
    /// Print a TypeScript type annotation (e.g., `: number`)
    pub(super) fn print_type_annotation(&mut self, annotation: &internal::TSTypeAnnotation) {
        self.write(": ");
        self.print_ts_type(&annotation.type_annotation);
    }

    /// Print a TypeScript type expression
    fn print_ts_type(&mut self, ts_type: &TSType) {
        match ts_type {
            TSType::TSNumberKeyword(_) => self.write("number"),
            // TODO: TSStringKeyword, TSBooleanKeyword, etc.
        }
    }
}
