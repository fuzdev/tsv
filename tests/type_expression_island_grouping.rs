// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! An expression standing in a TYPE is read in a grouping of its own — a type literal's
//! computed key in its brackets (`ComputedPropertyName : [ AssignmentExpression[+In] ]`,
//! ecma262) — whatever the type itself stands in.
//!
//! The gates read against the grouping depth are: whether `in` is an operator (off at
//! a `for` header's own depth), whether an arrow function may keep its return type (off
//! directly in a conditional's consequent) and whether `as` is an assertion (off at the
//! top of an `{#each}` head). A type opens no delimiter of its own, so each has to be
//! lifted where the expression inside it begins, as an object literal's braces lift it
//! for the literal's own keys.
//!
//! Not fixturable as one unit: the `for` header and the consequent are TypeScript, the
//! `{#each}` head is Svelte, and the controls are rejections with a named error.

use serde_json::Value;

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

/// How many nodes of `kind` `source`'s wire holds.
fn count(source: &str, kind: &str) -> usize {
    fn walk(node: &Value, kind: &str) -> usize {
        match node {
            Value::Object(map) => {
                usize::from(map.get("type").and_then(Value::as_str) == Some(kind))
                    + map.values().map(|child| walk(child, kind)).sum::<usize>()
            }
            Value::Array(items) => items.iter().map(|item| walk(item, kind)).sum(),
            _ => 0,
        }
    }
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("the case parses");
    let json = tsv_debug::json::wire_value(&tsv_ts::convert_ast_json_bytes(&program, source));
    walk(&json, kind)
}

fn error_message(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let rendered = tsv_ts::parse(source, &arena)
        .expect_err("the case must not parse")
        .to_string();
    rendered.lines().next().expect("a message line").to_owned()
}

#[test]
fn in_is_an_operator_in_a_type_in_a_for_header() {
    for (source, printed) in [
        // A type-argument list with a separator, which is tried: its tail must not fail
        // on the `in`, or the `<` is read as a comparison.
        (
            "for (x = h<A, { [c || d in e]: 1 }>(y); ; ) {}",
            "for (x = h<A, { [c || (d in e)]: 1 }>(y); ;) {}",
        ),
        // The same list with no separator, which is read committed.
        (
            "for (x = h<{ [c || d in e]: 1 }>(y); ; ) {}",
            "for (x = h<{ [c || (d in e)]: 1 }>(y); ;) {}",
        ),
    ] {
        assert_eq!(stable(source), printed, "printed from {source:?}");
        for text in [source, printed] {
            assert_eq!(count(text, "TSTypeParameterInstantiation"), 1, "{text:?}");
            assert_eq!(count(text, "CallExpression"), 1, "{text:?}");
        }
    }
    // An import type's options are an object literal, grouped by its own braces. Options
    // that are none make no import type to tsc or to acorn-typescript, which read two
    // comparisons over a dynamic import — as this does where the tail fails on the `in`.
    assert_eq!(
        stable("for (x = h<A, import('m', c || d in e)>(y); ; ) {}"),
        "for (x = h < A, import('m', c || (d in e)) > y; ;) {}"
    );
    // A type that is no type argument: an assertion's.
    assert_eq!(
        stable("for (x = y as { [c || d in e]: 1 }; ; ) {}"),
        "for (x = y as { [c || (d in e)]: 1 }; ;) {}"
    );
    // A tried list inside a tried list's computed key.
    assert_eq!(
        stable("for (x = a < b, { [h<A, { [c || d in e]: 1 }>(y)]: 1 }, c + d > (e); ; ) {}"),
        "for (x = a < b, { [h<A, { [c || (d in e)]: 1 }>(y)]: 1 }, c + d > e; ;) {}"
    );
}

#[test]
fn a_less_than_in_a_computed_key_opens_no_region_past_its_bracket() {
    // The key's `]` is a token the printed form always holds, so a `<` inside the key
    // opens no list a later `>` could close: that `>` takes its clarity pair, and breaks
    // behind itself, like any other.
    for (source, printed) in [
        (
            "let v: { [a < b]: 1 } = c > await d;",
            "let v: { [a < b]: 1 } = c > (await d);",
        ),
        (
            "x = [y as { [a < b]: 1 }, c > d % e + f];",
            "x = [y as { [a < b]: 1 }, c > (d % e) + f];",
        ),
    ] {
        assert_eq!(stable(source), printed, "printed from {source:?}");
    }
}

#[test]
fn a_for_headers_own_in_is_still_its_separator() {
    // The grouping closes with the key, so the header's own `in` reads as before.
    for source in ["for (a in b) {\n}", "for (const k in o as { [p]: 1 }) {\n}"] {
        assert_eq!(stable(source), source);
        assert_eq!(count(source, "ForInStatement"), 1, "{source:?}");
    }
    // And an `in` at the header's own depth is still no operator there.
    assert_eq!(
        error_message("for (x = a in b; ; ) {}"),
        "Invalid assignment target"
    );
    assert_eq!(
        error_message("for (var x = h<{ [k]: 1 }>(y) in o) {}"),
        "for-in loop variable declaration may not have an initializer"
    );
}

#[test]
fn an_arrow_keeps_its_return_type_in_a_type_in_a_consequent() {
    // Directly in a consequent a parenthesized arrow's `:` may be the conditional's
    // own; inside a computed key's brackets it cannot be.
    for source in [
        "x = c ? h<A, { [(p): T => q]: 1 }>(y) : w;",
        "x = c ? h<{ [(p): T => q]: 1 }>(y) : w;",
    ] {
        assert_eq!(stable(source), source);
        assert_eq!(
            count(source, "TSTypeParameterInstantiation"),
            1,
            "{source:?}"
        );
        assert_eq!(count(source, "ArrowFunctionExpression"), 1, "{source:?}");
    }
}

#[test]
fn as_is_an_assertion_in_a_type_in_an_each_head() {
    // The head's own `as` is the block's separator at the head's top level, and an
    // assertion inside a computed key's brackets.
    let source = "<script lang=\"ts\">\n\tlet a;\n</script>\n\n{#each h<A, { [c as d]: 1 }>(y) as item}<p>{item}</p>{/each}\n";
    let output = tsv_svelte::format_str(source).expect("the head parses");
    assert_eq!(output, source);
}
