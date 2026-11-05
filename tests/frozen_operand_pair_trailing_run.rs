// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A pair the printer keeps around a value opens its trailing run at the value's printed end,
//! so a comment written last in a stripped shell down the value's right spine prints inside the
//! pair ahead of the run written after that shell (`docs/comments.md` §The right-spine shell
//! run). A FROZEN operand is the exception: a format-ignore directive ahead of it prints its
//! slice verbatim, the stripped shell and its comment included, so the pair must not claim that
//! comment as well — claimed, it printed twice, and the copy grew by a line every pass.
//!
//! Not a fixture: prettier moves the directive past the `=` and welds the two comments onto one
//! line at every one of these, a prettier behaviour of its own, so the transitions are pinned
//! here directly.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::__private::parse(source, &arena).expect("parse failed");
    tsv_ts::__private::format_ast(&program, source)
}

/// Assert `source` formats to `expected` in ONE pass, `expected` is a fixed point, and the
/// stripped shell's comment prints exactly once.
fn assert_frozen_once(source: &str, expected: &str) {
    let pass1 = format(source);
    assert_eq!(pass1, expected, "one-pass convergence");
    assert_eq!(
        format(&pass1),
        pass1,
        "the target form must be a fixed point"
    );
    assert_eq!(
        pass1.matches("// c\n").count(),
        1,
        "the frozen shell's comment prints once"
    );
}

/// An `as` operand whose directive freezes the operand: its pair keeps the slice's run.
#[test]
fn as_operand_frozen() {
    assert_frozen_once(
        "k = (\n// prettier-ignore\na + (b // c\n) // d\n) as T;\n",
        "k = (\n\t// prettier-ignore\n\ta + (b // c\n) // d\n) as T;\n",
    );
}

/// A `satisfies` operand, with a block ahead of the comment written after the pair.
#[test]
fn satisfies_operand_frozen_block_then_line() {
    assert_frozen_once(
        "k = (\n// prettier-ignore\na + (b // c\n) /* x */ // d\n) satisfies T;\n",
        "k = (\n\t// prettier-ignore\n\ta + (b // c\n) /* x */ // d\n) satisfies T;\n",
    );
}

/// The required pair ahead of `!`.
#[test]
fn non_null_operand_frozen_block_then_line() {
    assert_frozen_once(
        "k = (\n// prettier-ignore\na + (b // c\n) /* x */ // d\n)!;\n",
        "k = (\n\t// prettier-ignore\n\ta + (b // c\n) /* x */ // d\n)!;\n",
    );
}

/// The `export default` line-comment shell, the directive inside the pair.
#[test]
fn export_default_frozen_inside_pair() {
    assert_frozen_once(
        "export default (\n// prettier-ignore\na + (b // c\n) /* x */ // d\n);\n",
        "export default\n\t// prettier-ignore\n\t(\n\t\ta + (b // c\n) /* x */ // d\n\t);\n",
    );
}

/// The `export default` line-comment shell, the directive ahead of the pair.
#[test]
fn export_default_frozen_before_pair() {
    assert_frozen_once(
        "export default\n// prettier-ignore\n(a + (b // c\n) /* x */ // d\n);\n",
        "export default\n\t// prettier-ignore\n\t(\n\t\ta + (b // c\n) /* x */ // d\n\t);\n",
    );
}

/// Control: a declarator's value shell routes a frozen value to the frozen kept pair.
#[test]
fn declarator_value_frozen() {
    assert_frozen_once(
        "let k = (\n// prettier-ignore\na + (b // c\n) // d\n);\n",
        "let k =\n\t// prettier-ignore\n\t(\n\t\ta + (b // c\n) // d\n\t);\n",
    );
}

/// Control: a JSDoc cast's frozen inner keeps its own span-end run.
#[test]
fn jsdoc_cast_inner_frozen() {
    assert_frozen_once(
        "k = /** @type {T} */ (\n// prettier-ignore\na + (b // c\n) // d\n);\n",
        "k = /** @type {T} */ (\n\t// prettier-ignore\n\ta + (b // c\n) // d\n);\n",
    );
}

/// Control: a frozen `case` test opens its head→`:` gap at the test's end.
#[test]
fn case_test_frozen() {
    assert_frozen_once(
        "switch (x) {\n\tcase (\n// prettier-ignore\na + (b // c\n) // d\n):\n\t\tf();\n}\n",
        "switch (x) {\n\tcase\n\t\t// prettier-ignore\n\t\ta + (b // c\n) // d\n\t\t:\n\t\tf();\n}\n",
    );
}
