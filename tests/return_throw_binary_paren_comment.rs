// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A comment between a binary `return` / `throw` operand and the `)` of grouping parens the
//! author wrote around it stays inside the pair the printer emits — whether the statement
//! ends in a `;` or ASI ends it at that `)`.
//!
//! The fixed points are pinned by the `statements/return_throw/operand_paren_binary_*`
//! fixtures. These rows hold the authorings a fixture cannot: spellings prettier needs a
//! second pass for, carriers a fixture would have to repeat per variant, and the one cell
//! that converges in two passes on purpose.

/// Format a TypeScript document.
fn format(source: &str) -> String {
    tsv_ts::format_str(source).expect("format failed")
}

/// The document's comments, sorted — the multiset a format must conserve.
fn comments(source: &str) -> Vec<String> {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    let mut found: Vec<String> = program
        .comments
        .iter()
        .map(|comment| comment.content(source).to_owned())
        .collect();
    found.sort();
    found
}

/// Why `authored` does not reach `fixed_point` in one pass, stay there, and keep its
/// comments — `None` when it does all three.
fn one_pass_failure(authored: &str, fixed_point: &str) -> Option<String> {
    let once = format(authored);
    if once != fixed_point {
        return Some(format!(
            "one pass must reach the fixed point for {authored:?}: got {once:?}"
        ));
    }
    if format(&once) != fixed_point {
        return Some(format!(
            "the second pass must stay at the fixed point for {authored:?}"
        ));
    }
    if comments(authored) != comments(&once) {
        return Some(format!("every comment must survive for {authored:?}"));
    }
    None
}

/// Assert every (authoring, fixed point) row holds, naming each row that does not.
fn assert_all_one_pass(cases: &[(&str, &str)]) {
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|(authored, fixed_point)| one_pass_failure(authored, fixed_point))
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} rows fail:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

/// Assert `authored` reaches `fixed_point` in one pass, stays there, and keeps its comments.
fn assert_one_pass(authored: &str, fixed_point: &str) {
    assert_all_one_pass(&[(authored, fixed_point)]);
}

/// Each pair is (authoring, the single-pass fixed point).
const HELD_PAIR_CASES: &[(&str, &str)] = &[
    // ASI ends the statement at the `)`: a line comment trailing the operand
    (
        "function f() {\n\treturn (\n\t\ta && b // c\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\ta && b // c\n\t);\n}\n",
    ),
    // ASI, a line comment on a line of its own
    (
        "function f() {\n\treturn (\n\t\ta && b\n\t\t// c\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\ta && b\n\t\t// c\n\t);\n}\n",
    ),
    // ASI, `throw`, a block comment on a line of its own
    (
        "function f() {\n\tthrow (\n\t\ta || b\n\t\t/* c */\n\t)\n}\n",
        "function f() {\n\tthrow (\n\t\ta || b\n\t\t/* c */\n\t);\n}\n",
    ),
    // ASI, a block comment spanning lines
    (
        "function f() {\n\treturn (\n\t\ta ?? b /* c1\n\t\tc2 */\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\ta ?? b /* c1\n\t\tc2 */\n\t);\n}\n",
    ),
    // ASI, a second pair: the comment inside the inner one
    (
        "function f() {\n\treturn ((\n\t\ta && b // c\n\t))\n}\n",
        "function f() {\n\treturn (\n\t\ta && b // c\n\t);\n}\n",
    ),
    // ASI, a second pair: the comment between the two closers
    (
        "function f() {\n\treturn (\n\t\t(a && b) // c\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\ta && b // c\n\t);\n}\n",
    ),
    // ASI, a second pair: an own-line comment between the two closers
    (
        "function f() {\n\treturn (\n\t\t(a && b)\n\t\t// c\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\ta && b\n\t\t// c\n\t);\n}\n",
    ),
    // a second pair, a line comment inside each: two comments stay two, with a `;` …
    (
        "function f() {\n\treturn ((\n\t\ta && b // c1\n\t) // c2\n\t);\n}\n",
        "function f() {\n\treturn (\n\t\ta && b // c1\n\t\t// c2\n\t);\n}\n",
    ),
    // … and under ASI
    (
        "function f() {\n\treturn ((\n\t\ta && b // c1\n\t) // c2\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\ta && b // c1\n\t\t// c2\n\t);\n}\n",
    ),
    // ASI, comments whose text holds the terminator and the closer
    (
        "function f() {\n\treturn (\n\t\ta && b /* ; */ // )\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\ta && b /* ; */ // )\n\t);\n}\n",
    ),
    // ASI, a pair the last operand requires
    (
        "function f() {\n\treturn (\n\t\ta && (b || c) // c\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\ta && (b || c) // c\n\t);\n}\n",
    ),
    // ASI, a comment inside the chain and one trailing it
    (
        "function f() {\n\treturn (\n\t\ta && // c1\n\t\tb // c2\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\ta && // c1\n\t\tb // c2\n\t);\n}\n",
    ),
    // ASI, the operand glued to the `(`
    (
        "function f() {\n\treturn (a && b // c\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\ta && b // c\n\t);\n}\n",
    ),
    // ASI, a block comment leading the operand and a line comment trailing it
    (
        "function f() {\n\treturn (/* c0 */ a && b // c1\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\t/* c0 */ a && b // c1\n\t);\n}\n",
    ),
    // ASI, a block comment then a line comment on the operand's line
    (
        "function f() {\n\treturn (\n\t\ta && b /* c1 */ // c2\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\ta && b /* c1 */ // c2\n\t);\n}\n",
    ),
    // ASI, a block comment on the operand's line, then a comment on its own line: the
    // run holds the pair although its first comment alone would not
    (
        "function f() {\n\treturn (\n\t\ta && b /* c1 */\n\t\t// c2\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\ta && b /* c1 */\n\t\t// c2\n\t);\n}\n",
    ),
    // ASI, a pair of block comments the author wrote on one line of their own
    (
        "function f() {\n\treturn (\n\t\ta + b\n\t\t/* c1 */ /* c2 */\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\ta + b\n\t\t/* c1 */ /* c2 */\n\t);\n}\n",
    ),
    // ASI, a blank line above the own-line comment survives
    (
        "function f() {\n\treturn (\n\t\ta < b\n\n\t\t// c\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\ta < b\n\n\t\t// c\n\t);\n}\n",
    ),
    // a blank line between the comment and the `)` does not survive, with a `;` …
    (
        "function f() {\n\treturn (\n\t\ta && b // c\n\n\t);\n}\n",
        "function f() {\n\treturn (\n\t\ta && b // c\n\t);\n}\n",
    ),
    // … under ASI …
    (
        "function f() {\n\treturn (\n\t\ta && b // c\n\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\ta && b // c\n\t);\n}\n",
    ),
    // … and below an own-line comment
    (
        "function f() {\n\treturn (\n\t\ta && b\n\t\t// c\n\n\t);\n}\n",
        "function f() {\n\treturn (\n\t\ta && b\n\t\t// c\n\t);\n}\n",
    ),
    // a comment past the `)` trails the `;`: behind a line comment inside the pair …
    (
        "function f() {\n\treturn (\n\t\ta && b // c1\n\t) /* c2 */;\n}\n",
        "function f() {\n\treturn (\n\t\ta && b // c1\n\t); /* c2 */\n}\n",
    ),
    // … behind an own-line comment inside the pair …
    (
        "function f() {\n\treturn (\n\t\ta && b\n\t\t// c1\n\t) /* c2 */;\n}\n",
        "function f() {\n\treturn (\n\t\ta && b\n\t\t// c1\n\t); /* c2 */\n}\n",
    ),
    // … and from a line of its own ahead of the `;`
    (
        "function f() {\n\treturn (\n\t\ta && b // c1\n\t)\n\t// c2\n\t;\n}\n",
        "function f() {\n\treturn (\n\t\ta && b // c1\n\t);\n\t// c2\n}\n",
    ),
    // ASI, switch-case consequents
    (
        "function f() {\n\tswitch (x) {\n\t\tcase 1:\n\t\t\treturn (\n\t\t\t\ta && b // c1\n\t\t\t)\n\t\tdefault:\n\t\t\tthrow (\n\t\t\t\ta || b\n\t\t\t\t// c2\n\t\t\t)\n\t}\n}\n",
        "function f() {\n\tswitch (x) {\n\t\tcase 1:\n\t\t\treturn (\n\t\t\t\ta && b // c1\n\t\t\t);\n\t\tdefault:\n\t\t\tthrow (\n\t\t\t\ta || b\n\t\t\t\t// c2\n\t\t\t);\n\t}\n}\n",
    ),
    // ASI, a statement of the document itself, with a statement after it
    (
        "throw (\n\ta || b // c\n)\nfn()\n",
        "throw (\n\ta || b // c\n);\nfn();\n",
    ),
    // ASI, a statement of the document itself, ending the document
    (
        "throw (\n\ta || b\n\t// c\n)\n",
        "throw (\n\ta || b\n\t// c\n);\n",
    ),
    // ASI, a clause body ahead of an `else`
    (
        "function f() {\n\tif (cond)\n\t\treturn (\n\t\t\ta && b // c\n\t\t)\n\telse fn()\n}\n",
        "function f() {\n\tif (cond)\n\t\treturn (\n\t\t\ta && b // c\n\t\t);\n\telse fn();\n}\n",
    ),
    // a clause body owns the comment past its `)`: a block on the `)` line …
    (
        "function f() {\n\tif (cond)\n\t\treturn (\n\t\t\ta && b // c1\n\t\t) /* c2 */;\n\telse fn();\n}\n",
        "function f() {\n\tif (cond)\n\t\treturn (\n\t\t\ta && b // c1\n\t\t); /* c2 */\n\telse fn();\n}\n",
    ),
    // … and a line comment there, behind an own-line comment inside the pair
    (
        "function f() {\n\tif (cond)\n\t\treturn (\n\t\t\ta && b\n\t\t\t// c1\n\t\t) // c2\n\t\t;\n\telse fn();\n}\n",
        "function f() {\n\tif (cond)\n\t\treturn (\n\t\t\ta && b\n\t\t\t// c1\n\t\t); // c2\n\telse fn();\n}\n",
    ),
    // ASI, a clause body with no `else`
    (
        "function f() {\n\tif (cond)\n\t\tthrow (\n\t\t\ta && b\n\t\t\t// c\n\t\t)\n\tfn()\n}\n",
        "function f() {\n\tif (cond)\n\t\tthrow (\n\t\t\ta && b\n\t\t\t// c\n\t\t);\n\tfn();\n}\n",
    ),
    // ASI, a single-line block comment trailing an operand that breaks stays before the `)`
    (
        "function f() {\n\treturn (\n\t\taaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &&\n\t\tbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb /* c */\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\taaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &&\n\t\tbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb /* c */\n\t);\n}\n",
    ),
    // several pairs, a comment inside each and past the last `)`, with a `;`: every
    // comment inside keeps its place in the one pair printed, and the comments past the
    // last `)` trail the `;` in the order written
    (
        "function f() {\n\treturn (((a && b // c1\n\t) /* c2 */) // c3\n\t) /* c4 */; // c5\n}\n",
        "function f() {\n\treturn (\n\t\ta && b // c1\n\t\t/* c2 */ // c3\n\t); /* c4 */ // c5\n}\n",
    ),
    // the inner pair is the last operand's own
    (
        "function f() {\n\treturn (\n\t\ta && (b // c1\n\t\t) // c2\n\t);\n}\n",
        "function f() {\n\treturn (\n\t\ta && b // c1\n\t\t// c2\n\t);\n}\n",
    ),
    (
        "function f() {\n\treturn (\n\t\ta && (b /* c1 */) // c2\n\t)\n}\n",
        "function f() {\n\treturn (\n\t\ta && b /* c1 */ // c2\n\t);\n}\n",
    ),
    // no pair around the argument: a block comment spanning lines in the last operand's own
    // pair opens the one the next pass reads it inside
    (
        "function f() {\n\treturn a && (b /* c1\n\tc2 */)\n}\n",
        "function f() {\n\treturn (\n\t\ta && b /* c1\n\tc2 */\n\t);\n}\n",
    ),
];

#[test]
fn comment_inside_the_pair_holds_it_in_one_pass() {
    assert_all_one_pass(HELD_PAIR_CASES);
}

/// A line comment the pair holds does not count toward the operand's width: an operand
/// that fits stays on one line however long the comment behind it runs.
#[test]
fn held_line_comment_does_not_break_a_fitting_operand() {
    let fixed_point = "function f() {\n\treturn (\n\t\taaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb // cccccccccccccccccccccccccccccccccccccccccccc\n\t);\n}\n";
    assert_one_pass(fixed_point, fixed_point);
    assert_one_pass(&fixed_point.replace(");", ")"), fixed_point);
}

/// The pair ends the held run's line itself, so a list around the statement keeps the
/// layout it takes with no comment there: a hugged callback stays hugged.
#[test]
fn held_run_does_not_break_an_enclosing_list() {
    assert_all_one_pass(&[
        (
            "f(a, () => {\n\treturn (\n\t\ta && b // c\n\t)\n})\n",
            "f(a, () => {\n\treturn (\n\t\ta && b // c\n\t);\n});\n",
        ),
        (
            "f(() => {\n\tthrow (\n\t\ta || b\n\t\t// c\n\t)\n}, 1)\n",
            "f(() => {\n\tthrow (\n\t\ta || b\n\t\t// c\n\t);\n}, 1);\n",
        ),
    ]);
}

/// The one run that does NOT hold the pair: single-line block comments trailing an
/// operand that fits. The pair is conditional on the operand breaking, so the comment
/// prints ahead of the `;` on the first pass and trails it on the second — the same two
/// steps from the semicolon-free authoring as from the terminated one, and no comment
/// lost on the way.
#[test]
fn fitting_same_line_block_converges_in_two_passes() {
    for (keyword, asi, terminated) in [
        (
            "return",
            "function f() {\n\treturn (a && b /* c */)\n}\n",
            "function f() {\n\treturn (a && b /* c */);\n}\n",
        ),
        (
            "throw",
            "function f() {\n\tthrow (\n\t\ta && b /* c */\n\t)\n}\n",
            "function f() {\n\tthrow (\n\t\ta && b /* c */\n\t);\n}\n",
        ),
    ] {
        let first = format(asi);
        assert_eq!(
            first,
            format!("function f() {{\n\t{keyword} a && b /* c */;\n}}\n"),
            "the first pass keeps the comment ahead of the `;` for {asi:?}"
        );
        assert_eq!(
            first,
            format(terminated),
            "both authorings take the same first pass"
        );
        let second = format(&first);
        assert_eq!(
            second,
            format!("function f() {{\n\t{keyword} a && b; /* c */\n}}\n"),
            "the second pass trails the `;` for {asi:?}"
        );
        assert_eq!(format(&second), second, "and that form is a fixed point");
        assert_eq!(
            comments(asi),
            comments(&second),
            "the comment survives for {asi:?}"
        );
    }
}

/// Assert every Svelte (authoring, fixed point) row is reached in one pass and stays there,
/// naming each row that does not. The fixed point holds each comment, so its text is the
/// comment check.
fn assert_all_svelte_one_pass(cases: &[(&str, &str)]) {
    let svelte = |source: &str| tsv_svelte::format_str(source).expect("format failed");
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|(authored, fixed_point)| {
            let once = svelte(authored);
            if once != *fixed_point {
                return Some(format!(
                    "one pass must reach the fixed point for {authored:?}: got {once:?}"
                ));
            }
            (svelte(&once) != *fixed_point)
                .then(|| format!("the second pass must stay at the fixed point for {authored:?}"))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} rows fail:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

/// The same pair inside a Svelte `<script>`: a function body and the script's own list.
#[test]
fn svelte_script_carriers_hold_the_pair() {
    assert_all_svelte_one_pass(&[
        (
            "<script>\n\tfunction f() {\n\t\treturn (\n\t\t\ta && b\n\t\t\t// c\n\t\t)\n\t}\n</script>\n",
            "<script>\n\tfunction f() {\n\t\treturn (\n\t\t\ta && b\n\t\t\t// c\n\t\t);\n\t}\n</script>\n",
        ),
        (
            "<script>\n\tthrow (\n\t\ta || b // c\n\t)\n</script>\n",
            "<script>\n\tthrow (\n\t\ta || b // c\n\t);\n</script>\n",
        ),
    ]);
}

/// A Svelte block head inside a whitespace-sensitive element flattens the soft breaks of
/// its expression and keeps the hard ones. The held pair is made of hard breaks, so it
/// prints there as it does everywhere else: open, the run inside it in the order written,
/// and a comment past the `)` behind the `;`.
#[test]
fn flattened_block_head_keeps_the_held_pair() {
    assert_all_svelte_one_pass(&[
        // a line comment inside the pair, a block comment past it
        (
            "<pre>{#if xs.some((x) => {\n\treturn (\n\t\tx && b // c1\n\t); /* c2 */\n})}a{/if}</pre>\n",
            "<pre>{#if xs.some((x) => {\n\t\treturn (\n\t\t\tx && b // c1\n\t\t); /* c2 */\n\t})}a{/if}</pre>\n",
        ),
        // a block comment spanning lines
        (
            "<pre>{#if xs.some((x) => {\n\treturn (\n\t\tx && b /* c1\n\t\tc2 */\n\t);\n})}a{/if}</pre>\n",
            "<pre>{#if xs.some((x) => {\n\t\treturn (\n\t\t\tx && b /* c1\n\t\tc2 */\n\t\t);\n\t})}a{/if}</pre>\n",
        ),
        // a line comment alone, with a `;` …
        (
            "<pre>{#if xs.some((x) => {\n\treturn (\n\t\tx && b // c1\n\t);\n})}a{/if}</pre>\n",
            "<pre>{#if xs.some((x) => {\n\t\treturn (\n\t\t\tx && b // c1\n\t\t);\n\t})}a{/if}</pre>\n",
        ),
        // … and under ASI
        (
            "<pre>{#if xs.some((x) => {\n\treturn (\n\t\tx && b // c1\n\t)\n})}a{/if}</pre>\n",
            "<pre>{#if xs.some((x) => {\n\t\treturn (\n\t\t\tx && b // c1\n\t\t);\n\t})}a{/if}</pre>\n",
        ),
        // `throw`, a comment on a line of its own
        (
            "<pre>{#if xs.some((x) => {\n\tthrow (\n\t\tx && b\n\t\t// c1\n\t);\n})}a{/if}</pre>\n",
            "<pre>{#if xs.some((x) => {\n\t\tthrow (\n\t\t\tx && b\n\t\t\t// c1\n\t\t);\n\t})}a{/if}</pre>\n",
        ),
    ]);
}
