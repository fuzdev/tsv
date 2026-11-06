// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A `case` block followed by more statements takes its own line below the label, and a
//! run of block comments glued between the `:` and its `{` goes down with it — the
//! `statements/switch/case_block_then_statement` fixture. A comment the author broke after
//! is not glued to the block, so it stays trailing the label, and in a run only the
//! comments glued all the way to the `{` count as glued. The first two spellings pinned here
//! are ones a fixture cannot carry: prettier relocates a label-trailing comment across the
//! `:` (`case 1 /* a */:`).

fn format(source: &str) -> String {
    tsv_ts::format(source).expect("formats")
}

/// Each pair is (authoring, the single-pass fixed point).
const CASES: &[(&str, &str)] = &[
    (
        "switch (a) {\n\tcase 1: /* a */\n\t\t{\n\t\t\tf();\n\t\t}\n\t\tbreak;\n}\n",
        "switch (a) {\n\tcase 1: /* a */\n\t\t{\n\t\t\tf();\n\t\t}\n\t\tbreak;\n}\n",
    ),
    (
        "switch (a) {\n\tcase 1: /* a */\n\t/* b */ {\n\t\tf();\n\t}\n\tbreak;\n}\n",
        "switch (a) {\n\tcase 1: /* a */\n\t\t/* b */ {\n\t\t\tf();\n\t\t}\n\t\tbreak;\n}\n",
    ),
    // a multi-line block's own line breaks are not a break after it: glued to the `{`, it
    // goes down with the block, as prettier prints it
    (
        "switch (a) {\n\tcase 1: /* a\nb */ {\n\t\tf();\n\t}\n\tbreak;\n}\n",
        "switch (a) {\n\tcase 1:\n\t\t/* a\nb */ {\n\t\t\tf();\n\t\t}\n\t\tbreak;\n}\n",
    ),
];

#[test]
fn only_a_run_glued_to_the_brace_leaves_the_label_with_the_block() {
    for (authored, fixed_point) in CASES {
        let once = format(authored);
        assert_eq!(
            once, *fixed_point,
            "one pass must reach the fixed point for {authored:?}"
        );
        assert_eq!(format(&once), once, "not idempotent for {authored:?}");
    }
}
