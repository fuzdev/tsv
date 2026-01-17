// Expression-specific lookahead helpers for arrow function and type argument disambiguation
//
// These functions scan raw bytes to disambiguate syntactic constructs that
// look similar initially but parse differently:
// - Arrow functions vs parenthesized expressions: `(x) => y` vs `(x)`
// - Generic arrow functions vs comparison: `<T>() => x` vs `a < b`
// - Type arguments vs comparison chain: `foo<T>()` vs `foo < a`
//
// All functions operate on byte slices for performance (no tokenization needed).

use super::scan::{
    is_identifier_start, skip_block_comment, skip_identifier, skip_line_comment,
    skip_string_literal, skip_whitespace, skip_whitespace_and_comments,
};

/// Scan through parentheses and check if followed by `=>`
///
/// Assumes `pos` is at the opening `(`. Handles:
/// - Nested parentheses
/// - String literals inside parens
/// - Comments (line and block)
/// - Optional type annotation after `)`: `)` or `): type`
///
/// Returns `true` if the pattern `(...) =>` or `(...): type =>` is found.
pub(super) fn scan_parens_then_arrow(bytes: &[u8], mut pos: usize) -> bool {
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
pub(super) fn scan_identifier_then_arrow(bytes: &[u8], pos: usize) -> bool {
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
pub(super) fn scan_angle_brackets(bytes: &[u8], pos: usize) -> usize {
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
pub(super) fn is_function_type_start(bytes: &[u8], pos: usize) -> bool {
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
pub(super) fn scan_for_closing_angle_bracket(bytes: &[u8], mut pos: usize) -> bool {
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
