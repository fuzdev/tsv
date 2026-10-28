// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! An own-line comment in the stripped grouping shell of an instantiation's operand, at a
//! restricted production: the run leads the argument from inside the hanging pair, as it does
//! without the type arguments (`return (/* c */⏎a)()`), so no line terminator follows the
//! keyword.
//!
//! The fixture `typescript/typescript_specific/generics/instantiation_operand_left_side_shell_comment_prettier_divergence`
//! pins the `return` and `yield` authorings. Its `throw` cell rides the variant in the
//! already-normalized form, because prettier's output from the authored one does not parse, so
//! the one-pass hang from the authored `throw` spelling is asserted here.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

#[test]
fn throw_takes_the_hanging_pair_in_one_pass() {
    for (source, printed) in [
        (
            "function f() {\n\tthrow (/* c */\n\ta)<T>;\n}\n",
            "function f() {\n\tthrow (\n\t\t/* c */\n\t\ta<T>\n\t);\n}\n",
        ),
        (
            "function f() {\n\tthrow (// c\n\ta)<T>;\n}\n",
            "function f() {\n\tthrow (\n\t\t// c\n\t\ta<T>\n\t);\n}\n",
        ),
        (
            "function f() {\n\tthrow (/* c */\n\ta)()<T>;\n}\n",
            "function f() {\n\tthrow (\n\t\t/* c */\n\t\ta()<T>\n\t);\n}\n",
        ),
    ] {
        let out = format(source);
        assert_eq!(out, printed, "printed from {source:?}");
        assert_eq!(format(&out), out, "a fixed point: {source:?}");
    }
}
