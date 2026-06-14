//! Operator precedence and flattening logic for binary expressions
//!
//! Implements prettier's "parens for clarity" behavior where mixing operators
//! at the same precedence level may require parentheses for readability.
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
/// 5: ^ (bitwise XOR)
/// 6: & (bitwise AND)
/// 7: ==, ===, !=, !== (equality)
/// 8: <, >, <=, >=, in, instanceof (relational)
/// 9: <<, >>, >>> (bitshift)
/// 10: +, - (additive)
/// 11: *, /, % (multiplicative)
/// 12: ** (exponentiation) - right-associative!
pub fn get_precedence(op: BinaryOperator) -> PrecedenceLevel {
    match op {
        BinaryOperator::QuestionQuestion => 1,
        BinaryOperator::PipePipe => 2,
        BinaryOperator::AmpersandAmpersand => 3,
        BinaryOperator::Pipe => 4,
        BinaryOperator::Caret => 5,
        BinaryOperator::Ampersand => 6,
        BinaryOperator::EqualsEquals
        | BinaryOperator::EqualsEqualsEquals
        | BinaryOperator::BangEquals
        | BinaryOperator::BangEqualsEquals => 7,
        BinaryOperator::LessThan
        | BinaryOperator::GreaterThan
        | BinaryOperator::LessThanEquals
        | BinaryOperator::GreaterThanEquals
        | BinaryOperator::Instanceof
        | BinaryOperator::In => 8,
        BinaryOperator::LeftShift
        | BinaryOperator::RightShift
        | BinaryOperator::UnsignedRightShift => 9,
        BinaryOperator::Plus | BinaryOperator::Minus => 10,
        BinaryOperator::Star | BinaryOperator::Slash | BinaryOperator::Percent => 11,
        BinaryOperator::StarStar => 12,
    }
}

/// Returns true if the operator is right-associative
pub fn is_right_associative(op: BinaryOperator) -> bool {
    matches!(op, BinaryOperator::StarStar)
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

    // Step 5: Exponentiation is right-associative
    // x ** y ** z → x ** (y ** z)
    if parent_op == BinaryOperator::StarStar {
        return false;
    }

    // Step 6: Bitshift operators don't flatten with each other
    // x << y << z → (x << y) << z
    if is_bitshift_operator(parent_op) && is_bitshift_operator(child_op) {
        return false;
    }

    // Step 7: Chained modulo doesn't flatten
    // x % y % z → (x % y) % z
    // Prettier adds parens for clarity when chaining modulo with itself
    if parent_op == BinaryOperator::Percent && child_op == BinaryOperator::Percent {
        return false;
    }

    // Default: can flatten (no parens needed)
    true
}

/// Check if operator is a bitshift operator (<<, >>, >>>)
fn is_bitshift_operator(op: BinaryOperator) -> bool {
    matches!(
        op,
        BinaryOperator::LeftShift | BinaryOperator::RightShift | BinaryOperator::UnsignedRightShift
    )
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
    fn test_chained_modulo_doesnt_flatten() {
        // a % b % c → (a % b) % c (needs parens for clarity)
        assert!(!should_flatten(
            BinaryOperator::Percent,
            BinaryOperator::Percent
        ));
    }

    #[test]
    fn test_precedence_levels() {
        // Verify precedence ordering (lower number = lower precedence)
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
        assert!(get_precedence(BinaryOperator::Pipe) < get_precedence(BinaryOperator::Caret));
        assert!(get_precedence(BinaryOperator::Caret) < get_precedence(BinaryOperator::Ampersand));
        assert!(
            get_precedence(BinaryOperator::Ampersand)
                < get_precedence(BinaryOperator::EqualsEquals)
        );
        assert!(
            get_precedence(BinaryOperator::EqualsEquals) < get_precedence(BinaryOperator::LessThan)
        );
        assert!(
            get_precedence(BinaryOperator::LessThan) < get_precedence(BinaryOperator::LeftShift)
        );
        assert!(get_precedence(BinaryOperator::LeftShift) < get_precedence(BinaryOperator::Plus));
        assert!(get_precedence(BinaryOperator::Plus) < get_precedence(BinaryOperator::Star));
        assert!(get_precedence(BinaryOperator::Star) < get_precedence(BinaryOperator::StarStar));
    }

    #[test]
    fn test_right_associative() {
        // Only ** is right-associative
        assert!(is_right_associative(BinaryOperator::StarStar));
        assert!(!is_right_associative(BinaryOperator::Plus));
        assert!(!is_right_associative(BinaryOperator::Star));
    }

    #[test]
    fn test_relational_and_logical_dont_flatten() {
        // a < b && c > d → no parens needed (different precedence)
        assert!(!should_flatten(
            BinaryOperator::AmpersandAmpersand,
            BinaryOperator::LessThan
        ));
    }

    #[test]
    fn test_exponentiation_never_flattens() {
        // ** is right-associative: a parent of ** short-circuits flattening (Step 5),
        // even against another ** at the same precedence level.
        assert!(!should_flatten(
            BinaryOperator::StarStar,
            BinaryOperator::StarStar
        ));
    }

    #[test]
    fn test_bitshift_pairs_dont_flatten() {
        // Bitshift operators share precedence (9) but prettier parenthesizes chains
        // for clarity (Step 6) — unlike additive ops, same-op bitshift does NOT flatten.
        assert!(!should_flatten(
            BinaryOperator::LeftShift,
            BinaryOperator::LeftShift
        ));
        assert!(!should_flatten(
            BinaryOperator::LeftShift,
            BinaryOperator::RightShift
        ));
        assert!(!should_flatten(
            BinaryOperator::UnsignedRightShift,
            BinaryOperator::RightShift
        ));
    }

    #[test]
    fn test_right_associative_negatives() {
        // Only ** is right-associative; pin a representative spread of the rest.
        for op in [
            BinaryOperator::PipePipe,
            BinaryOperator::AmpersandAmpersand,
            BinaryOperator::LeftShift,
            BinaryOperator::Percent,
            BinaryOperator::Slash,
            BinaryOperator::EqualsEqualsEquals,
        ] {
            assert!(!is_right_associative(op));
        }
    }
}
