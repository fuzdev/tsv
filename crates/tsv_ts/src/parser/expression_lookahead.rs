// Expression-specific lookahead helpers for arrow function and type argument disambiguation
//
// These functions scan raw bytes to disambiguate syntactic constructs that
// look similar initially but parse differently:
// - Arrow functions vs parenthesized expressions: `(x) => y` vs `(x)`
// - Generic arrow functions vs comparison: `<T>() => x` vs `a < b`
// - Type arguments vs comparison chain: `foo<T>()` vs `foo < a`
//
// All functions operate on byte slices for performance (no tokenization needed).

use super::expression_type_args::TypeArgScan;
use super::scan::{
    identifier_starts_at, is_word_at, skip_identifier, skip_numeric_literal,
    skip_whitespace_and_comments,
};
use crate::lexer::is_es_line_terminator_at;
use tsv_lang::source_scan::{
    OperandAnchor, PAREN_HOP_NEEDLES, TriviaProfile, is_hop_needle, skip_regex_literal, skip_trivia,
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

/// A matched arrow head's byte extents, built by the parser's three head
/// predicates and read by the consequent-context rule
/// ([`super::Parser::parse_arrow_or_rewind`]) — which asks two questions of it,
/// both keyed on what the match already found so neither re-walks the head.
#[derive(Clone, Copy)]
pub(super) struct ArrowHead {
    /// The `(` opening the parameter list — `None` for a `<T>(…)` head. tsc's
    /// `isParenthesizedArrowFunctionExpressionWorker` answers `Tristate.Unknown`
    /// for every `<` outside JSX, so a generic head never commits; keeping the
    /// fact HERE rather than at the site that must not ask means a fourth head
    /// site cannot forget it.
    paren: Option<usize>,
    /// The `)` closing the parameter list.
    close: usize,
}

impl ArrowHead {
    /// The head opening at the `(` at `paren` — a plain `(a) =>` head, or the
    /// `(` an `async` precedes, which tsc classifies by the same rules.
    pub(super) fn at_paren(bytes: &[u8], paren: usize) -> Option<Self> {
        scan_arrow_head_close(bytes, paren).map(|close| Self {
            paren: Some(paren),
            close,
        })
    }

    /// The head whose parameter list opens at `paren`, behind type parameters
    /// (`<T>(a) =>`). Never commits — see [`ArrowHead::paren`].
    pub(super) fn behind_type_parameters(bytes: &[u8], paren: usize) -> Option<Self> {
        scan_arrow_head_close(bytes, paren).map(|close| Self { paren: None, close })
    }

    /// Whether the head carries a return-type annotation — a `:` right after its
    /// `)`. The consequent-context rule needs this AHEAD of the parse: a head that
    /// then fails to parse as an arrow is rewound only when annotated, so the
    /// parser's own record of a return type (written after the parameters) comes
    /// too late for the head whose parameters were the failure.
    pub(super) fn has_return_type(self, bytes: &[u8]) -> bool {
        bytes.get(skip_whitespace_and_comments(bytes, self.close + 1)) == Some(&b':')
    }

    /// Whether tsc reads this head as a signature without asking — its
    /// `Tristate.True` (see [`paren_head_commits_to_signature`]).
    pub(super) fn commits_to_signature(self, bytes: &[u8]) -> bool {
        self.paren
            .is_some_and(|paren| paren_head_commits_to_signature(bytes, paren))
    }
}

/// tsc's `isModifierKind` set, minus `async` — the words that may open a
/// parameter-property parameter (`(public a)`). `async` is excluded because it is
/// the arrow's own modifier, and because `(async a)` reads as two names.
///
/// Order is alphabetical for reading only; the lookup is a linear compare over fourteen
/// short words on a cold path.
const PARAM_MODIFIERS: &[&[u8]] = &[
    b"abstract",
    b"accessor",
    b"const",
    b"declare",
    b"default",
    b"export",
    b"in",
    b"out",
    b"override",
    b"private",
    b"protected",
    b"public",
    b"readonly",
    b"static",
];

/// Whether the parameter list opening at the `(` at `paren` is one tsc reads as a
/// signature **without asking** — `Tristate.True` from
/// `isParenthesizedArrowFunctionExpressionWorker`, where
/// `tryParseParenthesizedArrowFunctionExpression` parses committed and passes
/// `allowReturnTypeInArrowFunction: true`.
///
/// The distinction is load-bearing for the consequent-context rule, and in BOTH
/// directions. tsc's `true` reaches the arrow's **body**, so an annotated head
/// inside a committed arrow's body keeps its own annotation and the conditional
/// runs out of `:`; speculating on such a head instead truncates the body, finds
/// the `:` the inner annotation would have taken, and keeps the outer arrow — a
/// reading neither tsc nor acorn has. And the converse: a head tsc leaves
/// `Unknown` (`(a)`, `(a, b)`, `(a = 1)`, `([a])`, `({a})`, every `<T>(…)`) has a
/// parenthesized-expression reading, so committing it would reject a program tsc
/// accepts.
///
/// Each arm below is one of tsc's, in its order — a `)`, a binding-pattern open, a
/// rest `...`, a parameter property, then the first name's follower. One
/// deliberate deviation: this reads any identifier-shaped word where tsc asks
/// `isIdentifier()`, so a **reserved** word (`(if: T)`, `(true?: T)`) commits here
/// and does not there. That cannot move a verdict — `(<reserved> :` and
/// `(<reserved> ?` spell no parenthesized expression either, so both readings
/// reject — and it keeps a keyword table out of a byte scan. `this` needs no such
/// argument: tsc admits it explicitly.
fn paren_head_commits_to_signature(bytes: &[u8], paren: usize) -> bool {
    debug_assert_eq!(bytes.get(paren), Some(&b'('));
    let second = skip_whitespace_and_comments(bytes, paren + 1);
    match bytes.get(second) {
        // `()` — a signature when a return type, the `=>` or an error-recovery
        // body brace follows.
        Some(b')') => {
            let third = skip_whitespace_and_comments(bytes, second + 1);
            matches!(bytes.get(third), Some(b':' | b'{')) || bytes[third..].starts_with(b"=>")
        }
        // `([` / `({` — a binding pattern, or a parenthesized array/object.
        Some(b'[' | b'{') => false,
        // `(...` — a rest parameter, which no expression spells.
        Some(b'.') => bytes[second..].starts_with(b"..."),
        Some(_) if identifier_starts_at(bytes, second) => {
            let name = &bytes[second..skip_identifier(bytes, second)];
            let after_name = skip_whitespace_and_comments(bytes, second + name.len());
            // `(public a` — a parameter property. A modifier NOT followed by a
            // name is just a name (`(readonly)`), and an `as` after it makes the
            // pair an assertion on one (`(public as B)`).
            if PARAM_MODIFIERS.contains(&name) && identifier_starts_at(bytes, after_name) {
                return !is_word_at(bytes, after_name, b"as");
            }
            match bytes.get(after_name) {
                // `(a:` — an annotated parameter.
                Some(b':') => true,
                // `(a?` — optional only when `:`, `,`, `=` or `)` follows; any
                // other follower makes the `?` a conditional's own (`(a ? b : c)`,
                // `(a ?? b)`). The `=` must be the assignment token, not the head
                // of `==` or `=>`.
                Some(b'?') => {
                    let past = skip_whitespace_and_comments(bytes, after_name + 1);
                    match bytes.get(past) {
                        Some(b':' | b',' | b')') => true,
                        Some(b'=') => !matches!(bytes.get(past + 1), Some(b'=' | b'>')),
                        _ => false,
                    }
                }
                // `(a,` / `(a=` / `(a)` — could be either; ask by parsing.
                _ => false,
            }
        }
        _ => false,
    }
}

/// Scan an arrow head's parameter list from the `(` at `start`, returning the `)`
/// that closes it when `=>` follows — `None` where the pattern does not hold.
///
/// Handles nested parentheses, the strings / templates / comments / regexes a
/// `(` or `)` can hide inside ([`matching_paren_close`]), and an optional
/// return-type annotation after the `)` (so both `(...) =>` and `(...): type =>`
/// match).
pub(super) fn scan_arrow_head_close(bytes: &[u8], start: usize) -> Option<usize> {
    matching_paren_close(bytes, start).filter(|&close| check_arrow_after_paren(bytes, close + 1))
}

/// Find the `)` closing the `(` at `start`, stepping over every literal a `(` or `)`
/// can hide inside — strings, templates, comments, and **regex literals**.
///
/// The regex step is what separates this from
/// [`matching_delimiter_close`]: that walk leaves a `/…/`
/// significant, so a pattern holding an unescaped `)` (`/\)=>/`) hands it a close the
/// source does not have. Every caller that acts on the `)` ALONE — with no follow-token
/// filter behind it to refuse a false one — must use this walk, because a false close
/// picks up whatever byte follows it as if it were the group's own
/// ([`paren_list_then_arrow`]'s `=>`).
pub(super) fn matching_paren_close(bytes: &[u8], start: usize) -> Option<usize> {
    if start >= bytes.len() || bytes[start] != b'(' {
        return None;
    }

    let end = bytes.len();
    let mut pos = start;
    let mut depth = 0;
    // The regex-vs-division anchor, rebuilt where a `/` asks for it rather than
    // maintained per byte; `OperandAnchor` owns the rule.
    let mut anchor = OperandAnchor::new(start);
    while pos < end {
        // Only `PAREN_HOP_NEEDLES` can move this scan; every other byte reaches
        // the `_ => {}` arm below and was stepped over one at a time, so the walk
        // hops between them a word at a time instead of reading each one
        // (`swar::next_byte_of`, the byte-scan ladder's top rung). The hop is
        // invisible to `anchor` for the same reason it is at
        // `source_scan::scan_to_matching_brace`: `OperandAnchor` resolves lazily
        // and *backwards* from the `/` that asks, so the crossed bytes were never
        // its to see.
        //
        // Per pass over 1,666 TypeScript files this crosses 320,445 inert bytes
        // in runs averaging 5.4, 95% of them in runs of eight or more — but **74%
        // of its hops are adjacent**, past `tsv_css`'s `string_end`, so the hop
        // alone pays the entry cost three times in four for nothing. Measured, it
        // is a REGRESSION without `is_hop_needle` in front of it: +0.015% of the
        // wire run, and it moved the TypeScript format run by −0.046 points where
        // the pre-tested form moves it by −0.174. **The pre-test is not a
        // refinement of this hop; it is the whole of it.**
        if !is_hop_needle(bytes[pos], PAREN_HOP_NEEDLES) {
            pos = tsv_lang::swar::next_byte_of(&bytes[..end], pos, PAREN_HOP_NEEDLES);
            if pos >= end {
                break;
            }
        }
        // Strings, templates, and comments are opaque — a `(`/`)` inside one is
        // not a real delimiter. The shared cursor skips all three in one place
        // (including backtick templates, which this scan historically missed).
        if let Some(past) = skip_trivia(bytes, pos, end, TriviaProfile::JS) {
            anchor.skipped_trivia(bytes, pos, past);
            pos = past;
            continue;
        }
        // Regex literals are the one trivia kind the cursor leaves significant
        // (it needs previous-token context). Skip a real regex so a `)`/`(`
        // inside its pattern isn't counted — e.g. a param default `(a = /\)/)`.
        if bytes[pos] == b'/' && anchor.starts_regex(bytes, pos, start) {
            pos = skip_regex_literal(bytes, pos, end);
            anchor.skipped_operand(pos);
            continue;
        }
        match bytes[pos] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(pos);
                }
            }
            _ => {}
        }
        pos += 1;
    }
    None
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
/// annotation". That counterexample's other half — `a ? (b) : c => d`, where the
/// one type IS followed by `=>` and the `:` is still the conditional's — is not
/// this scan's to settle: it needs the arrow's whole extent, so the parser asks it
/// by speculation (`Parser::parse_arrow_or_rewind`), as tsc does. Two shapes put a
/// stray `=>` within reach of such a scan, and in neither is the `:` a return-type
/// colon:
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
                let Some(close) = matching_angle_close(bytes, pos + 1, TypeArgScan::Parse) else {
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
/// Three callers ask it, for three different `(`s, because it is one question about
/// one shape:
///
/// - the return-type scan above, deciding whether a `=>` past the group is the type's
///   own or the enclosing arrow's;
/// - the type-argument head
///   ([`is_type_arguments_start`](super::Parser::is_type_arguments_start)'s `(` arm),
///   where a parameter list behind the `<` makes every byte to the region's own `>` the
///   function type's.
///
/// - the type parser's own `(` (`Parser::parse_parenthesized_or_function_type`), at a
///   full-type position: a parameter list, or else a parenthesized type whatever its
///   first token is.
///
/// **tsc's `isUnambiguouslyStartOfFunctionType` admits one shape more**: it runs
/// `parseModifiers` ahead of the parameter start, so `(readonly a: T) => U` and
/// `(public a: T) => U` are function types to the compiler. acorn-typescript's
/// `tsSkipParameterStart` has no modifier step and rejects both outright, so this —
/// the rule the AST wire answers to — is acorn's. tsc's extra step is spelled
/// separately, for the one site that owes it an answer
/// ([`paren_starts_modified_parameter_list`]).
pub(super) fn paren_starts_function_type(bytes: &[u8], paren: usize) -> bool {
    parameter_list_starts_at(bytes, skip_whitespace_and_comments(bytes, paren + 1))
}

/// [`paren_starts_function_type`] with tsc's MODIFIER step ahead of the parameter
/// start — `parseModifiers` in `skipParameterStart`, which is what makes
/// `(readonly a: T) => U` and `(public a: T) => U` function types to the compiler.
///
/// Only the type-argument head asks it, and for one reason: a parameter property is
/// a *grammar* error rather than a parse failure, so the compiler and prettier both
/// read `f<(readonly a: T) => U>(x)` as a generic call and complain about the
/// modifier. Reading the region as a comparison instead would print a different
/// program, so tsv claims it too and lets the type parse produce the rejection.
/// acorn-typescript rejects the input outright, so there is no wire to match and
/// nothing is lost by taking tsc's shape here.
///
/// A modifier is only one where a parameter start follows it (tsc's
/// `canFollowModifier`): everywhere else the same word is the parameter's own NAME,
/// which is what keeps `(readonly: T)`, `(readonly)` and `(readonly, b)` reading
/// exactly as [`paren_starts_function_type`] reads them, and `(readonly + 1)` /
/// `(readonly.a)` out of this arm entirely. At least one modifier must be consumed,
/// so every plain parameter list is the other predicate's alone.
pub(super) fn paren_starts_modified_parameter_list(bytes: &[u8], paren: usize) -> bool {
    let mut pos = skip_whitespace_and_comments(bytes, paren + 1);
    let mut modified = false;
    while identifier_starts_at(bytes, pos) {
        let word = &bytes[pos..skip_identifier(bytes, pos)];
        if !PARAM_MODIFIERS.contains(&word) {
            break;
        }
        let after = skip_whitespace_and_comments(bytes, pos + word.len());
        if !identifier_starts_at(bytes, after) && !matches!(bytes.get(after), Some(b'{' | b'[')) {
            break;
        }
        modified = true;
        pos = after;
    }
    modified && parameter_list_starts_at(bytes, pos)
}

/// The body of [`paren_starts_function_type`], asked at the first significant byte
/// INSIDE the `(` rather than at the paren — so the modifier step can re-ask it past
/// the modifiers it consumed.
fn parameter_list_starts_at(bytes: &[u8], first: usize) -> bool {
    let mut pos = first;
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

/// Check whether an `=>` follows the identifier that ends at `ident_end`
///
/// Detects single-parameter arrow functions without parentheses: `x => expr`
/// Returns `true` if pattern `identifier =>` is found (with optional whitespace/comments).
///
/// `ident_end` is the END of the name, not its start: the caller is positioned on a
/// token the lexer has already scanned and delimited, so the name's extent is a fact
/// the parser holds rather than one this scan re-derives. Re-deriving it walked the
/// whole name a second time — a mean of 6.7 bytes over 518,615 calls per pass across
/// the corpus, the single hottest source line in this lookahead on all three boards —
/// and re-derived it through a class that is deliberately **wider** than the lexer's
/// (`skip_identifier` steps over any multi-byte sequence rather than validating
/// `ID_Continue`, see its module). The two therefore agree on every name the lexer
/// accepted — measured over all 518,615 calls, with no disagreement — and can differ
/// only where the next byte is a non-ASCII non-identifier one, i.e. only where the
/// source is already a lex error. Taking the token's own end is the narrower answer as
/// well as the free one.
pub(super) fn scan_arrow_after_identifier(bytes: &[u8], ident_end: usize) -> bool {
    // **89% of calls have nothing to skip** (measured: 462,995 of 518,615 per pass over
    // the corpus, and 90.4% on TypeScript alone), so the byte right after the name
    // usually settles the question on its own — the byte-scan ladder's bottom rung,
    // where a pair of compares beats entering a walk whose first act is an L1 table
    // load on the branch's critical path.
    match bytes.get(ident_end) {
        // `x=>1` — the arrow is glued to the name.
        Some(b'=') => bytes.get(ident_end + 1) == Some(&b'>'),
        // Only whitespace or a comment can separate a name from its arrow, and every
        // whitespace byte is `<= 0x20` (ECMAScript `WhiteSpace` ∪ `LineTerminator`
        // has no other ASCII member) while a comment opens with `/`. So a printable
        // ASCII byte that is neither answers `false` outright: the walk would return
        // `ident_end` unmoved and the test below would read this same byte. Non-ASCII
        // falls through — the five bytes that can lead `<NBSP>`/`<ZWNBSP>`/`Zs`/LS/PS
        // are whitespace only after a decode, which is the walk's job, not this test's.
        Some(&b) if b > b' ' && b < 0x80 && b != b'/' => false,
        _ => {
            // Skip whitespace and comments after identifier: `a /* comment */ =>`
            let pos = skip_whitespace_and_comments(bytes, ident_end);

            // Check for =>
            pos + 1 < bytes.len() && bytes[pos] == b'=' && bytes[pos + 1] == b'>'
        }
    }
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
        // needed (unlike `matching_paren_close`): this scans type-argument
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

/// Scan for closing `>` at angle depth 0, tracking all delimiter depths.
///
/// Used by `is_type_arguments_start` to verify that a sequence like `<T | U>`
/// or `<T, (x: number) => void>` is actually type arguments (finds matching `>`).
///
/// Returns `true` if a matching `>` is found before hitting an unbalanced
/// `)`, `]`, `}`, or `;` at depth 0, and the close isn't followed by a token
/// that makes the `>` a comparison instead: a `>`/`>>`/`>>>` (relational or
/// shift) run, or an expression-starting token on the same line.
///
/// The follower half is the one the two readings disagree on. [`TypeArgScan::Relex`] asks
/// about the REGION alone and stops at the matching `>`: past a line terminator the
/// [`TypeArgScan::Parse`] arm below commits the list ahead of any expression, and whether the
/// printer puts a break there is unknowable where the question is asked. (Which followers
/// commit, for tsc and for acorn-typescript, is stated in `docs/conformance_prettier_ts.md`
/// §Relational chain type-argument parens; where tsc refuses one this parse commits on,
/// [`TypeArgScan`]'s soundness property is why the pair still stands.) The `>`-led run is
/// NOT part of that half — a `>>` / `>>>` is one token the printer can never split, so both
/// readings reject it.
pub(super) fn scan_for_closing_angle_bracket(bytes: &[u8], pos: usize, scan: TypeArgScan) -> bool {
    match matching_angle_close(bytes, pos, scan) {
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
            // line break an expression-starting token leaves the `<…>` an instantiation
            // too, whether it then begins a new statement via ASI (`x<y>⏎c`) or continues
            // as an operand (`x<y>⏎+1`). The whole follower split, tsc's beside acorn's, is
            // in `docs/conformance_prettier_ts.md` §Relational chain type-argument parens.
            if !scan.reads_source_as_written() {
                return true;
            }
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
    let Some(close) = matching_angle_close(bytes, pos, TypeArgScan::Parse) else {
        return false;
    };
    let after = skip_whitespace_and_comments(bytes, close + 1);
    if after >= bytes.len() || bytes[after] != b'(' {
        return false;
    }
    paren_list_then_arrow(bytes, after)
}

/// Whether the `(` at `paren` opens a parameter list whose matching `)` is
/// followed by `=>` — the tail shared by a plain function type (`(params) =>`), a
/// generic function type's split (`<…>(params) =>`) and a construct type
/// (`new (params) =>`). `paren` must point at the `(`; comments between `)` and
/// `=>` are skipped. The `=>` is the signal that separates these types from a
/// shift/comparison chain or a `new Foo()` value (neither of which has `(…) =>`).
///
/// The close comes from [`matching_paren_close`], the REGEX-AWARE walk, because the
/// `=>` this reads is whatever byte follows the `)` — so a pattern holding an
/// unescaped `)` (`a < (b, /\)=>/) > c`, a comparison chain to both oracles) would
/// otherwise hand it a close inside the literal and an arrow that is the pattern's own
/// text.
#[inline]
pub(super) fn paren_list_then_arrow(bytes: &[u8], paren: usize) -> bool {
    matching_paren_close(bytes, paren).is_some_and(|paren_close| {
        let after_params = skip_whitespace_and_comments(bytes, paren_close + 1);
        after_params + 1 < bytes.len()
            && bytes[after_params] == b'='
            && bytes[after_params + 1] == b'>'
    })
}

/// Whether `pos` begins a construct-signature type `new (params) => R` — the
/// `new`-prefixed sibling of [`paren_starts_function_type`]. True iff a whole-word
/// `new` is followed by a parenthesized parameter list whose matching `)` is
/// followed by `=>`. The `=>` is what separates the construct TYPE from a
/// `new Foo()` value expression, so `a < new Foo() > (c)` stays a comparison
/// while `f<new () => T>(x)` is a generic call — the same [`paren_list_then_arrow`]
/// tail the plain and generic function-type heads require. Callers
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
// TODO: this walk is O(distance to the enclosing delimiter) and every `<` in a construct
// pays it, so a call whose arguments are all comparisons is quadratic in the argument count
// — `f(a1 < b1, …, aN < bN)` quadruples per doubling of N, against a `+` control that stays
// flat and unmeasurable. (A wall-clock figure is deliberately not quoted: the ratio is the
// property, and an absolute second belongs to one machine.) Both readings pay it — a `<` that
// opens no type-argument list reads to the construct's own close before it can say so. A memo keyed on the enclosing delimiter's span would collapse it; nothing
// real has the shape, so it has not been worth the state.
///
/// Operator disambiguation: `<=` and `>=` are comparison operators (not angle
/// brackets) and `=>` is an arrow operator (not a closing bracket).
///
/// The unbalanced `)` is the one stop the two readings split on. Under
/// [`TypeArgScan::Relex`] a `)` the region did not open is a **paren shell the printer
/// strips**, and the question is about the PRINTED bytes, where it is absent — so it does
/// not end the region and the scan reads on. Without that, the pair this reading exists to
/// justify would erase itself on the next pass: `(a < b) > c` re-read as source stops the
/// scan at its own `)` and prints back out as `a < b > c`. `]`, `}` and a top-level `;`
/// stop both readings — a bracket or brace is never a shell the printer strips, and a `;`
/// ends the statement in either spelling.
pub(super) fn matching_angle_close(
    bytes: &[u8],
    mut pos: usize,
    scan: TypeArgScan,
) -> Option<usize> {
    let mut angle_depth: i32 = 1;
    let mut paren_depth: i32 = 0;
    let mut bracket_depth: i32 = 0;
    let mut brace_depth: i32 = 0;
    let end = bytes.len();

    while pos < end {
        // Strings, templates, and comments are opaque (the shared cursor skips
        // all three); a `<`/`>`/`;` inside one isn't significant. No regex skip is
        // needed (unlike `matching_paren_close`): this verifies a type-argument
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
                    if scan.reads_source_as_written() {
                        return None; // Unbalanced - hit call/group end
                    }
                    // A shell the printer strips is not in the form this reading grades;
                    // re-level and read on (see the doc above).
                    paren_depth = 0;
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
pub(super) fn has_line_terminator_between(bytes: &[u8], from: usize, to: usize) -> bool {
    let mut pos = from;
    while pos < to && pos < bytes.len() {
        if is_es_line_terminator_at(bytes, pos) {
            return true;
        }
        pos += 1;
    }
    false
}
