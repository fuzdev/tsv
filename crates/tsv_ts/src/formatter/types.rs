// Type annotation formatting for TypeScript
//
// Handles formatting of TypeScript-specific type syntax:
// - Type annotations (: Type)
// - Type keywords (number, string, boolean, etc.)
// - Future: Complex types (unions, intersections, generics, etc.)

use crate::ast::internal::{self, TSType};
use crate::formatter_core::Formatter;

impl Formatter {
    /// Format a TypeScript type annotation (e.g., `: number`)
    pub(super) fn format_type_annotation(&mut self, annotation: &internal::TSTypeAnnotation) {
        self.write(": ");
        self.format_ts_type(&annotation.type_annotation);
    }

    /// Format a TypeScript type expression
    fn format_ts_type(&mut self, ts_type: &TSType) {
        match ts_type {
            TSType::TSNumberKeyword(_) => self.write("number"),
            // TODO: TSStringKeyword, TSBooleanKeyword, etc.
        }
    }
}
