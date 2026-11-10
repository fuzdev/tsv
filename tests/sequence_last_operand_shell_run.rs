// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A comment run written last inside a redundant pair around a sequence's last operand, at a
//! position where the sequence prints its own pair and floats its trailing comments out: the
//! envelope claims the run and prints it after its `)` (`Printer::sequence_float_run`). The
//! claimed shapes are fixtures (`deferred_trailing_line_comment_shell_prettier_divergence`,
//! `last_operand_shell_trailing_block_prettier_divergence`), and so are the multi-line blocks
//! ahead of a token that must share the `)`'s line
//! (`last_operand_shell_multiline_block_non_null_prettier_divergence`,
//! `last_operand_shell_multiline_block_cast_prettier_divergence`); these pin the shapes the
//! claim leaves alone or that no fixture can hold.

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

/// The number of top-level statements `source` parses to: a line break the formatter moves in
/// front of a postfix token can end the statement there instead of failing the reparse.
fn statement_count(source: &str) -> usize {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::__private::parse(source, &arena);
    assert!(program.is_ok(), "{source:?} must parse: {program:?}");
    let parsed = program.expect("checked above");
    parsed.view().stmt_list(parsed.body).len()
}

/// A multi-line block written last inside a shell down a sequence's last operand, where a
/// token that must share the sequence's `)` line follows it. Printed after the `)`, its line
/// break would end the expression ahead of the token: a dead document, or before `![0]` a
/// second statement. The output must parse to the source's statements and settle in one pass.
/// The update targets are no fixture: acorn and prettier reject an update of a sequence (tsc
/// reports it from its checker). `!!` and `!(…) as T` pin the chain base's and a unary's mark.
#[test]
fn sequence_shell_multiline_block_before_a_tight_follower_keeps_the_statement() {
    for source in [
        "f((a, (b /* x\n */))!);\n",
        "x = (a, (b /* x\n */))![0];\n",
        "x = (a, (b /* x\n */))!!;\n",
        "x = !(a, (b /* x\n */)) as T;\n",
        "x = (a, (b /* x\n */))++;\n",
        "x = (a, (b /* x\n */))--;\n",
        "x = (a, b2 < (b /* x\n */))++;\n",
        "x = (a, b2 < (b /* x\n */))--;\n",
        "x = (a, (b /* y */ /* x\n */))!;\n",
        "x = (a, (b /* x\n */\n/* y */))!;\n",
        "x = (a, b < (c /* x\n */) // y\n)!;\n",
    ] {
        let once = format(source);
        assert_eq!(
            statement_count(&once),
            statement_count(source),
            "the output of {source:?} must keep its statements, got {once:?}"
        );
        assert_eq!(format(&once), once, "{source:?} must settle in one pass");
    }
}

/// A single-line block on the run's line, then an own-line one, puts no line break ahead of
/// the token: the run trails the `)` and defers, as ahead of any follower.
#[test]
fn sequence_shell_single_line_block_before_a_tight_follower_trails_the_pair() {
    let source = "x = (a, (b /* y */\n/* z */))!;\n";
    let once = format(source);
    assert_eq!(once, "x = (a, b) /* y */!;\n/* z */\n", "{source:?}");
    assert_eq!(format(&once), once, "{source:?} must settle in one pass");
}

/// The same block where a single-line block also sits between the sequence and the `!`: the
/// output must parse to the source's statements.
#[test]
fn sequence_shell_multiline_block_before_a_commented_non_null_parses() {
    // TODO: the second pass moves the `!`'s block inside the shell the sequence adds
    // (`x = (⏎\t(a, b) /* x⏎\t */⏎\t/* t */⏎)!;`), where the non-null keeps the author's
    // pair for the line the run spans
    for source in [
        "x = (a, (b /* x\n */) /* t */)!;\n",
        "f((a, (b /* x\n */) /* t */)!);\n",
    ] {
        let once = format(source);
        assert_eq!(statement_count(&once), 1, "{source:?}: got {once:?}");
        let twice = format(&once);
        assert_eq!(statement_count(&twice), 1, "{source:?}: got {twice:?}");
    }
}

/// The same block where the token follows an enclosing call's `)`, or stands past a line
/// break: it is no follower of the sequence, whose `)` is followed by the call's, so the block
/// trails the sequence inside the call as it does ahead of any loose follower.
#[test]
fn sequence_shell_multiline_block_inside_a_call_before_a_tight_follower_trails_the_operand() {
    let source = "f((a, (b /* x\n */)))!;\n";
    assert_eq!(
        format(source),
        "f(\n\t(a, b) /* x\n\t */\n)!;\n",
        "{source:?}"
    );
    for source in [
        source,
        "f((a, (b /* x\n */))) as T;\n",
        "g(f((a, (b /* x\n */))))!;\n",
        "f(c, (a, (b /* x\n */)))!;\n",
        "x = f((a, (b /* x\n */)))++;\n",
        "f((a, (b /* x\n */)))\n!y;\n",
        "f((a, (b /* x\n */\n/* y */)))!;\n",
    ] {
        let once = format(source);
        assert_eq!(
            statement_count(&once),
            statement_count(source),
            "the output of {source:?} must keep its statements, got {once:?}"
        );
        assert_eq!(format(&once), once, "{source:?} must settle in one pass");
    }
}

/// A sequence nested as the last operand of another is followed by the outer sequence's `)`,
/// never by the token, so its block stays inside the outer pair. Ahead of a cast or a postfix
/// update that pair keeps it there in one pass; every output must parse to one statement.
#[test]
fn nested_sequence_shell_multiline_block_before_a_tight_follower() {
    for source in [
        "x = (a, (b2, b < (c /* x\n */)))++;\n",
        "x = (a, (b2, (b /* x\n */)))--;\n",
        "x = (a, (b2, (b /* x\n */))) as T;\n",
        "x = (a, (b2, (b /* x\n */))) satisfies T;\n",
    ] {
        let once = format(source);
        assert_eq!(statement_count(&once), 1, "{source:?}: got {once:?}");
        assert_eq!(format(&once), once, "{source:?} must settle in one pass");
    }
    // TODO: the non-null spelling settles in two passes: the second keeps a pair around the
    // outer sequence and the block (`x = (⏎\t(a, (b2, b)) /* x⏎\t */⏎)!;`), the form a single
    // sequence takes in one pass
    for source in [
        "x = (a, (b2, (b /* x\n */)))!;\n",
        "x = (a, (b2, (b /* x\n */)))![0];\n",
    ] {
        let once = format(source);
        assert_eq!(statement_count(&once), 1, "{source:?}: got {once:?}");
        let twice = format(&once);
        assert_eq!(statement_count(&twice), 1, "{source:?}: got {twice:?}");
    }
}

/// The same block under a unary operator inside a pair the follower's parent adds
/// (`(void …)!`, `(!…)++`): the unary inherits the follower, so the sequence keeps its own
/// shell inside the unary's operand, the second pass's form, in one pass.
#[test]
fn unary_operand_sequence_shell_multiline_block_inside_an_added_pair_settles() {
    for (source, expected) in [
        (
            "x = (void (a, (b /* x\n */)))!;\n",
            "x = (void (\n\t(a, b) /* x\n\t */\n))!;\n",
        ),
        (
            "x = (!(a, (b /* x\n */)))++;\n",
            "x = (!(\n\t(a, b) /* x\n\t */\n))++;\n",
        ),
    ] {
        let once = format(source);
        assert_eq!(once, expected, "{source:?}");
        assert_eq!(format(&once), once, "{source:?} must settle in one pass");
    }
}

/// An own-line block in the shell is no trailing run of the operand's line: it defers past the
/// statement's `;`, whatever follows the sequence.
#[test]
fn sequence_shell_own_line_multiline_block_trails_the_statement() {
    let source = "x = (a, (b\n/* x\n */))!;\n";
    let once = format(source);
    assert_eq!(once, "x = (a, b)!;\n/* x\n */\n", "{source:?}");
    assert_eq!(format(&once), once, "{source:?} must settle in one pass");
}

/// An `{#each}` head's `as` is the block's separator, not a cast: the head expression ends
/// before it, so a line break may stand there, and the block trails the sequence's `)` as it
/// does ahead of any loose follower, settling in one pass.
#[test]
fn each_head_separator_is_no_tight_follower() {
    for (source, expected) in [
        (
            "{#each (a, (b /* x\n */)) as item}{item}{/each}\n",
            "{#each (a, b) /* x\n */\nas item}\n\t{item}\n{/each}\n",
        ),
        (
            "{#each (a, (b /* x\n */)) as item, i (item)}{item}{/each}\n",
            "{#each (a, b) /* x\n */\nas item, i (item)}\n\t{item}\n{/each}\n",
        ),
        (
            "{#each (a, (b /* x\n */)) as T}{T}{/each}\n",
            "{#each (a, b) /* x\n */\nas T}\n\t{T}\n{/each}\n",
        ),
        (
            "{#each (a, (b2 < (b /* x\n */))) as item}{item}{/each}\n",
            "{#each (a, b2 < b) /* x\n */\nas item}\n\t{item}\n{/each}\n",
        ),
    ] {
        let once = tsv_svelte::format(source).expect("format failed");
        assert_eq!(once, expected, "{source:?}");
        let twice = tsv_svelte::format(&once).expect("the output failed to format");
        assert_eq!(twice, once, "{source:?} must settle in one pass");
    }
}
