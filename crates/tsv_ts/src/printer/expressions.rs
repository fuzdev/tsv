// Expression printing for TypeScript
//
// Handles printing of different expression types:
// - Literals (strings, numbers, booleans, etc.)
// - Identifiers (variable names, function names, etc.)
// - Future: Binary operations, function calls, member access, etc.

use super::Printer;
use crate::ast::internal::{self, Expression, LiteralValue};
use tsv_lang::printing::{is_same_line, format_string_literal, StringFormatOptions};
use tsv_lang::SymbolResolver;

impl<'a> Printer<'a> {
    /// Print an expression
    pub(crate) fn print_expression(&mut self, expression: &Expression) {
        // TODO: Add comment support for additional expression types as they're implemented
        // Future expressions that will need comment handling:
        //   - ArrayExpression: Comments before/after elements, trailing comma
        //   - CallExpression: Comments around arguments, after callee
        //   - MemberExpression: Comments around dot/bracket notation
        //   - BinaryExpression: Comments around operators (+, -, *, etc.)
        //   - ConditionalExpression: Comments around ?, : in ternary
        //   - ArrowFunctionExpression: Comments in parameter lists, around =>
        //   - TemplateLiteral: Comments in template expressions ${}
        // Reference: Current object expression implementation as pattern
        match expression {
            Expression::Literal(lit) => self.print_literal(lit),
            Expression::Identifier(id) => self.print_identifier(id),
            Expression::ObjectExpression(obj) => self.print_object_expression(obj),
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

    /// Print an object expression: `{ prop: value, ... }`
    fn print_object_expression(&mut self, obj: &internal::ObjectExpression) {
        // TODO: Improve multiline detection heuristic
        // Current: Multiline if object contains ANY comments
        // Better: Also consider:
        //   - Line width exceeded (sum of property lengths > print_width)
        //   - Nested objects/arrays (recursively check for Expression::ObjectExpression)
        //   - Number of properties (>3 properties might benefit from multiline)
        //   - Original source formatting (preserve user intent)
        // See prettier's willBreak() logic for comprehensive approach

        // Check if object contains comments (use multiline if so)
        let has_comments = self.has_comments_between(obj.span.start, obj.span.end);

        self.write("{");

        if !obj.properties.is_empty() {
            if has_comments {
                // Multiline formatting with comments
                self.write("\n");
                self.indent_level += 1;

                let mut prev_end = obj.span.start + 1; // After opening brace

                for prop in &obj.properties {
                    // Print leading comments before property
                    self.print_leading_comments(prev_end, prop.span.start);

                    self.write_indent();
                    self.print_expression(&prop.key);

                    // Print colon and value (unless shorthand)
                    if !prop.shorthand {
                        self.write(": ");
                        self.print_expression(&prop.value);
                    }

                    // Print trailing inline comment after property value (same line only)
                    let prop_end = if prop.shorthand {
                        prop.key.span().end
                    } else {
                        prop.value.span().end
                    };

                    // Print trailing inline comments on same line as property
                    // Block comments go before comma, line comments go after comma (matches prettier)
                    let mut has_line_comment = false;
                    for comment in self.comments.iter() {
                        if comment.span.start >= prop_end
                            && comment.span.start < obj.span.end
                            && is_same_line(self.source, prop_end, comment.span.start)
                        {
                            if comment.is_block {
                                // Block comment: print before comma
                                self.write(" ");
                                self.print_comment(comment);
                            } else {
                                // Line comment: defer until after comma
                                has_line_comment = true;
                            }
                        }
                    }

                    self.write(",");

                    // Print line comments after comma
                    if has_line_comment {
                        for comment in self.comments.iter() {
                            if comment.span.start >= prop_end
                                && comment.span.start < obj.span.end
                                && is_same_line(self.source, prop_end, comment.span.start)
                                && !comment.is_block
                            {
                                self.write(" ");
                                self.print_comment(comment);
                            }
                        }
                    }

                    self.write("\n");
                    prev_end = prop.span.end;
                }

                // Print any final comments before closing brace
                self.print_leading_comments(prev_end, obj.span.end);

                self.indent_level -= 1;
                self.write_indent();
            } else {
                // Single-line formatting (no comments)
                self.write(" ");

                for (i, prop) in obj.properties.iter().enumerate() {
                    self.print_expression(&prop.key);

                    if !prop.shorthand {
                        self.write(": ");
                        self.print_expression(&prop.value);
                    }

                    if i < obj.properties.len() - 1 {
                        self.write(", ");
                    }
                }

                self.write(" ");
            }
        }

        self.write("}");
    }
}
