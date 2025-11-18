// Expression parsing

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::Parser;

impl<'a> Parser<'a> {
    // TODO: This currently parses only primary expressions (literals, identifiers, objects).
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
            TokenKind::BraceOpen => self.parse_object_expression(),
            _ => Err(ParseError::InvalidExpression {
                found: self.current_kind().to_string(),
                position: self.current_pos().0,
                context: None,
            }),
        }
    }

    /// Parse object literal: `{ prop: value, ... }`
    ///
    /// Currently supports:
    /// - Simple properties: `{ prop: value }`
    /// - Shorthand properties: `{ prop }` (key equals value)
    /// - Trailing commas: `{ a: 1, }`
    /// - Empty objects: `{}`
    ///
    /// TODO: Future enhancements for full JavaScript/TypeScript support:
    /// - Computed property names: `{ [expr]: value }`
    /// - Method shorthand: `{ foo() {} }`
    /// - Getter/setter: `{ get foo() {}, set foo(v) {} }`
    /// - Spread properties: `{ ...obj }`
    /// - String/number literal keys: `{ "key": value, 123: value }`
    fn parse_object_expression(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();
        self.expect(&TokenKind::BraceOpen)?; // consume '{'

        let mut properties = Vec::new();

        // Handle empty object: `{}`
        if self.check(&TokenKind::BraceClose) {
            let (_, end) = self.current_pos();
            self.advance()?; // consume '}'
            return Ok(Expression::ObjectExpression(ObjectExpression {
                properties,
                span: Span::new(start as u32, end as u32),
            }));
        }

        // Parse properties
        loop {
            let prop_start = self.current_pos().0;

            // TODO: Extract to parse_property_key() method for better extensibility
            // Should support:
            // - Identifiers (currently implemented)
            // - String literals: { "prop-name": value }
            // - Number literals: { 123: value }
            // - Computed properties: { [expr]: value }
            // See: ECMAScript spec PropertyDefinition

            // Parse property key (currently only identifiers supported)
            let key = match self.current_kind() {
                TokenKind::Identifier => {
                    let (key_start, key_end) = self.current_pos();
                    let symbol = self.intern(self.current_value());
                    self.advance()?;
                    Expression::Identifier(Identifier {
                        name: symbol,
                        type_annotation: None,
                        span: Span::new(key_start as u32, key_end as u32),
                    })
                }
                _ => {
                    return Err(ParseError::InvalidSyntax {
                        message: format!(
                            "Expected property key, found {}",
                            self.current_kind()
                        ),
                        position: prop_start,
                        context: None,
                    })
                }
            };

            // Check for shorthand property: `{ prop }` vs `{ prop: value }`
            let (value, shorthand) = if self.check(&TokenKind::Colon) {
                self.advance()?; // consume ':'
                (self.parse_expression()?, false)
            } else {
                // Shorthand: key is duplicated as value
                (key.clone(), true)
            };

            // Property span should end at value's end, not including trailing comments
            let prop_end = value.span().end as usize;
            properties.push(Property {
                key,
                value,
                shorthand,
                // TODO: Support computed property names: { [expr]: value }
                // When TokenKind::BracketOpen is found at property key position,
                // parse as computed property and set computed: true
                // See: ECMAScript spec ComputedPropertyName
                computed: false,
                // TODO: Support method shorthand: { foo() {} }
                // Detect when property value is a function and key matches method syntax
                // Set method: true and adjust value to FunctionExpression
                // Also needed: getter/setter syntax { get foo() {}, set foo(v) {} }
                // See: ECMAScript spec MethodDefinition
                method: false,
                span: Span::new(prop_start as u32, prop_end as u32),
            });

            // TODO: Support spread properties: { ...obj }
            // When TokenKind::DotDotDot is found, parse as SpreadElement
            // Requires new AST node type or Property variant
            // See: ECMAScript spec SpreadElement in ObjectLiteral

            // TODO: Refactor to use expect_list_separator() helper method
            // This comma/terminator pattern is duplicated across:
            // - Object properties (here)
            // - Array elements (future)
            // - Function parameters (future)
            // - Type parameters (future)
            // Extract to Parser helper for consistency

            // Check for comma or closing brace
            if self.check(&TokenKind::Comma) {
                self.advance()?; // consume ','
                // Allow trailing comma: `{ a: 1, }`
                if self.check(&TokenKind::BraceClose) {
                    break;
                }
            } else if self.check(&TokenKind::BraceClose) {
                break;
            } else {
                return Err(ParseError::InvalidSyntax {
                    message: format!(
                        "Expected ',' or '}}' after property, found {}",
                        self.current_kind()
                    ),
                    position: self.current_pos().0,
                    context: None,
                });
            }
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?; // consume '}'

        Ok(Expression::ObjectExpression(ObjectExpression {
            properties,
            span: Span::new(start as u32, end as u32),
        }))
    }
}
