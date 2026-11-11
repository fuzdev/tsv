// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! An instantiation expression called or tagged keeps its pair (`(f<T>)()`, `` (f<T>)`x` ``):
//! bare, the list becomes the call's or the tag's own type arguments, a different tree to tsc
//! and acorn-typescript alike.
//!
//! The fixture `typescript_specific/generics/instantiation_callee_tag_paren_prettier_divergence`
//! pins the spellings prettier lands on as a stripped form. The ones asserted here have no such
//! form a fixture can carry:
//!
//! - over an OPEN optional chain, the pair is where the chain ends. Called
//!   (`(a?.b<T>)(c).d`), prettier strips it to `a?.b<T>(c).d`, which short-circuits the call
//!   and the member with it — a different program with the types stripped. tsv's wire for the
//!   instantiation over the open chain nests the chain inside the instantiation where
//!   acorn-typescript nests it outside, so the input's `expected.json` cannot be acorn's; the
//!   output and its fixed point are pinned here instead, through tsv's own tree.
//! - tagged (`` (a?.b<T>)`x` ``, `` (a?.b()?.k<T>)`t` ``), prettier's stripped output is an
//!   optional chain as a tag, which does not parse, so no `output_prettier.*` exists.

use serde_json::Value;

fn format(source: &str) -> String {
    let program = tsv_ts::__private::parse(source).expect("parse failed");
    tsv_ts::__private::format_ast(&program, source)
}

fn expression_json(source: &str) -> Value {
    let program = tsv_ts::__private::parse(source).expect("parse failed");
    let json =
        tsv_debug::json::wire_value(&tsv_ts::__private::convert_ast_json_bytes(&program, source));
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
            for key in ["start", "end"] {
                map.remove(key);
            }
            map.values_mut().for_each(strip_positions);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_positions),
        _ => {}
    }
}

/// Assert `source` is its own format, and that the printed form parses to the same tree.
fn assert_fixed_point(source: &str) {
    let printed = format(source);
    assert_eq!(printed, source, "the pair must stay: {source:?}");
    assert_eq!(format(&printed), printed, "F1: {source:?}");
    assert_eq!(
        expression_json(&printed),
        expression_json(source),
        "the printed form must parse to the input's tree: {source:?}"
    );
}

#[test]
fn an_open_chain_called_through_an_instantiation_keeps_its_pair() {
    assert_fixed_point("(a?.b<T>)(c).d;\n");
    assert_fixed_point("x = (a?.b<T>)(c).d;\n");
    // the call takes no list of its own: the instantiation is its callee
    let call = expression_json("(a?.b<T>)(c);\n");
    assert_eq!(call["type"], "CallExpression");
    assert!(
        call.get("typeArguments").is_none_or(Value::is_null),
        "the list stays on the instantiation: {call}"
    );
}

#[test]
fn an_open_chain_tagged_through_an_instantiation_keeps_its_pair() {
    assert_fixed_point("(a?.b<T>)`x`;\n");
    assert_fixed_point("(a?.b()?.k<T>)`t`;\n");
}

#[test]
fn an_operator_over_the_call_keeps_the_pair() {
    assert_fixed_point("await (f<T>)();\n");
    assert_fixed_point("(f<T>)()!;\n");
}

#[test]
fn an_optional_call_strips_the_pair() {
    // an optional call reads the instantiation as its callee bare, so both spellings are one
    // tree and the pair is redundant
    for source in ["(f<T>)?.();\n", "(a?.b<T>)?.(c);\n"] {
        let printed = format(source);
        assert_eq!(format(&printed), printed, "F1: {source:?}");
    }
    assert_eq!(format("(f<T>)?.();\n"), "f<T>?.();\n");
    assert_eq!(
        expression_json("(f<T>)?.();\n"),
        expression_json("f<T>?.();\n"),
        "one tree either way"
    );
}

#[test]
fn a_leftmost_class_reads_the_pair_the_printer_keeps() {
    // `export default` parenthesizes a leftmost `class` the callee would otherwise open on:
    // a plain call keeps the instantiation's pair, which then opens the value, while an
    // optional call strips it, so the walk reads through to the class and wraps the value
    for (source, printed) in [
        (
            "export default (class {}<T>)();\n",
            "export default (class {}<T>)();\n",
        ),
        (
            "export default (class {}<T>)?.();\n",
            "export default (class {}<T>?.());\n",
        ),
    ] {
        assert_eq!(format(source), printed, "printed from {source:?}");
        assert_eq!(format(printed), printed, "F1: {source:?}");
    }
}

#[test]
fn a_frozen_callee_takes_the_pair_its_call_owes() {
    // a format-ignored callee prints verbatim inside the pair its position takes: one for a
    // plain call, none for an optional one
    for (source, printed) in [
        (
            "x = (\n// prettier-ignore\nf<T>\n)(1);\n",
            "x =\n\t// prettier-ignore\n\t(f<T>)(1);\n",
        ),
        (
            "x = (\n// prettier-ignore\nf<T>\n)?.(1);\n",
            "x =\n\t// prettier-ignore\n\tf<T>?.(1);\n",
        ),
    ] {
        assert_eq!(format(source), printed, "printed from {source:?}");
        assert_eq!(format(printed), printed, "F1: {source:?}");
    }
}
