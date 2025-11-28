//! Operator precedence and flattening logic for binary expressions
//!
//! Implements prettier's "parens for clarity" behavior where mixing operators
//! at the same precedence level may require parentheses for readability.
//!
//! TODO: Future operator support (when parser implements them):
//! - Bitshift operators: <<, >>, >>> (would be precedence level 10)
//! - Exponentiation: ** (right-associative, precedence level 11)
//! - Bitwise with comparison: special handling for clarity
//!   e.g., `a < b & c < d` should become `(a < b) & (c < d)`
//!
//! Based on prettier's implementation:
//! - ~/dev/prettier/src/language-js/utils/index.js (lines 792-813)
//! - ~/dev/prettier/src/language-js/needs-parens.js

use super::internal::BinaryOperator;

/// Operator precedence level (higher = tighter binding)
pub type PrecedenceLevel = u8;

/// Get precedence level for an operator
///
/// Precedence levels (lower number = weaker binding, evaluated last):
/// 1: ?? (nullish coalescing)
/// 2: || (logical OR)
/// 3: && (logical AND)
/// 4: | (bitwise OR)
/// 5: & (bitwise AND)
/// 6: ==, ===, !=, !== (equality)
/// 7: <, >, <=, >= (relational)
/// 8: +, - (additive)
/// 9: *, /, % (multiplicative)
pub fn get_precedence(op: BinaryOperator) -> PrecedenceLevel {
    match op {
        BinaryOperator::QuestionQuestion => 1,
        BinaryOperator::PipePipe => 2,
        BinaryOperator::AmpersandAmpersand => 3,
        BinaryOperator::Pipe => 4,
        BinaryOperator::Ampersand => 5,
        BinaryOperator::EqualsEquals
        | BinaryOperator::EqualsEqualsEquals
        | BinaryOperator::BangEquals
        | BinaryOperator::BangEqualsEquals => 6,
        BinaryOperator::LessThan
        | BinaryOperator::GreaterThan
        | BinaryOperator::LessThanEquals
        | BinaryOperator::GreaterThanEquals => 7,
        BinaryOperator::Plus | BinaryOperator::Minus => 8,
        BinaryOperator::Star | BinaryOperator::Slash | BinaryOperator::Percent => 9,
    }
}

/// Check if operators can be written together without parens
///
/// Based on prettier's shouldFlatten logic from:
/// ~/dev/prettier/src/language-js/utils/index.js (lines 750-790)
///
/// Returns false (need parens) when:
/// - Operators have different precedence levels
/// - Both are equality operators (x == y == z needs parens)
/// - Mixing modulo with other multiplicative operators
/// - Different multiplicative operators (*, /, %)
///
/// Returns true (can flatten, no parens) when:
/// - Same operator at same precedence (x && y && z)
/// - Compatible operators at same level
pub fn should_flatten(parent_op: BinaryOperator, child_op: BinaryOperator) -> bool {
    let parent_prec = get_precedence(parent_op);
    let child_prec = get_precedence(child_op);

    // Step 1: Different precedence = don't flatten
    if parent_prec != child_prec {
        return false;
    }

    // Step 2: Equality operators don't flatten with each other
    // x == y == z needs parens: (x == y) == z
    if is_equality_operator(parent_op) && is_equality_operator(child_op) {
        return false;
    }

    // Step 3: Mixed modulo/multiplicative operators don't flatten
    // x * y % z stays, but we don't flatten different ones
    if parent_op != child_op
        && ((child_op == BinaryOperator::Percent && is_multiplicative_operator(parent_op))
            || (parent_op == BinaryOperator::Percent && is_multiplicative_operator(child_op)))
    {
        return false;
    }

    // Step 4: Different multiplicative operators don't flatten
    // x * y / z needs careful handling
    if parent_op != child_op
        && is_multiplicative_operator(parent_op)
        && is_multiplicative_operator(child_op)
    {
        return false;
    }

    // TODO: Chained modulo special case (future work)
    // Prettier doesn't flatten chained modulo: `a % b % c` should become `(a % b) % c`
    // even though it's the same operator. This would require:
    //   if parent_op == BinaryOperator::Percent && child_op == BinaryOperator::Percent {
    //       return false;
    //   }
    // See: TODO_LINE_WRAPPING.md for more details

    // Default: can flatten (no parens needed)
    true
}

/// Check if operator is an equality operator (==, ===, !=, !==)
fn is_equality_operator(op: BinaryOperator) -> bool {
    matches!(
        op,
        BinaryOperator::EqualsEquals
            | BinaryOperator::EqualsEqualsEquals
            | BinaryOperator::BangEquals
            | BinaryOperator::BangEqualsEquals
    )
}

/// Check if operator is a multiplicative operator (*, /, %)
fn is_multiplicative_operator(op: BinaryOperator) -> bool {
    matches!(
        op,
        BinaryOperator::Star | BinaryOperator::Slash | BinaryOperator::Percent
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logical_ops_dont_flatten() {
        // a && b || c → (a && b) || c
        assert!(!should_flatten(
            BinaryOperator::PipePipe,
            BinaryOperator::AmpersandAmpersand
        ));
        // a || b && c → a || (b && c)
        assert!(!should_flatten(
            BinaryOperator::AmpersandAmpersand,
            BinaryOperator::PipePipe
        ));
    }

    #[test]
    fn test_nullish_and_logical_dont_flatten() {
        // a ?? b || c → (a ?? b) || c
        assert!(!should_flatten(
            BinaryOperator::PipePipe,
            BinaryOperator::QuestionQuestion
        ));
        // a ?? b && c → (a ?? b) && c
        assert!(!should_flatten(
            BinaryOperator::AmpersandAmpersand,
            BinaryOperator::QuestionQuestion
        ));
    }

    #[test]
    fn test_same_op_flattens() {
        // a && b && c → no parens
        assert!(should_flatten(
            BinaryOperator::AmpersandAmpersand,
            BinaryOperator::AmpersandAmpersand
        ));
        // a || b || c → no parens
        assert!(should_flatten(
            BinaryOperator::PipePipe,
            BinaryOperator::PipePipe
        ));
        // a ?? b ?? c → no parens
        assert!(should_flatten(
            BinaryOperator::QuestionQuestion,
            BinaryOperator::QuestionQuestion
        ));
    }

    #[test]
    fn test_equality_dont_flatten() {
        // a == b == c → needs parens
        assert!(!should_flatten(
            BinaryOperator::EqualsEquals,
            BinaryOperator::EqualsEquals
        ));
        // a === b === c → needs parens
        assert!(!should_flatten(
            BinaryOperator::EqualsEqualsEquals,
            BinaryOperator::EqualsEqualsEquals
        ));
    }

    #[test]
    fn test_different_precedence_dont_flatten() {
        // a + b * c → different precedence, don't flatten
        assert!(!should_flatten(BinaryOperator::Plus, BinaryOperator::Star));
        // a && b < c → different precedence, don't flatten
        assert!(!should_flatten(
            BinaryOperator::AmpersandAmpersand,
            BinaryOperator::LessThan
        ));
    }

    #[test]
    fn test_additive_ops_flatten() {
        // a + b + c → no parens
        assert!(should_flatten(BinaryOperator::Plus, BinaryOperator::Plus));
        // a + b - c → can flatten
        assert!(should_flatten(BinaryOperator::Plus, BinaryOperator::Minus));
        assert!(should_flatten(BinaryOperator::Minus, BinaryOperator::Plus));
    }

    #[test]
    fn test_multiplicative_ops_dont_flatten_when_different() {
        // a * b / c → don't flatten different multiplicative
        assert!(!should_flatten(BinaryOperator::Star, BinaryOperator::Slash));
        assert!(!should_flatten(BinaryOperator::Slash, BinaryOperator::Star));
    }

    #[test]
    fn test_same_multiplicative_flattens() {
        // a * b * c → can flatten
        assert!(should_flatten(BinaryOperator::Star, BinaryOperator::Star));
        // a / b / c → can flatten
        assert!(should_flatten(BinaryOperator::Slash, BinaryOperator::Slash));
    }

    #[test]
    fn test_modulo_with_multiplicative_dont_flatten() {
        // a * b % c → don't flatten
        assert!(!should_flatten(
            BinaryOperator::Star,
            BinaryOperator::Percent
        ));
        assert!(!should_flatten(
            BinaryOperator::Percent,
            BinaryOperator::Star
        ));
        // a / b % c → don't flatten
        assert!(!should_flatten(
            BinaryOperator::Slash,
            BinaryOperator::Percent
        ));
        assert!(!should_flatten(
            BinaryOperator::Percent,
            BinaryOperator::Slash
        ));
    }

    #[test]
    fn test_same_modulo_flattens() {
        // a % b % c → can flatten
        assert!(should_flatten(
            BinaryOperator::Percent,
            BinaryOperator::Percent
        ));
    }

    #[test]
    fn test_precedence_levels() {
        // Verify precedence ordering
        assert!(
            get_precedence(BinaryOperator::QuestionQuestion)
                < get_precedence(BinaryOperator::PipePipe)
        );
        assert!(
            get_precedence(BinaryOperator::PipePipe)
                < get_precedence(BinaryOperator::AmpersandAmpersand)
        );
        assert!(
            get_precedence(BinaryOperator::AmpersandAmpersand)
                < get_precedence(BinaryOperator::Pipe)
        );
        assert!(get_precedence(BinaryOperator::Pipe) < get_precedence(BinaryOperator::Ampersand));
        assert!(
            get_precedence(BinaryOperator::Ampersand)
                < get_precedence(BinaryOperator::EqualsEquals)
        );
        assert!(
            get_precedence(BinaryOperator::EqualsEquals) < get_precedence(BinaryOperator::LessThan)
        );
        assert!(get_precedence(BinaryOperator::LessThan) < get_precedence(BinaryOperator::Plus));
        assert!(get_precedence(BinaryOperator::Plus) < get_precedence(BinaryOperator::Star));
    }

    #[test]
    fn test_relational_and_logical_dont_flatten() {
        // a < b && c > d → no parens needed (different precedence)
        assert!(!should_flatten(
            BinaryOperator::AmpersandAmpersand,
            BinaryOperator::LessThan
        ));
    }
}
