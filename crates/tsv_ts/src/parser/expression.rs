// Expression parsing using Pratt parser for operator precedence

use crate::ast::internal::{
    AssignmentExpression, AwaitExpression, BinaryExpression, BinaryOperator, BlockStatement,
    CallExpression, ConditionalExpression, Expression, IdentName, Identifier, ImportExpression,
    ImportPhase, JsdocCast, Literal, LiteralValue, MemberExpression, MetaProperty, NewExpression,
    ParenthesizedExpression, RegexLiteral, SequenceExpression, SpreadElement, Statement, Super,
    TSAsExpression, TSInstantiationExpression, TSNonNullExpression, TSSatisfiesExpression,
    TSTypeAssertion, TSTypeParameterInstantiation, TaggedTemplateExpression, TemplateLiteral,
    ThisExpression, UnaryExpression, UnaryOperator, UpdateExpression, UpdateOperator,
    YieldExpression,
};
use crate::lexer::{KeywordKind, TokenKind};
use crate::parser::expression_assignable::AssignableContext;
use tsv_lang::printing::visual_width;
use tsv_lang::{ParseError, Span, TAB_WIDTH};

use super::Parser;
use super::expression_lookahead::{matching_angle_close, scan_parens_then_arrow};
use super::scan::{parse_number_literal, skip_whitespace_and_comments};

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
/// TypeScript `as` / `satisfies` bind at RELATIONAL precedence — tsc's
/// `getBinaryOperatorPrecedence` returns `OperatorPrecedence.Relational` for both
/// (the same tier as `<` / `>` / `instanceof` / `in`), and acorn-typescript gates
/// them on `tt._in.binop` (relational) in its `parseExprOp` override. This equals
/// the relational `left_bp` in `infix_operator_info`, so `x === y as T` parses
/// `x === (y as T)` (tighter than equality / logical / bitwise / `??`) while
/// `a + b as T` stays `(a + b) as T` (looser than additive / shift /
/// multiplicative). Left-associative — `a as T as U` is `(a as T) as U`, and
/// `a < b as T` is `(a < b) as T` (breaks as the right operand of a same-tier
/// relational operator) — with the right-hand side consumed as a *type* by
/// `parse_type`, not an expression.
const BP_AS: u8 = 19;
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
#[derive(Debug, Clone, Copy)]
struct ParsedExpr<'arena> {
    /// The parsed expression with semantic span (may exclude surrounding parens).
    ///
    /// Arena-boxed (not held by value): the parser's recursion threads `ParsedExpr`
    /// up the precedence ladder, so keeping the fat `Expression` (160 B) inline made
    /// every `parse_*` return a ~176 B value by sret. The expression is allocated in
    /// the arena where it is built (where most callers already re-`alloc`'d it as a
    /// child anyway), so the recursion moves an 8 B reference instead. The format
    /// printer is unaffected: it reads the `Expression` enum, whose definition is
    /// unchanged — only this parser-internal wrapper holds a reference.
    expr: &'arena Expression<'arena>,
    /// Actual start position before any opening parentheses.
    ///
    /// `u32` (not `usize`): source positions fit in `u32` (the parser rejects
    /// inputs > 4 GB), and the narrower positions keep `ParsedExpr` minimal (one
    /// pointer + two `u32`) so a boxed error niche-packs into the `&Expression`'s
    /// spare space for free, leaving `Result<ParsedExpr, Box<ParseError>>`
    /// register-returnable instead of sret-returned — the recursion's hot return.
    actual_start: u32,
    /// Actual end position after consuming any closing parentheses
    actual_end: u32,
}

// `ParsedExpr` is the Pratt recursion's hot return: one `&Expression` reference plus two
// `u32` paren-bound positions (16 B on 64-bit). Boxing the error niche-packs it into the
// reference's spare space for free — the fallible `Result<ParsedExpr, Box<ParseError>>` is
// the *same size* as the success value (no error bloat) — so the recursion returns it in
// registers instead of via an sret stack slot. An unboxed 96 B `ParseError`, or `usize`
// positions, would push it over the register threshold. The asserts are width-relative (a
// `&Expression` is one `usize`), so they also hold for the 32-bit `wasm32` build.
const _: () =
    assert!(size_of::<ParsedExpr<'static>>() == size_of::<usize>() + 2 * size_of::<u32>());
const _: () = assert!(
    size_of::<Result<ParsedExpr<'static>, Box<ParseError>>>() == size_of::<ParsedExpr<'static>>()
);

impl<'arena> ParsedExpr<'arena> {
    /// Create a ParsedExpr where actual_start/end match the expression's semantic
    /// span. The expression is arena-boxed here (see the `expr` field doc).
    fn from_expr(arena: &'arena bumpalo::Bump, expr: Expression<'arena>) -> Self {
        let expr = arena.alloc(expr);
        let span = expr.span();
        Self {
            actual_start: span.start,
            actual_end: span.end,
            expr,
        }
    }

    /// Create a ParsedExpr with explicit start and end positions. Every caller
    /// already holds the semantic start (it just built the node's `span` from it),
    /// so it's passed directly rather than re-derived — `Expression::span()` is a
    /// wide match over every variant, and re-reading it through the fresh arena
    /// pointer on this hot expression-construction path is pure overhead. `actual_end`
    /// is a `usize` source position (narrowed to the `u32` field here, at the boundary).
    fn with_start_end(
        arena: &'arena bumpalo::Bump,
        expr: Expression<'arena>,
        actual_start: u32,
        actual_end: usize,
    ) -> Self {
        Self {
            expr: arena.alloc(expr),
            actual_start,
            actual_end: actual_end as u32,
        }
    }

    /// Create a ParsedExpr from an already-arena-boxed expression with explicit
    /// paren bounds. Unlike `from_expr`/`with_start_end`, the caller supplies the
    /// `&'arena Expression` — the inner's own allocation reused (a discarded
    /// grouping paren) or a freshly built wrapper node (a JSDoc cast, or a
    /// preserved `ParenthesizedExpression`).
    fn with_bounds(
        expr: &'arena Expression<'arena>,
        actual_start: usize,
        actual_end: usize,
    ) -> Self {
        Self {
            expr,
            actual_start: actual_start as u32,
            actual_end: actual_end as u32,
        }
    }

    /// Whether this is a leading *unparenthesized* expression that is itself a
    /// complete `AssignmentExpression` no subscript or operator may extend — a
    /// bare arrow function (always), or a bare `yield` expression when parsing
    /// inside a generator (`in_yield`). Both are top-level `AssignmentExpression`
    /// alternatives (ecma262 §13.15 — `ArrowFunction` / `YieldExpression`), not a
    /// `ConditionalExpression`, binary operand, or `LeftHandSideExpression`, so
    /// both `parse_prefix_expression` (call / member / postfix) and
    /// `parse_expression_bp` (binary / `as` / assignment / ternary) must stop when
    /// this holds. The check is span-anchored: a bare head's own span starts
    /// exactly at `actual_start`, but parenthesizing (`(() => {})` / `(yield)`)
    /// shifts `actual_start` to the outer `(` while the inner span starts later,
    /// so the two diverge — a parenthesized arrow/yield is a primary that CAN be
    /// an operand and is (correctly) not flagged.
    ///
    /// `yield` is only a `YieldExpression` in `[+Yield]`; outside a generator it
    /// is a (deferred) reserved-word identifier that prettier accepts as an
    /// operand, so the guard must not fire there — hence the `in_yield` gate. See
    /// the parser's `in_yield` field.
    fn is_bare_assignment_head(&self, in_yield: bool) -> bool {
        let is_head = matches!(self.expr, Expression::ArrowFunctionExpression(_))
            || (in_yield && matches!(self.expr, Expression::YieldExpression(_)));
        is_head && self.actual_start == self.expr.span().start
    }
}

/// How `parse_postfix_expression` should treat the subscript chain.
///
/// `ClassHeritage` is the `extends <expr>` clause — a `LeftHandSideExpression` that
/// deviates from a normal subscript chain in two ways: a bare `<T>` (type args NOT
/// followed by a call) stops the chain so the class can split it into
/// `super_type_parameters`, and a postfix `++`/`--` is left UNCONSUMED (the arm is
/// `Normal`-only), so `class C extends a++ {}` parses the heritage as `a` and the
/// stray `++` is rejected. In `Normal` mode a postfix `++`/`--` IS consumed as an
/// `UpdateExpression`, which then ends the chain — an update expression is not a
/// `LeftHandSideExpression`, so no further subscript can apply.
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
        // Nullish coalescing: same precedence as `||` (tsc's `Coalesce = LogicalOR`),
        // left-associative — so the (ungrammatical) `a ?? b || c` mix groups as
        // `(a ?? b) || c` like tsc/prettier, not `a ?? (b || c)`. Valid code never
        // mixes `??` with `||`/`&&` unparenthesized (the grammar forbids it), so this
        // shared level only decides error-recovery grouping.
        TokenKind::QuestionQuestion => Some((7, 8, BinaryOperator::QuestionQuestion)),
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
        // The spine returns an arena ref; the public boundary hands back an owned
        // `Expression` (shallow clone — children are refs) for by-value AST fields.
        Ok(self.parse_expression_bp(BP_COMMA)?.expr.clone())
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
        Ok(self.parse_expression_bp(BP_ASSIGNMENT)?.expr.clone())
    }

    /// Parse a computed property/member name `[ AssignmentExpression ]`, assuming the
    /// opening `[` has already been consumed and consuming the closing `]`.
    ///
    /// A computed key is a single `AssignmentExpression` (ecma262 `ComputedPropertyName`
    /// / `ClassElementName`), so a comma `SequenceExpression` (`[a, b]`, TS1171) and a
    /// spread (`[...a]`) are grammar violations acorn rejects at parse — tsv rejects too
    /// for drop-in parity. The comma falls out of `parse_assignment_expression` (which
    /// stops before `,`, then `expect(']')` fails on it); the spread parses as a primary
    /// expression tsv accepts in array/object/call positions, so it needs an explicit
    /// guard. A member-access *subscript* (`obj[a, b]`) is a full `Expression` and uses
    /// `parse_expression` directly — not this helper.
    pub(super) fn parse_computed_member_key(&mut self) -> Result<Expression<'arena>, ParseError> {
        if matches!(self.current_kind(), TokenKind::DotDotDot) {
            return Err(self.error_msg("A computed property name cannot be a spread element"));
        }
        let key = self.parse_assignment_expression()?;
        self.expect(&TokenKind::BracketClose)?;
        Ok(key)
    }

    /// `parse_expression`'s ref-returning sibling: hands back the spine's arena
    /// ref directly, for consumers whose AST field is `&'arena Expression` —
    /// skipping the owned boundary's shallow clone + re-alloc round trip.
    pub(super) fn parse_expression_ref(
        &mut self,
    ) -> Result<&'arena Expression<'arena>, ParseError> {
        Ok(self.parse_expression_bp(BP_COMMA)?.expr)
    }

    /// `parse_assignment_expression`'s ref-returning sibling (see
    /// `parse_expression_ref`).
    pub(super) fn parse_assignment_expression_ref(
        &mut self,
    ) -> Result<&'arena Expression<'arena>, ParseError> {
        Ok(self.parse_expression_bp(BP_ASSIGNMENT)?.expr)
    }

    /// Parse an expression without allowing `in` as a binary operator.
    ///
    /// Used in for-loop headers to distinguish `for (x in y)` from expressions.
    /// The `in` keyword is recognized as the for-in separator, not as a binary operator.
    pub(super) fn parse_expression_no_in(&mut self) -> Result<Expression<'arena>, ParseError> {
        self.with_context_flag(|p| &mut p.allow_in, false, Self::parse_expression)
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
        self.with_context_flag(|p| &mut p.allow_in, true, f)
    }

    /// Fold a trailing TypeScript `as` / `satisfies` type assertion at the current
    /// token into `left`, returning `true` if one was consumed (the infix loop should
    /// `continue`) or `false` if the current token isn't a type assertion to take here
    /// (the loop should stop).
    ///
    /// `as` / `satisfies` bind at the RELATIONAL tier (`BP_AS`), left-associative, with
    /// a *type* (via `parse_type`) on the right — so `x === y as T` is `x === (y as T)`
    /// while `a + b as T` stays `(a + b) as T`. Three conditions leave the keyword
    /// unconsumed (each returns `false`) — ASI on a preceding line break, the Svelte
    /// `#each` binding-separator gate, and relational precedence; see the guards.
    fn try_parse_type_assertion(
        &mut self,
        min_bp: u8,
        expr_start: usize,
        left: &mut ParsedExpr<'arena>,
    ) -> Result<bool, Box<ParseError>> {
        let is_as = match self.current_kind() {
            TokenKind::Keyword(KeywordKind::As) => true,
            TokenKind::Keyword(KeywordKind::Satisfies) => false,
            _ => return Ok(false),
        };
        // ASI: a preceding line terminator ends the expression, so the keyword starts a
        // fresh statement (tsc's `parseBinaryExpressionRest` / acorn-typescript both
        // break on `hasPrecedingLineBreak`). Unconditional — it holds inside grouping
        // too (`(a⏎as B)` can't close), so it precedes the `#each` gate.
        if self.had_line_terminator {
            return Ok(false);
        }
        // `as` is the Svelte `#each … as pattern` binding separator when type assertions
        // are disabled and we're outside grouping — leave it for the `#each` parser.
        // Inside grouping (`(x as T)`) it is always a type assertion.
        if !self.allow_ts_type_assertions && self.grouping_depth == 0 {
            return Ok(false);
        }
        // Relational binding power: looser than the caller's minimum (e.g. as the right
        // operand of a same-tier relational operator — `a < b as T` is `(a < b) as T`).
        if BP_AS < min_bp {
            return Ok(false);
        }

        let arena = self.arena;
        self.advance()?; // consume `as` / `satisfies`
        let type_annotation = arena.alloc(self.parse_type()?);
        let span = Span::new(expr_start as u32, type_annotation.span().end);
        let expr = if is_as {
            Expression::TSAsExpression(TSAsExpression {
                expression: left.expr,
                type_annotation,
                span,
            })
        } else {
            Expression::TSSatisfiesExpression(TSSatisfiesExpression {
                expression: left.expr,
                type_annotation,
                span,
            })
        };
        *left = ParsedExpr {
            expr: arena.alloc(expr),
            actual_start: expr_start as u32,
            actual_end: span.end,
        };
        Ok(true)
    }

    /// Pratt parser: parse expression with minimum binding power
    ///
    /// Returns ParsedExpr with actual end position tracking for parentheses
    fn parse_expression_bp(&mut self, min_bp: u8) -> Result<ParsedExpr<'arena>, Box<ParseError>> {
        let arena = self.arena;
        // Track the true start position (before any parentheses)
        // This is needed because grouped expressions like (a && b) should have their
        // containing binary expression span include the opening paren
        let expr_start = self.current_pos().0;

        // Parse prefix expression (primary or unary)
        let mut left = self.parse_prefix_expression()?;

        // A leading *unparenthesized* arrow function — or a bare `yield` inside a
        // generator — is a complete `AssignmentExpression` (see
        // `is_bare_assignment_head`), so no operator may extend it: a trailing
        // binary/logical operator, `as` / `satisfies` assertion, assignment target,
        // or ternary `?` is a syntax error (tsc TS1005 / prettier reject; acorn's
        // `parseExprOps` and `parseMaybeAssign`-above-`parseMaybeConditional`
        // enforce the same). Only a sequence `,` or a statement terminator may
        // follow. An operator that can *start* an expression is absorbed first — a
        // concise arrow body (`() => x || a`) or the yield argument
        // (`yield a || b`, `yield +c`) — so this fires only for a truly complete
        // head with no argument/body to absorb it.
        let leading_bare_head = left.is_bare_assignment_head(self.in_yield);

        // Parse infix binary operators and TypeScript `as` / `satisfies` type
        // assertions in one precedence-climbing loop. `as` / `satisfies` bind at
        // the RELATIONAL tier (`BP_AS`), left-associative, consuming a *type* on the
        // right — so `x === y as T` is `x === (y as T)` while `a + b as T` stays
        // `(a + b) as T`. (They were previously a second phase below every binary
        // operator, which mis-grouped `(x === y) as T`.)
        loop {
            // A leading bare arrow / bare yield takes no infix operator or `as` /
            // `satisfies` assertion — it is already a complete assignment expression.
            if leading_bare_head {
                break;
            }
            let kind = self.current_kind();

            let Some((left_bp, right_bp, operator)) = infix_operator_info(kind) else {
                // Not a binary operator — the only remaining infix forms are the
                // TypeScript `as` / `satisfies` type assertions.
                if self.try_parse_type_assertion(min_bp, expr_start, &mut left)? {
                    continue;
                }
                break;
            };

            // Skip `in` operator when allow_in is false (parsing for-loop headers),
            // unless inside grouping delimiters where `in` is always a binary operator
            if matches!(operator, BinaryOperator::In) && !self.allow_in && self.grouping_depth == 0
            {
                break;
            }

            // Check if operator binds tighter than minimum
            if left_bp < min_bp {
                break;
            }

            // ES2016+: Unary expression as left operand of ** without parens is a syntax error
            // `-2 ** 3` is ambiguous - must be `(-2) ** 3` or `-(2 ** 3)`. An `await`
            // operand is a UnaryExpression per the grammar (`await UnaryExpression`), so
            // `await b ** 2` is the same error — must be `(await b) ** 2` or `await (b ** 2)`.
            // Detect unparenthesized unary: actual_start equals expression span start
            if operator == BinaryOperator::StarStar
                && matches!(
                    left.expr,
                    Expression::UnaryExpression(_) | Expression::AwaitExpression(_)
                )
                && left.actual_start == left.expr.span().start
            {
                return Err(Box::new(ParseError::InvalidSyntax {
                    message: "Unary expression cannot be the left operand of ** without parentheses. Use (-x) ** y or -(x ** y).".to_string(),
                    position: left.expr.span().start as usize,
                    context: None,
                }));
            }

            self.advance()?; // consume operator

            // Parse right-hand side with right binding power
            let right = self.parse_expression_bp(right_bp)?;

            // Create binary expression
            // Use expr_start (which includes any opening paren) instead of left.actual_start
            // Use right.actual_end (position after parsing) to include closing parens
            let span = Span::new(expr_start as u32, right.actual_end);
            left = ParsedExpr {
                expr: arena.alloc(Expression::BinaryExpression(BinaryExpression {
                    left: left.expr,
                    operator,
                    right: right.expr,
                    span,
                })),
                actual_start: expr_start as u32,
                actual_end: right.actual_end,
            };
        }

        // Handle assignment operator (after binary ops, before ternary)
        // Assignment is right-associative and has low precedence
        // Check for simple `=` and compound assignment operators (+=, -=, etc.)
        // A leading bare arrow / bare yield is not a `LeftHandSideExpression`, so it
        // cannot be an assignment target (`() => {} = a` / `yield = a` are syntax
        // errors).
        if !leading_bare_head
            && min_bp <= BP_ASSIGNMENT
            && let Some(operator) = self.try_assignment_operator()
        {
            self.advance()?; // consume assignment operator

            // Parse right-hand side (assignment is right-associative, so same precedence)
            let right = self.parse_expression_bp(BP_ASSIGNMENT)?;

            // Convert left side to pattern if needed (cover grammar). Assignment is the
            // one context that accepts a type-assertion target (`(x as T) = …`).
            // `to_assignable` consumes by value; clone the arena ref (shallow — children
            // are refs) on this cold assignment-target path.
            let left_pattern =
                self.to_assignable(left.expr.clone(), AssignableContext::Assignment)?;

            let span = Span::new(expr_start as u32, right.actual_end);
            left = ParsedExpr {
                expr: arena.alloc(Expression::AssignmentExpression(AssignmentExpression {
                    left: arena.alloc(left_pattern),
                    operator,
                    right: right.expr,
                    span,
                })),
                actual_start: expr_start as u32,
                actual_end: right.actual_end,
            };
        }

        // Handle ternary operator (lowest precedence among binary-like ops, above comma)
        // Handle at BP_ASSIGNMENT to include in assignment expressions but not in binary ops
        // A leading bare arrow / bare yield cannot be a `ConditionalExpression`
        // test — the ternary's test is a `ShortCircuitExpression`, which never
        // derives `ArrowFunction` / `YieldExpression`, so `() => {} ? b : c` and
        // `yield ? b : c` are syntax errors. (Both parsers reject the yield form;
        // for the arrow form acorn over-leniently accepts it — a cataloged
        // `tsv_rejects` divergence, see conformance_svelte.md.)
        if !leading_bare_head && min_bp <= BP_ASSIGNMENT && self.eat(TokenKind::Question) {
            // Parse consequent (then branch) - use BP_ASSIGNMENT to exclude comma operator
            // This ensures (a ? b : c, d) parses as ((a ? b : c), d) not (a ? b : (c, d))
            // The consequent is `AssignmentExpression[+In]` — `in` is always the
            // binary operator here, even inside a for-header init. The alternate
            // is `[?In]` and inherits the outer context (so `for (a ? b : x in y;;)`
            // still rejects).
            // `with_allow_in` threads the unboxed `ParseError`; unbox the spine's
            // boxed error inside the closure (cold path) and let the outer `?` re-box.
            let consequent = self.with_allow_in(|p| {
                p.parse_expression_bp(BP_ASSIGNMENT)
                    .map_err(ParseError::from)
            })?;

            // Expect ':'
            self.expect(&TokenKind::Colon)?;

            // Parse alternate (else branch) - use BP_ASSIGNMENT to exclude comma operator
            let alternate = self.parse_expression_bp(BP_ASSIGNMENT)?;

            let span = Span::new(expr_start as u32, alternate.actual_end);
            left = ParsedExpr {
                expr: arena.alloc(Expression::ConditionalExpression(ConditionalExpression {
                    test: left.expr,
                    consequent: consequent.expr,
                    alternate: alternate.expr,
                    span,
                })),
                actual_start: expr_start as u32,
                actual_end: alternate.actual_end,
            };
        }

        // Handle comma operator (lowest precedence, after ternary)
        // Only handle at top level (BP_COMMA) to avoid conflicts with comma in
        // function calls, array literals, and object literals
        if min_bp == BP_COMMA && self.check(&TokenKind::Comma) {
            let mut expressions = self.bvec();
            // SequenceExpression stores its elements by value; clone the arena refs
            // (shallow) into the slice. Sequence expressions are rare.
            expressions.push(left.expr.clone());
            let mut last_end = left.actual_end;

            while self.eat(TokenKind::Comma) {
                // Parse next expression - use BP_ASSIGNMENT to stop before next comma
                let next = self.parse_expression_bp(BP_ASSIGNMENT)?;
                expressions.push(next.expr.clone());
                last_end = next.actual_end;
            }

            let span = Span::new(expr_start as u32, last_end);
            left = ParsedExpr {
                expr: arena.alloc(Expression::SequenceExpression(SequenceExpression {
                    expressions: expressions.into_bump_slice(),
                    span,
                })),
                actual_start: expr_start as u32,
                actual_end: last_end,
            };
        }

        Ok(left)
    }

    /// Parse prefix expression returning ParsedExpr with actual end position
    fn parse_prefix_expression(&mut self) -> Result<ParsedExpr<'arena>, Box<ParseError>> {
        let parsed = match self.current_kind() {
            TokenKind::Minus | TokenKind::Plus | TokenKind::Bang | TokenKind::Tilde => {
                let expr = self.parse_unary_expression()?;
                ParsedExpr::from_expr(self.arena, expr)
            }
            TokenKind::PlusPlus | TokenKind::MinusMinus => {
                let expr = self.parse_prefix_update_expression()?;
                ParsedExpr::from_expr(self.arena, expr)
            }
            TokenKind::Keyword(KeywordKind::New) => {
                let expr = self.parse_new_expression()?;
                ParsedExpr::from_expr(self.arena, expr)
            }
            TokenKind::Keyword(KeywordKind::Import) => {
                // Could be: import('module') or import.meta
                let expr = self.parse_import_or_meta_property()?;
                ParsedExpr::from_expr(self.arena, expr)
            }
            TokenKind::Keyword(KeywordKind::Typeof | KeywordKind::Void | KeywordKind::Delete) => {
                let expr = self.parse_unary_keyword_expression()?;
                ParsedExpr::from_expr(self.arena, expr)
            }
            // A single unparenthesized arrow parameter may be a contextual keyword
            // that is a valid `BindingIdentifier` (`any => x`, `async => x`,
            // `from => x`). The byte scan requires `=>` *immediately* after the word,
            // so `async x => …` / `async () => …` (a real async arrow) fall through to
            // the `async` arm, and non-binding reserved words (`yield =>`, `await =>`
            // at Module) are excluded by `can_be_binding_name` and reject downstream.
            // This mirrors the plain-`Identifier` arrow check below; `await` at Script
            // `[~Await]` keeps its dedicated handling in the `Await` arm.
            TokenKind::Keyword(kw)
                if kw.can_be_binding_name() && self.is_single_param_arrow_start() =>
            {
                let expr = self.parse_single_param_arrow_function()?;
                ParsedExpr::from_expr(self.arena, expr)
            }
            TokenKind::Keyword(KeywordKind::Await) => {
                if self.in_await {
                    // `[+Await]` context: a real await expression.
                    let expr = self.parse_await_expression()?;
                    ParsedExpr::from_expr(self.arena, expr)
                } else if self.await_is_identifier() {
                    if self.is_single_param_arrow_start() {
                        // Script `[~Await]`: `await => …` is an arrow whose single
                        // `BindingIdentifier` parameter is `await`.
                        ParsedExpr::from_expr(self.arena, self.parse_single_param_arrow_function()?)
                    } else {
                        // Script `[~Await]`: `await` is an ordinary `IdentifierReference`.
                        self.parse_await_identifier_reference()?
                    }
                } else {
                    // Module `[~Await]`: `await` is reserved and there is no
                    // `[+Await]` to make it an await expression.
                    return Err(Box::new(self.error_msg(
                        "'await' is only allowed inside an async function or at the top level of a module",
                    )));
                }
            }
            TokenKind::Keyword(KeywordKind::Yield) => {
                let expr = self.parse_yield_expression()?;
                ParsedExpr::from_expr(self.arena, expr)
            }
            TokenKind::Keyword(KeywordKind::Async) => {
                // Could be async arrow function, async function expression, or just identifier
                // Look ahead to determine what follows 'async'
                let peek = self.peek_kind();
                if peek == TokenKind::Keyword(KeywordKind::Function)
                    && !self.peek_preceded_by_line_terminator()
                {
                    // Async function expression: `async function() {}` — `function` must be on the
                    // same line (ECMAScript `async [no LineTerminator here] function`); a line break
                    // demotes `async` to an ordinary identifier expression, so `const x = async⏎
                    // function () {}` is a syntax error (a bodiless function declaration), matching
                    // acorn/prettier — not a silently-accepted async function expression.
                    let (start, _) = self.current_pos();
                    self.advance()?; // consume 'async'
                    let expr = self.parse_async_function_expression(start)?;
                    ParsedExpr::from_expr(self.arena, expr)
                } else if peek == TokenKind::ParenOpen {
                    // `async(...)` — could be async arrow or call to function named `async`
                    // Scan ahead: if `(...)` is followed by `=>`, it's an async arrow function
                    let paren_start = self.peek_start();
                    if scan_parens_then_arrow(self.source.as_bytes(), paren_start) {
                        let (start, _) = self.current_pos();
                        self.advance()?; // consume 'async'
                        let expr = self.parse_async_arrow_function_after_async(start)?;
                        ParsedExpr::from_expr(self.arena, expr)
                    } else {
                        // `async(2)` — call to function named `async`
                        self.parse_primary_expression()?
                    }
                } else if matches!(peek, TokenKind::Identifier | TokenKind::LessThan) {
                    // Async arrow function: `async x => ...` or `async <T>() => ...`
                    let (start, _) = self.current_pos();
                    self.advance()?; // consume 'async'
                    let expr = self.parse_async_arrow_function_after_async(start)?;
                    ParsedExpr::from_expr(self.arena, expr)
                } else {
                    // `async` used as identifier (e.g., `[async]`, `async = 1`)
                    self.parse_primary_expression()?
                }
            }
            TokenKind::At => {
                // Decorated class expression: `@dec class {}`. Decorators are
                // otherwise statement-only; in expression position the only thing
                // that may follow a decorator list is a class expression.
                let expr = self.parse_decorated_class_expression()?;
                ParsedExpr::from_expr(self.arena, expr)
            }
            TokenKind::Keyword(KeywordKind::Class) => {
                // Class expression: `class { }` or `class Foo<T> { }`
                let expr = self.parse_class_expression()?;
                ParsedExpr::from_expr(self.arena, expr)
            }
            TokenKind::Keyword(KeywordKind::Function) => {
                // Function expression: `function() {}` or `function name() {}`
                let expr = self.parse_function_expression()?;
                ParsedExpr::from_expr(self.arena, expr)
            }
            TokenKind::LessThan | TokenKind::LeftShift => {
                // Generic arrow function: `<T>() => ...` — never begins with a
                // `<<` shift token (its type-parameter list opens with a single
                // `<`), so a shift here can only open a type assertion whose
                // type is a generic function type: `<<T>() => R>x`.
                if self.current.kind == TokenKind::LessThan
                    && self.is_generic_arrow_function_start()
                {
                    let expr = self.parse_generic_arrow_function()?;
                    ParsedExpr::from_expr(self.arena, expr)
                } else {
                    // TypeScript type assertion: `<T>expr`
                    let expr = self.parse_type_assertion()?;
                    ParsedExpr::from_expr(self.arena, expr)
                }
            }
            TokenKind::Identifier => {
                // Check for single-param arrow function: `x => expr`
                if self.is_single_param_arrow_start() {
                    let expr = self.parse_single_param_arrow_function()?;
                    ParsedExpr::from_expr(self.arena, expr)
                } else {
                    // Regular identifier
                    self.parse_primary_expression()?
                }
            }
            _ => self.parse_primary_expression()?,
        };

        // An unparenthesized arrow function — or a bare `yield` inside a generator
        // — is a complete AssignmentExpression, so no `.`-member subscript can
        // follow (`() => {}()` is invalid JS; `yield.a` is too — the `.` doesn't
        // extend the head). A parenthesized head (`(() => {})()` / `(yield).a`,
        // detected by the span gap) is a callable primary. A subscript token that
        // *starts* an expression is instead absorbed earlier as the yield argument
        // (`yield [x]` is `yield` of an array; `yield (e)` its parenthesized arg),
        // so only the no-argument bare head reaches here.
        if parsed.is_bare_assignment_head(self.in_yield) {
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
                // A leading `...` is a SpreadElement (ecma262 ArgumentList); everywhere
                // else falls through to a plain AssignmentExpression.
                let arg = self.parse_spread_or_assignment_element()?;
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

    /// Parse the expression part of a decorator (the `expr` in `@expr`), per the
    /// ES decorators grammar (TC39 Stage 3, which `tsv` supports):
    ///
    /// - `@(Expression)` — a parenthesized **full** expression, or
    /// - `@Ident(.Ident | .#private)*` — a `DecoratorMemberExpression`: an
    ///   identifier reference followed by a `.`-member chain whose properties
    ///   are identifier names or private names (`DecoratorMemberExpression .
    ///   PrivateIdentifier` in the TC39 grammar),
    ///
    /// each optionally followed by a **single** trailing call `(...)`.
    ///
    /// Deliberately narrower than a full `AssignmentExpression`: binary
    /// operators, computed/optional member access (`[…]`, `?.`), tagged
    /// templates, `!`, and `++`/`--` are not part of the grammar. Leaving them
    /// unconsumed is what lets the construct *after* the decorator parse — e.g.
    /// the `*` of a decorated generator method (`@fn *a() {}`), which a
    /// full-expression parse would otherwise swallow as multiplication. A full
    /// expression must be parenthesized (`@(a + b)`).
    /// Mirrors `@sveltejs/acorn-typescript`'s `parseDecorator`, except the
    /// `.#private` chain step, which acorn-typescript rejects (a lag behind the
    /// TC39 grammar — see `docs/conformance_svelte.md` §TypeScript Corrections).
    pub(in crate::parser) fn parse_decorator_expression(
        &mut self,
    ) -> Result<Expression<'arena>, ParseError> {
        let head_start = self.current_pos().0;
        let is_parenthesized = *self.current_kind() == TokenKind::ParenOpen;

        // `@(Expression)` rides the full expression grammar inside the parens:
        // `parse_primary_expression` handles the `(` via `parse_paren_expression`
        // (sequence, arrow, paren-stripping) exactly as a standalone `(expr)`.
        // Otherwise the head must be an identifier reference — the same set
        // `parse_primary_expression` already accepts as an `Identifier` (plain
        // names plus value-position contextual keywords like `async`/`as`, and
        // `await` at `Script` goal), never `this`/`super`/a literal/`new`.
        let mut expr = self.parse_primary_expression()?;
        if !is_parenthesized {
            if !matches!(expr.expr, Expression::Identifier(_)) {
                // `parse_primary_expression` has already consumed the head, so
                // anchor the error at its start, not the now-current token.
                return Err(ParseError::InvalidSyntax {
                    message: "Expected an identifier or '(' as the decorator expression"
                        .to_string(),
                    position: head_start,
                    context: None,
                });
            }

            // `DecoratorMemberExpression`: a `.`-member chain. Property names are
            // liberal (keywords allowed: `@a.class`) and private names are part
            // of the grammar (`@C.#p`); computed `[…]` and optional `?.` access
            // are not.
            while *self.current_kind() == TokenKind::Dot {
                self.advance()?; // consume '.'
                let (property, prop_end) = self.parse_dot_property()?;
                let span = Span::new(expr.actual_start, prop_end as u32);
                expr = ParsedExpr::with_start_end(
                    self.arena,
                    Expression::MemberExpression(MemberExpression {
                        object: expr.expr,
                        property: self.arena.alloc(property),
                        computed: false,
                        optional: false,
                        span,
                    }),
                    expr.actual_start,
                    prop_end,
                );
            }
        }

        // A single optional trailing call (`@foo()`, `@a.b.c()`, `@(expr)()`).
        // Only one: `parseMaybeDecoratorArguments` runs once, so `@foo()()` and
        // `@foo().bar` are not decorators — a parenthesized form covers them.
        if *self.current_kind() == TokenKind::ParenOpen {
            self.advance()?; // consume '('
            let (arguments, paren_end) = self.parse_call_arguments()?;
            let arguments = arguments.into_bump_slice();
            // The call node starts at the callee's own start (acorn's
            // `startNodeAtNode(expr)`), not at any stripped wrapping paren.
            let start = expr.expr.span().start;
            let span = Span::new(start, paren_end as u32);
            expr = ParsedExpr::with_start_end(
                self.arena,
                Expression::CallExpression(CallExpression {
                    callee: expr.expr,
                    type_arguments: None,
                    arguments,
                    optional: false,
                    span,
                }),
                start,
                paren_end,
            );
        }

        // Hand back an owned `Expression` (shallow clone — children are arena
        // refs), matching `parse_assignment_expression`'s by-value boundary.
        Ok(expr.expr.clone())
    }

    /// Wrap `expr` in a `TSNonNullExpression`, consuming the current `!` token. The
    /// span starts at `expr.actual_start` so it covers any grouping parens (`(a?.b)!`),
    /// which `TSNonNullExpression::seals_optional_chain` relies on. Callers gate on
    /// `Bang` + `!had_line_terminator` (ASI: a line terminator before `!` makes it a
    /// prefix `!` on the next statement instead).
    fn wrap_non_null_assertion(
        &mut self,
        expr: ParsedExpr<'arena>,
    ) -> Result<ParsedExpr<'arena>, Box<ParseError>> {
        let (_, op_end) = self.current_pos();
        self.advance()?; // consume '!'
        let span = Span::new(expr.actual_start, op_end as u32);
        Ok(ParsedExpr::with_start_end(
            self.arena,
            Expression::TSNonNullExpression(TSNonNullExpression {
                expression: expr.expr,
                span,
            }),
            expr.actual_start,
            op_end,
        ))
    }

    /// Parse the property after a consumed `.` or `?.`: a private name
    /// (`obj.#p`) or an identifier/keyword name — keywords are valid property
    /// names (`obj.class`, `obj.if`, `obj.default()`). Returns the property
    /// expression and its end position; errors when neither follows.
    fn parse_dot_property(&mut self) -> Result<(Expression<'arena>, usize), ParseError> {
        if *self.current_kind() == TokenKind::Hash {
            let private_id = self.parse_private_identifier()?;
            let end = private_id.span.end_usize();
            Ok((Expression::PrivateIdentifier(private_id), end))
        } else if self.current_is_identifier_or_keyword() {
            let (prop_start, prop_end) = self.current_pos();
            // Property names are span-identity but decode `\u` escapes (`x.a` →
            // name `a`; ecma262 IdentifierName StringValue) — acorn parity.
            let name = self.current_ident_name();
            self.advance()?;
            Ok((
                Expression::Identifier(Identifier::simple(
                    name,
                    Span::new(prop_start as u32, prop_end as u32),
                )),
                prop_end,
            ))
        } else {
            Err(self.error_expected_after("property name", "."))
        }
    }

    /// Parse postfix expressions: member access (`.`, `[`), call expressions (`()`)
    ///
    /// Handles chained expressions like `obj.prop`, `arr[0]`, `foo()`, `obj.method().prop`
    fn parse_postfix_expression(
        &mut self,
        mut left: ParsedExpr<'arena>,
        mode: SubscriptMode,
    ) -> Result<ParsedExpr<'arena>, Box<ParseError>> {
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
                    let (property, prop_end) = self.parse_dot_property()?;

                    let span = Span::new(left.actual_start, prop_end as u32);
                    left = ParsedExpr::with_start_end(
                        self.arena,
                        Expression::MemberExpression(MemberExpression {
                            object: left.expr,
                            property: arena.alloc(property),
                            computed: false,
                            optional: false,
                            span,
                        }),
                        left.actual_start,
                        prop_end,
                    );
                }
                TokenKind::QuestionDot => {
                    // Optional chaining: obj?.prop, obj?.#private, obj?.[expr], obj?.()
                    self.advance()?; // consume '?.'
                    optional_chained = true;

                    match self.current_kind() {
                        TokenKind::Hash | TokenKind::Identifier | TokenKind::Keyword(_) => {
                            // obj?.prop or obj?.#private - optional property access
                            let (property, prop_end) = self.parse_dot_property()?;

                            let span = Span::new(left.actual_start, prop_end as u32);
                            left = ParsedExpr::with_start_end(
                                self.arena,
                                Expression::MemberExpression(MemberExpression {
                                    object: left.expr,
                                    property: arena.alloc(property),
                                    computed: false,
                                    optional: true,
                                    span,
                                }),
                                left.actual_start,
                                prop_end,
                            );
                        }
                        TokenKind::BracketOpen => {
                            // obj?.[expr] - optional computed access
                            self.advance()?; // consume '['
                            self.grouping_depth += 1;

                            let index = self.parse_expression_ref()?;

                            let (_, bracket_end) = self.current_pos();
                            self.expect(&TokenKind::BracketClose)?; // consume ']'
                            self.grouping_depth -= 1;

                            let span = Span::new(left.actual_start, bracket_end as u32);
                            left = ParsedExpr::with_start_end(
                                self.arena,
                                Expression::MemberExpression(MemberExpression {
                                    object: left.expr,
                                    property: index,
                                    computed: true,
                                    optional: true,
                                    span,
                                }),
                                left.actual_start,
                                bracket_end,
                            );
                        }
                        TokenKind::ParenOpen => {
                            // obj?.() - optional call
                            self.advance()?; // consume '('
                            let (arguments, paren_end) = self.parse_call_arguments()?;
                            let arguments = arguments.into_bump_slice();

                            let span = Span::new(left.actual_start, paren_end as u32);
                            left = ParsedExpr::with_start_end(
                                self.arena,
                                Expression::CallExpression(CallExpression {
                                    callee: left.expr,
                                    type_arguments: None,
                                    arguments,
                                    optional: true,
                                    span,
                                }),
                                left.actual_start,
                                paren_end,
                            );
                        }
                        TokenKind::LessThan | TokenKind::LeftShift
                            if self.is_type_arguments_start() =>
                        {
                            // obj?.<T>(args) - optional call with explicit type arguments;
                            // only a call may follow (`a?.<T>` without `(` is a syntax error)
                            let type_args = self.parse_type_parameter_instantiation()?;
                            if *self.current_kind() != TokenKind::ParenOpen {
                                return Err(Box::new(self.error_expected_after(
                                    "'('",
                                    "type arguments in optional call",
                                )));
                            }
                            self.advance()?; // consume '('
                            let (arguments, paren_end) = self.parse_call_arguments()?;
                            let arguments = arguments.into_bump_slice();

                            let span = Span::new(left.actual_start, paren_end as u32);
                            left = ParsedExpr::with_start_end(
                                self.arena,
                                Expression::CallExpression(CallExpression {
                                    callee: left.expr,
                                    type_arguments: Some(type_args),
                                    arguments,
                                    optional: true,
                                    span,
                                }),
                                left.actual_start,
                                paren_end,
                            );
                        }
                        _ => {
                            return Err(Box::new(
                                self.error_expected_after("property name, '[', or '('", "?."),
                            ));
                        }
                    }
                }
                TokenKind::BracketOpen => {
                    // Computed member access: arr[0]
                    self.advance()?; // consume '['
                    self.grouping_depth += 1;

                    let index = self.parse_expression_ref()?;

                    let (_, bracket_end) = self.current_pos();
                    self.expect(&TokenKind::BracketClose)?; // consume ']'
                    self.grouping_depth -= 1;

                    let span = Span::new(left.actual_start, bracket_end as u32);
                    left = ParsedExpr::with_start_end(
                        self.arena,
                        Expression::MemberExpression(MemberExpression {
                            object: left.expr,
                            property: index,
                            computed: true,
                            optional: false,
                            span,
                        }),
                        left.actual_start,
                        bracket_end,
                    );
                }
                TokenKind::ParenOpen => {
                    // Call expression: foo()
                    self.advance()?; // consume '('
                    let (arguments, paren_end) = self.parse_call_arguments()?;
                    let arguments = arguments.into_bump_slice();

                    let span = Span::new(left.actual_start, paren_end as u32);
                    // When callee is TSInstantiationExpression (e.g., foo<T>), flatten:
                    // TSInstantiationExpression + CallExpression → CallExpression with typeArguments
                    let (callee, type_arguments) = match left.expr {
                        Expression::TSInstantiationExpression(inst) => {
                            (inst.expression, Some(inst.type_arguments.clone()))
                        }
                        other => (other, None),
                    };
                    left = ParsedExpr::with_start_end(
                        self.arena,
                        Expression::CallExpression(CallExpression {
                            callee,
                            type_arguments,
                            arguments,
                            optional: false,
                            span,
                        }),
                        left.actual_start,
                        paren_end,
                    );
                }
                TokenKind::NoSubstitutionTemplate | TokenKind::TemplateHead => {
                    // Tagged template expression: tag`content`
                    let quasi = self.parse_template_literal(true)?;
                    if let Expression::TemplateLiteral(template) = quasi {
                        // An optional chain can't be a template tag (per spec): `a?.b`x``
                        // is a syntax error. A parenthesized chain (`(a?.b)`x``) seals the
                        // chain (consumed as its own primary, so `optional_chained` is
                        // false here) and is valid.
                        if optional_chained {
                            return Err(Box::new(self.error_msg(
                                "Optional chaining cannot appear in the tag of tagged template expressions",
                            )));
                        }
                        // When tag is TSInstantiationExpression (e.g., tag<T>), flatten:
                        // TSInstantiationExpression + TaggedTemplate → TaggedTemplate with typeArguments
                        let (tag, type_arguments) = match left.expr {
                            Expression::TSInstantiationExpression(inst) => {
                                (inst.expression, Some(inst.type_arguments.clone()))
                            }
                            other => (other, None),
                        };
                        left = self.build_tagged_template(
                            tag,
                            left.actual_start,
                            type_arguments,
                            template,
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

                    let span = Span::new(left.actual_start, op_end as u32);
                    left = ParsedExpr::with_start_end(
                        self.arena,
                        Expression::UpdateExpression(UpdateExpression {
                            operator,
                            argument: left.expr,
                            prefix: false,
                            span,
                        }),
                        left.actual_start,
                        op_end,
                    );
                    // A postfix `++`/`--` yields an UpdateExpression, which is not a
                    // `LeftHandSideExpression` — no further member/call subscript can
                    // apply (`a++[b]` / `a++.c` / `a++()` are grammar errors; acorn
                    // stops here). Break so a following `[`/`.`/`(` starts a new
                    // construct (e.g. the next member in a class body, across ASI).
                    break;
                }
                TokenKind::LessThan | TokenKind::LeftShift => {
                    // Might be TSInstantiationExpression: f<T>, expr<Type>
                    // (a `<<` shift token splits when its tail opens a generic
                    // function type: `f<<T>(v: T) => void>()`).
                    // If it parses as type arguments it's instantiation; otherwise
                    // let binary expression handle it as comparison/shift.
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
                        // An instantiation expression can't be followed by a property
                        // access (`f<T>.x`); a call or tagged template, which the loop
                        // below flattens into the expression, stays valid.
                        self.reject_instantiation_property_access()
                            .map_err(Box::new)?;
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
    ) -> Result<&'arena Expression<'arena>, ParseError> {
        let atom = self.parse_heritage_atom()?;
        let parsed = self.parse_postfix_expression(atom, SubscriptMode::ClassHeritage)?;
        Ok(parsed.expr)
    }

    /// Parse the primary atom of an `extends` clause (acorn's `parseExprAtom` with
    /// `canBeArrow = false`). `parse_primary_expression` covers identifiers,
    /// literals, parens, arrays, objects, templates, `this`/`super`, and regex; the
    /// keyword-led expression atoms (`new`, `class`, `function`, `async function`,
    /// `import`) sit above the primary layer and are dispatched here.
    fn parse_heritage_atom(&mut self) -> Result<ParsedExpr<'arena>, Box<ParseError>> {
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
                self.arena,
                self.parse_async_function_expression(start)?,
            ));
        }
        match self.current_kind() {
            TokenKind::Keyword(KeywordKind::New) => Ok(ParsedExpr::from_expr(
                self.arena,
                self.parse_new_expression()?,
            )),
            TokenKind::Keyword(KeywordKind::Class) => Ok(ParsedExpr::from_expr(
                self.arena,
                self.parse_class_expression()?,
            )),
            TokenKind::Keyword(KeywordKind::Function) => Ok(ParsedExpr::from_expr(
                self.arena,
                self.parse_function_expression()?,
            )),
            TokenKind::Keyword(KeywordKind::Import) => Ok(ParsedExpr::from_expr(
                self.arena,
                self.parse_import_or_meta_property()?,
            )),
            _ => {
                let parsed = self.parse_primary_expression()?;
                // Reject a top-level (unparenthesized) arrow: `class C extends (a) => b {}`.
                // acorn parses the heritage atom with `canBeArrow = false`, so an outer
                // arrow isn't a valid superclass. A *parenthesized* arrow
                // (`extends (a => b) {}`) is fine — there the arrow's own span starts
                // past the `(`, so it differs from `actual_start`. Same test the prefix
                // layer uses to seal parenthesized arrows.
                if matches!(parsed.expr, Expression::ArrowFunctionExpression(_))
                    && parsed.actual_start == parsed.expr.span().start
                {
                    return Err(Box::new(self.error_msg(
                        "Arrow functions cannot be used as a class heritage expression",
                    )));
                }
                Ok(parsed)
            }
        }
    }

    /// Check whether the current `<` (or `<<`) opens type arguments immediately
    /// followed by a call: `<T>(`. Used by the `ClassHeritage` subscript mode to
    /// tell a call's type arguments (`getMixin<T>(Base)`, consumed) from a bare
    /// superclass instantiation (`extends Base<T>`, left for the
    /// `super_type_parameters` split). `matching_angle_close` handles an `=>`
    /// arrow's `>` (`getMixin<() => void>(Base)`), comments, and strings; for a
    /// `<<` token, the second `<` re-raises the depth so the scan finds the
    /// outer close.
    fn is_type_args_followed_by_call(&self) -> bool {
        let bytes = self.source.as_bytes();
        let start = self.current.start as usize;

        // Must start with '<'
        if start >= bytes.len() || bytes[start] != b'<' {
            return false;
        }

        match matching_angle_close(bytes, start + 1) {
            None => false,
            Some(close) => {
                let after = skip_whitespace_and_comments(bytes, close + 1);
                after < bytes.len() && bytes[after] == b'('
            }
        }
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
    fn parse_primary_expression(&mut self) -> Result<ParsedExpr<'arena>, Box<ParseError>> {
        match self.current_kind() {
            TokenKind::Number => {
                let literal = self.parse_number_or_bigint_literal()?;
                let end = self.current_pos().1;
                let start = literal.span.start;
                self.advance()?;
                Ok(ParsedExpr::with_start_end(self.arena,
                    Expression::Literal(literal),
                    start,
                    end,
                ))
            }
            TokenKind::String => {
                let (start, end) = self.current_pos();
                let cooked = self.extract_string_cooked();
                self.advance()?;
                Ok(ParsedExpr::with_start_end(self.arena,
                    Expression::Literal(Literal {
                        value: LiteralValue::String(cooked),
                        span: Span::new(start as u32, end as u32),
                    }),
                    start as u32,
                    end,
                ))
            }
            TokenKind::Identifier => {
                let (start, end) = self.current_pos();
                let name = self.current_ident_name();
                self.advance()?;
                Ok(ParsedExpr::with_start_end(self.arena,
                    Expression::Identifier(Identifier::simple(
                        name,
                        Span::new(start as u32, end as u32),
                    )),
                    start as u32,
                    end,
                ))
            }
            // `await` as an ordinary `IdentifierReference` (Script `[~Await]`) —
            // e.g. a `new` callee (`new await()`) or any primary reference.
            TokenKind::Keyword(KeywordKind::Await) if self.await_is_identifier() => {
                self.parse_await_identifier_reference()
            }
            TokenKind::BraceOpen => Ok(ParsedExpr::from_expr(self.arena,self.parse_object_expression()?)),
            TokenKind::BracketOpen => Ok(ParsedExpr::from_expr(self.arena,self.parse_array_expression()?)),
            TokenKind::Keyword(KeywordKind::True) => {
                let (start, end) = self.current_pos();
                self.advance()?;
                Ok(ParsedExpr::with_start_end(self.arena,
                    Expression::Literal(Literal {
                        value: LiteralValue::Boolean(true),
                        span: Span::new(start as u32, end as u32),
                    }),
                    start as u32,
                    end,
                ))
            }
            TokenKind::Keyword(KeywordKind::False) => {
                let (start, end) = self.current_pos();
                self.advance()?;
                Ok(ParsedExpr::with_start_end(self.arena,
                    Expression::Literal(Literal {
                        value: LiteralValue::Boolean(false),
                        span: Span::new(start as u32, end as u32),
                    }),
                    start as u32,
                    end,
                ))
            }
            TokenKind::Keyword(KeywordKind::Null) => {
                let (start, end) = self.current_pos();
                self.advance()?;
                Ok(ParsedExpr::with_start_end(self.arena,
                    Expression::Literal(Literal {
                        value: LiteralValue::Null,
                        span: Span::new(start as u32, end as u32),
                    }),
                    start as u32,
                    end,
                ))
            }
            TokenKind::Keyword(KeywordKind::This) => {
                let (start, end) = self.current_pos();
                self.advance()?;
                Ok(ParsedExpr::with_start_end(self.arena,
                    Expression::ThisExpression(ThisExpression {
                        span: Span::new(start as u32, end as u32),
                    }),
                    start as u32,
                    end,
                ))
            }
            TokenKind::Keyword(KeywordKind::Super) => {
                let (start, end) = self.current_pos();
                self.advance()?;
                Ok(ParsedExpr::with_start_end(self.arena,
                    Expression::Super(Super {
                        span: Span::new(start as u32, end as u32),
                    }),
                    start as u32,
                    end,
                ))
            }
            TokenKind::ParenOpen => {
                // Could be: arrow function `() => ...` or parenthesized expression `(expr)`
                self.parse_paren_expression()
            }
            TokenKind::NoSubstitutionTemplate | TokenKind::TemplateHead => {
                // Untagged template literal (a primary expression): invalid escapes
                // are a syntax error (only tagged templates tolerate them).
                Ok(ParsedExpr::from_expr(self.arena,self.parse_template_literal(false)?))
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
                    self.peek.is_none(),
                    "regex relex with populated peek cache"
                );
                let lexer_start = self.current.start as usize;
                let (regex_token, pattern_end) = self.lexer.read_regex_literal(lexer_start)?;
                let lexer_end = regex_token.end as usize;

                // Pattern and flags are verbatim source slices (escapes preserved),
                // recovered from spans rather than owned strings. `pattern_end` is the
                // closing `/` (local): pattern is [slash+1, close), flags are
                // [close+1, token end). Spans are stored in host coordinates
                // (`span_pos` applies the `base_offset` shift).
                let pattern_span =
                    Span::new(self.span_pos(lexer_start + 1), self.span_pos(pattern_end));
                let flags_span =
                    Span::new(self.span_pos(pattern_end + 1), self.span_pos(lexer_end));
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

                Ok(ParsedExpr::with_start_end(self.arena,
                    Expression::RegexLiteral(RegexLiteral {
                        pattern_span,
                        flags_span,
                        pattern_width,
                        span: Span::new(self.span_pos(lexer_start), self.span_pos(lexer_end)),
                    }),
                    self.span_pos(lexer_start),
                    lexer_end + self.base_offset,
                ))
            }
            TokenKind::Hash => {
                // Private identifier as standalone expression (for brand check: #field in obj)
                let private_id = self.parse_private_identifier()?;
                let end = private_id.span.end_usize();
                let start = private_id.span.start;
                Ok(ParsedExpr::with_start_end(self.arena,
                    Expression::PrivateIdentifier(private_id),
                    start,
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
                let name = self.current_raw_ident_name();
                self.advance()?;
                Ok(ParsedExpr::with_start_end(self.arena,
                    Expression::Identifier(Identifier::simple(
                        name,
                        Span::new(start as u32, end as u32),
                    )),
                    start as u32,
                    end,
                ))
            }
            _ => Err(Box::new(ParseError::InvalidExpression {
                found: self.current_kind().to_string(),
                position: self.current_pos().0,
                context: None,
            })),
        }
    }

    /// Parse parenthesized expression or arrow function, returning ParsedExpr
    ///
    /// Distinguishes between:
    /// - Arrow function: `() => ...`, `(x) => ...`, `(x, y) => ...`
    /// - Grouped expression: `(expr)`
    ///
    /// Uses lookahead to detect arrow functions by scanning for `=>` after `)`.
    fn parse_paren_expression(&mut self) -> Result<ParsedExpr<'arena>, Box<ParseError>> {
        // Check if this looks like an arrow function by scanning ahead
        if self.is_arrow_function_start() {
            return Ok(ParsedExpr::from_expr(
                self.arena,
                self.parse_arrow_function()?,
            ));
        }

        // Parse as grouped expression: (expr)
        // Track actual_start BEFORE '(' and actual_end AFTER ')' for correct spans
        // when this expression is used as a callee: (a ? b : c)() should have
        // CallExpression span starting at '(', not at 'a'
        let (paren_start, _) = self.current_pos();
        let is_jsdoc_cast = self.paren_preceded_by_jsdoc_cast_comment(paren_start);
        self.expect(&TokenKind::ParenOpen)?; // consume '('

        self.grouping_depth += 1;
        let parsed = self.parse_expression_bp(BP_COMMA)?;

        // Capture the end position of ')' before consuming it
        let (_, paren_end) = self.current_pos();
        self.expect(&TokenKind::ParenClose)?; // consume ')'
        self.grouping_depth -= 1;

        // Grouping parens are normally discarded (the inner's own allocation flows
        // through, paren-free like acorn/Svelte). Two positions preserve them as an
        // explicit wrapper node spanning `(`…`)`, while keeping the paren-inclusive
        // `actual_start`/`actual_end` bounds so containing expressions still span the
        // parens:
        // - **JSDoc type cast** (`/** @type {T} */ (expr)`) — the parens are
        //   semantically required (dropping them drops the cast), so a preceding
        //   `@type`/`@satisfies` block comment wraps the inner in a `JsdocCast`. Cast
        //   semantics subsume grouping, so this wins when both apply.
        // - **Snippet-parameter sub-parse** (`preserve_parens`) — acorn's
        //   `preserveParens` without Svelte's `remove_parens`; wraps in a
        //   layout-transparent `ParenthesizedExpression` so only the wire shape moves.
        // Comments themselves are still located positionally in the flat
        // `Vec<Comment>` at print time.
        let paren_span = Span::new(paren_start as u32, paren_end as u32);
        let expr = if is_jsdoc_cast {
            self.alloc(Expression::JsdocCast(JsdocCast {
                inner: parsed.expr,
                span: paren_span,
            }))
        } else if self.preserve_parens {
            self.alloc(Expression::ParenthesizedExpression(
                ParenthesizedExpression {
                    expression: parsed.expr,
                    span: paren_span,
                },
            ))
        } else {
            parsed.expr
        };
        Ok(ParsedExpr::with_bounds(expr, paren_start, paren_end))
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
        // Resolve the comment through the lexer's spans rather than re-scanning the
        // source. A `/*` can appear inside this comment's own body *or* a preceding
        // line comment / string literal, and byte scanning can't tell those apart from
        // the real opener — mis-slicing the content would drop a real cast
        // (`/** z /* @type {T} */`) or fabricate one (`/* z /** @type {T} */`). The
        // comment ending here was just drained into `self.comments`; match it by its
        // host-coordinate end and test its exact (delimiter-excluded) content.
        let token_end = (i + self.base_offset) as u32;
        self.comments
            .iter()
            .rev()
            .find(|c| c.span.end == token_end)
            .is_some_and(|c| {
                c.is_block && {
                    let start = c.content_span.start as usize - self.base_offset;
                    let end = c.content_span.end as usize - self.base_offset;
                    is_jsdoc_type_cast_comment(&self.source[start..end])
                }
            })
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

        // Parse <Type> (`<<` splits when the assertion type is a generic
        // function type: `<<T>() => R>x`)
        self.expect_less_than_in_type()?; // consume '<'
        let type_annotation = self.parse_type()?;
        self.expect(&TokenKind::GreaterThan)?; // consume '>'

        // Parse the expression - use high binding power since type assertion is prefix
        let parsed = self.parse_expression_bp(BP_UNARY)?;

        // Reject an unparenthesized arrow operand (`<T>x => x`): the assertion
        // operand is a UnaryExpression, which an arrow is not — TypeScript
        // errors here. acorn-typescript instead backtracks and reads `<T>` as
        // the arrow's type parameters (a deliberate, cataloged divergence —
        // conformance_svelte.md §Type assertion vs. generic arrow). A
        // *parenthesized* arrow (`<T>(() => {})`) stays a valid operand — same
        // span-gap test the prefix layer uses to seal parenthesized arrows.
        if matches!(parsed.expr, Expression::ArrowFunctionExpression(_))
            && parsed.actual_start == parsed.expr.span().start
        {
            return Err(
                self.error_msg("An arrow function cannot be the operand of a type assertion")
            );
        }
        let end = parsed.actual_end;

        Ok(Expression::TSTypeAssertion(TSTypeAssertion {
            type_annotation: arena.alloc(type_annotation),
            expression: parsed.expr,
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
    ) -> Result<ParsedExpr<'arena>, Box<ParseError>> {
        // Parse type parameter instantiation: <T, U>
        let type_args = self.parse_type_parameter_instantiation()?;
        let end = type_args.span.end;

        let inst = TSInstantiationExpression {
            expression: left.expr,
            type_arguments: type_args,
            span: Span::new(left.actual_start, end),
        };

        Ok(ParsedExpr::with_start_end(
            self.arena,
            Expression::TSInstantiationExpression(inst),
            left.actual_start,
            end as usize,
        ))
    }

    /// Reject a property access immediately after a TypeScript instantiation
    /// expression. A `f<T>` (and a bare `new A<T>` — type arguments with no call)
    /// may not be followed by `.`/`?.` property access: acorn rejects `f<T>.x` /
    /// `f<T>?.x` / `f<T>?.[x]` ("Invalid property access after an instantiation
    /// expression"). Errors when the *current* token starts such an access — a `.`,
    /// or a `?.` whose next token is not `(` (an optional *call* `f<T>?.()` stays
    /// valid). A plain call (`f<T>()`) or tagged template (`` f<T>`x` ``) never
    /// reaches this check. Shared by the subscript-loop and `new`-expression paths.
    fn reject_instantiation_property_access(&mut self) -> Result<(), ParseError> {
        if matches!(self.current_kind(), TokenKind::Dot)
            || (matches!(self.current_kind(), TokenKind::QuestionDot)
                && !self.peek_is(&TokenKind::ParenOpen))
        {
            return Err(self.error_msg("Invalid property access after an instantiation expression"));
        }
        Ok(())
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
            // Private helper; the dispatcher routes only -/+/!/~ here.
            #[allow(clippy::unreachable)] // caller dispatches on the same token set
            _ => unreachable!("parse_unary_expression called with non-unary operator"),
        };
        self.advance()?;

        // Parse the operand with high binding power (unary is right-associative)
        // This allows chained unary: --x, -+x, !!x, ~~x etc.
        // and proper precedence: -a * b parses as (-a) * b
        let parsed = self.parse_expression_bp(BP_UNARY)?;

        // Use actual_end to include trailing parens in the span (matches Svelte's behavior)
        // For `!(a && b)`, the span should be from `!` to `)`, not just to `b`
        let end = parsed.actual_end;

        Ok(Expression::UnaryExpression(UnaryExpression {
            operator,
            argument: parsed.expr,
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
            // Private helper; the dispatcher routes only typeof/void/delete here.
            #[allow(clippy::unreachable)] // caller dispatches on the same token set
            _ => unreachable!("parse_unary_keyword_expression called with non-keyword operator"),
        };
        self.advance()?;

        // Parse the operand with high binding power (unary is right-associative)
        let parsed = self.parse_expression_bp(BP_UNARY)?;
        let end = parsed.actual_end;

        Ok(Expression::UnaryExpression(UnaryExpression {
            operator,
            argument: parsed.expr,
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
        let end = parsed.actual_end;

        Ok(Expression::AwaitExpression(AwaitExpression {
            argument: parsed.expr,
            span: Span::new(start as u32, end),
        }))
    }

    /// Consume the current `await` token as an ordinary `IdentifierReference`
    /// (Script `[~Await]`). The caller must have verified `await_is_identifier()`
    /// — used for a primary reference and a `new` callee (`new await()`).
    fn parse_await_identifier_reference(&mut self) -> Result<ParsedExpr<'arena>, Box<ParseError>> {
        let (start, end) = self.current_pos();
        let name = self.current_raw_ident_name();
        self.advance()?;
        Ok(ParsedExpr::with_start_end(
            self.arena,
            Expression::Identifier(Identifier::simple(
                name,
                Span::new(start as u32, end as u32),
            )),
            start as u32,
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
            let end = parsed.actual_end;
            (Some(parsed.expr), end)
        } else if self.can_insert_semicolon() || matches!(self.current_kind(), TokenKind::Eof) {
            // No argument - yield with no value
            (None, yield_end as u32)
        } else if self.is_expression_start() {
            // Parse the argument
            let parsed = self.parse_expression_bp(BP_YIELD)?;
            let end = parsed.actual_end;
            (Some(parsed.expr), end)
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
            // `/` / `/=` after `yield` is a regex-literal argument, not division:
            // the lexer emits `Slash`/`SlashEquals` by default (the parser re-lexes
            // it as a regex only in primary position), and a bare `yield` can't be
            // the left operand of division without parens, so `yield /a/g` is
            // `yield (/a/g)` (matching acorn). The primary parser resyncs the regex.
            | TokenKind::Slash
            | TokenKind::SlashEquals
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
            // Private helper; the dispatcher routes only ++/-- here.
            #[allow(clippy::unreachable)] // caller dispatches on the same token set
            _ => unreachable!("parse_prefix_update_expression called with non-update operator"),
        };
        self.advance()?;

        // Parse the operand - update expressions apply to member expressions or identifiers
        // Use high binding power so ++x.y parses correctly
        let parsed = self.parse_expression_bp(BP_UNARY)?;

        let end = parsed.actual_end;

        Ok(Expression::UpdateExpression(UpdateExpression {
            operator,
            argument: parsed.expr,
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
                // Both tokens are already consumed; their spans cover exactly
                // `new` / `target`, so the names are the verbatim spans.
                let meta_span = Span::new(start as u32, new_end as u32);
                let prop_span = Span::new(prop_start as u32, prop_end as u32);
                return Ok(Expression::MetaProperty(MetaProperty {
                    meta: Identifier::simple(IdentName::from_span(meta_span), meta_span),
                    property: Identifier::simple(IdentName::from_span(prop_span), prop_span),
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
            ParsedExpr::from_expr(self.arena, expr)
        } else {
            match self.current_kind() {
                TokenKind::Keyword(KeywordKind::New) => {
                    let nested = self.parse_new_expression()?;
                    ParsedExpr::from_expr(self.arena, nested)
                }
                TokenKind::Keyword(KeywordKind::Function) => {
                    // Function expression: `new function() {}`
                    let expr = self.parse_function_expression()?;
                    ParsedExpr::from_expr(self.arena, expr)
                }
                TokenKind::Keyword(KeywordKind::Class) => {
                    // Class expression: `new class {}`
                    let expr = self.parse_class_expression()?;
                    ParsedExpr::from_expr(self.arena, expr)
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
                    ParsedExpr::from_expr(self.arena, expr)
                }
                _ => {
                    // We use primary + postfix parsing but stop before call expressions
                    self.parse_primary_expression()?
                }
            }
        };

        // Parse member access chains: new Foo.Bar.Baz()
        let mut callee = callee_parsed;
        // `<T>` in the callee chain belongs to a trailing tagged template
        // (`new Foo<T>`x``, part of the callee) when a template follows — consumed
        // in the loop — otherwise it is the `new`'s own (`new Foo<T>()`) and this
        // holds it for the argument parse below.
        let mut type_arguments: Option<TSTypeParameterInstantiation<'arena>> = None;
        loop {
            match self.current_kind() {
                TokenKind::Dot => {
                    self.advance()?; // consume '.'
                    // Keywords are valid property names: new Foo.class()
                    if !self.current_is_identifier_or_keyword() {
                        return Err(self.error_expected_after("property name", "."));
                    }
                    let (prop_start, prop_end) = self.current_pos();
                    // Property names decode `\u` escapes (span-identity otherwise) — acorn parity.
                    let name = self.current_ident_name();
                    self.advance()?;

                    // actual_start covers a parenthesized callee's `(` (`new (a()).b`)
                    let span = Span::new(callee.actual_start, prop_end as u32);
                    callee = ParsedExpr::with_start_end(
                        self.arena,
                        Expression::MemberExpression(MemberExpression {
                            object: callee.expr,
                            property: arena.alloc(Expression::Identifier(Identifier::simple(
                                name,
                                Span::new(prop_start as u32, prop_end as u32),
                            ))),
                            computed: false,
                            optional: false,
                            span,
                        }),
                        callee.actual_start,
                        prop_end,
                    );
                }
                TokenKind::BracketOpen => {
                    self.advance()?; // consume '['
                    let index = self.parse_expression_ref()?;
                    let (_, bracket_end) = self.current_pos();
                    self.expect(&TokenKind::BracketClose)?;

                    let span = Span::new(callee.actual_start, bracket_end as u32);
                    callee = ParsedExpr::with_start_end(
                        self.arena,
                        Expression::MemberExpression(MemberExpression {
                            object: callee.expr,
                            property: index,
                            computed: true,
                            optional: false,
                            span,
                        }),
                        callee.actual_start,
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
                TokenKind::NoSubstitutionTemplate | TokenKind::TemplateHead => {
                    // A tagged template extends the callee itself: `new Foo`x`` is
                    // `new (Foo`x`)()`, not `(new Foo())`x``. Per the grammar, a
                    // `new` callee is an ordinary MemberExpression, whose production
                    // includes `MemberExpression TemplateLiteral` — so a trailing tag
                    // before any explicit `(...)` binds to the callee, not the `new`.
                    callee = self.attach_new_callee_tag(callee, None)?;
                }
                // `<T>` here is either the tag's type arguments (`new Foo<T>`x``,
                // where they flatten onto a TaggedTemplateExpression callee — acorn
                // parity) or the `new`'s own (`new Foo<T>()`). A following template
                // disambiguates: it consumes them into the tag and the callee chain
                // continues (`new Foo<T>`x`.bar`); otherwise they are the `new`'s and
                // the chain ends here.
                _ if self.check_less_than_in_type() && self.is_type_arguments_start() => {
                    let ta = self.parse_type_parameter_instantiation()?;
                    if matches!(
                        self.current_kind(),
                        TokenKind::NoSubstitutionTemplate | TokenKind::TemplateHead
                    ) {
                        callee = self.attach_new_callee_tag(callee, Some(ta))?;
                    } else {
                        type_arguments = Some(ta);
                        break;
                    }
                }
                _ => break,
            }
        }

        // Parse optional arguments: new Date() vs new Date. `new`'s argument list is an
        // `Arguments` grouping delimiter, so it shares `parse_call_arguments` (which
        // bumps `grouping_depth`) — `in` is a binary operator inside the arguments even
        // in a for-header init, matching call arguments (ecma262 `ArgumentList[+In]`).
        let (arguments, end): (&'arena [Expression<'arena>], u32) =
            if self.eat(TokenKind::ParenOpen) {
                let (args, paren_end) = self.parse_call_arguments()?;
                (args.into_bump_slice(), paren_end as u32)
            } else {
                // new Date without parens - valid JS; bare instantiation type args
                // (`new A<T>`) extend the span past the callee. A bare `new A<T>`
                // (type args, no call arguments) is instantiation-style: like `f<T>`
                // it can't be followed by a property access (`new A<T>.x` rejects);
                // `new A<T>()` took the `(` branch above, so its later `.prop` is an
                // ordinary member access.
                if type_arguments.is_some() {
                    self.reject_instantiation_property_access()?;
                }
                let end = type_arguments
                    .as_ref()
                    .map_or(callee.actual_end, |ta| ta.span.end);
                (&[], end)
            };

        Ok(Expression::NewExpression(NewExpression {
            callee: callee.expr,
            type_arguments,
            arguments,
            span: Span::new(start as u32, end),
        }))
    }

    /// Extend a `new` callee with the tagged template at the current token,
    /// carrying the tag's `<T>` in `type_arguments` when present (`new Foo<T>`x``).
    /// The `new`-callee member loop's two tag sites — with and without a preceding
    /// instantiation — share this. Returns `tag` unchanged if the parsed quasi is
    /// not a `TemplateLiteral` (unreachable: `parse_template_literal` yields only
    /// that).
    fn attach_new_callee_tag(
        &mut self,
        tag: ParsedExpr<'arena>,
        type_arguments: Option<TSTypeParameterInstantiation<'arena>>,
    ) -> Result<ParsedExpr<'arena>, ParseError> {
        let quasi = self.parse_template_literal(true)?;
        let Expression::TemplateLiteral(template) = quasi else {
            return Ok(tag);
        };
        Ok(self.build_tagged_template(tag.expr, tag.actual_start, type_arguments, template))
    }

    /// Build a `TaggedTemplateExpression` from an already-parsed tag, its optional
    /// `<T>` type arguments, and the parsed quasi. Shared by `attach_new_callee_tag`
    /// (the `new`-callee sites) and the ordinary postfix tagged-template arm so the
    /// node's span math — the semantic span and the paren bounds are both
    /// `tag_start..quasi.span.end` (a tag has no parens of its own) — lives in one
    /// place. `tag` is the resolved tag expression and `tag_start` its
    /// paren-inclusive start; the postfix arm flattens a `tag<T>`
    /// `TSInstantiationExpression` off the tag first, lifting `<T>` into
    /// `type_arguments`.
    fn build_tagged_template(
        &self,
        tag: &'arena Expression<'arena>,
        tag_start: u32,
        type_arguments: Option<TSTypeParameterInstantiation<'arena>>,
        quasi: TemplateLiteral<'arena>,
    ) -> ParsedExpr<'arena> {
        let end = quasi.span.end;
        ParsedExpr::with_start_end(
            self.arena,
            Expression::TaggedTemplateExpression(TaggedTemplateExpression {
                tag,
                type_arguments,
                quasi,
                span: Span::new(tag_start, end),
            }),
            tag_start,
            end as usize,
        )
    }

    /// Reject a spread element where an `ImportCall` argument (an
    /// `AssignmentExpression`) is expected — `import(...x)` / `import.source(...x)`
    /// are syntax errors (acorn agrees). An `import(...)` argument is a bare
    /// `AssignmentExpression`, not an `ArgumentList`, so a leading `...` is invalid;
    /// this guard reports it precisely (guarding both argument positions) rather than
    /// letting it fall through to the generic primary-expression error.
    fn reject_import_call_spread(&self) -> Result<(), ParseError> {
        if matches!(self.current_kind(), TokenKind::DotDotDot) {
            return Err(self.error_msg("Cannot use spread element in import() argument"));
        }
        Ok(())
    }

    /// Parse import expression or import.meta meta property
    ///
    /// Handles:
    /// - `import('module')` - dynamic import expression
    /// - `import.source('module')` / `import.defer('module')` - phased dynamic
    ///   import (Stage-3 source-phase-imports / import-defer proposals)
    /// - `import.meta` - meta property
    fn parse_import_or_meta_property(&mut self) -> Result<Expression<'arena>, ParseError> {
        let (start, import_end) = self.current_pos();
        self.advance()?; // consume 'import'

        // Check for `import.meta` / `import.source(…)` / `import.defer(…)`.
        let mut phase = ImportPhase::None;
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
                // Both tokens are already consumed; their spans cover exactly
                // `import` / `meta`, so the names are the verbatim spans.
                let meta_span = Span::new(start as u32, import_end as u32);
                let prop_span = Span::new(prop_start as u32, prop_end as u32);
                return Ok(Expression::MetaProperty(MetaProperty {
                    meta: Identifier::simple(IdentName::from_span(meta_span), meta_span),
                    property: Identifier::simple(IdentName::from_span(prop_span), prop_span),
                    span: Span::new(start as u32, prop_end as u32),
                }));
            }
            // `import.source(…)` / `import.defer(…)` — the import-phase proposals.
            // The phase keyword must be immediately followed by the `(` call: a bare
            // `import.source` or a member access `import.source.x` is a syntax error
            // (enforced by the `ParenOpen` expect below).
            if *self.current_kind() == TokenKind::Identifier {
                match self.current_value() {
                    "source" => phase = ImportPhase::Source,
                    "defer" => phase = ImportPhase::Defer,
                    _ => return Err(self.error_expected_after("'meta'", "import.")),
                }
                self.advance()?; // consume 'source' / 'defer'
            } else {
                return Err(self.error_expected_after("'meta'", "import."));
            }
        }

        // Dynamic import: import('module') or import('module', options), possibly
        // phased as import.source(...) / import.defer(...).
        self.expect(&TokenKind::ParenOpen)?;

        // The argument list is a grouping delimiter — `in` is always the binary
        // operator inside it, even within a for-header init (the args are
        // `AssignmentExpression[+In]`). Mirrors `parse_call_arguments`.
        self.grouping_depth += 1;

        self.reject_import_call_spread()?;

        // Parse the source expression (usually a string literal)
        let source = self.parse_assignment_expression_ref()?;

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
            self.reject_import_call_spread()?; // the options arg is likewise no spread
            options = Some(self.parse_assignment_expression_ref()?);
            self.eat(TokenKind::Comma); // optional trailing comma after the options
        }

        // Capture end position before consuming ')'
        let (_, paren_end) = self.current_pos();
        self.expect(&TokenKind::ParenClose)?;
        self.grouping_depth -= 1;

        Ok(Expression::ImportExpression(ImportExpression {
            source,
            options,
            phase,
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

    /// Parse an array-element / argument that permits a leading spread
    /// (`...AssignmentExpression`) — the array-literal, call-argument, and
    /// new-argument positions where a `SpreadElement` is grammatical (ecma262
    /// `ElementList` / `ArgumentList`). Everywhere else a bare `...` is a syntax
    /// error: `parse_primary_expression` does not accept it, so spread stays
    /// confined to exactly these list contexts (object literals and parameter
    /// lists build their own spread/rest nodes inline).
    pub(super) fn parse_spread_or_assignment_element(
        &mut self,
    ) -> Result<Expression<'arena>, ParseError> {
        if matches!(self.current_kind(), TokenKind::DotDotDot) {
            self.parse_spread_element()
        } else {
            self.parse_assignment_expression()
        }
    }

    /// Parse spread element: `...expr`
    fn parse_spread_element(&mut self) -> Result<Expression<'arena>, ParseError> {
        let (start, _) = self.current_pos();
        self.expect(&TokenKind::DotDotDot)?; // consume '...'

        // Use assignment_expression because comma separates array elements/object properties
        let argument = self.parse_assignment_expression_ref()?;
        // Use prev_token_end() to include closing paren when argument is parenthesized
        let end = self.prev_token_end() as u32;

        Ok(Expression::SpreadElement(SpreadElement {
            argument,
            span: Span::new(start as u32, end),
        }))
    }
}
