// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A `new` callee that is an instantiation expression over a call keeps its paren pair, as
//! any callee holding a call on its left spine does: bare, `new f()<T>()` constructs `f`
//! and calls the result with `<T>`, where `new (f()<T>)()` constructs the instantiated
//! result of `f()`.
//!
//! The fixture `typescript/expressions/new/call_callee_instantiation_parens_prettier_divergence`
//! pins these spellings in a template expression, where Svelte's parse matches tsv's tree.
//! In a `<script>` body acorn-typescript hoists the list off the parenthesized callee onto
//! the `new` (`new (f())<T>()`'s tree), which tsv's wire does not reproduce, so the script
//! spellings are asserted here instead, through tsv's own tree.
//!
//! An instantiation that prints its own pair ends the walk: the call inside that pair can
//! no longer take the `new`'s argument list, so the callee takes no second one. The member
//! twins (`new (f()<T>).k()`) are cells of the `new/call_callee_parens_ts` fixture; the `!`
//! twins are asserted here, since prettier strips their pair to `new f()<T>!()`, which no
//! parser accepts.
//!
//! An optional chain the author sealed keeps its pair under an instantiation
//! (`typescript_specific/generics/instantiation_sealed_chain_prettier_divergence`). The
//! spellings that normalize to those fixed points are asserted here: prettier's output from
//! them does not parse (`` new a?.b()<T>`t`() ``), so no fixture variant can carry them. Their
//! trees are not compared through tsv's wire, which hoists a list off an unparenthesized
//! instantiation callee and not off a parenthesized one (the script-body difference above).

use serde_json::Value;

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

fn expression_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    let json = tsv_debug::json::wire_value(&tsv_ts::convert_ast_json_bytes(&program, source));
    let mut expression = json
        .pointer("/body/0/expression")
        .expect("an expression statement")
        .clone();
    strip_positions(&mut expression);
    expression
}

fn strip_positions(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for key in ["start", "end", "loc"] {
                map.remove(key);
            }
            map.values_mut().for_each(strip_positions);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_positions),
        _ => {}
    }
}

#[test]
fn instantiation_over_a_call_keeps_the_pair() {
    for (source, printed) in [
        ("new (f()<T>)();", "new (f()<T>)();\n"),
        ("new (a.b()<T>)();", "new (a.b()<T>)();\n"),
        ("new (f().c<T>)(x);", "new (f().c<T>)(x);\n"),
        ("new (f()`x`<T>)();", "new (f()`x`<T>)();\n"),
        ("new (f()!<T>)();", "new (f()!<T>)();\n"),
        // the `new` takes its argument list, the pair stays
        ("new (f()<T>);", "new (f()<T>)();\n"),
    ] {
        let out = format(source);
        assert_eq!(out, printed, "printed from {source:?}");
        assert_eq!(format(&out), out, "a fixed point: {source:?}");
        assert_eq!(
            expression_json(&out),
            expression_json(source),
            "the printed form reparses to the same tree: {source:?}"
        );
    }
}

#[test]
fn a_pair_the_callee_prints_takes_no_second() {
    for source in ["new (f()<T>)!();\n", "new (f()<T>)!.k();\n"] {
        assert_eq!(format(source), source);
    }
}

#[test]
fn a_sealed_chain_keeps_its_pair_under_an_instantiation() {
    for (source, printed) in [
        ("new ((a?.b())<T>)`t`();", "new (a?.b())<T>`t`();\n"),
        ("new ((a?.b())<T>)();", "new (a?.b())<T>();\n"),
        ("new ((a?.b())!<T>)();", "new (a?.b())!<T>();\n"),
        ("new ((a?.b)<T>)();", "new (a?.b)<T>();\n"),
        ("((a?.b)<T>)?.();", "(a?.b)<T>?.();\n"),
    ] {
        let out = format(source);
        assert_eq!(out, printed, "printed from {source:?}");
        assert_eq!(format(&out), out, "a fixed point: {source:?}");
    }
}
