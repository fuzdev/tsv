// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Node-type pins for the `[no LineTerminator here]` restriction on a class
//! modifier keyword — which keywords carry one and which do not.
//!
//! Two questions, one rule, and tsv answers both with tsc:
//!
//! * `export default abstract⏎class Base {}` — `abstract` binds to `class` only on
//!   its own line (tsc's `nextTokenIsClassKeywordOnSameLine`, reached from
//!   `nextTokenCanFollowDefaultKeyword`), so a break demotes it to the
//!   default-exported *expression* and the class becomes its own statement: two
//!   statements, not one exported abstract class.
//! * `class C { static⏎c = 3 }` — `static` carries **no** such restriction.
//!   ecma262's `ClassElement : static FieldDefinition `;`` has no
//!   `[no LineTerminator here]` (sec-class-definitions), and tsc gives `static` its
//!   own arm in `nextTokenCanFollowModifier` for exactly that reason, skipping the
//!   `nextTokenIsOnSameLineAndCanFollowModifier` its contextual-modifier siblings
//!   take: one member carrying the modifier, not two.
//!
//! acorn-typescript is wrong on both, in opposite directions — welding where the
//! restriction exists, splitting where it does not — so neither reading can be
//! pinned as a fixture `expected.json`. Nor can either ride a fixture divergence
//! marker: both are reachable only from an input that is nobody's fixed point (both
//! formatters normalize the line break away), and a variant tsv parses differently
//! from the canonical parser has no pin shape (`docs/fixture_overview.md` P4). So
//! the *formatting* claim rides the `unformatted_asi` variant of
//! [abstract/export_default_line_break](../tests/fixtures/typescript/declarations/class/abstract/export_default_line_break/)
//! and [class/modifier_line_break](../tests/fixtures/typescript/declarations/class/modifier_line_break/),
//! and this file asserts the trees directly — the same split
//! [arrow_consequent_return_type.rs](./arrow_consequent_return_type.rs) makes for
//! its own acorn divergence. Every expectation here was graded against tsc's live
//! parser (`createSourceFile(...).parseDiagnostics` and its tree) before it was
//! written down, and prettier's own `js/classes/multiple-static.js` snapshot pins
//! the `static` reading a third time. Cataloged in `docs/conformance_svelte.md`
//! §TypeScript Corrections.

use serde_json::Value;

fn parse_json(source: &str) -> Value {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_debug::json::wire_value(&tsv_ts::convert_ast_json_bytes(&program, source))
}

fn accepts(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_ts::parse(source, &arena).is_ok()
}

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

/// Stands in for a string the wire does not carry, so a wrong shape fails the assertion
/// that names the expectation rather than panicking somewhere in a helper.
const NONE: &str = "<none>";

/// The string at `pointer`, or [`NONE`] where nothing is.
fn str_at(json: &Value, pointer: &str) -> String {
    json.pointer(pointer)
        .and_then(Value::as_str)
        .map_or_else(|| NONE.to_owned(), str::to_owned)
}

/// A node object's `type`.
fn type_of(node: &Value) -> String {
    str_at(node, "/type")
}

/// The node array at `pointer`, which `what` names for the failure message.
fn nodes_at<'a>(json: &'a Value, pointer: &str, what: &'static str) -> &'a [Value] {
    json.pointer(pointer).and_then(Value::as_array).expect(what)
}

/// The top-level statement types of `source`.
fn statement_types(source: &str) -> Vec<String> {
    nodes_at(&parse_json(source), "/body", "a program body")
        .iter()
        .map(type_of)
        .collect()
}

/// The sole class declaration's members, as `(type, key name, static)` triples.
fn class_members(source: &str) -> Vec<(String, String, bool)> {
    nodes_at(&parse_json(source), "/body/0/body/body", "a class body")
        .iter()
        .map(|m| {
            (
                type_of(m),
                str_at(m, "/key/name"),
                m.get("static").and_then(Value::as_bool).unwrap_or(false),
            )
        })
        .collect()
}

/// `export default abstract⏎class` is two statements: the break demotes `abstract`
/// to the exported expression, leaving the class its own declaration. Falling
/// through to the expression arm rather than rejecting is what also keeps the bare
/// `export default abstract;` parseable.
#[test]
fn export_default_abstract_line_break_is_two_statements() {
    let broken = "export default abstract\nclass Base {}\n";
    assert_eq!(
        statement_types(broken),
        vec!["ExportDefaultDeclaration", "ClassDeclaration"],
        "a break after `abstract` must not weld into one exported abstract class"
    );
    assert_eq!(
        str_at(&parse_json(broken), "/body/0/declaration/type"),
        "Identifier",
        "the demoted `abstract` is the default-exported expression"
    );

    // The same-line spelling is the one abstract class, and the bare default export
    // of the identifier parses on its own.
    let same_line = "export default abstract class Base {}\n";
    assert_eq!(statement_types(same_line), vec!["ExportDefaultDeclaration"]);
    assert_eq!(
        str_at(&parse_json(same_line), "/body/0/declaration/type"),
        "ClassDeclaration"
    );
    assert!(accepts("export default abstract;\n"));

    // A comment carrying a line terminator is a line terminator (ecma262
    // sec-comments), so it demotes exactly as a raw break does — while a
    // single-line one leaves the binding intact.
    assert_eq!(
        statement_types("export default abstract /* c */ class Base {}\n"),
        vec!["ExportDefaultDeclaration"]
    );
    assert_eq!(
        statement_types("export default abstract /* c\n*/ class Base {}\n"),
        vec!["ExportDefaultDeclaration", "ClassDeclaration"]
    );
}

/// `static` alone among the class-member modifiers carries no
/// `[no LineTerminator here]`, so it binds across a break where every contextual
/// sibling demotes to a field of its own name.
#[test]
fn static_binds_across_a_line_break_where_its_siblings_demote() {
    assert_eq!(
        class_members("class C {\n\tstatic\n\tc = 3\n}\n"),
        vec![("PropertyDefinition".to_owned(), "c".to_owned(), true)],
        "`static` must stay a modifier across the break"
    );
    assert_eq!(
        class_members("class C {\n\tstatic\n\tm() {}\n}\n"),
        vec![("MethodDefinition".to_owned(), "m".to_owned(), true)]
    );

    // prettier's own js/classes/multiple-static.js: the first `static` is the
    // modifier of a static field *named* `static`, the third the modifier of the
    // method. Two members, both static.
    assert_eq!(
        class_members("class C {\n\tstatic\n\tstatic\n\tstatic\n\ta() {}\n}\n"),
        vec![
            ("PropertyDefinition".to_owned(), "static".to_owned(), true),
            ("MethodDefinition".to_owned(), "a".to_owned(), true),
        ]
    );

    // Every restricted sibling demotes instead: two members, the modifier word
    // becoming a bodiless field of its own name via ASI.
    for modifier in [
        "readonly",
        "public",
        "private",
        "protected",
        "declare",
        "override",
        "abstract",
        "accessor",
        "async",
    ] {
        let members = class_members(&format!("class C {{\n\t{modifier}\n\tx = 1\n}}\n"));
        assert_eq!(
            members,
            vec![
                (
                    "PropertyDefinition".to_owned(),
                    (*modifier).to_owned(),
                    false
                ),
                ("PropertyDefinition".to_owned(), "x".to_owned(), false),
            ],
            "`{modifier}` must demote across a line break"
        );
    }
}

/// Both fixtures' formatting claim, on the standalone-TS path: the variant
/// normalizes to the form the fixture holds as `input`, which is what makes the
/// AST difference unreachable from any fixed point.
#[test]
fn the_line_broken_spellings_normalize_to_their_fixed_points() {
    assert_eq!(
        format("export default abstract\nclass Base {}\n"),
        "export default abstract;\nclass Base {}\n"
    );
    assert_eq!(
        format("class C {\n\tstatic\n\tc = 3\n}\n"),
        "class C {\n\tstatic c = 3;\n}\n"
    );
    assert_eq!(
        format("class C {\n\tstatic\n\tstatic\n\tstatic\n\ta() {}\n}\n"),
        "class C {\n\tstatic static;\n\tstatic a() {}\n}\n"
    );
}
