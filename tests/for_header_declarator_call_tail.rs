// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! A `for` header's own declarator is a `VariableDeclarator` to prettier's
//! `printMemberExpression` `shouldInline` (member.js), so its initializer takes the
//! call-object clause exactly as a statement-level declarator does: a lone `.prop` off a
//! call WITH ARGUMENTS carries no break point, and the width sheds into the call's
//! arguments instead of dropping the lookup to a line of its own.
//!
//! Not a fixture, and the reason is a *neighbouring* difference rather than this rule. A
//! for-header declarator's `=` is a flat concat — `build_for_init_doc` never applies the
//! assignment layout its statement-level twin gets — so prettier breaks after the `=`
//! (`let a =⏎\tf(…)`) where tsv never does. Any over-width initializer shows it, whatever
//! the value is, so no input in this position can be a `tests/fixtures` entry: tsv's
//! output is not prettier's. What IS pinnable is the clause's own effect, which is the
//! *inner* shape both formatters agree on — the arguments break and `.prop` rides the
//! `)`. That is what these tests assert.
//!
//! The statement-level and object/class/parameter positions ARE fixtured, at
//! `tests/fixtures/typescript/expressions/member/call_base_lone_tail_long/`.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

/// 90 `d`s: long enough that the header must break and the initializer cannot fit on the
/// clause line, short enough that no output line runs past print width.
fn prop() -> String {
    "d".repeat(90)
}

/// Whether the lookup got a break point — some line is exactly the lookup. Asked this way
/// rather than by matching an indent prefix, so the assertion stays about the break point
/// when the header's own depth changes.
fn lookup_owns_a_line(out: &str) -> bool {
    let lookup = format!(".{};", prop());
    out.lines().any(|line| line.trim_start() == lookup)
}

/// The clause fires: the call has an argument and the tail is one plain lookup, so the
/// argument list is the break point and `.prop` stays welded to the `)`.
#[test]
fn for_header_declarator_glues_a_lone_tail_off_a_call_with_arguments() {
    let out = format(&format!("for (let a = fnn(arg).{}; ; ) g();\n", prop()));
    assert!(
        out.contains(&format!(").{};", prop())),
        "the lookup must ride the closing paren: {out}"
    );
    assert!(
        out.contains("fnn(\n"),
        "the argument list must be the break point: {out}"
    );
    assert!(
        !lookup_owns_a_line(&out),
        "the lookup must NOT have a break point of its own: {out}"
    );
}

/// The same header with a **zero-argument** call is outside the clause
/// (`isCallExpressionWithArguments`), so the lookup keeps its break point — the control
/// that makes the assertion above about the clause rather than about the position.
#[test]
fn for_header_declarator_keeps_the_break_point_off_an_argumentless_call() {
    let out = format(&format!("for (let a = fnn().{}; ; ) g();\n", prop()));
    assert!(
        lookup_owns_a_line(&out),
        "the lookup must drop to its own line: {out}"
    );
}

/// A **two-lookup** tail is outside the clause too: the last lookup's object is a member,
/// not the call. Second control, on the other half of the shape test.
#[test]
fn for_header_declarator_keeps_the_break_point_on_a_two_lookup_tail() {
    let out = format(&format!("for (let a = fnn(arg).bb.{}; ; ) g();\n", prop()));
    assert!(
        lookup_owns_a_line(&out),
        "the last lookup must drop to its own line: {out}"
    );
}
