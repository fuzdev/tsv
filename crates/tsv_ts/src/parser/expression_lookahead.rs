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
    identifier_starts_at, is_word_at, skip_identifier, skip_numeric_literal,
    skip_whitespace_and_comments,
};
use crate::lexer::is_es_line_terminator_at;
use tsv_lang::source_scan::{
    TriviaProfile, is_regex_start_after, operand_end_after, skip_regex_literal, skip_trivia,
    trivia_ends_operand,
};

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

/// `>` at `pos` is a `>=` comparison operator, but NOT `>=>` (close-angle +
/// arrow) nor `>==` / `>===` (close-angle + `==`/`===`). The trailing-`=` and
/// trailing-`>` carve-outs keep the `>` available as a type-argument close: a
/// `>=` right after a would-be `<…>` close whose next byte is `=` or `>` can
/// only be `>` + `==`/`===`/`=>` (never a real `>=` operand — `x >= = y` /
/// `x >= > y` are nonsense), so acorn re-scans just the `>` and `f<T>==c` reads
/// as `(f<T>) == c`.
#[inline]
fn is_greater_equal_op(bytes: &[u8], pos: usize) -> bool {
    pos + 1 < bytes.len()
        && bytes[pos + 1] == b'='
        && !(pos + 2 < bytes.len() && matches!(bytes[pos + 2], b'>' | b'='))
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
    // Just past the last significant byte — the anchor `is_regex_start_after` reads.
    let mut operand_end = start;
    while pos < end {
        // Strings, templates, and comments are opaque — a `(`/`)` inside one is
        // not a real delimiter. The shared cursor skips all three in one place
        // (including backtick templates, which this scan historically missed).
        if let Some(past) = skip_trivia(bytes, pos, end, TriviaProfile::JS) {
            if trivia_ends_operand(bytes, pos) {
                operand_end = past;
            }
            pos = past;
            continue;
        }
        // Regex literals are the one trivia kind the cursor leaves significant
        // (it needs previous-token context). Skip a real regex so a `)`/`(`
        // inside its pattern isn't counted — e.g. a param default `(a = /\)/)`.
        if bytes[pos] == b'/' && is_regex_start_after(bytes, operand_end, start) {
            pos = skip_regex_literal(bytes, pos, end);
            operand_end = pos;
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
        operand_end = operand_end_after(bytes, pos, operand_end);
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
    // Just past the last significant byte — the anchor `is_regex_start_after` reads.
    // The `(` at `start` is significant, and the scan begins past it.
    let mut operand_end = start + 1;
    while pos < end {
        if let Some(past) = skip_trivia(bytes, pos, end, TriviaProfile::JS) {
            if trivia_ends_operand(bytes, pos) {
                operand_end = past;
            }
            pos = past;
            continue;
        }
        if bytes[pos] == b'/' && is_regex_start_after(bytes, operand_end, start) {
            pos = skip_regex_literal(bytes, pos, end);
            operand_end = pos;
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
        operand_end = operand_end_after(bytes, pos, operand_end);
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

/// Whether the return-type annotation opening at the `:` at `colon` is followed
/// **immediately** by `=>` — the signal that the `(…)` before it was an arrow's
/// parameter list, not a parenthesized expression.
///
/// Mirrors tsc's rule (`parseParenthesizedArrowFunctionExpression`): scan exactly
/// ONE type, then require `=>` right after it. Hunting for any `=>` at bracket
/// depth 0 is not enough, and tsc's own source names the counterexample —
/// "`a ? (b): c` will have `(b):` parsed as a signature with a return type
/// annotation". Two shapes put a stray `=>` within reach of such a scan, and in
/// neither is the `:` a return-type colon:
///
/// - a depth-0 `?`, which in a type is only a conditional type's `?` and so
///   requires an `extends` before it (`a ? (b, c) : d ? (p) => 1 : e`)
/// - a `=>` belonging to a **function type** in the annotation, which is the
///   type's own arrow and not the signature's (`a ? (b, c) : (p) => 1`)
///
/// Bytes that cannot occur at the top level of a type end the scan, so an
/// assignment (`d = …`), a non-null assertion (`d! ? …`) or a regex body
/// containing `=>` stops it rather than being scanned through.
fn scan_for_arrow(bytes: &[u8], colon: usize) -> bool {
    let end = bytes.len();
    let mut pos = colon + 1;
    // A `?` is type syntax only inside a conditional type, which is introduced by
    // `extends`. Tracked as a flag rather than a count: an undercount would
    // reject a valid annotation, and the whole point of this scan is to stop
    // over-rejecting.
    let mut saw_extends = false;
    let mut position = TypePos::Full;

    while pos < end {
        pos = skip_whitespace_and_comments(bytes, pos);
        if pos >= end {
            break;
        }

        // Strings and templates are opaque (comments were consumed above) and,
        // as literal types, are atoms.
        if let Some(past) = skip_trivia(bytes, pos, end, TriviaProfile::JS) {
            position = TypePos::Atom;
            pos = past;
            continue;
        }

        // Consume whole identifiers in one step, so a name's bytes can never be
        // read as operators and so a keyword is matched as a word (never as the
        // opening of an `extendsFoo`).
        if identifier_starts_at(bytes, pos) {
            position = if is_word_at(bytes, pos, b"extends") {
                // The extends-type and both branches are full types.
                saw_extends = true;
                TypePos::Full
            } else if TYPE_FULL_POSITION_WORDS
                .iter()
                .any(|w| is_word_at(bytes, pos, w))
            {
                TypePos::Full
            } else if TYPE_PREFIX_WORDS.iter().any(|w| is_word_at(bytes, pos, w)) {
                TypePos::Operand
            } else {
                TypePos::Atom
            };
            pos = skip_identifier(bytes, pos);
            continue;
        }

        // A numeric literal type (`-1` included, via the `-` arm below).
        if bytes[pos].is_ascii_digit() {
            pos = skip_numeric_literal(bytes, pos);
            position = TypePos::Atom;
            continue;
        }

        match bytes[pos] {
            b'(' => {
                // A `(` after a complete atom is not type syntax — the type
                // already ended, so the `:` was never a return-type colon
                // (`d as (p) => T`, `async (p) => 1`).
                if position == TypePos::Atom {
                    return false;
                }
                let Some(close) = matching_delimiter_close(bytes, pos) else {
                    return false;
                };
                // At a full-type position the `(` may open a function type's
                // parameter list, whose `=>` belongs to the TYPE and must not be
                // mistaken for the signature's. At an operand position the
                // grammar has no function type, so it is a parenthesized type
                // and a following `=>` is the enclosing arrow's.
                if position == TypePos::Full && paren_starts_function_type(bytes, pos) {
                    let after = skip_whitespace_and_comments(bytes, close + 1);
                    if bytes.get(after) == Some(&b'=') && bytes.get(after + 1) == Some(&b'>') {
                        // The function type's return type is itself a full type.
                        pos = after + 2;
                        position = TypePos::Full;
                        continue;
                    }
                }
                pos = close + 1;
                position = TypePos::Atom;
            }
            // An object or tuple/array type, or an index/array suffix on an atom.
            b'{' | b'[' => {
                let Some(close) = matching_delimiter_close(bytes, pos) else {
                    return false;
                };
                pos = close + 1;
                position = TypePos::Atom;
            }
            b'<' => {
                let Some(close) = matching_angle_close(bytes, pos + 1) else {
                    return false;
                };
                // At a full-type position `<…>` opens a generic function type,
                // whose parameter list follows; anywhere else it is a
                // type-argument list, which completes an atom.
                if position != TypePos::Full {
                    position = TypePos::Atom;
                }
                pos = close + 1;
            }
            // The type ended here, so this `=>` is the signature's arrow.
            b'=' if bytes.get(pos + 1) == Some(&b'>') => return true,
            // Union / intersection constituents, qualified-name parts, and a
            // negative literal type's sign all take a higher-precedence operand,
            // never a bare function type.
            b'|' | b'&' | b'.' | b'-' => {
                position = TypePos::Operand;
                pos += 1;
            }
            // A conditional type's `?` and `:` each introduce a branch, which is
            // a full type.
            b'?' => {
                if !saw_extends {
                    return false;
                }
                position = TypePos::Full;
                pos += 1;
            }
            b':' => {
                position = TypePos::Full;
                pos += 1;
            }
            // Anything else cannot be type syntax, so the type ended without an
            // arrow: a statement or list boundary (`;`, `,`), an operator
            // (`d = …`, `d! ? …`, a `/…/` regex whose body holds a `=>`), or an
            // unbalanced `)`/`]`/`}` closing the group this scan started inside.
            _ => return false,
        }
    }
    false
}

/// Where the type scan stands in the type grammar — which shapes the next token
/// may legally take. The `Full`/`Operand` split is the byte-scan form of the
/// type parser's own `fn_type_disallowed` flag; the two encode the same rule and
/// must move together.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TypePos {
    /// A complete type may start here, function and construct types included.
    Full,
    /// A higher-precedence operand (a union/intersection constituent, a
    /// `keyof`/`typeof` operand): a `(` is a parenthesized type, never a
    /// function type's parameter list.
    Operand,
    /// A type atom just ended; only suffixes (`[]`, `<…>`, `.`) and operators
    /// may follow.
    Atom,
}

/// Type-level prefix keywords that take an operand of their own, so a `(` right
/// after one still belongs to the type (`keyof (A | B)`, `typeof import('m')`)
/// rather than ending it.
const TYPE_PREFIX_WORDS: &[&[u8]] = &[b"keyof", b"typeof", b"readonly", b"infer", b"import"];

/// Type-level keywords after which a *whole* type may start again, function and
/// construct types included — the [`TypePos::Full`] counterpart of
/// [`TYPE_PREFIX_WORDS`]. `new`/`abstract` open a construct type, whose
/// `(params) =>` is the function-type shape. A type predicate's `is` hands its
/// operand to the same type entry as the plain `: type` branch — acorn-typescript's
/// `tsParseTypeOrTypePredicateAnnotation` calls `tsParseTypeAnnotation` either way,
/// and tsv's two predicate paths likewise call `Parser::parse_type` — so `x is T`
/// stands where the return-type colon itself stood, and `x is (A | B)[]` /
/// `x is (p: P) => R` scan like any other annotation.
///
/// `extends` belongs to this class too but keeps its own arm: it additionally arms
/// the conditional-type `?`. Matching is whole-word, so `isFoo` stays an atom.
const TYPE_FULL_POSITION_WORDS: &[&[u8]] = &[b"new", b"abstract", b"is"];

/// Whether the `(` at `paren` opens a **function type**'s parameter list rather
/// than a parenthesized type — acorn-typescript's
/// `tsIsUnambiguouslyStartOfFunctionType`, which decides whether a following
/// `=>` belongs to the type (`(b: B) => C`) or terminates it (`(B | C) => …`,
/// where the `=>` is the enclosing arrow's).
///
/// True for `()`, `(...`, and a parameter start (identifier, `this`, or a
/// balanced `{…}`/`[…]` pattern) followed by `:`, `,`, `?`, `=`, or by `) =>`.
///
/// The token-level twin of this question, asked once the parser has committed to
/// parsing a type, is `Parser::paren_starts_function_type_params` — same acorn
/// rule, same full-type-position precondition, different layer.
fn paren_starts_function_type(bytes: &[u8], paren: usize) -> bool {
    let mut pos = skip_whitespace_and_comments(bytes, paren + 1);
    match bytes.get(pos) {
        // `()` and `(...` are unambiguous.
        None => return false,
        Some(b')') => return true,
        Some(b'.') if bytes[pos..].starts_with(b"...") => return true,
        _ => {}
    }
    // A parameter start: an identifier (covers `this` and contextual keywords),
    // or a destructuring pattern, which is skipped as a balanced group rather
    // than parsed — a malformed one only fails the follow-token check below.
    pos = if identifier_starts_at(bytes, pos) {
        skip_identifier(bytes, pos)
    } else if matches!(bytes[pos], b'{' | b'[') {
        match matching_delimiter_close(bytes, pos) {
            Some(close) => close + 1,
            None => return false,
        }
    } else {
        return false;
    };
    pos = skip_whitespace_and_comments(bytes, pos);
    match bytes.get(pos) {
        // `( xxx :` and `( xxx ,`
        Some(b':' | b',') => true,
        // `( xxx ?` — the optional-parameter marker, not `?.` or `??`
        Some(b'?') => !matches!(bytes.get(pos + 1), Some(b'.' | b'?')),
        // `( xxx =` — a default value, not `=>` (an arrow) or `==` (equality)
        Some(b'=') => !matches!(bytes.get(pos + 1), Some(b'>' | b'=')),
        // `( xxx ) =>`
        Some(b')') => {
            let after = skip_whitespace_and_comments(bytes, pos + 1);
            bytes.get(after) == Some(&b'=') && bytes.get(after + 1) == Some(&b'>')
        }
        _ => false,
    }
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
    if identifier_starts_at(bytes, after_paren) {
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
/// `)`, `]`, `}`, or `;` at depth 0, and the close isn't followed by a token
/// that makes the `>` a comparison instead: a `>`/`>>`/`>>>` (relational or
/// shift) run, or an expression-starting token on the same line.
pub(super) fn scan_for_closing_angle_bracket(bytes: &[u8], pos: usize) -> bool {
    match matching_angle_close(bytes, pos) {
        None => false,
        Some(close) => {
            let after = skip_whitespace_and_comments(bytes, close + 1);
            if after >= bytes.len() {
                return true;
            }
            // A `>`-led follow token means the `>` we matched as the close was
            // really the first `>` of a longer relational/shift run: acorn re-reads
            // `a<b>>c` as `a < (b >> c)` and `a<b>>>c` as `a < (b >>> c)` (its
            // `tsMatchRightRelational` / bitShift bail), never `(a<b>) > c`. So a
            // `>` here disqualifies the type-argument reading — except when the run
            // is immediately closed by `=` (`>=` / `>>=`), which acorn keeps as an
            // instantiation follow (`f<T> >= c` is `(f<T>) >= c`).
            if bytes[after] == b'>' {
                let mut run = after;
                while run < bytes.len() && bytes[run] == b'>' {
                    run += 1;
                }
                return bytes.get(run) == Some(&b'=');
            }
            // Otherwise: a token that can start an expression (with no intervening
            // line break) makes this `>` a comparison operator (acorn's
            // `tokenCanStartExpression && !hasPrecedingLineBreak` bail). `(` (call),
            // a template (tagged template), and other non-expression tokens (`;`,
            // `,`, `)`, `.`, an operator…) continue the instantiation — and across a
            // line break an expression-starting token begins a new statement via ASI,
            // leaving the `<…>` an instantiation.
            !starts_expression_after_type_args(bytes, after)
                || has_line_terminator_between(bytes, close + 1, after)
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
    paren_list_then_arrow(bytes, after)
}

/// Whether the `(` at `paren` opens a parameter list whose matching `)` is
/// followed by `=>` — the tail shared by a generic function type's split
/// (`<…>(params) =>`) and a construct type (`new (params) =>`). `paren` must
/// point at the `(`; comments between `)` and `=>` are skipped. The `=>` is the
/// signal that separates these types from a shift/comparison chain or a
/// `new Foo()` value (neither of which has `(…) =>`).
#[inline]
fn paren_list_then_arrow(bytes: &[u8], paren: usize) -> bool {
    matching_delimiter_close(bytes, paren).is_some_and(|paren_close| {
        let after_params = skip_whitespace_and_comments(bytes, paren_close + 1);
        after_params + 1 < bytes.len()
            && bytes[after_params] == b'='
            && bytes[after_params + 1] == b'>'
    })
}

/// Whether `pos` begins a construct-signature type `new (params) => R` — the
/// `new`-prefixed sibling of [`is_function_type_start`]. True iff a whole-word
/// `new` is followed by a parenthesized parameter list whose matching `)` is
/// followed by `=>`. The `=>` is what separates the construct TYPE from a
/// `new Foo()` value expression, so `a < new Foo() > (c)` stays a comparison
/// while `f<new () => T>(x)` is a generic call — mirroring the `=>` requirement
/// in [`is_function_type_start`] / [`is_generic_function_type_start`]. Callers
/// still gate on the closing-`>` follow-token scan, so a construct type only
/// reads as type arguments when a call/tagged-template/end token actually
/// follows the `>`. `pos` may point at `new` directly or (for `abstract new`)
/// past the `abstract` keyword.
pub(super) fn is_construct_type_start(bytes: &[u8], pos: usize) -> bool {
    // Whole-word `new` (not an identifier like `newType`).
    if !is_word_at(bytes, pos, b"new") {
        return false;
    }
    let paren = skip_whitespace_and_comments(bytes, pos + b"new".len());
    if bytes.get(paren) != Some(&b'(') {
        return false;
    }
    paren_list_then_arrow(bytes, paren)
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

/// Find the `)`/`]`/`}` closing the `(`/`[`/`{` at `open`, or `None` if a
/// different delimiter closes unbalanced first or the input ends. Strings,
/// templates, and comments are opaque, like `matching_angle_close` (and like it,
/// no regex skip — a `)` inside a regex parameter default could close early,
/// which only fails toward the shift/comparison reading).
pub(super) fn matching_delimiter_close(bytes: &[u8], open: usize) -> Option<usize> {
    // paren, bracket, brace
    let target = match bytes.get(open)? {
        b'(' => 0,
        b'[' => 1,
        b'{' => 2,
        _ => return None,
    };
    let mut depths = [0i32; 3];
    let end = bytes.len();
    let mut pos = open;

    while pos < end {
        if let Some(past) = skip_trivia(bytes, pos, end, TriviaProfile::JS) {
            pos = past;
            continue;
        }
        let slot = match bytes[pos] {
            b'(' | b')' => Some(0),
            b'[' | b']' => Some(1),
            b'{' | b'}' => Some(2),
            _ => None,
        };
        if let Some(slot) = slot {
            if matches!(bytes[pos], b'(' | b'[' | b'{') {
                depths[slot] += 1;
            } else {
                depths[slot] -= 1;
                if slot == target && depths[slot] == 0 {
                    return Some(pos);
                }
                if depths[slot] < 0 {
                    return None; // Unbalanced - a different group ended here
                }
            }
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
    if identifier_starts_at(bytes, pos) {
        // `in` and `instanceof` are binary keyword operators — acorn's `tt._in` /
        // `tt._instanceof` have `startsExpr = false`, so a would-be `<…>` close
        // followed by one continues the instantiation (`f<T> in x` is `(f<T>) in x`)
        // rather than becoming a comparison. Every other identifier/keyword
        // (`as`/`satisfies` included — acorn lexes those as plain names) starts an
        // expression and disqualifies the type-argument reading.
        return !matches!(
            &bytes[pos..skip_identifier(bytes, pos)],
            b"in" | b"instanceof"
        );
    }
    let b = bytes[pos];
    // `!` starts an expression only as prefix negation (`!x`); `!=` / `!==` are
    // equality operators (acorn's tokens aren't `startsExpr`), so a would-be close
    // followed by one continues the instantiation (`f<T> != c` is `(f<T>) != c`).
    if b == b'!' {
        return bytes.get(pos + 1) != Some(&b'=');
    }
    b.is_ascii_digit()
        || matches!(b, b'\'' | b'"' | b'[' | b'{' | b'~' | b'+' | b'-')
        || (b == b'.' && pos + 1 < bytes.len() && bytes[pos + 1].is_ascii_digit())
}

/// Whether the byte range contains an ECMAScript line terminator (LF, CR,
/// U+2028, U+2029 — the latter two as UTF-8 `e2 80 a8`/`a9`).
fn has_line_terminator_between(bytes: &[u8], from: usize, to: usize) -> bool {
    let mut pos = from;
    while pos < to && pos < bytes.len() {
        if is_es_line_terminator_at(bytes, pos) {
            return true;
        }
        pos += 1;
    }
    false
}
