// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A callback argument whose signature will break refuses the hug that renders its head on the
//! callee's line, and the call breaks every argument out — prettier's `ArgExpansionBailout`, asked
//! of each half its printers gate. The rows pin the standalone `.ts` path of that refusal, and the
//! halves the `calls/callback_signature_*` fixtures do not reach: an arrow's TYPE PARAMETERS (asked
//! beside its parameter list, as `printFunctionParameters` asks `willBreak(typeParametersDoc)` with
//! `willBreak(printed)`), a parameter default written on one line whose body still prints its
//! members on lines of their own (a block, a class), and the layout a `//` after `=>` takes, which
//! refuses the same signatures (the comment's relocation is cataloged, so no parity fixture holds
//! it). The doc layer forces breaks no source newline spells — a matrix array, a member chain the
//! chain printer breaks — so no source scan can stand in for asking the docs. A comment in an EMPTY
//! parameter list is the one break the refusal does not read (the list returns ahead of the
//! bailout): the argument hugs with the comment on its own line. The hugged sole parameter of a
//! `function` callback keeps the hug as a block-bodied arrow's does (prettier collapses the
//! authored object there and hugs; tsv keeps the expansion, `docs/conformance_prettier_ts.md`
//! §Hugged sole-parameter object type); a blank line between that object type's members refuses
//! the hug only where the literal prints expanded.

fn format(source: &str) -> String {
    tsv_ts::format_str(source).expect("formats")
}

/// Each pair is (authoring, the single-pass fixed point).
const CASES: &[(&str, &str)] = &[
    // an authored-expanded constraint breaks the type parameters, so every argument breaks out
    (
        "fn(x, <T extends {\n\ta: string;\n}>(a: T) => {\n\tg(a);\n});\n",
        "fn(\n\tx,\n\t<\n\t\tT extends {\n\t\t\ta: string;\n\t\t}\n\t>(\n\t\ta: T\n\t) => {\n\t\tg(a);\n\t}\n);\n",
    ),
    // a one-line default whose block body breaks breaks every argument out
    (
        "fn(x, (cb = () => { g(); }) => {\n\th();\n});\n",
        "fn(\n\tx,\n\t(\n\t\tcb = () => {\n\t\t\tg();\n\t\t}\n\t) => {\n\t\th();\n\t}\n);\n",
    ),
    // so does a class body, around a hugged array body
    (
        "fn((cb = class { a = 1; }) => [cb]);\n",
        "fn(\n\t(\n\t\tcb = class {\n\t\t\ta = 1;\n\t\t}\n\t) => [cb]\n);\n",
    ),
    // a matrix default array forces a break no newline spells, so every argument breaks out
    (
        "fn(lead, (a = [[1, 2], [3, 4]]) => g(a));\n",
        "fn(\n\tlead,\n\t(\n\t\ta = [\n\t\t\t[1, 2],\n\t\t\t[3, 4]\n\t\t]\n\t) => g(a)\n);\n",
    ),
    // so does a member chain of more than two calls the chain printer breaks
    (
        "fn(lead, (a = x.a(() => 1).b(() => 2).c(() => 3).d(() => 4)) => g(a));\n",
        "fn(\n\tlead,\n\t(\n\t\ta = x\n\t\t\t.a(() => 1)\n\t\t\t.b(() => 2)\n\t\t\t.c(() => 3)\n\t\t\t.d(() => 4)\n\t) => g(a)\n);\n",
    ),
    // a comment in an empty parameter list keeps its line and the hug, around a call body
    (
        "fn(x, (\n\t// c\n) => call());\n",
        "fn(x, (\n\t// c\n) => call());\n",
    ),
    // a curried callee asks the same question as every other expand-last site
    (
        "fn(a)(lead, (a = {\n\tb: 1\n}) => {\n\tg(a);\n});\n",
        "fn(a)(\n\tlead,\n\t(\n\t\ta = {\n\t\t\tb: 1\n\t\t}\n\t) => {\n\t\tg(a);\n\t}\n);\n",
    ),
    // a `//` after `=>` takes its own layout, which refuses the same signatures: after a
    // preceding argument, ...
    (
        "fn(lead, (a = {\n\tb: 1\n}) => // c\n\tg(a));\n",
        "fn(\n\tlead,\n\t(\n\t\ta = {\n\t\t\tb: 1\n\t\t}\n\t) => // c\n\t\tg(a)\n);\n",
    ),
    // ... in a member chain, ...
    (
        "obj.a.fn(lead, (a = {\n\tb: 1\n}) => // c\n\tg(a));\n",
        "obj.a.fn(\n\tlead,\n\t(\n\t\ta = {\n\t\t\tb: 1\n\t\t}\n\t) => // c\n\t\tg(a)\n);\n",
    ),
    // ... and as a member chain's lone argument
    (
        "obj.a.fn((a = {\n\tb: 1\n}) => // c\n\tg(a));\n",
        "obj.a.fn(\n\t(\n\t\ta = {\n\t\t\tb: 1\n\t\t}\n\t) => // c\n\t\tg(a)\n);\n",
    ),
    // a `function` callback's hugged sole parameter keeps the hug, the expansion kept
    (
        "fn(lead, function (a: {\n\tb: C;\n}) {\n\tg(a);\n});\n",
        "fn(lead, function (a: {\n\tb: C;\n}) {\n\tg(a);\n});\n",
    ),
    // a blank line between a hugged sole parameter's members breaks the signature only where
    // the literal prints expanded: written on one line, the literal prints flat, dropping the
    // blank (prettier expands it, a separate gap), so the hug stays; this row pins F1, not
    // the form
    (
        "fn(lead, (a: { b: C;\n\n\td: E }) => {\n\tg(a);\n});\n",
        "fn(lead, (a: { b: C; d: E }) => {\n\tg(a);\n});\n",
    ),
];

#[test]
fn callback_signature_hug_refusal_reaches_the_fixed_point() {
    for (authored, fixed_point) in CASES {
        let once = format(authored);
        assert_eq!(
            once, *fixed_point,
            "one pass must reach the fixed point for {authored:?}"
        );
        assert_eq!(format(&once), once, "not idempotent for {authored:?}");
    }
}
