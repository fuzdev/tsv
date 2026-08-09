//! `with` is a `ReservedWord`, not a contextual keyword — it can never be a name.
//!
//! tsv is **strict mode only**, and the `with` statement is one of the two lexically
//! sloppy-mode constructs it rejects outright (the other is the legacy octal literal).
//! But the rejection has to come from the *word*, not from a downstream parse accident:
//! with `with` left as a plain `Identifier`, `with (a);` reads as a CALL to a function
//! named `with` and formats to `with(a);`, so a sloppy-mode program is silently
//! reinterpreted rather than refused, and `var with = 1` / `x = with` / `function
//! f(with) {}` all parse. `Identifier : IdentifierName but not ReservedWord` excludes
//! the word at the *production* level — this is not a deferrable early error, and acorn
//! rejects every name use below.
//!
//! The word survives in the positions where the grammar spells it out or where any
//! `IdentifierName` is allowed: the import-attributes clause (`with { type: 'json' }`),
//! a property key, a member access after `.`, a method name, a type member. Those are
//! the reason the lexer left it an identifier in the first place, so they carry the
//! regression risk and are asserted here alongside the rejections.

fn accepts(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_ts::parse(source, &arena).is_ok()
}

fn check(cases: &[(&str, bool)]) {
    let mut failures = Vec::new();
    for (source, want) in cases {
        let got = accepts(source);
        if got != *want {
            let verb = if *want {
                "should ACCEPT"
            } else {
                "should REJECT"
            };
            failures.push(format!("{source}\n    -> {verb}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} shapes have the wrong verdict:\n  {}",
        failures.len(),
        cases.len(),
        failures.join("\n  ")
    );
}

/// The statement itself, in every body shape — including the degenerate empty body,
/// which is the one that currently slips through as a call expression.
#[test]
fn with_statement_is_rejected() {
    check(&[
        ("with (a);", false),
        ("with (a) b;", false),
        ("with (a) { b; }", false),
        ("with (a.b) {}", false),
        ("if (c) with (a) {}", false),
        ("function f() { with (a) {} }", false),
    ]);
}

/// Every name channel: a reserved word is barred by a production, so all three reject.
#[test]
fn with_is_not_a_name() {
    check(&[
        // BindingIdentifier
        ("var with = 1;", false),
        ("let with = 1;", false),
        ("function with() {}", false),
        ("function f(with) {}", false),
        ("class with {}", false),
        ("try {} catch (with) {}", false),
        ("import with from 'm';", false),
        // IdentifierReference
        ("x = with;", false),
        ("with.x;", false),
        ("with();", false),
        ("typeof with;", false),
        ("const o = { with };", false),
        // LabelIdentifier
        ("with: for (;;) break with;", false),
    ]);
}

/// The contextual positions the word must keep — an `IdentifierName` slot, or the
/// import-attributes clause the grammar spells out. These are why `with` was left
/// unlexed as a keyword, so they are the regression surface for making it one.
#[test]
fn with_survives_in_identifier_name_positions() {
    check(&[
        ("import x from 'm' with { type: 'json' };", true),
        ("import 'm' with { type: 'json' };", true),
        // A re-export names ANOTHER module's binding, so both sides of the specifier
        // are `ModuleExportName` — an `IdentifierName`, reserved words included.
        ("export { with } from 'm';", true),
        ("export { with as w } from 'm';", true),
        ("import { with as w } from 'm';", true),
        ("export * from 'm' with { type: 'json' };", true),
        ("export { a } from 'm' with { type: 'json' };", true),
        ("import('m', { with: { type: 'json' } });", true),
        ("const o = { with: 1 };", true),
        ("o.with;", true),
        ("o?.with;", true),
        ("o.with();", true),
        ("class C { with() {} }", true),
        ("class C { static with = 1; }", true),
        ("interface I { with(): void }", true),
        ("type T = { with: number };", true),
        ("enum E { with }", true),
        ("const o = { with: 1 }.with;", true),
    ]);
}
