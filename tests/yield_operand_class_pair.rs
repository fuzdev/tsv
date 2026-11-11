// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! The pair a decorated class keeps at the leftmost position of a plain `yield` operand
//! (`Printer::yield_operand_class_pair_target`, `docs/conformance_prettier_ts.md` §TypeScript,
//! "`yield` operand decorated class pair") where the operand prints a pair of its own ahead of
//! the class: one a comment before the author's `)` retains, one a binary or a member puts
//! around the class or around a node holding it, and the hanging pair an own-line comment in
//! a member gap takes.
//!
//! That pair carries the class's `(`, so the class takes no second one: pinned here as a PAIR
//! COUNT, not as a layout. The retained shell prints inline (`yield (@dec⏎class {} /* c */)`),
//! not broken open as the class's own pair is, and a fixture would claim that layout correct.

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
    let program = tsv_ts::__private::parse(source).expect("the case parses");
    let mut value =
        tsv_debug::json::wire_value(&tsv_ts::__private::convert_ast_json_bytes(&program, source));
    strip(&mut value);
    value
}

/// How many `(` the output prints between the `yield` keyword and the class's `@`.
fn pairs_before_the_class(output: &str) -> usize {
    let after_yield = &output[output.find("yield").expect("a yield") + "yield".len()..];
    let before_class = &after_yield[..after_yield.find('@').expect("a decorator")];
    before_class.matches('(').count()
}

#[test]
fn a_pair_the_operand_prints_is_the_class_pair() {
    for source in [
        // a comment before the `)` retains the author's shell
        "function* g() {\n\tyield (@dec class {} /* c */);\n}\n",
        "function* g() {\n\tyield ((@dec class {}).x /* c */);\n}\n",
        "function* g() {\n\tyield ((@dec class {}) /* c */);\n}\n",
        "function* g() {\n\tyield ((@dec class {}) % 2 /* c */);\n}\n",
        // a dividend's pair, and a member object's around a binary holding the class
        "function* g() {\n\tyield (@dec class {}) / 2;\n}\n",
        "function* g() {\n\tyield ((@dec class {}) + 1).x;\n}\n",
        // the hanging pair an own-line comment in a member gap takes
        "function* g() {\n\tyield (@dec class {})\n\t// c\n\t.x;\n}\n",
        "function* g() {\n\tyield (@dec class {})\n\t\t// c\n\t\t.x.y();\n}\n",
    ] {
        let output = tsv_ts::format(source).expect("the case formats");
        assert_eq!(
            pairs_before_the_class(&output),
            1,
            "one pair ahead of the class: {source:?} → {output:?}"
        );
        assert_eq!(
            tsv_ts::format(&output).expect("the output formats"),
            output,
            "not a fixed point: {source:?}"
        );
        assert_eq!(
            tree(source),
            tree(&output),
            "the program changed: {source:?}"
        );
    }
}

/// A comment inside the author's shell around the class ALONE, behind the class, is printed by
/// what follows the shell (a retained non-null or assertion shell, an operator's gap, a
/// template's), so the class keeps no pair of its own there: a second pair would close ahead
/// of the comment and the next pass would move it. Pinned as a fixed point with at most one
/// pair ahead of the class and the program kept; a tail that retains no shell prints the
/// class bare, which acorn rejects (the known gap the rule's rustdoc names).
#[test]
fn a_comment_in_the_class_shell_takes_no_second_pair() {
    for source in [
        "function* g() {\n\tyield (@dec class {} // c\n)!.x;\n}\n",
        "function* g() {\n\tyield (@dec class {} // c\n) as T;\n}\n",
        "function* g() {\n\tyield (@dec class {} // c\n) < 1;\n}\n",
        "function* g() {\n\tyield (@dec class {} /* c */)`t`;\n}\n",
        "function* g() {\n\tyield (@dec class {}/* c */)!;\n}\n",
        "function* g() {\n\tyield (@dec class {} // c\n) ? a : b;\n}\n",
        "function* g() {\n\tyield (@dec class {} /* c */).x;\n}\n",
    ] {
        let output = tsv_ts::format(source).expect("the case formats");
        assert!(
            pairs_before_the_class(&output) <= 1,
            "at most one pair ahead of the class: {source:?} → {output:?}"
        );
        assert_eq!(
            tsv_ts::format(&output).expect("the output formats"),
            output,
            "not a fixed point: {source:?}"
        );
        assert_eq!(
            tree(source),
            tree(&output),
            "the program changed: {source:?}"
        );
    }
}
