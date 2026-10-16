// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A `+` or `-` after a would-be type-argument close refuses the list on every line.
//!
//! tsc's `canFollowTypeArgumentsInExpression` turns `<`, `>`, `+` and `-` away before it
//! asks about a line break — `+` and `-` are unary operators in that position — so
//! `a = f<T>⏎+c` is the comparison `(f < T) > +c`, as it is on one line. acorn-typescript
//! asks the line break first and reads `(f<T>) + c` past one; tsv follows tsc. The
//! spelling is no format fixed point (the printer joins the line and pairs the chain's `<`
//! operand), so the oracle-backed side is the `unformatted_ours_line_break` variant of the
//! fixture `typescript/expressions/binary/relational_chain_type_arg_parens_unary_follow_prettier_divergence`,
//! and the tree a variant cannot pin is asserted here. The catalog entry is
//! `docs/conformance_svelte.md` §TypeScript Corrections.

use serde_json::Value;

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_debug::json::wire_value(&tsv_ts::convert_ast_json_bytes(&program, source))
}

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

fn str_at<'a>(json: &'a Value, pointer: &str) -> Option<&'a str> {
    json.pointer(pointer).and_then(Value::as_str)
}

/// Asserts the node at `at` is the comparison chain `(head < T) > ±operand`: one
/// statement, a `>` over a `<`, and a unary right operand.
fn assert_comparison_chain(source: &str, at: &str, operator: &str) {
    let json = parse_json(source);
    assert_eq!(
        json.pointer("/body")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "one statement, no ASI split: {source:?} {json}"
    );
    assert_eq!(
        str_at(&json, &format!("{at}/type")),
        Some("BinaryExpression"),
        "{source:?} {json}"
    );
    assert_eq!(
        str_at(&json, &format!("{at}/operator")),
        Some(">"),
        "the chain's `>`: {source:?} {json}"
    );
    assert_eq!(
        str_at(&json, &format!("{at}/left/operator")),
        Some("<"),
        "the chain's `<`: {source:?} {json}"
    );
    assert_eq!(
        str_at(&json, &format!("{at}/right/type")),
        Some("UnaryExpression"),
        "a unary right operand: {source:?} {json}"
    );
    assert_eq!(
        str_at(&json, &format!("{at}/right/operator")),
        Some(operator),
        "{source:?} {json}"
    );
}

#[test]
fn a_unary_follower_past_a_line_break_refuses_the_list() {
    for (source, operator) in [
        ("a = f<T>\n+c;", "+"),
        ("a = f<T>\n-c;", "-"),
        ("a = f<T>\n+ +c;", "+"),
        ("a = f<T>\n- -c;", "-"),
        ("a = a.b<T>\n+c;", "+"),
        // every ECMAScript line terminator, and a comment holding one
        ("a = f<T>\r\n-c;", "-"),
        ("a = f<T>\r+c;", "+"),
        ("a = f<T>\u{2028}+c;", "+"),
        ("a = f<T>\u{2029}-c;", "-"),
        ("a = f<T> // x\n+c;", "+"),
        ("a = f<T> /* x\n*/ -c;", "-"),
    ] {
        assert_comparison_chain(source, "/body/0/expression/right", operator);
    }
    // inside an argument list
    assert_comparison_chain("g(f<T>\n+c);", "/body/0/expression/arguments/0", "+");
    // a `new` callee: the `new` takes no arguments and is the `<`'s left operand
    let at = "/body/0/expression/right";
    assert_comparison_chain("a = new f<T>\n-c;", at, "-");
    let json = parse_json("a = new f<T>\n-c;");
    assert_eq!(
        str_at(&json, &format!("{at}/left/left/type")),
        Some("NewExpression"),
        "{json}"
    );
    assert_eq!(
        json.pointer(&format!("{at}/left/left/arguments"))
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0),
        "an argument-less `new`: {json}"
    );
}

#[test]
fn the_same_line_reading_is_unchanged() {
    for (source, operator) in [
        ("a = f<T> + c;", "+"),
        ("a = f<T>-c;", "-"),
        ("a = f<T> /* x */ +c;", "+"),
    ] {
        assert_comparison_chain(source, "/body/0/expression/right", operator);
    }
}

#[test]
fn other_followers_past_a_line_break_keep_the_list() {
    // `++` / `--` and `+=` / `-=` are other tokens: past a line break tsc keeps the list,
    // ending the statement before a prefix update and assigning to the instantiation.
    let json = parse_json("a = f<T>\n++c;");
    assert_eq!(
        str_at(&json, "/body/0/expression/right/type"),
        Some("TSInstantiationExpression"),
        "{json}"
    );
    assert_eq!(
        str_at(&json, "/body/1/expression/type"),
        Some("UpdateExpression"),
        "{json}"
    );
    let json = parse_json("a = f<T>\n+= c;");
    assert_eq!(
        str_at(&json, "/body/0/expression/right/left/type"),
        Some("TSInstantiationExpression"),
        "{json}"
    );
    // a free-standing next statement, a call, a tagged template, a binary operator
    let json = parse_json("a = f<T>\nc;");
    assert_eq!(
        str_at(&json, "/body/0/expression/right/type"),
        Some("TSInstantiationExpression"),
        "{json}"
    );
    for (source, at, kind) in [
        (
            "a = f<T>\n(c);",
            "/body/0/expression/right",
            "CallExpression",
        ),
        (
            "a = f<T>\n`c`;",
            "/body/0/expression/right",
            "TaggedTemplateExpression",
        ),
        (
            "a = f<T>\n* c;",
            "/body/0/expression/right/left",
            "TSInstantiationExpression",
        ),
    ] {
        let json = parse_json(source);
        assert_eq!(
            str_at(&json, &format!("{at}/type")),
            Some(kind),
            "{source:?} {json}"
        );
    }
}

#[test]
fn the_printer_joins_the_line_into_the_paired_chain() {
    for (source, printed) in [
        ("a = f<T>\n+c;", "a = (f < T) > +c;\n"),
        ("a = new f<T>\n-c;", "a = (new f() < T) > -c;\n"),
        ("g(f<T>\n+c);", "g((f < T) > +c);\n"),
    ] {
        assert_eq!(format(source), printed, "{source:?}");
    }
}
