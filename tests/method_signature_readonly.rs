// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A `readonly` modifier on a type member: carried by a method signature as by a property
//! or index signature, printed with the comments around it, and a modifier only when its
//! member follows on the same line (`Parser::parse_type_element`; the fixture claims are
//! `types/method_signature_readonly_svelte_prettier_divergence` and
//! `types/readonly_member_name_line_break`). What is pinned here is what those fixtures
//! cannot carry: comment placements in the modifier's gap, the members `readonly` turns
//! into methods, and the wire field.
//!
//! Every case is a fixed point whose reparse keeps the program the source wrote.

use serde_json::Value;

/// The span-free wire of `source`, so a reparse must keep the same program.
fn tree(source: &str) -> Value {
    fn strip(node: &mut Value) {
        match node {
            Value::Object(map) => {
                for key in ["start", "end", "loc", "name_loc"] {
                    map.remove(key);
                }
                map.values_mut().for_each(strip);
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("the case parses");
    let mut value = tsv_debug::json::wire_value(&tsv_ts::convert_ast_json_bytes(&program, source));
    strip(&mut value);
    value
}

/// `source` formatted, asserted a fixed point whose reparse is the program `source` wrote.
#[track_caller]
fn stable(source: &str) -> String {
    let output = tsv_ts::format(source).expect("the case formats");
    assert_eq!(
        tsv_ts::format(&output).expect("the output formats"),
        output,
        "not a fixed point: {source}"
    );
    assert_eq!(tree(source), tree(&output), "the program changed: {source}");
    output.trim_end().to_owned()
}

/// The members of the one interface `source` declares.
fn members(source: &str) -> Vec<Value> {
    let value = tree(source);
    value["body"][0]["body"]["body"]
        .as_array()
        .expect("an interface body")
        .clone()
}

#[test]
fn the_modifier_keeps_the_comments_around_it() {
    for (source, expected) in [
        (
            "interface I { readonly /* c */ x(): void }",
            "interface I {\n\treadonly /* c */ x(): void;\n}",
        ),
        (
            "interface I { /* a */ readonly /* b */ [k](): void }",
            "interface I {\n\t/* a */ readonly /* b */ [k](): void;\n}",
        ),
        (
            "type T = { readonly /* c */ x?(): void };",
            "type T = { readonly /* c */ x?(): void };",
        ),
        // An accessor signature is an invalid program to tsc's parser, which tsv defers:
        // the modifier is kept with the rest.
        (
            "interface I { readonly /* a */ get /* b */ x(): T }",
            "interface I {\n\treadonly /* a */ get /* b */ x(): T;\n}",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
    }
}

#[test]
fn readonly_new_is_a_method_named_new() {
    // tsc decides a construct signature at the member's first token, so behind the
    // modifier `new` is a method's name.
    assert_eq!(
        stable("interface I { readonly new (): T }"),
        "interface I {\n\treadonly new(): T;\n}"
    );
    let method = &members("interface I { readonly new (): T }")[0];
    assert_eq!(method["type"], "TSMethodSignature");
    assert_eq!(method["key"]["name"], "new");
    // Without it, the construct signature.
    assert_eq!(
        members("interface I { new (): T }")[0]["type"],
        "TSConstructSignatureDeclaration"
    );
}

#[test]
fn a_line_break_ends_the_modifier() {
    // A comment holding a line break is a line break.
    for (source, expected) in [
        (
            "interface I { readonly\nx(): void }",
            "interface I {\n\treadonly;\n\tx(): void;\n}",
        ),
        (
            "interface I { readonly /* c\n */ x(): void }",
            "interface I {\n\treadonly;\n\t/* c\n\t */ x(): void;\n}",
        ),
        (
            "interface I { readonly // c\nx: T }",
            "interface I {\n\treadonly; // c\n\tx: T;\n}",
        ),
    ] {
        assert_eq!(stable(source), expected, "{source}");
        assert_eq!(members(source).len(), 2, "{source}");
    }
}

#[test]
fn the_wire_carries_the_modifier_only_when_written() {
    let members = members("interface I { readonly x(): void; y(): void }");
    assert_eq!(members[0]["readonly"], true);
    assert!(members[1].get("readonly").is_none());
}
