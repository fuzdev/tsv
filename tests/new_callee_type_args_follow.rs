// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A `new` whose callee carries type arguments and no argument list. What follows the close
//! decides which node the list belongs to, and a reading that finishes the `new` early, or
//! takes a list tsc refuses, builds a different program:
//!
//! - **a line break, then a member** — `new f<T>⏎[x](y)` constructs the computed member
//!   `f<T>[x]` (tsc and acorn-typescript alike: the break commits the list, and the member is
//!   part of the callee). Finishing the `new` at the close instead reads `new f<T>()[x](y)`,
//!   a call on a member of the constructed object. The fixture
//!   `typescript/expressions/new/callee_type_args_follow` pins the printed form through its
//!   `unformatted_line_break` variant; a variant's TREE has no pin shape there, so the trees
//!   are asserted here.
//! - **a second type argument list** — to acorn-typescript, whose tree Svelte compiles,
//!   `new f<T><U>(x)` constructs the instantiation `f<T>` with the type arguments `<U>`:
//!   `new (f<T>)<U>(x)`. tsc takes no list ahead of a `<` and reads the comparison
//!   `(new f < T) > <U>(x)`. tsv builds acorn's tree (the drop-in wire) and prints the
//!   spelling as written, so each parser reads the output as it read the input — the fixture
//!   `new/callee_type_args_second_list_prettier_divergence`, whose trees are asserted here
//!   (a fixture's `expected.json` holds one input's; these are the spellings around it).
//!   Where tsc's parser rejects the text outright — the assertion `<U>` left with no operand
//!   (`new a.b<T><U>()`, `new f<T><U>`, `new f<T><U>(...x)`), or a second list that is no
//!   single type (`new f<T><U, V>(x)`, `new f<T><U,>(x)`) — acorn still accepts, and tsv keeps acorn's tree and prints the
//!   REPAIRED spelling every parser reads alike, `new (a.b<T>)<U>()`: the repair
//!   `docs/conformance_prettier_ts.md` §TypeScript (Instantiation expression parens) states
//!   for `fn<T> >= 1`. The repair has no fixture: in a `<script>` body prettier's
//!   TypeScript parse is tsc's and throws on the bare spelling, and in a template prettier
//!   lands on `new a.b<T>()<U>()`, a call on the constructed object, which no marker
//!   documents. The call twin `f<T><U>` repairs the same way, to `(f<T>)<U>` (its
//!   template spelling is a fixture variant, below), while `f<T><U>(x)`, which tsc
//!   accepts, prints as written. An authored pair, `new (f<T>)<U>(x)`, is kept
//!   (`typescript_specific/generics/instantiation_paren_type_args_follow_prettier_divergence`).
//!
//! The line-break and repair readings are asserted through a Svelte template expression too,
//! the path the fixtures carry only in a `<script>` body.
//!
//! The `?.` follower every parser rejects (`new f<T>?.(x)`) is a fixture
//! (`input_invalid_*` in `new/callee_type_args_follow`), not an assertion here.

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

fn svelte_format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let root = tsv_svelte::parse(source, &arena).expect("component should parse");
    tsv_svelte::format(&root, source)
}

/// The shape of the first template expression tag's expression, from tsv's Svelte wire.
fn template_expression_shape(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let root = tsv_svelte::parse(source, &arena).expect("component should parse");
    let json = tsv_debug::json::wire_value(&tsv_svelte::convert_ast_json_bytes(&root, source));
    let nodes = json
        .pointer("/fragment/nodes")
        .and_then(Value::as_array)
        .expect("a fragment");
    let tag = nodes
        .iter()
        .find(|n| n.get("type").and_then(Value::as_str) == Some("ExpressionTag"))
        .expect("an expression tag");
    shape(tag.get("expression").expect("the tag's expression"))
}

fn component(template: &str) -> String {
    format!("<script lang=\"ts\"></script>\n\n{template}\n")
}

/// `source` prints as `printed`, and `printed` is a fixed point.
fn assert_prints_fixed(source: &str, printed: &str) {
    let out = format(source);
    assert_eq!(out, printed, "printed from {source:?}");
    assert_eq!(format(&out), out, "a fixed point: {source:?}");
}

fn type_args(node: &Value) -> String {
    node.get("typeArguments")
        .and_then(|t| t.get("params"))
        .and_then(Value::as_array)
        .map_or_else(String::new, |params| {
            let names: Vec<String> = params.iter().map(shape).collect();
            format!("<{}>", names.join(","))
        })
}

fn list(node: &Value, key: &str) -> String {
    node.get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().map(shape).collect::<Vec<_>>().join(","))
        .unwrap_or_default()
}

/// A compact rendering of an expression tree: every node the assertions below care about
/// spells its kind, so two trees that print alike but nest differently render differently.
fn shape(node: &Value) -> String {
    let kind = node.get("type").and_then(Value::as_str).unwrap_or("?");
    let child = |key: &str| node.get(key).map(shape).unwrap_or_default();
    match kind {
        "Identifier" => node
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_owned(),
        "TSTypeReference" => format!("{}{}", child("typeName"), type_args(node)),
        "TSInstantiationExpression" => format!("inst({}{})", child("expression"), type_args(node)),
        "NewExpression" => format!(
            "new[{}]{}({})",
            child("callee"),
            type_args(node),
            list(node, "arguments")
        ),
        "CallExpression" => format!(
            "call[{}]{}({})",
            child("callee"),
            type_args(node),
            list(node, "arguments")
        ),
        "MemberExpression" => {
            if node.get("computed").and_then(Value::as_bool) == Some(true) {
                format!("{}[{}]", child("object"), child("property"))
            } else {
                format!("{}.{}", child("object"), child("property"))
            }
        }
        "TaggedTemplateExpression" => format!("tag[{}]{}`", child("tag"), type_args(node)),
        other => other.to_owned(),
    }
}

fn expression_shape(source: &str) -> String {
    let json = parse_json(source);
    shape(
        json.pointer("/body/0/expression")
            .expect("an expression statement"),
    )
}

/// Past a line break the type arguments bind to the callee: the member after the break
/// is part of what `new` constructs, and the argument list (if any) is the `new`'s own.
#[test]
fn line_break_member_stays_in_the_callee() {
    for (source, expected) in [
        ("new f<T>\n[x](y);", "new[inst(f<T>)[x]](y)"),
        ("new f<T>\n[x];", "new[inst(f<T>)[x]]()"),
        ("new a.b<T>\n[x]();", "new[inst(a.b<T>)[x]]()"),
        ("new f<T>\n[x].y();", "new[inst(f<T>)[x].y]()"),
        ("new f<T>\n[x]\n`y`;", "new[tag[inst(f<T>)[x]]`]()"),
    ] {
        assert_eq!(
            expression_shape(source),
            expected,
            "the member after the break belongs to the callee: {source:?}"
        );
    }
}

/// The printed form keeps that reading on one line: the pair ends the list ahead of the
/// `[`, which bare would re-read the `<…>` as a comparison.
#[test]
fn line_break_member_prints_the_pair() {
    for (source, printed) in [
        ("new f<T>\n[x](y);", "new (f<T>)[x](y);\n"),
        ("new f<T>\n[x];", "new (f<T>)[x]();\n"),
        ("new f<T>\n[x]\n`y`;", "new (f<T>)[x]`y`();\n"),
    ] {
        let out = format(source);
        assert_eq!(out, printed, "printed from {source:?}");
        assert_eq!(
            expression_shape(&out),
            expression_shape(source),
            "the printed form reparses to the same tree: {source:?}"
        );
    }
}

/// A comment in the break's gap does not move the member out of the callee. Where the
/// printer puts the comment beside the pair it adds is not pinned here; that it survives,
/// and that the output reparses to the same tree, is.
#[test]
fn line_break_member_with_a_comment_stays_in_the_callee() {
    for (source, comment) in [
        ("new f<T>\n/*c*/[x](y);", "/*c*/"),
        ("new f<T> // c\n[x](y);", "// c"),
    ] {
        assert_eq!(
            expression_shape(source),
            "new[inst(f<T>)[x]](y)",
            "the member after the break belongs to the callee: {source:?}"
        );
        let out = format(source);
        assert!(
            out.contains(comment),
            "the comment survives: {source:?} -> {out:?}"
        );
        assert_eq!(
            expression_shape(&out),
            expression_shape(source),
            "the printed form reparses to the same tree: {source:?} -> {out:?}"
        );
    }
}

/// The same readings through a Svelte template expression, which tsv parses with the
/// same expression parser under Svelte's own wire.
#[test]
fn template_expression_reads_and_prints_the_same() {
    for (template, expected_shape, printed) in [
        (
            "{new f<T>\n[x](y)}",
            "new[inst(f<T>)[x]](y)",
            "{new (f<T>)[x](y)}",
        ),
        (
            "{new a.b<T><U>()}",
            "new[inst(a.b<T>)]<U>()",
            "{new (a.b<T>)<U>()}",
        ),
        ("{new f<T><U>}", "new[inst(f<T>)]<U>()", "{new (f<T>)<U>()}"),
        (
            "{new f<T><U>`x`}",
            "new[tag[inst(f<T>)]<U>`]()",
            "{new f<T><U>`x`}",
        ),
        (
            "{new f<T><U>(x)}",
            "new[inst(f<T>)]<U>(x)",
            "{new f<T><U>(x)}",
        ),
    ] {
        let source = component(template);
        assert_eq!(
            template_expression_shape(&source),
            expected_shape,
            "acorn-typescript's tree: {template:?}"
        );
        assert_eq!(
            svelte_format(&source),
            component(printed),
            "printed from {template:?}"
        );
        assert_eq!(
            template_expression_shape(&component(printed)),
            expected_shape,
            "the printed form reparses to the same tree: {template:?}"
        );
    }
}

/// A second type argument list after the close belongs to the `new` (or the call, or the
/// tag), and the first stays on the callee as an instantiation: acorn-typescript's tree, the
/// one Svelte compiles, whether or not tsc's parser accepts the spelling.
#[test]
fn second_type_argument_list_reads_as_acorn_does() {
    for (source, expected) in [
        ("new f<T><U>(x);", "new[inst(f<T>)]<U>(x)"),
        ("new a.b<T><U>(x);", "new[inst(a.b<T>)]<U>(x)"),
        ("new f<T><U>(x)(y);", "call[new[inst(f<T>)]<U>(x)](y)"),
        ("new f<T><U>(x, y);", "new[inst(f<T>)]<U>(x,y)"),
        ("new f<T>\n<U>(x);", "new[inst(f<T>)]<U>(x)"),
        ("new f<A<B>><U>(x);", "new[inst(f<A<B>>)]<U>(x)"),
        ("new f<T><U>`x`;", "new[tag[inst(f<T>)]<U>`]()"),
        // tsc's parser rejects these; acorn-typescript's tree is still the wire
        ("new f<T><U>;", "new[inst(f<T>)]<U>()"),
        ("new f<T><U>();", "new[inst(f<T>)]<U>()"),
        ("new a.b<T><U>();", "new[inst(a.b<T>)]<U>()"),
        ("new f<T>\n<U>;", "new[inst(f<T>)]<U>()"),
        ("new f<T><U>(...x);", "new[inst(f<T>)]<U>(SpreadElement)"),
        ("new f<T><U,>(x);", "new[inst(f<T>)]<U>(x)"),
        // the call twin, already acorn's
        ("f<T><U>(x);", "call[inst(f<T>)]<U>(x)"),
    ] {
        assert_eq!(
            expression_shape(source),
            expected,
            "acorn-typescript's tree: {source:?}"
        );
    }
}

/// tsc accepts these as comparisons, so they print as written: an argument list appended
/// to a `new` that tsc reads as the comparison would join the assertion's operand
/// (`` <U>`x`() ``, `<U>(x)()`) and change tsc's reading, and a pair added inside a longer
/// chain of lists would move where tsc's comparison ends.
#[test]
fn comparison_read_spellings_print_as_written() {
    for source in [
        "new f<T><U>`x`;\n",
        "new f<T><U>`x`.y;\n",
        "new new f<T><U>(x);\n",
        "new new f<T><U>`x`;\n",
        "new f<T><U><V>(x);\n",
        "f<T><U><V>(x);\n",
    ] {
        assert_eq!(format(source), source);
    }
}

/// Where tsc's parser rejects the bare spelling, tsv prints the pair that settles the first
/// list, so tsc reads the output as acorn-typescript read the input.
#[test]
fn tsc_rejected_second_type_argument_list_prints_the_repair() {
    for (source, printed) in [
        ("new f<T><U>;", "new (f<T>)<U>();\n"),
        ("new f<T><U>();", "new (f<T>)<U>();\n"),
        ("new a.b<T><U>();", "new (a.b<T>)<U>();\n"),
        ("new f<T>\n<U>;", "new (f<T>)<U>();\n"),
        ("new f<T><U>(...x);", "new (f<T>)<U>(...x);\n"),
        ("new f<T><U,>(x);", "new (f<T>)<U>(x);\n"),
        // the argument-less call twin; its template spelling is the fixture
        // `instantiation_paren_type_args_follow_prettier_divergence`'s bare variant
        ("f<T><U>;", "(f<T>)<U>;\n"),
        // the second list reads as an assertion to tsc, which takes one type, no trailing comma
        ("new f<T><U, V>(x);", "new (f<T>)<U, V>(x);\n"),
        ("f<T><U, V>(x);", "(f<T>)<U, V>(x);\n"),
        ("f<T><U,>(x);", "(f<T>)<U>(x);\n"),
        ("f<T><U, V>`x`;", "(f<T>)<U, V>`x`;\n"),
        ("new f<T><U, V>`x`;", "new (f<T>)<U, V>`x`();\n"),
        ("f<T><U>();", "(f<T>)<U>();\n"),
        ("f<T><U>(...x);", "(f<T>)<U>(...x);\n"),
    ] {
        let out = format(source);
        assert_eq!(out, printed, "printed from {source:?}");
        assert_eq!(
            expression_shape(&out),
            expression_shape(source),
            "the printed form reparses to the same tree: {source:?}"
        );
    }
}

/// Whether a head's chain of lists continues is its owner's answer, read from the tree:
/// a comparison `<` after the chain is no list, and a pair the printer adds itself ends
/// the chain. Every repair is a fixed point that reparses to the input's tree.
#[test]
fn second_list_repair_is_decided_from_the_tree() {
    for (source, printed) in [
        ("f<T><U> < 1;", "((f<T>)<U>) < 1;\n"),
        ("f<T><U> <= 1;", "(f<T>)<U> <= 1;\n"),
        ("f<T><U><V>;", "((f<T>)<U>)<V>;\n"),
        ("f<T><U>\n<V>;", "((f<T>)<U>)<V>;\n"),
        ("new f<T><U><V>();", "new ((f<T>)<U>)<V>();\n"),
    ] {
        let out = format(source);
        assert_eq!(out, printed, "printed from {source:?}");
        assert_eq!(format(&out), out, "the repair is a fixed point: {source:?}");
        assert_eq!(
            expression_shape(&out),
            expression_shape(source),
            "the printed form reparses to the same tree: {source:?}"
        );
    }
}

/// A head that ends an open optional chain takes no repair pair: the pair would cut the
/// chain in two, moving the short-circuit (`(a?.b<T>)<U>()` calls whatever the chain
/// produced). The bare spelling stays — acorn's reading intact, tsc rejecting it as it
/// did the input. A chain the author sealed takes the pair as any head does.
#[test]
fn second_list_after_an_open_optional_chain_prints_bare() {
    for source in [
        "a?.b<T><U>();\n",
        "a?.b<T><U>(...x);\n",
        "a?.b<T><U, V>(y);\n",
        "a?.b<T><U>;\n",
        "a?.b.c<T><U>();\n",
    ] {
        assert_eq!(format(source), source);
    }
}

/// A pair the author wrote around an expression tsc reads as a comparison survives in
/// every position that would strip it: bare, an operator ahead of it binds to its first
/// operand and a postfix after it joins the assertion's operand — and an argument-less
/// `new` printed without its list would take the postfix as its own.
#[test]
fn authored_pair_around_a_comparison_read_survives() {
    for source in [
        "(new f<T><U>`x`)(y);\n",
        "(new f<T><U>`x`).y;\n",
        "(new f<T><U>`x`)[0];\n",
        "(new f<T><U>`x`)`y`;\n",
        "(new f<T><U>`x`)!;\n",
        "(new f<T><U>`x`)?.(y);\n",
        "(new new f<T><U>(x))(y);\n",
        "(f<T><U>(x)) * 2;\n",
        "a * (f<T><U>(x));\n",
        "-(f<T><U>(x));\n",
        "typeof (new f<T><U>(x));\n",
        "(new f<T><U>(x)).y;\n",
        "(f<T><U>(x)) as W;\n",
        "(f<T><U>(x))<V>;\n",
        "(f<T><U>(x)).y.z();\n",
    ] {
        assert_eq!(format(source), source);
        assert_eq!(
            expression_shape(source),
            expression_shape(&format(source)),
            "the tree survives: {source:?}"
        );
    }
    // a `new` callee: the pair stays, and the `new` takes its argument list
    for (source, printed) in [
        ("new (f<T><U>`x`);", "new (f<T><U>`x`)();\n"),
        ("new (new f<T><U>(x));", "new (new f<T><U>(x))();\n"),
    ] {
        assert_eq!(format(source), printed);
    }
}

/// A first list tsc's comparison MAY read as an operand prints as written, the pair only
/// where tsc definitely refuses it: a spread is an array-literal element and a
/// parenthesized arrow is a `<` operand, so neither is a refusal — and the paren around
/// that arrow, which the type printer would strip anywhere else, is kept there, so the
/// print is read the way the source was.
#[test]
fn first_list_that_may_read_as_an_operand_prints_as_written() {
    for (source, printed) in [
        ("f<[...A]><U>(x);", "f<[...A]><U>(x);\n"),
        ("f<[A, ...B]><U>(x);", "f<[A, ...B]><U>(x);\n"),
        ("f<A | (() => T)><U>(x);", "f<A | (() => T)><U>(x);\n"),
        ("f<(() => T)><U>(x);", "f<(() => T)><U>(x);\n"),
        ("f<(() => A)><U>`t`;", "f<(() => A)><U>`t`;\n"),
        ("f<(() => A)><U><V>(x);", "f<(() => A)><U><V>(x);\n"),
        ("f<(() => A)><U>(x).y;", "f<(() => A)><U>(x).y;\n"),
        ("new f<(() => A)><U>(x);", "new f<(() => A)><U>(x);\n"),
        ("f<A<(() => T)>><U>(x);", "f<A<(() => T)>><U>(x);\n"),
        ("f<[...(() => T)]><U>(x);", "f<[...(() => T)]><U>(x);\n"),
        // a position that needs a pair around a function type takes the kept one
        (
            "f<{ a: (() => T) extends A ? B : C }><U>(x);",
            "f<{ a: (() => T) extends A ? B : C }><U>(x);\n",
        ),
        (
            "f<{ a: A extends (() => infer V extends B) ? V : C }><U>(x);",
            "f<{ a: A extends (() => infer V extends B) ? V : C }><U>(x);\n",
        ),
        ("f<[(() => T)]><U>(x);", "f<[(() => T)]><U>(x);\n"),
        ("f<((() => T))><U>(x);", "f<(() => T)><U>(x);\n"),
        ("f<keyof (() => T)><U>(x);", "f<keyof (() => T)><U>(x);\n"),
        // outside a bare chain's first list the paren is stripped as redundant
        ("f<(() => T)>(x);", "f<() => T>(x);\n"),
        ("f<(() => T)><U>();", "(f<() => T>)<U>();\n"),
        // a definite refusal repairs
        ("f<unique symbol><U>(x);", "(f<unique symbol>)<U>(x);\n"),
        ("f<T[]><U>(x);", "(f<T[]>)<U>(x);\n"),
        ("f<[A?]><U>(x);", "(f<[A?]>)<U>(x);\n"),
        ("f<[a: A]><U>(x);", "(f<[a: A]>)<U>(x);\n"),
        ("f<[...A[]]><U>(x);", "(f<[...A[]]>)<U>(x);\n"),
    ] {
        assert_prints_fixed(source, printed);
    }
    // the same in a Svelte template expression
    let template = component("{f<(() => A)><U>(x)}");
    let out = svelte_format(&template);
    assert_eq!(out, template);
    assert_eq!(svelte_format(&out), out);
}

/// The paren a bare chain's first list keeps around a function type is kept when it holds a
/// comment too, with the comment inside it where the author wrote it: stripped, the arrow
/// is no `<` operand, so the next pass would read a refusal and take the pair.
#[test]
fn first_list_commented_function_paren_is_kept() {
    for (source, printed) in [
        (
            "f<(/* c */ () => T)><U>(x);",
            "f<(/* c */ () => T)><U>(x);\n",
        ),
        (
            "f<(() => T /* c */)><U>(x);",
            "f<(() => T /* c */)><U>(x);\n",
        ),
        (
            "f<(/* a */ () => T /* b */)><U>(x);",
            "f<(/* a */ () => T /* b */)><U>(x);\n",
        ),
        (
            "f<((/* c */ () => T))><U>(x);",
            "f<(/* c */ () => T)><U>(x);\n",
        ),
        (
            "f<A<(/* c */ () => T)>><U>(x);",
            "f<A<(/* c */ () => T)>><U>(x);\n",
        ),
        (
            "f<[(/* c */ () => T)]><U>(x);",
            "f<[(/* c */ () => T)]><U>(x);\n",
        ),
        (
            "f<[...(/* c */ () => T)]><U>(x);",
            "f<[...(/* c */ () => T)]><U>(x);\n",
        ),
        (
            "f<A | (/* c */ () => T)><U>(x);",
            "f<A | (/* c */ () => T)><U>(x);\n",
        ),
        (
            "new f<(/* c */ () => T)><U>(x);",
            "new f<(/* c */ () => T)><U>(x);\n",
        ),
        (
            "f<(/* c */ () => T)><U>`t`;",
            "f<(/* c */ () => T)><U>`t`;\n",
        ),
        (
            "f<(/* c */ () => T)><U><V>(x);",
            "f<(/* c */ () => T)><U><V>(x);\n",
        ),
        (
            "f<(/* c */ () => T)><U>(x).y;",
            "f<(/* c */ () => T)><U>(x).y;\n",
        ),
        (
            "f<( // c\n() => T)><U>(x);",
            "f<\n\t( // c\n\t\t() => T\n\t)\n><U>(x);\n",
        ),
        (
            "f<(() => T // c\n)><U>(x);",
            "f<\n\t(\n\t\t() => T // c\n\t)\n><U>(x);\n",
        ),
        (
            "f<(\n// c\n() => T)><U>(x);",
            "f<\n\t(\n\t\t// c\n\t\t() => T\n\t)\n><U>(x);\n",
        ),
        (
            "f<A<( // c\n() => T)>><U>(x);",
            "f<\n\tA<\n\t\t( // c\n\t\t\t() => T\n\t\t)\n\t>\n><U>(x);\n",
        ),
        (
            "f<[(() => T // c\n)]><U>(x);",
            "f<\n\t[\n\t\t(\n\t\t\t() => T // c\n\t\t)\n\t]\n><U>(x);\n",
        ),
        (
            "f<A | ( // c\n() => T)><U>(x);",
            "f<\n\t| A\n\t| ( // c\n\t\t\t() => T\n\t  )\n><U>(x);\n",
        ),
        (
            "f<[...( // c\n() => T)]><U>(x);",
            "f<\n\t[\n\t\t...( // c\n\t\t\t() => T\n\t\t)\n\t]\n><U>(x);\n",
        ),
        (
            "new f<(// c\n() => T)><U>(x);",
            "new f<\n\t( // c\n\t\t() => T\n\t)\n><U>(x);\n",
        ),
        // a comment in a redundant shell around the kept one is the list's to emit
        (
            "f<(// c\n(() => T))><U>(x);",
            "f< // c\n\t(() => T)\n><U>(x);\n",
        ),
        // outside a bare chain's first list the commented paren strips as anywhere
        ("f<(/* c */ () => T)>(x);", "f</* c */ () => T>(x);\n"),
        ("f<(/* c */ () => T)><U>();", "(f</* c */ () => T>)<U>();\n"),
    ] {
        assert_prints_fixed(source, printed);
    }
}

/// tsc reads `void[]` as the unary `void []`, an operand of its comparison, so a first list
/// holding it prints as written.
#[test]
fn first_list_void_array_prints_as_written() {
    for source in [
        "f<void[]><U>(x);\n",
        "f<A | void[]><U>(x);\n",
        "new f<void[]><U>(x);\n",
    ] {
        assert_prints_fixed(source, source);
    }
    // a `void` with nothing to apply to still refuses
    assert_prints_fixed("f<void><U>(x);", "(f<void>)<U>(x);\n");
}

/// An authored pair is found across every JavaScript whitespace code point, not only the
/// ASCII ones: a no-break space, a byte-order mark, a line or paragraph separator, an
/// ideographic space inside the pair is still inside the pair.
#[test]
fn authored_pair_is_found_across_unicode_whitespace() {
    for (source, printed) in [
        ("(\u{a0}new f<T><U>`x`)(y);", "(new f<T><U>`x`)(y);\n"),
        ("(\u{a0}f<T><U>(x)) + 1;", "(f<T><U>(x)) + 1;\n"),
        ("-(\u{feff}f<T><U>(x));", "-(f<T><U>(x));\n"),
        ("(f<T><U>(x)\u{3000}) * 2;", "(f<T><U>(x)) * 2;\n"),
        ("(f<T><U>(x)\u{2028}).y;", "(f<T><U>(x)).y;\n"),
        ("a * (\u{2029}f<T><U>(x));", "a * (f<T><U>(x));\n"),
    ] {
        assert_eq!(format(source), printed, "printed from {source:?}");
    }
}

/// In a class heritage acorn-typescript ends the superclass before a second list its
/// follower refuses (`{`), and reads that list as the heritage's own; tsc rejects the
/// bare spelling, so tsv keeps acorn's tree and prints the pair both read alike.
#[test]
fn heritage_second_list_reads_as_acorn_does() {
    let source = "class D extends new f<T><() => U> {}";
    let json = parse_json(source);
    assert_eq!(
        json.pointer("/body/0/superClass").map(shape).as_deref(),
        Some("new[f]<T>()"),
        "the superclass ends at the `new`: {json}"
    );
    assert!(
        json.pointer("/body/0/superTypeArguments").is_some()
            || json.pointer("/body/0/superTypeParameters").is_some(),
        "the second list is the heritage's own: {json}"
    );
    assert_eq!(format(source), "class D extends (new f<T>())<() => U> {}\n");
}

/// A type operator is an expression to tsc when its operand opens with `[` or `(`
/// (`keyof [A]` is the element access `keyof[A]`, `keyof (A)` the call `keyof(A)`), so tsc
/// reads the comparison and the spelling prints as written; only an operand that is
/// itself no expression (`[A][]`, `(A)[]`) or opens otherwise (`A[]`, `symbol`) refuses.
#[test]
fn type_operator_over_a_bracket_or_paren_prints_as_written() {
    for source in [
        "f<readonly [A]><U>(x);\n",
        "new f<readonly [A]><U>(x);\n",
        "f<readonly [A, B]><U>(x);\n",
        "f<keyof [A]><U>(x);\n",
        "f<keyof [A]><U>`x`;\n",
        "f<keyof [A][0]><U>(x);\n",
        "f<keyof (() => T)><U>(x);\n",
        "f<[keyof [B]]><U>(x);\n",
        "f<A<keyof [B]>><U>(x);\n",
        "f<A | keyof [B]><U>(x);\n",
    ] {
        assert_prints_fixed(source, source);
    }
    for (source, printed) in [
        ("f<keyof A[]><U>(x);", "(f<keyof A[]>)<U>(x);\n"),
        ("f<readonly A[]><U>(x);", "(f<readonly A[]>)<U>(x);\n"),
        (
            "f<readonly string[]><U>(x);",
            "(f<readonly string[]>)<U>(x);\n",
        ),
        ("f<keyof (A)[]><U>(x);", "(f<keyof A[]>)<U>(x);\n"),
        ("f<readonly [A][]><U>(x);", "(f<readonly [A][]>)<U>(x);\n"),
        ("f<unique symbol><U>(x);", "(f<unique symbol>)<U>(x);\n"),
    ] {
        assert_prints_fixed(source, printed);
    }
}

/// In the first list of a chain printed bare, a paren the author wrote directly under a
/// type operator is kept, a comment in it included (`keyof (A)` is a call to tsc and must
/// stay one), and whether an operand opens with `[` or `(` is read off the operand AS
/// PRINTED — a pair the type printer adds counts (`readonly (readonly [A])`), one it strips
/// does not (`keyof (A)[0]` prints `keyof A[0]`, which tsc refuses, so the chain takes the
/// pair). Every output is a fixed point.
#[test]
fn first_list_operator_operand_is_read_as_printed() {
    for (source, printed) in [
        ("f<keyof (A)><U>(x);", "f<keyof (A)><U>(x);\n"),
        (
            "f<keyof /* c */ (A)><U>(x);",
            "f<keyof /* c */ (A)><U>(x);\n",
        ),
        ("f<keyof\n(A)><U>(x);", "f<keyof (A)><U>(x);\n"),
        ("f<keyof\u{3000}(A)><U>(x);", "f<keyof (A)><U>(x);\n"),
        ("new f<keyof (A)><U>(x);", "new f<keyof (A)><U>(x);\n"),
        ("f<A<keyof (B)>><U>(x);", "f<A<keyof (B)>><U>(x);\n"),
        // the redundant paren down an indexed access's left spine is stripped, and the
        // stripped operand opens with `A`, so the chain takes the pair
        ("f<keyof (A)[0]><U>(x);", "(f<keyof A[0]>)<U>(x);\n"),
        // a comment in the kept paren stays inside it, and one ahead of it hangs the pair
        (
            "f<keyof (// c\nA)><U>(x);",
            "f<\n\tkeyof ( // c\n\t\tA\n\t)\n><U>(x);\n",
        ),
        (
            "f<keyof (\n// c\nA)><U>(x);",
            "f<\n\tkeyof (\n\t\t// c\n\t\tA\n\t)\n><U>(x);\n",
        ),
        (
            "f<keyof ((// c\nA))><U>(x);",
            "f<\n\tkeyof ( // c\n\t\tA\n\t)\n><U>(x);\n",
        ),
        (
            "f<keyof // x\n(A)><U>(x);",
            "f<\n\tkeyof // x\n\t\t(A)\n><U>(x);\n",
        ),
        // a chain that takes the pair strips the redundant paren as anywhere
        ("f<keyof (A)><U>();", "(f<keyof A>)<U>();\n"),
    ] {
        let out = format(source);
        assert_eq!(out, printed, "printed from {source:?}");
        assert_eq!(format(&out), out, "a fixed point: {source:?}");
    }
    // The pair the prefix-operator rule adds counts: the operand prints opening with `(`,
    // so the spelling prints as written. (tsv's own expression lookahead refuses a
    // `readonly (` type argument that tsc and acorn read, so a second pass reads this
    // output as a comparison — a separate over-rejection, not this rule's.)
    assert_eq!(
        format("f<readonly readonly [A]><U>(x);"),
        "f<readonly (readonly [A])><U>(x);\n"
    );
}
