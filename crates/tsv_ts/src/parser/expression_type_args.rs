// Type-argument byte-scan lookahead: disambiguates `<Type, ...>` from the
// less-than operator without lexing, by scanning raw source bytes after `<`.

use super::Parser;
use super::expression_lookahead::{
    is_construct_type_start, is_function_type_start, is_generic_function_type_start,
    scan_for_closing_angle_bracket,
};
use super::scan::{
    is_identifier_start, is_word_at, skip_identifier, skip_numeric_literal,
    skip_whitespace_and_comments,
};

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Check if current position starts type arguments: `<Type, ...>`
    ///
    /// Uses lookahead to distinguish from comparison operator.
    /// Dispatches based on first token after `<`:
    /// - Type keywords: `<string>`, `<never>`, etc.
    /// - Identifiers: `<T>`, `<Ns.Type>`, `<T | U>`, `<T, U>`
    /// - Function types: `<(x: T) => R>`, `<() => R>`
    /// - Parenthesized types: `<(A | B) & C>`, `<(() => void) | null>`
    /// - Object/tuple/literal types: `<{ a: T }>`, `<[T, U]>`, `<"foo">`
    pub(super) fn is_type_arguments_start(&self) -> bool {
        let bytes = self.source.as_bytes();
        let start = self.current.start as usize;

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
                // Exception: `this.` is member access, not type (allow `this /* comment */ .`)
                if bytes[pos..].starts_with(b"this") {
                    let after_this = skip_whitespace_and_comments(bytes, pos + b"this".len());
                    if after_this < bytes.len() && bytes[after_this] == b'.' {
                        return false;
                    }
                }
                // A keyword can also be a value (`null`, `true`, `undefined`, or a variable
                // named `string`, etc.), so `x < null` is a comparison. Confirm a closing
                // `>` follows before committing to type arguments.
                scan_for_closing_angle_bracket(bytes, pos)
            }

            // Identifier: type reference like `<T>` or `<Ns.Type>`
            _ if is_identifier_start(bytes[pos]) => {
                self.check_identifier_type_arg_pattern(bytes, pos)
            }

            // A type argument starting with `(`: a function type (`<(x: T) => R>`,
            // `<() => R>`) or a parenthesized type (`<(A | B) & C>`,
            // `<(() => void) | null>`). `is_function_type_start` fast-paths the arrow
            // shapes; otherwise fall back to the closing-`>` + follow-token scan (as the
            // `{`/`[`/literal arms do), so `x < (b)` and `x < (b) > c` stay comparisons
            // while `callee<(T)>(…)` and `x < (b) > (c)` are type arguments — matching
            // acorn's `canFollowTypeArgumentsInExpression`.
            b'(' => {
                is_function_type_start(bytes, pos) || scan_for_closing_angle_bracket(bytes, pos)
            }

            // A second `<` — the tail of a `<<` shift token, or a spaced
            // `< <` — can only open a generic function type
            // (`f<<T>(v: T) => void>()`); shift chains (`a << b > c`) never
            // match its `>`-then-`(` shape.
            b'<' => is_generic_function_type_start(bytes, pos + 1),

            // Object/tuple/string/template literal types — but the same tokens start
            // object, array, string, and template *value* literals, so `x < 'b'` and
            // `x < {a: 1}` are comparisons. Confirm a closing `>` follows (the scan skips
            // string contents and balances braces/brackets) before committing to type args.
            b'{' | b'[' | b'\'' | b'"' | b'`' => scan_for_closing_angle_bracket(bytes, pos),

            // Numeric literal types: `<42>`, `<-1>` — but `x < 42` is a comparison, so
            // confirm a closing `>` follows. The scan treats every numeric-literal byte
            // (digits, `.`, hex/exponent chars, `_`, `n`) as neutral, gliding over the
            // whole literal to its follow-token.
            b'0'..=b'9' | b'-' => scan_for_closing_angle_bracket(bytes, pos),

            // A leading `|`/`&` on the first union/intersection member
            // (`f<| A | B>()`, `f<& A & B>()`) — the form prettier itself emits
            // whenever such a type argument breaks across lines. Neither byte can
            // start an expression, so a `<` followed by one is never a comparison;
            // the closing-`>` + follow-token scan still runs, as in every other arm,
            // so an unterminated `<` stays unclaimed.
            b'|' | b'&' => scan_for_closing_angle_bracket(bytes, pos),

            // Not a recognized type argument start
            _ => false,
        }
    }

    /// Check if identifier at `pos` is followed by valid type argument patterns.
    ///
    /// Leading keywords that introduce a *non-reference* type are handled first:
    /// - `import('m').T` — an import type; always a valid type, so the closing-`>`
    ///   follow-token scan decides call vs comparison (matches acorn).
    /// - `new (…) => R` / `abstract new (…) => R` — a construct-signature type; the
    ///   `(…) =>` shape distinguishes it from a `new Foo()` value expression (which
    ///   stays a comparison), then the same scan confirms the close + follow token.
    ///
    /// Otherwise the leading word is a type reference: after scanning the full
    /// qualified name (e.g., `Ns.Type.Sub`), checks what follows:
    /// - `>` or `<`: definitely type args
    /// - `,`, `|`, `&`: scan for matching `>` to confirm type args
    /// - `[`: disambiguate indexed type vs array access
    /// - `extends`: type constraint
    fn check_identifier_type_arg_pattern(&self, bytes: &[u8], pos: usize) -> bool {
        // The leading identifier's end is located once and reused by the keyword
        // dispatch below and by the qualified-name loop's first step.
        let end = skip_identifier(bytes, pos);

        // Leading keyword forms that start a non-reference type. `import` is always
        // a valid type (scan decides); `new`/`abstract new` require the construct
        // shape so `f<new B()>(x)` and `a < new B() > (c)` stay comparisons.
        match &bytes[pos..end] {
            b"import" => return scan_for_closing_angle_bracket(bytes, pos),
            b"new" => {
                return is_construct_type_start(bytes, pos)
                    && scan_for_closing_angle_bracket(bytes, pos);
            }
            b"abstract" => {
                let after = skip_whitespace_and_comments(bytes, end);
                if is_construct_type_start(bytes, after)
                    && scan_for_closing_angle_bracket(bytes, pos)
                {
                    return true;
                }
                // A bare `abstract` is an ordinary type reference — fall through.
            }
            _ => {}
        }

        // Skip the leading identifier (already located as `end`) and any qualified
        // parts (e.g., Namespace.Type.SubType).
        let mut pos = skip_whitespace_and_comments(bytes, end);
        loop {
            // If followed by '.', continue scanning qualified name
            if pos < bytes.len() && bytes[pos] == b'.' {
                pos += 1;
                pos = skip_whitespace_and_comments(bytes, pos);
                if pos < bytes.len() && is_identifier_start(bytes[pos]) {
                    pos = skip_identifier(bytes, pos);
                    pos = skip_whitespace_and_comments(bytes, pos);
                    continue;
                }
            }
            break;
        }

        if pos >= bytes.len() {
            return false;
        }

        match bytes[pos] {
            // `||` and `&&` are logical operators, NOT type operators (`a || b`, not args)
            b'|' | b'&' if pos + 1 < bytes.len() && bytes[pos + 1] == bytes[pos] => false,

            // After the (qualified) type name: `>` closes the list, `<` opens a nested
            // one (`<A<B>>`), and `,` `|` `&` separate args. Each is confirmed by scanning
            // for the matching `>` — which rejects a trailing identifier, so `a < b > c`
            // and `a < b < c` stay comparisons. (`,` `|` `&` are neutral to the scan, so
            // starting at `pos` is equivalent to starting past the separator.)
            b'>' | b'<' | b',' | b'|' | b'&' => scan_for_closing_angle_bracket(bytes, pos),

            // Indexed type vs array access: `T[K]` vs `arr[0]`. Confirmed by the same
            // closing-`>` scan as the arms above — `T[K]` shaped bytes are equally a
            // member access on a comparison's right operand, so only the matching `>`
            // (and its follow token) tells them apart: `f(a < B[c], d)` and
            // `a < B[c] > d` stay comparisons, `f<A[B], C>(x)` is an instantiation.
            b'[' => {
                self.check_indexed_type_pattern(bytes, pos)
                    && scan_for_closing_angle_bracket(bytes, pos)
            }

            // Type constraint: `T extends U`. Whole-word — an identifier that merely
            // starts with `extends` is an ordinary operand (`a < b` ⏎ `extendsFoo()`,
            // where ASI ends the statement) — and confirmed by the closing-`>` scan
            // like every sibling arm.
            b'e' if is_word_at(bytes, pos, b"extends") => {
                scan_for_closing_angle_bracket(bytes, pos)
            }

            _ => false,
        }
    }

    /// Whether the `[` at `pos` can open an indexed-access type rather than an array
    /// index. A pre-filter only: every shape that stays grammatical both ways is handed
    /// to the caller's closing-`>` scan, which arbitrates.
    ///
    /// - `T[]`, `T["key"]`, `T[keyof U]`, `T[typeof x]`: indexed type
    /// - `T[K]`, `T[0]`, `T[-1]` followed by `>`, `,`, or another `[`: indexed type
    /// - `T[A | B]`, `T[0 | 1]`, `T[A[B]]`, `T[A.B]`, `T[A<B>]`,
    ///   `T[A extends B ? C : D]`: the index is itself a type, so the scan decides
    /// - `a[b - 1]`, `a[0 + 1]`, `a[c || d]`, `a[c <= d]`: arithmetic, or a
    ///   logical/shift/relational operator — an expression, never a type → array access
    /// - `a[-b]`: a unary negation, not a negative literal → array access
    fn check_indexed_type_pattern(&self, bytes: &[u8], pos: usize) -> bool {
        let inside = skip_whitespace_and_comments(bytes, pos + 1);
        let Some(&first) = bytes.get(inside) else {
            return false;
        };

        match first {
            // Empty brackets `T[]` — array type
            b']' => true,

            // Numeric literal index: `T[0]`, `T[-1]`, `T[0 | 1]`. A numeric literal is as
            // valid a type as it is an array index, so the literal alone decides nothing —
            // what FOLLOWS it does, under the same rule a reference index answers to.
            b'0'..=b'9' | b'-' => {
                let after_number = skip_numeric_literal(bytes, inside);
                // No literal starts here at all (`-b`) — a unary negation, so the index is
                // an expression. Guarding on this is what stops the `-` from swallowing an
                // identifier and landing on the same `]` a real literal ends at.
                if after_number == inside {
                    return false;
                }
                continues_as_type(bytes, skip_whitespace_and_comments(bytes, after_number))
            }

            // String literal key: `T["key"]`, `T['key']` — indexed access type
            b'\'' | b'"' | b'`' => true,

            // Identifier index: check for type keywords then what follows the identifier
            _ if is_identifier_start(first) => {
                let after_id = skip_identifier(bytes, inside);

                // Type operator keywords: `T[keyof U]`, `T[typeof x]`
                let kw = &bytes[inside..after_id];
                if kw == b"keyof" || kw == b"typeof" {
                    return true;
                }

                continues_as_type(bytes, skip_whitespace_and_comments(bytes, after_id))
            }

            // Unknown pattern — default to NOT type args (safer for JS expressions)
            _ => false,
        }
    }

    /// Check if position points to a TypeScript type keyword.
    ///
    /// Called on every `<`/`<<` disambiguation in the postfix loop, so ordinary
    /// relational comparisons (`i < n`) and shifts hit it — keep it cheap. A first-byte
    /// `match` dispatches to only the same-initial-letter candidate(s), so a byte that
    /// can't begin any of the 19 keywords (a digit, `(`, `[`, a quote, or an identifier
    /// starting with one of the other 14 letters) bails in O(1) instead of scanning all
    /// 19. Byte-identical to the prior linear scan: each keyword is checked with the
    /// same full-string compare + non-identifier-boundary condition, and no keyword is a
    /// prefix of another, so at most one can match at a position.
    fn is_type_keyword_at(&self, bytes: &[u8], pos: usize) -> bool {
        // Full keyword match at `pos`, not part of a longer identifier.
        let kw = |k: &[u8]| -> bool {
            pos + k.len() <= bytes.len()
                && &bytes[pos..pos + k.len()] == k
                && bytes
                    .get(pos + k.len())
                    .is_none_or(|&b| !b.is_ascii_alphanumeric() && b != b'_')
        };
        match bytes.get(pos) {
            Some(b'n') => kw(b"never") || kw(b"number") || kw(b"null"),
            Some(b's') => kw(b"string") || kw(b"symbol"),
            Some(b'b') => kw(b"boolean") || kw(b"bigint"),
            Some(b'a') => kw(b"any"),
            Some(b'u') => kw(b"unknown") || kw(b"undefined") || kw(b"unique"),
            Some(b'v') => kw(b"void"),
            Some(b'o') => kw(b"object"),
            // Type operators that can start a type: typeof, keyof, infer, readonly, unique
            Some(b't') => kw(b"this") || kw(b"true") || kw(b"typeof"),
            Some(b'f') => kw(b"false"),
            Some(b'k') => kw(b"keyof"),
            Some(b'i') => kw(b"infer"),
            Some(b'r') => kw(b"readonly"),
            _ => false,
        }
    }
}

/// Whether the byte at `after_operand` — the first non-trivia byte past an index operand —
/// continues a TYPE rather than an expression.
///
/// Shared by both operand kinds, and it must stay shared: `T[K | J]` and `T[0 | 1]` are
/// the same question, and answering it in one place is what keeps a numeric index from
/// being read more narrowly than a reference one.
fn continues_as_type(bytes: &[u8], after_operand: usize) -> bool {
    match bytes.get(after_operand) {
        Some(b']') => {
            let after_close = skip_whitespace_and_comments(bytes, after_operand + 1);
            // Type args end with `>`, continue with `,`, or chain another index group
            // (`T[K][J]`) — the caller's closing-`>` scan arbitrates all three
            matches!(bytes.get(after_close), Some(b'>' | b',' | b'['))
        }
        // `||` and `&&` are logical operators, so the index is an expression — only the
        // single `|`/`&` are type operators (as in the caller's own arm). Likewise `<<` is
        // a shift and `<=` a comparison, neither a type's `<`.
        Some(b'|' | b'&') if bytes.get(after_operand + 1) == bytes.get(after_operand) => false,
        Some(b'<') if matches!(bytes.get(after_operand + 1), Some(b'<' | b'=')) => false,
        // Type-continuation tokens: the index is a union or intersection (`T[A | B]`), a
        // nested index (`T[A[B]]`), a qualified name (`T[A.B]`), a generic reference
        // (`T[A<B>]`), or a conditional (`T[A extends B ? C : D]`). None of these can be
        // arithmetic, so hand the decision to the caller's closing-`>` scan.
        Some(b'|' | b'&' | b'[' | b'.' | b'<') => true,
        Some(b'e') if is_word_at(bytes, after_operand, b"extends") => true,
        // Anything else after the operand (e.g. `b - 1]`) is arithmetic — an expression,
        // never a type, and the one case the closing-`>` scan cannot arbitrate
        // (`a < arr[b - 1] > (c)` is grammatical both ways).
        _ => false,
    }
}
