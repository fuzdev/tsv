// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A comment run written last inside a redundant pair that ends an operand, where the
//! printer adds a pair of its own around that operand (a binary operand's clarity pair, the
//! pair a cast prints around a binary operand): the region past the operand's printed end
//! is claimed by the added pair and printed after its `)`
//! (`Printer::minted_pair_trailing_region`). The single-run shapes are fixtures
//! (`clarity_pair_operand_shell_trailing_block_prettier_divergence`,
//! `as_satisfies_operand_pair_shell_trailing_block_prettier_divergence`); these are the
//! shapes no fixture can hold.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::__private::parse(source, &arena).expect("parse failed");
    tsv_ts::__private::format_ast(&program, source)
}

/// A same-line block ahead of a `//` or an own-line block in the same region: the block
/// trails the added `)` and the rest prints where the operand's own emitters print it, in
/// one pass. A `//` that ends up trailing the statement or the argument's comma is the
/// deferral every stripped shell's `//` takes, so its form is pinned exactly; an own-line
/// block's landing is not this claim's to decide, so only the claimed placement and the
/// single pass are.
#[test]
fn mixed_run_after_an_added_pair_settles_in_one_pass() {
    for (source, expected) in [
        (
            "f((a - (b /* c */ // l\n)) as T, e);\n",
            "f(\n\t(a - b) /* c */ as T, // l\n\te\n);\n",
        ),
        (
            "x = a >> (b /* c */ // l\n) >> d;\n",
            "x = (a >> b) /* c */ >> d; // l\n",
        ),
    ] {
        let once = format(source);
        assert_eq!(once, expected, "first pass of {source:?}");
        assert_eq!(format(&once), once, "{source:?} must settle in one pass");
    }
    for (source, claimed) in [
        (
            "x = (a - (b /* c */\n/* d */)) satisfies T;\n",
            "(a - b) /* c */ satisfies T",
        ),
        (
            "f(a >> (b /* c */\n/* d */) >> d, e);\n",
            "(a >> b) /* c */ >> d",
        ),
    ] {
        let once = format(source);
        assert!(
            once.contains(claimed),
            "{source:?} must print {claimed:?}, got {once:?}"
        );
        assert_eq!(format(&once), once, "{source:?} must settle in one pass");
    }
}

/// A multi-line block stays inside the pair a cast prints around its operand, and so does
/// every comment in a region holding one: after the `)` it would put a line break between
/// the `)` and the `as` / `satisfies` keyword, and the output would end the expression at
/// the `)` and fail to parse. The forms inside the pair take a second pass to settle, so no
/// fixture can pin them.
#[test]
fn cast_operand_multiline_block_stays_inside_the_pair() {
    for source in [
        "x = (a - (b /* c\nd */)) satisfies T;\n",
        "x = (a + (b /* c\nd */)) as T;\n",
        "f((a - (b /* c\nd */)) satisfies T, e);\n",
        "x = (a - (b /* c */ /* d\ne */)) as T;\n",
    ] {
        let output = format(source);
        let arena = bumpalo::Bump::new();
        assert!(
            tsv_ts::__private::parse(&output, &arena).is_ok(),
            "the output of {source:?} must parse, got {output:?}"
        );
    }
}
