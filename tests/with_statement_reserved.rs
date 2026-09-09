//! `with` is a `ReservedWord`, not a contextual keyword — it can never be a name.
//!
//! The `with` STATEMENT is sloppy-mode Script code: strict code disallows it by an early
//! error keyed on `IsStrict`, so tsv parses it at `Goal::Script` unless a `"use strict"`
//! prologue is in force, and rejects it under `Goal::Module` (always strict). That is a
//! verdict about the statement, not about the word: `with` is barred from every NAME
//! position in every mode, and the bar has to come from the *word* rather than from a
//! downstream parse accident. With `with` left as a plain `Identifier`, `with (a);` reads
//! as a CALL to a function named `with` and formats to `with(a);`, so a sloppy-mode
//! program is silently reinterpreted rather than parsed, and `var with = 1` / `x = with` /
//! `function f(with) {}` all parse. `Identifier : IdentifierName but not ReservedWord`
//! excludes the word at the *production* level — not a deferrable early error, and acorn
//! rejects every name use below at both goals.
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

fn accepts_script(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_ts::parse_with_goal(source, tsv_ts::Goal::Script, &arena).is_ok()
}

fn check_with(accept: fn(&str) -> bool, cases: &[(&str, bool)]) {
    let mut failures = Vec::new();
    for (source, want) in cases {
        let got = accept(source);
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

fn check(cases: &[(&str, bool)]) {
    check_with(accepts, cases);
}

fn check_script(cases: &[(&str, bool)]) {
    check_with(accepts_script, cases);
}

/// The statement itself, in every body shape — including the degenerate empty body,
/// which is the one that would otherwise slip through as a call expression. Module code
/// is strict by definition, so every shape rejects at the default goal.
#[test]
fn with_statement_is_rejected_at_module_goal() {
    check(&[
        ("with (a);", false),
        ("with (a) b;", false),
        ("with (a) { b; }", false),
        ("with (a.b) {}", false),
        ("if (c) with (a) {}", false),
        ("function f() { with (a) {} }", false),
    ]);
}

/// The same shapes at `Goal::Script` with no directive: sloppy code, so every one parses.
/// A `"use strict"` prologue — at the Program or in the enclosing function body — turns
/// the verdict back over, and a class body is strict without any directive at all.
#[test]
fn with_statement_is_accepted_in_sloppy_script() {
    check_script(&[
        ("with (a);", true),
        ("with (a) b;", true),
        ("with (a) { b; }", true),
        ("with (a.b) {}", true),
        ("if (c) with (a) {}", true),
        ("function f() { with (a) {} }", true),
        ("with (a) with (b) c;", true),
        ("l: with (a) b;", true),
        // strict again, by directive or by class
        ("\"use strict\"; with (a) {}", false),
        ("'use strict'; with (a) {}", false),
        ("function f() { \"use strict\"; with (a) {} }", false),
        ("class C { m() { with (a) {} } }", false),
        ("class C { static { with (a) {} } }", false),
    ]);
}

/// Every name channel: a reserved word is barred by a production, so all three reject —
/// under every mode, which is why the sloppy-Script half is asserted beside the Module one
/// at the end of this test.
#[test]
fn with_is_not_a_name() {
    let cases: &[(&str, bool)] = &[
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
    ];
    check(cases);
    check_script(cases);
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
