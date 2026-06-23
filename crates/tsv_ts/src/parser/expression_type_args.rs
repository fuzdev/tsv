// Type-argument byte-scan lookahead: disambiguates `<Type, ...>` from the
// less-than operator without lexing, by scanning raw source bytes after `<`.

use super::Parser;
use super::expression_lookahead::{is_function_type_start, scan_for_closing_angle_bracket};
use super::scan::{is_identifier_start, skip_identifier, skip_whitespace_and_comments};

impl<'a> Parser<'a> {
    /// Check if current position starts type arguments: `<Type, ...>`
    ///
    /// Uses lookahead to distinguish from comparison operator.
    /// Dispatches based on first token after `<`:
    /// - Type keywords: `<string>`, `<never>`, etc.
    /// - Identifiers: `<T>`, `<Ns.Type>`, `<T | U>`, `<T, U>`
    /// - Function types: `<(x: T) => R>`, `<() => R>`
    /// - Object/tuple/literal types: `<{ a: T }>`, `<[T, U]>`, `<"foo">`
    pub(super) fn is_type_arguments_start(&self) -> bool {
        let bytes = self.source.as_bytes();
        let start = self.current_start;

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

            // Function type: `<(x: T) => R>` or `<() => R>`
            b'(' => is_function_type_start(bytes, pos),

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

            // Not a recognized type argument start
            _ => false,
        }
    }

    /// Check if identifier at `pos` is followed by valid type argument patterns.
    ///
    /// After scanning the full qualified name (e.g., `Ns.Type.Sub`), checks what follows:
    /// - `>` or `<`: definitely type args
    /// - `,`, `|`, `&`: scan for matching `>` to confirm type args
    /// - `[`: disambiguate indexed type vs array access
    /// - `extends`: type constraint
    fn check_identifier_type_arg_pattern(&self, bytes: &[u8], pos: usize) -> bool {
        // Skip identifier and any qualified parts (e.g., Namespace.Type.SubType)
        let mut pos = pos;
        loop {
            pos = skip_identifier(bytes, pos);
            pos = skip_whitespace_and_comments(bytes, pos);

            // If followed by '.', continue scanning qualified name
            if pos < bytes.len() && bytes[pos] == b'.' {
                pos += 1;
                pos = skip_whitespace_and_comments(bytes, pos);
                if pos < bytes.len() && is_identifier_start(bytes[pos]) {
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

    /// Check if position points to a TypeScript type keyword
    fn is_type_keyword_at(&self, bytes: &[u8], pos: usize) -> bool {
        const TYPE_KEYWORDS: &[&[u8]] = &[
            b"never",
            b"string",
            b"number",
            b"boolean",
            b"any",
            b"unknown",
            b"void",
            b"null",
            b"undefined",
            b"symbol",
            b"bigint",
            b"object",
            b"this",
            b"true",
            b"false",
            // Type operators that can start a type
            b"typeof",
            b"keyof",
            b"infer",
            b"readonly",
            b"unique",
        ];

        for kw in TYPE_KEYWORDS {
            if pos + kw.len() <= bytes.len() && &bytes[pos..pos + kw.len()] == *kw {
                // Check it's not part of a longer identifier
                let next_pos = pos + kw.len();
                if next_pos >= bytes.len()
                    || (!bytes[next_pos].is_ascii_alphanumeric() && bytes[next_pos] != b'_')
                {
                    return true;
                }
            }
        }
        false
    }
}
