// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A type argument list whose first argument is an indexed access continued by a union or
//! intersection member (`f<A[K] | B>(x)`). tsc and acorn-typescript both read a call, and
//! both read the comparison chain where the member is no type (`a < b[c] | d() > (e)`).
//!
//! The fixtures under `typescript/syntax/disambiguation/less_than_index_*` pin the printed
//! forms. What they cannot hold is the SECOND pass of a spelling the printer rewrites into
//! the list's bare form — a leading `|`, a redundant type paren, a comparison-looking
//! authoring whose operand pair the printer strips. Each printed form is a new source the
//! parser reads again, so each is asserted here to print, and then to stay, as written.

use serde_json::Value;

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

fn svelte_format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let root = tsv_svelte::parse(source, &arena).expect("component should parse");
    tsv_svelte::format(&root, source)
}

/// The kind of the first statement's expression: `CallExpression` for the list read as
/// type arguments, a `BinaryExpression` (or a conditional over one) for the comparison chain.
fn expression_kind(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    let json = tsv_debug::json::wire_value(&tsv_ts::convert_ast_json_bytes(&program, source));
    json.pointer("/body/0/expression/type")
        .and_then(Value::as_str)
        .expect("an expression statement")
        .to_owned()
}

/// `source` prints as `printed`, and `printed` is a fixed point.
fn assert_prints_fixed(source: &str, printed: &str) {
    let out = format(source);
    assert_eq!(out, printed, "printed from {source:?}");
    assert_eq!(format(&out), out, "a fixed point: {source:?}");
}

/// The node kinds of the first statement's expression, pre-order — enough to tell a call
/// on `readonly(d)[e]` inside a comparison from a generic call over a `readonly` type.
fn expression_kinds(source: &str) -> Vec<String> {
    fn walk(node: &Value, out: &mut Vec<String>) {
        match node {
            Value::Object(map) => {
                if let Some(kind) = map.get("type").and_then(Value::as_str) {
                    if kind.starts_with("TS") {
                        return;
                    }
                    out.push(kind.to_owned());
                }
                map.values().for_each(|child| walk(child, out));
            }
            Value::Array(items) => items.iter().for_each(|item| walk(item, out)),
            _ => {}
        }
    }
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    let json = tsv_debug::json::wire_value(&tsv_ts::convert_ast_json_bytes(&program, source));
    let mut out = Vec::new();
    walk(
        json.pointer("/body/0/expression")
            .expect("an expression statement"),
        &mut out,
    );
    out
}

/// Spellings whose printed form drops what kept the list apart from a comparison on the
/// first pass: the second pass must read the same call.
#[test]
fn rewritten_spellings_are_fixed_points() {
    for (source, printed) in [
        ("f<| A[K] | B>(x);\n", "f<A[K] | B>(x);\n"),
        ("f<& A[0] & B>(x);\n", "f<A[0] & B>(x);\n"),
        ("f<(A[K] | B)>(x);\n", "f<A[K] | B>(x);\n"),
        ("f<(A[K]) | B>(x);\n", "f<A[K] | B>(x);\n"),
        ("f<A[(K)] | B>(x);\n", "f<A[K] | B>(x);\n"),
        ("f<typeof x[K] | B>(x);\n", "f<(typeof x)[K] | B>(x);\n"),
        ("f < A[K] | B > (x);\n", "f<A[K] | B>(x);\n"),
        ("a < (b[c] | d) > (e);\n", "a<b[c] | d>(e);\n"),
        ("a < (b[c]) | d > (e);\n", "a<b[c] | d>(e);\n"),
        ("a < (b[c] | d) >\ne;\n", "a<b[c] | d>;\ne;\n"),
        ("f<A[K] | B>\n(x);\n", "f<A[K] | B>(x);\n"),
        ("f<A[K] | B>\n[x];\n", "(f<A[K] | B>)[x];\n"),
    ] {
        assert_prints_fixed(source, printed);
        assert_ne!(expression_kind(printed), "BinaryExpression", "{printed:?}");
    }
}

/// A member that is no type keeps the chain a comparison, and so does every re-read of
/// its printed form, the operand pairs included.
#[test]
fn expression_members_stay_comparisons() {
    for (source, printed) in [
        ("a < b[c] | d() > (e);\n", "(a < b[c]) | (d() > e);\n"),
        (
            "a < b[c] | d ? e : f > (g);\n",
            "(a < b[c]) | d ? e : f > g;\n",
        ),
        ("a < b[c] | -d > (e);\n", "(a < b[c]) | (-d > e);\n"),
        ("a < b[c] | d + 1 > (e);\n", "(a < b[c]) | (d + 1 > e);\n"),
        ("a < b[c] & d.e() > (f);\n", "(a < b[c]) & (d.e() > f);\n"),
        (
            "a < b[c] | d | e() > (f);\n",
            "(a < b[c]) | d | (e() > f);\n",
        ),
        (
            "a < b[c] | d[e + 1] > (f);\n",
            "(a < b[c]) | (d[e + 1] > f);\n",
        ),
        // an indexed access takes no type arguments of its own
        ("f<A[K]<C>>(x);\n", "f < A[K] < C >> x;\n"),
        // nor is a second list behind that one a claim
        ("f<A[K]<C><D>, B>(x);\n", "(f < (A[K]<C>)<D>, B > x);\n"),
        ("(a < b[c] | d) > (e);\n", "((a < b[c]) | d) > e;\n"),
        ("(a < b[c] & d) >= (e);\n", "((a < b[c]) & d) >= e;\n"),
        ("a < (b[c] | d()) > e;\n", "(a < (b[c] | d())) > e;\n"),
        // a literal type takes one sign; a second one is an expression
        ("a < - -1 > (c);\n", "a < -(-1) > c;\n"),
        ("f<- -1>(x);\n", "f < -(-1) > x;\n"),
    ] {
        assert_prints_fixed(source, printed);
        assert_ne!(expression_kind(source), "CallExpression", "{source:?}");
    }
}

/// A negative literal type's sign is a token of its own: spaced from its digits, broken
/// from them, or with a comment between, it is the literal type the printed form glues,
/// as a member and as the list's only argument.
#[test]
fn spaced_negative_literals_read_as_types() {
    for (source, printed) in [
        ("f<A | - 1>(x);\n", "f<A | -1>(x);\n"),
        ("f<A & - 1>(x);\n", "f<A & -1>(x);\n"),
        ("f<A | -\n1>(x);\n", "f<A | -1>(x);\n"),
        ("f<A | - /* c */ 1>(x);\n", "f<A | -/* c */ 1>(x);\n"),
        ("a < b | - 1 > (c);\n", "a<b | -1>(c);\n"),
        ("f<- 1>(x);\n", "f<-1>(x);\n"),
        ("f<- 1 | A>(x);\n", "f<-1 | A>(x);\n"),
        ("f<A[K] | - 1>(x);\n", "f<A[K] | -1>(x);\n"),
    ] {
        assert_prints_fixed(source, printed);
        assert_eq!(expression_kind(printed), "CallExpression", "{printed:?}");
    }
}

/// A `readonly` operand is an array or tuple type: a parenthesized one only when its
/// postfix run ends in an array suffix `[]`. Otherwise acorn-typescript refuses the type
/// and reads the call `readonly(d)[e]` in a comparison, and so does tsv — printed so that
/// acorn reads it the same way again.
#[test]
fn readonly_parenthesized_operand_needs_an_array_suffix() {
    for (source, printed) in [
        ("a < readonly (d)[e] > (f);\n", "a < readonly(d)[e] > f;\n"),
        ("f<readonly (A)[K]>(x);\n", "f < readonly(A)[K] > x;\n"),
        (
            "a < b[c] | readonly (d)[e] > (f);\n",
            "(a < b[c]) | (readonly(d)[e] > f);\n",
        ),
    ] {
        assert_prints_fixed(source, printed);
        let kinds = expression_kinds(source);
        assert_eq!(
            kinds.first().map(String::as_str),
            Some("BinaryExpression"),
            "{source:?}"
        );
        assert!(
            kinds.iter().any(|kind| kind == "CallExpression"),
            "the call `readonly(…)`: {source:?} {kinds:?}"
        );
    }
    for (source, printed) in [
        ("f<readonly (d)[]>(x);\n", "f<readonly d[]>(x);\n"),
        ("f<readonly (d)[e][]>(x);\n", "f<readonly d[e][]>(x);\n"),
    ] {
        assert_prints_fixed(source, printed);
        assert_eq!(
            expression_kinds(source),
            ["CallExpression", "Identifier", "Identifier"],
            "{source:?}"
        );
    }
}

/// An index's body is graded by the same walk as the list around it, through any paren
/// shell: a body only the type grammar spells keeps the call, and its shelled spellings
/// print bare and stay calls.
#[test]
fn index_bodies_read_as_types_through_shells() {
    for (source, printed) in [
        ("f<A[((A))]>(x);\n", "f<A[A]>(x);\n"),
        ("f<A[(B)[]]>(x);\n", "f<A[B[]]>(x);\n"),
        ("f<A[(() => B)]>(x);\n", "f<A[() => B]>(x);\n"),
        ("f<A[({ a: 1 })]>(x);\n", "f<A[{ a: 1 }]>(x);\n"),
        ("f<A[(import('m'))]>(x);\n", "f<A[import('m')]>(x);\n"),
        ("f<A[(keyof B) | C]>(x);\n", "f<A[keyof B | C]>(x);\n"),
        ("f < A[() => B] > (x);\n", "f<A[() => B]>(x);\n"),
        ("f<A[() => B]>\n(x);\n", "f<A[() => B]>(x);\n"),
        // a type predicate, as a function type's return and as the argument itself
        ("f<A[(a) => a is B]>(x);\n", "f<A[(a) => a is B]>(x);\n"),
        ("f<A | this is B>(x);\n", "f<A | this is B>(x);\n"),
        // an assertion predicate's subject ends at a shell's `)` as it does at the `]`
        (
            "f<A[((a) => asserts a)]>(x);\n",
            "f<A[(a) => asserts a]>(x);\n",
        ),
        // a type query of an import takes two lists: the import type's, then its own
        (
            "f<A[typeof import('m')<C><D>]>(x);\n",
            "f<A[typeof import('m')<C><D>]>(x);\n",
        ),
        (
            "f<A[B | typeof import('m').B<C><D>]>(x);\n",
            "f<A[B | typeof import('m').B<C><D>]>(x);\n",
        ),
        (
            "f<A[keyof typeof import('m')<C><D>]>(x);\n",
            "f<A[keyof typeof import('m')<C><D>]>(x);\n",
        ),
        (
            "f<A[typeof import('m')<C><D> | B]>(x);\n",
            "f<A[typeof import('m')<C><D> | B]>(x);\n",
        ),
        (
            "f<A[typeof import('m')<C><D>[K]]>(x);\n",
            "f<A[(typeof import('m')<C><D>)[K]]>(x);\n",
        ),
        (
            "f<typeof import('m')<C><D>>(x);\n",
            "f<typeof import('m')<C><D>>(x);\n",
        ),
        // (in any index: as a function type's return, as a bar's member)
        (
            "f<A[() => typeof import('m')<C><D>]>(x);\n",
            "f<A[() => typeof import('m')<C><D>]>(x);\n",
        ),
        (
            "f<A[{} | typeof import('m')<C><D>]>(x);\n",
            "f<A[{} | typeof import('m')<C><D>]>(x);\n",
        ),
        // (a comma inside the specifier is no options argument)
        (
            "f<A[() => typeof import('m,')<C><D>]>(x);\n",
            "f<A[() => typeof import('m,')<C><D>]>(x);\n",
        ),
        // (over an import with options too, in an index that opens as a type)
        (
            "f<A[typeof import('m', { with: { a: 'b' } })<C><D>[K]]>(x);\n",
            "f<A[(typeof import('m', { with: { a: 'b' } })<C><D>)[K]]>(x);\n",
        ),
        // an import with options and no second list is the type tsc reads wherever a type
        // is graded: a function type's return in any index
        (
            "f<A[() => typeof import('m', { with: {} })]>(x, y);\n",
            "f<A[() => typeof import('m', { with: {} })]>(x, y);\n",
        ),
        // a parameter of function type holds an arrow, which is no default
        (
            "f<A[(a: () => void) => B]>(x);\n",
            "f<A[(a: () => void) => B]>(x);\n",
        ),
        // (nor does a `=` inside a string or a comment)
        ("f<A[(a: '=') => B]>(x);\n", "f<A[(a: '=') => B]>(x);\n"),
        (
            "f<A[(a /* = */) => B]>(x);\n",
            "f<A[(a /* = */) => B]>(x);\n",
        ),
        // nor is a `=` inside a destructuring pattern or a type-parameter list: no default
        // of the parameter's own, and both parsers read the function type
        (
            "f<A[({ a = 1 }) => 0]>(x);\n",
            "f<A[({ a = 1 }) => 0]>(x);\n",
        ),
        ("f<A[([a = 1]) => 0]>(x);\n", "f<A[([a = 1]) => 0]>(x);\n"),
        (
            "f<A[({ a: b = 1 }) => 0]>(x);\n",
            "f<A[({ a: b = 1 }) => 0]>(x);\n",
        ),
        (
            "f<A[(a: <T = 1>() => T) => B]>(x);\n",
            "f<A[(a: <T = 1>() => T) => B]>(x);\n",
        ),
        // (behind a generic head too, whose own type parameter's default is none either)
        (
            "f<A[<T>({ a = 1 }) => B]>(x);\n",
            "f<A[<T>({ a = 1 }) => B]>(x);\n",
        ),
        (
            "f<A[<T>(a: <U = 1>() => U) => B]>(x);\n",
            "f<A[<T>(a: <U = 1>() => U) => B]>(x);\n",
        ),
        ("f<A[<T = 1>(a) => B]>(x);\n", "f<A[<T = 1>(a) => B]>(x);\n"),
        // `readonly` over a type operator's array
        (
            "f<A[B | readonly typeof b[]]>(x);\n",
            "f<A[B | readonly (typeof b)[]]>(x);\n",
        ),
        (
            "f<A[readonly typeof b[]]>(x);\n",
            "f<A[readonly (typeof b)[]]>(x);\n",
        ),
        (
            "f<A[readonly typeof b<C>[]]>(x);\n",
            "f<A[readonly (typeof b<C>)[]]>(x);\n",
        ),
        (
            "f<readonly typeof b[]>(x);\n",
            "f<readonly (typeof b)[]>(x);\n",
        ),
        // a generic construct type, in an index, as the argument itself, and shelled
        ("f<A[new <T>() => T]>(x);\n", "f<A[new <T>() => T]>(x);\n"),
        ("f<new <T>() => T>(x);\n", "f<new <T>() => T>(x);\n"),
        ("f<(new <T>() => T)>(x);\n", "f<new <T>() => T>(x);\n"),
        (
            "f<A[abstract new <T>() => T]>(x);\n",
            "f<A[abstract new <T>() => T]>(x);\n",
        ),
    ] {
        assert_prints_fixed(source, printed);
        assert_eq!(expression_kind(printed), "CallExpression", "{printed:?}");
    }
    // (an import with options as a bar's member there, ahead of a template)
    let tagged = "f<A[{} | import('m', { with: {} }).B]>`t`;\n";
    assert_prints_fixed(tagged, tagged);
    assert_eq!(expression_kind(tagged), "TaggedTemplateExpression");
}

/// Where the body is an expression the `<` is a comparison, whatever follows the would-be
/// closing `>` — and so is every re-read of the printed form, with whichever pair the
/// chain's own region takes.
#[test]
fn expression_index_bodies_stay_comparisons() {
    for (source, printed) in [
        ("f<A[a.b()]>(x);\n", "(f < A[a.b()]) > x;\n"),
        ("f<A['a' + b]>(x);\n", "(f < A['a' + b]) > x;\n"),
        ("f<A[keyof(b, c)]>(x);\n", "(f < A[keyof(b, c)]) > x;\n"),
        ("f<A[() => b + 1]>(x);\n", "f < A[() => b + 1] > x;\n"),
        // (an `asserts` an expression spells heads no predicate for the reading of the printed form)
        (
            "x = a < b[asserts in is] > c;\n",
            "x = a < b[asserts in is] > c;\n",
        ),
        (
            "a < b[c | d()] | e > (f);\n",
            "(a < b[c | d()]) | (e > f);\n",
        ),
        // inside an index a dynamic import is the expression acorn-typescript reads (to tsc
        // an import type takes any specifier, so the compiler reads a generic call there),
        // and at the list's own level it is every parser's ahead of a follower that refuses
        // the list; an import type is an operand like any other, so what follows it has to
        // continue a type
        ("a < b[import(c)[d]] > (e);\n", "a < b[import(c)[d]] > e;\n"),
        ("a < import(c) || d > e;\n", "a < import(c) || d > e;\n"),
        (
            "a < b | import('m') + 1 > (e);\n",
            "(a < b) | (import('m') + 1 > e);\n",
        ),
        // a paren shell inside an index is read by what it holds
        ("a < b[(c, d)] > (e);\n", "a < b[(c, d)] > e;\n"),
        ("a < b[c = d] > (e);\n", "a < b[(c = d)] > e;\n"),
        ("a < b[(c = d)] > `t`;\n", "a < b[(c = d)] > `t`;\n"),
        // a nested argument list ends a type only where a type's own follower stands
        ("a < b | c<d>(e) > (f);\n", "(a < b) | (c<d>(e) > f);\n"),
        (
            "a < b[c] | d<e>(f) > (g);\n",
            "(a < b[c]) | (d<e>(f) > g);\n",
        ),
        ("a < d<e>(f) > (g);\n", "a < d<e>(f) > g;\n"),
        // only a type name takes an argument list, inside an index as outside one
        ("f<A | string<C>>(x);\n", "(f < A) | (string < C >> x);\n"),
        ("f<'a'<C>>(x);\n", "f < 'a' < C >> x;\n"),
        ("f<A[string<C>]>(x);\n", "(f < A[string<C>]) > x;\n"),
        ("f<A[B | 'a'<C>]>(x);\n", "(f < A[B | 'a'<C>]) > x;\n"),
        ("f<A[(B)<C>]>(x);\n", "(f < A[B<C>]) > x;\n"),
        // a parenthesized name behind a bar, where the chain's own pair ends the region
        ("f<A | (B)<C>>(x);\n", "(f < A) | (B < C >> x);\n"),
        ("f<A | (B)<C>>`t`;\n", "(f < A) | (B < C >> `t`);\n"),
        // and heading the region, where the follower refuses the list
        ("f<(A)<C>> x;\n", "f < A < C >> x;\n"),
        ("f<(A | B)<C>> x;\n", "f < (A | B) < C >> x;\n"),
        (
            "f<(typeof a)<C> | D> x;\n",
            "(f < (typeof a)<C>) | (D > x);\n",
        ),
        // a negative literal's list that closes on a shift is one to no parser
        ("f<-1<C>>(x);\n", "f < -1 < C >> x;\n"),
        ("f<-1<C>>`t`;\n", "f < -1 < C >> `t`;\n"),
        ("f<A | -1<C>>(x);\n", "(f < A) | (-1 < C >> x);\n"),
        ("f<A[-1<C>>d]>(x);\n", "(f < A[-1 < C >> d]) > x;\n"),
        // and a shift behind one opens nothing
        ("f<A[-1 << c]>(x);\n", "f < A[-1 << c] > x;\n"),
        ("f<-1 << c>(x);\n", "f < -1 << c > x;\n"),
        // a second list that ends an index's body is the instantiation acorn-typescript
        // reads, where tsc has nothing for its type assertion to take and rejects
        (
            "f<A[import('m').A<B><C>]>(x);\n",
            "(f < A[(import('m').A<B>)<C>]) > x;\n",
        ),
        ("a < b[A<B><C>] > (c);\n", "(a < b[(A<B>)<C>]) > c;\n"),
        ("a < b[(A<B><C>)] > (c);\n", "(a < b[(A<B>)<C>]) > c;\n"),
        // and one ahead of a call's arguments or a template is the call no type spells,
        // which prints as written — in the index itself and as an arrow function's body
        (
            "f<A[() => b<C><D>(e)]>(x);\n",
            "(f < A[() => b<C><D>(e)]) > x;\n",
        ),
        (
            "f<A[() => b<C><D>(e)]>`t`;\n",
            "(f < A[() => b<C><D>(e)]) > `t`;\n",
        ),
        ("f<A[b<C><D>(e)]>(x);\n", "(f < A[b<C><D>(e)]) > x;\n"),
        ("f<A[b<C><D>`t`]>(x);\n", "(f < A[b<C><D>`t`]) > x;\n"),
        (
            "f<A[() => b<C><D><E>(e)]>(x);\n",
            "(f < A[() => b<C><D><E>(e)]) > x;\n",
        ),
        // so is one no operand follows for tsc's type assertion: the compiler rejects
        // the spelling, and the instantiation is the only reading
        (
            "f<A[() => b<C><D> | B]>(x);\n",
            "(f < A[() => (b<C>)<D> | B]) > x;\n",
        ),
        ("f<A[b<C><D>, B]>(x);\n", "(f < A[((b<C>)<D>, B)]) > x;\n"),
        (
            "f<A[string<C><D>, B]>(x);\n",
            "(f < A[((string<C>)<D>, B)]) > x;\n",
        ),
        (
            "f<A[b<C><D> ? p : q]>(x);\n",
            "(f < A[(b<C>)<D> ? p : q]) > x;\n",
        ),
        // a generic arrow function's body is graded like any other
        ("f<A[<T>() => b + 1]>(x);\n", "f < A[<T>() => b + 1] > x;\n"),
        // a `<` past a line break opens no list of the name before it: inside an index
        // the expression's own instantiation is what both parsers read
        ("f<A[B\n<C>]>(x);\n", "(f < A[B<C>]) > x;\n"),
        ("f<A[() => B\n<C>]>(x);\n", "(f < A[() => B<C>]) > x;\n"),
        ("a < b[c\n<d>>(e)] > (f);\n", "(a < b[c < d >> e]) > f;\n"),
        ("f<B\n<C> | D>(x);\n", "(f < B<C>) | (D > x);\n"),
        (
            "f<typeof b\n<C, D> & E>(x);\n",
            "(f < typeof b<C, D>) & (E > x);\n",
        ),
        ("f<A | B\n<C>>(x);\n", "(f < A) | (B < C >> x);\n"),
        // a radix literal takes no exponent, so its `e` is a digit and the `-` arithmetic
        ("f<0xe-1>(x);\n", "f < 0xe - 1 > x;\n"),
        // a `[` past a line break is no index of the name before it, inside an index too
        ("f<A[B\n[c]]>(x);\n", "(f < A[B[c]]) > x;\n"),
        // a `.` behind a complete numeric literal is a member access
        ("a < b[c] | 1..x > (e);\n", "(a < b[c]) | ((1).x > e);\n"),
        ("a < 1..x > (e);\n", "a < (1).x > e;\n"),
        // `<=` opens no list, and `is` past a line break begins a statement
        ("a < c <= d >\nc;\n", "a < c <= d > c;\n"),
        ("a < b\nis > (c);\n", "a < b;\nis > c;\n"),
    ] {
        assert_prints_fixed(source, printed);
        assert_ne!(expression_kind(source), "CallExpression", "{source:?}");
    }
}

/// A chain's paren pair is decided from the SOURCE, about the form the printer emits — so
/// where the two spell an operand differently the first pass has to answer for the printed
/// one, or the second pass moves the pair.
#[test]
fn chains_reach_their_fixed_point_in_one_pass() {
    for (source, printed) in [
        // a comment between a sign and its digits keeps a pair of its own
        ("x = a < - /* c */ 1 > d;\n", "x = a < -(/* c */ 1) > d;\n"),
        (
            "x = a < (- /* c */ 1) > d;\n",
            "x = a < -(/* c */ 1) > d;\n",
        ),
        (
            "x = a < b[- /* c */ 1] > d;\n",
            "x = a < b[-(/* c */ 1)] > d;\n",
        ),
        // a shell the printer strips, around a sign's digits or a `typeof`'s operand
        ("a < -(1) > c;\n", "(a < -1) > c;\n"),
        ("f<-1[c]> e;\n", "(f < -(1)[c]) > e;\n"),
        ("a < b[typeof (c)] > d;\n", "(a < b[typeof c]) > d;\n"),
        // a shell the printer strips, ahead of a qualified name's next segment
        ("a < (b).c > d;\n", "(a < b.c) > d;\n"),
        ("a < b[(c).d] > e;\n", "(a < b[c.d]) > e;\n"),
        ("a < (string).x > c;\n", "(a < string.x) > c;\n"),
        // a comparison chain nested in the index prints with a pair of its own, which the
        // enclosing chain reads through
        ("a < b[B<C>[]] > c;\n", "(a < b[(B < C) > []]) > c;\n"),
        (
            "a < b[B < C, D > []] > c;\n",
            "(a < b[(B < C, D > [])]) > c;\n",
        ),
        ("a < b[c < d > e] > f;\n", "(a < b[(c < d) > e]) > f;\n"),
        (
            "a < b[(c)<d | e()>[]] > f;\n",
            "(a < b[(c < d) | (e() > [])]) > f;\n",
        ),
        // a shift in the index is no list's opener, and a `>` there closes none
        ("a < b[c << d] > e;\n", "a < b[c << d] > e;\n"),
        ("a < b[c > d] >\ne;\n", "a < b[c > d] > e;\n"),
        ("a < b[c > (d)] > (e);\n", "a < b[c > d] > e;\n"),
        // past the body's first operand, anything that could continue a type takes the
        // rest of the index as it stands
        ("a < b[c | d()] > e;\n", "(a < b[c | d()]) > e;\n"),
        ("a < b[(c) | (d, e)] > f;\n", "(a < b[c | (d, e)]) > f;\n"),
        ("a < b[c[d, e]] > f;\n", "(a < b[c[(d, e)]]) > f;\n"),
        ("a < b[c.d()] > e;\n", "(a < b[c.d()]) > e;\n"),
        ("a < b[import.meta] > c;\n", "(a < b[import.meta]) > c;\n"),
        ("a < b[this.c] > d;\n", "(a < b[this.c]) > d;\n"),
        ("a < b[keyof] > c;\n", "(a < b[keyof]) > c;\n"),
        ("a < b[unique - 1] > c;\n", "(a < b[unique - 1]) > c;\n"),
        ("a < b['c' + d] > e;\n", "(a < b['c' + d]) > e;\n"),
        // and a body whose first operand is followed by anything else keeps the chain bare
        ("a < b[c + 1] > d;\n", "a < b[c + 1] > d;\n"),
        ("a < b[infer - 1] > c;\n", "a < b[infer - 1] > c;\n"),
        ("a < b[(c, d)] > e;\n", "a < b[(c, d)] > e;\n"),
        // an instantiation nested in the index prints as written
        ("a < b[c<D>] > e;\n", "(a < b[c<D>]) > e;\n"),
        // an arrow function's bare-name parameter takes a pair
        ("a < b[x => y] > c;\n", "(a < b[(x) => y]) > c;\n"),
        ("a < b[x => y + 1] > c;\n", "a < b[(x) => y + 1] > c;\n"),
        // a member tail on a numeric literal
        ("a < (1.5.x) > c;\n", "a < (1.5).x > c;\n"),
        // an import head and a negative literal's list are regions a parser claims
        ("a < import(c) > d;\n", "(a < import(c)) > d;\n"),
        ("f<-1<C>(e)> x;\n", "(f < -1<C>(e)) > x;\n"),
        // a member on a negative literal prints on a parenthesized one, no literal type
        ("f<-1..x> e;\n", "f < -(1).x > e;\n"),
        // a comment that carries a line break between a name and its list keeps the break
        // in the printed form, where the name heading the region is a claim
        (
            "f<B // c\n<C>(e)> x;\n",
            "(f <\n\tB // c\n\t<C>(e)) >\n\tx;\n",
        ),
        (
            "f<B /* c\n */<C>(e)> x;\n",
            "(f <\n\tB/* c\n\t */ <C>(e)) >\n\tx;\n",
        ),
        ("f<B /* c */\n<C>(e)> x;\n", "f < B/* c */ <C>(e) > x;\n"),
        // a shell the printer keeps heads the printed region too, with the list behind it
        (
            "x = f < (typeof a)<C>`t` > y;\n",
            "x = (f < (typeof a)<C>`t`) > y;\n",
        ),
        ("x = f < (!!a)<C>(e) > y;\n", "x = (f < (!!a)<C>(e)) > y;\n"),
        (
            "x = f < (typeof a)<C>(e)[0] > y;\n",
            "x = (f < (typeof a)<C>(e)[0]) > y;\n",
        ),
        (
            "x = f < (a | b)<C>(e) > y;\n",
            "x = (f < (a | b)<C>(e)) > y;\n",
        ),
        (
            "x = f < (-a)<C> | d > y;\n",
            "x = (f < (-a)<C>) | (d > y);\n",
        ),
        // and one whose content is no type's keeps the chain bare
        (
            "x = f < (a ?? b)<C>(e) > y;\n",
            "x = f < (a ?? b)<C>(e) > y;\n",
        ),
        (
            "x = f < (typeof a())<C>(e) > y;\n",
            "x = f < (typeof a())<C>(e) > y;\n",
        ),
        ("a < b[1..x] > c;\n", "(a < b[(1).x]) > c;\n"),
    ] {
        assert_prints_fixed(source, printed);
        assert_ne!(expression_kind(source), "CallExpression", "{source:?}");
    }
}

/// The pair is what keeps a chain a chain once a line break lands past its closing `>`,
/// where the parser commits a type-argument list ahead of any expression. Each chain's
/// printed form is re-read with the break in place and has to print as it did.
#[test]
fn printed_chains_survive_a_break_past_the_close() {
    for source in [
        "a < b[{ d: 1 }] > c;\n",
        "a < b[[d]] > c;\n",
        "a < b[() => d] > c;\n",
        "a < b[x => y] > c;\n",
        "a < b[(x) => x.y] > c;\n",
        "a < b[<T,>(x: T) => y] > c;\n",
        "a < b[import('m')] > c;\n",
        "a < b[d = e] > c;\n",
        "a < b[(d, e)] > c;\n",
        "a < b[(d = e)[f]] > c;\n",
        "a < b[(d, e) | f] > c;\n",
        "a < b[(x) => (y = z)] > c;\n",
        "a < b[(x) => (y, z)] > c;\n",
        "a < b[keyof(d = e)] > c;\n",
        "a < b[keyof(d)] > c;\n",
        "a < b[d<E>] > c;\n",
        "a < b[d<E>[]] > c;\n",
        "a < b[d < e > f] > c;\n",
        "a < b[d < e > []] > c;\n",
        "a < b[d < e, f > []] > c;\n",
        "a < b[d < e | f > []] > c;\n",
        "a < b[(d).e] > c;\n",
        "a < b[(d)<E>] > c;\n",
        "a < b[typeof (d)] > c;\n",
        "a < b[- /* x */ 1] > c;\n",
        "a < b[-(1)] > c;\n",
        "a < b[1..x] > c;\n",
        "a < b[d | e()] > c;\n",
        "a < b[d ? e : f] > c;\n",
        "a < (d).e > c;\n",
        "a < d<e>(f) > c;\n",
        "a < import('m') > c;\n",
        "a < import(d) + 1 > c;\n",
        "a < import('m') + 1 > c;\n",
        "a < b[import(d)] > c;\n",
        "a < -(1) > c;\n",
        "a < typeof (d) > c;\n",
        "a < d <= e > c;\n",
        "a < 1..x > c;\n",
        "a < (typeof d)<E>`t` > c;\n",
        "a < (!d)<E>(g)[0] > c;\n",
        "a < (d | e)<E>(g) > c;\n",
        "a < (d ?? e)<E>(g) > c;\n",
    ] {
        let out = format(source);
        let close = " > c;\n";
        assert!(out.ends_with(close), "a chain closing on `> c;`: {out:?}");
        let broken = format!("{} >\nc;\n", &out[..out.len() - close.len()]);
        assert_eq!(format(&broken), out, "from {source:?}");
    }
}

/// A second list behind a first whose index holds a function type: tsc reads the bare
/// spelling as a comparison over an arrow function indexing `A`, acorn-typescript as a
/// call over an instantiation, and the line prints as written so each reads the output as
/// it read the input. Where the arrow's body spells nothing an expression does, tsc has no
/// comparison to read and the chain takes the pair every parser reads alike.
#[test]
fn second_list_behind_a_function_type_index_prints_as_written() {
    for (source, printed) in [
        ("f<A[() => B]><D>(x);\n", "f<A[() => B]><D>(x);\n"),
        (
            "f<A[(a: B) => C] | E><D>(x);\n",
            "f<A[(a: B) => C] | E><D>(x);\n",
        ),
        (
            "f<A[() => () => B]><D>(x);\n",
            "f<A[() => () => B]><D>(x);\n",
        ),
        ("f<A[() => B[]]><D>(x);\n", "(f<A[() => B[]]>)<D>(x);\n"),
        (
            "f<A[(a) => a is B]><D>(x);\n",
            "(f<A[(a) => a is B]>)<D>(x);\n",
        ),
    ] {
        assert_prints_fixed(source, printed);
        assert_eq!(expression_kind(printed), "CallExpression", "{printed:?}");
    }
}

/// Regions the lookahead claims and the type parse then rejects — loud errors, each kept
/// because no printed form of the chain reads back, to every parser, as the input did, or
/// because the claim that guards such a chain is coarser than its cause. The fixture
/// `typescript/expressions/binary/relational_claimed_region_svelte_divergence` records
/// what acorn-typescript reads; its `tsv_rejects.txt` reads the first error alone, so each
/// line is asserted here.
#[test]
fn claimed_regions_stay_rejected() {
    for source in [
        // A paren shell heading the region, with a list behind it. Both parsers read a
        // comparison chain; around a name the printer strips the shell, and the bare
        // name's list then runs on to the region's own close, a generic call or a tagged
        // template.
        "f<(A)<C>>(x);\n",
        "f<(A)<C>>`t`;\n",
        "f<(A)<C>><D>(x);\n",
        "f<(A)<C>>\nx;\n",
        "f<((A.B))<C>>(x);\n",
        "f<(/* c */ A)<C>>`t`;\n",
        "f<(typeof a)<C>>`t`;\n",
        "f<(typeof a)<C>>(x);\n",
        "f<(!A)<C>>`t`;\n",
        "f<(A)<C>, D>`t`;\n",
        "f<(A)<C>, D>(x);\n",
        "g(f<(A)<C>, D>(x));\n",
        // Whatever the shell holds: a name the printer makes by stripping an inner shell,
        // a shell it keeps, a head the bare region would claim on its own.
        "f<(typeof (b))<C>>`t`;\n",
        "f<(typeof (b))<C>>(x);\n",
        "f<(typeof (b))<C>>(x, y);\n",
        "f<((b).c)<C>, D>(x, y);\n",
        "g(f<((b).c)<C>, D>(x));\n",
        "f<((import('m')).B)<C>>`t`;\n",
        "f<((string).x)<C>>`t`;\n",
        "f<(typeof (a).b)<C>, D>`t`;\n",
        "f<(import(c))<C>>`t`;\n",
        "f<(keyof)<C>>`t`;\n",
        "f<((b, c))<C>>`t`;\n",
        "f<(A | B)<C>>`t`;\n",
        "f<(A | B)<C>>(x);\n",
        "f<(-1)<C>>`t`;\n",
        // Whatever follows the list: a comma inside it or a shift at its close leaves the
        // printed chain no pair, a stripped head may claim the region by itself, and a
        // shell the printer keeps may hold what tsc reads as a parameter list.
        "f<(A)<C> | D>(x);\n",
        "f<(typeof a)<C> | D>(x);\n",
        "f<((a, b))<C<D>> | D>(x);\n",
        "f<(A)<C>[]>(x);\n",
        "g(f<(A)<C, D>[0]>(x));\n",
        "f<(A)<C, D>[]>`t`;\n",
        "f<(A)<C<D>>[K]>(x);\n",
        "f<(A)<C, D> + 1>(x);\n",
        "f<(A)<C> || D>(x);\n",
        "f<((-1))<C, D> + 1>`t`;\n",
        "f<(keyof)<C, D>[]>(x);\n",
        // A name whose list opens past a line break, heading the region the same way:
        // the printer folds the break, and the printed name takes the list.
        "f<B\n<C>>(x);\n",
        "f<B\n<C>, D>(x, y);\n",
        "f<B\n<C, D>[K]>(x, y);\n",
        "f<B\n<C> || d>(x);\n",
        // (`async` and `await` are no names a list past a break stands off from: the
        // printed `async<C>(e)` is a generic arrow function's head)
        "f<A[async\n<C>(e)]>(x);\n",
        "f<A[B | async\n<C>(e)]>(x);\n",
        "f<A[await\n<[C]> + 1]>(x);\n",
        // An import type's list past a line break: acorn-typescript takes it, tsc ends the
        // type at the break.
        "f<import('m').B\n<C>>(x);\n",
        "f<A | import('m').B\n<C>>(x);\n",
        "f<import('m').B\n<C> | D>(x);\n",
        "f<A[B | import('m').B\n<C>]>(x);\n",
        // A second list behind a keyword's or a literal's own, heading the region: the
        // instantiation acorn-typescript reads prints in a pair, a shell heading the region.
        "f<string<C><D>, B>(x);\n",
        "g(f<string<C><D>, B>(x));\n",
        "f<string<[C]><D>, B>`t`;\n",
        "f<'a'<C><D>>(x);\n",
        "f<this<C><D> | B>(x);\n",
        // A negative literal's list with a follower an instantiation takes: acorn-typescript
        // reads a generic call over the literal type of `-(1<C>(e))`, tsc a comparison chain.
        "f<-1<C>(e)>(x);\n",
        "f<-1<C>(e)>`t`;\n",
        "f<-1<C>`u`>(x);\n",
        "f<A | -1<C>(e)>(x);\n",
        "f<-1<C> | D>(x);\n",
        "f<A[-1<C>(e)]>(x);\n",
        // (the same literal as an arrow function's body in an index, or behind `unique`)
        "f<A[() => -1<C>]>(x);\n",
        "f<A[unique -1<C>]>(x);\n",
        // A member tail glued to a negative literal's digits: acorn-typescript reads a
        // generic call over the literal type of `-(1..x)`, tsc a comparison chain.
        "f<-1..x>(x);\n",
        "f<- .5.x>`t`;\n",
        "f<A | -1..x>(x);\n",
        "f<A[-1..x]>(x);\n",
        "f<-1..x_1>(x);\n",
        // The spaced postfix of a negative literal — a call's arguments, a template, a
        // member — in an index whose body opens as a type: on a name, a literal or a shell
        // that a bar, a `.`, a `[` or a `<` follows, on a string or template key, or on a
        // `keyof` / `typeof`. The same split as the glued tail's.
        "f<A[B | -1(e)]>(x);\n",
        "f<A[B & -1`t`]>(x);\n",
        "f<A[A[-1`t`]]>(x);\n",
        "f<A[-1 .x]>(x);\n",
        "f<A[B | -1 .x]>(x);\n",
        "f<A[B | -1?.x]>(x);\n",
        "f<A['a' | -1(e)]>(x);\n",
        "f<A[keyof -1(e)]>(x);\n",
        "f<A[B | (-1(e))]>(x);\n",
        "f<A[(B) | -1(e)]>(x);\n",
        "f<A[(-1) | -1(e)]>(x);\n",
        "f<A[B<C> | -1(e)]>(x);\n",
        "f<A[B.c | -1(e)]>(x);\n",
        "f<A[typeof b | -1(e)]>(x);\n",
        "f<A[B][C | -1(e)]>(x);\n",
        "f<B | A[-1 .x]>(x);\n",
        "f<A[B | -1(e)], C>(x);\n",
        "f<A[B | -1(e)]>`t`;\n",
        "f<A[B | -1(e)]>\nx;\n",
        // (a non-null `!` is one more postfix that parser takes, and so is whatever
        // follows a member tail glued to the digits — the name the tail's own byte run
        // stops short of included)
        "f<A[B | -1!]>(x);\n",
        "f<A[B | -1..x()]>(x);\n",
        "f<A[B | -1..x!]>(x);\n",
        "f<A[B | -1..$x]>(x);\n",
        "f<(-1..x())>(x);\n",
        // (a literal the printer makes by stripping its shell is the same literal)
        "f<A[B | -(1)(e)]>(x);\n",
        "f<A[B | -((1))(e)]>(x);\n",
        "f<A[B | -(1)(e)], C>(x);\n",
        "f<A[B | -(1)`t`]>(x);\n",
        "f<A[B | -(1)!]>(x);\n",
        // (and in an index behind a nested list or an import type, whatever opens it)
        "f<A<B>[-1(e)]>(x);\n",
        "f<import('m').A[-1(e)]>(x);\n",
        // A meta-property in an index: acorn-typescript reads a type named `new.target`,
        // tsc rejects the line.
        "f<A[B | new.target]>(x);\n",
        "f<A[B | new.target.x]>(x);\n",
        "f<A[new.target]>(x);\n",
        "f<A[-1 | new.target]>(x);\n",
        // (a type query's second list is its own inside an index alone)
        "f<typeof import('m')<C><D> + 1>(x);\n",
        // (`readonly` stands over a type query's ARRAY)
        "f<A[readonly typeof b]>(x);\n",
        "f<A[B | readonly typeof b[K]]>(x);\n",
        // An import head that is no import type: tsc reads the generic call its import
        // type takes any specifier for, acorn-typescript a comparison over a dynamic import.
        "f<import(c)>(e);\n",
        "f<import(c)[K]>(x);\n",
        "f<import(c)!>(x);\n",
        "a < b | import(c) > (e);\n",
        "f<import.meta>(x);\n",
        // (the claim is the closing scan, which reads nothing ahead of the close: a chain
        // both parsers read as one is taken with the rest once a line break follows it)
        "a < import(c) || d >\ne;\n",
        // A second list behind a nested one: acorn-typescript reads a comparison over an
        // instantiation, tsc rejects or reads another chain.
        "f<A<B><C>>(x);\n",
        "f<A<B><C> + 1>(x);\n",
        "f<A<B><C>[]>(x);\n",
        "f<A<B><C>(e)>(x);\n",
        "f<A[A<B><C>[]]>(x);\n",
        // (inside an index, wherever an operand follows the second list for tsc's type
        // assertion to take — an arrow function's body included)
        "f<A[() => b<C><D>[0]]>(x);\n",
        "f<A[b<C><D> + 1]>(x);\n",
        "f<A[b<C><D><E>[0]]>(x);\n",
        "f<A[string<C><D>[]]>(x);\n",
        "f<A | this<C><D> + 1>(x);\n",
        "f<A[B\n<C>\n<D>[]]>(x);\n",
        "f<A | B\n<C><D> + 1>(x);\n",
        // An arrow function whose parameter list holds a default, in an index that opens
        // as a type: tsc reads a function type there, acorn-typescript an arrow function.
        "f<A[A[(a = 1) => 0]]>(x);\n",
        "f<A<B>[(a = 1) => B]>(x);\n",
        "f<A[A[<T>(a = 1) => 0]]>(x);\n",
        "f<A<B>[<T>(a = 1) => B]>(x);\n",
        // (behind a shell or an indexed access in an index too, where an operand follows)
        "f<A[(b)<C><D> + 1]>(x);\n",
        "f<A[B[K]<C><D> + 1]>(x);\n",
        "f<A[(typeof b)<C><D>[0]]>(x);\n",
        // An assertion predicate's subject takes an `is` and nothing else: with a bar
        // behind it the index body is neither a type acorn-typescript reads nor an
        // expression.
        "f<A[(a) => asserts a | B]>(x);\n",
    ] {
        let arena = bumpalo::Bump::new();
        assert!(tsv_ts::parse(source, &arena).is_err(), "{source:?}");
    }
}

/// A negative literal ahead of a postfix the type grammar has no spelling for — a call's
/// arguments, a template, a member spaced from the digits. acorn-typescript reads each
/// literal with its expression parser, which takes the postfix, so the line is a generic
/// call over the literal type to it; tsc reads a comparison chain, and that chain is what
/// tsv parses and prints — at the list's own level, and in an index whose body does not
/// open as a type. Where the member tail is GLUED to the digits, or the index body does
/// open as one, the region stays claimed instead (`claimed_regions_stay_rejected`): the
/// spellings are one construct read two ways, which the catalog's negative-literal entry
/// records.
#[test]
fn negative_literal_postfix_reads_as_the_comparison_tsc_reads() {
    for (source, printed) in [
        ("f<-1 .x>(x);\n", "f < -(1).x > x;\n"),
        ("f<-1(e)>(x);\n", "f < -1(e) > x;\n"),
        ("f<-1`t`>(x);\n", "f < -1`t` > x;\n"),
        // a tail the claim's byte run does not reach to the list's close
        ("f<-1..x + 1>(x);\n", "f < -(1).x + 1 > x;\n"),
        ("f<-1..x.y(e)>(x);\n", "f < -(1).x.y(e) > x;\n"),
        // in an index whose body opens on the literal itself, on an arrow function, or on
        // a head no type-continuation token follows
        ("f<A[-1(e)]>(x);\n", "f < A[-1(e)] > x;\n"),
        ("f<A[-1`t`]>(x);\n", "f < A[-1`t`] > x;\n"),
        ("f<A[-1?.x]>(x);\n", "f < A[-(1)?.x] > x;\n"),
        ("f<A[-1(e) | B]>`t`;\n", "f < A[-1(e) | B] > `t`;\n"),
        ("f<A[(-1(e)) | B]>(x);\n", "f < A[-1(e) | B] > x;\n"),
        ("f<A[() => -1 .x]>(x);\n", "f < A[() => -(1).x] > x;\n"),
        ("f<A[(a) => -1 .x]>(x);\n", "f < A[(a) => -(1).x] > x;\n"),
        ("f<A[(a) => -1!]>(x);\n", "f < A[(a) => -1!] > x;\n"),
        (
            "f<A[() => B | -1(e)]>(x);\n",
            "f < A[() => B | -1(e)] > x;\n",
        ),
        (
            "f<A[readonly -1(e)]>(x);\n",
            "f < A[readonly - 1(e)] > x;\n",
        ),
        ("f<A[{} | -1(e)]>(x);\n", "f < A[{} | -1(e)] > x;\n"),
        ("f<A[K][-1(e)]>(x);\n", "f < A[K][-1(e)] > x;\n"),
        ("f<(A)[-1(e)]>(x);\n", "f < A[-1(e)] > x;\n"),
        // (a body whose second operand opens past a line break opens as no type)
        ("f<A[B\n[0] | -1(e)]>(x);\n", "(f < A[B[0] | -1(e)]) > x;\n"),
        // (a glued member tail with a further postfix, the literal opening the body)
        ("f<A[-1..x()]>(x);\n", "f < A[-(1).x()] > x;\n"),
        ("f<A[-1..x!]>(x);\n", "f < A[-(1).x!] > x;\n"),
        ("f<A[-1..$x]>(x);\n", "f < A[-(1).$x] > x;\n"),
        ("f<A[-1!]>(x);\n", "f < A[-1!] > x;\n"),
        ("f<(-1 .x())>(x);\n", "f < -(1).x() > x;\n"),
        ("f<-1..$x>(x);\n", "f < -(1).$x > x;\n"),
        // (behind an arrow function's bare-name parameter, which prints in a pair)
        (
            "f<A[(a => B | -1..x)]>[0];\n",
            "f < A[(a) => B | -(1).x] > [0];\n",
        ),
        (
            "f<A[a => B | -1(e)]>(x);\n",
            "f < A[(a) => B | -1(e)] > x;\n",
        ),
        // (a commented sign, which prints in a pair of its own)
        (
            "f<A[() => B | - /* c */ 1 | C]> x;\n",
            "f < A[() => B | -(/* c */ 1) | C] > x;\n",
        ),
        // (a glued member tail where the index opens as no type)
        ("f<A[() => -1..x]>(x);\n", "f < A[() => -(1).x] > x;\n"),
        ("f<A[B | -(1).x]>(x);\n", "(f < A[B | -(1).x]) > x;\n"),
        // (a shelled literal's call where the index opens as no type, and its member anywhere)
        ("f<A[-(1)(e)]>(x);\n", "f < A[-1(e)] > x;\n"),
        (
            "f<A[() => B | -(1)(e)]>(x);\n",
            "f < A[() => B | -1(e)] > x;\n",
        ),
        (
            "x = f < A[B | -(1).x] > y;\n",
            "x = (f < A[B | -(1).x]) > y;\n",
        ),
        (
            "x = f < A[() => B | -(1).x] > y;\n",
            "x = f < A[() => B | -(1).x] > y;\n",
        ),
        ("x = (f < A[-1 .x]) > y;\n", "x = f < A[-(1).x] > y;\n"),
        // (an operator word behind the literal is no postfix)
        ("f<A[B | -1 in y]>(x);\n", "(f < A[B | ((-1) in y)]) > x;\n"),
        // (a shift is no type continuation)
        (
            "f<A[B << c > d > [0] | -1(e)]>(x);\n",
            "f < A[(B << c > d > [0]) | -1(e)] > x;\n",
        ),
        // (a `!=` is an operator)
        ("f<A[B | -1 != c]>(x);\n", "(f < A[B | (-1 != c)]) > x;\n"),
        // (a conditional's `?` is no postfix, with or without a `.`-led literal behind it)
        (
            "f<A[B | -1 ? a : b]>(x);\n",
            "(f < A[B | -1 ? a : b]) > x;\n",
        ),
        (
            "f<A[B | -1 ?.5 : b]>(x);\n",
            "(f < A[B | -1 ? 0.5 : b]) > x;\n",
        ),
        // (the claim is of the index alone: the region's own close still decides)
        ("f<A[B | -1(e)]> y;\n", "(f < A[B | -1(e)]) > y;\n"),
    ] {
        assert_prints_fixed(source, printed);
        assert_ne!(expression_kind(source), "CallExpression", "{source:?}");
    }
}

/// An index body that opens on `{` or `[` is graded no further, as an arrow function's
/// body there is not — each line of
/// `typescript/expressions/binary/relational_index_ungraded_body_svelte_divergence`, whose
/// `tsv_rejects.txt` reads the first error alone.
#[test]
fn ungraded_index_bodies_stay_rejected() {
    for source in [
        "a < b[{ c: d + 1 }] > (t, u);\n",
        "a < b[{ ...s }] > (t, u);\n",
        "a < b[[c || d]] > (t, u);\n",
        "a < b[[c++]] > `t`;\n",
        "a < b[[c, d + 1]] > (t, u);\n",
        "a < b[[c + 1][0]] > (t, u);\n",
        "a < b | c[{ ...s }] > (t, u);\n",
        "a < b[c][{ ...s }] > (t, u);\n",
        "a < b[() => [c + 1]] > (t, u);\n",
        "a < b[() => { return c; }] > (t, u);\n",
    ] {
        let arena = bumpalo::Bump::new();
        assert!(tsv_ts::parse(source, &arena).is_err(), "{source:?}");
    }
}

/// A kept shell with a list behind it heads its region whatever stands ABOVE that list on
/// the printed spine — a second call, a member, an optional call, an arithmetic tail —
/// since the parser's claim behind such a shell reads nothing past the list. So the chain
/// keeps its pair there, at a width that puts the `>` at a line's end included.
#[test]
fn kept_shell_lists_keep_their_pair_under_more_spine() {
    for (source, printed) in [
        (
            "x = f < (typeof a)<C>(e)(g) > y;\n",
            "x = (f < (typeof a)<C>(e)(g)) > y;\n",
        ),
        (
            "x = f < (typeof a)<C>(e).m > y;\n",
            "x = (f < (typeof a)<C>(e).m) > y;\n",
        ),
        (
            "x = f < (!a)<C>`t`.m > y;\n",
            "x = (f < (!a)<C>`t`.m) > y;\n",
        ),
        (
            "x = f < (a | b)<C>?.(e) > y;\n",
            "x = (f < (a | b)<C>?.(e)) > y;\n",
        ),
        (
            "x = f < (typeof a)<C>(e) + 1 > y;\n",
            "x = (f < (typeof a)<C>(e) + 1) > y;\n",
        ),
        ("x = f < (-1)<C>(e)! > y;\n", "x = (f < (-1)<C>(e)!) > y;\n"),
        (
            "x = f < ((typeof a)<C>(e) as T) > y;\n",
            "x = (f < ((typeof a)<C>(e) as T)) > y;\n",
        ),
        // a computed member in the shell is one more content the parser's claim covers
        (
            "x = f < (!a[0])<T>(d) > c;\n",
            "x = (f < (!a[0])<T>(d)) > c;\n",
        ),
        // an OPTIONAL generic call prints `?.` behind the shell, which opens no list
        (
            "x = f < (typeof a)?.<T>(d) > c;\n",
            "x = f < (typeof a)?.<T>(d) > c;\n",
        ),
        // a computed member under another hop closes the region at that hop
        ("x = f < (a = b)[0].m > c;\n", "x = f < (a = b)[0].m > c;\n"),
        // a union that holds a call is no type to the parser's grade of the shell
        (
            "x = f < (a() | b)<C>(e) > y;\n",
            "x = f < (a() | b)<C>(e) > y;\n",
        ),
        (
            "x = f < (a | b())<C>(e) > y;\n",
            "x = f < (a | b())<C>(e) > y;\n",
        ),
        // a shell whose content the pair rule does not answer for stays bare under any spine
        (
            "x = f < (a ? b : c)<C>(e).m > y;\n",
            "x = f < (a ? b : c)<C>(e).m > y;\n",
        ),
    ] {
        assert_prints_fixed(source, printed);
    }

    // past the print width, where the `>` ends a line: each printed form reads back
    let tail = "y".repeat(50);
    for (head, printed_head) in [
        (
            "x = f < (typeof a)<C>(e)(g) >",
            "x =\n\t(f < (typeof a)<C>(e)(g)) >",
        ),
        (
            "x = f < (typeof a)<C>(e).m >",
            "x =\n\t(f < (typeof a)<C>(e).m) >",
        ),
        ("x = f < (!a)<C>`t`.m >", "x =\n\t(f < (!a)<C>`t`.m) >"),
        (
            "x = f < (a | b)<C>?.(e) >",
            "x =\n\t(f < (a | b)<C>?.(e)) >",
        ),
        (
            "x = f < (typeof a)<C>(e) + 1 >",
            "x =\n\t(f < (typeof a)<C>(e) + 1) >",
        ),
        ("x = f < (!a[0])<T>(d) >", "x =\n\t(f < (!a[0])<T>(d)) >"),
    ] {
        let source = format!("{head} {tail} + {tail};\n");
        let printed = format!("{printed_head}\n\t{tail} +\n\t\t{tail};\n");
        assert_prints_fixed(&source, &printed);
    }
    // a JSDoc cast keeps its own pair, and heads the region the same way
    assert_prints_fixed(
        &format!("x = f < /** @type {{T}} */ (a)<C>(e).m > {tail} + {tail};\n"),
        &format!("x =\n\t(f < /** @type {{T}} */ (a)<C>(e).m) >\n\t{tail} +\n\t\t{tail};\n"),
    );
    assert_prints_fixed(
        &format!("if (f < (typeof a)<C>(e)(g) > {tail} + {tail}) z;\n"),
        &format!("if (\n\t(f < (typeof a)<C>(e)(g)) >\n\t{tail} +\n\t\t{tail}\n)\n\tz;\n"),
    );
    // a `//` in the shell breaks the chain at any width
    assert_prints_fixed(
        "x = f < (// c\ntypeof a)<C<D>>(e)(g) > y;\n",
        "x =\n\t(f <\n\t\t// c\n\t\t(typeof a)<C<D>>(e)(g)) >\n\ty;\n",
    );
}

/// Two regions the parse claims on a form the printer itself writes, so the chain owes
/// its pair there: a BigInt literal's member, which prints glued to the suffix, and — the
/// other way round — a type query's index, where no claim is made and none is owed.
#[test]
fn claims_the_printed_form_holds_are_read_on_it() {
    let tail = "y".repeat(50);
    for (source, printed) in [
        ("x = f < -1n.x > y;\n", "x = (f < -1n.x) > y;\n"),
        ("x = f < (-1n.x) > y;\n", "x = (f < -1n.x) > y;\n"),
        ("x = f < -1_0n.x > y;\n", "x = (f < -1_0n.x) > y;\n"),
        ("x = f < -1n .x > y;\n", "x = (f < -1n.x) > y;\n"),
        ("x = f < -0x1fn.x > y;\n", "x = (f < -0x1fn.x) > y;\n"),
        // (a call behind the member is no claim, glued or not)
        ("x = f < -1n.x() > y;\n", "x = f < -1n.x() > y;\n"),
    ] {
        assert_prints_fixed(source, printed);
        let close = " > y;\n";
        let wide = format!(
            "{} > {tail} + {tail};\n",
            &source[..source.len() - close.len()]
        );
        let out = format(&wide);
        assert_eq!(format(&out), out, "a fixed point at width: {source:?}");
    }

    // A type QUERY of an import is no import type: an index behind it is claimed by what
    // opens it, like any other, so a negative literal's postfix there is the chain.
    for (source, printed) in [
        (
            "f<typeof import('m')[-1(e)]>(x);\n",
            "f < typeof import('m')[-1(e)] > x;\n",
        ),
        (
            "f<typeof import('m').B[-1`t`]>(x);\n",
            "f < typeof import('m').B[-1`t`] > x;\n",
        ),
        (
            "f<typeof import('m')[-1!]>(x);\n",
            "f < typeof import('m')[-1!] > x;\n",
        ),
        (
            "f<A | typeof import('m')[-1(e)]>(x);\n",
            "(f < A) | (typeof import('m')[-1(e)] > x);\n",
        ),
    ] {
        assert_prints_fixed(source, printed);
    }
    let wide = format!("x = a < typeof import('m')[-1(e)] > {tail} + {tail};\n");
    let out = format(&wide);
    assert_eq!(format(&out), out, "a fixed point at width");
}

/// Index bodies both parsers read as an expression, each printed as the chain it is: a
/// negative literal's list ahead of an operand (a comparison over `-1 < C`), and an arrow
/// function whose parameter list holds a default, which no function type spells to
/// acorn-typescript.
#[test]
fn expression_index_bodies_print_as_their_chain() {
    for (source, printed) in [
        (
            "f<A[() => -1<C>[]]>(x);\n",
            "(f < A[() => (-1 < C) > []]) > x;\n",
        ),
        (
            "f<A[(a) => -1<C>[]]>(x);\n",
            "(f < A[(a) => (-1 < C) > []]) > x;\n",
        ),
        (
            "f<A[<T>() => -1<C>[]]>(x);\n",
            "(f < A[<T>() => (-1 < C) > []]) > x;\n",
        ),
        (
            "f<A[import('m') | -1<C>[]]>(x);\n",
            "(f < A[import('m') | ((-1 < C) > [])]) > x;\n",
        ),
        (
            "f<A[() => -1.5<C>[]]>(x);\n",
            "(f < A[() => (-1.5 < C) > []]) > x;\n",
        ),
        ("f<A[(a = 1) => 0]>(x);\n", "f < A[(a = 1) => 0] > x;\n"),
        ("f<A[(a = 1) => 0]>`t`;\n", "f < A[(a = 1) => 0] > `t`;\n"),
        (
            "f<A[(a: T = 1) => 0]>(x);\n",
            "f < A[(a: T = 1) => 0] > x;\n",
        ),
        (
            "f<A[(a, b = 1) => 0]>(x);\n",
            "f < A[(a, b = 1) => 0] > x;\n",
        ),
        (
            "f<A[({ a } = b) => 0]>(x);\n",
            "f < A[({ a } = b) => 0] > x;\n",
        ),
        ("f<A[(a = 1) => B]>(x);\n", "f < A[(a = 1) => B] > x;\n"),
        // an import with options is no import type to acorn-typescript: a query over one
        // with two lists is an expression where the index opens as no type
        (
            "f<A[() => typeof import('m', { with: { a: 'b' } })<C><D>]>(x);\n",
            "(f < A[() => typeof (import('m', { with: { a: 'b' } })<C>)<D>]) > x;\n",
        ),
        (
            "f<A[() => typeof import('m', { with: { a: 'b' } })<C><D>[K]]>(x);\n",
            "(f < A[() => ((typeof import('m', { with: { a: 'b' } })<C>) < D) > [K]]) > x;\n",
        ),
        // a negative literal's list past a line break, in an index that opens as no type
        (
            "f<A[() => -1\n<C>(e)]>(x);\n",
            "(f < A[() => -1<C>(e)]) > x;\n",
        ),
        // `async` past a break a comment carries into the output
        (
            "f<A[() => async /* c\n */<C<D>>(e)]>(x, y);\n",
            "(f <\n\tA[\n\t\t() =>\n\t\t\tasync/* c\n\t\t\t */ <C<D>>(e)\n\t]) >\n\t(x, y);\n",
        ),
        // an import type's `<` past a line break, inside the index of a negative literal:
        // acorn-typescript reads that index as an expression, so it welds no list there
        (
            "f<-1[import('m').B\n<C<D>>[]]>`t`;\n",
            "(f < -(1)[import('m').B < C < D >> []]) > `t`;\n",
        ),
        (
            "f<-1[(a: B) => import('m').B\n<C<D>>[]]>(x, y);\n",
            "(f < -(1)[(a: B) => import('m').B < C < D >> []]) > (x, y);\n",
        ),
        // (an `==` or an arrow in a parameter's own default is no default of the list's)
        (
            "f<A[(a: T, b = (c) => c == d) => 0]>(x);\n",
            "f < A[(a: T, b = (c) => c == d) => 0] > x;\n",
        ),
        // a generic head is the same split: the chain, with no pair the source did not have
        (
            "f<A[<T>(a = 1) => B]>(x, y);\n",
            "f < A[<T>(a = 1) => B] > (x, y);\n",
        ),
        (
            "f<A[<T>({ a } = {}) => B]>(x, y);\n",
            "f < A[<T>({ a } = {}) => B] > (x, y);\n",
        ),
        (
            "f<A[<T>(a = (b = 1)) => B]>`t`;\n",
            "f < A[<T>(a = (b = 1)) => B] > `t`;\n",
        ),
        (
            "f<A[() => <T>(a: T = 1) => B]>(x, y);\n",
            "f <\n\tA[\n\t\t() =>\n\t\t\t<T>(a: T = 1) =>\n\t\t\t\tB\n\t] >\n\t(x, y);\n",
        ),
        (
            "f<A[<T>(a = 1 // c\n) => B]>(x, y);\n",
            "f <\n\tA[\n\t\t<T>(\n\t\t\ta = 1 // c\n\t\t) => B\n\t] >\n\t(x, y);\n",
        ),
        // (a pair the author wrote is kept)
        (
            "(f < A[<T>(a = 1) => B]) > (x, y);\n",
            "(f < A[<T>(a = 1) => B]) > (x, y);\n",
        ),
        // (and a chain written bare stays bare, ahead of a follower that commits no list too)
        (
            "f<A[<T>(a = 1) => B]> [0];\n",
            "f < A[<T>(a = 1) => B] > [0];\n",
        ),
        // (with a return no type spells the head decides nothing: the chain both parsers
        // read, which takes its pair — in an index that opens as a type too)
        (
            "f<A[<T>(a = 1) => b.c()]>(x, y);\n",
            "(f < A[<T>(a = 1) => b.c()]) > (x, y);\n",
        ),
        (
            "f<import('m').B[<T>(a = 1) => b + 1]>`t`;\n",
            "(f < import('m').B[<T>(a = 1) => b + 1]) > `t`;\n",
        ),
    ] {
        assert_prints_fixed(source, printed);
        assert_ne!(expression_kind(source), "CallExpression", "{source:?}");
    }
    // at a width that ends a line on the `>`, the bare chain reads back
    let tail = "y".repeat(50);
    let out = format(&format!("x = f < A[(a = 1) => 0] > {tail} + {tail};\n"));
    assert_eq!(
        out,
        format!("x =\n\tf <\n\tA[(a = 1) => 0] >\n\t{tail} +\n\t\t{tail};\n")
    );
    assert_eq!(format(&out), out);
}

/// A non-null `!` inside an index is tsc's JSDoc non-nullable type and an expression's own
/// to acorn-typescript, whose chain tsv reads. Where the index opens as no type the chain
/// prints bare, so the compiler still reads the generic call it read; where it opens as
/// one the chain takes its pair.
#[test]
fn non_null_in_an_index_keeps_the_chain_bare_where_no_type_opens() {
    for (source, printed) in [
        (
            "f<A[() => typeof b!]>`t`;\n",
            "f < A[() => typeof b!] > `t`;\n",
        ),
        (
            "f<A[a => typeof b!]>(x);\n",
            "f < A[(a) => typeof b!] > x;\n",
        ),
        ("f<A[() => b[!c]]>`t`;\n", "f < A[() => b[!c]] > `t`;\n"),
        ("f<A[(B | C)!]>`t`;\n", "f < A[(B | C)!] > `t`;\n"),
        ("f<A[B | c!]>(x);\n", "(f < A[B | c!]) > x;\n"),
        // a pair the author wrote is kept: that text was the comparison to tsc already
        (
            "x = (f < A[() => typeof b!]) > `t`;\n",
            "x = (f < A[() => typeof b!]) > `t`;\n",
        ),
    ] {
        assert_prints_fixed(source, printed);
        assert_ne!(expression_kind(source), "CallExpression", "{source:?}");
    }
}

/// The same list in a Svelte template expression, where Svelte's own parser is
/// acorn-typescript: the leading-bar and spaced-sign spellings print bare and stay calls.
#[test]
fn template_expression_is_a_fixed_point() {
    let source = concat!(
        "<script lang=\"ts\"></script>\n\n",
        "{f<| A[K] | B>(x)}\n{a < (b[c] | d) > (e)}\n{f<A | - 1>(x)}\n",
    );
    let printed = concat!(
        "<script lang=\"ts\"></script>\n\n",
        "{f<A[K] | B>(x)}\n{a<b[c] | d>(e)}\n{f<A | -1>(x)}\n",
    );
    let out = svelte_format(source);
    assert_eq!(out, printed);
    assert_eq!(svelte_format(&out), out);
}
