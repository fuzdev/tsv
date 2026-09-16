// Type-argument byte-scan lookahead: disambiguates `<Type, ...>` from the
// less-than operator without lexing, by scanning raw source bytes after `<`.

use super::Parser;
use super::expression_lookahead::{
    has_line_terminator_between, is_construct_type_start, is_function_type_start,
    is_generic_function_type_start, matching_delimiter_close, scan_for_closing_angle_bracket,
};
use super::scan::{
    identifier_starts_at, is_word_at, skip_identifier, skip_numeric_literal,
    skip_whitespace_and_comments,
};
use tsv_lang::source_scan::{TriviaProfile, skip_template_literal, skip_trivia};

/// Which reading of the type-argument lookahead a caller wants — the one parameter the two
/// readings are threaded through, so the walk stays a single body and they cannot drift.
///
/// Both readings ask the same structural question. They differ in WHICH TEXT they ask it of
/// and WHOSE grammar answers: [`Parse`] grades the source bytes as the author wrote them
/// against acorn-typescript, tsv's AST drop-in oracle; [`Relex`] grades the form the printer
/// would emit from them against every parser that will read tsv's output — tsc, and tsv's
/// own [`Parse`].
///
/// **Four tokens the printer MOVES**, each a place the printed form is not the source:
///
/// - **a line break before a `[`, which the printer folds away.** A break there ends the type
///   to tsc and acorn-typescript alike (the loop in each is named in
///   `docs/conformance_prettier_ts.md` §Relational chain type-argument parens), so
///   `fn<A⏎[T]>(t, u)` is a comparison chain — and every parse-then-format entry point
///   folds that break (`tsv_lang::printing::normalize_carriage_returns`, plus the printer's
///   own soft-line joins) before the output is read back. [`type_operand_follow_commits`]'s
///   `[` arm, and [`continues_as_type`]'s index-chain twin.
/// - **trivia between a numeric type's SIGN and its digits, likewise folded.** `fn<-⏎⏎1>(t)`
///   is a comparison chain whose printed form is `fn < -1 > t`, one literal type in the
///   region ([`skip_signed_numeric_literal`]).
/// - **a paren shell, which the printer strips**, read at both ends of the region. Past an
///   operand, a `)` the region did not open is not in the printed form, so it neither ends
///   the operand nor ends the scan ([`skip_relex_operand_suffixes`], and
///   [`matching_angle_close`](super::expression_lookahead::matching_angle_close) mid-scan) —
///   without which the pair this reading justifies would erase itself on the next pass,
///   since tsv's own output is the shell-bearing spelling (`(a < b) > c`). At the HEAD, the
///   shell is looked THROUGH and the head question asked of its content
///   ([`type_arg_head_commits`]'s `(` arm), so `a < (arr[b - 1]) > c` grades the arithmetic
///   its own paren-free twin does.
/// - **a line break past the `>`, which the printer may or may not add.** Past one, tsv's
///   own [`Parse`] (with acorn-typescript) commits the list ahead of any expression. Which
///   followers commit, for tsc and for acorn-typescript, is stated once in the same catalog
///   entry; where tsc refuses a follower that [`Parse`] commits on, the pair is owed to
///   soundness (below), not to tsc. Whether the printer takes the break is unknowable where
///   parens are decided, so [`Relex`] asks about the REGION alone and skips the follower
///   entirely ([`scan_for_closing_angle_bracket`]).
///
/// **One token the two ORACLES read differently**, no printer move involved: the non-null
/// `!`. `T!` is tsc's `JSDocNonNullableType` — prefix and postfix — so `<b!>` is a
/// type-argument list to the compiler, where acorn-typescript reads a comparison and tsv's
/// parse follows it. Admitting it at [`Parse`] would move tsv's AST off the drop-in
/// contract; refusing it at [`Relex`] would emit an output tsc reads as a different program
/// ([`skip_relex_operand_suffixes`] for the postfix, [`type_arg_head_commits`]'s `!` arm for
/// the prefix).
///
/// **Two properties bound every relaxation above, and neither is "[`Relex`] is never
/// stricter than [`Parse`] on one string".** That comparison grades both readings against
/// the SAME bytes, which is a proxy — it holds only where the source and the printed form
/// give [`Parse`] the same answer at that `<`, and the whole reason [`Relex`] exists is the
/// places they do not (a line break, a shell). The two properties name the texts apart:
///
/// - **Soundness.** For every document `D`: if tsv's own [`Parse`] reads a type-argument
///   region at the `<` of `format(D)`, the printer must have put a pair there. tsv's parser
///   is one of the readers of the output the verdict is about, so a `<` [`Relex`] leaves
///   bare and [`Parse`] then claims is an output tsv cannot reparse. **Nothing gates this
///   today**: `gaps:audit` grades the fixture tree as authored plus its injections, and no
///   fixture can hold the shapes that break it — a fixture input must format to itself, and
///   the broken form is one only the printer produces.
/// - **Independence.** [`Relex`] must agree with ITSELF across the redundant-paren
///   spellings of one program: `a < X > c` and `a < (X) > c` are one document, so they owe
///   one verdict. Gated by `deno task paren:audit` (its `< > chain` and `< > operand`
///   classes). This is soundness's dual: a `(` arm that graded nothing would give a
///   parenthesized operand a pair its bare twin lacks.
///
/// The bracketed heads are where soundness bites first: neither grades its body, so a `>`
/// with a line terminator past it — or a `(` or a template on the same line — commits them on
/// any content, and tsv reads `x < { ...s } >⏎c` and `x < { ...s } > (c, d)` as type-argument
/// lists and then rejects both — where acorn and tsc both read the comparison chain. Grading the body under [`Relex`] alone (a tuple and a type
/// literal are types, an array of update expressions and an object spread are not) would
/// therefore print a bare chain tsv itself cannot reparse. The pair stands on every
/// bracketed head instead; the underlying over-rejection is [`Parse`]'s, and belongs to the
/// parse oracles.
///
/// **The `(` arm's look-through carries soundness only where the printer STRIPS the shell.**
/// Where it does, the content IS the printed bytes and the two readings are asking about one
/// text. Where the printer **keeps** the shell, they are not: [`Relex`] answers about the
/// content while [`Parse`] answers about the `(`-headed region that is actually printed, and
/// on any content that is no type they disagree — [`Relex`] declines the pair and `format`
/// emits a document [`Parse`] then claims as a type-argument list. Two triggers reach it, the
/// same two the bracketed heads have: a follower that commits the head with no break at all
/// (`x < (a = b) > (c, d)`, or a template or arrow-in-parens follower, is unreparseable on one
/// line), and a width at which the `>` ends a line (`x < (a = b) > c` is harmless only while
/// it fits). The class is every operand whose shell survives printing and whose head and
/// follow token are no type — assignment and compound assignment, a conditional, `||` /
/// `&&` / `??`, equality, `^`, `in` / `instanceof`, `await`, `yield`, `as` / `satisfies`. A
/// kept shell whose content happens to SPELL a type is not in it: a sequence spells the
/// argument separator, `&` and `|` a type intersection or union, and `() =>` a function
/// type, so [`Relex`] commits and the pair stands. The residual is the same [`Parse`]-side
/// head over-rejection as the bracketed ones, and it is the parse oracles' subject.
///
/// [`Parse`]: TypeArgScan::Parse
/// [`Relex`]: TypeArgScan::Relex
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TypeArgScan {
    /// What the PARSER commits to: the input's own bytes, line terminators and parens and
    /// all.
    Parse,
    /// What the PRINTED form would re-lex as, which is the printer's question. Never decides
    /// a parse; it only records whether a paren pair has to stand between two tokens (see
    /// `Printer::needs_parens_binary_operand`).
    Relex,
}

impl TypeArgScan {
    /// Whether this reading grades the source bytes AS WRITTEN, against acorn. False for
    /// [`Relex`](TypeArgScan::Relex), which grades the printed form — whose readers are
    /// named on [`TypeArgScan`], along with the sites that read each difference.
    #[inline]
    pub(super) const fn reads_source_as_written(self) -> bool {
        matches!(self, TypeArgScan::Parse)
    }
}

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
    ///
    /// `scan` names which of the two readings the caller wants — see [`TypeArgScan`]. The
    /// parser always asks [`TypeArgScan::Parse`]; the one [`TypeArgScan::Relex`] asker is a
    /// `>` reducing over a `<` left operand, recording on that `<` node whether its PRINTED
    /// form would re-lex as a type-argument list.
    pub(super) fn is_type_arguments_start(&self, scan: TypeArgScan) -> bool {
        self.is_type_arguments_start_at(self.current.start as usize, scan)
    }

    /// [`Parser::is_type_arguments_start`] asked at an EXPLICIT `<`, rather than at the
    /// token the parser is sitting on.
    ///
    /// The relaxed [`TypeArgScan::Relex`] reading is taken at a `>` — the point at which
    /// both tokens of the join it grades exist — by which time the lexer has long left the
    /// `<` behind, so that caller supplies the offset itself.
    pub(super) fn is_type_arguments_start_at(&self, lt_start: usize, scan: TypeArgScan) -> bool {
        let bytes = self.source.as_bytes();

        // Must start with '<'
        if lt_start >= bytes.len() || bytes[lt_start] != b'<' {
            return false;
        }

        // Skip whitespace AND comments after '<' - comments can appear before types
        let pos = skip_whitespace_and_comments(bytes, lt_start + 1);
        pos < bytes.len() && type_arg_head_commits(bytes, pos, scan)
    }
}

/// Whether the type-argument HEAD at `pos` — the first significant byte past a `<` —
/// opens a type-argument list, dispatching on that byte. Split out of
/// [`Parser::is_type_arguments_start`] because the `(` arm re-enters it: under
/// [`TypeArgScan::Relex`] a paren shell the printer strips is not in the graded form, so
/// the head question is asked again of its CONTENT.
fn type_arg_head_commits(bytes: &[u8], pos: usize, scan: TypeArgScan) -> bool {
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
            return check_identifier_type_arg_pattern(bytes, pos, scan);
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
            return check_identifier_type_arg_pattern(bytes, pos, scan);
        }

        // A type-operator keyword is keyword-then-operand: the follow token is its
        // operand, so the follow-token filter's `_ => false` default would reject
        // `f<typeof x>()` and `f<keyof U>()`. Each operator asks its own
        // operand-shape question instead — see `type_operator_commits`.
        Some(TypeKeywordKind::Operator(op)) => {
            return type_operator_commits(bytes, op, pos, scan);
        }

        None => {}
    }

    // Dispatch based on first token after '<'
    match bytes[pos] {
        // Identifier: type reference like `<T>` or `<Ns.Type>`
        _ if identifier_starts_at(bytes, pos) => {
            check_identifier_type_arg_pattern(bytes, pos, scan)
        }

        // A type argument starting with `(`: a function type (`<(x: T) => R>`,
        // `<() => R>`) or a parenthesized type (`<(A | B) & C>`,
        // `<(() => void) | null>`). `is_function_type_start` fast-paths the arrow
        // shapes; otherwise skip the balanced group and ask the shared follow-token
        // filter (as the `{`/`[`/literal arms do), so `x < (b)`, `x < (b) > c` and
        // `x < (a) ? q : r > (t, u)` stay comparisons while `callee<(T)>(…)` and
        // `x < (b) > (c)` are type arguments — matching acorn-typescript's follower handling
        // (quoted in `docs/conformance_prettier_ts.md` §Relational chain type-argument parens).
        //
        // Past the arrow fast path, [`TypeArgScan::Relex`] LOOKS THROUGH the shell instead
        // of skipping it: a redundant pair is not in the form that reading grades
        // (`a < (arr[b - 1]) > c` prints as `a < arr[b - 1] > c`), so the head question is
        // the CONTENT's, asked with the content's own first byte. Skipping to the follow
        // filter would grade nothing at all there — the filter is the region's only
        // discriminator and [`TypeArgScan::Relex`] deletes its follow-token half, so every
        // parenthesized operand would commit, type or not. The closing side needs no rule
        // of its own: [`skip_relex_operand_suffixes`] already steps over the `)`.
        b'(' => {
            if is_function_type_start(bytes, pos) {
                true
            } else if scan.reads_source_as_written() {
                matching_delimiter_close(bytes, pos)
                    .is_some_and(|close| type_operand_follow_commits(bytes, close + 1, scan))
            } else {
                type_arg_head_commits(bytes, skip_whitespace_and_comments(bytes, pos + 1), scan)
            }
        }

        // A non-null `!` ahead of the operand — `<!T>` is tsc's JSDoc non-nullable type,
        // so `a < !b >⏎c` is an instantiation plus a statement where the flat line is a
        // comparison chain. Only [`TypeArgScan::Relex`] reads it: acorn-typescript, tsv's
        // parse oracle, has no such type, so admitting it at [`TypeArgScan::Parse`] would
        // move a PARSE off the drop-in contract. `!=` / `!==` are operators, never a mark.
        b'!' if !scan.reads_source_as_written() && bytes.get(pos + 1) != Some(&b'=') => {
            type_arg_head_commits(bytes, skip_whitespace_and_comments(bytes, pos + 1), scan)
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
        //
        // Neither brace nor bracket is a shell the printer strips, so
        // [`TypeArgScan::Relex`] cannot look through one the way the `(` arm does — and it
        // deliberately does not grade the BODY either, though tsc's answer turns on it
        // (`<[1, 2]>` and `<{ x: B }>` are types to the compiler, `<[b, c++]>` and
        // `<{ ...s }>` are not). Grading it would make the relaxed reading STRICTER than
        // this one on the same bytes, and that direction is unsound: the head arm here
        // commits on any matching `>` that a line terminator, a `(` or a template follows, so
        // tsv's own parser reads `x < { ...s } >⏎c` as a type-argument list and then REJECTS
        // it — where acorn and tsc both read the comparison chain. A bare output would be one
        // tsv cannot reparse, so the pair stands on every bracketed head. See
        // [`TypeArgScan`]'s soundness property.
        b'{' | b'[' => matching_delimiter_close(bytes, pos)
            .is_some_and(|close| type_operand_follow_commits(bytes, close + 1, scan)),

        // String literal types — the same follow-token question after the literal:
        // `f<'a' | 'b'>()` commits, `x < 'a' + 'b' > (t, u)` stays a comparison.
        b'\'' | b'"' => skip_trivia(bytes, pos, bytes.len(), TriviaProfile::JS)
            .is_some_and(|after| type_operand_follow_commits(bytes, after, scan)),

        // Template literal types — skipped interpolation-aware (the opaque
        // quote-to-quote trivia scan would mis-pair backticks across a nested
        // `` `${`x`}` ``), then the same follow-token question.
        b'`' => {
            let after = skip_template_literal(bytes, pos, bytes.len());
            type_operand_follow_commits(bytes, after, scan)
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
            let after = skip_signed_numeric_literal(bytes, pos, scan);
            after > pos && type_operand_follow_commits(bytes, after, scan)
        }

        // A leading `|`/`&` on the first union/intersection member
        // (`f<| A | B>()`, `f<& A & B>()`) — the form prettier itself emits
        // whenever such a type argument breaks across lines. Neither byte can
        // start an expression, so a `<` followed by one is never a comparison;
        // the closing-`>` + follow-token scan still runs, as in every other arm,
        // so an unterminated `<` stays unclaimed.
        b'|' | b'&' => scan_for_closing_angle_bracket(bytes, pos, scan),

        // Not a recognized type argument start
        _ => false,
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
fn type_operator_commits(
    bytes: &[u8],
    op: TypeOperator,
    kw_start: usize,
    scan: TypeArgScan,
) -> bool {
    let after_kw = skip_whitespace_and_comments(bytes, skip_identifier(bytes, kw_start));

    match op {
        // acorn speculatively parses ANY type operand (`p < keyof - 1 > `t`` and
        // `p < unique [0] > (t, u)` are instantiations), so once an operand can start,
        // only the closing-`>` scan decides.
        TypeOperator::Keyof | TypeOperator::Unique => {
            can_start_type_operand(bytes, after_kw)
                && scan_for_closing_angle_bracket(bytes, kw_start, scan)
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
            type_operand_follow_commits(bytes, skip_qualified_tail(bytes, after_head), scan)
        }

        // The operand is a lone binding identifier (never qualified). Skip it and ask the
        // shared follow filter: a constraint commits through the filter's `extends` arm
        // (`f<infer T extends U ? A : B>()`), while `p < infer - 1 > `t`` — no identifier
        // at all — is a comparison on the value `infer`.
        TypeOperator::Infer => {
            identifier_starts_at(bytes, after_kw)
                && type_operand_follow_commits(bytes, skip_identifier(bytes, after_kw), scan)
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
            type_operand_follow_commits(bytes, after_head, scan)
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
fn check_identifier_type_arg_pattern(bytes: &[u8], pos: usize, scan: TypeArgScan) -> bool {
    // The leading identifier's end is located once and reused by the keyword
    // dispatch below and by the qualified-name loop's first step.
    let end = skip_identifier(bytes, pos);

    // Leading keyword forms that start a non-reference type. `import` is always
    // a valid type (scan decides); `new`/`abstract new` require the construct
    // shape so `f<new B()>(x)` and `a < new B() > (c)` stay comparisons.
    match &bytes[pos..end] {
        b"import" => return scan_for_closing_angle_bracket(bytes, pos, scan),
        b"new" => {
            return is_construct_type_start(bytes, pos)
                && scan_for_closing_angle_bracket(bytes, pos, scan);
        }
        b"abstract" => {
            let after = skip_whitespace_and_comments(bytes, end);
            if is_construct_type_start(bytes, after)
                && scan_for_closing_angle_bracket(bytes, pos, scan)
            {
                return true;
            }
            // A bare `abstract` is an ordinary type reference — fall through.
        }
        _ => {}
    }

    // Skip any qualified parts past the leading identifier (already located as
    // `end`, e.g. `Namespace.Type.SubType`), then ask the shared follow filter.
    type_operand_follow_commits(bytes, skip_qualified_tail(bytes, end), scan)
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
///
/// The operand's own end moves under [`TypeArgScan::Relex`]: what the printed form does not
/// hold between the operand and this token is stepped over first
/// ([`skip_relex_operand_suffixes`]).
fn type_operand_follow_commits(bytes: &[u8], after_operand: usize, scan: TypeArgScan) -> bool {
    let pos = skip_relex_operand_suffixes(
        bytes,
        skip_whitespace_and_comments(bytes, after_operand),
        scan,
    );
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
        b'>' | b'<' | b',' | b'|' | b'&' => scan_for_closing_angle_bracket(bytes, pos, scan),

        // Indexed type vs array access: `T[K]` vs `arr[0]`. Confirmed by the same
        // closing-`>` scan as the arms above — `T[K]` shaped bytes are equally a
        // member access on a comparison's right operand, so only the matching `>`
        // (and its follow token) tells them apart: `f(a < B[c], d)` and
        // `a < B[c] > d` stay comparisons, `f<A[B], C>(x)` is an instantiation.
        //
        // A `[` past a line terminator is no index at all: the type grammar takes its
        // postfix operators on the operand's own line (tsc and acorn-typescript alike — the
        // loop in each is named in `docs/conformance_prettier_ts.md` §Relational chain
        // type-argument parens), and so does [`Parser::parse_type`]. Committing to
        // type arguments here would hand that parser a `<B⏎[c]>` it stops reading at
        // `B` — `a <⏎B // c⏎[c] >⏎d` is the comparison the same bytes on one line are.
        //
        // That break is exactly what the PRINTER folds away, which is why the
        // [`TypeArgScan::Relex`] reading passes the gate: `fn<A⏎[T]>(t, u)` parses as a
        // comparison chain and prints as `fn < A[T] > (t, u)`, whose region is a
        // type-argument list. See [`TypeArgScan`].
        b'[' => {
            !(scan.reads_source_as_written()
                && has_line_terminator_between(bytes, after_operand, pos))
                && check_indexed_type_pattern(bytes, pos, scan)
                && scan_for_closing_angle_bracket(bytes, pos, scan)
        }

        // Type constraint: `T extends U`. Whole-word — an identifier that merely
        // starts with `extends` is an ordinary operand (`a < b` ⏎ `extendsFoo()`,
        // where ASI ends the statement) — and confirmed by the closing-`>` scan
        // like every sibling arm.
        b'e' if is_word_at(bytes, pos, b"extends") => {
            scan_for_closing_angle_bracket(bytes, pos, scan)
        }

        _ => false,
    }
}

/// Whether the `[` at `pos` can open an indexed-access type rather than an array
/// index. A pre-filter only: every shape that stays grammatical both ways is handed
/// to the caller's closing-`>` scan, which arbitrates.
///
/// - `T[]`, `T["key"]`, `T[keyof U]`, `T[typeof x]`: indexed type
/// - `T[| A | B]`, `T[& A & B]`: a leading union/intersection bar opens only a type
/// - `T[K]`, `T[0]`, `T[-1]` followed by `>`, `,`, or another `[`: indexed type
/// - `T[A | B]`, `T[0 | 1]`, `T[A[B]]`, `T[A.B]`, `T[A<B>]`,
///   `T[A extends B ? C : D]`: the index is itself a type, so the scan decides
/// - `T[(A | B)[]]`, `T[(A)]`: a paren shell around any of the above
/// - `a[b - 1]`, `a[0 + 1]`, `a[c || d]`, `a[c <= d]`: arithmetic, or a
///   logical/shift/relational operator — an expression, never a type → array access
/// - `a[-b]`: a unary negation, not a negative literal → array access
fn check_indexed_type_pattern(bytes: &[u8], pos: usize, scan: TypeArgScan) -> bool {
    let inside = skip_whitespace_and_comments(bytes, pos + 1);
    // Empty brackets `T[]` — array type
    if bytes.get(inside) == Some(&b']') {
        return true;
    }
    index_operand_is_type(bytes, inside, b']', scan)
}

/// Whether the operand at `inside` — the first byte of an index, or of a paren shell
/// inside one — reads as a TYPE up to `closer` (`]` for the index itself, `)` for a
/// shell), on the same rule at both depths so a shell cannot admit what its index
/// refuses, or refuse what it admits.
fn index_operand_is_type(bytes: &[u8], inside: usize, closer: u8, scan: TypeArgScan) -> bool {
    let Some(&first) = bytes.get(inside) else {
        return false;
    };

    match first {
        // A leading `|` / `&` (single — `||` / `&&` are the logical operators) opens a
        // union or intersection and nothing else: no expression begins with one. The
        // union printer's own leading-pipe layout puts one here
        // (`fn<⏎A[⏎| B // c⏎| C]⏎>()`), so its output has to read back as the
        // instantiation it printed.
        b'|' | b'&' => bytes.get(inside + 1) != Some(&first),

        // A paren shell: its content is an index operand by the same rule, closed by `)`,
        // and what follows the shell continues the type or does not (`T[(A | B)[]]`).
        b'(' => matching_delimiter_close(bytes, inside).is_some_and(|close| {
            let content = skip_whitespace_and_comments(bytes, inside + 1);
            index_operand_is_type(bytes, content, b')', scan)
                && continues_as_type(bytes, close + 1, closer, scan)
        }),

        // Numeric literal index: `T[0]`, `T[-1]`, `T[.5]`, `T[0 | 1]`. A numeric
        // literal is as valid a type as it is an array index, so the literal alone
        // decides nothing — what FOLLOWS it does, under the same rule a reference
        // index answers to. The head class is [`numeric_literal_starts_at`], the
        // caller's own: an index read more narrowly than a head is the same question
        // answered twice (`f<.5>()` and `f<A[.5]>()` are both instantiations to
        // acorn, and only this arm sees the second).
        _ if numeric_literal_starts_at(bytes, inside) => {
            let after_number = skip_signed_numeric_literal(bytes, inside, scan);
            // No literal starts here at all (`-b`) — a unary negation, so the index is
            // an expression. Guarding on this is what stops the `-` from swallowing an
            // identifier and landing on the same `]` a real literal ends at.
            if after_number == inside {
                return false;
            }
            continues_as_type(bytes, after_number, closer, scan)
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

            continues_as_type(bytes, after_id, closer, scan)
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
/// past the head identifier, returning the name's own end: the byte after its last
/// segment, ahead of any trivia (every caller hands it to [`type_operand_follow_commits`],
/// which skips the trivia itself and must see it to gate a `[` on the name's line). The
/// `typeof` and `readonly` operator arms walk their entity-name operands with the same
/// steps.
///
/// `end` advances only over a COMPLETE `.Ident` segment, so "just past a name" holds by
/// construction. A `.` with no identifier behind it ends no name — `TypeName` is
/// `IdentifierReference | NamespaceName . IdentifierReference` — so it is left where it
/// is, for the caller's follow check to read as the token it is. tsc consumes such a `.`
/// (`parseOptional(DotToken)`, then a synthesized missing identifier), but that is its
/// error RECOVERY, which a non-recovering lookahead has no use for: the same `.` ends no
/// EXPRESSION either (`MemberExpression . IdentifierName`), so every input that reaches
/// this branch is rejected on both readings and the two spellings cannot disagree.
fn skip_qualified_tail(bytes: &[u8], after_head: usize) -> usize {
    // The name's own end — never the trivia past it, which the follow filter skips
    // itself and reads for a line terminator ahead of a `[`.
    let mut end = after_head;
    loop {
        let dot = skip_whitespace_and_comments(bytes, end);
        if bytes.get(dot) != Some(&b'.') {
            return end;
        }
        let after_dot = skip_whitespace_and_comments(bytes, dot + 1);
        if !identifier_starts_at(bytes, after_dot) {
            return end;
        }
        end = skip_identifier(bytes, after_dot);
    }
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

/// Whether what follows an index operand ending at `operand_end` continues a TYPE
/// rather than an expression, up to `closer` — the `]` of the index, or the `)` of a
/// paren shell inside it.
///
/// Shared by both operand kinds, and it must stay shared: `T[K | J]` and `T[0 | 1]` are
/// the same question, and answering it in one place is what keeps a numeric index from
/// being read more narrowly than a reference one. Takes the operand's own end rather than
/// the next token, because a `[` continuation is legal only on the operand's line (see the
/// `[` arm of [`type_operand_follow_commits`]); a `[` past a line terminator is where the
/// index ends, so it is graded as the "anything else" arm.
fn continues_as_type(bytes: &[u8], operand_end: usize, closer: u8, scan: TypeArgScan) -> bool {
    let after_operand = skip_whitespace_and_comments(bytes, operand_end);
    match bytes.get(after_operand) {
        Some(&c) if c == closer => {
            if closer == b')' {
                // The shell closed; what follows IT continues the index operand
                // (`(A | B)[]`, `(A)`), graded from the `)` as the operand's end.
                return continues_as_type(bytes, after_operand + 1, b']', scan);
            }
            // Past the index's own `]`, the relexing reading steps over what the printed
            // form does not hold between the operand and the token that settles it: a `)`
            // closing a shell the printer strips (`(a < B[c]) > d` prints as
            // `a < B[c] > d`) and a non-null `!`, which is a type to tsc and to no one
            // else (`a < B[c]! > d`). See [`TypeArgScan`].
            let after_close = skip_relex_operand_suffixes(
                bytes,
                skip_whitespace_and_comments(bytes, after_operand + 1),
                scan,
            );
            // Type args end with `>`, continue with `,`, or chain another index group
            // (`T[K][J]`, on the same line) — the caller's closing-`>` scan arbitrates
            // all three
            match bytes.get(after_close) {
                Some(b'>' | b',') => true,
                Some(b'[') => {
                    !(scan.reads_source_as_written()
                        && has_line_terminator_between(bytes, after_operand + 1, after_close))
                }
                _ => false,
            }
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
        Some(b'[') => {
            !(scan.reads_source_as_written()
                && has_line_terminator_between(bytes, operand_end, after_operand))
        }
        Some(b'|' | b'&' | b'.' | b'<') => true,
        Some(b'e') if is_word_at(bytes, after_operand, b"extends") => true,
        // Anything else after the operand (e.g. `b - 1]`) is arithmetic — an expression,
        // never a type, and the one case the closing-`>` scan cannot arbitrate
        // (`a < arr[b - 1] > (c)` is grammatical both ways).
        _ => false,
    }
}

/// Step past what stands between an operand and the token that settles the region, but is
/// invisible to the reading that grades the PRINTED form. Two bytes qualify, and the loop
/// alternates because either can wrap the other (`(b)!`, `(b!)`, `b!!`, `((b))`):
///
/// - a **`)` closing a paren SHELL the printer strips** — not in the graded form, so a `)`
///   the region did not open neither ends an operand nor ends the region (`(a < b) > c`
///   prints as `a < b > c`, whose `<`…`>` region IS a type-argument list). Without it the
///   pair this reading justifies would erase itself on the next pass, since tsv's own
///   output is the shell-bearing spelling.
/// - a **non-null `!`** — `T!` is tsc's JSDoc non-nullable type, so `<b!>` is a
///   type-argument list to the compiler while acorn-typescript, tsv's parse oracle, reads a
///   comparison. `!=` / `!==` are operators and stop the walk.
///
/// The identity under [`TypeArgScan::Parse`], where the same `)` ends a call or a group for
/// real and the same `!` is a non-null assertion on a VALUE.
#[inline]
fn skip_relex_operand_suffixes(bytes: &[u8], mut pos: usize, scan: TypeArgScan) -> usize {
    if scan.reads_source_as_written() {
        return pos;
    }
    loop {
        match bytes.get(pos) {
            Some(b')') => pos = skip_whitespace_and_comments(bytes, pos + 1),
            Some(b'!') if bytes.get(pos + 1) != Some(&b'=') => {
                pos = skip_whitespace_and_comments(bytes, pos + 1);
            }
            _ => return pos,
        }
    }
}

/// Skip a numeric literal with its optional sign — the type-argument lookahead's own
/// wrapper over [`skip_numeric_literal`], reporting "nothing skipped" (the position it was
/// given) where no literal begins, exactly as that function does.
///
/// Under [`TypeArgScan::Relex`] it tolerates trivia between the sign and the digits — one of
/// the printer moves [`TypeArgScan`] enumerates: `fn<-⏎⏎1>(t)` is a comparison chain, whose
/// printed form is `fn < -1 > t` — one literal type in the region, and a `<` that would
/// re-lex. Grading the source bytes there instead takes two passes to reach the pair, which
/// a blank-injection run reads as a non-idempotency.
#[inline]
fn skip_signed_numeric_literal(bytes: &[u8], pos: usize, scan: TypeArgScan) -> usize {
    let end = skip_numeric_literal(bytes, pos);
    if end > pos || scan.reads_source_as_written() || bytes.get(pos) != Some(&b'-') {
        return end;
    }
    let after_sign = skip_whitespace_and_comments(bytes, pos + 1);
    let end = skip_numeric_literal(bytes, after_sign);
    if end > after_sign { end } else { pos }
}
