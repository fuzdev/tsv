// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! The pairs a comma-list region does not keep a second time: a node that prints a pair of its
//! own (a sequence, a JSDoc cast) holds its one pair, never two, and an assignment whose pair the
//! region leaves to the source keeps the pair the author wrote at every carrier, a `for`
//! header's clause included (`supplies_its_own_pair` in `crates/tsv_ts/src/printer/
//! relational_sibling_pairs.rs`; `docs/conformance_prettier_ts.md` §Relational chain
//! type-argument parens).
//!
//! Every case is a fixed point whose reparse keeps the program the source wrote.

use serde_json::Value;

/// The span-free wire of `source`, so a reparse must keep the same program.
fn tree(source: &str) -> Value {
    fn strip(node: &mut Value) {
        match node {
            Value::Object(map) => {
                for key in ["start", "end", "loc", "name_loc"] {
                    map.remove(key);
                }
                map.values_mut().for_each(strip);
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("the case parses");
    let mut value = tsv_debug::json::wire_value(&tsv_ts::convert_ast_json_bytes(&program, source));
    strip(&mut value);
    value
}

/// `source` formatted, asserted a fixed point whose reparse is the program `source` wrote.
#[track_caller]
fn stable(source: &str) -> String {
    let output = tsv_ts::format_str(source).expect("the case formats");
    assert_eq!(
        tsv_ts::format_str(&output).expect("the output formats"),
        output,
        "not a fixed point: {source}"
    );
    assert_eq!(tree(source), tree(&output), "the program changed: {source}");
    output.trim_end().to_owned()
}

#[test]
fn a_sequence_element_prints_its_one_pair() {
    for (source, expected) in [
        // The sequence's own pair is the pair the region would keep: printing both doubles it.
        ("fn((a < b, c), d > (e, f));", "fn((a < b, c), d > (e, f));"),
        (
            "fn(((a < b, c)), d > (e, f));",
            "fn((a < b, c), d > (e, f));",
        ),
        (
            "fn(a < b, ((c, d < e)), f > (g, h));",
            "fn(a < b, (c, d < e), f > (g, h));",
        ),
        (
            "const a1 = [(a < b, c), d > (e, f)];",
            "const a1 = [(a < b, c), d > (e, f)];",
        ),
        (
            "new Cls((a < b, c), d > (e, f));",
            "new Cls((a < b, c), d > (e, f));",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn a_jsdoc_cast_element_prints_its_one_pair() {
    for (source, expected) in [
        (
            "fn(/** @type {T} */ (a < b), c > (d, e));",
            "fn(/** @type {T} */ (a < b), c > (d, e));",
        ),
        (
            "fn((/** @type {T} */ (a < b)), c > (d, e));",
            "fn(/** @type {T} */ (a < b), c > (d, e));",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn an_assignment_keeps_its_authored_pair_at_every_carrier() {
    for (source, expected) in [
        (
            "fn((x = a < b), e > (f, g));",
            "fn((x = a < b), e > (f, g));",
        ),
        (
            "const a1 = [(x = a < b), e > (f, g)];",
            "const a1 = [(x = a < b), e > (f, g)];",
        ),
        // A `for` header prints its clause's operands itself: stripped there, the pair the
        // author wrote leaves `a < b, e > (f, g)`, which every parser reads as one generic call.
        (
            "for ((x = a < b), e > (f, g);;);",
            "for ((x = a < b), e > (f, g); ;);",
        ),
        (
            "for (let i = 0; i < n; (x = a < b), e > (f, g));",
            "for (let i = 0; i < n; (x = a < b), e > (f, g));",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}
