// Expression parsing using Pratt parser for operator precedence

use crate::ast::internal::{
    AssignmentExpression, AwaitExpression, BinaryExpression, BinaryOperator, BlockStatement,
    CallExpression, ConditionalExpression, Expression, Identifier, ImportExpression, JsdocCast,
    Literal, LiteralValue, MemberExpression, MetaProperty, NewExpression, RegexLiteral,
    SequenceExpression, SpreadElement, Statement, Super, TSAsExpression, TSInstantiationExpression,
    TSNonNullExpression, TSSatisfiesExpression, TSTypeAssertion, TaggedTemplateExpression,
    ThisExpression, UnaryExpression, UnaryOperator, UpdateExpression, UpdateOperator,
    YieldExpression,
};
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::printing::visual_width;
use tsv_lang::{ParseError, Span, TAB_WIDTH};

use super::Parser;
use super::expression_lookahead::scan_parens_then_arrow;
use super::scan::parse_number_literal;

//
// Binding Power Constants for Pratt Parser
//
// Higher values = tighter binding (evaluated first)
// Left-associative: left_bp < right_bp
// Right-associative: left_bp > right_bp

/// Comma operator (sequence expression) - lowest precedence
const BP_COMMA: u8 = 0;
/// Assignment operators (=, +=, etc.) and ternary conditional
const BP_ASSIGNMENT: u8 = 1;
/// TypeScript `as` and `satisfies` operators
const BP_TS_TYPE_ASSERTION: u8 = 2;
/// Yield expression (same as assignment — yield takes AssignmentExpression per spec)
const BP_YIELD: u8 = 1;
/// Unary operators (!, -, +, ~, typeof, void, delete, await, ++, --, new)
const BP_UNARY: u8 = 29;

/// Parsed expression with actual end position tracking
///
/// Used during parsing to track where expressions truly start and end after consuming
/// parentheses. This allows binary/call expressions to correctly include opening/closing
/// parens in their spans while keeping inner expression spans semantic.
///
/// Example: `(a && b) || c`
/// - Inner `&&` expression: span = `a && b` (semantic, excludes parens)
/// - Outer `||` expression: span starts at `(`, uses `actual_end` from left operand
///
/// Example: `(a ? b : c)()`
/// - Inner ternary: span = `a ? b : c` (semantic, excludes parens)
/// - CallExpression: span starts at `actual_start` (the `(`), not ternary's start
#[derive(Debug)]
struct ParsedExpr<'arena> {
    /// The parsed expression with semantic span (may exclude surrounding parens)
    expr: Expression<'arena>,
    /// Actual start position before any opening parentheses
    actual_start: usize,
    /// Actual end position after consuming any closing parentheses
    actual_end: usize,
}

impl<'arena> ParsedExpr<'arena> {
    /// Create a ParsedExpr where actual_start/end match the expression's semantic span
    fn from_expr(expr: Expression<'arena>) -> Self {
        let span = expr.span();
        Self {
            actual_start: span.start_usize(),
            actual_end: span.end_usize(),
            expr,
        }
    }

    /// Create a ParsedExpr with explicit actual_end (for parenthesized expressions)
    fn with_end(expr: Expression<'arena>, actual_end: usize) -> Self {
        Self {
            actual_start: expr.span().start_usize(),
            actual_end,
            expr,
        }
    }

    /// Create a ParsedExpr with explicit actual_start and actual_end (for parenthesized expressions)
    fn with_bounds(expr: Expression<'arena>, actual_start: usize, actual_end: usize) -> Self {
        Self {
            expr,
            actual_start,
            actual_end,
        }
    }
}

/// How `parse_postfix_expression` should treat the subscript chain.
///
/// `ClassHeritage` is the `extends <expr>` clause — a `LeftHandSideExpression` that
/// deviates from a normal subscript chain in two ways: a bare `<T>` (type args NOT
/// followed by a call) stops the chain so the class can split it into
/// `super_type_parameters`, and a postfix `++`/`--` stops too (acorn rejects
/// `class C extends a++ {}`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum SubscriptMode {
    Normal,
    ClassHeritage,
}

/// Mirror of prettier's `isTypeCastComment` (`is-type-cast-comment.js`): the
/// comment text (between `/*` and `*/`) starts with `*` — i.e. the `/**` form —
/// and contains `@type` or `@satisfies` followed by a word boundary.
fn is_jsdoc_type_cast_comment(value: &str) -> bool {
    value.starts_with('*') && contains_type_or_satisfies_tag(value)
}

/// Whether `value` contains `@type` or `@satisfies` with a trailing word
/// boundary (so `@types` / `@typedef` don't match), matching prettier's
/// `/@(?:type|satisfies)\b/`. The boundary is **ASCII** (`[A-Za-z0-9_]`) to
/// mirror that regex exactly — JS `\b` has no `u` flag here, so `@typeñ` ends
/// the tag at the non-ASCII `ñ` and counts as a cast.
fn contains_type_or_satisfies_tag(value: &str) -> bool {
    for tag in ["@type", "@satisfies"] {
        let mut from = 0;
        while let Some(rel) = value[from..].find(tag) {
            let abs = from + rel;
            let after = abs + tag.len();
            let boundary = value[after..]
                .chars()
                .next()
                .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'));
            if boundary {
                return true;
            }
            from = abs + 1;
        }
    }
    false
}

/// Infix operator info: binding powers and the corresponding binary operator.
///
/// Returns `(left_bp, right_bp, operator)` if the token is a binary operator.
/// - `left_bp < right_bp` for left-associative operators
/// - `left_bp > right_bp` for right-associative operators (e.g., `**`)
///
/// Uses standard JS operator precedence.
fn infix_operator_info(kind: &TokenKind) -> Option<(u8, u8, BinaryOperator)> {
    match kind {
        // Nullish coalescing: lowest binary precedence
        TokenKind::QuestionQuestion => Some((5, 6, BinaryOperator::QuestionQuestion)),
        // Logical OR
        TokenKind::PipePipe => Some((7, 8, BinaryOperator::PipePipe)),
        // Logical AND
        TokenKind::AmpersandAmpersand => Some((9, 10, BinaryOperator::AmpersandAmpersand)),
        // Bitwise OR
        TokenKind::Pipe => Some((11, 12, BinaryOperator::Pipe)),
        // Bitwise XOR
        TokenKind::Caret => Some((13, 14, BinaryOperator::Caret)),
        // Bitwise AND
        TokenKind::Ampersand => Some((15, 16, BinaryOperator::Ampersand)),
        // Equality
        TokenKind::EqualsEquals => Some((17, 18, BinaryOperator::EqualsEquals)),
        TokenKind::BangEquals => Some((17, 18, BinaryOperator::BangEquals)),
        TokenKind::EqualsEqualsEquals => Some((17, 18, BinaryOperator::EqualsEqualsEquals)),
        TokenKind::BangEqualsEquals => Some((17, 18, BinaryOperator::BangEqualsEquals)),
        // Relational (including in, instanceof)
        TokenKind::LessThan => Some((19, 20, BinaryOperator::LessThan)),
        TokenKind::GreaterThan => Some((19, 20, BinaryOperator::GreaterThan)),
        TokenKind::LessThanEquals => Some((19, 20, BinaryOperator::LessThanEquals)),
        TokenKind::GreaterThanEquals => Some((19, 20, BinaryOperator::GreaterThanEquals)),
        TokenKind::Keyword(KeywordKind::Instanceof) => Some((19, 20, BinaryOperator::Instanceof)),
        TokenKind::Keyword(KeywordKind::In) => Some((19, 20, BinaryOperator::In)),
        // Bitshift
        TokenKind::LeftShift => Some((21, 22, BinaryOperator::LeftShift)),
        TokenKind::RightShift => Some((21, 22, BinaryOperator::RightShift)),
        TokenKind::UnsignedRightShift => Some((21, 22, BinaryOperator::UnsignedRightShift)),
        // Additive
        TokenKind::Plus => Some((23, 24, BinaryOperator::Plus)),
        TokenKind::Minus => Some((23, 24, BinaryOperator::Minus)),
        // Multiplicative
        TokenKind::Star => Some((25, 26, BinaryOperator::Star)),
        TokenKind::Slash => Some((25, 26, BinaryOperator::Slash)),
        TokenKind::Percent => Some((25, 26, BinaryOperator::Percent)),
        // Exponentiation (right-associative: left_bp > right_bp)
        TokenKind::StarStar => Some((28, 27, BinaryOperator::StarStar)),
        _ => None,
    }
}

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Parse an expression using Pratt parsing for operator precedence
    ///
    /// This is the top-level entry point that handles ALL expression forms including
    /// the comma operator (sequence expression). Use `parse_assignment_expression()`
    /// for contexts where comma is a separator (function args, array elements, etc.)
    pub(super) fn parse_expression(&mut self) -> Result<Expression<'arena>, ParseError> {
        Ok(self.parse_expression_bp(BP_COMMA)?.expr)
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
    pub(super) fn parse_assignment_expression(&mut self) -> Result<Expression<'arena>, ParseError> {
        // Use BP_ASSIGNMENT to skip comma handling (which only triggers at BP_COMMA)
        Ok(self.parse_expression_bp(BP_ASSIGNMENT)?.expr)
    }

    /// Parse an expression without allowing `in` as a binary operator.
    ///
    /// Used in for-loop headers to distinguish `for (x in y)` from expressions.
    /// The `in` keyword is recognized as the for-in separator, not as a binary operator.
    pub(super) fn parse_expression_no_in(&mut self) -> Result<Expression<'arena>, ParseError> {
        let old_allow_in = self.allow_in;
        self.allow_in = false;
        let result = self.parse_expression();
        self.allow_in = old_allow_in;
        result
    }

    /// Run `f` with the `[In]` grammar parameter forced to `[+In]` (`allow_in =
    /// true`), restoring the prior value afterward (even on error). Used at the
    /// grammar productions that reset `[+In]` within a for-header init — the
    /// ternary consequent, function/class bodies, param defaults — where a bare
    /// `in` is the binary operator, not the for-in separator. A no-op outside a
    /// for-header init (where `allow_in` is already `true`); a nested for-header
    /// re-disables it via `parse_expression_no_in`'s own save/restore.
    pub(super) fn with_allow_in<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        let old_allow_in = self.allow_in;
        self.allow_in = true;
        let result = f(self);
        self.allow_in = old_allow_in;
        result
    }

    /// Pratt parser: parse expression with minimum binding power
    ///
    /// Returns ParsedExpr with actual end position tracking for parentheses
    fn parse_expression_bp(&mut self, min_bp: u8) -> Result<ParsedExpr<'arena>, ParseError> {
        let arena = self.arena;
        // Track the true start position (before any parentheses)
        // This is needed because grouped expressions like (a && b) should have their
        // containing binary expression span include the opening paren
        let expr_start = self.current_pos().0;

        // Parse prefix expression (primary or unary)
        let mut left = self.parse_prefix_expression_with_end()?;

        // Parse infix operators and TypeScript type assertions (as/satisfies)
        // Binary operators have higher precedence than as/satisfies, so we check binary first.
        // After parsing as/satisfies, we loop back to check for more binary operators.
        // Example: `a + b as T` → `(a + b) as T` (binary first)
        // Example: `a as T + b` → `(a as T) + b` (as first, then binary)
        'infix: loop {
            // First, try to parse binary operators (higher precedence)
            loop {
                let kind = self.current_kind();

                // Check if this is an infix operator
                let Some((left_bp, right_bp, operator)) = infix_operator_info(kind) else {
                    break;
                };

                // Skip `in` operator when allow_in is false (parsing for-loop headers),
                // unless inside grouping delimiters where `in` is always a binary operator
                if matches!(operator, BinaryOperator::In)
                    && !self.allow_in
                    && self.grouping_depth == 0
                {
                    break;
                }

                // Check if operator binds tighter than minimum
                if left_bp < min_bp {
                    break;
                }

                // ES2016+: Unary expression as left operand of ** without parens is a syntax error
                // `-2 ** 3` is ambiguous - must be `(-2) ** 3` or `-(2 ** 3)`
                // Detect unparenthesized unary: actual_start equals expression span start
                if operator == BinaryOperator::StarStar
                    && matches!(left.expr, Expression::UnaryExpression(_))
                    && left.actual_start == left.expr.span().start as usize
                {
                    return Err(ParseError::InvalidSyntax {
                        message: "Unary expression cannot be the left operand of ** without parentheses. Use (-x) ** y or -(x ** y).".to_string(),
                        position: left.expr.span().start as usize,
                        context: None,
                    });
                }

                self.advance()?; // consume operator

                // Parse right-hand side with right binding power
                let right = self.parse_expression_bp(right_bp)?;

                // Create binary expression
                // Use expr_start (which includes any opening paren) instead of left.actual_start as u32
                // Use right.actual_end (position after parsing) to include closing parens
                let span = Span::new(expr_start as u32, right.actual_end as u32);
                left = ParsedExpr {
                    expr: Expression::BinaryExpression(BinaryExpression {
                        left: arena.alloc(left.expr),
                        operator,
                        right: arena.alloc(right.expr),
                        span,
                    }),
                    actual_start: expr_start,
                    actual_end: right.actual_end,
                };
            }

            // Then, try TypeScript `as` and `satisfies` operators (lower precedence than binary)
            // Enabled when: explicitly allowed (normal TS context), OR inside grouping
            // delimiters where `as` can never be the Svelte `#each` binding separator
            if min_bp <= BP_TS_TYPE_ASSERTION
                && (self.allow_ts_type_assertions || self.grouping_depth > 0)
            {
                match self.current_kind() {
                    TokenKind::Keyword(KeywordKind::As) => {
                        self.advance()?; // consume 'as'
                        let type_annotation = arena.alloc(self.parse_type_no_asi_bracket()?);
                        let span = Span::new(expr_start as u32, type_annotation.span().end);
                        left = ParsedExpr {
                            expr: Expression::TSAsExpression(TSAsExpression {
                                expression: arena.alloc(left.expr),
                                type_annotation,
                                span,
                            }),
                            actual_start: expr_start,
                            actual_end: span.end_usize(),
                        };
                        // After parsing as, loop back to check for more binary operators
                        continue 'infix;
                    }
                    TokenKind::Keyword(KeywordKind::Satisfies) => {
                        self.advance()?; // consume 'satisfies'
                        let type_annotation = arena.alloc(self.parse_type_no_asi_bracket()?);
                        let span = Span::new(expr_start as u32, type_annotation.span().end);
                        left = ParsedExpr {
                            expr: Expression::TSSatisfiesExpression(TSSatisfiesExpression {
                                expression: arena.alloc(left.expr),
                                type_annotation,
                                span,
                            }),
                            actual_start: expr_start,
                            actual_end: span.end_usize(),
                        };
                        // After parsing satisfies, loop back to check for more binary operators
                        continue 'infix;
                    }
                    _ => {}
                }
            }

            // No more infix operators or type assertions
            break;
        }

        // Handle assignment operator (after binary ops, before ternary)
        // Assignment is right-associative and has low precedence
        // Check for simple `=` and compound assignment operators (+=, -=, etc.)
        if min_bp <= BP_ASSIGNMENT
            && let Some(operator) = self.try_assignment_operator()
        {
            self.advance()?; // consume assignment operator

            // Parse right-hand side (assignment is right-associative, so same precedence)
            let right = self.parse_expression_bp(BP_ASSIGNMENT)?;

            // Convert left side to pattern if needed (cover grammar)
            let left_pattern = self.to_assignable(left.expr)?;

            let span = Span::new(expr_start as u32, right.actual_end as u32);
            left = ParsedExpr {
                expr: Expression::AssignmentExpression(AssignmentExpression {
                    left: arena.alloc(left_pattern),
                    operator,
                    right: arena.alloc(right.expr),
                    span,
                }),
                actual_start: expr_start,
                actual_end: right.actual_end,
            };
        }

        // Handle ternary operator (lowest precedence among binary-like ops, above comma)
        // Handle at BP_ASSIGNMENT to include in assignment expressions but not in binary ops
        if min_bp <= BP_ASSIGNMENT && self.eat(TokenKind::Question) {
            // Parse consequent (then branch) - use BP_ASSIGNMENT to exclude comma operator
            // This ensures (a ? b : c, d) parses as ((a ? b : c), d) not (a ? b : (c, d))
            // The consequent is `AssignmentExpression[+In]` — `in` is always the
            // binary operator here, even inside a for-header init. The alternate
            // is `[?In]` and inherits the outer context (so `for (a ? b : x in y;;)`
            // still rejects).
            let consequent = self.with_allow_in(|p| p.parse_expression_bp(BP_ASSIGNMENT))?;

            // Expect ':'
            self.expect(&TokenKind::Colon)?;

            // Parse alternate (else branch) - use BP_ASSIGNMENT to exclude comma operator
            let alternate = self.parse_expression_bp(BP_ASSIGNMENT)?;

            let span = Span::new(expr_start as u32, alternate.actual_end as u32);
            left = ParsedExpr {
                expr: Expression::ConditionalExpression(ConditionalExpression {
                    test: arena.alloc(left.expr),
                    consequent: arena.alloc(consequent.expr),
                    alternate: arena.alloc(alternate.expr),
                    span,
                }),
                actual_start: expr_start,
                actual_end: alternate.actual_end,
            };
        }

        // Handle comma operator (lowest precedence, after ternary)
        // Only handle at top level (BP_COMMA) to avoid conflicts with comma in
        // function calls, array literals, and object literals
        if min_bp == BP_COMMA && self.check(&TokenKind::Comma) {
            let mut expressions = self.bvec();
            expressions.push(left.expr);
            let mut last_end = left.actual_end;

            while self.eat(TokenKind::Comma) {
                // Parse next expression - use BP_ASSIGNMENT to stop before next comma
                let next = self.parse_expression_bp(BP_ASSIGNMENT)?;
                expressions.push(next.expr);
                last_end = next.actual_end;
            }

            let span = Span::new(expr_start as u32, last_end as u32);
            left = ParsedExpr {
                expr: Expression::SequenceExpression(SequenceExpression {
                    expressions: expressions.into_bump_slice(),
                    span,
                }),
                actual_start: expr_start,
                actual_end: last_end,
            };
        }

        Ok(left)
    }

    /// Parse prefix expression returning ParsedExpr with actual end position
    fn parse_prefix_expression_with_end(&mut self) -> Result<ParsedExpr<'arena>, ParseError> {
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
            TokenKind::Keyword(KeywordKind::Import) => {
                // Could be: import('module') or import.meta
                let expr = self.parse_import_or_meta_property()?;
                ParsedExpr::from_expr(expr)
            }
            TokenKind::Keyword(KeywordKind::Typeof | KeywordKind::Void | KeywordKind::Delete) => {
                let expr = self.parse_unary_keyword_expression()?;
                ParsedExpr::from_expr(expr)
            }
            TokenKind::Keyword(KeywordKind::Await) => {
                if self.in_await {
                    // `[+Await]` context: a real await expression.
                    let expr = self.parse_await_expression()?;
                    ParsedExpr::from_expr(expr)
                } else if self.await_is_identifier() {
                    if self.is_single_param_arrow_start() {
                        // Script `[~Await]`: `await => …` is an arrow whose single
                        // `BindingIdentifier` parameter is `await`.
                        ParsedExpr::from_expr(self.parse_single_param_arrow_function()?)
                    } else {
                        // Script `[~Await]`: `await` is an ordinary `IdentifierReference`.
                        self.parse_await_identifier_reference()?
                    }
                } else {
                    // Module `[~Await]`: `await` is reserved and there is no
                    // `[+Await]` to make it an await expression.
                    return Err(self.error_msg(
                        "'await' is only allowed inside an async function or at the top level of a module",
                    ));
                }
            }
            TokenKind::Keyword(KeywordKind::Yield) => {
                let expr = self.parse_yield_expression()?;
                ParsedExpr::from_expr(expr)
            }
            TokenKind::Keyword(KeywordKind::Async) => {
                // Could be async arrow function, async function expression, or just identifier
                // Look ahead to determine what follows 'async'
                let peek = self.peek_kind();
                if peek == TokenKind::Keyword(KeywordKind::Function) {
                    // Async function expression: `async function() {}`
                    let (start, _) = self.current_pos();
                    self.advance()?; // consume 'async'
                    let expr = self.parse_async_function_expression(start)?;
                    ParsedExpr::from_expr(expr)
                } else if peek == TokenKind::ParenOpen {
                    // `async(...)` — could be async arrow or call to function named `async`
                    // Scan ahead: if `(...)` is followed by `=>`, it's an async arrow function
                    let paren_start = self.peek_start();
                    if scan_parens_then_arrow(self.source.as_bytes(), paren_start) {
                        let (start, _) = self.current_pos();
                        self.advance()?; // consume 'async'
                        let expr = self.parse_async_arrow_function_after_async(start)?;
                        ParsedExpr::from_expr(expr)
                    } else {
                        // `async(2)` — call to function named `async`
                        self.parse_primary_expression_with_end()?
                    }
                } else if matches!(peek, TokenKind::Identifier | TokenKind::LessThan) {
                    // Async arrow function: `async x => ...` or `async <T>() => ...`
                    let (start, _) = self.current_pos();
                    self.advance()?; // consume 'async'
                    let expr = self.parse_async_arrow_function_after_async(start)?;
                    ParsedExpr::from_expr(expr)
                } else {
                    // `async` used as identifier (e.g., `[async]`, `async = 1`)
                    self.parse_primary_expression_with_end()?
                }
            }
            TokenKind::At => {
                // Decorated class expression: `@dec class {}`. Decorators are
                // otherwise statement-only; in expression position the only thing
                // that may follow a decorator list is a class expression.
                let expr = self.parse_decorated_class_expression()?;
                ParsedExpr::from_expr(expr)
            }
            TokenKind::Keyword(KeywordKind::Class) => {
                // Class expression: `class { }` or `class Foo<T> { }`
                let expr = self.parse_class_expression()?;
                ParsedExpr::from_expr(expr)
            }
            TokenKind::Keyword(KeywordKind::Function) => {
                // Function expression: `function() {}` or `function name() {}`
                let expr = self.parse_function_expression()?;
                ParsedExpr::from_expr(expr)
            }
            TokenKind::LessThan => {
                // Generic arrow function: `<T>() => ...`
                if self.is_generic_arrow_function_start() {
                    let expr = self.parse_generic_arrow_function()?;
                    ParsedExpr::from_expr(expr)
                } else {
                    // TypeScript type assertion: `<T>expr`
                    let expr = self.parse_type_assertion()?;
                    ParsedExpr::from_expr(expr)
                }
            }
            TokenKind::Identifier => {
                // Check for single-param arrow function: `x => expr`
                if self.is_single_param_arrow_start() {
                    let expr = self.parse_single_param_arrow_function()?;
                    ParsedExpr::from_expr(expr)
                } else {
                    // Regular identifier
                    self.parse_primary_expression_with_end()?
                }
            }
            _ => self.parse_primary_expression_with_end()?,
        };

        // An unparenthesized arrow function is a complete AssignmentExpression —
        // no subscripts can follow (`() => {}()` is invalid JS; a `(` on the next
        // line starts a new statement via ASI). A parenthesized arrow
        // (`(() => {})()`, detected by the span gap) is a callable primary.
        if matches!(parsed.expr, Expression::ArrowFunctionExpression(_))
            && parsed.actual_start == parsed.expr.span().start_usize()
        {
            return Ok(parsed);
        }

        // Parse any postfix operations (member access, call expressions)
        self.parse_postfix_expression(parsed, SubscriptMode::Normal)
    }

    /// Parse call arguments: `(arg1, arg2, ...)`
    ///
    /// Assumes the opening `(` has already been consumed.
    /// Returns the arguments and the end position of the closing `)`.
    pub(super) fn parse_call_arguments(
        &mut self,
    ) -> Result<(bumpalo::collections::Vec<'arena, Expression<'arena>>, usize), ParseError> {
        self.grouping_depth += 1;
        let mut arguments = self.bvec();

        if !self.check(&TokenKind::ParenClose) {
            loop {
                let arg = self.parse_assignment_expression()?;
                arguments.push(arg);

                if !self.expect_list_separator(&TokenKind::Comma, &TokenKind::ParenClose)? {
                    break;
                }
            }
        }

        let (_, paren_end) = self.current_pos();
        self.expect(&TokenKind::ParenClose)?;
        self.grouping_depth -= 1;
        Ok((arguments, paren_end))
    }

    /// Parse postfix expressions: member access (`.`, `[`), call expressions (`()`)
    ///
    /// Handles chained expressions like `obj.prop`, `arr[0]`, `foo()`, `obj.method().prop`
    /// Wrap `expr` in a `TSNonNullExpression`, consuming the current `!` token. The
    /// span starts at `expr.actual_start` so it covers any grouping parens (`(a?.b)!`),
    /// which `TSNonNullExpression::seals_optional_chain` relies on. Callers gate on
    /// `Bang` + `!had_line_terminator` (ASI: a line terminator before `!` makes it a
    /// prefix `!` on the next statement instead).
    fn wrap_non_null_assertion(
        &mut self,
        expr: ParsedExpr<'arena>,
    ) -> Result<ParsedExpr<'arena>, ParseError> {
        let (_, op_end) = self.current_pos();
        self.advance()?; // consume '!'
        let span = Span::new(expr.actual_start as u32, op_end as u32);
        Ok(ParsedExpr::with_end(
            Expression::TSNonNullExpression(TSNonNullExpression {
                expression: self.alloc(expr.expr),
                span,
            }),
            op_end,
        ))
    }

    fn parse_postfix_expression(
        &mut self,
        mut left: ParsedExpr<'arena>,
        mode: SubscriptMode,
    ) -> Result<ParsedExpr<'arena>, ParseError> {
        let arena = self.arena;
        // Tracks whether an optional `?.` was consumed in THIS subscript chain (not
        // inside a parenthesized sub-expression — those are parsed as their own
        // primary with a fresh flag). An optional chain may not be the tag of a
        // tagged template (`a?.b`x`` is a syntax error); a parenthesized chain
        // (`(a?.b)`x``) seals it and is valid. Mirrors acorn's `optionalChained`.
        let mut optional_chained = false;
        loop {
            match self.current_kind() {
                TokenKind::Dot => {
                    // Member access: obj.prop or obj.#private
                    self.advance()?; // consume '.'

                    // Property must be an identifier, keyword, or private identifier
                    // Keywords are valid property names: obj.class, obj.if, obj.default()
                    let (property, prop_end) = if *self.current_kind() == TokenKind::Hash {
                        // Private identifier: obj.#private
                        let private_id = self.parse_private_identifier()?;
                        let end = private_id.span.end_usize();
                        (Expression::PrivateIdentifier(private_id), end)
                    } else if self.current_is_identifier_or_keyword() {
                        let (prop_start, prop_end) = self.current_pos();
                        let name = self.intern(self.current_property_name());
                        self.advance()?;
                        (
                            Expression::Identifier(Identifier::simple(
                                name,
                                Span::new(prop_start as u32, prop_end as u32),
                            )),
                            prop_end,
                        )
                    } else {
                        return Err(self.error_expected_after("property name", "."));
                    };

                    let span = Span::new(left.actual_start as u32, prop_end as u32);
                    left = ParsedExpr::with_end(
                        Expression::MemberExpression(MemberExpression {
                            object: arena.alloc(left.expr),
                            property: arena.alloc(property),
                            computed: false,
                            optional: false,
                            span,
                        }),
                        prop_end,
                    );
                }
                TokenKind::QuestionDot => {
                    // Optional chaining: obj?.prop, obj?.#private, obj?.[expr], obj?.()
                    self.advance()?; // consume '?.'
                    optional_chained = true;

                    match self.current_kind() {
                        TokenKind::Hash => {
                            // obj?.#private - optional private property access
                            let private_id = self.parse_private_identifier()?;
                            let prop_end = private_id.span.end_usize();

                            let span = Span::new(left.actual_start as u32, prop_end as u32);
                            left = ParsedExpr::with_end(
                                Expression::MemberExpression(MemberExpression {
                                    object: arena.alloc(left.expr),
                                    property: arena
                                        .alloc(Expression::PrivateIdentifier(private_id)),
                                    computed: false,
                                    optional: true,
                                    span,
                                }),
                                prop_end,
                            );
                        }
                        TokenKind::Identifier | TokenKind::Keyword(_) => {
                            // obj?.prop - optional property access
                            // Keywords are valid property names: obj?.class, obj?.if, obj?.default()
                            let (prop_start, prop_end) = self.current_pos();
                            let name = self.intern(self.current_property_name());
                            self.advance()?;

                            let span = Span::new(left.actual_start as u32, prop_end as u32);
                            left = ParsedExpr::with_end(
                                Expression::MemberExpression(MemberExpression {
                                    object: arena.alloc(left.expr),
                                    property: arena.alloc(Expression::Identifier(
                                        Identifier::simple(
                                            name,
                                            Span::new(prop_start as u32, prop_end as u32),
                                        ),
                                    )),
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
                            self.grouping_depth += 1;

                            let index = self.parse_expression()?;

                            let (_, bracket_end) = self.current_pos();
                            self.expect(&TokenKind::BracketClose)?; // consume ']'
                            self.grouping_depth -= 1;

                            let span = Span::new(left.actual_start as u32, bracket_end as u32);
                            left = ParsedExpr::with_end(
                                Expression::MemberExpression(MemberExpression {
                                    object: arena.alloc(left.expr),
                                    property: arena.alloc(index),
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
                            let (arguments, paren_end) = self.parse_call_arguments()?;
                            let arguments = arguments.into_bump_slice();

                            let span = Span::new(left.actual_start as u32, paren_end as u32);
                            left = ParsedExpr::with_end(
                                Expression::CallExpression(CallExpression {
                                    callee: arena.alloc(left.expr),
                                    type_arguments: None,
                                    arguments,
                                    optional: true,
                                    span,
                                }),
                                paren_end,
                            );
                        }
                        TokenKind::LessThan if self.is_type_arguments_start() => {
                            // obj?.<T>(args) - optional call with explicit type arguments;
                            // only a call may follow (`a?.<T>` without `(` is a syntax error)
                            let type_args = self.parse_type_parameter_instantiation()?;
                            if *self.current_kind() != TokenKind::ParenOpen {
                                return Err(self.error_expected_after(
                                    "'('",
                                    "type arguments in optional call",
                                ));
                            }
                            self.advance()?; // consume '('
                            let (arguments, paren_end) = self.parse_call_arguments()?;
                            let arguments = arguments.into_bump_slice();

                            let span = Span::new(left.actual_start as u32, paren_end as u32);
                            left = ParsedExpr::with_end(
                                Expression::CallExpression(CallExpression {
                                    callee: arena.alloc(left.expr),
                                    type_arguments: Some(type_args),
                                    arguments,
                                    optional: true,
                                    span,
                                }),
                                paren_end,
                            );
                        }
                        _ => {
                            return Err(
                                self.error_expected_after("property name, '[', or '('", "?.")
                            );
                        }
                    }
                }
                TokenKind::BracketOpen => {
                    // Computed member access: arr[0]
                    self.advance()?; // consume '['
                    self.grouping_depth += 1;

                    let index = self.parse_expression()?;

                    let (_, bracket_end) = self.current_pos();
                    self.expect(&TokenKind::BracketClose)?; // consume ']'
                    self.grouping_depth -= 1;

                    let span = Span::new(left.actual_start as u32, bracket_end as u32);
                    left = ParsedExpr::with_end(
                        Expression::MemberExpression(MemberExpression {
                            object: arena.alloc(left.expr),
                            property: arena.alloc(index),
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
                    let (arguments, paren_end) = self.parse_call_arguments()?;
                    let arguments = arguments.into_bump_slice();

                    let span = Span::new(left.actual_start as u32, paren_end as u32);
                    // When callee is TSInstantiationExpression (e.g., foo<T>), flatten:
                    // TSInstantiationExpression + CallExpression → CallExpression with typeArguments
                    let (callee, type_arguments) = match left.expr {
                        Expression::TSInstantiationExpression(inst) => {
                            (inst.expression, Some(inst.type_arguments))
                        }
                        other => (&*arena.alloc(other), None),
                    };
                    left = ParsedExpr::with_end(
                        Expression::CallExpression(CallExpression {
                            callee,
                            type_arguments,
                            arguments,
                            optional: false,
                            span,
                        }),
                        paren_end,
                    );
                }
                TokenKind::NoSubstitutionTemplate | TokenKind::TemplateHead => {
                    // Tagged template expression: tag`content`
                    let quasi = self.parse_template_literal(true)?;
                    let quasi_span = quasi.span();
                    if let Expression::TemplateLiteral(template) = quasi {
                        // An optional chain can't be a template tag (per spec): `a?.b`x``
                        // is a syntax error. A parenthesized chain (`(a?.b)`x``) seals the
                        // chain (consumed as its own primary, so `optional_chained` is
                        // false here) and is valid.
                        if optional_chained {
                            return Err(self.error_msg(
                                "Optional chaining cannot appear in the tag of tagged template expressions",
                            ));
                        }
                        let span = Span::new(left.actual_start as u32, quasi_span.end);
                        // When tag is TSInstantiationExpression (e.g., tag<T>), flatten:
                        // TSInstantiationExpression + TaggedTemplate → TaggedTemplate with typeArguments
                        let (tag, type_arguments) = match left.expr {
                            Expression::TSInstantiationExpression(inst) => {
                                (inst.expression, Some(inst.type_arguments))
                            }
                            other => (&*arena.alloc(other), None),
                        };
                        left = ParsedExpr::with_end(
                            Expression::TaggedTemplateExpression(TaggedTemplateExpression {
                                tag,
                                type_arguments,
                                quasi: template,
                                span,
                            }),
                            quasi_span.end_usize(),
                        );
                    }
                }
                kind @ TokenKind::PlusPlus | kind @ TokenKind::MinusMinus
                    if mode == SubscriptMode::Normal && !self.had_line_terminator =>
                {
                    // Postfix update expression: x++, x--
                    // ASI Rule: If there's a line terminator before ++/--, ASI fires
                    // and the ++/-- becomes a prefix operator on the next statement.
                    // So we only parse postfix if NO line terminator preceded this token.
                    let operator = if *kind == TokenKind::PlusPlus {
                        UpdateOperator::Increment
                    } else {
                        UpdateOperator::Decrement
                    };

                    let (_, op_end) = self.current_pos();
                    self.advance()?;

                    let span = Span::new(left.actual_start as u32, op_end as u32);
                    left = ParsedExpr::with_end(
                        Expression::UpdateExpression(UpdateExpression {
                            operator,
                            argument: arena.alloc(left.expr),
                            prefix: false,
                            span,
                        }),
                        op_end,
                    );
                }
                TokenKind::LessThan => {
                    // Might be TSInstantiationExpression: f<T>, expr<Type>
                    // If it parses as type arguments it's instantiation; otherwise
                    // let binary expression handle it as comparison.
                    //
                    // In ClassHeritage mode a bare `<T>` (type args NOT followed by a
                    // call) belongs to the class's `super_type_parameters` split, so we
                    // leave it for the caller; only `Base<T>(...)` is consumed here (the
                    // `(` arm below flattens the instantiation into a call with type
                    // arguments). A `<` that isn't type args stops the chain either way.
                    let consume = self.is_type_arguments_start()
                        && (mode == SubscriptMode::Normal || self.is_type_args_followed_by_call());
                    if consume {
                        left = self.parse_instantiation_expression(left)?;
                    } else {
                        break;
                    }
                }
                TokenKind::Bang if !self.had_line_terminator => {
                    // TypeScript non-null assertion: `expr!`.
                    left = self.wrap_non_null_assertion(left)?;
                }
                _ => break,
            }
        }

        Ok(left)
    }

    /// Parse the superclass expression of an `extends` clause.
    ///
    /// Per ecma262 §15.7 `ClassHeritage : extends LeftHandSideExpression`. Matches
    /// acorn's `parseExprSubscripts` with `canBeArrow = false`: a primary atom
    /// (identifier, literal, parenthesized expression, array/object, template,
    /// `class`/`function` expression, `new`, `import()`) followed by member/call/
    /// non-null/tagged-template subscripts. Non-LHS forms (ternary, binary, unary,
    /// `await`, assignment, and a top-level `=>` arrow) are excluded — the class body
    /// then errors when it fails to find `{`. A bare `<T>` is left for the class's
    /// `super_type_parameters` split via `SubscriptMode::ClassHeritage`.
    pub(in crate::parser) fn parse_heritage_expression(
        &mut self,
    ) -> Result<Expression<'arena>, ParseError> {
        let atom = self.parse_heritage_atom()?;
        let parsed = self.parse_postfix_expression(atom, SubscriptMode::ClassHeritage)?;
        Ok(parsed.expr)
    }

    /// Parse the primary atom of an `extends` clause (acorn's `parseExprAtom` with
    /// `canBeArrow = false`). `parse_primary_expression_with_end` covers identifiers,
    /// literals, parens, arrays, objects, templates, `this`/`super`, and regex; the
    /// keyword-led expression atoms (`new`, `class`, `function`, `async function`,
    /// `import`) sit above the primary layer and are dispatched here.
    fn parse_heritage_atom(&mut self) -> Result<ParsedExpr<'arena>, ParseError> {
        let (start, _) = self.current_pos();
        // `async function () {}` is the only `async`-led heritage atom. A bare `async`
        // (or an `async`-arrow, which isn't a valid heritage atom) falls through to the
        // primary path, where `async` parses as a plain identifier. Peeking needs
        // `&mut self`, so this can't be a match guard over the borrowing `current_kind`.
        if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Async))
            && self.peek_kind() == TokenKind::Keyword(KeywordKind::Function)
        {
            self.advance()?; // consume 'async'
            return Ok(ParsedExpr::from_expr(
                self.parse_async_function_expression(start)?,
            ));
        }
        match self.current_kind() {
            TokenKind::Keyword(KeywordKind::New) => {
                Ok(ParsedExpr::from_expr(self.parse_new_expression()?))
            }
            TokenKind::Keyword(KeywordKind::Class) => {
                Ok(ParsedExpr::from_expr(self.parse_class_expression()?))
            }
            TokenKind::Keyword(KeywordKind::Function) => {
                Ok(ParsedExpr::from_expr(self.parse_function_expression()?))
            }
            TokenKind::Keyword(KeywordKind::Import) => {
                Ok(ParsedExpr::from_expr(self.parse_import_or_meta_property()?))
            }
            _ => {
                let parsed = self.parse_primary_expression_with_end()?;
                // Reject a top-level (unparenthesized) arrow: `class C extends (a) => b {}`.
                // acorn parses the heritage atom with `canBeArrow = false`, so an outer
                // arrow isn't a valid superclass. A *parenthesized* arrow
                // (`extends (a => b) {}`) is fine — there the arrow's own span starts
                // past the `(`, so it differs from `actual_start`. Same test the prefix
                // layer uses to seal parenthesized arrows.
                if matches!(parsed.expr, Expression::ArrowFunctionExpression(_))
                    && parsed.actual_start == parsed.expr.span().start_usize()
                {
                    return Err(self.error_msg(
                        "Arrow functions cannot be used as a class heritage expression",
                    ));
                }
                Ok(parsed)
            }
        }
    }

    /// Check whether the current `<` opens type arguments immediately followed by a
    /// call: `<T>(`. Used by the `ClassHeritage` subscript mode to tell a call's type
    /// arguments (`getMixin<T>(Base)`, consumed) from a bare superclass instantiation
    /// (`extends Base<T>`, left for the `super_type_parameters` split).
    fn is_type_args_followed_by_call(&self) -> bool {
        let bytes = self.source.as_bytes();
        let mut pos = self.current_start;

        // Must start with '<'
        if pos >= bytes.len() || bytes[pos] != b'<' {
            return false;
        }
        pos += 1;

        // Track nesting to find the matching '>'
        let mut depth = 1;
        while pos < bytes.len() && depth > 0 {
            match bytes[pos] {
                b'<' => depth += 1,
                b'>' => depth -= 1,
                b'\'' | b'"' | b'`' => {
                    // Skip strings
                    let quote = bytes[pos];
                    pos += 1;
                    while pos < bytes.len() && bytes[pos] != quote {
                        if bytes[pos] == b'\\' {
                            pos += 1; // skip escaped char
                        }
                        pos += 1;
                    }
                }
                _ => {}
            }
            pos += 1;
        }

        // Skip whitespace after '>'
        while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t' | b'\n' | b'\r') {
            pos += 1;
        }

        // Check if '(' follows
        pos < bytes.len() && bytes[pos] == b'('
    }

    /// Parse a Number token into a Literal, handling BigInt suffix
    pub(crate) fn parse_number_or_bigint_literal(&self) -> Result<Literal<'arena>, ParseError> {
        let (start, end) = self.current_pos();
        let raw = self.current_value();
        if raw.ends_with('n') {
            // BigInt — no stored payload; digits via `Literal::bigint_digits(source)`.
            Ok(Literal {
                value: LiteralValue::BigInt,
                span: Span::new(start as u32, end as u32),
            })
        } else {
            let number = parse_number_literal(raw)
                .map_err(|_| self.error_msg_at(&format!("Invalid number: {raw}"), start))?;
            Ok(Literal {
                value: LiteralValue::Number(number),
                span: Span::new(start as u32, end as u32),
            })
        }
    }

    /// Parse primary expression returning ParsedExpr with actual end position
    fn parse_primary_expression_with_end(&mut self) -> Result<ParsedExpr<'arena>, ParseError> {
        match self.current_kind() {
            TokenKind::Number => {
                let literal = self.parse_number_or_bigint_literal()?;
                let end = self.current_pos().1;
                self.advance()?;
                Ok(ParsedExpr::with_end(
                    Expression::Literal(literal),
                    end,
                ))
            }
            TokenKind::String => {
                let (start, end) = self.current_pos();
                let cooked = self.extract_string_cooked();
                self.advance()?;
                Ok(ParsedExpr::with_end(
                    Expression::Literal(Literal {
                        value: LiteralValue::String(cooked),
                        span: Span::new(start as u32, end as u32),
                    }),
                    end,
                ))
            }
            TokenKind::Identifier => {
                let (start, end) = self.current_pos();
                let symbol = self.intern_identifier();
                self.advance()?;
                Ok(ParsedExpr::with_end(
                    Expression::Identifier(Identifier::simple(
                        symbol,
                        Span::new(start as u32, end as u32),
                    )),
                    end,
                ))
            }
            // `await` as an ordinary `IdentifierReference` (Script `[~Await]`) —
            // e.g. a `new` callee (`new await()`) or any primary reference.
            TokenKind::Keyword(KeywordKind::Await) if self.await_is_identifier() => {
                self.parse_await_identifier_reference()
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
            TokenKind::Keyword(KeywordKind::This) => {
                let (start, end) = self.current_pos();
                self.advance()?;
                Ok(ParsedExpr::with_end(
                    Expression::ThisExpression(ThisExpression {
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
                // Untagged template literal (a primary expression): invalid escapes
                // are a syntax error (only tagged templates tolerate them).
                Ok(ParsedExpr::from_expr(self.parse_template_literal(false)?))
            }
            TokenKind::Slash | TokenKind::SlashEquals => {
                // In expression context, `/` or `/=` starts a regex literal, not division
                // Division is only possible after a value (identifier, number, closing bracket)
                // but we're in parse_primary_expression which is called when expecting a value.
                //
                // When lexer sees `/=`, it returns SlashEquals (division assignment), but in
                // expression-start context this is actually a regex starting with `=` like /=\s*/.
                //
                // Use current_start directly (not current_pos) because the lexer expects positions
                // relative to its source slice, not the full document offset.
                //
                // The relex re-reads source from the `/`: a populated peek cache here
                // would leave a stale token behind, and comments drained by that peek
                // would be re-read as regex pattern chars.
                debug_assert!(
                    self.peek_cache.is_none(),
                    "regex relex with populated peek cache"
                );
                let lexer_start = self.current_start;
                let (regex_token, pattern_end) = self.lexer.read_regex_literal(lexer_start)?;
                let lexer_end = regex_token.end;

                // Calculate span with base_offset for the AST
                let span_start = lexer_start + self.base_offset;
                let span_end = lexer_end + self.base_offset;

                // Pattern and flags are verbatim source slices (escapes preserved),
                // recovered from spans rather than owned strings. `pattern_end` is the
                // closing `/` (local): pattern is [slash+1, close), flags are
                // [close+1, token end). Spans are stored in host coordinates.
                let pattern_span =
                    Span::new(span_start as u32 + 1, (pattern_end + self.base_offset) as u32);
                let flags_span =
                    Span::new((pattern_end + 1 + self.base_offset) as u32, span_end as u32);
                // Precompute the pattern's visual width so the "simple call argument"
                // width check stays source-free; saturate to the field's u16 range.
                let pattern_width = visual_width(&self.source[lexer_start + 1..pattern_end], TAB_WIDTH)
                    .min(u16::MAX as usize) as u16;

                // Advance past the regex token by reading the next token. The regex relex
                // bypasses advance_inner() (the lexer was resynced by read_regex_literal), so
                // mirror the bookkeeping we need by hand: record the regex token's end as
                // prev_end (so prev_token_end() covers the regex — object Property/SpreadElement
                // spans read it), set the current token, refresh the ASI line-terminator flag,
                // and absorb any comments that land right after the regex (`{a: /re/ /*c*/}`) the
                // same way advance_inner() does — without this the comment stays as the current
                // token and the next consumer rejects it.
                self.prev_end = lexer_end;
                let next_token = self.lexer.next_token()?;
                self.update_current(next_token);
                // Update ASI state - line terminators after regex enable semicolon insertion
                self.had_line_terminator = self.lexer.had_line_terminator();
                self.collect_comments()?;

                Ok(ParsedExpr::with_end(
                    Expression::RegexLiteral(RegexLiteral {
                        pattern_span,
                        flags_span,
                        pattern_width,
                        span: Span::new(span_start as u32, span_end as u32),
                    }),
                    span_end,
                ))
            }
            TokenKind::Hash => {
                // Private identifier as standalone expression (for brand check: #field in obj)
                let private_id = self.parse_private_identifier()?;
                let end = private_id.span.end_usize();
                Ok(ParsedExpr::with_end(
                    Expression::PrivateIdentifier(private_id),
                    end,
                ))
            }
            // TypeScript type keywords and contextual keywords are valid identifiers in expression context
            TokenKind::Keyword(KeywordKind::Number)
            | TokenKind::Keyword(KeywordKind::String)
            | TokenKind::Keyword(KeywordKind::Boolean)
            | TokenKind::Keyword(KeywordKind::Any)
            | TokenKind::Keyword(KeywordKind::Void)
            | TokenKind::Keyword(KeywordKind::Never)
            | TokenKind::Keyword(KeywordKind::Unknown)
            | TokenKind::Keyword(KeywordKind::Object)
            | TokenKind::Keyword(KeywordKind::Symbol)
            | TokenKind::Keyword(KeywordKind::Bigint)
            // `undefined` is a global identifier (not a ReservedWord), so in value
            // position it is an `Identifier` named "undefined" — never a literal.
            // This makes it a valid assignment target (`undefined = 12`). acorn
            // models it the same way.
            | TokenKind::Keyword(KeywordKind::Undefined)
            // Contextual keywords that can be identifiers
            | TokenKind::Keyword(KeywordKind::Async)
            | TokenKind::Keyword(KeywordKind::From)
            | TokenKind::Keyword(KeywordKind::As)
            | TokenKind::Keyword(KeywordKind::Satisfies) => {
                let (start, end) = self.current_pos();
                let symbol = self.intern(self.current_value());
                self.advance()?;
                Ok(ParsedExpr::with_end(
                    Expression::Identifier(Identifier::simple(
                        symbol,
                        Span::new(start as u32, end as u32),
                    )),
                    end,
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
    fn parse_paren_expression_with_end(&mut self) -> Result<ParsedExpr<'arena>, ParseError> {
        // Check if this looks like an arrow function by scanning ahead
        if self.is_arrow_function_start() {
            return Ok(ParsedExpr::from_expr(self.parse_arrow_function()?));
        }

        // Parse as grouped expression: (expr)
        // Track actual_start BEFORE '(' and actual_end AFTER ')' for correct spans
        // when this expression is used as a callee: (a ? b : c)() should have
        // CallExpression span starting at '(', not at 'a'
        //
        // JSDoc type cast parens (`/** @type {T} */ (expr)`) are special: the parens
        // are semantically required (without them the cast is dropped), so when a
        // `@type`/`@satisfies` block comment immediately precedes the `(` we wrap the
        // inner expression in a `JsdocCast` node to preserve them, instead of
        // discarding them like ordinary grouping parens. Comments themselves are
        // still located positionally in the flat `Vec<Comment>` at print time.
        let (paren_start, _) = self.current_pos();
        let is_jsdoc_cast = self.paren_preceded_by_jsdoc_cast_comment(paren_start);
        self.expect(&TokenKind::ParenOpen)?; // consume '('

        self.grouping_depth += 1;
        let parsed = self.parse_expression_bp(BP_COMMA)?;

        // Capture the end position of ')' before consuming it
        let (_, paren_end) = self.current_pos();
        self.expect(&TokenKind::ParenClose)?; // consume ')'
        self.grouping_depth -= 1;

        // Return expression with its original span (excluding parens), but with
        // actual_start before '(' and actual_end after ')' for containing expressions.
        // A JSDoc cast keeps the same actual bounds (so containing expressions still
        // get the paren-inclusive span) but carries the explicit wrapper node.
        if is_jsdoc_cast {
            let cast = Expression::JsdocCast(JsdocCast {
                inner: self.alloc(parsed.expr),
                span: Span::new(paren_start as u32, paren_end as u32),
            });
            return Ok(ParsedExpr::with_bounds(cast, paren_start, paren_end));
        }
        Ok(ParsedExpr::with_bounds(parsed.expr, paren_start, paren_end))
    }

    /// Whether a `(` at `paren_start` is immediately preceded by a JSDoc type-cast
    /// block comment (`/** @type {T} */` / `/** @satisfies {T} */`), making the
    /// parens a TypeScript cast that must be preserved.
    ///
    /// Mirrors prettier's predicate (`is-type-cast-comment.js` +
    /// `postprocess/index.js`): a **block** comment whose text starts with `*`
    /// (the `/**` form) and contains `@type`/`@satisfies` (word boundary), with
    /// only whitespace between the comment's `*/` and the `(`.
    fn paren_preceded_by_jsdoc_cast_comment(&self, paren_start: usize) -> bool {
        let bytes = self.source.as_bytes();
        // `paren_start` is a full-file offset (`current_pos` adds `base_offset`);
        // `self.source` is the local slice, so translate back to a local index.
        let mut i = paren_start - self.base_offset;
        // Walk back over whitespace immediately before '('.
        while i > 0 && bytes[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        // The preceding token must be a block comment ending exactly at `i`.
        if i < 4 || &bytes[i - 2..i] != b"*/" {
            return false;
        }
        // Find the start of that block comment (block comments don't nest in JS,
        // so the last `/*` before `*/` opens it).
        let Some(open) = self.source[..i - 2].rfind("/*") else {
            return false;
        };
        let value = &self.source[open + 2..i - 2]; // text between `/*` and `*/`
        is_jsdoc_type_cast_comment(value)
    }

    /// Parse a TypeScript angle-bracket type assertion: `<Type>expr`
    ///
    /// This is the old-style type assertion syntax. It's equivalent to `expr as Type`
    /// but doesn't work in JSX because it looks like an element.
    ///
    /// Example: `<string>value`, `<T>a`
    fn parse_type_assertion(&mut self) -> Result<Expression<'arena>, ParseError> {
        let arena = self.arena;
        let (start, _) = self.current_pos();

        // Parse <Type>
        self.expect(&TokenKind::LessThan)?; // consume '<'
        let type_annotation = self.parse_type()?;
        self.expect(&TokenKind::GreaterThan)?; // consume '>'

        // Parse the expression - use high binding power since type assertion is prefix
        let parsed = self.parse_expression_bp(BP_UNARY)?;
        let end = parsed.actual_end as u32;

        Ok(Expression::TSTypeAssertion(TSTypeAssertion {
            type_annotation: arena.alloc(type_annotation),
            expression: arena.alloc(parsed.expr),
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse a TypeScript instantiation expression `expr<Type>` from an
    /// already-parsed `left` operand.
    ///
    /// The caller gates this on `is_type_arguments_start()`: when `<` follows an
    /// expression it could be type arguments (`f<number>`, `arr<T, U>`) or a
    /// comparison (`a < b`), and that lookahead disambiguates before we commit.
    fn parse_instantiation_expression(
        &mut self,
        left: ParsedExpr<'arena>,
    ) -> Result<ParsedExpr<'arena>, ParseError> {
        // Parse type parameter instantiation: <T, U>
        let type_args = self.parse_type_parameter_instantiation()?;
        let end = type_args.span.end;

        let inst = TSInstantiationExpression {
            expression: self.alloc(left.expr),
            type_arguments: type_args,
            span: Span::new(left.actual_start as u32, end),
        };

        Ok(ParsedExpr::with_end(
            Expression::TSInstantiationExpression(inst),
            end as usize,
        ))
    }

    /// Parse unary expression: `-x`, `+x`, `!x`, `~x`
    ///
    /// Unary operators have higher precedence than all binary operators.
    /// The binding power (29) is higher than exponentiation (27-28).
    fn parse_unary_expression(&mut self) -> Result<Expression<'arena>, ParseError> {
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
        let parsed = self.parse_expression_bp(BP_UNARY)?;

        // Use actual_end to include trailing parens in the span (matches Svelte's behavior)
        // For `!(a && b)`, the span should be from `!` to `)`, not just to `b`
        let end = parsed.actual_end as u32;

        Ok(Expression::UnaryExpression(UnaryExpression {
            operator,
            argument: self.alloc(parsed.expr),
            prefix: true,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse unary keyword expression: `typeof x`, `void 0`, `delete obj.x`
    ///
    /// These keyword operators have the same precedence as other unary operators.
    fn parse_unary_keyword_expression(&mut self) -> Result<Expression<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        let operator = match self.current_kind() {
            TokenKind::Keyword(KeywordKind::Typeof) => UnaryOperator::Typeof,
            TokenKind::Keyword(KeywordKind::Void) => UnaryOperator::Void,
            TokenKind::Keyword(KeywordKind::Delete) => UnaryOperator::Delete,
            _ => unreachable!("parse_unary_keyword_expression called with non-keyword operator"),
        };
        self.advance()?;

        // Parse the operand with high binding power (unary is right-associative)
        let parsed = self.parse_expression_bp(BP_UNARY)?;
        let end = parsed.actual_end as u32;

        Ok(Expression::UnaryExpression(UnaryExpression {
            operator,
            argument: self.alloc(parsed.expr),
            prefix: true,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse await expression: `await promise`
    ///
    /// Await expressions have high precedence like unary operators.
    fn parse_await_expression(&mut self) -> Result<Expression<'arena>, ParseError> {
        let (start, _) = self.current_pos();
        self.advance()?; // consume 'await'

        // Parse the operand with high binding power (same as unary)
        let parsed = self.parse_expression_bp(BP_UNARY)?;
        let end = parsed.actual_end as u32;

        Ok(Expression::AwaitExpression(AwaitExpression {
            argument: self.alloc(parsed.expr),
            span: Span::new(start as u32, end),
        }))
    }

    /// Consume the current `await` token as an ordinary `IdentifierReference`
    /// (Script `[~Await]`). The caller must have verified `await_is_identifier()`
    /// — used for a primary reference and a `new` callee (`new await()`).
    fn parse_await_identifier_reference(&mut self) -> Result<ParsedExpr<'arena>, ParseError> {
        let (start, end) = self.current_pos();
        let symbol = self.intern("await");
        self.advance()?;
        Ok(ParsedExpr::with_end(
            Expression::Identifier(Identifier::simple(
                symbol,
                Span::new(start as u32, end as u32),
            )),
            end,
        ))
    }

    /// Parse yield expression: `yield`, `yield value`, or `yield* iterable`
    ///
    /// Yield expressions have low precedence (lowest in expressions).
    /// - `yield` with no argument yields undefined
    /// - `yield value` yields the given value
    /// - `yield* iterable` delegates to another generator/iterable
    fn parse_yield_expression(&mut self) -> Result<Expression<'arena>, ParseError> {
        let arena = self.arena;
        let (start, yield_end) = self.current_pos();
        self.advance()?; // consume 'yield'

        // Check for delegate: yield*
        let delegate = if matches!(self.current_kind(), TokenKind::Star) {
            self.advance()?; // consume '*'
            true
        } else {
            false
        };

        // Check if there's an argument
        // yield has very low precedence, so we need to be careful about what follows
        // If the next token can start an expression and isn't a statement terminator, parse it
        let (argument, end) = if delegate {
            // yield* requires an argument
            let parsed = self.parse_expression_bp(BP_YIELD)?;
            let end = parsed.actual_end as u32;
            (Some(&*arena.alloc(parsed.expr)), end)
        } else if self.can_insert_semicolon() || matches!(self.current_kind(), TokenKind::Eof) {
            // No argument - yield with no value
            (None, yield_end as u32)
        } else if self.is_expression_start() {
            // Parse the argument
            let parsed = self.parse_expression_bp(BP_YIELD)?;
            let end = parsed.actual_end as u32;
            (Some(&*arena.alloc(parsed.expr)), end)
        } else {
            // No argument
            (None, yield_end as u32)
        };

        Ok(Expression::YieldExpression(YieldExpression {
            argument,
            delegate,
            span: Span::new(start as u32, end),
        }))
    }

    /// Check if the current token can start an expression
    fn is_expression_start(&self) -> bool {
        match self.current_kind() {
            TokenKind::Identifier
            | TokenKind::Number
            | TokenKind::String
            | TokenKind::BraceOpen
            | TokenKind::BracketOpen
            | TokenKind::ParenOpen
            | TokenKind::Bang
            | TokenKind::Minus
            | TokenKind::Plus
            | TokenKind::Tilde
            | TokenKind::PlusPlus
            | TokenKind::MinusMinus
            | TokenKind::NoSubstitutionTemplate
            | TokenKind::TemplateHead
            | TokenKind::RegexLiteral
            | TokenKind::LessThan
            | TokenKind::Hash => true,
            // Most keywords can start expressions (as primaries, unary ops, or contextual identifiers).
            // Exclude only declaration/control-flow/binary-operator keywords that cannot.
            TokenKind::Keyword(kw) => !matches!(
                kw,
                KeywordKind::Const
                    | KeywordKind::Let
                    | KeywordKind::Var
                    | KeywordKind::If
                    | KeywordKind::Else
                    | KeywordKind::For
                    | KeywordKind::While
                    | KeywordKind::Do
                    | KeywordKind::Switch
                    | KeywordKind::Case
                    | KeywordKind::Default
                    | KeywordKind::Break
                    | KeywordKind::Continue
                    | KeywordKind::Try
                    | KeywordKind::Catch
                    | KeywordKind::Finally
                    | KeywordKind::Throw
                    | KeywordKind::Return
                    | KeywordKind::Export
                    | KeywordKind::Extends
                    | KeywordKind::Instanceof
                    | KeywordKind::In
                    | KeywordKind::Enum
                    | KeywordKind::Debugger
            ),
            _ => false,
        }
    }

    /// Parse prefix update expression: `++x`, `--x`
    ///
    /// Prefix increment/decrement has the same precedence as unary operators.
    fn parse_prefix_update_expression(&mut self) -> Result<Expression<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        let operator = match self.current_kind() {
            TokenKind::PlusPlus => UpdateOperator::Increment,
            TokenKind::MinusMinus => UpdateOperator::Decrement,
            _ => unreachable!("parse_prefix_update_expression called with non-update operator"),
        };
        self.advance()?;

        // Parse the operand - update expressions apply to member expressions or identifiers
        // Use high binding power so ++x.y parses correctly
        let parsed = self.parse_expression_bp(BP_UNARY)?;

        let end = parsed.actual_end as u32;

        Ok(Expression::UpdateExpression(UpdateExpression {
            operator,
            argument: self.alloc(parsed.expr),
            prefix: true,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse new expression: `new Date()`, `new Map()`, `new Foo.Bar()`
    /// Also handles `new.target` meta property.
    ///
    /// The `new` keyword has the same precedence as unary operators.
    /// It takes a callee (identifier or member expression) and optional arguments.
    fn parse_new_expression(&mut self) -> Result<Expression<'arena>, ParseError> {
        let arena = self.arena;
        let (start, new_end) = self.current_pos();
        self.advance()?; // consume 'new'

        // Check for new.target meta property
        if *self.current_kind() == TokenKind::Dot {
            self.advance()?; // consume '.'
            if *self.current_kind() == TokenKind::Identifier && self.current_value() == "target" {
                let (prop_start, prop_end) = self.current_pos();
                self.advance()?; // consume 'target'
                return Ok(Expression::MetaProperty(MetaProperty {
                    meta: Identifier::simple(
                        self.intern("new"),
                        Span::new(start as u32, new_end as u32),
                    ),
                    property: Identifier::simple(
                        self.intern("target"),
                        Span::new(prop_start as u32, prop_end as u32),
                    ),
                    span: Span::new(start as u32, prop_end as u32),
                }));
            }
            return Err(self.error_expected_after("'target'", "new."));
        }

        // Parse the callee - identifier, member expr, function/class expression, or nested `new`
        // Check async function first (peek borrows self, can't be inside match)
        let is_async_function = *self.current_kind() == TokenKind::Keyword(KeywordKind::Async)
            && self.peek_kind() == TokenKind::Keyword(KeywordKind::Function);

        let callee_parsed = if is_async_function {
            // Async function expression: `new async function() {}`
            let (start, _) = self.current_pos();
            self.advance()?; // consume 'async'
            let expr = self.parse_async_function_expression(start)?;
            ParsedExpr::from_expr(expr)
        } else {
            match self.current_kind() {
                TokenKind::Keyword(KeywordKind::New) => {
                    let nested = self.parse_new_expression()?;
                    ParsedExpr::from_expr(nested)
                }
                TokenKind::Keyword(KeywordKind::Function) => {
                    // Function expression: `new function() {}`
                    let expr = self.parse_function_expression()?;
                    ParsedExpr::from_expr(expr)
                }
                TokenKind::Keyword(KeywordKind::Class) => {
                    // Class expression: `new class {}`
                    let expr = self.parse_class_expression()?;
                    ParsedExpr::from_expr(expr)
                }
                TokenKind::Keyword(KeywordKind::Import) => {
                    // `new import.meta()` — `import.meta` is a MetaProperty (a
                    // MemberExpression), a valid `new` callee. `new import(...)`
                    // is not: an `import(...)` ImportCall is a CallExpression, not
                    // a MemberExpression — acorn agrees ("Cannot use new with
                    // import()").
                    let (import_start, _) = self.current_pos();
                    let expr = self.parse_import_or_meta_property()?;
                    if matches!(expr, Expression::ImportExpression(_)) {
                        return Err(self.error_msg_at("Cannot use new with import()", import_start));
                    }
                    ParsedExpr::from_expr(expr)
                }
                _ => {
                    // We use primary + postfix parsing but stop before call expressions
                    self.parse_primary_expression_with_end()?
                }
            }
        };

        // Parse member access chains: new Foo.Bar.Baz()
        let mut callee = callee_parsed;
        loop {
            match self.current_kind() {
                TokenKind::Dot => {
                    self.advance()?; // consume '.'
                    // Keywords are valid property names: new Foo.class()
                    if !self.current_is_identifier_or_keyword() {
                        return Err(self.error_expected_after("property name", "."));
                    }
                    let (prop_start, prop_end) = self.current_pos();
                    let name = self.intern(self.current_property_name());
                    self.advance()?;

                    // actual_start covers a parenthesized callee's `(` (`new (a()).b`)
                    let span = Span::new(callee.actual_start as u32, prop_end as u32);
                    callee = ParsedExpr::with_end(
                        Expression::MemberExpression(MemberExpression {
                            object: arena.alloc(callee.expr),
                            property: arena.alloc(Expression::Identifier(Identifier::simple(
                                name,
                                Span::new(prop_start as u32, prop_end as u32),
                            ))),
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

                    let span = Span::new(callee.actual_start as u32, bracket_end as u32);
                    callee = ParsedExpr::with_end(
                        Expression::MemberExpression(MemberExpression {
                            object: arena.alloc(callee.expr),
                            property: arena.alloc(index),
                            computed: true,
                            optional: false,
                            span,
                        }),
                        bracket_end,
                    );
                }
                TokenKind::QuestionDot => {
                    // An optional chain can't be a `new` callee (per spec): `new a?.b()`
                    // is a syntax error. A parenthesized chain (`new (a?.b)()`) seals it
                    // and is valid — its `?.` is consumed inside the primary, never here.
                    return Err(self.error_msg(
                        "Optional chaining cannot appear in the callee of new expressions",
                    ));
                }
                TokenKind::Bang if !self.had_line_terminator => {
                    // Non-null assertion on the callee: `new (a?.b)!()`. The trailing `(`
                    // then binds as the `new` argument list, not a call on the callee.
                    callee = self.wrap_non_null_assertion(callee)?;
                }
                _ => break,
            }
        }

        // Parse optional type arguments: new Map<K, V>()
        let type_arguments = if self.check(&TokenKind::LessThan) && self.is_type_arguments_start() {
            Some(self.parse_type_parameter_instantiation()?)
        } else {
            None
        };

        // Parse optional arguments: new Date() vs new Date
        let (arguments, end): (&'arena [Expression<'arena>], u32) =
            if self.eat(TokenKind::ParenOpen) {
                let mut args = self.bvec();

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
                (args.into_bump_slice(), paren_end as u32)
            } else {
                // new Date without parens - valid JS; bare instantiation type args
                // (`new A<T>`) extend the span past the callee
                let end = type_arguments
                    .as_ref()
                    .map_or(callee.actual_end as u32, |ta| ta.span.end);
                (&[], end)
            };

        Ok(Expression::NewExpression(NewExpression {
            callee: arena.alloc(callee.expr),
            type_arguments,
            arguments,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse import expression or import.meta meta property
    ///
    /// Handles:
    /// - `import('module')` - dynamic import expression
    /// - `import.meta` - meta property
    fn parse_import_or_meta_property(&mut self) -> Result<Expression<'arena>, ParseError> {
        let arena = self.arena;
        let (start, import_end) = self.current_pos();
        self.advance()?; // consume 'import'

        // Check for import.meta meta property
        if *self.current_kind() == TokenKind::Dot {
            self.advance()?; // consume '.'
            if *self.current_kind() == TokenKind::Identifier && self.current_value() == "meta" {
                // `import.meta` is a Module-goal-only construct (early error:
                // "Syntax Error if the syntactic goal symbol is not Module").
                if self.goal != crate::Goal::Module {
                    return Err(self.error_msg("'import.meta' is only allowed in a module"));
                }
                let (prop_start, prop_end) = self.current_pos();
                self.advance()?; // consume 'meta'
                return Ok(Expression::MetaProperty(MetaProperty {
                    meta: Identifier::simple(
                        self.intern("import"),
                        Span::new(start as u32, import_end as u32),
                    ),
                    property: Identifier::simple(
                        self.intern("meta"),
                        Span::new(prop_start as u32, prop_end as u32),
                    ),
                    span: Span::new(start as u32, prop_end as u32),
                }));
            }
            return Err(self.error_expected_after("'meta'", "import."));
        }

        // Dynamic import: import('module') or import('module', options)
        self.expect(&TokenKind::ParenOpen)?;

        // The argument list is a grouping delimiter — `in` is always the binary
        // operator inside it, even within a for-header init (the args are
        // `AssignmentExpression[+In]`). Mirrors `parse_call_arguments`.
        self.grouping_depth += 1;

        // Parse the source expression (usually a string literal)
        let source = self.parse_assignment_expression()?;

        // The `ImportCall` grammar allows an optional trailing comma after the
        // source (1-arg form) and after the options (2-arg form):
        //   import ( AssignmentExpression ,opt )
        //   import ( AssignmentExpression , AssignmentExpression ,opt )
        // A bare trailing comma after the source leaves no options arg; a comma
        // followed by an expression is the options arg. 3+ args are not in the
        // grammar and still error at the `)` below — a deliberate divergence
        // from acorn-typescript, which accepts 3+ args and rejects the trailing
        // comma. See docs/conformance_svelte.md.
        let mut options: Option<&'arena Expression<'arena>> = None;
        if self.eat(TokenKind::Comma) && !self.check(&TokenKind::ParenClose) {
            options = Some(arena.alloc(self.parse_assignment_expression()?));
            self.eat(TokenKind::Comma); // optional trailing comma after the options
        }

        // Capture end position before consuming ')'
        let (_, paren_end) = self.current_pos();
        self.expect(&TokenKind::ParenClose)?;
        self.grouping_depth -= 1;

        Ok(Expression::ImportExpression(ImportExpression {
            source: arena.alloc(source),
            options,
            span: Span::new(start as u32, paren_end as u32),
        }))
    }

    /// Parse a block statement: `{ stmt1; stmt2; }`
    ///
    /// Parses the statements inside a block body (used for function bodies).
    /// Parse a function/arrow block body, marking its directive prologue.
    ///
    /// Function bodies (unlike arbitrary blocks) carry a directive prologue per
    /// acorn — see `adapt_directive_prologue`.
    pub(super) fn parse_function_body(&mut self) -> Result<BlockStatement<'arena>, ParseError> {
        let (mut body, span) = self.parse_block_body()?;
        // Mark the directive prologue on the owned buffer before freezing it.
        self.adapt_directive_prologue(&mut body);
        Ok(BlockStatement {
            body: body.into_bump_slice(),
            span,
        })
    }

    pub(super) fn parse_block_statement(&mut self) -> Result<BlockStatement<'arena>, ParseError> {
        let (body, span) = self.parse_block_body()?;
        Ok(BlockStatement {
            body: body.into_bump_slice(),
            span,
        })
    }

    /// Parse a `{ … }` block's statements into an arena buffer plus the block's
    /// span. Shared by `parse_block_statement` and `parse_function_body`; the
    /// latter mutates the buffer (directive prologue) before freezing it.
    fn parse_block_body(
        &mut self,
    ) -> Result<(bumpalo::collections::Vec<'arena, Statement<'arena>>, Span), ParseError> {
        let (start, _) = self.current_pos();
        self.expect(&TokenKind::BraceOpen)?; // consume '{'

        let mut body = self.bvec();

        // Parse statements until we hit '}'
        while !self.check(&TokenKind::BraceClose) {
            if self.check(&TokenKind::Eof) {
                return Err(self.error_msg("Unexpected end of file in block"));
            }

            let stmt = self.parse_statement()?;
            body.push(stmt);
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?; // consume '}'

        Ok((body, Span::new(start as u32, end as u32)))
    }

    /// Parse spread element: `...expr`
    fn parse_spread_element(&mut self) -> Result<Expression<'arena>, ParseError> {
        let (start, _) = self.current_pos();
        self.expect(&TokenKind::DotDotDot)?; // consume '...'

        // Use assignment_expression because comma separates array elements/object properties
        let argument = self.parse_assignment_expression()?;
        // Use prev_token_end() to include closing paren when argument is parenthesized
        let end = self.prev_token_end() as u32;

        Ok(Expression::SpreadElement(SpreadElement {
            argument: self.alloc(argument),
            span: Span::new(start as u32, end),
        }))
    }
}
