// Type-argument byte-scan lookahead: disambiguates `<Type, ...>` from the
// less-than operator without lexing, by scanning raw source bytes after `<`.

use super::Parser;
use super::expression_lookahead::{
    is_construct_type_start, is_function_type_start, is_generic_function_type_start,
    scan_for_closing_angle_bracket,
};
use super::scan::{is_identifier_start, skip_identifier, skip_whitespace_and_comments};

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
    /// - `T["key"]`, `T[keyof U]`, `T[typeof x]`: indexed type
    /// - `a[b - 1]`: complex expression → array access (default)
    fn check_indexed_type_pattern(&self, bytes: &[u8], pos: usize) -> bool {
        let inside = skip_whitespace_and_comments(bytes, pos + 1);
        if inside >= bytes.len() {
            return false;
        }

        // Empty brackets `T[]` — array type
        if bytes[inside] == b']' {
            return true;
        }

        // Numeric index is definitely array access
        if bytes[inside].is_ascii_digit() {
            return false;
        }

        // Identifier index: check for type keywords then what follows `]`
        if is_identifier_start(bytes[inside]) {
            let after_id = skip_identifier(bytes, inside);

            // Type operator keywords: `T[keyof U]`, `T[typeof x]`
            let kw = &bytes[inside..after_id];
            if kw == b"keyof" || kw == b"typeof" {
                return true;
            }

            let after_bracket = skip_whitespace_and_comments(bytes, after_id);
            if after_bracket < bytes.len() && bytes[after_bracket] == b']' {
                let after_close = skip_whitespace_and_comments(bytes, after_bracket + 1);
                // Type args end with `>` or continue with `,`
                if after_close < bytes.len() && matches!(bytes[after_close], b'>' | b',') {
                    return true;
                }
                return false;
            }
            // Identifier followed by something other than `]` (e.g., `b - 1]`)
            // is a complex expression — array access, not indexed type
            return false;
        }

        // String literal key: `T["key"]`, `T['key']` — indexed access type
        if matches!(bytes[inside], b'\'' | b'"' | b'`') {
            return true;
        }

        // Unknown pattern — default to NOT type args (safer for JS expressions)
        false
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
