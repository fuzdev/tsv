// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A member chain whose callbacks have EMPTY bodies decides its layout from what prints,
//! not from the authored lines inside the braces: an empty body prints `{}` however it
//! was written, so reading a newline there expanded the chain on one pass and joined it
//! on the next. The spellings pinned here are the ones a fixture cannot carry — a CRLF
//! inside the braces (folded before the parse, so the fold and the decision must agree)
//! and a closing brace on its own line ahead of the call's `)` — beside the plain
//! `{⏎}` spelling the `calls/chained/callback_empty_block*` fixtures pin. A body of
//! bare `;`s prints `{}` too, so it reads as empty however it is written.

fn format(source: &str) -> String {
    tsv_ts::format(source).expect("formats")
}

/// Each pair is (authoring, the single-pass fixed point).
const CASES: &[(&str, &str)] = &[
    (
        "promise.then(() => {\n}).catch(() => {\n});\n",
        "promise.then(() => {}).catch(() => {});\n",
    ),
    (
        "promise.then(() => {\r\n}).catch(() => {\r\n});\r\n",
        "promise.then(() => {}).catch(() => {});\n",
    ),
    (
        "promise.then(() => {\n}\n).catch(() => {\n}\n);\n",
        "promise.then(() => {}).catch(() => {});\n",
    ),
    (
        "promise.then(function () {\n}).catch(function () {\n});\n",
        "promise.then(function () {}).catch(function () {});\n",
    ),
    (
        "promise.then(() => { ; }).catch(() => {});\n",
        "promise.then(() => {}).catch(() => {});\n",
    ),
    (
        "promise.then(function () { ; ; }).catch(function () {});\n",
        "promise.then(function () {}).catch(function () {});\n",
    ),
    // A comment inside an empty body prints on its own line, so the chain breaks.
    (
        "promise.then(() => {\n// c\n}).catch(() => {});\n",
        "promise\n\t.then(() => {\n\t\t// c\n\t})\n\t.catch(() => {});\n",
    ),
];

#[test]
fn empty_callback_bodies_reach_the_fixed_point_in_one_pass() {
    for (authored, fixed_point) in CASES {
        let once = format(authored);
        assert_eq!(
            once, *fixed_point,
            "one pass must reach the fixed point for {authored:?}"
        );
        assert_eq!(format(&once), once, "not idempotent for {authored:?}");
    }
}
