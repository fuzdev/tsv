// Expression printing for TypeScript
//
// Handles printing of different expression types:
// - Literals (strings, numbers, booleans, etc.)
// - Identifiers (variable names, function names, etc.)
// - Future: Binary operations, function calls, member access, etc.

use super::Printer;
use crate::ast::internal::{self, Expression, LiteralValue};
use tsv_lang::printing::{StringFormatOptions, format_string_literal};

impl<'a> Printer<'a> {
    /// Print an expression
    pub(crate) fn print_expression(&mut self, expression: &Expression) {
        match expression {
            Expression::Literal(lit) => self.print_literal(lit),
            Expression::Identifier(id) => self.print_identifier(id),
        }
    }

    /// Print a literal value
    fn print_literal(&mut self, literal: &internal::Literal) {
        match &literal.value {
            LiteralValue::Number(n) => {
                // Format the number value
                // Note: This normalizes the format (hex/binary become decimal)
                // Future: preserve original format if needed
                self.write(&n.to_string());
            }
            LiteralValue::String { content: _, quote } => {
                // Extract raw literal from source (preserves escape sequences)
                let start = literal.span.start as usize;
                let end = literal.span.end as usize;
                let raw_literal = &self.source[start..end];

                // Extract content without surrounding quotes
                let raw_content = &raw_literal[1..raw_literal.len() - 1];

                // Format using shared utility (handles quote selection and escaping)
                let formatted =
                    format_string_literal(raw_content, *quote, StringFormatOptions::default());

                self.write(&formatted);
            }
        }
    }

    /// Print an identifier
    pub(super) fn print_identifier(&mut self, identifier: &internal::Identifier) {
        // Resolve symbol from interner using centralized helper
        let name = self.resolve_symbol(identifier.name);
        self.write(&name);

        // Handle type annotations
        if let Some(type_annotation) = &identifier.type_annotation {
            self.print_type_annotation(type_annotation);
        }
    }
}
