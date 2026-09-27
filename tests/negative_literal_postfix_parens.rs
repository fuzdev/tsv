// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A negative literal type (`-1`, `-1n`) as the operand of a type's postfix brackets — an
//! array suffix `[]` or an indexed access `[K]` — where the two parsers read the same text
//! as different programs.
//!
//! tsc reads `-` as a negative literal type when a numeric or bigint literal follows, and
//! then runs its postfix loop, so `-1[]` is an array of `-1` and `-1[K]` an indexed access
//! on it. acorn-typescript — tsv's parse oracle, and the parser Svelte itself uses — reads
//! the literal with its EXPRESSION parser (`parseMaybeUnary`), which takes every `[…]` that
//! follows as a computed member: `-1[K]` is the literal type `-(1[K])`, and `-1[]` is a
//! syntax error at the `]`. With the pair both read the type tsc reads, so the pair an
//! author writes is kept (the fixtures
//! `typescript/types/negative_literal_postfix_parens_prettier_divergence` and its
//! `_comment_` sibling, where prettier strips it).
//!
//! The spellings here are the bare ones those fixtures cannot carry:
//!
//! - **acorn rejects, tsc accepts** — a bare literal ahead of a run holding a `[]`
//!   (`-1[]`, `-1[K][]`, `readonly -1[]`). tsv parses tsc's tree and REPAIRS the spelling,
//!   printing the pair every parser reads alike (`(-1)[]`), as it does `fn<T> >= 1` in the
//!   mirror direction. The bare text is not a tsv fixed point, so it cannot be an
//!   `input.*`; and a `.svelte` variant is graded by `svelte compile`, which rejects it.
//!   This file is the only pin of these, the tree of `-1[]` included.
//! - **both accept, as different programs** — a bare literal ahead of `[K]` runs only
//!   (`-1[K]`), and a pair the author wrote AROUND such a run (`(-1[K])[]`). tsv prints each
//!   as written, so neither parser's reading of the output differs from its reading of the
//!   input; adding a pair at the literal would hand acorn tsc's program instead of the one
//!   it read. tsv's own parse of these follows tsc rather than acorn, a parser divergence
//!   the fixture `typescript/types/negative_literal_indexed_access_svelte_prettier_divergence`
//!   pins (both trees, and prettier stripping the pair around the run); they recur here
//!   only as formatting claims on the standalone-TS path.
//!
//! The parser divergence is cataloged in `docs/conformance_svelte.md` §TypeScript
//! Corrections.

use serde_json::Value;

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_debug::json::wire_value(&tsv_ts::convert_ast_json_bytes(&program, source))
}

/// The `type` of the node at `pointer`, or `<none>` where nothing is.
fn node_type(json: &Value, pointer: &str) -> String {
    json.pointer(pointer)
        .and_then(|n| n.get("type"))
        .and_then(Value::as_str)
        .map_or_else(|| "<none>".to_owned(), str::to_owned)
}

/// The alias's value is `postfix` over the negative literal type `-1`: the postfix node at
/// `/body/0/typeAnnotation`, its operand at `operand_key`, and that operand tsc's
/// `LiteralType(PrefixUnaryExpression(-, 1))` — acorn's wire shape for it.
fn assert_postfix_over_negative_literal(source: &str, postfix: &str, operand_key: &str) {
    let json = parse_json(source);
    let value = "/body/0/typeAnnotation";
    let operand = format!("{value}/{operand_key}");
    let literal = format!("{operand}/literal");
    assert_eq!(node_type(&json, value), postfix, "{source:?}");
    assert_eq!(node_type(&json, &operand), "TSLiteralType", "{source:?}");
    assert_eq!(node_type(&json, &literal), "UnaryExpression", "{source:?}");
    assert_eq!(
        json.pointer(&format!("{literal}/operator"))
            .and_then(Value::as_str),
        Some("-"),
        "{source:?}"
    );
    assert_eq!(
        node_type(&json, &format!("{literal}/argument")),
        "Literal",
        "{source:?}"
    );
}

#[test]
fn the_bare_array_form_parses_as_tsc_reads_it() {
    // an array of the literal type, where acorn-typescript rejects at the `]` — the one tree
    // no fixture can pin (the indexed-access trees are the `_svelte_prettier_divergence`
    // fixture's)
    assert_postfix_over_negative_literal("type A = -1[];\n", "TSArrayType", "elementType");
    assert_postfix_over_negative_literal("type A = -1n[];\n", "TSArrayType", "elementType");
}

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

/// `source` prints as `printed`, and `printed` is a fixed point.
fn assert_prints_fixed(source: &str, printed: &str) {
    let out = format(source);
    assert_eq!(out, printed, "printed from {source:?}");
    assert_eq!(format(&out), out, "a fixed point: {source:?}");
}

#[test]
fn a_bare_literal_acorn_cannot_read_gains_the_pair() {
    for (bare, repaired) in [
        ("type A = -1[];\n", "type A = (-1)[];\n"),
        ("type A = -1n[];\n", "type A = (-1n)[];\n"),
        ("type A = -1[][];\n", "type A = (-1)[][];\n"),
        ("type A = -1[K][];\n", "type A = (-1)[K][];\n"),
        ("type A = readonly -1[];\n", "type A = readonly (-1)[];\n"),
        ("type A = [...-1[]];\n", "type A = [...(-1)[]];\n"),
        ("let a: -1[] = [];\n", "let a: (-1)[] = [];\n"),
        ("f<-1[]>(x);\n", "f<(-1)[]>(x);\n"),
    ] {
        assert_prints_fixed(bare, repaired);
    }
}

#[test]
fn a_spelling_both_parsers_read_prints_as_written() {
    for written in [
        "type A = -1[K];\n",
        "type A = -1n['a'];\n",
        "type A = -1[K][K];\n",
        "type A = (-1[K])[];\n",
        "type A = (-1[K])[K];\n",
        "f<-1[K]>(x);\n",
    ] {
        assert_prints_fixed(written, written);
    }
    // A redundant second pair around the run collapses to the one that settles acorn's
    // reading, never to a pair at the literal.
    assert_prints_fixed("type A = ((-1[K]))[];\n", "type A = (-1[K])[];\n");
}

#[test]
fn a_comment_in_the_sign_gap_lays_out_as_under_the_authored_pair() {
    // The repaired pair is read as authored on the next pass, so every layout gate must
    // answer from it on the first: the `=` hugs the paired value either way.
    for (source, printed) in [
        ("type A = -// c\n1[];\n", "type A = (-// c\n1)[];\n"),
        ("type A = -// c\n1n[];\n", "type A = (-// c\n1n)[];\n"),
        ("type A = (-// c\n1)[];\n", "type A = (-// c\n1)[];\n"),
        ("type A = -/* c */ 1[];\n", "type A = (-/* c */ 1)[];\n"),
        ("type A = -\n1[];\n", "type A = (-1)[];\n"),
        ("let a: -// c\n1[] = [];\n", "let a: (-// c\n1)[] = [];\n"),
        (
            "type A = B | -// c\n1[];\n",
            "type A =\n\t| B\n\t| (-// c\n\t  1)[];\n",
        ),
        (
            "type A = [-// c\n1[]];\n",
            "type A = [\n\t(-// c\n\t1)[]\n];\n",
        ),
        ("f<-// c\n1[]>(x);\n", "f<\n\t(-// c\n\t1)[]\n>(x);\n"),
    ] {
        assert_prints_fixed(source, printed);
    }
}

#[test]
fn a_line_break_ends_the_type_before_the_brackets() {
    // tsc stops the postfix run at a line break, so the brackets start a new statement and
    // there is no operand to protect: the CONTRAST that bounds the class. After a pair
    // acorn-typescript stops there too; after a bare literal its member read crosses the
    // break and rejects, and the `;` tsv prints settles that reading as well.
    assert_prints_fixed("type A = (-1)\n[];\n", "type A = -1;\n[];\n");
    assert_prints_fixed("type A = -1\n[];\n", "type A = -1;\n[];\n");
}
