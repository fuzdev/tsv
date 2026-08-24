// Type-argument byte-scan lookahead: disambiguates `<Type, ...>` from the
// less-than operator without lexing, by scanning raw source bytes after `<`.

use super::Parser;
use super::expression_lookahead::{
    is_construct_type_start, is_function_type_start, is_generic_function_type_start,
    matching_delimiter_close, scan_for_closing_angle_bracket,
};
use super::scan::{
    identifier_starts_at, is_word_at, skip_identifier, skip_numeric_literal,
    skip_whitespace_and_comments,
};
use tsv_lang::source_scan::{TriviaProfile, skip_template_literal, skip_trivia};

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Check if current position starts type arguments: `<Type, ...>`
    ///
    /// Uses lookahead to distinguish from the comparison operator, dispatching on the
    /// first token after `<`. [`type_keyword_at`] classifies the identifier-shaped heads
    /// ahead of the byte dispatch, since the identifier arm would otherwise claim them:
    /// - Type keywords: an atom (`<string>`, `<never>`), `this`, or an operator
    ///   (`<keyof T>`, `<typeof x>` — [`type_operator_commits`])
    /// - Identifiers: `<T>`, `<Ns.Type>`, `<T | U>`, `<T, U>`
    /// - Function types: `<(x: T) => R>`, `<() => R>`, `<<T>(v: T) => void>`
    /// - Parenthesized types: `<(A | B) & C>`, `<(() => void) | null>`
    /// - Object/tuple types: `<{ a: T }>`, `<[T, U]>`
    /// - Literal types: `<"foo">`, `` <`a${B}`> ``, `<42>`, `<-1>`, `<.5>`
    /// - A leading union/intersection bar: `<| A | B>`, `<& A & B>`
    ///
    /// Every operand-headed arm ends at the shared follow-token filter
    /// [`type_operand_follow_commits`], which is what decides call vs comparison.
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

        // Type keywords come ahead of the byte dispatch — every one is also
        // identifier-shaped, so the identifier arm below would otherwise claim them.
        match type_keyword_at(bytes, pos) {
            // An atom-shaped keyword (`string`, `null`, `this`, …) is as much a value
            // name as a type head, so it takes the identifier arm's follow-token
            // filter: only content that could continue a TYPE past the head commits.
            // That is what keeps ``p < string ? q : r > `t` `` a comparison (matching
            // acorn) while `f<string>()`, `f<string[]>()` and `p<string | q>(t, u)`
            // stay instantiations — the atom answers every follow-token question
            // exactly as an ordinary identifier does.
            Some(TypeKeywordKind::Atom) => {
                return check_identifier_type_arg_pattern(bytes, pos);
            }

            // `this` is an atom too, minus one rule: `this.` is member access, never a
            // type (allow `this /* comment */ .`). `this` cannot head a qualified type
            // name — unlike `string`, an ordinary identifier token to the type grammar —
            // so the filter's qualified-name walk must not see the `.`.
            Some(TypeKeywordKind::This) => {
                let after_this = skip_whitespace_and_comments(bytes, pos + b"this".len());
                if bytes.get(after_this) == Some(&b'.') {
                    return false;
                }
                return check_identifier_type_arg_pattern(bytes, pos);
            }

            // A type-operator keyword is keyword-then-operand: the follow token is its
            // operand, so the follow-token filter's `_ => false` default would reject
            // `f<typeof x>()` and `f<keyof U>()`. Each operator asks its own
            // operand-shape question instead — see `type_operator_commits`.
            Some(TypeKeywordKind::Operator(op)) => {
                return type_operator_commits(bytes, op, pos);
            }

            None => {}
        }

        // Dispatch based on first token after '<'
        match bytes[pos] {
            // Identifier: type reference like `<T>` or `<Ns.Type>`
            _ if identifier_starts_at(bytes, pos) => check_identifier_type_arg_pattern(bytes, pos),

            // A type argument starting with `(`: a function type (`<(x: T) => R>`,
            // `<() => R>`) or a parenthesized type (`<(A | B) & C>`,
            // `<(() => void) | null>`). `is_function_type_start` fast-paths the arrow
            // shapes; otherwise skip the balanced group and ask the shared follow-token
            // filter (as the `{`/`[`/literal arms do), so `x < (b)`, `x < (b) > c` and
            // `x < (a) ? q : r > (t, u)` stay comparisons while `callee<(T)>(…)` and
            // `x < (b) > (c)` are type arguments — matching acorn's
            // `canFollowTypeArgumentsInExpression`.
            b'(' => {
                is_function_type_start(bytes, pos)
                    || matching_delimiter_close(bytes, pos)
                        .is_some_and(|close| type_operand_follow_commits(bytes, close + 1))
            }

            // A second `<` — the tail of a `<<` shift token, or a spaced
            // `< <` — can only open a generic function type
            // (`f<<T>(v: T) => void>()`); shift chains (`a << b > c`) never
            // match its `>`-then-`(` shape.
            b'<' => is_generic_function_type_start(bytes, pos + 1),

            // Object/tuple literal types — but `{`/`[` equally start object and array
            // *value* literals, so `x < {a: 1}` is a comparison. Skip the balanced
            // group, then only a type-continuing follow token commits: `f<{ a: T }>()`
            // and `f<[T, U] | null>()` are type arguments, `x < [1] ? q : r > (t, u)`
            // is a comparison whatever sits past the would-be closing `>`.
            b'{' | b'[' => matching_delimiter_close(bytes, pos)
                .is_some_and(|close| type_operand_follow_commits(bytes, close + 1)),

            // String literal types — the same follow-token question after the literal:
            // `f<'a' | 'b'>()` commits, `x < 'a' + 'b' > (t, u)` stays a comparison.
            b'\'' | b'"' => skip_trivia(bytes, pos, bytes.len(), TriviaProfile::JS)
                .is_some_and(|after| type_operand_follow_commits(bytes, after)),

            // Template literal types — skipped interpolation-aware (the opaque
            // quote-to-quote trivia scan would mis-pair backticks across a nested
            // `` `${`x`}` ``), then the same follow-token question.
            b'`' => {
                let after = skip_template_literal(bytes, pos, bytes.len());
                type_operand_follow_commits(bytes, after)
            }

            // Numeric literal types: `<42>`, `<-1>`, `<.5>` — but `x < 42` is a
            // comparison, so the literal alone decides nothing. Skip it (sign and a
            // missing integer part included: `DecimalLiteral :: . DecimalDigits` is a
            // literal type like any other, so `f<.5>()` is an instantiation to acorn;
            // `-b` skips nothing and is a unary negation, never a type), then only a
            // type-continuing follow token commits: `f<-1>()`, `f<.5>()` and
            // `f<0 | 1>()` are type arguments, `x < 1 + 2 > (t, u)` and
            // ``x < .5 ? q : r > `t` `` stay comparisons.
            //
            // The head class is [`numeric_literal_starts_at`], shared with the index and
            // operand sites so a literal cannot be admitted at one and refused at another.
            _ if numeric_literal_starts_at(bytes, pos) => {
                let after = skip_numeric_literal(bytes, pos);
                after > pos && type_operand_follow_commits(bytes, after)
            }

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
}

/// Whether the type-operator keyword `op`, spelled at `kw_start`, opens type arguments.
///
/// Each operator takes a different operand class, measured against acorn's own type
/// parser, so this dispatches on the operator's IDENTITY rather than re-deriving it from
/// the keyword's first byte — a new [`TypeOperator`] then fails to compile here instead
/// of silently inheriting whichever arm a byte match happened to fall into.
///
/// No operand ⇒ nothing commits: a bare operator keyword is never a complete type, and
/// (unlike an atom) cannot head a qualified type name either — acorn reads
/// `p < keyof.a > (t, u)` as a comparison on the VALUE `keyof.a` and `p < keyof > (t, u)`
/// as one on the value `keyof`. All four contextual operators are ordinary names there;
/// the reserved `typeof`'s no-operand shapes are errors in both readings.
fn type_operator_commits(bytes: &[u8], op: TypeOperator, kw_start: usize) -> bool {
    let after_kw = skip_whitespace_and_comments(bytes, skip_identifier(bytes, kw_start));

    match op {
        // acorn speculatively parses ANY type operand (`p < keyof - 1 > `t`` and
        // `p < unique [0] > (t, u)` are instantiations), so once an operand can start,
        // only the closing-`>` scan decides.
        TypeOperator::Keyof | TypeOperator::Unique => {
            can_start_type_operand(bytes, after_kw)
                && scan_for_closing_angle_bracket(bytes, kw_start)
        }

        // The operand is an entity name (`x`, `Ns.x`) or an `import('m')` head with a
        // member tail, and nothing else (`p < typeof 1 > (t, u)` and
        // `p < typeof [0] ? q : r > `t`` are comparisons — `typeof` takes any
        // *expression* operand but only an entity-name *type* operand). Skip the operand
        // and ask the shared follow filter, so `p < typeof x ? q : r > `t`` stays a
        // comparison while `f<typeof x>()` commits.
        TypeOperator::Typeof => {
            if !identifier_starts_at(bytes, after_kw) {
                return false;
            }
            let head_end = skip_identifier(bytes, after_kw);
            let after_head = if is_word_at(bytes, after_kw, b"import") {
                let paren = skip_whitespace_and_comments(bytes, head_end);
                if bytes.get(paren) == Some(&b'(') {
                    match matching_delimiter_close(bytes, paren) {
                        Some(close) => close + 1,
                        None => return false,
                    }
                } else {
                    head_end
                }
            } else {
                head_end
            };
            type_operand_follow_commits(bytes, skip_qualified_tail(bytes, after_head))
        }

        // The operand is a lone binding identifier (never qualified). Skip it and ask the
        // shared follow filter: a constraint commits through the filter's `extends` arm
        // (`f<infer T extends U ? A : B>()`), while `p < infer - 1 > `t`` — no identifier
        // at all — is a comparison on the value `infer`.
        TypeOperator::Infer => {
            identifier_starts_at(bytes, after_kw)
                && type_operand_follow_commits(bytes, skip_identifier(bytes, after_kw))
        }

        // The operand is an array/tuple type: an element type reference (`readonly T[]`,
        // `readonly Ns.T[]`) or a tuple literal (`readonly [T, U]`); a parenthesized
        // operand is a call in a comparison to acorn (`p < readonly (x) > (t, u)`), and a
        // literal one is no type at all. Skip the operand and ask the shared follow filter
        // — its indexed arm is what tells `readonly zz[] ? q : r` (a comparison) from
        // `f<readonly zz[]>()`.
        TypeOperator::Readonly => {
            let after_head = if identifier_starts_at(bytes, after_kw) {
                skip_qualified_tail(bytes, skip_identifier(bytes, after_kw))
            } else if bytes.get(after_kw) == Some(&b'[') {
                match matching_delimiter_close(bytes, after_kw) {
                    Some(close) => close + 1,
                    None => return false,
                }
            } else {
                return false;
            };
            type_operand_follow_commits(bytes, after_head)
        }
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
/// qualified name (e.g., `Ns.Type.Sub`), the shared follow-token filter
/// [`type_operand_follow_commits`] decides.
fn check_identifier_type_arg_pattern(bytes: &[u8], pos: usize) -> bool {
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
            if is_construct_type_start(bytes, after) && scan_for_closing_angle_bracket(bytes, pos) {
                return true;
            }
            // A bare `abstract` is an ordinary type reference — fall through.
        }
        _ => {}
    }

    // Skip any qualified parts past the leading identifier (already located as
    // `end`, e.g. `Namespace.Type.SubType`), then ask the shared follow filter.
    type_operand_follow_commits(bytes, skip_qualified_tail(bytes, end))
}

/// Whether the first significant token at/after `after_operand` can CONTINUE a
/// type-argument list past a complete first operand — the follow-token filter every
/// operand-headed arm of [`Parser::is_type_arguments_start`] shares: the identifier
/// arm asks it past the qualified name, the atom-keyword, literal (`{…}`, `[…]`,
/// string, template, numeric) and parenthesized-group arms past their operand. One
/// emitter for one question — a head whose filter drifted from the others would
/// answer the same source two ways.
///
/// Anything outside the commit set (`?`, arithmetic, `.`, a second literal, …)
/// cannot continue a type, so the `<` is the less-than operator however the bytes
/// past the would-be closing `>` read — which is what keeps
/// ``p < string ? q : r > `t` `` and `x < 1 + 2 > (t, u)` comparisons (matching
/// acorn) even though a template tag / `(` does not start an expression and would
/// otherwise let the closing-`>` scan commit.
fn type_operand_follow_commits(bytes: &[u8], after_operand: usize) -> bool {
    let pos = skip_whitespace_and_comments(bytes, after_operand);
    if pos >= bytes.len() {
        return false;
    }

    match bytes[pos] {
        // `||` and `&&` are logical operators, NOT type operators (`a || b`, not args)
        b'|' | b'&' if pos + 1 < bytes.len() && bytes[pos + 1] == bytes[pos] => false,

        // After the operand: `>` closes the list, `<` opens a nested one (`<A<B>>`),
        // and `,` `|` `&` separate args. Each is confirmed by scanning
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
            check_indexed_type_pattern(bytes, pos) && scan_for_closing_angle_bracket(bytes, pos)
        }

        // Type constraint: `T extends U`. Whole-word — an identifier that merely
        // starts with `extends` is an ordinary operand (`a < b` ⏎ `extendsFoo()`,
        // where ASI ends the statement) — and confirmed by the closing-`>` scan
        // like every sibling arm.
        b'e' if is_word_at(bytes, pos, b"extends") => scan_for_closing_angle_bracket(bytes, pos),

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
fn check_indexed_type_pattern(bytes: &[u8], pos: usize) -> bool {
    let inside = skip_whitespace_and_comments(bytes, pos + 1);
    let Some(&first) = bytes.get(inside) else {
        return false;
    };

    match first {
        // Empty brackets `T[]` — array type
        b']' => true,

        // Numeric literal index: `T[0]`, `T[-1]`, `T[.5]`, `T[0 | 1]`. A numeric
        // literal is as valid a type as it is an array index, so the literal alone
        // decides nothing — what FOLLOWS it does, under the same rule a reference
        // index answers to. The head class is [`numeric_literal_starts_at`], the
        // caller's own: an index read more narrowly than a head is the same question
        // answered twice (`f<.5>()` and `f<A[.5]>()` are both instantiations to
        // acorn, and only this arm sees the second).
        _ if numeric_literal_starts_at(bytes, inside) => {
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
        _ if identifier_starts_at(bytes, inside) => {
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

/// Classify the TypeScript type keyword at `pos`, if any.
///
/// Called on every `<`/`<<` disambiguation in the postfix loop, so ordinary
/// relational comparisons (`i < n`) and shifts hit it — keep it cheap. A first-byte
/// `match` dispatches to only the same-initial-letter candidate(s), so a byte that
/// can't begin any of the 20 keywords (a digit, `(`, `[`, a quote, or an identifier
/// starting with one of the other 14 letters) bails in O(1) instead of scanning all
/// 20. No keyword is a prefix of another, so at most one can match at a position.
///
/// The [`TypeKeywordKind`] split is what the caller dispatches on: an **atom** is a
/// complete type by itself and takes the identifier arm's follow-token filter (a
/// bare closing-`>` scan over-rejected ``p < string ? q : r > `t` `` — `?` cannot
/// continue a type, so the line is a conditional to acorn); an **operator** is
/// keyword-then-operand, where that filter's `_ => false` default would reject
/// `f<typeof x>()`, so its arm asks a per-operator operand question instead (see
/// [`type_operator_commits`]).
///
/// The whole-word test is [`is_word_at`], the same one every other keyword lookahead
/// asks. A hand-rolled boundary gets two character classes wrong in the same
/// direction: `$` read as *ending* the word (so `string$` matches `string`), and
/// every byte `>= 0x80` too (so `stringµ` does). Either way an
/// ordinary identifier went down the keyword arm rather than the identifier arm's
/// filter [`check_identifier_type_arg_pattern`].
///
/// ⚠️ A mis-dispatch only bites where the token past the would-be closing `>` does
/// **not** start an expression, because an expression-starting one disqualifies the
/// type-argument reading in *both* arms. So `a < string$ ? b : c > d` parsed fine
/// even with the bad boundary, while ``a < string$ ? b : c > `t` `` and
/// `a < string$ ? b : c > (d, e)` were parse errors — a case that reaches only one
/// arm is the only case a mis-dispatch can be observed through, and the fixtures pin
/// those rather than the inert form:
/// [less_than_keyword_boundary](../../../../tests/fixtures/typescript/syntax/disambiguation/less_than_keyword_boundary/)
/// (the word boundary),
/// [less_than_keyword_follow](../../../../tests/fixtures/typescript/syntax/disambiguation/less_than_keyword_follow/)
/// (the atom/operator follow-token split).
#[inline]
fn type_keyword_at(bytes: &[u8], pos: usize) -> Option<TypeKeywordKind> {
    use TypeKeywordKind::{Atom, Operator, This};
    use TypeOperator::{Infer, Keyof, Readonly, Typeof, Unique};
    // Full keyword match at `pos`, not part of a longer identifier.
    let kw = |k: &[u8]| is_word_at(bytes, pos, k);
    match bytes.get(pos) {
        Some(b'n') if kw(b"never") || kw(b"number") || kw(b"null") => Some(Atom),
        Some(b's') if kw(b"string") || kw(b"symbol") => Some(Atom),
        Some(b'b') if kw(b"boolean") || kw(b"bigint") => Some(Atom),
        Some(b'a') if kw(b"any") => Some(Atom),
        Some(b'u') if kw(b"unknown") || kw(b"undefined") => Some(Atom),
        Some(b'u') if kw(b"unique") => Some(Operator(Unique)),
        Some(b'v') if kw(b"void") => Some(Atom),
        Some(b'o') if kw(b"object") => Some(Atom),
        Some(b't') if kw(b"this") => Some(This),
        Some(b't') if kw(b"true") => Some(Atom),
        Some(b't') if kw(b"typeof") => Some(Operator(Typeof)),
        Some(b'f') if kw(b"false") => Some(Atom),
        Some(b'k') if kw(b"keyof") => Some(Operator(Keyof)),
        Some(b'i') if kw(b"infer") => Some(Operator(Infer)),
        Some(b'r') if kw(b"readonly") => Some(Operator(Readonly)),
        _ => None,
    }
}

/// How a TypeScript type keyword heads a type, which decides the follow-token
/// question [`Parser::is_type_arguments_start`] asks after matching one.
#[derive(Clone, Copy)]
enum TypeKeywordKind {
    /// A complete type by itself (`string`, `null`, `true`, …) — equally a value
    /// name, so it takes the identifier arm's follow-token filter.
    Atom,
    /// `this` — an [`Atom`](TypeKeywordKind::Atom) that additionally cannot head a
    /// qualified type name. Split out rather than re-tested at the use site: the
    /// classifier already matched the word, and asking again by prefix is the same
    /// re-derivation [`TypeOperator`] exists to avoid.
    This,
    /// A keyword-then-operand type head — the follow token is its operand, so the
    /// identifier filter's `_ => false` default cannot apply; [`type_operator_commits`]
    /// asks the operand-shape question this operator answers to.
    Operator(TypeOperator),
}

/// The five keyword-then-operand type heads. Carried by [`TypeKeywordKind::Operator`]
/// rather than re-derived from the keyword's first byte, so the operand-class dispatch in
/// [`type_operator_commits`] is exhaustive: the byte form needed `u`-initial operators to
/// be only `unique` (`unknown`/`undefined` being atoms) and gave `readonly` the catch-all
/// arm, which a sixth operator would have joined silently.
#[derive(Clone, Copy)]
enum TypeOperator {
    Keyof,
    Unique,
    Typeof,
    Infer,
    Readonly,
}

/// Skip the qualified tail of a name — the `.Seg` chain of `Ns.Type.Sub` — starting just
/// past the head identifier, returning the position of the first significant byte after
/// the last segment. The `typeof` and `readonly` operator arms walk their entity-name
/// operands with the same steps.
///
/// `pos` advances only over a COMPLETE `.Ident` segment, so "just past a name" holds by
/// construction. A `.` with no identifier behind it ends no name — `TypeName` is
/// `IdentifierReference | NamespaceName . IdentifierReference` — so it is left where it
/// is, for the caller's follow check to read as the token it is. tsc consumes such a `.`
/// (`parseOptional(DotToken)`, then a synthesized missing identifier), but that is its
/// error RECOVERY, which a non-recovering lookahead has no use for: the same `.` ends no
/// EXPRESSION either (`MemberExpression . IdentifierName`), so every input that reaches
/// this branch is rejected on both readings and the two spellings cannot disagree.
fn skip_qualified_tail(bytes: &[u8], after_head: usize) -> usize {
    let mut pos = skip_whitespace_and_comments(bytes, after_head);
    while bytes.get(pos) == Some(&b'.') {
        let after_dot = skip_whitespace_and_comments(bytes, pos + 1);
        if !identifier_starts_at(bytes, after_dot) {
            break;
        }
        pos = skip_whitespace_and_comments(bytes, skip_identifier(bytes, after_dot));
    }
    pos
}

/// Whether the token at `pos` can BEGIN a type — the same head classes
/// [`Parser::is_type_arguments_start`]'s own dispatch admits: an identifier or keyword,
/// a group or literal opener, a leading `|`/`&`, or a numeric sign or point. The
/// `keyof`/`unique` arm peeks this to tell `f<keyof U>()` (an operand follows — the
/// closing-`>` scan decides) from `p < keyof ? q : r` (no operand can start at `?`, so
/// `keyof` is an ordinary value name and nothing can commit).
///
/// ⚠️ The class must stay the dispatch's own. A head admitted there and refused here
/// reads the same literal two ways, and the second reading is silent: it was what left
/// `f<keyof .5>()` an over-rejection while `f<.5>()` and `f<keyof -1>()` — the same
/// literal one byte apart, and the same literal one operator apart — both parsed. The
/// numeric class is [`numeric_literal_starts_at`] for that reason, and its `.` gate is
/// load-bearing HERE specifically: this is the one site where a `.` has a live second
/// reading.
fn can_start_type_operand(bytes: &[u8], pos: usize) -> bool {
    match bytes.get(pos) {
        Some(b'(' | b'<' | b'{' | b'[' | b'\'' | b'"' | b'`' | b'|' | b'&') => true,
        Some(_) => numeric_literal_starts_at(bytes, pos) || identifier_starts_at(bytes, pos),
        None => false,
    }
}

/// Whether a numeric literal begins at `pos` — the head test the type-argument
/// lookahead's three numeric sites share: the dispatch's own literal arm, an indexed
/// access's index ([`check_indexed_type_pattern`]), and a `keyof`/`unique`
/// operand ([`can_start_type_operand`]).
///
/// ⚠️ A `.` opens a literal only where a DIGIT follows it. Anywhere else the `.` is a
/// member tail on whatever precedes it, and at an operator keyword BOTH readings are
/// live: `f<keyof .5>()` is an instantiation over the literal type `.5`, while
/// `p < keyof.a > (t, u)` is a COMPARISON on the value `keyof.a` — an operator keyword
/// cannot head a qualified type name. Admitting a bare `.` there commits the lookahead
/// and turns that comparison into a parse error, which is why the three sites ask one
/// predicate rather than three byte classes:
/// [less_than_keyword_member_prettier_divergence](../../../../tests/fixtures/typescript/syntax/disambiguation/less_than_keyword_member_prettier_divergence/)
/// pins the comparison,
/// [less_than_numeric_leading_dot](../../../../tests/fixtures/typescript/syntax/disambiguation/less_than_numeric_leading_dot/)
/// the instantiation.
///
/// The sign is admitted unconditionally, as every arm's `-` handling always has: `-b` is
/// a unary negation, and [`skip_numeric_literal`] reports that by skipping nothing, which
/// each caller checks (`after > pos`).
#[inline]
fn numeric_literal_starts_at(bytes: &[u8], pos: usize) -> bool {
    match bytes.get(pos) {
        Some(b'0'..=b'9' | b'-') => true,
        Some(b'.') => matches!(bytes.get(pos + 1), Some(b'0'..=b'9')),
        _ => false,
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
