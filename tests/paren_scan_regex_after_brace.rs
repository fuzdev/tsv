// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A `/` after a statement's `}`, read by `tsv_ts`'s own paren scans.
//!
//! Two scans in `tsv_ts` find a `)` ahead of the parse by walking bytes — the arrow-head
//! lookahead and the printer's paren scan — and each has to read a `/` as this crate's parser
//! will, or a regex literal holding a `)` or a `=>` ends the scan early. After a `}` they read
//! a regex (`tsv_ts::OPERAND_GRAMMAR` leaves the `}` to the bytes), which is what a statement's
//! `}` is followed by: a block after a statement that ended without its `;`, a declaration's
//! body, a type literal.
//!
//! A tokenizer that takes one of those braces for an object literal's — acorn's does, at
//! several of them — reads the `/` after it as a division, and a scan reading it that way
//! stops at the `)` inside the regex: the program is rejected, or a comment moves across the
//! `)` the scan stopped short of. acorn's reading of a `}` belongs to the scan of a region
//! acorn parses (`tsv_ts::ACORN_ISLAND_GRAMMAR`, a Svelte island's extent) — where each of
//! these braces, directly inside a body, answers as a block's too.

use tsv_lang::source_scan::{BracedOperandWalk, OperandGrammar, ScanStart};

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

#[test]
fn an_arrow_head_scan_reads_a_regex_after_a_statements_brace() {
    for (source, formatted) in [
        (
            "x = (() => {\n\ta\n\t{} /)=>/.test(s);\n});\n",
            "x = () => {\n\ta;\n\t{\n\t}\n\t/)=>/.test(s);\n};\n",
        ),
        (
            "x = (() => {\n\ta = b\n\t{}\n\t/)=>/.test(s);\n});\n",
            "x = () => {\n\ta = b;\n\t{\n\t}\n\t/)=>/.test(s);\n};\n",
        ),
        (
            "x = (() => {\n\tl: {}\n\t/)=>/.test(s);\n});\n",
            "x = () => {\n\tl: {\n\t}\n\t/)=>/.test(s);\n};\n",
        ),
        (
            "x = (() => {\n\tif (c) {}\n\t/)=>/.test(s);\n});\n",
            "x = () => {\n\tif (c) {\n\t}\n\t/)=>/.test(s);\n};\n",
        ),
        (
            "x = (() => {\n\tfunction f() {}\n\t/)=>/.test(s);\n});\n",
            "x = () => {\n\tfunction f() {}\n\t/)=>/.test(s);\n};\n",
        ),
        (
            "x = (() => {\n\tclass A {}\n\t/)=>/.test(s);\n});\n",
            "x = () => {\n\tclass A {}\n\t/)=>/.test(s);\n};\n",
        ),
        (
            "x = (() => {\n\tabstract class A {}\n\t/)=>/.test(s);\n});\n",
            "x = () => {\n\tabstract class A {}\n\t/)=>/.test(s);\n};\n",
        ),
        (
            "x = (() => {\n\tenum E { A }\n\t/)=>/.test(s);\n});\n",
            "x = () => {\n\tenum E {\n\t\tA\n\t}\n\t/)=>/.test(s);\n};\n",
        ),
        (
            "x = (() => {\n\tinterface I {}\n\t/)=>/.test(s);\n});\n",
            "x = () => {\n\tinterface I {}\n\t/)=>/.test(s);\n};\n",
        ),
        (
            "x = (() => {\n\tnamespace N {}\n\t/)=>/.test(s);\n});\n",
            "x = () => {\n\tnamespace N {}\n\t/)=>/.test(s);\n};\n",
        ),
        (
            "x = (() => {\n\ttype T = {}\n\t/)=>/.test(s);\n});\n",
            "x = () => {\n\ttype T = {};\n\t/)=>/.test(s);\n};\n",
        ),
        (
            "x = (() => {\n\tdeclare const z: {}\n\t/)=>/.test(s);\n});\n",
            "x = () => {\n\tdeclare const z: {};\n\t/)=>/.test(s);\n};\n",
        ),
        (
            "x = (() => {\n\tfunction f(): {} {}\n\t/)=>/.test(s);\n});\n",
            "x = () => {\n\tfunction f(): {} {}\n\t/)=>/.test(s);\n};\n",
        ),
        (
            "x = (() => {\n\tenum E { A }\n\t/\\)/.test(s); // c\n});\n",
            "x = () => {\n\tenum E {\n\t\tA\n\t}\n\t/\\)/.test(s); // c\n};\n",
        ),
        (
            "x = (() => {\n\tenum E { A }\n\t/[(]/.test(s);\n});\n",
            "x = () => {\n\tenum E {\n\t\tA\n\t}\n\t/[(]/.test(s);\n};\n",
        ),
        (
            "x = ((a) => {\n\tenum E { A }\n\t/[)] => /.test(s);\n});\n",
            "x = (a) => {\n\tenum E {\n\t\tA\n\t}\n\t/[)] => /.test(s);\n};\n",
        ),
    ] {
        assert_eq!(format(source), formatted, "{source:?}");
    }
}

#[test]
fn a_printer_paren_scan_keeps_each_comment_on_its_side_of_the_paren() {
    // The printer finds a head's `)` to split the comments around it. A scan that stopped
    // at the `)` inside the regex put the comment after the real one on both sides of it.
    for (source, formatted) in [
        (
            "function f(a = () => {\n\tenum E { A }\n\t/\\)/.test(s);\n}, /* c */ b) { /* d */ }\n",
            "function f(\n\ta = () => {\n\t\tenum E {\n\t\t\tA\n\t\t}\n\t\t/\\)/.test(s);\n\t},\n\t/* c */ b\n) {\n\t/* d */\n}\n",
        ),
        (
            "function f(a = () => {\n\tx\n\t{} /[)]/.test(s);\n} /* e */) /* c */ { /* d */ }\n",
            "function f(\n\ta = () => {\n\t\tx;\n\t\t{\n\t\t}\n\t\t/[)]/.test(s);\n\t} /* e */\n) /* c */ {\n\t/* d */\n}\n",
        ),
        (
            "const g = (a = () => {\n\tinterface I {}\n\t/[)]/.test(s);\n} /* e */) /* c */ => { /* d */ };\n",
            "const g = (\n\ta = () => {\n\t\tinterface I {}\n\t\t/[)]/.test(s);\n\t} /* e */\n) /* c */ => {\n\t/* d */\n};\n",
        ),
        (
            "class C { m(a = () => {\n\ttype T = {}\n\t/[)]/.test(s);\n} /* e */) /* c */ { /* d */ } }\n",
            "class C {\n\tm(\n\t\ta = () => {\n\t\t\ttype T = {};\n\t\t\t/[)]/.test(s);\n\t\t} /* e */\n\t) /* c */ {\n\t\t/* d */\n\t}\n}\n",
        ),
        (
            "if ((() => {\n\tenum E { A }\n\t/[)]/.test(s);\n})() /* e */) /* c */ { /* d */ }\n",
            "if (\n\t(() => {\n\t\tenum E {\n\t\t\tA\n\t\t}\n\t\t/[)]/.test(s);\n\t})() /* e */\n) /* c */ {\n\t/* d */\n}\n",
        ),
        (
            "foo((() => {\n\tnamespace N {}\n\t/[)]/.test(s);\n}) /* e */, /* c */ 1);\n",
            "foo(\n\t() => {\n\t\tnamespace N {}\n\t\t/[)]/.test(s);\n\t} /* e */,\n\t/* c */ 1\n);\n",
        ),
    ] {
        assert_eq!(format(source), formatted, "{source:?}");
        assert_eq!(format(formatted), formatted, "{formatted:?}");
    }
}

#[test]
fn the_crates_own_scans_never_ask_what_a_brace_closed() {
    // Only the island grammar carries the walk that tells an object literal's `}` from a
    // block's; this crate's scans read a regex after every one.
    let object_divided = b"{} / 2";
    let closes = |grammar: OperandGrammar| {
        (grammar.closes_braced_operand)(
            object_divided,
            1,
            0,
            ScanStart::Expression,
            &mut BracedOperandWalk::default(),
        )
    };
    assert!(!closes(tsv_ts::OPERAND_GRAMMAR));
    assert!(closes(tsv_ts::ACORN_ISLAND_GRAMMAR));
}
