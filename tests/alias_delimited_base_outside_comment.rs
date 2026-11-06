// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A type alias whose value opens on a type literal, mapped type or tuple keeps that
//! delimiter on the `=` line when every comment in the value sits inside it — the
//! `types/aliases/rhs_delimited_base_comment` fixtures. A comment anywhere else in the value
//! is not the delimiter's to lay out, so the value keeps the break after the `=`. The
//! spellings pinned here — a `//` in the index brackets, or ahead of the base — are ones a
//! fixture cannot carry: prettier relocates the first past the `;` and is not idempotent on
//! it, and prints the second flush at the operator's own indent.

fn format(source: &str) -> String {
    tsv_ts::format(source).expect("formats")
}

/// Each pair is (authoring, the single-pass fixed point).
const CASES: &[(&str, &str)] = &[
    (
        "type A = {\n\ta: B;\n}[// c\n\tkeyof B];\n",
        "type A =\n\t{\n\t\ta: B;\n\t}[// c\n\tkeyof B];\n",
    ),
    (
        "type A = {\n\ta: B;\n\t// c1\n}[// c2\n\tkeyof B];\n",
        "type A =\n\t{\n\t\ta: B;\n\t\t// c1\n\t}[// c2\n\tkeyof B];\n",
    ),
    // a comment that hangs what follows it AHEAD of the base, inside a required paren pair
    (
        "type A = (keyof // c1\n{\n\ta: B;\n\t// c2\n})[K];\n",
        "type A =\n\t(keyof // c1\n\t\t{\n\t\t\ta: B;\n\t\t\t// c2\n\t\t})[K];\n",
    ),
];

#[test]
fn a_comment_outside_the_delimited_base_keeps_the_break_after_the_eq() {
    for (authored, fixed_point) in CASES {
        let once = format(authored);
        assert_eq!(
            once, *fixed_point,
            "one pass must reach the fixed point for {authored:?}"
        );
        assert_eq!(format(&once), once, "not idempotent for {authored:?}");
    }
}
