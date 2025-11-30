// Operator expression printing for TypeScript
//
// Handles printing of unary and binary expressions with:
// - Operator precedence and parenthesization
// - Clarity-based parens (mixing logical operators, etc.)

use super::Printer;
use crate::ast::internal::{self, BinaryOperator, Expression};
use tsv_lang::doc::{self, Doc};

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
    if is_logical_operator(parent_op) && is_logical_operator(child_op) && parent_op != child_op {
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
    // When modulo is the child and parent has lower precedence, only add parens for +/-
    // This acts as a gate - if child is %, we return here and skip subsequent checks
    // See: prettier/src/language-js/needs-parens.js (lines 434-440 in v3.6.2)
    // Note: prettier 3.8.0+ adds parens for bitwise too (PR #18163), but we match 3.6.2
    if parent_prec < child_prec && child_op == BinaryOperator::Percent {
        return is_additive_operator(parent_op);
    }

    // 5. Add parenthesis when working with bitwise operators and different precedence
    // It's not strictly needed but helps with code understanding
    // See: prettier/src/language-js/needs-parens.js (lines 442-446)
    if crate::ast::precedence::is_bitwise_operator(parent_op) && child_prec != parent_prec {
        return true;
    }

    false
}

/// Check if operator is a logical operator (&&, ||, ??)
fn is_logical_operator(op: BinaryOperator) -> bool {
    matches!(
        op,
        BinaryOperator::AmpersandAmpersand
            | BinaryOperator::PipePipe
            | BinaryOperator::QuestionQuestion
    )
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
    pub(super) fn print_binary_expression(&mut self, binary: &internal::BinaryExpression) {
        self.print_binary_operand(&binary.left, binary.operator, false);
        self.write(" ");
        self.write(binary.operator.as_str());
        self.write(" ");
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
            Expression::BinaryExpression(_) => doc::concat(vec![
                doc::text("("),
                self.build_expression_doc(&unary.argument),
                doc::text(")"),
            ]),
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

    /// Build a Doc for a binary expression
    ///
    /// Implements prettier's "add parens for clarity" behavior where mixing certain
    /// operators requires parentheses for readability:
    /// - `a && b || c` → `(a && b) || c` (mixing && and ||)
    /// - `a || b && c` → `a || (b && c)`
    /// - `a == b == c` → `(a == b) == c` (chained equality)
    /// - `x + (y + z)` → preserves right-side parens for same precedence
    ///
    /// See: prettier/src/language-js/print/binaryish.js
    pub(super) fn build_binary_doc(&self, binary: &internal::BinaryExpression) -> Doc {
        let left_doc = self.build_binary_operand_doc(&binary.left, binary.operator, false);
        let right_doc = self.build_binary_operand_doc(&binary.right, binary.operator, true);

        doc::concat(vec![
            left_doc,
            doc::text(" "),
            doc::text(binary.operator.as_str()),
            doc::text(" "),
            right_doc,
        ])
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
                    doc::concat(vec![doc::text("("), child_doc, doc::text(")")])
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
}
