//! A binding pattern admits only a `BindingIdentifier` or a nested pattern as a target
//! (ecma262 `BindingElement`, `SingleNameBinding`, `BindingRestElement`), so a
//! parenthesized or member target in one is a syntax error. An ASSIGNMENT pattern
//! nested in a binding element's default is not a binding pattern, though, and keeps
//! both. The fixtures `typescript/expressions/destructuring/paren_target` and
//! `member_target` pin the family.
//!
//! What stays here is the one spelling no fixture can hold: an assignment pattern with a
//! parenthesized target as a PARAMETER's default, written without parens around the
//! assignment (`function f(a = [(b)] = x) {}`). tsc and acorn accept it, but prettier's
//! parser throws ("Invalid parenthesized assignment pattern"), so neither a formatted
//! input nor an `unformatted_*` variant can carry it. These tests pin the parse, the node
//! shape, and the reprint.

use serde_json::Value;

/// The wire JSON of `source`, or the parse error.
fn parse_json(source: &str) -> Result<Value, tsv_lang::ParseError> {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena)?;
    Ok(tsv_debug::json::wire_value(
        &tsv_ts::convert_ast_json_bytes(&program, source),
    ))
}

/// The formatted output of `source`, or the parse error.
fn format(source: &str) -> Result<String, tsv_lang::ParseError> {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena)?;
    Ok(tsv_ts::format(&program, source))
}

/// Assert that the node at `default` is an `AssignmentExpression` whose left is an
/// `ArrayPattern` holding the bare identifier `b` — the grouping paren dropped, the
/// assignment kept.
fn assert_default_is_assignment_pattern(json: &Value, default: &str) {
    let at = |path: &str| {
        json.pointer(&format!("{default}{path}"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    assert_eq!(
        at("/type").as_deref(),
        Some("AssignmentExpression"),
        "{json}"
    );
    assert_eq!(at("/left/type").as_deref(), Some("ArrayPattern"), "{json}");
    assert_eq!(
        at("/left/elements/0/type").as_deref(),
        Some("Identifier"),
        "{json}"
    );
    assert_eq!(at("/left/elements/0/name").as_deref(), Some("b"), "{json}");
}

#[test]
fn function_param_default_assignment_pattern_keeps_its_paren_target() {
    let source = "function f(a = [(b)] = x) {}";
    let json = parse_json(source).expect("tsc and acorn accept it");
    assert_default_is_assignment_pattern(&json, "/body/0/params/0/right");
    assert_eq!(
        format(source).expect("parses"),
        "function f(a = ([b] = x)) {}\n"
    );
}

#[test]
fn arrow_param_element_default_assignment_pattern_keeps_its_paren_target() {
    let source = "([a = [(b)] = x]) => 1;";
    let json = parse_json(source).expect("tsc and acorn accept it");
    assert_default_is_assignment_pattern(&json, "/body/0/expression/params/0/elements/0/right");
    assert_eq!(format(source).expect("parses"), "([a = ([b] = x)]) => 1;\n");
}
