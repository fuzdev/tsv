// Expression parsing using Pratt parser for operator precedence

use crate::ast::internal::{
    ArrayExpression, ArrayPattern, ArrowFunctionBody, ArrowFunctionExpression,
    AssignmentExpression, AssignmentOperator, AssignmentPattern, AwaitExpression, BinaryExpression,
    BinaryOperator, BlockStatement, CallExpression, ConditionalExpression, Expression,
    FunctionExpression, Identifier, ImportExpression, Literal, LiteralValue, MemberExpression,
    MetaProperty, NewExpression, ObjectExpression, ObjectPattern, ObjectPatternProperty,
    ObjectProperty, Property, PropertyKind, RegexLiteral, RestElement, SequenceExpression,
    SpreadElement, Super, TSAsExpression, TSInstantiationExpression, TSNonNullExpression,
    TSSatisfiesExpression, TSTypeAssertion, TaggedTemplateExpression, TemplateElement,
    TemplateLiteral, UnaryExpression, UnaryOperator, UpdateExpression, UpdateOperator,
    YieldExpression,
};
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::Parser;
use super::scan::{
    is_identifier_start, parse_number_literal, skip_block_comment, skip_identifier,
    skip_line_comment, skip_string_literal, skip_whitespace, skip_whitespace_and_comments,
};

// =============================================================================
// Binding Power Constants for Pratt Parser
// =============================================================================
// Higher values = tighter binding (evaluated first)
// Left-associative: left_bp < right_bp
// Right-associative: left_bp > right_bp

/// Comma operator (sequence expression) - lowest precedence
const BP_COMMA: u8 = 0;
/// Assignment operators (=, +=, etc.) and ternary conditional
const BP_ASSIGNMENT: u8 = 1;
/// TypeScript `as` and `satisfies` operators
const BP_TS_TYPE_ASSERTION: u8 = 2;
/// Yield expression
const BP_YIELD: u8 = 3;
/// Unary operators (!, -, +, ~, typeof, void, delete, await, ++, --, new)
const BP_UNARY: u8 = 29;

// =============================================================================
// Expression-Specific Lookahead Helpers
// =============================================================================

/// Scan through parentheses and check if followed by `=>`
///
/// Assumes `pos` is at the opening `(`. Handles:
/// - Nested parentheses
/// - String literals inside parens
/// - Comments (line and block)
/// - Optional type annotation after `)`: `)` or `): type`
///
/// Returns `true` if the pattern `(...) =>` or `(...): type =>` is found.
fn scan_parens_then_arrow(bytes: &[u8], mut pos: usize) -> bool {
    if pos >= bytes.len() || bytes[pos] != b'(' {
        return false;
    }

    let mut depth = 0;
    while pos < bytes.len() {
        match bytes[pos] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return check_arrow_after_paren(bytes, pos + 1);
                }
            }
            b'"' | b'\'' => {
                pos = skip_string_literal(bytes, pos);
                continue; // Don't increment pos again
            }
            b'/' if pos + 1 < bytes.len() => {
                // Handle comments
                match bytes[pos + 1] {
                    b'/' => {
                        pos = skip_line_comment(bytes, pos);
                        continue; // Don't increment pos again
                    }
                    b'*' => {
                        pos = skip_block_comment(bytes, pos);
                        continue; // Don't increment pos again
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        pos += 1;
    }
    false
}

/// Check if `=>` follows (possibly with type annotation `: type`)
#[inline]
fn check_arrow_after_paren(bytes: &[u8], pos: usize) -> bool {
    let pos = skip_whitespace_and_comments(bytes, pos);
    // Check for => directly
    if pos + 1 < bytes.len() && bytes[pos] == b'=' && bytes[pos + 1] == b'>' {
        return true;
    }
    // Check for type annotation: ): type =>
    if pos < bytes.len() && bytes[pos] == b':' {
        return scan_for_arrow(bytes, pos);
    }
    false
}

/// Scan forward looking for `=>` (used after type annotations)
///
/// Properly handles:
/// - Statement boundaries: stops at `;` (not an arrow function)
/// - Nested structures: tracks depth for `()`, `[]`, `{}`, `<>` to find `=>` at depth 0
/// - Type function signatures: `(x: (a: number) => void): T => ...` correctly finds outer `=>`
fn scan_for_arrow(bytes: &[u8], mut pos: usize) -> bool {
    let mut paren_depth = 0;
    let mut bracket_depth = 0;
    let mut brace_depth = 0;
    let mut angle_depth = 0;

    while pos < bytes.len() {
        pos = skip_whitespace_and_comments(bytes, pos);
        if pos >= bytes.len() {
            break;
        }

        // Check if we're at the outermost nesting level (no open brackets/braces/parens/angles)
        let at_depth_zero =
            paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 && angle_depth == 0;

        match bytes[pos] {
            // Statement boundary - not an arrow function (only at depth 0)
            // Semicolons inside braces are valid separators in object type literals
            b';' if at_depth_zero => return false,

            // Track nesting depth
            b'(' => paren_depth += 1,
            b')' => {
                if paren_depth > 0 {
                    paren_depth -= 1;
                }
            }
            b'[' => bracket_depth += 1,
            b']' => {
                if bracket_depth > 0 {
                    bracket_depth -= 1;
                }
            }
            b'{' => brace_depth += 1,
            b'}' => {
                if brace_depth > 0 {
                    brace_depth -= 1;
                } else {
                    // Unbalanced brace - end of scope
                    return false;
                }
            }
            b'<' => angle_depth += 1,
            b'>' => {
                // Only decrement if not part of `=>`
                if pos > 0 && bytes[pos - 1] != b'=' && angle_depth > 0 {
                    angle_depth -= 1;
                }
            }

            // Check for `=>` at depth 0
            b'=' if pos + 1 < bytes.len() && bytes[pos + 1] == b'>' && at_depth_zero => {
                return true;
            }
            b'=' if pos + 1 < bytes.len() && bytes[pos + 1] == b'>' => {
                // Not at depth zero - skip past `=>` to avoid matching the `>` as angle close
                pos += 1;
            }

            // Skip string literals to avoid matching delimiters inside them
            b'"' | b'\'' | b'`' => {
                pos = skip_string_literal(bytes, pos);
                continue; // Don't increment pos again
            }

            _ => {}
        }
        pos += 1;
    }
    false
}

/// Check if position starts with an identifier followed by `=>`
///
/// Detects single-parameter arrow functions without parentheses: `x => expr`
/// Returns `true` if pattern `identifier =>` is found (with optional whitespace).
fn scan_identifier_then_arrow(bytes: &[u8], pos: usize) -> bool {
    // Skip the identifier (already validated by lexer as TokenKind::Identifier)
    let end = skip_identifier(bytes, pos);

    // Skip whitespace after identifier
    let pos = skip_whitespace(bytes, end);

    // Check for =>
    pos + 1 < bytes.len() && bytes[pos] == b'=' && bytes[pos + 1] == b'>'
}

/// Scan through angle brackets `<...>` for type parameters
///
/// Assumes `pos` is at `<`. Returns position after closing `>`, or 0 if not found.
/// Handles nested angle brackets, comments, and arrow functions in constraints: `<T extends () => void>`
fn scan_angle_brackets(bytes: &[u8], pos: usize) -> usize {
    if pos >= bytes.len() || bytes[pos] != b'<' {
        return 0;
    }

    let mut pos = pos + 1;
    let mut depth = 1;

    while pos < bytes.len() && depth > 0 {
        match bytes[pos] {
            b'<' => depth += 1,
            b'>' => {
                // Check if this is `=>` (arrow) rather than `>` (close angle)
                if pos > 0 && bytes[pos - 1] != b'=' {
                    depth -= 1;
                }
            }
            b'"' | b'\'' | b'`' => {
                pos = skip_string_literal(bytes, pos);
                continue; // Don't increment pos again
            }
            b'/' if pos + 1 < bytes.len() => {
                // Handle comments
                match bytes[pos + 1] {
                    b'/' => {
                        pos = skip_line_comment(bytes, pos);
                        continue;
                    }
                    b'*' => {
                        pos = skip_block_comment(bytes, pos);
                        continue;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        pos += 1;
    }

    if depth == 0 { pos } else { 0 }
}

/// Check if `(` at `pos` starts a function type (not a grouped expression).
///
/// Function type patterns:
/// - `(identifier:` or `(identifier?:` → parameter with type annotation
/// - `() =>` → no-params function type
///
/// Non-function patterns:
/// - `(expr)` → grouped expression
/// - `(a, b)` → tuple or call args (without type annotations)
fn is_function_type_start(bytes: &[u8], pos: usize) -> bool {
    if pos >= bytes.len() || bytes[pos] != b'(' {
        return false;
    }

    let after_paren = skip_whitespace(bytes, pos + 1);
    if after_paren >= bytes.len() {
        return false;
    }

    // `(identifier:` or `(identifier?:` → function type parameter
    if is_identifier_start(bytes[after_paren]) {
        let after_id = skip_whitespace(bytes, skip_identifier(bytes, after_paren));
        if after_id < bytes.len() && matches!(bytes[after_id], b':' | b'?') {
            return true;
        }
    }

    // `() =>` → no-params function type
    if bytes[after_paren] == b')' {
        let after_close = skip_whitespace(bytes, after_paren + 1);
        if after_close + 1 < bytes.len()
            && bytes[after_close] == b'='
            && bytes[after_close + 1] == b'>'
        {
            return true;
        }
    }

    false
}

/// Scan for closing `>` at angle depth 0, tracking all delimiter depths.
///
/// Used by `is_type_arguments_start` to verify that a sequence like `<T | U>`
/// or `<T, (x: number) => void>` is actually type arguments (finds matching `>`).
///
/// Returns `true` if a matching `>` is found before hitting an unbalanced
/// `)`, `]`, `}`, or `;` at depth 0.
///
/// Assumes scanning starts AFTER the initial `<` (i.e., angle_depth starts at 1).
fn scan_for_closing_angle_bracket(bytes: &[u8], mut pos: usize) -> bool {
    let mut angle_depth: i32 = 1;
    let mut paren_depth: i32 = 0;
    let mut bracket_depth: i32 = 0;
    let mut brace_depth: i32 = 0;

    while pos < bytes.len() {
        match bytes[pos] {
            b'<' => angle_depth += 1,
            b'>' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                angle_depth -= 1;
                if angle_depth == 0 {
                    return true;
                }
            }
            b'(' => paren_depth += 1,
            b')' => {
                paren_depth -= 1;
                if paren_depth < 0 {
                    return false; // Unbalanced - hit call/group end
                }
            }
            b'[' => bracket_depth += 1,
            b']' => {
                bracket_depth -= 1;
                if bracket_depth < 0 {
                    return false; // Unbalanced - hit array end
                }
            }
            b'{' => brace_depth += 1,
            b'}' => {
                brace_depth -= 1;
                if brace_depth < 0 {
                    return false; // Unbalanced - hit block end
                }
            }
            b';' => return false, // Statement end
            // Skip comments to avoid false matches on `>` inside them
            b'/' if pos + 1 < bytes.len() => match bytes[pos + 1] {
                b'/' => {
                    pos = skip_line_comment(bytes, pos);
                    continue;
                }
                b'*' => {
                    pos = skip_block_comment(bytes, pos);
                    continue;
                }
                _ => {}
            },
            // Skip string literals to avoid false matches on `>` inside them
            b'"' | b'\'' | b'`' => {
                pos = skip_string_literal(bytes, pos);
                continue;
            }
            _ => {}
        }
        pos += 1;
    }
    false
}

/// Extract content from template head: `content${ → "content"
#[inline]
fn extract_template_head_content(raw: &str) -> &str {
    if raw.len() >= 3 {
        &raw[1..raw.len() - 2]
    } else {
        ""
    }
}

/// Extract content from template tail: }content` → "content"
#[inline]
fn extract_template_tail_content(raw: &str) -> &str {
    if raw.len() >= 2 {
        &raw[1..raw.len() - 1]
    } else {
        ""
    }
}

/// Extract content from no-substitution template: `content` → "content"
#[inline]
fn extract_template_simple_content(raw: &str) -> &str {
    extract_template_tail_content(raw) // Same logic: strip first and last char
}

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
struct ParsedExpr {
    /// The parsed expression with semantic span (may exclude surrounding parens)
    expr: Expression,
    /// Actual start position before any opening parentheses
    actual_start: usize,
    /// Actual end position after consuming any closing parentheses
    actual_end: usize,
}

impl ParsedExpr {
    /// Create a ParsedExpr where actual_start/end match the expression's semantic span
    fn from_expr(expr: Expression) -> Self {
        let span = expr.span();
        Self {
            actual_start: span.start_usize(),
            actual_end: span.end_usize(),
            expr,
        }
    }

    /// Create a ParsedExpr with explicit actual_end (for parenthesized expressions)
    fn with_end(expr: Expression, actual_end: usize) -> Self {
        Self {
            actual_start: expr.span().start_usize(),
            actual_end,
            expr,
        }
    }

    /// Create a ParsedExpr with explicit actual_start and actual_end (for parenthesized expressions)
    fn with_bounds(expr: Expression, actual_start: usize, actual_end: usize) -> Self {
        Self {
            expr,
            actual_start,
            actual_end,
        }
    }
}

/// Infix operator info: binding powers and the corresponding binary operator.
///
/// Returns `(left_bp, right_bp, operator)` if the token is a binary operator.
/// - `left_bp < right_bp` for left-associative operators
/// - `left_bp > right_bp` for right-associative operators (e.g., `**`)
///
/// Uses standard JS operator precedence.
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
    pub(super) fn parse_assignment_expression(&mut self) -> Result<Expression, ParseError> {
        // Use BP_ASSIGNMENT to skip comma handling (which only triggers at BP_COMMA)
        Ok(self.parse_expression_bp(BP_ASSIGNMENT)?.expr)
    }

    /// Parse an expression without allowing `in` as a binary operator.
    ///
    /// Used in for-loop headers to distinguish `for (x in y)` from expressions.
    /// The `in` keyword is recognized as the for-in separator, not as a binary operator.
    pub(super) fn parse_expression_no_in(&mut self) -> Result<Expression, ParseError> {
        let old_allow_in = self.allow_in;
        self.allow_in = false;
        let result = self.parse_expression();
        self.allow_in = old_allow_in;
        result
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
            let kind = self.current_kind();

            // Check if this is an infix operator
            let Some((left_bp, right_bp, operator)) = infix_operator_info(kind) else {
                break;
            };

            // Skip `in` operator when allow_in is false (parsing for-loop headers)
            if matches!(operator, BinaryOperator::In) && !self.allow_in {
                break;
            }

            // Check if operator binds tighter than minimum
            if left_bp < min_bp {
                break;
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
                    left: Box::new(left.expr),
                    operator,
                    right: Box::new(right.expr),
                    span,
                }),
                actual_start: expr_start,
                actual_end: right.actual_end,
            };
        }

        // Handle TypeScript `as` and `satisfies` operators (after binary ops, before assignment)
        // These have lower precedence than binary operators but higher than assignment
        // Example: `a + b as T` parses as `(a + b) as T`
        // Only parse when allow_ts_type_assertions is true (disabled in Svelte template contexts)
        while min_bp <= BP_TS_TYPE_ASSERTION && self.allow_ts_type_assertions {
            match self.current_kind() {
                TokenKind::Keyword(KeywordKind::As) => {
                    self.advance()?; // consume 'as'
                    let type_annotation = Box::new(self.parse_type()?);
                    let span = Span::new(expr_start as u32, type_annotation.span().end);
                    left = ParsedExpr {
                        expr: Expression::TSAsExpression(TSAsExpression {
                            expression: Box::new(left.expr),
                            type_annotation,
                            span,
                        }),
                        actual_start: expr_start,
                        actual_end: span.end_usize(),
                    };
                }
                TokenKind::Keyword(KeywordKind::Satisfies) => {
                    self.advance()?; // consume 'satisfies'
                    let type_annotation = Box::new(self.parse_type()?);
                    let span = Span::new(expr_start as u32, type_annotation.span().end);
                    left = ParsedExpr {
                        expr: Expression::TSSatisfiesExpression(TSSatisfiesExpression {
                            expression: Box::new(left.expr),
                            type_annotation,
                            span,
                        }),
                        actual_start: expr_start,
                        actual_end: span.end_usize(),
                    };
                }
                _ => break,
            }
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
                    left: Box::new(left_pattern),
                    operator,
                    right: Box::new(right.expr),
                    span,
                }),
                actual_start: expr_start,
                actual_end: right.actual_end,
            };
        }

        // Handle ternary operator (lowest precedence among binary-like ops, above comma)
        // Handle at BP_ASSIGNMENT to include in assignment expressions but not in binary ops
        if min_bp <= BP_ASSIGNMENT && self.check(&TokenKind::Question) {
            self.advance()?; // consume '?'

            // Parse consequent (then branch) - use BP_ASSIGNMENT to exclude comma operator
            // This ensures (a ? b : c, d) parses as ((a ? b : c), d) not (a ? b : (c, d))
            let consequent = self.parse_expression_bp(BP_ASSIGNMENT)?;

            // Expect ':'
            self.expect(&TokenKind::Colon)?;

            // Parse alternate (else branch) - use BP_ASSIGNMENT to exclude comma operator
            let alternate = self.parse_expression_bp(BP_ASSIGNMENT)?;

            let span = Span::new(expr_start as u32, alternate.actual_end as u32);
            left = ParsedExpr {
                expr: Expression::ConditionalExpression(ConditionalExpression {
                    test: Box::new(left.expr),
                    consequent: Box::new(consequent.expr),
                    alternate: Box::new(alternate.expr),
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
            let mut expressions = vec![left.expr];
            let mut last_end = left.actual_end;

            while self.eat(TokenKind::Comma) {
                // Parse next expression - use BP_ASSIGNMENT to stop before next comma
                let next = self.parse_expression_bp(BP_ASSIGNMENT)?;
                expressions.push(next.expr);
                last_end = next.actual_end;
            }

            let span = Span::new(expr_start as u32, last_end as u32);
            left = ParsedExpr {
                expr: Expression::SequenceExpression(SequenceExpression { expressions, span }),
                actual_start: expr_start,
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
                let expr = self.parse_await_expression()?;
                ParsedExpr::from_expr(expr)
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
                } else if matches!(
                    peek,
                    TokenKind::ParenOpen | TokenKind::Identifier | TokenKind::LessThan
                ) {
                    // Async arrow function: `async () => ...` or `async x => ...` or `async <T>() => ...`
                    let (start, _) = self.current_pos();
                    self.advance()?; // consume 'async'
                    let expr = self.parse_async_arrow_function_after_async(start)?;
                    ParsedExpr::from_expr(expr)
                } else {
                    // `async` used as identifier (e.g., `[async]`, `async = 1`)
                    self.parse_primary_expression_with_end()?
                }
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

        // Parse any postfix operations (member access, call expressions)
        self.parse_postfix_expression(parsed)
    }

    /// Parse call arguments: `(arg1, arg2, ...)`
    ///
    /// Assumes the opening `(` has already been consumed.
    /// Returns the arguments and the end position of the closing `)`.
    pub(super) fn parse_call_arguments(&mut self) -> Result<(Vec<Expression>, usize), ParseError> {
        let mut arguments = Vec::new();

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
        Ok((arguments, paren_end))
    }

    /// Parse postfix expressions: member access (`.`, `[`), call expressions (`()`)
    ///
    /// Handles chained expressions like `obj.prop`, `arr[0]`, `foo()`, `obj.method().prop`
    fn parse_postfix_expression(&mut self, mut left: ParsedExpr) -> Result<ParsedExpr, ParseError> {
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
                            object: Box::new(left.expr),
                            property: Box::new(property),
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

                    match self.current_kind() {
                        TokenKind::Hash => {
                            // obj?.#private - optional private property access
                            let private_id = self.parse_private_identifier()?;
                            let prop_end = private_id.span.end_usize();

                            let span = Span::new(left.actual_start as u32, prop_end as u32);
                            left = ParsedExpr::with_end(
                                Expression::MemberExpression(MemberExpression {
                                    object: Box::new(left.expr),
                                    property: Box::new(Expression::PrivateIdentifier(private_id)),
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
                                    object: Box::new(left.expr),
                                    property: Box::new(Expression::Identifier(Identifier::simple(
                                        name,
                                        Span::new(prop_start as u32, prop_end as u32),
                                    ))),
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

                            let span = Span::new(left.actual_start as u32, bracket_end as u32);
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
                            let (arguments, paren_end) = self.parse_call_arguments()?;

                            let span = Span::new(left.actual_start as u32, paren_end as u32);
                            left = ParsedExpr::with_end(
                                Expression::CallExpression(CallExpression {
                                    callee: Box::new(left.expr),
                                    type_arguments: None,
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

                    let index = self.parse_expression()?;

                    let (_, bracket_end) = self.current_pos();
                    self.expect(&TokenKind::BracketClose)?; // consume ']'

                    let span = Span::new(left.actual_start as u32, bracket_end as u32);
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
                    let (arguments, paren_end) = self.parse_call_arguments()?;

                    let span = Span::new(left.actual_start as u32, paren_end as u32);
                    left = ParsedExpr::with_end(
                        Expression::CallExpression(CallExpression {
                            callee: Box::new(left.expr),
                            type_arguments: None,
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
                        let span = Span::new(left.actual_start as u32, quasi_span.end);
                        left = ParsedExpr::with_end(
                            Expression::TaggedTemplateExpression(TaggedTemplateExpression {
                                tag: Box::new(left.expr),
                                quasi: template,
                                span,
                            }),
                            quasi_span.end_usize(),
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
                            argument: Box::new(left.expr),
                            prefix: false,
                            span,
                        }),
                        op_end,
                    );
                }
                TokenKind::LessThan => {
                    // Might be TSInstantiationExpression: f<T>, expr<Type>
                    // Try to parse as type arguments - if successful, it's instantiation
                    // Otherwise, let binary expression handle it as comparison
                    if let Some(inst) = self.try_parse_instantiation_expression(&mut left)? {
                        left = inst;
                    } else {
                        break;
                    }
                }
                TokenKind::Bang if !self.had_line_terminator => {
                    // TypeScript non-null assertion: expr!
                    // ASI Rule: If there's a line terminator before !, ASI fires
                    // and the ! becomes a unary NOT operator on the next statement.
                    let (_, op_end) = self.current_pos();
                    self.advance()?;

                    let span = Span::new(left.actual_start as u32, op_end as u32);
                    left = ParsedExpr::with_end(
                        Expression::TSNonNullExpression(TSNonNullExpression {
                            expression: Box::new(left.expr),
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
                let raw = self.current_value();
                let number = parse_number_literal(raw)
                    .map_err(|_| self.error_msg_at(&format!("Invalid number: {raw}"), start))?;
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
                let (content, quote) = self.extract_string_literal();
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
            // Contextual keywords that can be identifiers
            | TokenKind::Keyword(KeywordKind::Async)
            | TokenKind::Keyword(KeywordKind::From)
            | TokenKind::Keyword(KeywordKind::As)
            | TokenKind::Keyword(KeywordKind::Satisfies) => {
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
        // Track actual_start BEFORE '(' and actual_end AFTER ')' for correct spans
        // when this expression is used as a callee: (a ? b : c)() should have
        // CallExpression span starting at '(', not at 'a'
        let (paren_start, _) = self.current_pos();
        self.expect(&TokenKind::ParenOpen)?; // consume '('

        let parsed = self.parse_expression_bp(BP_COMMA)?;

        // Capture the end position of ')' before consuming it
        let (_, paren_end) = self.current_pos();
        self.expect(&TokenKind::ParenClose)?; // consume ')'

        // Return expression with its original span (excluding parens), but with
        // actual_start before '(' and actual_end after ')' for containing expressions
        Ok(ParsedExpr::with_bounds(parsed.expr, paren_start, paren_end))
    }

    /// Check if current position starts an arrow function
    ///
    /// Scans ahead looking for pattern: `(` ... `)` `=>`
    fn is_arrow_function_start(&self) -> bool {
        scan_parens_then_arrow(self.source.as_bytes(), self.current_start)
    }

    /// Check if current position starts a single-param arrow function: `x =>`
    ///
    /// Scans ahead looking for pattern: `identifier` `=>`
    fn is_single_param_arrow_start(&self) -> bool {
        scan_identifier_then_arrow(self.source.as_bytes(), self.current_start)
    }

    /// Check if current position starts a generic arrow function: `<T>() =>`
    ///
    /// Scans ahead looking for pattern: `<` ... `>` `(` ... `)` `=>`
    fn is_generic_arrow_function_start(&self) -> bool {
        let bytes = self.source.as_bytes();
        let start = self.current_start;

        // Must start with '<'
        if start >= bytes.len() || bytes[start] != b'<' {
            return false;
        }

        // Scan through type parameters: <T, U extends V, ...>
        let pos = scan_angle_brackets(bytes, start);
        if pos == 0 {
            return false;
        }

        // After '>', check for `(...) =>`
        let pos = skip_whitespace(bytes, pos);
        scan_parens_then_arrow(bytes, pos)
    }

    /// Parse generic arrow function: `<T>() => ...`, `<T, U extends V>() => ...`
    fn parse_generic_arrow_function(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();

        // Parse type parameters: <T, U extends V, ...>
        let type_parameters = self.parse_type_parameters()?;

        // Capture paren position before parsing params
        let (params_start, _) = self.current_pos();

        // Parse parameter list
        let params = self.parse_parameter_list()?;

        // Check for return type annotation: <T>(): type => ... or type predicate
        let return_type = if self.check(&TokenKind::Colon) {
            Some(self.parse_return_type_annotation()?)
        } else {
            None
        };

        self.expect(&TokenKind::Arrow)?; // consume '=>'

        let body = self.parse_arrow_body()?;
        let end = body.span().end;

        Ok(Expression::ArrowFunctionExpression(
            ArrowFunctionExpression {
                type_parameters: Some(type_parameters),
                params,
                body,
                return_type,
                r#async: false,
                params_start: Some(params_start as u32),
                span: Span::new(start as u32, end),
            },
        ))
    }

    /// Parse a TypeScript angle-bracket type assertion: `<Type>expr`
    ///
    /// This is the old-style type assertion syntax. It's equivalent to `expr as Type`
    /// but doesn't work in JSX because it looks like an element.
    ///
    /// Example: `<string>value`, `<T>a`
    fn parse_type_assertion(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();

        // Parse <Type>
        self.expect(&TokenKind::LessThan)?; // consume '<'
        let type_annotation = self.parse_type()?;
        self.expect(&TokenKind::GreaterThan)?; // consume '>'

        // Parse the expression - use high binding power since type assertion is prefix
        let parsed = self.parse_expression_bp(BP_UNARY)?;
        let end = parsed.actual_end as u32;

        Ok(Expression::TSTypeAssertion(TSTypeAssertion {
            type_annotation: Box::new(type_annotation),
            expression: Box::new(parsed.expr),
            span: Span::new(start as u32, end),
        }))
    }

    /// Try to parse a TypeScript instantiation expression: `expr<Type>`
    ///
    /// When we see `<` after an expression, this could be:
    /// - Type arguments: `f<number>`, `arr<T, U>`
    /// - Comparison: `a < b`
    ///
    /// We use lookahead to detect type arguments by checking for type-like patterns.
    fn try_parse_instantiation_expression(
        &mut self,
        left: &mut ParsedExpr,
    ) -> Result<Option<ParsedExpr>, ParseError> {
        // Check if this looks like type arguments using lookahead
        if !self.is_type_arguments_start() {
            return Ok(None);
        }

        // Parse type parameter instantiation: <T, U>
        let type_args = self.parse_type_parameter_instantiation()?;
        let end = type_args.span.end;

        let inst = TSInstantiationExpression {
            expression: Box::new(std::mem::replace(
                &mut left.expr,
                Expression::Super(Super {
                    span: Span::new(0, 0),
                }),
            )),
            type_arguments: type_args,
            span: Span::new(left.actual_start as u32, end),
        };

        Ok(Some(ParsedExpr::with_end(
            Expression::TSInstantiationExpression(inst),
            end as usize,
        )))
    }

    /// Check if current position starts type arguments: `<Type, ...>`
    ///
    /// Uses lookahead to distinguish from comparison operator.
    /// Dispatches based on first token after `<`:
    /// - Type keywords: `<string>`, `<never>`, etc.
    /// - Identifiers: `<T>`, `<Ns.Type>`, `<T | U>`, `<T, U>`
    /// - Function types: `<(x: T) => R>`, `<() => R>`
    /// - Object/tuple/literal types: `<{ a: T }>`, `<[T, U]>`, `<"foo">`
    pub(super) fn is_type_arguments_start(&self) -> bool {
        let bytes = self.source.as_bytes();
        let start = self.current_start;

        // Must start with '<'
        if start >= bytes.len() || bytes[start] != b'<' {
            return false;
        }

        // Skip whitespace AND comments after '<' - comments can appear before types
        let pos = skip_whitespace_and_comments(bytes, start + 1);
        if pos >= bytes.len() {
            return false;
        }

        // Dispatch based on first token after '<'
        match bytes[pos] {
            // Type keywords: string, number, boolean, never, any, unknown, void, etc.
            _ if self.is_type_keyword_at(bytes, pos) => {
                // Exception: `this.` is member access, not type
                if bytes[pos..].starts_with(b"this") {
                    let after_this = skip_whitespace(bytes, pos + 4);
                    if after_this < bytes.len() && bytes[after_this] == b'.' {
                        return false;
                    }
                }
                true
            }

            // Identifier: type reference like `<T>` or `<Ns.Type>`
            _ if is_identifier_start(bytes[pos]) => {
                self.check_identifier_type_arg_pattern(bytes, pos)
            }

            // Function type: `<(x: T) => R>` or `<() => R>`
            b'(' => is_function_type_start(bytes, pos),

            // Object type `{`, tuple type `[`, string literal types
            b'{' | b'[' | b'\'' | b'"' => true,

            // Not a recognized type argument start
            _ => false,
        }
    }

    /// Check if identifier at `pos` is followed by valid type argument patterns.
    ///
    /// After scanning the full qualified name (e.g., `Ns.Type.Sub`), checks what follows:
    /// - `>` or `<`: definitely type args
    /// - `,`, `|`, `&`: scan for matching `>` to confirm type args
    /// - `[`: disambiguate indexed type vs array access
    /// - `extends`: type constraint
    fn check_identifier_type_arg_pattern(&self, bytes: &[u8], pos: usize) -> bool {
        // Skip identifier and any qualified parts (e.g., Namespace.Type.SubType)
        let mut pos = pos;
        loop {
            pos = skip_identifier(bytes, pos);
            pos = skip_whitespace_and_comments(bytes, pos);

            // If followed by '.', continue scanning qualified name
            if pos < bytes.len() && bytes[pos] == b'.' {
                pos += 1;
                pos = skip_whitespace_and_comments(bytes, pos);
                if pos < bytes.len() && is_identifier_start(bytes[pos]) {
                    continue;
                }
            }
            break;
        }

        if pos >= bytes.len() {
            return false;
        }

        match bytes[pos] {
            // Definitely type args
            b'>' | b'<' => true,

            // `||` and `&&` are logical operators, NOT type operators
            b'|' | b'&' if pos + 1 < bytes.len() && bytes[pos + 1] == bytes[pos] => false,

            // Union/intersection/comma: scan for matching `>` to confirm
            b',' | b'|' | b'&' => {
                scan_for_closing_angle_bracket(bytes, skip_whitespace_and_comments(bytes, pos + 1))
            }

            // Indexed type vs array access: `T[K]` vs `arr[0]`
            b'[' => self.check_indexed_type_pattern(bytes, pos),

            // Type constraint: `T extends U`
            b'e' if bytes[pos..].starts_with(b"extends") => true,

            _ => false,
        }
    }

    /// Check if `[` at `pos` starts an indexed type (not array access).
    ///
    /// - `arr[0]`: numeric index → array access
    /// - `arr[i]` followed by `<` or `;`: array access
    /// - `T[K]` followed by `>` or `,`: indexed type
    fn check_indexed_type_pattern(&self, bytes: &[u8], pos: usize) -> bool {
        let inside = skip_whitespace(bytes, pos + 1);
        if inside >= bytes.len() {
            return false;
        }

        // Numeric index is definitely array access
        if bytes[inside].is_ascii_digit() {
            return false;
        }

        // Identifier index: check what follows `]`
        if is_identifier_start(bytes[inside]) {
            let after_id = skip_identifier(bytes, inside);
            let after_bracket = skip_whitespace(bytes, after_id);
            if after_bracket < bytes.len() && bytes[after_bracket] == b']' {
                let after_close = skip_whitespace(bytes, after_bracket + 1);
                // Type args end with `>` or continue with `,`
                if after_close < bytes.len() && matches!(bytes[after_close], b'>' | b',') {
                    return true;
                }
                return false;
            }
        }

        // Other patterns (string keys, complex expressions) - assume type args
        true
    }

    /// Check if position points to a TypeScript type keyword
    fn is_type_keyword_at(&self, bytes: &[u8], pos: usize) -> bool {
        const TYPE_KEYWORDS: &[&[u8]] = &[
            b"never",
            b"string",
            b"number",
            b"boolean",
            b"any",
            b"unknown",
            b"void",
            b"null",
            b"undefined",
            b"symbol",
            b"bigint",
            b"object",
            b"this",
            b"true",
            b"false",
        ];

        for kw in TYPE_KEYWORDS {
            if pos + kw.len() <= bytes.len() && &bytes[pos..pos + kw.len()] == *kw {
                // Check it's not part of a longer identifier
                let next_pos = pos + kw.len();
                if next_pos >= bytes.len()
                    || (!bytes[next_pos].is_ascii_alphanumeric() && bytes[next_pos] != b'_')
                {
                    return true;
                }
            }
        }
        false
    }

    /// Parse object literal: `{ prop: value, ... }`
    ///
    /// Supports all JS/TypeScript object literal features:
    /// - Simple properties: `{ prop: value }`
    /// - Shorthand properties: `{ prop }` (key equals value)
    /// - Computed property names: `{ [expr]: value }`
    /// - Method shorthand: `{ foo() {} }`, `{ async foo() {} }`
    /// - Getter/setter: `{ get foo() {}, set foo(v) {} }`
    /// - Spread properties: `{ ...obj }`
    /// - String/number literal keys: `{ "key": value, 123: value }`
    /// - Trailing commas: `{ a: 1, }`
    /// - Empty objects: `{}`
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
                let prop_end = argument.span().end_usize();
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

            // Check for async method: `async foo() {}` or `async *gen() {}`
            // async is tokenized as a keyword, and is treated as method when followed by property name or *
            let is_async_method =
                if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Async))
                    && (self.peek_is_property_name() || self.peek_is(&TokenKind::Star))
                {
                    self.advance()?; // consume 'async'
                    true
                } else {
                    false
                };

            // Check for generator method: `*gen() {}` or `async *gen() {}`
            let is_generator = self.eat(TokenKind::Star);

            // Check for getter/setter: `get x() {}` or `set x(v) {}`
            // These are contextual keywords - only treated as get/set when followed by a property name
            // Note: async getters/setters and generator getters/setters are not valid in JS
            let accessor_kind = if !is_async_method
                && !is_generator
                && *self.current_kind() == TokenKind::Identifier
            {
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
            // Track if key is a restricted keyword (can't be used in shorthand)
            let (key, computed, is_restricted_keyword) = match self.current_kind() {
                // Computed property: { [expr]: value }
                TokenKind::BracketOpen => {
                    self.advance()?; // consume '['
                    let key_expr = self.parse_expression()?;
                    self.expect(&TokenKind::BracketClose)?; // consume ']'
                    (key_expr, true, false)
                }
                // Both identifiers and keywords can be property keys: { foo: 1, object: 2, in: 3 }
                TokenKind::Identifier => {
                    let (key_start, key_end) = self.current_pos();
                    let symbol = self.intern_identifier();
                    self.advance()?;
                    (
                        Expression::Identifier(Identifier::simple(
                            symbol,
                            Span::new(key_start as u32, key_end as u32),
                        )),
                        false,
                        false,
                    )
                }
                TokenKind::Keyword(kw) => {
                    let (key_start, key_end) = self.current_pos();
                    let symbol = self.intern_identifier();
                    // Track if this keyword cannot be used as identifier reference in shorthand
                    let restricted = matches!(
                        kw,
                        KeywordKind::Await | KeywordKind::Yield | KeywordKind::Let
                    );
                    self.advance()?;
                    (
                        Expression::Identifier(Identifier::simple(
                            symbol,
                            Span::new(key_start as u32, key_end as u32),
                        )),
                        false,
                        restricted,
                    )
                }
                TokenKind::String => {
                    // String literal key: {"prop-name": value}
                    let (key_start, key_end) = self.current_pos();
                    let (content, quote) = self.extract_string_literal();
                    self.advance()?;
                    (
                        Expression::Literal(Literal {
                            value: LiteralValue::String { content, quote },
                            span: Span::new(key_start as u32, key_end as u32),
                        }),
                        false,
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
                        false,
                    )
                }
                _ => {
                    return Err(self.error_expected_found_at("property key", prop_start));
                }
            };

            // Determine property kind, value, shorthand, and method flags
            let (kind, value, shorthand, method) = if let Some(accessor) = accessor_kind {
                // Getter/setter: `get x() {}` or `set x(v) {}`
                // Note: getters/setters cannot be async
                let func_expr = self.parse_method_body(prop_start as u32, false, false)?;
                (
                    accessor,
                    Expression::FunctionExpression(func_expr),
                    false,
                    false,
                )
            } else if self.check(&TokenKind::ParenOpen)
                || self.check(&TokenKind::LessThan)
                || is_async_method
                || is_generator
            {
                // Method shorthand: `{ foo() {} }`, `{ foo<T>() {} }`, `{ async foo() {} }`, or `{ *gen() {} }`
                let func_expr =
                    self.parse_method_body(key.span().start, is_async_method, is_generator)?;
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
                // Restricted keywords (await, yield, let) can't be used as shorthand identifiers
                if is_restricted_keyword {
                    return Err(self.error_msg_at(
                        "Cannot use restricted keyword as shorthand property",
                        key.span().start_usize(),
                    ));
                }
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
                // Restricted keywords (await, yield, let) can't be used as shorthand identifiers
                if is_restricted_keyword {
                    return Err(self.error_msg_at(
                        "Cannot use restricted keyword as shorthand property",
                        key.span().start_usize(),
                    ));
                }
                (PropertyKind::Init, key.clone(), true, false)
            };

            // Property span should end at value's end, not including trailing comments
            let prop_end = value.span().end_usize();
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
    /// Supports all JS/TypeScript array literal features:
    /// - All expression types as elements
    /// - Spread elements: `[...arr]`
    /// - Elision (holes/sparse arrays): `[, a]`, `[1,,3]`, `[, , a]`
    /// - Trailing commas: `[1, 2, 3,]`
    /// - Empty arrays: `[]`
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
        let parsed = self.parse_expression_bp(BP_UNARY)?;

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
        let parsed = self.parse_expression_bp(BP_UNARY)?;
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
        let parsed = self.parse_expression_bp(BP_UNARY)?;
        let end = parsed.actual_end as u32;

        Ok(Expression::AwaitExpression(AwaitExpression {
            argument: Box::new(parsed.expr),
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse yield expression: `yield`, `yield value`, or `yield* iterable`
    ///
    /// Yield expressions have low precedence (lowest in expressions).
    /// - `yield` with no argument yields undefined
    /// - `yield value` yields the given value
    /// - `yield* iterable` delegates to another generator/iterable
    fn parse_yield_expression(&mut self) -> Result<Expression, ParseError> {
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
            (Some(Box::new(parsed.expr)), end)
        } else if self.can_insert_semicolon() || matches!(self.current_kind(), TokenKind::Eof) {
            // No argument - yield with no value
            (None, yield_end as u32)
        } else if self.is_expression_start() {
            // Parse the argument
            let parsed = self.parse_expression_bp(BP_YIELD)?;
            let end = parsed.actual_end as u32;
            (Some(Box::new(parsed.expr)), end)
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
        matches!(
            self.current_kind(),
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
                | TokenKind::Keyword(KeywordKind::True)
                | TokenKind::Keyword(KeywordKind::False)
                | TokenKind::Keyword(KeywordKind::Null)
                | TokenKind::Keyword(KeywordKind::Undefined)
                | TokenKind::Keyword(KeywordKind::New)
                | TokenKind::Keyword(KeywordKind::Function)
                | TokenKind::Keyword(KeywordKind::Class)
                | TokenKind::Keyword(KeywordKind::Async)
                | TokenKind::Keyword(KeywordKind::Await)
                | TokenKind::Keyword(KeywordKind::Yield)
                | TokenKind::Keyword(KeywordKind::Typeof)
                | TokenKind::Keyword(KeywordKind::Delete)
                | TokenKind::Keyword(KeywordKind::Void)
                | TokenKind::Keyword(KeywordKind::Super)
                | TokenKind::Keyword(KeywordKind::Import)
        )
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
        let parsed = self.parse_expression_bp(BP_UNARY)?;

        let end = parsed.actual_end as u32;

        Ok(Expression::UpdateExpression(UpdateExpression {
            operator,
            argument: Box::new(parsed.expr),
            prefix: true,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse new expression: `new Date()`, `new Map()`, `new Foo.Bar()`
    /// Also handles `new.target` meta property.
    ///
    /// The `new` keyword has the same precedence as unary operators.
    /// It takes a callee (identifier or member expression) and optional arguments.
    fn parse_new_expression(&mut self) -> Result<Expression, ParseError> {
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

        // Parse the callee - this could be an identifier, member expression, or even nested `new`
        // Handle nested new: `new new Foo()()`
        let callee_parsed = if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::New)) {
            let nested = self.parse_new_expression()?;
            ParsedExpr::from_expr(nested)
        } else {
            // We use primary + postfix parsing but stop before call expressions
            self.parse_primary_expression_with_end()?
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

                    let span = Span::new(callee.expr.span().start, prop_end as u32);
                    callee = ParsedExpr::with_end(
                        Expression::MemberExpression(MemberExpression {
                            object: Box::new(callee.expr),
                            property: Box::new(Expression::Identifier(Identifier::simple(
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

        // Parse optional type arguments: new Map<K, V>()
        let type_arguments = if self.check(&TokenKind::LessThan) && self.is_type_arguments_start() {
            Some(self.parse_type_parameter_instantiation()?)
        } else {
            None
        };

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
            // new Date without parens - valid JS
            (Vec::new(), callee.actual_end as u32)
        };

        Ok(Expression::NewExpression(NewExpression {
            callee: Box::new(callee.expr),
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
    fn parse_import_or_meta_property(&mut self) -> Result<Expression, ParseError> {
        let (start, import_end) = self.current_pos();
        self.advance()?; // consume 'import'

        // Check for import.meta meta property
        if *self.current_kind() == TokenKind::Dot {
            self.advance()?; // consume '.'
            if *self.current_kind() == TokenKind::Identifier && self.current_value() == "meta" {
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

        // Parse the source expression (usually a string literal)
        let source = self.parse_assignment_expression()?;

        // Check for optional second argument (import options/attributes)
        let options = if self.check(&TokenKind::Comma) {
            self.advance()?; // consume ','
            Some(Box::new(self.parse_assignment_expression()?))
        } else {
            None
        };

        // Capture end position before consuming ')'
        let (_, paren_end) = self.current_pos();
        self.expect(&TokenKind::ParenClose)?;

        Ok(Expression::ImportExpression(ImportExpression {
            source: Box::new(source),
            options,
            span: Span::new(start as u32, paren_end as u32),
        }))
    }

    /// Parse arrow function body: expression or block statement
    fn parse_arrow_body(&mut self) -> Result<ArrowFunctionBody, ParseError> {
        if self.check(&TokenKind::BraceOpen) {
            let block = self.parse_block_statement()?;
            Ok(ArrowFunctionBody::BlockStatement(block))
        } else {
            // Use assignment_expression so comma doesn't consume next object property
            let expr = self.parse_assignment_expression()?;
            Ok(ArrowFunctionBody::Expression(Box::new(expr)))
        }
    }

    /// Parse arrow function with parentheses: `() => expr` or `(x, y) => expr` or `() => { ... }`
    ///
    /// Supports:
    /// - No parameters: `() => expr`
    /// - Single parameter: `(x) => expr`
    /// - Multiple parameters: `(x, y) => expr`
    /// - Destructuring parameters: `([a, b]) => ...`, `({x, y}) => ...`
    /// - Default values: `(a = 1) => ...`
    /// - Expression body: `() => expr`
    /// - Block body: `() => { ... }`
    ///
    /// Note: Single parameter without parens (`x => expr`) is handled by
    /// `parse_single_param_arrow_function()`.
    fn parse_arrow_function(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();

        // Capture paren position before parsing params
        let (params_start, _) = self.current_pos();

        // Parse parameter list (reuse shared method)
        let params = self.parse_parameter_list()?;

        // Check for return type annotation: (): type => ... or type predicate
        let return_type = if self.check(&TokenKind::Colon) {
            Some(self.parse_return_type_annotation()?)
        } else {
            None
        };

        self.expect(&TokenKind::Arrow)?; // consume '=>'

        let body = self.parse_arrow_body()?;
        let end = body.span().end;

        Ok(Expression::ArrowFunctionExpression(
            ArrowFunctionExpression {
                type_parameters: None, // Generic arrows like <T>() => {} are handled by parse_generic_arrow_function()
                params,
                body,
                return_type,
                r#async: false, // Non-async arrow function; async ones are parsed via parse_async_arrow_function
                params_start: Some(params_start as u32),
                span: Span::new(start as u32, end),
            },
        ))
    }

    /// Parse single-parameter arrow function without parentheses: `x => expr`
    fn parse_single_param_arrow_function(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();

        // Parse the single identifier parameter
        debug_assert!(matches!(self.current_kind(), TokenKind::Identifier));
        let (id_start, id_end) = self.current_pos();
        let symbol = self.intern_identifier();
        self.advance()?;

        let params = vec![Expression::Identifier(Identifier::simple(
            symbol,
            Span::new(id_start as u32, id_end as u32),
        ))];

        self.expect(&TokenKind::Arrow)?; // consume '=>'

        let body = self.parse_arrow_body()?;
        let end = body.span().end;

        Ok(Expression::ArrowFunctionExpression(
            ArrowFunctionExpression {
                type_parameters: None,
                params,
                body,
                return_type: None, // Single-param without parens can't have return type
                r#async: false,
                params_start: None, // No parens for single-param arrows
                span: Span::new(start as u32, end),
            },
        ))
    }

    /// Parse async arrow function after 'async' has been consumed: `() => ...`, `x => ...`, or `<T>() => ...`
    fn parse_async_arrow_function_after_async(
        &mut self,
        start: usize,
    ) -> Result<Expression, ParseError> {
        // Check for type parameters: `async <T>() => ...`
        let type_parameters = if self.check(&TokenKind::LessThan) {
            Some(self.parse_type_parameters()?)
        } else {
            None
        };

        // Parse parameter list or single parameter
        // Note: with type parameters, must have parentheses
        let (params, params_start) = if self.check(&TokenKind::ParenOpen) {
            let (paren_pos, _) = self.current_pos();
            (self.parse_parameter_list()?, Some(paren_pos as u32))
        } else if type_parameters.is_none() && matches!(self.current_kind(), TokenKind::Identifier)
        {
            // Single parameter without parens: `async x => ...`
            // (Not allowed with type parameters)
            let (id_start, id_end) = self.current_pos();
            let symbol = self.intern_identifier();
            self.advance()?;
            (
                vec![Expression::Identifier(Identifier::simple(
                    symbol,
                    Span::new(id_start as u32, id_end as u32),
                ))],
                None,
            )
        } else {
            return Err(self.error_expected_after("'(' or identifier", "async"));
        };

        // Check for return type annotation or type predicate
        let return_type = if self.check(&TokenKind::Colon) {
            Some(self.parse_return_type_annotation()?)
        } else {
            None
        };

        self.expect(&TokenKind::Arrow)?; // consume '=>'

        let body = self.parse_arrow_body()?;
        let end = body.span().end;

        Ok(Expression::ArrowFunctionExpression(
            ArrowFunctionExpression {
                type_parameters,
                params,
                body,
                return_type,
                r#async: true,
                params_start,
                span: Span::new(start as u32, end),
            },
        ))
    }

    /// Parse method body for method shorthand: `foo() { return 1; }`
    ///
    /// This parses the parameter list and block body for a method definition.
    /// The key has already been parsed by the caller.
    fn parse_method_body(
        &mut self,
        start: u32,
        is_async: bool,
        is_generator: bool,
    ) -> Result<FunctionExpression, ParseError> {
        // Parse optional type parameters: <T, U>
        let type_parameters = if self.check(&TokenKind::LessThan) {
            Some(self.parse_type_parameters()?)
        } else {
            None
        };

        // Capture paren position before parsing params (for comment detection)
        let (params_start, _) = self.current_pos();
        let params = self.parse_parameter_list()?;

        // Check for return type annotation: (): type or type predicate
        let return_type = if self.check(&TokenKind::Colon) {
            Some(self.parse_return_type_annotation()?)
        } else {
            None
        };

        let body = self.parse_block_statement()?;
        let end = body.span.end;

        Ok(FunctionExpression {
            id: None, // Method shorthand has no function name
            type_parameters,
            params,
            return_type,
            body,
            generator: is_generator,
            r#async: is_async,
            params_start: params_start as u32,
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
                return Err(self.error_msg("Unexpected end of file in block"));
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
                let raw = self.current_value();
                let content = extract_template_simple_content(raw).to_string();
                let cooked = self.current_decoded().map_or_else(
                    || Some(content.clone()),
                    |decoded| Some(decoded.to_string()),
                );

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
                let raw = self.current_value();
                let content = extract_template_head_content(raw).to_string();
                let cooked = self.current_decoded().map_or_else(
                    || Some(content.clone()),
                    |decoded| Some(decoded.to_string()),
                );

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
                        return Err(self.error_expected_found_at(
                            "'}' at end of template interpolation",
                            brace_start,
                        ));
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

                    match *self.current_kind() {
                        TokenKind::TemplateMiddle => {
                            // More interpolations to come: }content${
                            let content = extract_template_head_content(&raw).to_string();
                            let cooked = self.current_decoded().map_or_else(
                                || Some(content.clone()),
                                |decoded| Some(decoded.to_string()),
                            );

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
                            let content = extract_template_tail_content(&raw).to_string();
                            let cooked = self.current_decoded().map_or_else(
                                || Some(content.clone()),
                                |decoded| Some(decoded.to_string()),
                            );

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
                            return Err(
                                self.error_expected_found_at("template middle or tail", elem_start)
                            );
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
            _ => Err(self.error_expected_found_at("template literal", start)),
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
                    type_annotation: None,
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
                    type_annotation: None,
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
            _ => Err(self.error_msg_at("Invalid assignment target", expr.span().start_usize())),
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
