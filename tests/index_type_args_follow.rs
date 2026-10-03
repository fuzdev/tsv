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
