// Expression parsing using Pratt parser for operator precedence

use crate::ast::internal::{
    ArrayExpression, ArrowFunctionBody, ArrowFunctionExpression, BinaryExpression, BinaryOperator,
    CallExpression, ConditionalExpression, Expression, Identifier, Literal, LiteralValue,
    MemberExpression, ObjectExpression, ObjectProperty, Property, SpreadElement, UnaryExpression,
    UnaryOperator,
};
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::Parser;

/// Parsed expression with actual end position tracking
///
/// Used during parsing to track where expressions truly end after consuming
/// parentheses. This allows binary expressions to correctly include opening/closing
/// parens in their spans while keeping inner expression spans semantic.
///
/// Example: `(a && b) || c`
/// - Inner `&&` expression: span = `a && b` (semantic, excludes parens)
/// - Outer `||` expression: span starts at `(`, uses `actual_end` from left operand
#[derive(Debug)]
struct ParsedExpr {
    /// The parsed expression with semantic span (may exclude surrounding parens)
    expr: Expression,
    /// Actual end position after consuming any closing parentheses
    actual_end: usize,
}

impl ParsedExpr {
    /// Create a ParsedExpr where actual_end matches the expression's semantic span
    fn from_expr(expr: Expression) -> Self {
        let actual_end = expr.span().end as usize;
        Self { expr, actual_end }
    }

    /// Create a ParsedExpr with explicit actual_end (for parenthesized expressions)
    fn with_end(expr: Expression, actual_end: usize) -> Self {
        Self { expr, actual_end }
    }
}

/// Get infix binding power for a token (left and right)
/// Returns None if not an infix operator
/// Uses standard JavaScript operator precedence
fn infix_binding_power(kind: &TokenKind) -> Option<(u8, u8)> {
    // Binding power: higher = binds tighter
    // (left_bp, right_bp): left_bp < right_bp for right-associative
    match kind {
        // Nullish coalescing: lowest binary precedence
        TokenKind::QuestionQuestion => Some((5, 6)),
        // Logical OR
        TokenKind::PipePipe => Some((7, 8)),
        // Logical AND
        TokenKind::AmpersandAmpersand => Some((9, 10)),
        // Bitwise OR
        TokenKind::Pipe => Some((11, 12)),
        // Bitwise AND
        TokenKind::Ampersand => Some((13, 14)),
        // Equality
        TokenKind::EqualsEquals
        | TokenKind::BangEquals
        | TokenKind::EqualsEqualsEquals
        | TokenKind::BangEqualsEquals => Some((15, 16)),
        // Relational
        TokenKind::LessThan
        | TokenKind::GreaterThan
        | TokenKind::LessThanEquals
        | TokenKind::GreaterThanEquals => Some((17, 18)),
        // Additive
        TokenKind::Plus | TokenKind::Minus => Some((19, 20)),
        // Multiplicative
        TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Some((21, 22)),
        _ => None,
    }
}

/// Convert token kind to binary operator
fn token_to_binary_operator(kind: &TokenKind) -> Option<BinaryOperator> {
    match kind {
        TokenKind::Plus => Some(BinaryOperator::Plus),
        TokenKind::Minus => Some(BinaryOperator::Minus),
        TokenKind::Star => Some(BinaryOperator::Star),
        TokenKind::Slash => Some(BinaryOperator::Slash),
        TokenKind::Percent => Some(BinaryOperator::Percent),
        TokenKind::LessThan => Some(BinaryOperator::LessThan),
        TokenKind::GreaterThan => Some(BinaryOperator::GreaterThan),
        TokenKind::LessThanEquals => Some(BinaryOperator::LessThanEquals),
        TokenKind::GreaterThanEquals => Some(BinaryOperator::GreaterThanEquals),
        TokenKind::EqualsEquals => Some(BinaryOperator::EqualsEquals),
        TokenKind::EqualsEqualsEquals => Some(BinaryOperator::EqualsEqualsEquals),
        TokenKind::BangEquals => Some(BinaryOperator::BangEquals),
        TokenKind::BangEqualsEquals => Some(BinaryOperator::BangEqualsEquals),
        TokenKind::AmpersandAmpersand => Some(BinaryOperator::AmpersandAmpersand),
        TokenKind::PipePipe => Some(BinaryOperator::PipePipe),
        TokenKind::QuestionQuestion => Some(BinaryOperator::QuestionQuestion),
        TokenKind::Ampersand => Some(BinaryOperator::Ampersand),
        TokenKind::Pipe => Some(BinaryOperator::Pipe),
        _ => None,
    }
}

impl<'a> Parser<'a> {
    /// Parse an expression using Pratt parsing for operator precedence
    pub(super) fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        Ok(self.parse_expression_bp(0)?.expr)
    }

    /// Pratt parser: parse expression with minimum binding power
    ///
    /// Returns ParsedExpr with actual end position tracking for parentheses
    fn parse_expression_bp(&mut self, min_bp: u8) -> Result<ParsedExpr, ParseError> {
        // Track the true start position (before any parentheses)
        // This is needed because grouped expressions like (a && b) should have their
        // containing binary expression span include the opening paren
        let expr_start = self.current_pos().0;

        // Parse prefix expression (primary or unary)
        let mut left = self.parse_prefix_expression_with_end()?;

        // Parse infix operators
        loop {
            let kind = self.current_kind().clone();

            // Check if this is an infix operator
            let Some((left_bp, right_bp)) = infix_binding_power(&kind) else {
                break;
            };

            // Check if operator binds tighter than minimum
            if left_bp < min_bp {
                break;
            }

            // Get operator
            let operator = token_to_binary_operator(&kind).unwrap();
            self.advance()?; // consume operator

            // Parse right-hand side with right binding power
            let right = self.parse_expression_bp(right_bp)?;

            // Create binary expression
            // Use expr_start (which includes any opening paren) instead of left.expr.span().start
            // Use right.actual_end (position after parsing) to include closing parens
            let span = Span::new(expr_start as u32, right.actual_end as u32);
            left = ParsedExpr {
                expr: Expression::BinaryExpression(BinaryExpression {
                    left: Box::new(left.expr),
                    operator,
                    right: Box::new(right.expr),
                    span,
                }),
                actual_end: right.actual_end,
            };
        }

        // Handle ternary operator (lowest precedence, above comma)
        // Only handle at top level (min_bp == 0) to ensure proper precedence
        if min_bp == 0 && self.check(&TokenKind::Question) {
            self.advance()?; // consume '?'

            // Parse consequent (then branch)
            let consequent = self.parse_expression_bp(0)?;

            // Expect ':'
            self.expect(&TokenKind::Colon)?;

            // Parse alternate (else branch)
            let alternate = self.parse_expression_bp(0)?;

            let span = Span::new(expr_start as u32, alternate.actual_end as u32);
            left = ParsedExpr {
                expr: Expression::ConditionalExpression(ConditionalExpression {
                    test: Box::new(left.expr),
                    consequent: Box::new(consequent.expr),
                    alternate: Box::new(alternate.expr),
                    span,
                }),
                actual_end: alternate.actual_end,
            };
        }

        Ok(left)
    }

    /// Parse prefix expression returning ParsedExpr with actual end position
    fn parse_prefix_expression_with_end(&mut self) -> Result<ParsedExpr, ParseError> {
        let parsed = match self.current_kind() {
            TokenKind::Minus | TokenKind::Plus | TokenKind::Bang => {
                let expr = self.parse_unary_expression()?;
                ParsedExpr::from_expr(expr)
            }
            _ => self.parse_primary_expression_with_end()?,
        };

        // Parse any postfix operations (member access, call expressions)
        self.parse_postfix_expression(parsed)
    }

    /// Parse postfix expressions: member access (`.`, `[`), call expressions (`()`)
    ///
    /// Handles chained expressions like `obj.prop`, `arr[0]`, `foo()`, `obj.method().prop`
    fn parse_postfix_expression(&mut self, mut left: ParsedExpr) -> Result<ParsedExpr, ParseError> {
        loop {
            match self.current_kind() {
                TokenKind::Dot => {
                    // Member access: obj.prop
                    self.advance()?; // consume '.'

                    // Property must be an identifier
                    if self.current_kind() != TokenKind::Identifier {
                        return Err(ParseError::InvalidSyntax {
                            message: format!(
                                "Expected property name after '.', found {}",
                                self.current_kind()
                            ),
                            position: self.current_pos().0,
                            context: None,
                        });
                    }

                    let (prop_start, prop_end) = self.current_pos();
                    let name = self.intern(self.current_value());
                    self.advance()?;

                    let span = Span::new(left.expr.span().start, prop_end as u32);
                    left = ParsedExpr::with_end(
                        Expression::MemberExpression(MemberExpression {
                            object: Box::new(left.expr),
                            property: Box::new(Expression::Identifier(Identifier {
                                name,
                                type_annotation: None,
                                span: Span::new(prop_start as u32, prop_end as u32),
                            })),
                            computed: false,
                            optional: false,
                            span,
                        }),
                        prop_end,
                    );
                }
                TokenKind::BracketOpen => {
                    // Computed member access: arr[0]
                    self.advance()?; // consume '['

                    let index = self.parse_expression()?;

                    let (_, bracket_end) = self.current_pos();
                    self.expect(&TokenKind::BracketClose)?; // consume ']'

                    let span = Span::new(left.expr.span().start, bracket_end as u32);
                    left = ParsedExpr::with_end(
                        Expression::MemberExpression(MemberExpression {
                            object: Box::new(left.expr),
                            property: Box::new(index),
                            computed: true,
                            optional: false,
                            span,
                        }),
                        bracket_end,
                    );
                }
                TokenKind::ParenOpen => {
                    // Call expression: foo()
                    self.advance()?; // consume '('

                    let mut arguments = Vec::new();

                    // Parse arguments
                    if !self.check(&TokenKind::ParenClose) {
                        loop {
                            let arg = self.parse_expression()?;
                            arguments.push(arg);

                            if !self
                                .expect_list_separator(&TokenKind::Comma, &TokenKind::ParenClose)?
                            {
                                break;
                            }
                        }
                    }

                    let (_, paren_end) = self.current_pos();
                    self.expect(&TokenKind::ParenClose)?; // consume ')'

                    let span = Span::new(left.expr.span().start, paren_end as u32);
                    left = ParsedExpr::with_end(
                        Expression::CallExpression(CallExpression {
                            callee: Box::new(left.expr),
                            arguments,
                            optional: false,
                            span,
                        }),
                        paren_end,
                    );
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse primary expression returning ParsedExpr with actual end position
    fn parse_primary_expression_with_end(&mut self) -> Result<ParsedExpr, ParseError> {
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
                Ok(ParsedExpr::with_end(
                    Expression::Literal(Literal {
                        value: LiteralValue::Number(number),
                        span: Span::new(start as u32, end as u32),
                    }),
                    end,
                ))
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
                Ok(ParsedExpr::with_end(
                    Expression::Literal(Literal {
                        value: LiteralValue::String { content, quote },
                        span: Span::new(start as u32, end as u32),
                    }),
                    end,
                ))
            }
            TokenKind::Identifier => {
                let (start, end) = self.current_pos();
                let symbol = self.intern(self.current_value());
                self.advance()?;
                Ok(ParsedExpr::with_end(
                    Expression::Identifier(Identifier {
                        name: symbol,
                        type_annotation: None,
                        span: Span::new(start as u32, end as u32),
                    }),
                    end,
                ))
            }
            TokenKind::BraceOpen => Ok(ParsedExpr::from_expr(self.parse_object_expression()?)),
            TokenKind::BracketOpen => Ok(ParsedExpr::from_expr(self.parse_array_expression()?)),
            TokenKind::Keyword(KeywordKind::True) => {
                let (start, end) = self.current_pos();
                self.advance()?;
                Ok(ParsedExpr::with_end(
                    Expression::Literal(Literal {
                        value: LiteralValue::Boolean(true),
                        span: Span::new(start as u32, end as u32),
                    }),
                    end,
                ))
            }
            TokenKind::Keyword(KeywordKind::False) => {
                let (start, end) = self.current_pos();
                self.advance()?;
                Ok(ParsedExpr::with_end(
                    Expression::Literal(Literal {
                        value: LiteralValue::Boolean(false),
                        span: Span::new(start as u32, end as u32),
                    }),
                    end,
                ))
            }
            TokenKind::Keyword(KeywordKind::Null) => {
                let (start, end) = self.current_pos();
                self.advance()?;
                Ok(ParsedExpr::with_end(
                    Expression::Literal(Literal {
                        value: LiteralValue::Null,
                        span: Span::new(start as u32, end as u32),
                    }),
                    end,
                ))
            }
            TokenKind::ParenOpen => {
                // Could be: arrow function `() => ...` or parenthesized expression `(expr)`
                self.parse_paren_expression_with_end()
            }
            TokenKind::DotDotDot => Ok(ParsedExpr::from_expr(self.parse_spread_element()?)),
            _ => Err(ParseError::InvalidExpression {
                found: self.current_kind().to_string(),
                position: self.current_pos().0,
                context: None,
            }),
        }
    }

    /// Parse parenthesized expression or arrow function, returning ParsedExpr
    ///
    /// Distinguishes between:
    /// - Arrow function: `() => ...`, `(x) => ...`, `(x, y) => ...`
    /// - Grouped expression: `(expr)`
    ///
    /// Uses lookahead to detect arrow functions by scanning for `=>` after `)`.
    fn parse_paren_expression_with_end(&mut self) -> Result<ParsedExpr, ParseError> {
        // Check if this looks like an arrow function by scanning ahead
        if self.is_arrow_function_start() {
            return Ok(ParsedExpr::from_expr(self.parse_arrow_function()?));
        }

        // Parse as grouped expression: (expr)
        // This is the key case: we need to return the position AFTER the closing ')'
        self.expect(&TokenKind::ParenOpen)?; // consume '('

        let parsed = self.parse_expression_bp(0)?;

        // Capture the end position of ')' before consuming it
        let (_, paren_end) = self.current_pos();
        self.expect(&TokenKind::ParenClose)?; // consume ')'

        // Return expression with its original span (excluding parens), but with actual end after ')'
        Ok(ParsedExpr::with_end(parsed.expr, paren_end))
    }

    /// Check if current position starts an arrow function
    ///
    /// Scans ahead looking for pattern: `(` ... `)` `=>`
    fn is_arrow_function_start(&self) -> bool {
        // We're at '(' - scan forward to find matching ')' then check for '=>'
        let source = self.source;
        // Use current_start (without base_offset) since source is the script slice
        let start = self.current_start;

        // Simple scan: count parens, look for => after closing paren
        let mut depth = 0;
        let mut pos = start;
        let bytes = source.as_bytes();

        while pos < bytes.len() {
            match bytes[pos] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        // Found closing paren, check for =>
                        pos += 1;
                        // Skip whitespace
                        while pos < bytes.len()
                            && (bytes[pos] == b' '
                                || bytes[pos] == b'\t'
                                || bytes[pos] == b'\n'
                                || bytes[pos] == b'\r')
                        {
                            pos += 1;
                        }
                        // Check for =>
                        return pos + 1 < bytes.len()
                            && bytes[pos] == b'='
                            && bytes[pos + 1] == b'>';
                    }
                }
                b'"' | b'\'' => {
                    // Skip string literal
                    let quote = bytes[pos];
                    pos += 1;
                    while pos < bytes.len() && bytes[pos] != quote {
                        if bytes[pos] == b'\\' && pos + 1 < bytes.len() {
                            pos += 1; // skip escaped char
                        }
                        pos += 1;
                    }
                }
                _ => {}
            }
            pos += 1;
        }
        false
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

            // Check for spread: { ...obj }
            if self.check(&TokenKind::DotDotDot) {
                self.advance()?; // consume '...'
                let argument = self.parse_expression()?;
                let prop_end = argument.span().end as usize;
                properties.push(ObjectProperty::SpreadElement(SpreadElement {
                    argument: Box::new(argument),
                    span: Span::new(prop_start as u32, prop_end as u32),
                }));

                // Check for comma or closing brace
                if !self.expect_list_separator(&TokenKind::Comma, &TokenKind::BraceClose)? {
                    break;
                }
                continue;
            }

            // Parse property key
            // Supports: identifiers, string literals
            // TODO: Number literals, computed properties
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
                TokenKind::String => {
                    // String literal key: {"prop-name": value}
                    let (key_start, key_end) = self.current_pos();
                    let raw = self.current_value().to_string();
                    let quote = raw.chars().next().unwrap_or('"');
                    let content = if let Some(decoded) = self.current_decoded() {
                        decoded.to_string()
                    } else if raw.len() >= 2 {
                        raw[1..raw.len() - 1].to_string()
                    } else {
                        String::new()
                    };
                    self.advance()?;
                    Expression::Literal(Literal {
                        value: LiteralValue::String { content, quote },
                        span: Span::new(key_start as u32, key_end as u32),
                    })
                }
                _ => {
                    return Err(ParseError::InvalidSyntax {
                        message: format!("Expected property key, found {}", self.current_kind()),
                        position: prop_start,
                        context: None,
                    });
                }
            };

            // Check for shorthand property: `{ prop }` vs `{ prop: value }`
            let (value, shorthand) = if self.eat(TokenKind::Colon) {
                (self.parse_expression()?, false)
            } else {
                // Shorthand: key is duplicated as value
                (key.clone(), true)
            };

            // Property span should end at value's end, not including trailing comments
            let prop_end = value.span().end as usize;
            properties.push(ObjectProperty::Property(Property {
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
            }));

            // Check for comma or closing brace (with trailing comma support)
            if !self.expect_list_separator(&TokenKind::Comma, &TokenKind::BraceClose)? {
                break;
            }
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?; // consume '}'

        Ok(Expression::ObjectExpression(ObjectExpression {
            properties,
            span: Span::new(start as u32, end as u32),
        }))
    }

    /// Parse array literal: `[elem, ...]`
    ///
    /// Currently supports:
    /// - Elements: numbers, strings, booleans, null, identifiers, objects, arrays
    /// - Trailing commas: `[1, 2, 3,]`
    /// - Empty arrays: `[]`
    ///
    /// TODO: Future enhancements:
    /// - Sparse arrays: `[1,,3]` (missing elements)
    /// - Spread elements: `[...arr]`
    fn parse_array_expression(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();
        self.expect(&TokenKind::BracketOpen)?; // consume '['

        let mut elements = Vec::new();

        // Handle empty array: `[]`
        if self.check(&TokenKind::BracketClose) {
            let (_, end) = self.current_pos();
            self.advance()?; // consume ']'
            return Ok(Expression::ArrayExpression(ArrayExpression {
                elements,
                span: Span::new(start as u32, end as u32),
            }));
        }

        // Parse elements
        loop {
            // Parse element expression
            let elem = self.parse_expression()?;
            elements.push(Some(elem));

            // Check for comma or closing bracket (with trailing comma support)
            if !self.expect_list_separator(&TokenKind::Comma, &TokenKind::BracketClose)? {
                break;
            }
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BracketClose)?; // consume ']'

        Ok(Expression::ArrayExpression(ArrayExpression {
            elements,
            span: Span::new(start as u32, end as u32),
        }))
    }

    /// Parse unary expression: `-x`, `+x`, `!x`
    ///
    /// Unary operators have higher precedence than binary operators.
    /// The binding power (25) is higher than multiplicative (21-22).
    fn parse_unary_expression(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();

        let operator = match self.current_kind() {
            TokenKind::Minus => UnaryOperator::Minus,
            TokenKind::Plus => UnaryOperator::Plus,
            TokenKind::Bang => UnaryOperator::Bang,
            _ => unreachable!("parse_unary_expression called with non-unary operator"),
        };
        self.advance()?;

        // Parse the operand with high binding power (unary is right-associative)
        // This allows chained unary: --x, -+x, !!x, etc.
        // and proper precedence: -a * b parses as (-a) * b
        let parsed = self.parse_expression_bp(25)?;

        // Use actual_end to include trailing parens in the span (matches Svelte's behavior)
        // For `!(a && b)`, the span should be from `!` to `)`, not just to `b`
        let end = parsed.actual_end as u32;

        Ok(Expression::UnaryExpression(UnaryExpression {
            operator,
            argument: Box::new(parsed.expr),
            prefix: true,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse arrow function: `() => expr` or `(x, y) => expr` or `() => { ... }`
    ///
    /// Currently supports:
    /// - No parameters: `() => expr`
    /// - Single parameter: `(x) => expr`
    /// - Multiple parameters: `(x, y) => expr`
    /// - Expression body: `() => expr`
    /// - Block body: `() => { ... }` (body stored as span, not parsed)
    ///
    /// TODO: Support single parameter without parens: `x => expr`
    fn parse_arrow_function(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();
        self.expect(&TokenKind::ParenOpen)?; // consume '('

        // Parse parameter list
        let mut params = Vec::new();

        if !self.check(&TokenKind::ParenClose) {
            loop {
                // Parse parameter (identifier for now, no destructuring)
                let (param_start, param_end) = self.current_pos();
                if self.current_kind() != TokenKind::Identifier {
                    return Err(ParseError::InvalidSyntax {
                        message: format!("Expected parameter name, found {}", self.current_kind()),
                        position: param_start,
                        context: None,
                    });
                }
                let symbol = self.intern(self.current_value());
                self.advance()?;

                params.push(Identifier {
                    name: symbol,
                    type_annotation: None,
                    span: Span::new(param_start as u32, param_end as u32),
                });

                // Check for comma or closing paren
                if !self.expect_list_separator(&TokenKind::Comma, &TokenKind::ParenClose)? {
                    break;
                }
            }
        }

        self.expect(&TokenKind::ParenClose)?; // consume ')'
        self.expect(&TokenKind::Arrow)?; // consume '=>'

        // Parse body: expression or block
        let (body, expression) = if self.check(&TokenKind::BraceOpen) {
            // Block body: `() => { ... }`
            let (block_start, _) = self.current_pos();
            self.advance()?; // consume '{'

            // Skip block contents (simplified: just find matching brace)
            let mut brace_depth = 1;
            while brace_depth > 0 {
                match self.current_kind() {
                    TokenKind::BraceOpen => brace_depth += 1,
                    TokenKind::BraceClose => brace_depth -= 1,
                    TokenKind::Eof => {
                        return Err(ParseError::InvalidSyntax {
                            message: "Unexpected end of file in arrow function body".to_string(),
                            position: self.current_pos().0,
                            context: None,
                        });
                    }
                    _ => {}
                }
                if brace_depth > 0 {
                    self.advance()?;
                }
            }

            let (_, block_end) = self.current_pos();
            self.advance()?; // consume final '}'

            (
                ArrowFunctionBody::BlockStatement {
                    span: Span::new(block_start as u32, block_end as u32),
                },
                false,
            )
        } else {
            // Expression body: `() => expr`
            let expr = self.parse_expression()?;
            (ArrowFunctionBody::Expression(Box::new(expr)), true)
        };

        let end = body.span().end;

        Ok(Expression::ArrowFunctionExpression(
            ArrowFunctionExpression {
                params,
                body,
                expression,
                span: Span::new(start as u32, end),
            },
        ))
    }

    /// Parse spread element: `...expr`
    fn parse_spread_element(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();
        self.expect(&TokenKind::DotDotDot)?; // consume '...'

        let argument = self.parse_expression()?;
        let end = argument.span().end;

        Ok(Expression::SpreadElement(SpreadElement {
            argument: Box::new(argument),
            span: Span::new(start as u32, end),
        }))
    }
}
