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
//! - **both accept, as different programs** — a bare literal ahead of `[K]` runs only
//!   (`-1[K]`), and a pair the author wrote AROUND such a run (`(-1[K])[]`). tsv prints each
//!   as written, so neither parser's reading of the output differs from its reading of the
//!   input; adding a pair at the literal would hand acorn tsc's program instead of the one
//!   it read. tsv's own parse of these follows tsc rather than acorn, a separate parser
//!   divergence, so no `expected.json` can pin them either.

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
