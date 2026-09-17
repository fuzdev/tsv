// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! An at-rule's wire `name` is half-decoded the way every selector name is, because
//! `parseCss` reads both with the same `read_identifier`: a hex escape decodes to its
//! code point (`@a\41 b` → `aAb`), a hex escape of a backslash is spelled `\\`
//! (`@a\5c b` → `a\\b`), and an identity escape keeps its backslash (`@a\?b` → `a\?b`).
//! The internal AST's `name` stays fully decoded; only the public form changes.
//!
//! Not a fixture: prettier splits an escaped at-rule name into a name and a prelude
//! (`@a\5c b` → `@a \5c b`), so no escaped name is a prettier fixed point an `input.*`
//! could hold. The expected strings are `parseCss`'s, probed with `tsv_debug
//! canonical_parse --parser css`.

/// The wire `name` of the stylesheet's first at-rule.
fn atrule_name(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let stylesheet = tsv_css::parse(source, &arena).expect("parse failed");
    let wire = tsv_debug::json::wire_value(&tsv_css::convert_ast_json_bytes(&stylesheet, source));
    let node = &wire["children"][0];
    assert_eq!(node["type"], "Atrule", "expected an at-rule");
    node["name"].as_str().expect("name").to_owned()
}

#[test]
fn hex_escape_decodes() {
    assert_eq!(atrule_name("@a\\41 b;"), "aAb");
    assert_eq!(atrule_name("@\\6d edia screen {}"), "media");
    assert_eq!(atrule_name("@m\\A x {}"), "m\nx");
}

#[test]
fn hex_escaped_backslash_is_spelled_as_an_escape() {
    assert_eq!(atrule_name("@a\\5c b;"), "a\\\\b");
    assert_eq!(atrule_name("@a\\00005C b;"), "a\\\\b");
}

#[test]
fn identity_escape_keeps_its_backslash() {
    assert_eq!(atrule_name("@a\\?b;"), "a\\?b");
    assert_eq!(atrule_name("@a\\\\b;"), "a\\\\b");
}
