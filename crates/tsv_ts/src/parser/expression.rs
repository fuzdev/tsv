// Expression parsing

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::Parser;

impl<'a> Parser<'a> {
    // TODO: This currently parses only primary expressions (literals, identifiers).
    // For binary/unary expressions, we'll need:
    // 1. Operator precedence handling (Pratt parsing or precedence climbing)
    // 2. Support for: +, -, *, /, %, ==, !=, <, >, <=, >=, &&, ||, etc.
    // 3. Unary operators: !, -, +, typeof, void, etc.
    // 4. Parenthesized expressions
    // 5. Call expressions, member expressions, etc.
    pub(super) fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        match self.current_kind() {
            TokenKind::Number => {
                let (start, end) = self.current_pos();
                let raw = self.current_value().to_string();
                let number = raw.parse().map_err(|_| ParseError::InvalidSyntax {
                    message: format!("Invalid number: {}", raw),
                    position: start,
                    context: None,
                })?;
                self.advance()?;
                Ok(Expression::Literal(Literal {
                    value: LiteralValue::Number(number),
                    span: Span::new(start as u32, end as u32),
                }))
            }
            TokenKind::String => {
                let (start, end) = self.current_pos();
                let raw = self.current_value().to_string();

                // Extract quote character (first char of raw string)
                let quote = raw.chars().next().unwrap_or('"');

                // Use decoded value from lexer (escapes already processed)
                // If no decoded value, extract content without quotes (no escapes present)
                let content = if let Some(decoded) = self.current_decoded() {
                    decoded.to_string()
                } else {
                    // No escapes - extract content between quotes
                    if raw.len() >= 2 {
                        raw[1..raw.len() - 1].to_string()
                    } else {
                        String::new()
                    }
                };

                self.advance()?;
                Ok(Expression::Literal(Literal {
                    value: LiteralValue::String { content, quote },
                    span: Span::new(start as u32, end as u32),
                }))
            }
            TokenKind::Identifier => {
                let (start, end) = self.current_pos();
                let symbol = self.intern(self.current_value());
                self.advance()?;
                Ok(Expression::Identifier(Identifier {
                    name: symbol,
                    // TODO: Type annotations not yet supported in expression contexts.
                    // When implementing function parameters/signatures (e.g., `function foo(x: number)`),
                    // identifiers in expressions may need type annotation support.
                    // Currently only variable declarations support type annotations (see statement.rs).
                    type_annotation: None,
                    span: Span::new(start as u32, end as u32),
                }))
            }
            _ => Err(ParseError::InvalidExpression {
                found: self.current_kind().to_string(),
                position: self.current_pos().0,
                context: None,
            }),
        }
    }
}
