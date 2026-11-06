// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A pair the printer keeps or requires around a value, and a gap that follows a value, opens
//! its trailing run at the value's printed end, so a comment written last in a stripped shell
//! down the value's right spine prints ahead of the run written after that shell
//! (`docs/comments.md` §The right-spine shell run). A FROZEN operand is the exception: a
//! format-ignore directive ahead of it prints its slice verbatim, the stripped shell and its
//! comment included, so the pair or gap must not claim that comment as well — claimed, it
//! printed twice, and the copy grew by a line every pass. The same holds for a child the walk
//! down the right spine reaches (an assignment's value, a conditional's alternate, an arrow's
//! body, a `yield*` operand, a sequence's last operand): a directive in the gap past that
//! child's delimiter freezes it, and the walk stops ahead of it; a directive anywhere else in
//! the parent does not. The converse holds too: a comment written after the slice, inside a
//! pair the position still prints around it (the operand pair a chain's `!` requires,
//! `(⏎// prettier-ignore⏎a + b /* x */)!.m`), is the pair's to print, since the slice ends
//! ahead of it.
//!
//! Not a fixture: prettier moves the directive past the `=` and welds the two comments onto one
//! line at every one of these, or moves the block past the `)`, a prettier behaviour of its own,
//! so the transitions are pinned here directly.

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

/// A `new` callee, whose callee→arguments gap opens at the callee's end when frozen.
#[test]
fn new_callee_frozen() {
    assert_frozen_once(
        "k = new (\n// prettier-ignore\na + (b // c\n) /* x */ // d\n)();\n",
        "k = new\n\t// prettier-ignore\n\t(a + (b // c\n)) /* x */ // d\n\t\t();\n",
    );
}

/// A template tag.
#[test]
fn tag_frozen() {
    assert_frozen_once(
        "k = (\n// prettier-ignore\na + (b // c\n) /* x */ // d\n)`t`;\n",
        "k =\n\t// prettier-ignore\n\t(a + (b // c\n)) /* x */ // d\n\t`t`;\n",
    );
}

/// The `export =` line-comment shell.
#[test]
fn export_assignment_frozen() {
    assert_frozen_once(
        "export = (\n// prettier-ignore\na + (b // c\n) /* x */ // d\n);\n",
        "export =\n\t// prettier-ignore\n\t(\n\t\ta + (b // c\n) /* x */ // d\n\t);\n",
    );
}

/// A unary operator's comment-holder pair.
#[test]
fn unary_operand_frozen() {
    assert_frozen_once(
        "k = -(\n// prettier-ignore\na + (b // c\n) /* x */ // d\n);\n",
        "k = -(\n\t// prettier-ignore\n\ta + (b // c\n) /* x */ // d\n);\n",
    );
}

/// `await`'s operand pair.
#[test]
fn await_operand_frozen() {
    assert_frozen_once(
        "async function g() {\n\tk = await (\n// prettier-ignore\na + (b // c\n) /* x */ // d\n);\n}\n",
        "async function g() {\n\tk = await\n\t\t// prettier-ignore\n\t\t(\n\t\t\ta + (b // c\n) /* x */ // d\n\t\t);\n}\n",
    );
}

/// A binary chain's leftmost operand, its pair required.
#[test]
fn binary_left_operand_frozen() {
    assert_frozen_once(
        "k = (\n// prettier-ignore\na + (b // c\n) /* x */ // d\n) * 2;\n",
        "k =\n\t// prettier-ignore\n\t(a + (b // c\n)) /* x */ // d\n\t* 2;\n",
    );
}

/// A binary chain's leftmost operand, its pair redundant.
#[test]
fn logical_left_operand_frozen() {
    assert_frozen_once(
        "k = (\n// prettier-ignore\na + (b // c\n) /* x */ // d\n) && q;\n",
        "k =\n\t// prettier-ignore\n\ta + (b // c\n) /* x */ // d\n\t&& q;\n",
    );
}

/// A spread argument's stripped-paren interior.
#[test]
fn spread_argument_frozen() {
    assert_frozen_once(
        "k = [...(\n// prettier-ignore\na + (b // c\n) /* x */ // d\n)];\n",
        "k = [\n\t...\n\t\t// prettier-ignore\n\t\t(a + (b // c\n) /* x */) // d\n];\n",
    );
}

/// An expression statement's retained shell.
#[test]
fn expression_statement_frozen() {
    assert_frozen_once(
        "(\n// prettier-ignore\na + (b // c\n) /* x */ // d\n);\n",
        "(\n\t// prettier-ignore\n\ta + (b // c\n) /* x */ // d\n);\n",
    );
}

/// A member chain's frozen head: the first member gap opens at the head's span end, the
/// slice printing its stripped shell's comment.
#[test]
fn member_chain_head_frozen() {
    assert_frozen_once(
        "k = (\n// prettier-ignore\na + (b // c\n)).m;\n",
        "k =\n\t// prettier-ignore\n\t(a + (b // c\n)).m;\n",
    );
}

/// A call chain's frozen head: the first call's callee gap opens at the head's span end, so the
/// slice alone prints the stripped shell's comment.
#[test]
fn call_chain_head_frozen() {
    assert_frozen_once(
        "k = (\n// prettier-ignore\na + (b // c\n))().m;\n",
        "k =\n\t// prettier-ignore\n\t(a + (b // c\n))().m;\n",
    );
}

/// A callee whose right spine reaches a frozen value: the assignment's value is the slice, so the
/// callee gap opens past it.
#[test]
fn callee_spine_child_frozen() {
    assert_frozen_once(
        "k = (x =\n// prettier-ignore\na + (b // c\n))();\n",
        "k = (x =\n\t// prettier-ignore\n\ta + (b // c\n))();\n",
    );
}

/// A `new` callee.
#[test]
fn new_callee_spine_child_frozen() {
    assert_frozen_once(
        "k = new (x =\n// prettier-ignore\na + (b // c\n))();\n",
        "k = new (x =\n\t// prettier-ignore\n\ta + (b // c\n))();\n",
    );
}

/// A template tag.
#[test]
fn tag_spine_child_frozen() {
    assert_frozen_once(
        "k = (x =\n// prettier-ignore\na + (b // c\n))`t`;\n",
        "k = (x =\n\t// prettier-ignore\n\ta + (b // c\n))`t`;\n",
    );
}

/// A unary operand's retained pair, a block written after the slice.
#[test]
fn unary_operand_spine_child_frozen() {
    assert_frozen_once(
        "k = -(x =\n// prettier-ignore\na + (b // c\n) /* x */);\n",
        "k = -(\n\t(x =\n\t\t// prettier-ignore\n\t\ta + (b // c\n)) /* x */\n);\n",
    );
}

/// An `await` operand's retained pair.
#[test]
fn await_operand_spine_child_frozen() {
    assert_frozen_once(
        "k = await (x =\n// prettier-ignore\na + (b // c\n) /* x */);\n",
        "k = await (x =\n\t// prettier-ignore\n\ta + (b // c\n) /* x */);\n",
    );
}

/// An `as` operand's pair.
#[test]
fn as_operand_spine_child_frozen() {
    assert_frozen_once(
        "k = (x =\n// prettier-ignore\na + (b // c\n) /* x */) as T;\n",
        "k = (x =\n\t// prettier-ignore\n\ta + (b // c\n)) /* x */ as T;\n",
    );
}

/// A `case` test's head→`:` gap.
#[test]
fn case_test_spine_child_frozen() {
    assert_frozen_once(
        "switch (k) { case (x =\n// prettier-ignore\na + (b // c\n) /* x */): }\n",
        "switch (k) {\n\tcase (x =\n\t\t// prettier-ignore\n\t\ta + (b // c\n)) /* x */:\n}\n",
    );
}

/// A superclass.
#[test]
fn superclass_spine_child_frozen() {
    assert_frozen_once(
        "class A extends (x =\n// prettier-ignore\na + (b // c\n)) {}\n",
        "class A extends (x =\n\t// prettier-ignore\n\ta + (b // c\n)) {}\n",
    );
}

/// An `if` test.
#[test]
fn if_test_spine_child_frozen() {
    assert_frozen_once(
        "if (x =\n// prettier-ignore\na + (b // c\n)) {}\n",
        "if (\n\t(x =\n\t\t// prettier-ignore\n\t\ta + (b // c\n))\n) {\n}\n",
    );
}

/// A member gap.
#[test]
fn member_object_spine_child_frozen() {
    assert_frozen_once(
        "k = (x =\n// prettier-ignore\na + (b // c\n)).m;\n",
        "k = (\n\tx =\n\t\t// prettier-ignore\n\t\ta + (b // c\n)\n).m;\n",
    );
}

/// A retained value shell, a `//` written after the slice.
#[test]
fn value_shell_spine_child_frozen() {
    assert_frozen_once(
        "let k = (x =\n// prettier-ignore\na + (b // c\n) // d\n);\n",
        "let k = (\n\tx =\n\t\t// prettier-ignore\n\t\ta + (b // c\n) // d\n);\n",
    );
}

/// A conditional's alternate, frozen past its `:`.
#[test]
fn conditional_alternate_spine_child_frozen() {
    assert_frozen_once(
        "k = (q ? r :\n// prettier-ignore\na + (b // c\n))();\n",
        "k = (q\n\t? r\n\t:\n\t\t// prettier-ignore\n\t\ta + (b // c\n))();\n",
    );
}

/// An arrow's body, frozen past its `=>`.
#[test]
fn arrow_body_spine_child_frozen() {
    assert_frozen_once(
        "k = (() =>\n// prettier-ignore\na + (b // c\n))();\n",
        "k = (() =>\n\t// prettier-ignore\n\ta + (b // c\n))();\n",
    );
}

/// A `yield*` operand, frozen inside the pair its hang prints.
#[test]
fn yield_operand_spine_child_frozen() {
    assert_frozen_once(
        "function* g() { k = (yield*\n// prettier-ignore\na + (b // c\n))(); }\n",
        "function* g() {\n\tk = (yield* (\n\t\t// prettier-ignore\n\t\ta + (b // c\n)\n\t))();\n}\n",
    );
}

/// A block directive.
#[test]
fn callee_spine_child_frozen_block_directive() {
    assert_frozen_once(
        "k = (x =\n/* prettier-ignore */\na + (b // c\n))();\n",
        "k = (x =\n\t/* prettier-ignore */\n\ta + (b // c\n))();\n",
    );
}

/// Control: a directive ahead of the `=` freezes nothing, so the walk reaches the shell and
/// the callee gap prints its comment past the `)`.
#[test]
fn callee_spine_directive_before_operator() {
    assert_frozen_once(
        "k = (x\n// prettier-ignore\n= a + (b // c\n))();\n",
        "k = (x\n\t// prettier-ignore\n\t= a + b) // c\n\t();\n",
    );
}

/// Control: a directive ahead of the `:` freezes nothing either.
#[test]
fn callee_spine_directive_before_colon() {
    assert_frozen_once(
        "k = (q ? r\n// prettier-ignore\n: a + (b // c\n))();\n",
        "k = (q\n\t? r\n\t// prettier-ignore\n\t: a + b) // c\n\t();\n",
    );
}

/// Control: a directive freezing the consequent leaves the alternate's shell to the walk.
#[test]
fn callee_spine_directive_on_consequent() {
    assert_frozen_once(
        "k = (q ?\n// prettier-ignore\nr : a + (b // c\n))();\n",
        "k = (q\n\t?\n\t\t// prettier-ignore\n\t\tr\n\t: a + b) // c\n\t();\n",
    );
}

/// Control: the same, a `//` written after the pair, which the shell's comment stays ahead of.
#[test]
fn callee_spine_directive_on_consequent_then_line() {
    assert_frozen_once(
        "k = (a ? (\n// prettier-ignore\nb) : c + (d // c\n)) // d\n();\n",
        "k = (a\n\t?\n\t\t// prettier-ignore\n\t\tb\n\t: c + d) // c\n\t// d\n\t();\n",
    );
}

/// A sequence's last operand, frozen past its comma: the callee gap opens past the slice.
#[test]
fn sequence_last_operand_frozen_callee() {
    assert_frozen_once(
        "k = (z,\n// prettier-ignore\na + (b // c\n))();\n",
        "k =\n\t(z,\n\t// prettier-ignore\n\ta + (b // c\n))();\n",
    );
}

/// A retained value shell around the sequence.
#[test]
fn sequence_last_operand_frozen_value_shell() {
    assert_frozen_once(
        "let k = (z,\n// prettier-ignore\na + (b // c\n));\n",
        "let k =\n\t(z,\n\t// prettier-ignore\n\ta + (b // c\n));\n",
    );
}

/// An expression statement's terminator gap, a block written after the sequence.
#[test]
fn sequence_last_operand_frozen_statement_tail() {
    assert_frozen_once(
        "(z,\n// prettier-ignore\na + (b // c\n)) /* x */;\n",
        "(z,\n\t// prettier-ignore\n\ta + (b // c\n)); /* x */\n",
    );
}

/// A nested sequence's last operand.
#[test]
fn nested_sequence_last_operand_frozen() {
    assert_frozen_once(
        "k = (z, (y,\n// prettier-ignore\na + (b // c\n)))();\n",
        "k =\n\t(z,\n\t(y,\n\t// prettier-ignore\n\ta + (b // c\n)))();\n",
    );
}

/// Control: a comment that is no directive leaves the walk going into the last operand.
#[test]
fn sequence_last_operand_comment_not_directive() {
    assert_frozen_once(
        "k = (z,\n// e\na + (b // c\n))();\n",
        "k =\n\t(z,\n\t// e\n\ta + b)(); // c\n",
    );
}

/// Assert `source` formats to `expected` in ONE pass, `expected` is a fixed point, and `comment`
/// prints exactly once.
fn assert_frozen_keeps(source: &str, expected: &str, comment: &str) {
    let pass1 = format(source);
    assert_eq!(pass1, expected, "one-pass convergence");
    assert_eq!(
        format(&pass1),
        pass1,
        "the target form must be a fixed point"
    );
    assert_eq!(
        pass1.matches(comment).count(),
        1,
        "the comment after the frozen slice prints once"
    );
}

/// A frozen chain base that a `!` follows: the block written after the slice, inside the pair
/// the `!` requires, stays inside it.
#[test]
fn non_null_chain_base_frozen_block_after_slice() {
    assert_frozen_keeps(
        "k = (\n// prettier-ignore\na + b /* x */)!.m;\n",
        "k =\n\t// prettier-ignore\n\t(a + b /* x */)!.m;\n",
        "/* x */",
    );
}

/// The same block written on a line of its own.
#[test]
fn non_null_chain_base_frozen_own_line_block() {
    assert_frozen_keeps(
        "k = (\n// prettier-ignore\na + b\n/* x */\n)!.m;\n",
        "k =\n\t// prettier-ignore\n\t(a + b /* x */)!.m;\n",
        "/* x */",
    );
}

/// A block directive, and a call after the `!`.
#[test]
fn non_null_chain_base_frozen_block_directive_call() {
    assert_frozen_keeps(
        "k = (\n/* format-ignore */\na + b /* x */)!();\n",
        "k =\n\t/* format-ignore */\n\t(a + b /* x */)!();\n",
        "/* x */",
    );
}

/// A computed member after the `!`, over a conditional base.
#[test]
fn non_null_chain_base_frozen_computed_member() {
    assert_frozen_keeps(
        "k = (\n// format-ignore\na ? b : c /* x */)![0];\n",
        "k =\n\t// format-ignore\n\t(a ? b : c /* x */)![0];\n",
        "/* x */",
    );
}

/// A slice holding a stripped shell's comment of its own, the block written after it.
#[test]
fn non_null_chain_base_frozen_shell_then_block() {
    assert_frozen_keeps(
        "k = (\n// prettier-ignore\na + (b // c\n) /* x */)!.m;\n",
        "k =\n\t// prettier-ignore\n\t(a + (b // c\n) /* x */)!.m;\n",
        "/* x */",
    );
}

/// Control: with no `!`, the member gap prints the block past the pair.
#[test]
fn member_chain_base_frozen_block_after_slice() {
    assert_frozen_keeps(
        "k = (\n// prettier-ignore\na + b /* x */).m;\n",
        "k =\n\t// prettier-ignore\n\t(a + b) /* x */.m;\n",
        "/* x */",
    );
}

/// Control: a `!` ending the expression keeps its operand's pair and the block inside it.
#[test]
fn non_null_frozen_block_after_slice() {
    assert_frozen_keeps(
        "k = (\n// prettier-ignore\na + b /* x */)!;\n",
        "k = (\n\t// prettier-ignore\n\ta + b /* x */\n)!;\n",
        "/* x */",
    );
}

/// Control: a directive freezing an arrow callee's parameter leaves its body's shell to the
/// walk, so the comment prints inside the callee's own pair, ahead of the one written after it.
#[test]
fn function_callee_spine_directive_on_parameter() {
    assert_frozen_once(
        "k = ((\n// prettier-ignore\np) => a + (b // c\n)) // d\n();\n",
        "k = (\n\t(\n\t\t// prettier-ignore\n\t\tp\n\t) => a + b // c\n) // d\n\t();\n",
    );
}
