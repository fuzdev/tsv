// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A comment run in a stripped shell that a member chain prints MID-CHAIN, at an
//! assignment's value behind a prefix operator (`await`, `!`, `-`), converges in **one**
//! pass.
//!
//! The chain prints the shell's run in the member gap after its head, and the assignment
//! layout reads the run by the line it prints on there (`Printer::mid_chain_shell_run`): a
//! run with no `//` prints on the head's line whole, and in one holding a `//`, a comment the
//! source puts on a new line prints on one of its own while the rest trail the head. Behind
//! a prefix operator that reading alone decides whether the value hangs under the `=`, since
//! the chain's own line-comment check does not see through the operator; a reading that
//! disagrees with what prints hangs the value on one pass and un-hangs it on the next.
//!
//! Not a fixture: a cell whose run puts a `//` on a line of its own hangs the prefixed value
//! under the `=`, as its paren-free spelling does, where prettier moves the `//` past `.y` and
//! keeps the value on the operator's line, a relocation unrelated to the shell, so a fixture
//! would sanction a divergence that is not this one. The other cells sit beside them as the
//! run's remaining readings. The fixtures
//! `assignment/rhs_mid_chain_shell_comment_prettier_divergence` and
//! `assignment/rhs_mid_chain_shell_block_comment_long_prettier_divergence` pin the
//! operator-free cells.

fn format(source: &str) -> String {
    let program = tsv_ts::__private::parse(source).expect("parse failed");
    tsv_ts::__private::format_ast(&program, source)
}

fn assert_one_pass(authored: &str, fixed_point: &str) {
    let once = format(authored);
    assert_eq!(
        once, fixed_point,
        "one pass must reach the fixed point for {authored:?}"
    );
    assert_eq!(
        format(&once),
        fixed_point,
        "and stay there on the second pass for {authored:?}"
    );
}

/// Each pair is (shell authoring, the single-pass fixed point).
const CASES: &[(&str, &str)] = &[
    (
        "x = await (\n/* a */\n// b\nx.y).q();\n",
        "x =\n\tawait x\n\t\t/* a */\n\t\t// b\n\t\t.y.q();\n",
    ),
    (
        "x = !(\n/* a */\n// b\nx.y).q();\n",
        "x =\n\t!x\n\t\t/* a */\n\t\t// b\n\t\t.y.q();\n",
    ),
    (
        "x = -(\n/* a */\n// b\nx.y).q();\n",
        "x =\n\t-x\n\t\t/* a */\n\t\t// b\n\t\t.y.q();\n",
    ),
    (
        "const v = await (\n/* a */\n// b\nx.y).q();\n",
        "const v =\n\tawait x\n\t\t/* a */\n\t\t// b\n\t\t.y.q();\n",
    ),
    (
        "x = await (\n/* a */ // b\nx.y).q();\n",
        "x = await x\n\t/* a */ // b\n\t.y.q();\n",
    ),
    (
        "x = await (\n// a\nx.y).q();\n",
        "x =\n\tawait x\n\t\t// a\n\t\t.y.q();\n",
    ),
    (
        "x = await (\n/* c */\nx.y).q();\n",
        "x = await x /* c */.y\n\t.q();\n",
    ),
    (
        "x = !(\n/* c */\nx.y).q();\n",
        "x = !x /* c */.y\n\t.q();\n",
    ),
    (
        "x = await (/*\n */ x.y).q();\n",
        "x =\n\tawait /*\n\t */ x.y.q();\n",
    ),
    (
        "x = !(/*\n */ x.y).q();\n",
        "x =\n\t!(\n\t\t/*\n\t\t */ x.y.q()\n\t);\n",
    ),
];

#[test]
fn prefixed_value_mid_chain_shell_run_converges_in_one_pass() {
    for (authored, fixed_point) in CASES {
        assert_one_pass(authored, fixed_point);
    }
}

/// A run that opens with a `//` the source puts on a new line, a block behind it: the
/// comments print on lines of their own after an over-width head, so none trails it and the
/// value stays on the operator's line, as its paren-free spelling does.
#[test]
fn own_line_run_behind_an_over_width_head_converges_in_one_pass() {
    let head = "h".repeat(97);
    assert_one_pass(
        &format!("x = (\n// a\n/* b */\n{head}.y).q();\n"),
        &format!("x = {head}\n\t// a\n\t/* b */\n\t.y.q();\n"),
    );
}

/// A run with no `//` prints on the head's line whole, a block the source gave a line of
/// its own included, so the chain is no poorly breakable one and the value stays on the
/// operator's line.
#[test]
fn own_line_block_only_run_converges_in_one_pass() {
    assert_one_pass("x = (\n/* c */\nx.y).q();\n", "x = x /* c */.y\n\t.q();\n");
}
