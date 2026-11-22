// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A `/` after a `}`, read by `tsv_ts`'s own paren scans.
//!
//! Two scans in `tsv_ts` find a `)` ahead of the parse by walking bytes — the arrow-head
//! lookahead and the printer's paren scan — and each has to read a `/` as this crate's parser
//! will, or a regex literal holding a `)` or a `=>` ends the scan early, and a division read as
//! a regex runs the scan on past its statement. After a `}` they read the `/` as the grammar
//! does (`tsv_ts::__private::OPERAND_GRAMMAR`): a statement's `}` — a block after a statement
//! that ended without its `;`, a declaration's body — is followed by a regex, and an object
//! literal's or a function expression's by a division.
//!
//! A tokenizer that takes a statement's brace for an object literal's — acorn's does, at
//! several of them — reads the `/` after it as a division, and a scan reading it that way
//! stops at the `)` inside the regex: the program is rejected, or a comment moves across the
//! `)` the scan stopped short of. acorn's reading of a `}` belongs to the scan of a region
//! acorn parses (`tsv_ts::__private::ACORN_ISLAND_GRAMMAR`, a Svelte island's extent) — where each
//! of these braces, directly inside a body, answers as a block's too.

use tsv_lang::source_scan::{BracedOperandWalk, OperandGrammar, ScanStart};

fn format(source: &str) -> String {
    let program = tsv_ts::__private::parse(source).expect("parse failed");
    tsv_ts::__private::format_ast(&program, source)
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
fn the_crates_own_scans_read_a_brace_as_its_reader_does() {
    // This crate's scans read a `}` as its parser does, an island's as acorn does: both
    // divide after an object literal at the start of an expression.
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
    assert_eq!(closes(tsv_ts::__private::OPERAND_GRAMMAR), Some(true));
    assert_eq!(closes(tsv_ts::__private::ACORN_ISLAND_GRAMMAR), Some(true));
}

/// Whether `source` parses as a module.
fn parses(source: &str) -> bool {
    tsv_ts::__private::parse(source).is_ok()
}

/// The tails a scan that ran past its statement could read: nothing, a later `/` on the same
/// line (in code and in a comment), and lines that close a misread regex and spell a `)` and
/// an `=>` beyond it.
const TAILS: [&str; 5] = [
    "",
    " // a/b",
    "; y / 2;",
    "\ny = /) => 1/;",
    "\nconst z = ') => 1';",
];

#[test]
fn a_heads_verdict_never_depends_on_what_follows_the_statement() {
    // Accepted whatever follows: the `}` is an operand's, and the `/` after it divides.
    for head in [
        "x = (a = {} / 2) => a;",
        "x = (a = { b: 1 } / 2) => a / 2;",
        "x = (a = function () {} / 2) => a / 2;",
        "x = (a = class {} / 2) => a / 2;",
        "x = (a = async function () {} / 2) => a / 2;",
        "x = (a = c ? b : function () {} / 2) => a / 2;",
        "x = (a = b as {} / 2) => a / 2;",
        "x = (a = function (): { b: 1 } {} / 2) => a / 2;",
        "x = (a = `${{} / 2}`) => a / 2;",
        "f((a = {} / (2 / 1)) => a);",
        "x = c ? (a = {} / 2): T => a / 2 : y;",
        "x = (a = () => { y = {} / 2; }) => a / 2;",
        "x = (a = () => { if (c) {}\n/re/.test(s); }) => a / 2;",
        "x = <T,>(a = {} / 2) => a / 2;",
        // A method's body after its return type holds statements.
        "x = (a = class { m(): number { if (c) {}\n/re/.test(s); return 1 } }) => a;",
        "x = (a = class { m(): number { {} /)/.test(s); return 1 } }) => a;",
        "x = (a = { m(): number { {} /)/.test(s); return 1 } }) => a;",
        "x = (a = class { m(): { b: 1 } { {} /)/.test(s); return 1 } }) => a;",
        "x = (a = class { get b(): number { {} /)/.test(s); return 1 } }) => a;",
        "x = (a = class extends B { m(): number { {} /)/.test(s); return 1 } }) => a;",
        "x = (a = class { p: { b: 1 } = { b: 1 }; q = {} / 2 }) => a;",
        "x = (a = class { m(): number { return {} / 2 } }) => a;",
    ] {
        for tail in TAILS {
            let source = format!("{head}{tail}");
            assert!(parses(&source), "{source:?}");
        }
    }
    // A brace the scan cannot place stops it as the end of the input would, so the verdict
    // is the same whatever follows: an object or type literal at statement level before a
    // line break, which divides after `y = {}` and opens a regex after `type T = {}`.
    for head in [
        "x = (a = () => { type T = {}\n/re/.test(s); }) => a / 2;",
        "x = (a = () => { y = {}\n/ 2; }) => a / 2;",
    ] {
        let verdict = parses(head);
        for tail in TAILS {
            let source = format!("{head}{tail}");
            assert_eq!(parses(&source), verdict, "{source:?}");
        }
    }
}
