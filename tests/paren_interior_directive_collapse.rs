// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]
// the TS object-type literals (`{x:1}`) are source-under-test, not format placeholders
#![allow(clippy::literal_string_with_formatting_args)]

//! A format-ignore directive alone on its line INSIDE a redundant paren shell
//! (`type P = (⏎// prettier-ignore⏎{x:  1});`) is the paren's INTERIOR gap honoring:
//! the shell drops, the directive hoists to the head's own-line position, and the
//! paren-stripped inner freezes verbatim — in ONE pass, converging to the same fixed
//! point the bare authoring holds (`type P =⏎⇥// prettier-ignore⏎⇥{x:  1};`), which is
//! itself stable. `single_child_frozen`'s window deliberately ends at the shell's own
//! span start, so the in-shell directive is honored by the shell-interior seam
//! (`paren_interior_routed_inner`) instead — freezing the INNER slice only, with the shell's
//! gap comments emitted by the head's widened emitters and trailing shell-gap comments
//! lifted after the value, so every shell comment prints exactly once (the
//! `comments:audit` ledger guards the double-print side).
//!
//! Not a fixture: the authored form carries redundant parens that canonical output
//! drops — a token move the `unformatted_*` rules exclude — so the transitions are
//! pinned here directly (the target forms themselves are ordinary, fixture-covered
//! shapes).

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

/// Assert `source` formats to `expected` in ONE pass and `expected` is a fixed point.
fn assert_one_pass(source: &str, expected: &str) {
    let pass1 = format(source);
    assert_eq!(pass1, expected, "one-pass convergence");
    assert_eq!(
        format(&pass1),
        pass1,
        "the target form must be a fixed point"
    );
}

/// Alias host: the shell drops, the directive hoists own-line, the inner freezes —
/// one pass to the alias's own-line-directive fixed point.
#[test]
fn alias_directive_freezes_inner_and_drops_parens() {
    assert_one_pass(
        "type P = (\n\t// prettier-ignore\n\t{x:  1});\n",
        "type P =\n\t// prettier-ignore\n\t{x:  1};\n",
    );
}

/// Annotation host: same collapse, same freeze, same one-pass fixed point.
#[test]
fn annotation_directive_freezes_inner_and_drops_parens() {
    assert_one_pass(
        "let v: (\n\t// prettier-ignore\n\t{x:  1});\n",
        "let v:\n\t// prettier-ignore\n\t{x:  1};\n",
    );
}

/// Nested redundant shells (`((…))`): every layer drops, the interior window spans
/// them all, and the same fixed point is reached in one pass.
#[test]
fn nested_shells_collapse_to_the_same_fixed_point() {
    assert_one_pass(
        "type P = ((\n\t// prettier-ignore\n\t{x:  1}));\n",
        "type P =\n\t// prettier-ignore\n\t{x:  1};\n",
    );
}

/// A comment in the shell's TRAILING gap (`{x:  1} /* t */)`) survives the strip —
/// lifted after the frozen inner, so the collapse is lossless and prints it once.
#[test]
fn trailing_shell_comment_lifted_after_frozen_inner() {
    assert_one_pass(
        "type P = (\n\t// prettier-ignore\n\t{x:  1} /* t */);\n",
        "type P =\n\t// prettier-ignore\n\t{x:  1} /* t */;\n",
    );
}

/// Control: a PLAIN own-line comment in the shell keeps the existing strip-and-hang
/// path — the comment survives own-line but the inner REFORMATS (only a directive
/// earns the freeze).
#[test]
fn plain_comment_control_inner_reformats() {
    assert_one_pass(
        "type P = (\n\t// c\n\t{x:  1});\n",
        "type P =\n\t// c\n\t{ x: 1 };\n",
    );
}
