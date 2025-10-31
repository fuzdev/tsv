// Expression formatting for TypeScript
//
// Handles formatting of different expression types:
// - Literals (strings, numbers, booleans, etc.)
// - Identifiers (variable names, function names, etc.)
// - Future: Binary operations, function calls, member access, etc.

use crate::ast::internal::{self, Expression};
use crate::formatter_core::Formatter;

impl Formatter {
    /// Format an expression
    pub(crate) fn format_expression(&mut self, expression: &Expression) {
        match expression {
            Expression::Literal(lit) => self.format_literal(lit),
            Expression::Identifier(id) => self.format_identifier(id),
        }
    }

    /// Format a literal value
    fn format_literal(&mut self, literal: &internal::Literal) {
        // Use the raw string representation from the parser
        // This preserves the original numeric format (decimal, hex, binary, etc.)
        self.write(&literal.raw);
    }

    /// Format an identifier
    pub(super) fn format_identifier(&mut self, identifier: &internal::Identifier) {
        // Resolve symbol from interner using centralized helper
        let name = self.resolve_symbol(identifier.name);
        self.write(&name);

        // Handle type annotations
        if let Some(type_annotation) = &identifier.type_annotation {
            self.format_type_annotation(type_annotation);
        }
    }
}
