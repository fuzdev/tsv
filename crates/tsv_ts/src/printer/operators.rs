// Operator expression printing for TypeScript
//
// Handles printing of unary and binary expressions with:
// - Operator precedence and parenthesization
// - Clarity-based parens (mixing logical operators, etc.)

use super::Printer;
use crate::ast::internal::{self, BinaryOperator, Expression};
use tsv_lang::Span;
use tsv_lang::doc::{self, Doc};

/// Holds information about an operand in a binary expression chain
/// Used to track position information for comment placement
struct ChainOperand {
    doc: Doc,
    span: Span,
}

/// Check if child binary expression needs parens for clarity
///
/// Based on prettier's logic from:
/// - ~/dev/prettier/src/language-js/print/binaryish.js (shouldFlatten usage)
/// - ~/dev/prettier/src/language-js/needs-parens.js (lines 409-446)
/// - ~/dev/prettier/src/language-js/utils/index.js (shouldFlatten function)
///
/// Returns true when:
/// 1. Mixing different logical operators (&&, ||, ??) regardless of precedence
/// 2. Child has weaker precedence than parent (for correctness)
/// 3. Same precedence but can't flatten (determined by should_flatten rules)
/// 4. Right operand with same precedence (preserve programmer's grouping intent)
/// 5. Parent is a bitwise operator and precedences differ (for clarity)
pub(super) fn needs_parens_for_clarity(
    child: &internal::BinaryExpression,
    parent_op: BinaryOperator,
    is_right: bool,
) -> bool {
    let child_op = child.operator;

    // Special case: Logical operators (&&, ||, ??) mixing requires parens
    // prettier adds parens when mixing different logical operators for clarity
    if parent_op.is_logical() && child_op.is_logical() && parent_op != child_op {
        return true;
    }

    let parent_prec = parent_op.precedence();
    let child_prec = child_op.precedence();

    // Need parens when:
    // 1. Child has weaker precedence (lower number) - e.g., (a + b) * c needs parens around +
    if child_prec < parent_prec {
        return true;
    }

    // 2. Right operand with same precedence - preserve programmer's grouping intent
    // e.g., x + (y + z) keeps parens, but (x + y) + z removes them
    // See: prettier/src/language-js/needs-parens.js (lines 423-425)
    if is_right && child_prec == parent_prec {
        return true;
    }

    // 3. Same precedence but can't flatten - e.g., (a == b) == c, 2 ** (3 ** 2), etc.
    if child_prec == parent_prec && !parent_op.can_flatten_with(child_op) {
        return true;
    }

    // 4. Special handling for modulo with lower-precedence parent
    // When modulo is the child and parent has lower precedence, add parens for +/- and bitwise
    // This acts as a gate - if child is %, we return here and skip subsequent checks
    // See: prettier/src/language-js/needs-parens.js (lines 434-440)
    // PR #18163 (prettier 3.7): Add parens for bitwise operators too (<<, >>, etc.)
    if parent_prec < child_prec && child_op == BinaryOperator::Percent {
        return is_additive_operator(parent_op) || parent_op.is_bitwise();
    }

    // 5. Add parenthesis when working with bitwise operators and different precedence
    // It's not strictly needed but helps with code understanding
    // See: prettier/src/language-js/needs-parens.js (lines 442-446)
    if parent_op.is_bitwise() && child_prec != parent_prec {
        return true;
    }

    false
}

/// Check if operator is an additive operator (+, -)
fn is_additive_operator(op: BinaryOperator) -> bool {
    matches!(op, BinaryOperator::Plus | BinaryOperator::Minus)
}

impl<'a> Printer<'a> {
    /// Print an update expression: `++x`, `x++`, `--x`, `x--`
    pub(super) fn print_update_expression(&mut self, update: &internal::UpdateExpression) {
        if update.prefix {
            // Prefix: ++x, --x
            self.write(update.operator.as_str());
            self.print_expression(&update.argument);
        } else {
            // Postfix: x++, x--
            self.print_expression(&update.argument);
            self.write(update.operator.as_str());
        }
    }

    /// Print a unary expression: `-x`, `+x`, `!(a && b)`, `typeof x`, `void 0`
    pub(super) fn print_unary_expression(&mut self, unary: &internal::UnaryExpression) {
        self.write(unary.operator.as_str());

        // Keyword operators need a space before the operand
        if unary.operator.is_keyword_operator() {
            self.write(" ");
        }

        // Add parens around binary/logical expressions since unary has higher precedence
        // e.g., !(a && b) must keep parens, otherwise becomes !a && b (different meaning)
        match unary.argument.as_ref() {
            Expression::BinaryExpression(_) => {
                self.write("(");
                self.print_expression(&unary.argument);
                self.write(")");
            }
            _ => self.print_expression(&unary.argument),
        }
    }

    /// Print a binary expression: `a + b`, `x && y`
    ///
    /// Handles comments between operands (Prettier 3.7 #17723):
    /// - Block comments: `a && /* comment */ b` preserves inline
    /// - Line comments: `a && // comment\nb` forces line break
    pub(super) fn print_binary_expression(&mut self, binary: &internal::BinaryExpression) {
        self.print_binary_operand(&binary.left, binary.operator, false);
        self.write(" ");
        self.write(binary.operator.as_str());

        // Check for comments between operator and right operand
        let left_end = binary.left.span().end;
        let right_start = binary.right.span().start;
        let has_line_comment = self.has_line_comments_between(left_end, right_start);

        // Print comments
        self.print_inline_comments_between(left_end, right_start);

        if has_line_comment {
            // Line comment forces a line break
            self.write("\n");
            self.write_indent();
        } else {
            self.write(" ");
        }
        self.print_binary_operand(&binary.right, binary.operator, true);
    }

    /// Print operand with parens if needed for clarity
    fn print_binary_operand(
        &mut self,
        operand: &Expression,
        parent_op: BinaryOperator,
        is_right: bool,
    ) {
        match operand {
            Expression::BinaryExpression(child) => {
                if needs_parens_for_clarity(child, parent_op, is_right) {
                    self.write("(");
                    self.print_expression(operand);
                    self.write(")");
                } else {
                    self.print_expression(operand);
                }
            }
            _ => self.print_expression(operand),
        }
    }

    /// Build a Doc for an update expression
    pub(super) fn build_update_doc(&self, update: &internal::UpdateExpression) -> Doc {
        let argument_doc = self.build_expression_doc(&update.argument);
        let operator_doc = doc::text(update.operator.as_str());

        if update.prefix {
            // Prefix: ++x, --x
            doc::concat(vec![operator_doc, argument_doc])
        } else {
            // Postfix: x++, x--
            doc::concat(vec![argument_doc, operator_doc])
        }
    }

    /// Build a Doc for a unary expression
    pub(super) fn build_unary_doc(&self, unary: &internal::UnaryExpression) -> Doc {
        let argument_doc = match unary.argument.as_ref() {
            // Add parens around binary/logical expressions (unary has higher precedence)
            // For long expressions, the content inside parens can wrap
            Expression::BinaryExpression(binary) if binary.operator.is_logical() => {
                let inner = self.build_expression_doc(&unary.argument);
                doc::group(doc::concat(vec![
                    doc::text("("),
                    doc::indent_softline(inner),
                    doc::softline(),
                    doc::text(")"),
                ]))
            }
            Expression::BinaryExpression(_) => doc::concat(vec![
                doc::text("("),
                self.build_expression_doc(&unary.argument),
                doc::text(")"),
            ]),
            // Handle !! with logical expression: !!(a || b) - the inner ! wraps
            Expression::UnaryExpression(inner_unary) => {
                if Self::unary_contains_logical(&inner_unary.argument) {
                    // For !!(logical), we need the parens from the inner unary to wrap
                    self.build_expression_doc(&unary.argument)
                } else {
                    self.build_expression_doc(&unary.argument)
                }
            }
            _ => self.build_expression_doc(&unary.argument),
        };

        // Keyword operators need a space before the operand
        if unary.operator.is_keyword_operator() {
            doc::concat(vec![
                doc::text(unary.operator.as_str()),
                doc::text(" "),
                argument_doc,
            ])
        } else {
            doc::concat(vec![doc::text(unary.operator.as_str()), argument_doc])
        }
    }

    /// Check if a unary expression's argument (eventually) contains a logical expression
    fn unary_contains_logical(expr: &Expression) -> bool {
        match expr {
            Expression::BinaryExpression(binary) => binary.operator.is_logical(),
            Expression::UnaryExpression(unary) => Self::unary_contains_logical(&unary.argument),
            _ => false,
        }
    }

    /// Build a Doc for a binary expression
    ///
    /// Implements prettier's "add parens for clarity" behavior where mixing certain
    /// operators requires parentheses for readability:
    /// - `a && b || c` → `(a && b) || c` (mixing && and ||)
    /// - `a || b && c` → `a || (b && c)`
    /// - `a == b == c` → `(a == b) == c` (chained equality)
    /// - `x + (y + z)` → preserves right-side parens for same precedence
    ///
    /// Also supports line wrapping for long binary expressions:
    /// ```text
    /// a +
    /// b +
    /// c
    /// ```
    ///
    /// See: prettier/src/language-js/print/binaryish.js
    pub(super) fn build_binary_doc(&self, binary: &internal::BinaryExpression) -> Doc {
        // Use chain-based wrapping for all binary operators
        self.build_binary_chain_doc(binary)
    }

    /// Build a doc for a chain of binary operators with line wrapping support
    ///
    /// When the chain exceeds print width, breaks after operators:
    /// ```text
    /// a +
    /// b +
    /// c
    /// ```
    ///
    /// Flattens same-precedence operators that can be chained (e.g., a + b + c)
    /// but preserves parentheses where needed for clarity (e.g., a * b / c).
    ///
    /// Note: The binary expression doc itself does NOT add indent for continuations.
    /// Indentation comes from the parent context (e.g., assignment adds indent,
    /// function call args add indent, etc.).
    ///
    /// Handles comments between operands (Prettier 3.7 #17723):
    /// - Line comments force a line break
    /// - Block comments are printed inline
    fn build_binary_chain_doc(&self, binary: &internal::BinaryExpression) -> Doc {
        // Collect all operands (with spans) and operators in the chain
        let mut operands: Vec<ChainOperand> = Vec::new();
        let mut operators = Vec::new();
        self.collect_binary_chain_with_spans(binary, &mut operands, &mut operators);

        if operands.len() <= 1 {
            // Single operand, shouldn't happen but handle gracefully
            return self.build_expression_doc(&binary.left);
        }

        // Build: first + second + third
        // When flat: "first + second + third"
        // When broken:
        // "first +
        // second +
        // third"
        //
        // With comments:
        // "a && // comment
        // b"
        // "a && /* comment */ b"
        let mut parts = Vec::new();

        for (i, operand) in operands.iter().enumerate() {
            if i == 0 {
                parts.push(operand.doc.clone());
            } else {
                let prev_operand = &operands[i - 1];
                let operator = operators[i - 1];

                // Find the operator position in the source to split comments correctly
                // Comments before operator stay before, comments after operator stay after
                let op_str = operator.as_str();
                let range_start = prev_operand.span.end_usize();
                let range_end = operand.span.start_usize();
                let search_range = &self.source[range_start..range_end];
                let op_offset = search_range.find(op_str).unwrap_or(0);
                let op_pos = (range_start + op_offset) as u32;

                // Check for comments between prev operand and this operand
                let has_line_comment =
                    self.has_line_comments_between(prev_operand.span.end, operand.span.start);

                // Comments before the operator (trailing comments of left operand)
                let comments_before_op =
                    self.build_inline_comments_between_doc(prev_operand.span.end, op_pos);
                parts.push(comments_before_op);

                // Operator at end of previous line
                parts.push(doc::text(" "));
                parts.push(doc::text(op_str));

                // Comments after the operator (leading comments of right operand)
                let op_end = op_pos + op_str.len() as u32;

                if has_line_comment {
                    // Check if the comment is on its own line (has newline before it)
                    // or on the same line as the operator
                    let comment_on_own_line =
                        self.has_newline_before_comment(op_end, operand.span.start);

                    if comment_on_own_line {
                        // Comment is on its own line: `a &&\n// comment\nb`
                        // Output: `a &&\n// comment\nb`
                        parts.push(doc::hardline());
                        let comments_doc = self.build_inline_comments_between_doc_no_leading_space(
                            op_end,
                            operand.span.start,
                        );
                        parts.push(comments_doc);
                        parts.push(doc::hardline());
                    } else {
                        // Comment after operator: `a && // comment\nb`
                        // Output: `a && // comment\nb`
                        let comments_doc =
                            self.build_inline_comments_between_doc(op_end, operand.span.start);
                        parts.push(comments_doc);
                        parts.push(doc::hardline());
                    }
                } else {
                    // Check for block comments after operator
                    let comments_doc =
                        self.build_inline_comments_between_doc(op_end, operand.span.start);
                    parts.push(comments_doc);
                    // Normal line break (soft break when not forced)
                    parts.push(doc::line());
                }
                parts.push(operand.doc.clone());
            }
        }

        doc::group(doc::concat(parts))
    }

    /// Collect all operands (with spans) and operators from a chain of binary expressions
    ///
    /// Uses `should_flatten()` to determine which operators can be chained together.
    /// Left-associative: recursively flattens left side, keeps right operand as-is.
    fn collect_binary_chain_with_spans(
        &self,
        expr: &internal::BinaryExpression,
        operands: &mut Vec<ChainOperand>,
        operators: &mut Vec<BinaryOperator>,
    ) {
        // Recursively flatten left side if it can be chained with current operator
        if let Expression::BinaryExpression(left_binary) = &*expr.left {
            if expr.operator.can_flatten_with(left_binary.operator) {
                self.collect_binary_chain_with_spans(left_binary, operands, operators);
            } else {
                operands.push(ChainOperand {
                    doc: self.build_binary_operand_doc(&expr.left, expr.operator, false),
                    span: expr.left.span(),
                });
            }
        } else {
            operands.push(ChainOperand {
                doc: self.build_binary_operand_doc(&expr.left, expr.operator, false),
                span: expr.left.span(),
            });
        }

        // Add current operator
        operators.push(expr.operator);

        // Add right operand (don't flatten right side - left-associative)
        operands.push(ChainOperand {
            doc: self.build_binary_operand_doc(&expr.right, expr.operator, true),
            span: expr.right.span(),
        });
    }

    /// Build operand with parens if needed for clarity
    fn build_binary_operand_doc(
        &self,
        operand: &Expression,
        parent_op: BinaryOperator,
        is_right: bool,
    ) -> Doc {
        match operand {
            Expression::BinaryExpression(child) => {
                let child_doc = self.build_expression_doc(operand);

                if needs_parens_for_clarity(child, parent_op, is_right) {
                    doc::parens(child_doc)
                } else {
                    child_doc
                }
            }
            _ => self.build_expression_doc(operand),
        }
    }

    /// Print an await expression: `await promise`
    pub(super) fn print_await_expression(&mut self, await_expr: &internal::AwaitExpression) {
        self.write("await ");

        // Add parens around binary expressions since await has higher precedence
        match await_expr.argument.as_ref() {
            Expression::BinaryExpression(_) => {
                self.write("(");
                self.print_expression(&await_expr.argument);
                self.write(")");
            }
            _ => self.print_expression(&await_expr.argument),
        }
    }

    /// Build a Doc for an await expression
    pub(super) fn build_await_doc(&self, await_expr: &internal::AwaitExpression) -> Doc {
        let argument_doc = match await_expr.argument.as_ref() {
            // Add parens around binary expressions (await has higher precedence)
            Expression::BinaryExpression(_) => doc::concat(vec![
                doc::text("("),
                self.build_expression_doc(&await_expr.argument),
                doc::text(")"),
            ]),
            _ => self.build_expression_doc(&await_expr.argument),
        };

        doc::concat(vec![doc::text("await "), argument_doc])
    }

    /// Print a yield expression: `yield`, `yield value`, or `yield* iterable`
    pub(super) fn print_yield_expression(&mut self, yield_expr: &internal::YieldExpression) {
        if yield_expr.delegate {
            self.write("yield*");
            if let Some(ref arg) = yield_expr.argument {
                self.write(" ");
                self.print_expression(arg);
            }
        } else {
            self.write("yield");
            if let Some(ref arg) = yield_expr.argument {
                self.write(" ");
                self.print_expression(arg);
            }
        }
    }

    /// Build a Doc for a yield expression
    pub(super) fn build_yield_doc(&self, yield_expr: &internal::YieldExpression) -> Doc {
        let mut parts = Vec::new();

        if yield_expr.delegate {
            parts.push(doc::text("yield*"));
        } else {
            parts.push(doc::text("yield"));
        }

        if let Some(ref arg) = yield_expr.argument {
            parts.push(doc::text(" "));
            parts.push(self.build_expression_doc(arg));
        }

        doc::concat(parts)
    }

    /// Print a sequence expression: `(a, b, c)`
    ///
    /// Sequence expressions are always wrapped in parentheses to avoid
    /// ambiguity with comma separators in declarations, function calls, etc.
    pub(super) fn print_sequence_expression(&mut self, seq: &internal::SequenceExpression) {
        self.write("(");
        for (i, expr) in seq.expressions.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_expression(expr);
        }
        self.write(")");
    }

    /// Build a Doc for a sequence expression
    pub(super) fn build_sequence_doc(&self, seq: &internal::SequenceExpression) -> Doc {
        let mut parts = Vec::new();
        parts.push(doc::text("("));
        for (i, expr) in seq.expressions.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(", "));
            }
            parts.push(self.build_expression_doc(expr));
        }
        parts.push(doc::text(")"));
        doc::concat(parts)
    }

    /// Build a Doc for a sequence expression without outer parentheses.
    ///
    /// Used in contexts where the surrounding syntax already provides grouping,
    /// such as Svelte attribute expressions `={a, b, c}` where the braces
    /// provide the necessary grouping.
    pub(super) fn build_sequence_doc_bare(&self, seq: &internal::SequenceExpression) -> Doc {
        let mut parts = Vec::new();
        for (i, expr) in seq.expressions.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(", "));
            }
            parts.push(self.build_expression_doc(expr));
        }
        doc::concat(parts)
    }
}
