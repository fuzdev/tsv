// Expression parsing using Pratt parser for operator precedence

use crate::ast::internal::{
    ArrayExpression, ArrayPattern, ArrowFunctionBody, ArrowFunctionExpression,
    AssignmentExpression, AssignmentOperator, AssignmentPattern, AwaitExpression, BinaryExpression,
    BinaryOperator, BlockStatement, CallExpression, ConditionalExpression, Expression,
    FunctionExpression, Identifier, Literal, LiteralValue, MemberExpression, NewExpression,
    ObjectExpression, ObjectPattern, ObjectPatternProperty, ObjectProperty, Property, PropertyKind,
    RegexLiteral, RestElement, SequenceExpression, SpreadElement, Super, TaggedTemplateExpression,
    TemplateElement, TemplateLiteral, UnaryExpression, UnaryOperator, UpdateExpression,
    UpdateOperator,
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

/// Infix operator info: binding powers and the corresponding binary operator.
///
/// Returns `(left_bp, right_bp, operator)` if the token is a binary operator.
/// - `left_bp < right_bp` for left-associative operators
/// - `left_bp > right_bp` for right-associative operators (e.g., `**`)
///
/// Uses standard JavaScript operator precedence.
fn infix_operator_info(kind: &TokenKind) -> Option<(u8, u8, BinaryOperator)> {
    use BinaryOperator as Op;

    match kind {
        // Nullish coalescing: lowest binary precedence
        TokenKind::QuestionQuestion => Some((5, 6, Op::QuestionQuestion)),
        // Logical OR
        TokenKind::PipePipe => Some((7, 8, Op::PipePipe)),
        // Logical AND
        TokenKind::AmpersandAmpersand => Some((9, 10, Op::AmpersandAmpersand)),
        // Bitwise OR
        TokenKind::Pipe => Some((11, 12, Op::Pipe)),
        // Bitwise XOR
        TokenKind::Caret => Some((13, 14, Op::Caret)),
        // Bitwise AND
        TokenKind::Ampersand => Some((15, 16, Op::Ampersand)),
        // Equality
        TokenKind::EqualsEquals => Some((17, 18, Op::EqualsEquals)),
        TokenKind::BangEquals => Some((17, 18, Op::BangEquals)),
        TokenKind::EqualsEqualsEquals => Some((17, 18, Op::EqualsEqualsEquals)),
        TokenKind::BangEqualsEquals => Some((17, 18, Op::BangEqualsEquals)),
        // Relational (including in, instanceof)
        TokenKind::LessThan => Some((19, 20, Op::LessThan)),
        TokenKind::GreaterThan => Some((19, 20, Op::GreaterThan)),
        TokenKind::LessThanEquals => Some((19, 20, Op::LessThanEquals)),
        TokenKind::GreaterThanEquals => Some((19, 20, Op::GreaterThanEquals)),
        TokenKind::Keyword(KeywordKind::Instanceof) => Some((19, 20, Op::Instanceof)),
        TokenKind::Keyword(KeywordKind::In) => Some((19, 20, Op::In)),
        // Bitshift
        TokenKind::LeftShift => Some((21, 22, Op::LeftShift)),
        TokenKind::RightShift => Some((21, 22, Op::RightShift)),
        TokenKind::UnsignedRightShift => Some((21, 22, Op::UnsignedRightShift)),
        // Additive
        TokenKind::Plus => Some((23, 24, Op::Plus)),
        TokenKind::Minus => Some((23, 24, Op::Minus)),
        // Multiplicative
        TokenKind::Star => Some((25, 26, Op::Star)),
        TokenKind::Slash => Some((25, 26, Op::Slash)),
        TokenKind::Percent => Some((25, 26, Op::Percent)),
        // Exponentiation (right-associative: left_bp > right_bp)
        TokenKind::StarStar => Some((28, 27, Op::StarStar)),
        _ => None,
    }
}

impl<'a> Parser<'a> {
    /// Parse an expression using Pratt parsing for operator precedence
    ///
    /// This is the top-level entry point that handles ALL expression forms including
    /// the comma operator (sequence expression). Use `parse_assignment_expression()`
    /// for contexts where comma is a separator (function args, array elements, etc.)
    pub(super) fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        Ok(self.parse_expression_bp(0)?.expr)
    }

    /// Parse an assignment expression (excludes comma operator)
    ///
    /// Use this for contexts where comma is a separator rather than an operator:
    /// - Function call arguments: `foo(a, b)` - commas separate args
    /// - Array elements: `[a, b, c]` - commas separate elements
    /// - Object property values: `{x: a, y: b}` - commas separate properties
    /// - Variable initializers: `const x = expr` - comma would be ambiguous with declarators
    ///
    /// To use the comma operator in these contexts, wrap in parens: `foo((a, b))`
    pub(super) fn parse_assignment_expression(&mut self) -> Result<Expression, ParseError> {
        // Use min_bp = 1 to skip comma handling (which only triggers at min_bp == 0)
        Ok(self.parse_expression_bp(1)?.expr)
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
            let Some((left_bp, right_bp, operator)) = infix_operator_info(&kind) else {
                break;
            };

            // Check if operator binds tighter than minimum
            if left_bp < min_bp {
                break;
            }

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

        // Handle assignment operator (after binary ops, before ternary)
        // Assignment is right-associative and has low precedence
        if min_bp <= 1 && self.check(&TokenKind::Equals) {
            self.advance()?; // consume '='

            // Parse right-hand side (assignment is right-associative, so same precedence)
            let right = self.parse_expression_bp(1)?;

            // Convert left side to pattern if needed (cover grammar)
            let left_pattern = self.to_assignable(left.expr)?;

            let span = Span::new(expr_start as u32, right.actual_end as u32);
            left = ParsedExpr {
                expr: Expression::AssignmentExpression(AssignmentExpression {
                    left: Box::new(left_pattern),
                    operator: AssignmentOperator::Assign,
                    right: Box::new(right.expr),
                    span,
                }),
                actual_end: right.actual_end,
            };
        }

        // Handle ternary operator (lowest precedence among binary-like ops, above comma)
        // Handle at min_bp <= 1 to include in assignment expressions but not in binary ops
        if min_bp <= 1 && self.check(&TokenKind::Question) {
            self.advance()?; // consume '?'

            // Parse consequent (then branch) - use bp=1 to exclude comma operator
            // This ensures (a ? b : c, d) parses as ((a ? b : c), d) not (a ? b : (c, d))
            let consequent = self.parse_expression_bp(1)?;

            // Expect ':'
            self.expect(&TokenKind::Colon)?;

            // Parse alternate (else branch) - use bp=1 to exclude comma operator
            let alternate = self.parse_expression_bp(1)?;

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

        // Handle comma operator (lowest precedence, after ternary)
        // Only handle at top level (min_bp == 0) to avoid conflicts with comma in
        // function calls, array literals, and object literals
        if min_bp == 0 && self.check(&TokenKind::Comma) {
            let mut expressions = vec![left.expr];
            let mut last_end = left.actual_end;

            while self.eat(TokenKind::Comma) {
                // Parse next expression (ternary level, so min_bp = 0 is fine
                // since we're handling comma at even lower level here)
                let next = self.parse_expression_bp(1)?; // Use bp=1 to stop before next comma
                expressions.push(next.expr);
                last_end = next.actual_end;
            }

            let span = Span::new(expr_start as u32, last_end as u32);
            left = ParsedExpr {
                expr: Expression::SequenceExpression(SequenceExpression { expressions, span }),
                actual_end: last_end,
            };
        }

        Ok(left)
    }

    /// Parse prefix expression returning ParsedExpr with actual end position
    fn parse_prefix_expression_with_end(&mut self) -> Result<ParsedExpr, ParseError> {
        let parsed = match self.current_kind() {
            TokenKind::Minus | TokenKind::Plus | TokenKind::Bang | TokenKind::Tilde => {
                let expr = self.parse_unary_expression()?;
                ParsedExpr::from_expr(expr)
            }
            TokenKind::PlusPlus | TokenKind::MinusMinus => {
                let expr = self.parse_prefix_update_expression()?;
                ParsedExpr::from_expr(expr)
            }
            TokenKind::Keyword(KeywordKind::New) => {
                let expr = self.parse_new_expression()?;
                ParsedExpr::from_expr(expr)
            }
            TokenKind::Keyword(KeywordKind::Typeof | KeywordKind::Void | KeywordKind::Delete) => {
                let expr = self.parse_unary_keyword_expression()?;
                ParsedExpr::from_expr(expr)
            }
            TokenKind::Keyword(KeywordKind::Await) => {
                let expr = self.parse_await_expression()?;
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
                TokenKind::QuestionDot => {
                    // Optional chaining: obj?.prop, obj?.[expr], obj?.()
                    self.advance()?; // consume '?.'

                    match self.current_kind() {
                        TokenKind::Identifier => {
                            // obj?.prop - optional property access
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
                                    optional: true,
                                    span,
                                }),
                                prop_end,
                            );
                        }
                        TokenKind::BracketOpen => {
                            // obj?.[expr] - optional computed access
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
                                    optional: true,
                                    span,
                                }),
                                bracket_end,
                            );
                        }
                        TokenKind::ParenOpen => {
                            // obj?.() - optional call
                            self.advance()?; // consume '('

                            let mut arguments = Vec::new();

                            // Parse arguments (use assignment_expression to avoid comma operator)
                            if !self.check(&TokenKind::ParenClose) {
                                loop {
                                    let arg = self.parse_assignment_expression()?;
                                    arguments.push(arg);

                                    if !self.expect_list_separator(
                                        &TokenKind::Comma,
                                        &TokenKind::ParenClose,
                                    )? {
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
                                    optional: true,
                                    span,
                                }),
                                paren_end,
                            );
                        }
                        _ => {
                            return Err(ParseError::InvalidSyntax {
                                message: format!(
                                    "Expected property name, '[', or '(' after '?.', found {}",
                                    self.current_kind()
                                ),
                                position: self.current_pos().0,
                                context: None,
                            });
                        }
                    }
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

                    // Parse arguments (use assignment_expression to avoid comma operator)
                    if !self.check(&TokenKind::ParenClose) {
                        loop {
                            let arg = self.parse_assignment_expression()?;
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
                TokenKind::NoSubstitutionTemplate | TokenKind::TemplateHead => {
                    // Tagged template expression: tag`content`
                    let quasi = self.parse_template_literal()?;
                    let quasi_span = quasi.span();
                    if let Expression::TemplateLiteral(template) = quasi {
                        let span = Span::new(left.expr.span().start, quasi_span.end);
                        left = ParsedExpr::with_end(
                            Expression::TaggedTemplateExpression(TaggedTemplateExpression {
                                tag: Box::new(left.expr),
                                quasi: template,
                                span,
                            }),
                            quasi_span.end as usize,
                        );
                    }
                }
                kind @ TokenKind::PlusPlus | kind @ TokenKind::MinusMinus
                    if !self.had_line_terminator =>
                {
                    // Postfix update expression: x++, x--
                    // ASI Rule: If there's a line terminator before ++/--, ASI fires
                    // and the ++/-- becomes a prefix operator on the next statement.
                    // So we only parse postfix if NO line terminator preceded this token.
                    let operator = if kind == TokenKind::PlusPlus {
                        UpdateOperator::Increment
                    } else {
                        UpdateOperator::Decrement
                    };

                    let (_, op_end) = self.current_pos();
                    self.advance()?;

                    let span = Span::new(left.expr.span().start, op_end as u32);
                    left = ParsedExpr::with_end(
                        Expression::UpdateExpression(UpdateExpression {
                            operator,
                            argument: Box::new(left.expr),
                            prefix: false,
                            span,
                        }),
                        op_end,
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
                    message: format!("Invalid number: {raw}"),
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
            TokenKind::Keyword(KeywordKind::Undefined) => {
                let (start, end) = self.current_pos();
                self.advance()?;
                Ok(ParsedExpr::with_end(
                    Expression::Literal(Literal {
                        value: LiteralValue::Undefined,
                        span: Span::new(start as u32, end as u32),
                    }),
                    end,
                ))
            }
            TokenKind::Keyword(KeywordKind::Super) => {
                let (start, end) = self.current_pos();
                self.advance()?;
                Ok(ParsedExpr::with_end(
                    Expression::Super(Super {
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
            TokenKind::NoSubstitutionTemplate | TokenKind::TemplateHead => {
                Ok(ParsedExpr::from_expr(self.parse_template_literal()?))
            }
            TokenKind::Slash => {
                // In expression context, `/` is a regex literal, not division
                // Division is only possible after a value (identifier, number, closing bracket)
                // but we're in parse_primary_expression which is called when expecting a value
                //
                // Use current_start directly (not current_pos) because the lexer expects positions
                // relative to its source slice, not the full document offset.
                let lexer_start = self.current_start;
                let regex_token = self.lexer.read_regex_literal(lexer_start)?;
                let lexer_end = regex_token.end;

                // Calculate span with base_offset for the AST
                let span_start = lexer_start + self.base_offset;
                let span_end = lexer_end + self.base_offset;

                // Extract pattern and flags from the decoded value (format: "pattern\0flags")
                let decoded = regex_token.decoded.as_deref().unwrap_or("\0");
                let (pattern, flags) = decoded.split_once('\0').unwrap_or((decoded, ""));

                // Advance past the regex token by reading the next token
                // We need to update parser state manually since lexer was resynced by read_regex_literal
                let next_token = self.lexer.next_token()?;
                self.current_kind = next_token.kind;
                self.current_start = next_token.start;
                self.current_end = next_token.end;
                self.current_decoded = next_token.decoded;
                // Note: had_line_terminator not updated here - regex doesn't affect ASI context

                Ok(ParsedExpr::with_end(
                    Expression::RegexLiteral(RegexLiteral {
                        pattern: pattern.to_string(),
                        flags: flags.to_string(),
                        span: Span::new(span_start as u32, span_end as u32),
                    }),
                    span_end,
                ))
            }
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
    pub(super) fn parse_object_expression(&mut self) -> Result<Expression, ParseError> {
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
                // Use assignment_expression because comma separates properties
                let argument = self.parse_assignment_expression()?;
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

            // Check for getter/setter: `get x() {}` or `set x(v) {}`
            // These are contextual keywords - only treated as get/set when followed by a property name
            let accessor_kind = if self.current_kind() == TokenKind::Identifier {
                let is_get = self.current_value() == "get";
                let is_set = self.current_value() == "set";
                if (is_get || is_set) && self.peek_is_property_name() {
                    let kind = if is_get {
                        PropertyKind::Get
                    } else {
                        PropertyKind::Set
                    };
                    self.advance()?; // consume 'get' or 'set'
                    Some(kind)
                } else {
                    None
                }
            } else {
                None
            };

            // Parse property key
            // Supports: identifiers, keywords (as identifiers), string literals, number literals, computed keys
            let (key, computed) = match self.current_kind() {
                // Computed property: { [expr]: value }
                TokenKind::BracketOpen => {
                    self.advance()?; // consume '['
                    let key_expr = self.parse_expression()?;
                    self.expect(&TokenKind::BracketClose)?; // consume ']'
                    (key_expr, true)
                }
                // Both identifiers and keywords can be property keys: { foo: 1, object: 2, in: 3 }
                TokenKind::Identifier | TokenKind::Keyword(_) => {
                    let (key_start, key_end) = self.current_pos();
                    let symbol = self.intern(self.current_value());
                    self.advance()?;
                    (
                        Expression::Identifier(Identifier {
                            name: symbol,
                            type_annotation: None,
                            span: Span::new(key_start as u32, key_end as u32),
                        }),
                        false,
                    )
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
                    (
                        Expression::Literal(Literal {
                            value: LiteralValue::String { content, quote },
                            span: Span::new(key_start as u32, key_end as u32),
                        }),
                        false,
                    )
                }
                TokenKind::Number => {
                    // Number literal key: {0: value, 1: value}
                    let (key_start, key_end) = self.current_pos();
                    let value = self.current_value().parse::<f64>().unwrap_or(f64::NAN);
                    self.advance()?;
                    (
                        Expression::Literal(Literal {
                            value: LiteralValue::Number(value),
                            span: Span::new(key_start as u32, key_end as u32),
                        }),
                        false,
                    )
                }
                _ => {
                    return Err(ParseError::InvalidSyntax {
                        message: format!("Expected property key, found {}", self.current_kind()),
                        position: prop_start,
                        context: None,
                    });
                }
            };

            // Determine property kind, value, shorthand, and method flags
            let (kind, value, shorthand, method) = if let Some(accessor) = accessor_kind {
                // Getter/setter: `get x() {}` or `set x(v) {}`
                let func_expr = self.parse_method_body(prop_start as u32)?;
                (
                    accessor,
                    Expression::FunctionExpression(func_expr),
                    false,
                    false,
                )
            } else if self.check(&TokenKind::ParenOpen) {
                // Method shorthand: `{ foo() { return 1; } }`
                let func_expr = self.parse_method_body(key.span().start)?;
                (
                    PropertyKind::Init,
                    Expression::FunctionExpression(func_expr),
                    false,
                    true,
                )
            } else if self.eat(TokenKind::Colon) {
                // Use assignment_expression because comma separates properties
                (
                    PropertyKind::Init,
                    self.parse_assignment_expression()?,
                    false,
                    false,
                )
            } else if self.check(&TokenKind::Equals) && !computed {
                // Shorthand with default value: `{a = 1}` (only for simple identifiers)
                // This parses as an AssignmentExpression, which gets converted to
                // AssignmentPattern by to_assignable() when used in destructuring context
                self.advance()?; // consume '='
                let default_value = self.parse_assignment_expression()?;
                let assign_end = default_value.span().end;
                (
                    PropertyKind::Init,
                    Expression::AssignmentExpression(AssignmentExpression {
                        left: Box::new(key.clone()),
                        operator: AssignmentOperator::Assign,
                        right: Box::new(default_value),
                        span: Span::new(key.span().start, assign_end),
                    }),
                    true,
                    false,
                )
            } else {
                // Shorthand: key is duplicated as value
                (PropertyKind::Init, key.clone(), true, false)
            };

            // Property span should end at value's end, not including trailing comments
            let prop_end = value.span().end as usize;
            properties.push(ObjectProperty::Property(Property {
                key,
                value,
                kind,
                shorthand,
                computed,
                method,
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
    /// - Elision (holes): `[, a]`, `[a, , b]`, `[, , a]`
    pub(super) fn parse_array_expression(&mut self) -> Result<Expression, ParseError> {
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

        // Parse elements (including elision/holes)
        loop {
            // Check for elision (hole): leading comma means empty slot
            if self.check(&TokenKind::Comma) {
                elements.push(None); // hole
                self.advance()?; // consume ','
                // Check if we hit the closing bracket (trailing comma after hole)
                if self.check(&TokenKind::BracketClose) {
                    break;
                }
                continue;
            }

            // Check for closing bracket (end of array)
            if self.check(&TokenKind::BracketClose) {
                break;
            }

            // Parse element expression (use assignment_expression because comma separates elements)
            let elem = self.parse_assignment_expression()?;
            elements.push(Some(elem));

            // Check for comma or closing bracket
            if self.check(&TokenKind::Comma) {
                self.advance()?; // consume ','
                // Check for trailing comma
                if self.check(&TokenKind::BracketClose) {
                    break;
                }
            } else if self.check(&TokenKind::BracketClose) {
                break;
            } else {
                return Err(ParseError::InvalidExpression {
                    found: format!("'{}'", self.current_kind()),
                    position: self.current_pos().0,
                    context: None,
                });
            }
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BracketClose)?; // consume ']'

        Ok(Expression::ArrayExpression(ArrayExpression {
            elements,
            span: Span::new(start as u32, end as u32),
        }))
    }

    /// Parse unary expression: `-x`, `+x`, `!x`, `~x`
    ///
    /// Unary operators have higher precedence than all binary operators.
    /// The binding power (29) is higher than exponentiation (27-28).
    fn parse_unary_expression(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();

        let operator = match self.current_kind() {
            TokenKind::Minus => UnaryOperator::Minus,
            TokenKind::Plus => UnaryOperator::Plus,
            TokenKind::Bang => UnaryOperator::Bang,
            TokenKind::Tilde => UnaryOperator::Tilde,
            _ => unreachable!("parse_unary_expression called with non-unary operator"),
        };
        self.advance()?;

        // Parse the operand with high binding power (unary is right-associative)
        // This allows chained unary: --x, -+x, !!x, ~~x etc.
        // and proper precedence: -a * b parses as (-a) * b
        let parsed = self.parse_expression_bp(29)?;

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

    /// Parse unary keyword expression: `typeof x`, `void 0`, `delete obj.x`
    ///
    /// These keyword operators have the same precedence as other unary operators.
    fn parse_unary_keyword_expression(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();

        let operator = match self.current_kind() {
            TokenKind::Keyword(KeywordKind::Typeof) => UnaryOperator::Typeof,
            TokenKind::Keyword(KeywordKind::Void) => UnaryOperator::Void,
            TokenKind::Keyword(KeywordKind::Delete) => UnaryOperator::Delete,
            _ => unreachable!("parse_unary_keyword_expression called with non-keyword operator"),
        };
        self.advance()?;

        // Parse the operand with high binding power (unary is right-associative)
        let parsed = self.parse_expression_bp(29)?;
        let end = parsed.actual_end as u32;

        Ok(Expression::UnaryExpression(UnaryExpression {
            operator,
            argument: Box::new(parsed.expr),
            prefix: true,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse await expression: `await promise`
    ///
    /// Await expressions have high precedence like unary operators.
    fn parse_await_expression(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();
        self.advance()?; // consume 'await'

        // Parse the operand with high binding power (same as unary)
        let parsed = self.parse_expression_bp(29)?;
        let end = parsed.actual_end as u32;

        Ok(Expression::AwaitExpression(AwaitExpression {
            argument: Box::new(parsed.expr),
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse prefix update expression: `++x`, `--x`
    ///
    /// Prefix increment/decrement has the same precedence as unary operators.
    fn parse_prefix_update_expression(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();

        let operator = match self.current_kind() {
            TokenKind::PlusPlus => UpdateOperator::Increment,
            TokenKind::MinusMinus => UpdateOperator::Decrement,
            _ => unreachable!("parse_prefix_update_expression called with non-update operator"),
        };
        self.advance()?;

        // Parse the operand - update expressions apply to member expressions or identifiers
        // Use high binding power so ++x.y parses correctly
        let parsed = self.parse_expression_bp(29)?;

        let end = parsed.actual_end as u32;

        Ok(Expression::UpdateExpression(UpdateExpression {
            operator,
            argument: Box::new(parsed.expr),
            prefix: true,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse new expression: `new Date()`, `new Map()`, `new Foo.Bar()`
    ///
    /// The `new` keyword has the same precedence as unary operators.
    /// It takes a callee (identifier or member expression) and optional arguments.
    fn parse_new_expression(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();
        self.advance()?; // consume 'new'

        // Parse the callee - this could be an identifier, member expression, or even nested `new`
        // We use primary + postfix parsing but stop before call expressions
        let callee_parsed = self.parse_primary_expression_with_end()?;

        // Parse member access chains: new Foo.Bar.Baz()
        let mut callee = callee_parsed;
        loop {
            match self.current_kind() {
                TokenKind::Dot => {
                    self.advance()?; // consume '.'
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

                    let span = Span::new(callee.expr.span().start, prop_end as u32);
                    callee = ParsedExpr::with_end(
                        Expression::MemberExpression(MemberExpression {
                            object: Box::new(callee.expr),
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
                    self.advance()?; // consume '['
                    let index = self.parse_expression()?;
                    let (_, bracket_end) = self.current_pos();
                    self.expect(&TokenKind::BracketClose)?;

                    let span = Span::new(callee.expr.span().start, bracket_end as u32);
                    callee = ParsedExpr::with_end(
                        Expression::MemberExpression(MemberExpression {
                            object: Box::new(callee.expr),
                            property: Box::new(index),
                            computed: true,
                            optional: false,
                            span,
                        }),
                        bracket_end,
                    );
                }
                _ => break,
            }
        }

        // Parse optional arguments: new Date() vs new Date
        let (arguments, end) = if self.check(&TokenKind::ParenOpen) {
            self.advance()?; // consume '('
            let mut args = Vec::new();

            if !self.check(&TokenKind::ParenClose) {
                loop {
                    // Use assignment_expression because comma separates arguments
                    let arg = self.parse_assignment_expression()?;
                    args.push(arg);

                    if !self.expect_list_separator(&TokenKind::Comma, &TokenKind::ParenClose)? {
                        break;
                    }
                }
            }

            let (_, paren_end) = self.current_pos();
            self.expect(&TokenKind::ParenClose)?;
            (args, paren_end as u32)
        } else {
            // new Date without parens - valid JavaScript
            (Vec::new(), callee.actual_end as u32)
        };

        Ok(Expression::NewExpression(NewExpression {
            callee: Box::new(callee.expr),
            arguments,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse arrow function: `() => expr` or `(x, y) => expr` or `() => { ... }`
    ///
    /// Currently supports:
    /// - No parameters: `() => expr`
    /// - Single parameter: `(x) => expr`
    /// - Multiple parameters: `(x, y) => expr`
    /// - Destructuring parameters: `([a, b]) => ...`, `({x, y}) => ...`
    /// - Default values: `(a = 1) => ...`
    /// - Expression body: `() => expr`
    /// - Block body: `() => { ... }` (body stored as span, not parsed)
    ///
    /// TODO: Support single parameter without parens: `x => expr`
    fn parse_arrow_function(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();

        // Parse parameter list (reuse shared method)
        let params = self.parse_parameter_list()?;

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
            // Use assignment_expression so comma doesn't consume next object property
            let expr = self.parse_assignment_expression()?;
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

    /// Parse method body for method shorthand: `foo() { return 1; }`
    ///
    /// This parses the parameter list and block body for a method definition.
    /// The key has already been parsed by the caller.
    fn parse_method_body(&mut self, start: u32) -> Result<FunctionExpression, ParseError> {
        let params = self.parse_parameter_list()?;
        let body = self.parse_block_statement()?;
        let end = body.span.end;

        Ok(FunctionExpression {
            id: None, // Method shorthand has no function name
            params,
            body,
            span: Span::new(start, end),
        })
    }

    /// Parse a block statement: `{ stmt1; stmt2; }`
    ///
    /// Parses the statements inside a block body (used for function bodies).
    pub(super) fn parse_block_statement(&mut self) -> Result<BlockStatement, ParseError> {
        let (start, _) = self.current_pos();
        self.expect(&TokenKind::BraceOpen)?; // consume '{'

        let mut body = Vec::new();

        // Parse statements until we hit '}'
        while !self.check(&TokenKind::BraceClose) {
            if self.check(&TokenKind::Eof) {
                return Err(ParseError::InvalidSyntax {
                    message: "Unexpected end of file in block".to_string(),
                    position: self.current_pos().0,
                    context: None,
                });
            }

            let stmt = self.parse_statement()?;
            body.push(stmt);
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?; // consume '}'

        Ok(BlockStatement {
            body,
            span: Span::new(start as u32, end as u32),
        })
    }

    /// Parse spread element: `...expr`
    fn parse_spread_element(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();
        self.expect(&TokenKind::DotDotDot)?; // consume '...'

        // Use assignment_expression because comma separates array elements/object properties
        let argument = self.parse_assignment_expression()?;
        let end = argument.span().end;

        Ok(Expression::SpreadElement(SpreadElement {
            argument: Box::new(argument),
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse template literal: `hello ${name}`
    ///
    /// Handles both simple templates (no interpolation) and templates with expressions.
    /// See also `parse_template_literal_type()` in statement.rs for type context version.
    pub(super) fn parse_template_literal(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();
        let mut quasis = Vec::new();
        let mut expressions = Vec::new();

        match self.current_kind() {
            TokenKind::NoSubstitutionTemplate => {
                // Simple template with no interpolation: `hello world`
                let (elem_start, elem_end) = self.current_pos();
                let raw = self.current_value().to_string();

                // Extract content between backticks
                let content = if raw.len() >= 2 {
                    raw[1..raw.len() - 1].to_string()
                } else {
                    String::new()
                };

                // Decode escapes for cooked value
                let cooked = if let Some(decoded) = self.current_decoded() {
                    Some(decoded.to_string())
                } else {
                    Some(content.clone())
                };

                self.advance()?;

                quasis.push(TemplateElement {
                    raw: content,
                    cooked,
                    tail: true,
                    span: Span::new(elem_start as u32, elem_end as u32),
                });

                Ok(Expression::TemplateLiteral(TemplateLiteral {
                    quasis,
                    expressions,
                    span: Span::new(start as u32, elem_end as u32),
                }))
            }
            TokenKind::TemplateHead => {
                // Template with interpolation: `hello ${name}...`
                let (elem_start, elem_end) = self.current_pos();
                let raw = self.current_value().to_string();

                // Extract content: remove leading ` and trailing ${
                let content = if raw.len() >= 3 {
                    raw[1..raw.len() - 2].to_string()
                } else {
                    String::new()
                };

                let cooked = if let Some(decoded) = self.current_decoded() {
                    Some(decoded.to_string())
                } else {
                    Some(content.clone())
                };

                self.advance()?;

                quasis.push(TemplateElement {
                    raw: content,
                    cooked,
                    tail: false,
                    span: Span::new(elem_start as u32, elem_end as u32),
                });

                // Parse expressions and remaining template parts
                loop {
                    // Parse the interpolated expression
                    let expr = self.parse_expression()?;
                    expressions.push(expr);

                    // Expect closing } of the interpolation
                    let (brace_start, _) = self.current_pos();
                    if !self.check(&TokenKind::BraceClose) {
                        return Err(ParseError::InvalidSyntax {
                            message: format!(
                                "Expected '}}' at end of template interpolation, found {}",
                                self.current_kind()
                            ),
                            position: brace_start,
                            context: None,
                        });
                    }

                    // Get the raw end position (without base_offset) for the lexer
                    let raw_brace_end = self.current_raw_end();

                    // Skip the } in the lexer without getting next token normally
                    // (calling advance() would try to lex ` as a new token)
                    // Instead, tell the lexer to skip past the } and read template content
                    let next_token = self.lexer.continue_template_from_brace(raw_brace_end)?;
                    self.update_current(next_token);

                    let (elem_start, elem_end) = self.current_pos();
                    let raw = self.current_value().to_string();

                    match self.current_kind().clone() {
                        TokenKind::TemplateMiddle => {
                            // More interpolations to come: }content${
                            let content = if raw.len() >= 3 {
                                raw[1..raw.len() - 2].to_string()
                            } else {
                                String::new()
                            };

                            let cooked = if let Some(decoded) = self.current_decoded() {
                                Some(decoded.to_string())
                            } else {
                                Some(content.clone())
                            };

                            self.advance()?;

                            quasis.push(TemplateElement {
                                raw: content,
                                cooked,
                                tail: false,
                                span: Span::new(elem_start as u32, elem_end as u32),
                            });
                        }
                        TokenKind::TemplateTail => {
                            // End of template: }content`
                            let content = if raw.len() >= 2 {
                                raw[1..raw.len() - 1].to_string()
                            } else {
                                String::new()
                            };

                            let cooked = if let Some(decoded) = self.current_decoded() {
                                Some(decoded.to_string())
                            } else {
                                Some(content.clone())
                            };

                            self.advance()?;

                            quasis.push(TemplateElement {
                                raw: content,
                                cooked,
                                tail: true,
                                span: Span::new(elem_start as u32, elem_end as u32),
                            });

                            break;
                        }
                        _ => {
                            return Err(ParseError::InvalidSyntax {
                                message: format!(
                                    "Expected template middle or tail, found {}",
                                    self.current_kind()
                                ),
                                position: elem_start,
                                context: None,
                            });
                        }
                    }
                }

                let end = quasis.last().map_or(start as u32, |q| q.span.end);

                Ok(Expression::TemplateLiteral(TemplateLiteral {
                    quasis,
                    expressions,
                    span: Span::new(start as u32, end),
                }))
            }
            _ => Err(ParseError::InvalidSyntax {
                message: format!("Expected template literal, found {}", self.current_kind()),
                position: start,
                context: None,
            }),
        }
    }

    // =========================================================================
    // Pattern Conversion (Cover Grammar)
    // =========================================================================

    /// Convert an expression to an assignable pattern (cover grammar)
    ///
    /// This implements the ECMAScript "cover grammar" for assignment targets.
    /// When we parse `{a, b} = obj`, we first parse `{a, b}` as an ObjectExpression,
    /// then convert it to an ObjectPattern when we see the `=`.
    ///
    /// Conversions:
    /// - ObjectExpression → ObjectPattern
    /// - ArrayExpression → ArrayPattern
    /// - SpreadElement → RestElement
    /// - BinaryExpression with = (shorthand default) → AssignmentPattern
    /// - Identifier, MemberExpression → unchanged (valid assignment targets)
    pub(super) fn to_assignable(&self, expr: Expression) -> Result<Expression, ParseError> {
        match expr {
            // Identifier is already a valid assignment target
            Expression::Identifier(_) => Ok(expr),

            // Member expression is a valid assignment target
            Expression::MemberExpression(_) => Ok(expr),

            // Convert ObjectExpression to ObjectPattern
            Expression::ObjectExpression(obj) => {
                let properties = obj
                    .properties
                    .into_iter()
                    .map(|prop| self.object_property_to_pattern(prop))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Expression::ObjectPattern(ObjectPattern {
                    properties,
                    span: obj.span,
                }))
            }

            // Convert ArrayExpression to ArrayPattern
            Expression::ArrayExpression(arr) => {
                let elements = arr
                    .elements
                    .into_iter()
                    .map(|elem| elem.map(|e| self.to_assignable(e)).transpose())
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Expression::ArrayPattern(ArrayPattern {
                    elements,
                    span: arr.span,
                }))
            }

            // Convert SpreadElement to RestElement
            Expression::SpreadElement(spread) => {
                let argument = self.to_assignable(*spread.argument)?;
                Ok(Expression::RestElement(RestElement {
                    argument: Box::new(argument),
                    span: spread.span,
                }))
            }

            // AssignmentExpression in pattern context becomes AssignmentPattern
            // This handles default values like `{a = 1}` which was parsed as shorthand
            Expression::AssignmentExpression(assign) => {
                let left = self.to_assignable(*assign.left)?;
                Ok(Expression::AssignmentPattern(AssignmentPattern {
                    left: Box::new(left),
                    right: assign.right,
                    span: assign.span,
                }))
            }

            // Already a pattern (can happen with nested patterns)
            Expression::ObjectPattern(_)
            | Expression::ArrayPattern(_)
            | Expression::AssignmentPattern(_)
            | Expression::RestElement(_) => Ok(expr),

            // Invalid assignment target
            _ => Err(ParseError::InvalidSyntax {
                message: "Invalid assignment target".to_string(),
                position: expr.span().start as usize,
                context: None,
            }),
        }
    }

    /// Convert an object property to a pattern property
    fn object_property_to_pattern(
        &self,
        prop: ObjectProperty,
    ) -> Result<ObjectPatternProperty, ParseError> {
        match prop {
            ObjectProperty::Property(p) => {
                // Convert the value to a pattern
                let value = self.to_assignable(p.value)?;

                Ok(ObjectPatternProperty::Property(Property {
                    key: p.key,
                    value,
                    method: p.method,
                    shorthand: p.shorthand,
                    computed: p.computed,
                    kind: p.kind,
                    span: p.span,
                }))
            }
            ObjectProperty::SpreadElement(spread) => {
                // Convert spread to rest element
                let argument = self.to_assignable(*spread.argument)?;
                Ok(ObjectPatternProperty::RestElement(RestElement {
                    argument: Box::new(argument),
                    span: spread.span,
                }))
            }
        }
    }
}
