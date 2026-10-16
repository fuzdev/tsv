// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A `<` after a would-be type-argument close refuses the list on every line.
//!
//! tsc's `canFollowTypeArgumentsInExpression` turns `<`, `>`, `+` and `-` away before it
//! asks about a line break — a list followed by `<` never makes sense to it — so
//! `a = f<T><U>(z)` is the comparison `(f < T) > <U>(z)`, whose right operand is the
//! angle-bracket type assertion `<U>(z)`, on one line or with the `<U>` on the next.
//! acorn-typescript reads a second type argument list on any line, the call
//! `(f<T>)<U>(z)`; tsv follows tsc. The bare spelling is no format fixed point (the
//! printer pairs the chain's `<` operand), so the oracle-backed side is the
//! `unformatted_ours_same_line` and `unformatted_ours_line_break` variants of the fixture
//! `typescript/expressions/binary/relational_chain_type_arg_parens_assertion_follow_prettier_divergence`,
//! and the tree a variant cannot pin is asserted here. So is the other side of the rule:
//! where tsc rejects the text — a list its comparison cannot read, or a comparison whose
//! `,` the context refuses, a Svelte head's grammar included — every list in it reads as
//! acorn-typescript reads it. The
//! catalog entry is `docs/conformance_svelte.md` §TypeScript Corrections.

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

fn parse_svelte_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let root = tsv_svelte::parse(source, &arena).expect("parse failed");
    tsv_debug::json::wire_value(&tsv_svelte::convert_ast_json_bytes(&root, source))
}

fn str_at<'a>(json: &'a Value, pointer: &str) -> Option<&'a str> {
    json.pointer(pointer).and_then(Value::as_str)
}

/// Asserts the node at `at` is the comparison chain `(head < T) > <U>operand`: one
/// statement, a `>` over a `<`, and a type assertion whose operand has kind `operand`.
fn assert_comparison_over_assertion(source: &str, at: &str, operand: &str) {
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
        Some("TSTypeAssertion"),
        "a type assertion as the right operand: {source:?} {json}"
    );
    assert_eq!(
        str_at(&json, &format!("{at}/right/expression/type")),
        Some(operand),
        "the assertion's operand: {source:?} {json}"
    );
}

#[test]
fn a_less_than_follower_refuses_the_list_on_any_line() {
    let at = "/body/0/expression/right";
    for source in [
        "a = f<T><U>(z);",
        "a = f<T> <U>(z);",
        "a = f<T> < U > (z);",
        "a = f<T>\n<U>(z);",
        "a = a.b<T><U>(z);",
        "a = a.b<T>\n<U>(z);",
        // every ECMAScript line terminator, and a comment holding one
        "a = f<T>\r\n<U>(z);",
        "a = f<T>\r<U>(z);",
        "a = f<T>\u{2028}<U>(z);",
        "a = f<T>\u{2029}<U>(z);",
        "a = f<T> // x\n<U>(z);",
        "a = f<T> /* x\n*/ <U>(z);",
    ] {
        assert_comparison_over_assertion(source, at, "Identifier");
    }
    // the assertion takes the whole unary operand: a member, a call, a template
    for (source, operand) in [
        ("a = f<T><U>(z).y;", "MemberExpression"),
        ("a = f<T>\n<U>(z)(y);", "CallExpression"),
        ("a = f<T><U>`z`;", "TemplateLiteral"),
        ("a = f<T>\n<U>(z)`t`;", "TaggedTemplateExpression"),
        ("a = f<T><U>(z, y);", "SequenceExpression"),
    ] {
        assert_comparison_over_assertion(source, at, operand);
    }
    // inside an argument list
    assert_comparison_over_assertion(
        "g(f<T><U>(z));",
        "/body/0/expression/arguments/0",
        "Identifier",
    );
    assert_comparison_over_assertion(
        "g(f<T>\n<U>(z));",
        "/body/0/expression/arguments/0",
        "Identifier",
    );
    // a third list is an assertion over an assertion
    let json = parse_json("a = f<T><U><V>(z);");
    assert_eq!(
        str_at(&json, &format!("{at}/right/expression/type")),
        Some("TSTypeAssertion"),
        "{json}"
    );
}

#[test]
fn a_new_callee_takes_no_arguments_and_is_the_less_than_operand() {
    let at = "/body/0/expression/right";
    for source in ["a = new f<T><U>(z);", "a = new f<T>\n<U>(z);"] {
        assert_comparison_over_assertion(source, at, "Identifier");
        let json = parse_json(source);
        assert_eq!(
            str_at(&json, &format!("{at}/left/left/type")),
            Some("NewExpression"),
            "{source:?} {json}"
        );
        assert_eq!(
            json.pointer(&format!("{at}/left/left/arguments"))
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0),
            "an argument-less `new`: {source:?} {json}"
        );
    }
}

#[test]
fn a_nested_close_reads_as_a_shift() {
    // `>>` closes nothing: `(f < A) < (B >> <U>(z))`
    for source in ["a = f<A<B>><U>(z);", "a = new f<A<B>>\n<U>(z);"] {
        let json = parse_json(source);
        let at = "/body/0/expression/right";
        assert_eq!(
            str_at(&json, &format!("{at}/operator")),
            Some("<"),
            "{source:?} {json}"
        );
        assert_eq!(
            str_at(&json, &format!("{at}/left/operator")),
            Some("<"),
            "{source:?} {json}"
        );
        assert_eq!(
            str_at(&json, &format!("{at}/right/operator")),
            Some(">>"),
            "{source:?} {json}"
        );
        assert_eq!(
            str_at(&json, &format!("{at}/right/right/type")),
            Some("TSTypeAssertion"),
            "{source:?} {json}"
        );
    }
}

#[test]
fn a_list_that_a_paren_or_a_call_closes_is_kept() {
    // the pair settles the first list at its own `)`; a call's `)` is no list close
    for source in ["a = (f<T>)<U>(z);", "a = (f<T>)\n<U>(z);"] {
        let json = parse_json(source);
        assert_eq!(
            str_at(&json, "/body/0/expression/right/type"),
            Some("CallExpression"),
            "{source:?} {json}"
        );
        assert_eq!(
            str_at(&json, "/body/0/expression/right/callee/type"),
            Some("TSInstantiationExpression"),
            "{source:?} {json}"
        );
    }
    let json = parse_json("a = f<T>()<U>(z);");
    assert_eq!(
        str_at(&json, "/body/0/expression/right/callee/type"),
        Some("CallExpression"),
        "{json}"
    );
}

#[test]
fn the_printer_joins_the_line_into_the_paired_chain() {
    for (source, printed) in [
        ("a = f<T><U>(z);", "a = (f < T) > <U>z;\n"),
        ("a = f<T>\n<U>(z);", "a = (f < T) > <U>z;\n"),
        ("a = new f<T>\n<U>(z);", "a = (new f() < T) > <U>z;\n"),
        ("a = f<T><U>(z).y;", "a = (f < T) > <U>z.y;\n"),
        ("g(f<T>\n<U>(z));", "g((f < T) > <U>z);\n"),
    ] {
        assert_eq!(format(source), printed, "{source:?}");
    }
}

/// Asserts the node at `at` is acorn-typescript's call `f<…><…>(…)`: a `CallExpression`
/// whose callee is an instantiation, no comparison anywhere on the way down.
fn assert_acorn_call(json: &Value, at: &str, source: &str) {
    assert_eq!(
        str_at(json, &format!("{at}/type")),
        Some("CallExpression"),
        "{source:?} {json}"
    );
    assert_eq!(
        str_at(json, &format!("{at}/callee/type")),
        Some("TSInstantiationExpression"),
        "{source:?} {json}"
    );
}

#[test]
fn where_tsc_rejects_the_text_every_list_is_acorns() {
    // A list kept because its comparison does not parse (`f<() => A>` ahead of a `<`, a
    // `string[]` no expression reads) makes the text one tsc rejects, so acorn-typescript's
    // reading stands for every list in it: on the same spine, in an argument, in another
    // statement — never a comparison over one of its lists
    for (source, at) in [
        ("a = f<() => A><U><V>(x);", "/body/0/expression/right"),
        (
            "a = f<string[]><U>(x)<V><W>(y);",
            "/body/0/expression/right",
        ),
        ("a = f<void><U><V>(x);", "/body/0/expression/right"),
        ("a = new a.b<T><U>()<V><W>(y);", "/body/0/expression/right"),
        (
            "a = f<string[]><U>(g<T><V>(y));",
            "/body/0/expression/right/arguments/0",
        ),
        (
            "g(f<T><U>(x)); h<string[]><V>(y);",
            "/body/0/expression/arguments/0",
        ),
        (
            "h<string[]><V>(y); g(f<T><U>(x));",
            "/body/1/expression/arguments/0",
        ),
    ] {
        assert_acorn_call(&parse_json(source), at, source);
    }
    // `f<() => A><U><V>(x)` calls `f<() => A><U>` with `<V>`
    let json = parse_json("a = f<() => A><U><V>(x);");
    assert_eq!(
        str_at(&json, "/body/0/expression/right/callee/expression/type"),
        Some("TSInstantiationExpression"),
        "{json}"
    );
}

#[test]
fn a_comma_list_its_context_refuses_reads_as_acorn() {
    // tsc's comparison over a comma list parses as an expression, but the context then
    // refuses the `,`: tsc rejects the text, and acorn-typescript — so Svelte — reads
    // the lists
    for (source, at) in [
        ("const a = f<T, U><V>(x);", "/body/0/declarations/0/init"),
        (
            "const k = () => f<T, U><V>(x);",
            "/body/0/declarations/0/init/body",
        ),
        ("a ? f<T, U><V>(x) : d;", "/body/0/expression/consequent"),
        (
            "y = { k: f<T, U><V>(x) };",
            "/body/0/expression/right/properties/0/value",
        ),
        // and a comparison given up before it, in another statement
        (
            "g(f<T><U>(x)); const a = f<T, U><V>(x);",
            "/body/0/expression/arguments/0",
        ),
    ] {
        assert_acorn_call(&parse_json(source), at, source);
    }
    // a `+` past a line break: acorn's instantiation added to `c`
    let json = parse_json("const a = f<T, U>\n+c;");
    let at = "/body/0/declarations/0/init";
    assert_eq!(
        str_at(&json, &format!("{at}/operator")),
        Some("+"),
        "{json}"
    );
    assert_eq!(
        str_at(&json, &format!("{at}/left/type")),
        Some("TSInstantiationExpression"),
        "{json}"
    );
    // the same through Svelte, in a `<script lang="ts">` and a template expression
    let json = parse_svelte_json(
        "<script lang=\"ts\">const a = f<T, U><V>(x);</script>{a ? f<T, U><V>(x) : d}",
    );
    let script = "/instance/content/body/0/declarations/0/init";
    assert_acorn_call(&json, script, "script");
    assert_acorn_call(&json, "/fragment/nodes/0/expression/consequent", "template");
    // a text both parsers reject keeps the first parse's error
    let arena = bumpalo::Bump::new();
    assert!(tsv_ts::parse("const a = f<T, U><V>(x) +;", &arena).is_err());
}

#[test]
fn a_svelte_head_refusing_tscs_comparison_reads_as_acorn() {
    // The host's refusal of an island is part of the island's parse: a head whose
    // grammar refuses tsc's comparison reading — a bare second declarator, a stop at
    // the comparison's `,` short of the `as` / `then`, a sequence where a call must
    // stand — is re-read as acorn-typescript reads it, as Svelte reads it
    const TS: &str = "<script lang=\"ts\"></script>";
    for (template, at) in [
        (
            "{#each xs as x}{@const a = f<T, U><V>(x)}{a}{/each}",
            "/fragment/nodes/0/body/nodes/0/declaration/declarations/0/init",
        ),
        (
            "{#each f<T, U><V>(x) as item}{/each}",
            "/fragment/nodes/0/expression",
        ),
        (
            "{#each f<T, U><V>(x)}{/each}",
            "/fragment/nodes/0/expression",
        ),
        (
            "{#each f<T, U><V>(x), i}{/each}",
            "/fragment/nodes/0/expression",
        ),
        // the iterable's re-read past an assertion run
        (
            "{#each f<T, U><V>(x) as A[] as item}{/each}",
            "/fragment/nodes/0/expression/expression",
        ),
        (
            "{#await f<T, U><V>(x) then v}{/await}",
            "/fragment/nodes/0/expression",
        ),
        (
            "{#await f<T, U><V>(x) catch e}{/await}",
            "/fragment/nodes/0/expression",
        ),
        (
            "{#await f<T, U><V>(x)}{/await}",
            "/fragment/nodes/0/expression",
        ),
        ("{@render f<T, U><V>(x)}", "/fragment/nodes/0/expression"),
        // tsc's comparison is valid here, but no call: `{@render}` refuses it
        ("{@render f<T><U>(x)}", "/fragment/nodes/0/expression"),
        // under tsc's comparison the line break ends the declaration ahead of `>`
        (
            "{const a = f<T,\nU\n><V>(x)}",
            "/fragment/nodes/0/declaration/declarations/0/init",
        ),
        (
            "{let a = f<T,\nU\n><V>(x)}",
            "/fragment/nodes/0/declaration/declarations/0/init",
        ),
    ] {
        let source = format!("{TS}{template}");
        assert_acorn_call(&parse_svelte_json(&source), at, template);
    }
}

#[test]
fn a_svelte_head_tsc_reads_keeps_the_comparison() {
    // where the head takes tsc's reading, it stands: `(f < T) > <U>(x)`
    const TS: &str = "<script lang=\"ts\"></script>";
    for (template, at) in [
        (
            "{#each xs as x}{@const a = f<T><U>(x)}{a}{/each}",
            "/fragment/nodes/0/body/nodes/0/declaration/declarations/0/init",
        ),
        (
            "{#each f<T><U>(x) as item}{/each}",
            "/fragment/nodes/0/expression",
        ),
        (
            "{#await f<T><U>(x) then v}{/await}",
            "/fragment/nodes/0/expression",
        ),
        (
            "{const a = f<T><U>(x)}",
            "/fragment/nodes/0/declaration/declarations/0/init",
        ),
        (
            "{#if f<T, U><V>(x)}{/if}",
            "/fragment/nodes/0/test/expressions/1",
        ),
    ] {
        let source = format!("{TS}{template}");
        let json = parse_svelte_json(&source);
        assert_eq!(
            str_at(&json, &format!("{at}/operator")),
            Some(">"),
            "{template:?} {json}"
        );
        assert_eq!(
            str_at(&json, &format!("{at}/right/type")),
            Some("TSTypeAssertion"),
            "{template:?} {json}"
        );
    }
    // a head neither reading admits keeps the host's own refusal
    for (template, message) in [
        ("{#each f<T, U><V>(x) junk}{/each}", "Expected '}'"),
        (
            "{#each xs as x}{@const a = 1, b = 2}{/each}",
            "{@const ...} must consist of a single variable declaration",
        ),
        ("{#await f<T, U><V>(x) junk}{/await}", "Expected '}'"),
        (
            "{@render a < b}",
            "{@render ...} tags can only contain call expressions",
        ),
        (
            "{const a, b = 1}",
            "`const` declarations require an initializer",
        ),
    ] {
        let source = format!("{TS}{template}");
        let arena = bumpalo::Bump::new();
        let error = tsv_svelte::parse(&source, &arena).expect_err(template);
        assert!(error.to_string().contains(message), "{template:?}: {error}");
    }
}

#[test]
fn a_svelte_head_both_readings_refuse_reports_acorns_error() {
    // Where both readings fail in an embedded text, the re-parse's error is the one
    // reported, whether the host refused a reading or the parse failed on its own: Svelte
    // reads every island and `<script>` as acorn does, so that error lands where Svelte's
    // does, not at tsc's stop inside the type-argument list. `at` is the text the error
    // points at, where Svelte's own points.
    const TS: &str = "<script lang=\"ts\"></script>\n";
    for (template, message, at) in [
        ("{a ? f<T, U><V>(x) : }", "Expected expression", "}"),
        ("{const a = f<T, U><V>(x) junk}", "Expected ';'", "junk}"),
        (
            "{const a = f<T, U><V>(x) /* c */}",
            "unexpected content after declaration",
            " /* c */}",
        ),
        (
            "{const a = f<T, U><V>(x), y}",
            "`const` declarations require an initializer",
            "const a",
        ),
        ("{#each f<T, U><V> as item}{/each}", "Expected '}'", "item}"),
        ("{#each f<T, U><V>(x) junk}{/each}", "Expected '}'", "junk}"),
        (
            "{#await f<T, U><V>(x) junk}{/await}",
            "Expected '}'",
            "junk}",
        ),
    ] {
        let source = format!("{TS}{template}");
        let arena = bumpalo::Bump::new();
        let error = tsv_svelte::parse(&source, &arena).expect_err(template);
        assert!(error.to_string().contains(message), "{template:?}: {error}");
        assert_eq!(
            error.position(),
            source.find(at),
            "{template:?} points at {at:?}: {error}"
        );
    }
    for (source, message, at) in [
        (
            "<script lang=\"ts\">const a = f<T, U><V>(x) junk;</script>",
            "Expected ';'",
            "junk;",
        ),
        (
            "<script lang=\"ts\">a ? f<T, U><V>(x) : ;</script>",
            "Expected expression, found ';'",
            ";</script>",
        ),
        (
            "<script lang=\"ts\">let a = f<T, U><V>(x), 1;</script>",
            "Expected identifier or destructuring pattern",
            "1;",
        ),
        (
            "<script lang=\"ts\">x = { p: f<T, U><V>(x) junk };</script>",
            "Expected ',' or '}' after list element",
            "junk };",
        ),
    ] {
        let arena = bumpalo::Bump::new();
        let error = tsv_svelte::parse(source, &arena).expect_err(source);
        assert!(error.to_string().contains(message), "{source:?}: {error}");
        assert_eq!(
            error.position(),
            source.find(at),
            "{source:?} points at {at:?}: {error}"
        );
    }
    // a standalone text has no host reading it as acorn does: where both readings fail
    // inside the parse, tsc's reading's error is reported
    for (source, message, at) in [
        ("a ? f<T, U><V>(x) : ;", "Expected ':', found ','", ", U>"),
        (
            "const a = f<T, U><V>(x) +;",
            "Expected expression, found ';'",
            ";",
        ),
    ] {
        let arena = bumpalo::Bump::new();
        let error = tsv_ts::parse(source, &arena).expect_err(source);
        assert!(error.to_string().contains(message), "{source:?}: {error}");
        assert_eq!(
            error.position(),
            source.find(at),
            "{source:?} points at {at:?}: {error}"
        );
    }
}

#[test]
fn a_comma_list_tsc_reads_stays_the_comparison() {
    // in an argument list the comparison's `,` separates two arguments: tsc accepts the
    // text, and its reading stands
    let json = parse_json("g(f<T, U><V>(x));");
    let at = "/body/0/expression/arguments";
    assert_eq!(
        json.pointer(at).and_then(Value::as_array).map(Vec::len),
        Some(2),
        "{json}"
    );
    assert_eq!(
        str_at(&json, &format!("{at}/0/operator")),
        Some("<"),
        "{json}"
    );
    assert_eq!(
        str_at(&json, &format!("{at}/1/operator")),
        Some(">"),
        "{json}"
    );
    assert_eq!(
        str_at(&json, &format!("{at}/1/right/type")),
        Some("TSTypeAssertion"),
        "{json}"
    );
}

#[test]
fn a_three_close_reads_as_a_shift() {
    // `f<A<B<C>>><U>(x)` is tsc's `((f < A) < B) < (C >>> <U>(x))`
    let json = parse_json("a = f<A<B<C>>><U>(x);");
    let at = "/body/0/expression/right";
    assert_eq!(
        str_at(&json, &format!("{at}/operator")),
        Some("<"),
        "{json}"
    );
    assert_eq!(
        str_at(&json, &format!("{at}/left/operator")),
        Some("<"),
        "{json}"
    );
    assert_eq!(
        str_at(&json, &format!("{at}/left/left/operator")),
        Some("<"),
        "{json}"
    );
    assert_eq!(
        str_at(&json, &format!("{at}/right/operator")),
        Some(">>>"),
        "{json}"
    );
    assert_eq!(
        str_at(&json, &format!("{at}/right/right/type")),
        Some("TSTypeAssertion"),
        "{json}"
    );
}

/// The operator tree of the expression `node` holds, parens erased: what each test below
/// asserts the source and the printed text both read as.
fn tree(node: &Value) -> String {
    let kind = node.get("type").and_then(Value::as_str).unwrap_or("?");
    let child = |key: &str| node.get(key).map(tree).unwrap_or_default();
    let operator = || node.get("operator").and_then(Value::as_str).unwrap_or("?");
    match kind {
        "Identifier" => node
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_owned(),
        "Literal" => node
            .get("raw")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_owned(),
        "TemplateLiteral" => "`t`".to_owned(),
        "BinaryExpression" | "AssignmentExpression" => {
            format!("({} {} {})", child("left"), operator(), child("right"))
        }
        "MemberExpression" if node.get("computed").and_then(Value::as_bool) == Some(true) => {
            format!("{}[{}]", child("object"), child("property"))
        }
        "MemberExpression" => format!("{}.{}", child("object"), child("property")),
        "NewExpression" => format!("new {}()", child("callee")),
        "TSTypeReference" => child("typeName"),
        "TSTypeAssertion" => format!("<{}>{}", child("typeAnnotation"), child("expression")),
        "TSInstantiationExpression" => {
            let params = node
                .pointer("/typeArguments/params")
                .and_then(Value::as_array)
                .map(|params| params.iter().map(tree).collect::<Vec<_>>().join(", "))
                .unwrap_or_default();
            format!("{}<{params}>", child("expression"))
        }
        other => other.to_owned(),
    }
}

fn expression_tree(source: &str) -> String {
    tree(
        parse_json(source)
            .pointer("/body/0/expression")
            .expect("an expression statement"),
    )
}

#[test]
fn a_relational_chain_over_a_shift_reads_as_tscs_tree() {
    // Each tree is tsc's tree of the source (checked against tsc's parser): a `<` region a
    // shift closes is no type-argument list, and an assertion after the shift is the
    // shift's right operand
    for (source, tsc_tree) in [
        ("a < b < c >> d << e;", "((a < b) < ((c >> d) << e))"),
        ("a < b < c >> d >>> e;", "((a < b) < ((c >> d) >>> e))"),
        (
            "f < A < B < C >>> d >> <U>x;",
            "(((f < A) < B) < ((C >>> d) >> <U>x))",
        ),
        (
            "f < A < B < C >> d >> <U>x;",
            "(((f < A) < B) < ((C >> d) >> <U>x))",
        ),
        (
            "x = (c < d) < d >> (c << d) >>> A<B>;",
            "(x = ((c < d) < ((d >> (c << d)) >>> A<B>)))",
        ),
        (
            "x = (a < b) < (c >> (d + 1)) << e;",
            "(x = ((a < b) < ((c >> (d + 1)) << e)))",
        ),
        (
            "(a < b) < (c < d) >> (C > d) << o.p;",
            "((a < b) < (((c < d) >> (C > d)) << o.p))",
        ),
        (
            "(a < b) < (c << d) < `t` >> A[0] >>> A[0];",
            "(((a < b) < (c << d)) < ((`t` >> A[0]) >>> A[0]))",
        ),
        (
            "x = (c < d) < (c >> d) > d;",
            "(x = (((c < d) < (c >> d)) > d))",
        ),
        (
            "x = c < d < c >> d > d;",
            "(x = (((c < d) < (c >> d)) > d))",
        ),
    ] {
        assert_eq!(expression_tree(source), tsc_tree, "{source:?}");
    }
}
