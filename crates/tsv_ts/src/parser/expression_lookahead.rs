// Expression-specific lookahead helpers for arrow function and type argument disambiguation
//
// These functions scan raw bytes to disambiguate syntactic constructs that
// look similar initially but parse differently:
// - Arrow functions vs parenthesized expressions: `(x) => y` vs `(x)`
// - Generic arrow functions vs comparison: `<T>() => x` vs `a < b`
// - Type arguments vs comparison chain: `foo<T>()` vs `foo < a`
//
// All functions operate on byte slices for performance (no tokenization needed).

use super::scan::{is_identifier_start, skip_identifier, skip_whitespace_and_comments};
use tsv_lang::source_scan::{TriviaProfile, is_regex_start, skip_regex_literal, skip_trivia};

/// `<` at `pos` is `<=` comparison operator, not an angle bracket open
#[inline]
fn is_less_equal_op(bytes: &[u8], pos: usize) -> bool {
    pos + 1 < bytes.len() && bytes[pos + 1] == b'='
}

/// `>` at `pos` is preceded by `=`, making it part of `=>` arrow operator
#[inline]
fn is_arrow_close(bytes: &[u8], pos: usize) -> bool {
    pos > 0 && bytes[pos - 1] == b'='
}

/// `>` at `pos` is `>=` comparison operator, but NOT `>=>` (close-angle + arrow)
#[inline]
fn is_greater_equal_op(bytes: &[u8], pos: usize) -> bool {
    pos + 1 < bytes.len()
        && bytes[pos + 1] == b'='
        && !(pos + 2 < bytes.len() && bytes[pos + 2] == b'>')
}

/// Scan through parentheses and check if followed by `=>`
///
/// Assumes `pos` is at the opening `(`. Handles:
/// - Nested parentheses
/// - String literals inside parens
/// - Comments (line and block)
/// - Optional type annotation after `)`: `)` or `): type`
///
/// Returns `true` if the pattern `(...) =>` or `(...): type =>` is found.
pub(super) fn scan_parens_then_arrow(bytes: &[u8], start: usize) -> bool {
    if start >= bytes.len() || bytes[start] != b'(' {
        return false;
    }

    let end = bytes.len();
    let mut pos = start;
    let mut depth = 0;
    while pos < end {
        // Strings, templates, and comments are opaque — a `(`/`)` inside one is
        // not a real delimiter. The shared cursor skips all three in one place
        // (including backtick templates, which this scan historically missed).
        if let Some(past) = skip_trivia(bytes, pos, end, TriviaProfile::JS) {
            pos = past;
            continue;
        }
        // Regex literals are the one trivia kind the cursor leaves significant
        // (it needs backward token lookback). Skip a real regex so a `)`/`(`
        // inside its pattern isn't counted — e.g. a param default `(a = /\)/)`.
        if bytes[pos] == b'/' && is_regex_start(bytes, pos, start) {
            pos = skip_regex_literal(bytes, pos, end);
            continue;
        }
        match bytes[pos] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return check_arrow_after_paren(bytes, pos + 1);
                }
            }
            _ => {}
        }
        pos += 1;
    }
    false
}

/// Whether the parenthesized content at `(` (a `(` immediately followed by a
/// `{`/`[`) is a parenthesized union/intersection *type* rather than a
/// destructuring-parameter list — i.e. the leading `{…}`/`[…]` is directly
/// followed (at the paren's top level) by a `|` or `&`.
///
/// This disambiguates `({ b: B } | C) => x` / `([B] | C) => x` (a parenthesized
/// union type sitting inside an enclosing arrow's return type, whose `=>` the
/// paren-only [`scan_parens_then_arrow`] mistakes for this paren's own arrow)
/// from a genuine function-type param list `({ a }) => U` / `([a]: T) => U`. A
/// destructuring parameter can only be followed by `:`, `?`, `,`, or `)`; a
/// `|`/`&` right after the pattern is never a valid parameter, so it is
/// unambiguously a union/intersection type. (A `|`/`&` *inside* the pattern's
/// own type annotation — `({ a }: T | U) => V` — sits after the `:`, past the
/// balanced pattern, so it is not matched.)
///
/// Assumes `bytes[start] == b'('`. Skips strings/templates/comments (and a real
/// regex, e.g. a `/}/` default) while matching the balanced leading `{…}`/`[…]`.
pub(super) fn paren_pattern_then_type_operator(bytes: &[u8], start: usize) -> bool {
    let end = bytes.len();
    if start >= end || bytes[start] != b'(' {
        return false;
    }
    let mut pos = skip_whitespace_and_comments(bytes, start + 1);
    let Some(&open @ (b'{' | b'[')) = bytes.get(pos) else {
        return false;
    };
    let close = if open == b'{' { b'}' } else { b']' };
    let mut depth = 0usize;
    while pos < end {
        if let Some(past) = skip_trivia(bytes, pos, end, TriviaProfile::JS) {
            pos = past;
            continue;
        }
        if bytes[pos] == b'/' && is_regex_start(bytes, pos, start) {
            pos = skip_regex_literal(bytes, pos, end);
            continue;
        }
        let b = bytes[pos];
        if b == open {
            depth += 1;
        } else if b == close {
            depth -= 1;
            if depth == 0 {
                let next = skip_whitespace_and_comments(bytes, pos + 1);
                return matches!(bytes.get(next), Some(b'|' | b'&'));
            }
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

        // Strings/templates are opaque (comments were already consumed above); a
        // delimiter inside one isn't significant. No regex skip is needed here
        // (unlike `scan_parens_then_arrow`): this walks type syntax after a `:` /
        // `)`, where a `/…/` regex literal can't appear, so a stray `/` is just an
        // insignificant byte.
        if let Some(past) = skip_trivia(bytes, pos, bytes.len(), TriviaProfile::JS) {
            pos = past;
            continue;
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
            b'<' if is_less_equal_op(bytes, pos) => pos += 1,
            b'<' => angle_depth += 1,
            b'>' if is_arrow_close(bytes, pos) => {} // `=>` handled by `=` match
            b'>' if is_greater_equal_op(bytes, pos) => pos += 1,
            b'>' if angle_depth > 0 => angle_depth -= 1,

            // Check for `=>` at depth 0
            b'=' if pos + 1 < bytes.len() && bytes[pos + 1] == b'>' && at_depth_zero => {
                return true;
            }
            b'=' if pos + 1 < bytes.len() && bytes[pos + 1] == b'>' => {
                // Not at depth zero - skip past `=>` to avoid matching the `>` as angle close
                pos += 1;
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
/// Returns `true` if pattern `identifier =>` is found (with optional whitespace/comments).
pub(super) fn scan_identifier_then_arrow(bytes: &[u8], pos: usize) -> bool {
    // Skip the identifier (already validated by lexer as TokenKind::Identifier)
    let end = skip_identifier(bytes, pos);

    // Skip whitespace and comments after identifier: `a /* comment */ =>`
    let pos = skip_whitespace_and_comments(bytes, end);

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

    let end = bytes.len();
    let mut pos = pos + 1;
    let mut depth = 1;

    while pos < end && depth > 0 {
        // Strings, templates, and comments are opaque (the shared cursor skips
        // all three); an angle inside one isn't significant. No regex skip is
        // needed (unlike `scan_parens_then_arrow`): this scans type-argument
        // syntax `<…>`, where a `/…/` regex literal can't appear.
        if let Some(past) = skip_trivia(bytes, pos, end, TriviaProfile::JS) {
            pos = past;
            continue;
        }
        match bytes[pos] {
            b'<' if is_less_equal_op(bytes, pos) => pos += 1,
            b'<' => depth += 1,
            b'>' if is_arrow_close(bytes, pos) => {}
            b'>' if is_greater_equal_op(bytes, pos) => pos += 1,
            b'>' => depth -= 1,
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

    let after_paren = skip_whitespace_and_comments(bytes, pos + 1);
    if after_paren >= bytes.len() {
        return false;
    }

    // `(identifier:` or `(identifier?:` → function type parameter
    if is_identifier_start(bytes[after_paren]) {
        let after_id = skip_whitespace_and_comments(bytes, skip_identifier(bytes, after_paren));
        if after_id < bytes.len() {
            match bytes[after_id] {
                // `(b: T)` typed parameter
                b':' => return true,
                // `(b?: T)` optional parameter — the `?` must be followed by `:`.
                // Otherwise it's a ternary operand `(b ? c : d)`, i.e. a comparison
                // `x < (b ? c : d)`, not a function type.
                b'?' => {
                    let after_q = skip_whitespace_and_comments(bytes, after_id + 1);
                    if after_q < bytes.len() && bytes[after_q] == b':' {
                        return true;
                    }
                }
                _ => {}
            }
        }
    }

    // `() =>` or `( /* comment */ ) =>` → no-params function type
    if bytes[after_paren] == b')' {
        let after_close = skip_whitespace_and_comments(bytes, after_paren + 1);
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
/// `)`, `]`, `}`, or `;` at depth 0, and the close isn't followed by an
/// expression-starting token (which would make the `>` a comparison instead).
pub(super) fn scan_for_closing_angle_bracket(bytes: &[u8], pos: usize) -> bool {
    match matching_angle_close(bytes, pos) {
        None => false,
        Some(close) => {
            // After closing `>` in type args, the next token is `(`
            // (a call), a template literal (a tagged template), or a
            // non-expression token (`;`, `,`, `)`, an operator…).
            // Any other expression-starting token — identifier,
            // number, string, `[`, `{`, or a prefix operator — means
            // this `>` is a comparison operator instead — but only
            // on the same line: across a line break the token starts
            // a new statement via ASI and the `<…>` is an
            // instantiation (acorn bails to relational on
            // `tokenCanStartExpression && !hasPrecedingLineBreak`).
            let after = skip_whitespace_and_comments(bytes, close + 1);
            !(after < bytes.len()
                && starts_expression_after_type_args(bytes, after)
                && !has_line_terminator_between(bytes, close + 1, after))
        }
    }
}

/// Whether `pos` (the first byte after an inner `<` that directly follows a
/// type-argument-opening `<`) begins a generic function type's type-parameter
/// list — the only type that can start with `<`, so the only valid reading of
/// a `<<` at a type-argument position (`f<<T>(v: T) => void>()`). True iff
/// the list's matching `>` is followed by a parenthesized parameter list whose
/// matching `)` is followed by `=>`. The `=>` requirement is what separates
/// the split from a shift-comparison chain whose right operand is
/// parenthesized (`a << b > (c)`): an arrow can never be a relational
/// operand, so real shift code never has `(…) =>` after the would-be close.
pub(super) fn is_generic_function_type_start(bytes: &[u8], pos: usize) -> bool {
    let Some(close) = matching_angle_close(bytes, pos) else {
        return false;
    };
    let after = skip_whitespace_and_comments(bytes, close + 1);
    if after >= bytes.len() || bytes[after] != b'(' {
        return false;
    }
    matching_paren_close(bytes, after + 1).is_some_and(|paren_close| {
        let after_params = skip_whitespace_and_comments(bytes, paren_close + 1);
        after_params + 1 < bytes.len()
            && bytes[after_params] == b'='
            && bytes[after_params + 1] == b'>'
    })
}

/// Find the `>` closing the angle-bracket list opened just before `pos`
/// (`pos` is the first byte after the `<`), or `None` if an unbalanced `)`,
/// `]`, `}`, or a top-level `;` intervenes.
///
/// Operator disambiguation: `<=` and `>=` are comparison operators (not angle
/// brackets) and `=>` is an arrow operator (not a closing bracket).
pub(super) fn matching_angle_close(bytes: &[u8], mut pos: usize) -> Option<usize> {
    let mut angle_depth: i32 = 1;
    let mut paren_depth: i32 = 0;
    let mut bracket_depth: i32 = 0;
    let mut brace_depth: i32 = 0;
    let end = bytes.len();

    while pos < end {
        // Strings, templates, and comments are opaque (the shared cursor skips
        // all three); a `<`/`>`/`;` inside one isn't significant. No regex skip is
        // needed (unlike `scan_parens_then_arrow`): this verifies a type-argument
        // sequence `<…>`, where a `/…/` regex literal can't appear.
        if let Some(past) = skip_trivia(bytes, pos, end, TriviaProfile::JS) {
            pos = past;
            continue;
        }
        match bytes[pos] {
            b'<' if is_less_equal_op(bytes, pos) => pos += 1,
            // Angle depth only tracks at delimiter depth 0 — `<`/`>` inside a
            // balanced `(…)`, `[…]`, or `{…}` (e.g. `<[T<A>]>`) pair up within
            // that delimiter and must not leak into the outer angle count.
            b'<' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                angle_depth += 1;
            }
            b'>' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                if is_arrow_close(bytes, pos) {
                    // `=>` arrow operator, not a closing angle bracket
                } else if is_greater_equal_op(bytes, pos) {
                    pos += 1; // skip the `=` too
                } else {
                    angle_depth -= 1;
                    if angle_depth == 0 {
                        return Some(pos);
                    }
                }
            }
            b'(' => paren_depth += 1,
            b')' => {
                paren_depth -= 1;
                if paren_depth < 0 {
                    return None; // Unbalanced - hit call/group end
                }
            }
            b'[' => bracket_depth += 1,
            b']' => {
                bracket_depth -= 1;
                if bracket_depth < 0 {
                    return None; // Unbalanced - hit array end
                }
            }
            b'{' => brace_depth += 1,
            b'}' => {
                brace_depth -= 1;
                if brace_depth < 0 {
                    return None; // Unbalanced - hit block end
                }
            }
            // Statement end — but only at the top level. Inside a balanced `{…}` a `;`
            // is an object-type member separator (`<{ a: number; b: string }>`).
            b';' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => return None,
            _ => {}
        }
        pos += 1;
    }
    None
}

/// Find the `)` closing the paren opened just before `pos` (`pos` is the first
/// byte after the `(`), or `None` if an unbalanced `]`/`}` or end of input
/// intervenes. Strings, templates, and comments are opaque, like
/// `matching_angle_close` (and like it, no regex skip — a `)` inside a regex
/// parameter default could close early, which only fails toward the
/// shift/comparison reading).
fn matching_paren_close(bytes: &[u8], mut pos: usize) -> Option<usize> {
    let mut paren_depth: i32 = 1;
    let mut bracket_depth: i32 = 0;
    let mut brace_depth: i32 = 0;
    let end = bytes.len();

    while pos < end {
        if let Some(past) = skip_trivia(bytes, pos, end, TriviaProfile::JS) {
            pos = past;
            continue;
        }
        match bytes[pos] {
            b'(' => paren_depth += 1,
            b')' => {
                paren_depth -= 1;
                if paren_depth == 0 {
                    return Some(pos);
                }
            }
            b'[' => bracket_depth += 1,
            b']' => {
                bracket_depth -= 1;
                if bracket_depth < 0 {
                    return None; // Unbalanced - hit array end
                }
            }
            b'{' => brace_depth += 1,
            b'}' => {
                brace_depth -= 1;
                if brace_depth < 0 {
                    return None; // Unbalanced - hit block end
                }
            }
            _ => {}
        }
        pos += 1;
    }
    None
}

/// Whether the token starting at `pos` can begin an expression and therefore
/// turns a would-be type-argument `<…>` into a relational chain (acorn's
/// `tokenCanStartExpression` bail): identifier (covers keyword operands like
/// `typeof`), numeric literal (including `.5`), string, `[`, `{`, or a prefix
/// operator. `(` (call) and `` ` `` (tagged template) continue the
/// instantiation instead and are deliberately excluded; regex is excluded
/// because acorn also rejects `x < y > /a/`.
fn starts_expression_after_type_args(bytes: &[u8], pos: usize) -> bool {
    let b = bytes[pos];
    is_identifier_start(b)
        || b.is_ascii_digit()
        || matches!(b, b'\'' | b'"' | b'[' | b'{' | b'!' | b'~' | b'+' | b'-')
        || (b == b'.' && pos + 1 < bytes.len() && bytes[pos + 1].is_ascii_digit())
}

/// Whether the byte range contains an ECMAScript line terminator (LF, CR,
/// U+2028, U+2029 — the latter two as UTF-8 `e2 80 a8`/`a9`).
fn has_line_terminator_between(bytes: &[u8], from: usize, to: usize) -> bool {
    let mut pos = from;
    while pos < to && pos < bytes.len() {
        match bytes[pos] {
            b'\n' | b'\r' => return true,
            0xe2 if pos + 2 < bytes.len()
                && bytes[pos + 1] == 0x80
                && (bytes[pos + 2] == 0xa8 || bytes[pos + 2] == 0xa9) =>
            {
                return true;
            }
            _ => {}
        }
        pos += 1;
    }
    false
}
