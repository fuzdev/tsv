// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A comment run written last inside a redundant pair around a sequence's last operand, at a
//! position where the sequence prints its own pair and floats its trailing comments out: the
//! envelope claims the run and prints it after its `)` (`Printer::sequence_float_run`). The
//! claimed shapes are fixtures (`deferred_trailing_line_comment_shell_prettier_divergence`,
//! `last_operand_shell_trailing_block_prettier_divergence`); these pin the shapes the claim
//! leaves alone or that no fixture can hold.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::__private::parse(source, &arena).expect("parse failed");
    tsv_ts::__private::format_ast(&program, source)
}

/// An own-line block with no `//` beside it stays with the operand's own emitters. Deferred
/// from the envelope past a ternary's `?`, it would print at the envelope's indent, and the
/// reparse, reading it as the test's trailing comment, would print it elsewhere. Prettier
/// moves it into the consequent, so the stable form is no fixture.
#[test]
fn own_line_block_ahead_of_a_ternary_question_mark_settles_in_one_pass() {
    for source in [
        "x = (a, (b\n/* c */)) ? d : e;\n",
        "f((a, (b\n/* c */)) ? d : e);\n",
    ] {
        let once = format(source);
        assert_eq!(format(&once), once, "{source:?} must settle in one pass");
    }
}

/// A multi-line block in a shell down the last operand of a sequence a cast or a non-null
/// operator follows stays inside the sequence's pair, as it does inside a cast's own pair:
/// after the `)` its line break would end the expression ahead of the keyword or the `!`.
#[test]
fn sequence_operand_multiline_block_stays_inside_the_pair() {
    for source in [
        "x = (a, b < (c /* x\n */)) as T;\n",
        "x = (a, b < (c /* x\n */)) satisfies T;\n",
        "x = (a, (b, (c /* x\n */))) as T;\n",
        "x = (a, b < (c /* x\n */))!;\n",
        "x = (a, b < (c /* x\n */))!.m;\n",
        "f((a, b < (c /* x\n */)) as T);\n",
    ] {
        let once = format(source);
        let arena = bumpalo::Bump::new();
        assert!(
            tsv_ts::__private::parse(&once, &arena).is_ok(),
            "the output of {source:?} must parse, got {once:?}"
        );
        let twice = format(&once);
        let arena = bumpalo::Bump::new();
        assert!(
            tsv_ts::__private::parse(&twice, &arena).is_ok(),
            "the second pass of {source:?} must parse, got {twice:?}"
        );
    }
}

/// A `!==` after the sequence is an operator, not a non-null `!`, and a word that only
/// starts with `as` is no keyword: a line break may stand before either, so the block takes
/// its place after the `)` and the form settles in one pass.
#[test]
fn sequence_operand_multiline_block_before_a_loose_follower_trails_the_pair() {
    for source in [
        "x = (a, b < (c /* x\n */)) !== y;\n",
        // A word that only starts with `as` begins the next statement.
        "f((a, b < (c /* x\n */)))\nassert(y);\n",
    ] {
        let once = format(source);
        assert!(
            once.contains("(a, b < c) /* x"),
            "{source:?}: the block must trail the pair, got {once:?}"
        );
        assert_eq!(format(&once), once, "{source:?} must settle in one pass");
    }
}
