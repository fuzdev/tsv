// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! The edges of the rule that the operand behind a `>` which may close a type-argument
//! region prints BARE — no clarity pair of the printer's own directly behind that token
//! (`BinaryExpression::may_close_type_arguments`, `needs_parens`'s
//! `clarity_pairs_behind_region_close`, `docs/conformance_prettier_ts.md` §Relational chain
//! type-argument parens).
//!
//! The oracle-backed body of the rule is fixtures
//! (`expressions/binary/relational_region_inner_gt_clarity_pair_prettier_divergence`, its
//! `_long` and `_recovered_list` siblings). What is pinned here are the cells a fixture
//! cannot hold, each for one of these reasons:
//!
//! - **the output is not a fixed point.** Where the pair behind the `>` is one the
//!   printed tokens need, the walk leaves everything as it is, and that output still
//!   opens on a `(` — the rule's known gap, which a later pass reads as a generic call.
//!   An `input.*` must format to itself, so those cells can only be asserted from the
//!   input side: what is pinned is that the pair whose `)` keeps two tokens apart is not
//!   the one withheld.
//! - **the cell is one of many near-identical ones.** How many `<` a `>>` or a `>>>` needs
//!   ahead of it, and where a region dies, is the parser's scoping — one shape per rule,
//!   which a fixture would carry as a pile of lines differing in one token.
//! - **the comment sits where a fixture's prose does.** A comment between the `>` and its
//!   operand is authored on the cell's own line.
//!
//! Every case is also asserted a fixed point unless it names itself as the gap: a pair
//! printed behind such a `>` shows as a second pass that rewrites the comparisons into a
//! call's type arguments.

/// `source` formatted, asserted a fixed point.
fn stable(source: &str) -> String {
    let output = tsv_ts::format_str(source).expect("the case parses");
    assert_eq!(
        tsv_ts::format_str(&output).expect("the output reparses"),
        output,
        "not a fixed point: {source}"
    );
    output.trim_end().to_string()
}

#[test]
fn every_clarity_pair_between_the_close_and_the_first_token_is_withheld() {
    for (source, expected) in [
        // The `await`'s pair, the mixed-arithmetic left operand's, and both at once.
        ("fn(a < b, c > await d);", "fn(a < b, c > await d);"),
        ("fn(a < b, c > d % e + f);", "fn(a < b, c > d % e + f);"),
        (
            "fn(a < b, c > await d % e + f);",
            "fn(a < b, c > await d % e + f);",
        ),
        (
            "fn(a < b, c > d % e % f + g);",
            "fn(a < b, c > d % e % f + g);",
        ),
        ("fn(a < b, c > -d % e + f);", "fn(a < b, c > -d % e + f);"),
        // Only what prints FIRST: an `await` further in keeps its pair.
        (
            "fn(a < b, c > d + await e);",
            "fn(a < b, c > d + (await e));",
        ),
        (
            "fn(a < b, c > !await d + e);",
            "fn(a < b, c > !(await d) + e);",
        ),
        // The `await`'s own operand pair is its own.
        (
            "fn(a < b, c > await (d, e));",
            "fn(a < b, c > await (d, e));",
        ),
        // Two `>` behind one region are each such a token.
        (
            "fn(a < b, c > await d, e > await f);",
            "fn(a < b, c > await d, e > await f);",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn a_called_or_tagging_function_takes_no_pair_behind_the_close() {
    // The pair around a function expression that is called, or used as a tag, is the
    // same kind: for the reader, with a bare spelling every parser reads alike. Each
    // case is that bare spelling and must print as written.
    for source in [
        "fn(a < b, c > function () {}());",
        "fn(a < b, c > function f() {}());",
        "fn(a < b, c > function* () {}());",
        "fn(a < b, c > async function () {}());",
        "fn(a < b, c > function () {}?.());",
        "fn(a < b, c > function () {}<T>());",
        "fn(a < b, c > function () {}`t`);",
        "fn(a < b, c > function () {}<T>`t`);",
        // Called twice, and as the base of a longer chain.
        "fn(a < b, c > function () {}()());",
        "fn(a < b, c > function () {}().e);",
        "fn(a < b, c > function () {}().e());",
        "fn(a < b, c > function () {}`t`.e);",
        // Under the clarity pairs above it, which are withheld with it.
        "fn(a < b, c > function () {}() % e + f);",
        "fn(a < b, c > function () {}().e % f + g);",
        // Behind a `>>` that closes two.
        "fn(a < b < c, d >> function () {}());",
    ] {
        assert_eq!(stable(source), source, "{source}");
    }
    for (source, expected) in [
        // No region open, or too few: the pair prints as it does anywhere.
        ("fn(c > function () {}());", "fn(c > (function () {})());"),
        (
            "fn(c > function () {}().e);",
            "fn(c > (function () {})().e);",
        ),
        (
            "fn(g(a < b), c > function () {}`t`);",
            "fn(g(a < b), c > (function () {})`t`);",
        ),
        (
            "fn(a < b, c >> function () {}());",
            "fn(a < b, c >> (function () {})());",
        ),
        // Only what prints FIRST behind the close.
        (
            "fn(a < b, c > d + function () {}());",
            "fn(a < b, c > d + (function () {})());",
        ),
        (
            "fn(a < b, c > new function () {}());",
            "fn(a < b, c > new (function () {})());",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn two_case_tests_with_no_statement_between_are_one_region() {
    // Only a `:` and a keyword part them, and tsv's own type-argument scan reads through
    // both: with the pair, the second case is a line its parser rejects. So the region
    // stays open across them, and the operand prints bare.
    for source in [
        "switch (k) {\n\tcase a < b < c:\n\tcase d >> await e:\n}",
        "switch (k) {\n\tcase a < b < c:\n\tdefault:\n\tcase d >> e % f + g:\n}",
        "switch (k) {\n\tcase a < b:\n\tcase c > await d:\n}",
    ] {
        assert_eq!(stable(source), source, "{source}");
    }
    assert!(
        tsv_ts::format_str("switch (k) {\n\tcase a < b < c:\n\tcase d >> (e):\n}").is_err(),
        "the scan stopped reading through a clause boundary: the region can end there too"
    );
}

#[test]
fn a_method_body_between_the_siblings_ends_no_region() {
    // An object literal's method is a body like an arrow's or a class's: its statements
    // start from the regions open around it, and those regions are still open past its
    // `}` — so the later sibling's operand prints bare, as it does with any other
    // argument between the two.
    for method in [
        "m() { x; }",
        "get m() { return x; }",
        "set m(v) { x; }",
        "async m() { x; }",
        "*m() { x; }",
        "async *m() { x; }",
        "[k]() { x; }",
        "'m'() { x; }",
    ] {
        for (template, bare) in [
            ("fn(a < b, { METHOD }, c > d % e + f);", "c > d % e + f"),
            ("x = [a < b, { METHOD }, c > await d];", "c > await d"),
        ] {
            let source = template.replace("METHOD", method);
            let output = stable(&source);
            assert!(output.contains(bare), "{source} printed {output}");
        }
    }
    // The body itself is inside the region, as an arrow's is.
    let output = stable("fn(a < b, { m() { return c > d % e + f; } });");
    assert!(output.contains("return c > d % e + f;"), "{output}");
}

#[test]
fn a_pair_the_author_wrote_stays() {
    for (source, expected) in [
        // No list can be read from these regions, so the parser built the comparison with
        // the pair in the source, and each authoring keeps its own.
        ("x = a < b && c > (await d);", "x = a < b && c > (await d);"),
        ("x = a < b && c > await d;", "x = a < b && c > await d;"),
        (
            "x = a < b && c > (d % e) + f;",
            "x = a < b && c > (d % e) + f;",
        ),
        // The walk stops AT the author's pair: the printer's own above it is withheld, and
        // everything inside it prints as it does anywhere.
        (
            "x = a < b && c > (await d) % e + f;",
            "x = a < b && c > (await d) % e + f;",
        ),
        (
            "x = a < b && c > (await d % e) + f;",
            "x = a < b && c > ((await d) % e) + f;",
        ),
        // A comment inside the pair does not hide it.
        (
            "x = a < b && c > (/* k */ await d);",
            "x = a < b && c > (/* k */ await d);",
        ),
        // A `(` before the operand is no pair around it without the `)` behind it: this
        // one wraps the whole right side, and is stripped as it is anywhere.
        (
            "x = a < b && c > (d % e + f);",
            "x = a < b && c > d % e + f;",
        ),
        // A called or tagging function's pair is the author's on the same terms.
        (
            "x = a < b && c > (function () {})();",
            "x = a < b && c > (function () {})();",
        ),
        (
            "x = a < b && c > function () {}();",
            "x = a < b && c > function () {}();",
        ),
        (
            "x = a < b && c > (function () {})`t`;",
            "x = a < b && c > (function () {})`t`;",
        ),
        (
            "x = a < b && c > (function () {})().e;",
            "x = a < b && c > (function () {})().e;",
        ),
        (
            "x = a < b && c > (function () {})() % e + f;",
            "x = a < b && c > (function () {})() % e + f;",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn a_pair_the_tree_owes_ends_the_walk_with_nothing_withheld() {
    // The operand still opens on a `(` — an assertion's pair, a sequence's own, a looser
    // operand's — so the clarity pairs above and inside it print as they do anywhere.
    // No list can be read from these regions, which is what lets each be a fixed point.
    for (source, expected) in [
        (
            "x = a < b && c > (d as T) % e + f;",
            "x = a < b && c > ((d as T) % e) + f;",
        ),
        (
            "x = a < b && c > (d % e + f) * g;",
            "x = a < b && c > ((d % e) + f) * g;",
        ),
        (
            "x = a < b && c > (d % e + f, g);",
            "x = a < b && c > ((d % e) + f, g);",
        ),
        // … and so do a member object's pair and a sequence's own, one level down.
        (
            "x = a < b && c > (await d).e % f + g;",
            "x = a < b && c > ((await d).e % f) + g;",
        ),
        (
            "x = a < b && c > (d, e).f % g + h;",
            "x = a < b && c > ((d, e).f % g) + h;",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn a_pair_whose_close_keeps_two_tokens_apart_is_not_withheld() {
    // Each output still opens on a `(` behind the `>` — the rule's known gap, so none is
    // asserted a fixed point. What must hold is that the pair survived: bare, the first is
    // `await g < T > +1` and the second ends a function body directly ahead of a `/`.
    // The last two are the same pairs one level down: the clarity pair above each is
    // left in place with it.
    for (source, kept) in [
        ("fn(a < b, c > await g<T>\n+ 1);", "c > (await g<T>) + 1"),
        (
            "fn(a < b, c > await function () {} / 2);",
            "c > (await function () {}) / 2",
        ),
        (
            "fn(a < b, c > function () {} / 2);",
            "c > (function () {}) / 2",
        ),
        ("fn(a < b, c > d * g<T>\n+ 1);", "c > (d * g<T>) + 1"),
        (
            "fn(a < b, c > await function () {} % 2 + f);",
            "c > ((await function () {}) % 2) + f",
        ),
        (
            "fn(a < b, c > await g<T> % 2 + f);",
            "c > ((await g<T>) % 2) + f",
        ),
        // A numeric literal's pair as a member object is the printer's own too, and its
        // bare spelling is one the number printer does not emit.
        ("fn(a < b, c > 0..e());", "c > (0).e()"),
    ] {
        let output = tsv_ts::format_str(source).expect("the case parses");
        assert!(output.contains(kept), "{source} printed {output}");
    }
}

#[test]
fn a_shift_closes_as_many_regions_as_it_has_angle_brackets() {
    for (source, expected) in [
        // One region is not enough for a `>>`, nor two for a `>>>` — however the two
        // opened: a `<<` is two and not three.
        ("fn(a < b, c >> await d);", "fn(a < b, c >> (await d));"),
        (
            "fn(a < b < c, d >>> await e);",
            "fn(a < b < c, d >>> (await e));",
        ),
        ("fn(a << b, c >>> await d);", "fn(a << b, c >>> (await d));"),
        // Two nested, two siblings, and a `<<`, which opens two.
        (
            "fn(a < b < c, d >> await e);",
            "fn(a < b < c, d >> await e);",
        ),
        (
            "fn(a < b, x < y, c >> await d);",
            "fn(a < b, x < y, c >> await d);",
        ),
        ("fn(a << b, c >> await d);", "fn(a << b, c >> await d);"),
        // Three, any way they open; a fourth changes nothing.
        (
            "fn(a < b < c < d, e >>> await f);",
            "fn(a < b < c < d, e >>> await f);",
        ),
        (
            "fn(a << b < c, d >>> await e);",
            "fn(a << b < c, d >>> await e);",
        ),
        (
            "fn(a < b < c < d < e, f >>> await g);",
            "fn(a < b < c < d < e, f >>> await g);",
        ),
        // A region that died is not counted: the call's `)` closed the first.
        (
            "fn(g(a < b) < c, d >> await e);",
            "fn(g(a < b) < c, d >> (await e));",
        ),
        // A grouping paren the printer strips closes nothing.
        (
            "fn((a < b) < c, d >> await e);",
            "fn(a < b < c, d >> await e);",
        ),
        // The root operand's own clarity pair under a shift is one of them.
        (
            "fn(a < b < c, d >> e % f + g);",
            "fn(a < b < c, d >> e % f + g);",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn a_comment_behind_the_close_stays_where_it_was_written() {
    for source in [
        "fn(a < b, c > /* k */ await d);",
        "fn(a < b, c > await /* k */ d);",
        "fn(a < b, c > await d /* k */);",
        "fn(a < b, c > /* k */ d % e + f);",
    ] {
        assert_eq!(stable(source), source, "{source}");
    }
}

#[test]
fn a_region_no_reading_reaches_keeps_the_pair() {
    for (source, expected) in [
        // The chain's own pair ends its region on a `)`.
        ("x = a < b > await c;", "x = (a < b) > (await c);"),
        // The closer of the delimiter the `<` was written in ends it.
        ("fn(g(a < b), c > await d);", "fn(g(a < b), c > (await d));"),
        (
            "fn([a < b], c > d % e + f);",
            "fn([a < b], c > (d % e) + f);",
        ),
        // A statement starts none.
        ("a < b; fn(c > await d);", "a < b;\nfn(c > (await d));"),
        // Nor does a module item: an `export` is a statement boundary like any other, and
        // so is everything declared under a later one.
        (
            "export const p = a < b;\nexport const q = c > await d;",
            "export const p = a < b;\nexport const q = c > (await d);",
        ),
        (
            "export const p = a < b;\nexport default c > await d;",
            "export const p = a < b;\nexport default c > (await d);",
        ),
        (
            "export const p = a < b;\nimport x from './x';\nexport const q = c > await d;",
            "export const p = a < b;\nimport x from './x';\nexport const q = c > (await d);",
        ),
        (
            "export const p = a < b;\nexport const f = async () => {\n\tconst q = c > await d;\n};",
            "export const p = a < b;\nexport const f = async () => {\n\tconst q = c > (await d);\n};",
        ),
        (
            "export const p = a < b;\nexport class A {\n\tasync m() {\n\t\treturn c > await d;\n\t}\n}",
            "export const p = a < b;\nexport class A {\n\tasync m() {\n\t\treturn c > (await d);\n\t}\n}",
        ),
        (
            "namespace N {\n\texport const p = a < b;\n\texport const q = c > d % e + f;\n}",
            "namespace N {\n\texport const p = a < b;\n\texport const q = c > (d % e) + f;\n}",
        ),
        // A `<` in the `>`'s own right operand does not stand ahead of it.
        ("fn(c > await d, a < b);", "fn(c > (await d), a < b);"),
        // Each `${…}` of a template is a delimiter of its own.
        (
            "x = `${a < b} ${c > await d}`;",
            "x = `${a < b} ${c > (await d)}`;",
        ),
        (
            "x = `${a < b}${i}${c > d % e + f}`;",
            "x = `${a < b}${i}${c > (d % e) + f}`;",
        ),
        // A do-while's test stands past its body, and a `case` test past the `switch`'s
        // discriminant and past the statements of the clause before it.
        (
            "do {\n\tp = a < b;\n} while (c > await d);",
            "do {\n\tp = a < b;\n} while (c > (await d));",
        ),
        (
            "do p = a < b;\nwhile (c > d % e + f);",
            "do p = a < b;\nwhile (c > (d % e) + f);",
        ),
        (
            "switch (a < b) {\n\tcase c > await d:\n}",
            "switch (a < b) {\n\tcase c > (await d):\n}",
        ),
        (
            "switch (k) {\n\tcase 1:\n\t\tp = a < b;\n\tcase c > d % e + f:\n}",
            "switch (k) {\n\tcase 1:\n\t\tp = a < b;\n\tcase c > (d % e) + f:\n}",
        ),
        // A method's body has regions of its own, and they die with it.
        (
            "x = {\n\tm() {\n\t\treturn a < b;\n\t},\n\tq: c > d % e + f\n};",
            "x = {\n\tm() {\n\t\treturn a < b;\n\t},\n\tq: c > (d % e) + f\n};",
        ),
        // A `<=` opens no region, and a `>=` closes none.
        ("fn(a <= b, c > await d);", "fn(a <= b, c > (await d));"),
        ("fn(a < b, c >= await d);", "fn(a < b, c >= (await d));"),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}
