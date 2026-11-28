// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A class member's modifiers may be written in **any order** and **repeated**, as tsc's
//! parser reads them: `parseModifiers` collects any run of modifier keywords, and both the
//! ordering rule (TS1029 `'static' modifier must precede 'readonly' modifier`) and the
//! repeat rule (TS1028 `Accessibility modifier already seen`, TS1030 `'readonly' modifier
//! already seen`) are its grammar checker's, so tsv defers both. Each modifier is a flag
//! on the member (the wire's `accessibility` / `static` / `readonly` / … keys), so a repeat
//! folds into it and the printer prints every modifier once, in the canonical order.
//!
//! The reorders and the repeats prettier formats ride the `unformatted_*` variants of
//! `typescript/declarations/class/member_modifier_order`; acorn-typescript rejects every
//! repeat and several orders, so what lives here is what no fixture can hold: the
//! **accessibility** repeat, which prettier's `typescript` parser rejects too
//! (`Accessibility modifier already seen`), the boundaries tsc's parser draws, and the
//! name guard. Every expectation was graded against tsc's live parser
//! (`createSourceFile(...).parseDiagnostics` and its tree). Cataloged in
//! `docs/conformance_svelte.md` §TypeScript Corrections.

use serde_json::Value;

fn parse_json(source: &str) -> Value {
    let program = tsv_ts::__private::parse(source).expect("parse failed");
    tsv_debug::json::wire_value(&tsv_ts::__private::convert_ast_json_bytes(&program, source))
}

fn format(source: &str) -> String {
    let program = tsv_ts::__private::parse(source).expect("parse failed");
    tsv_ts::__private::format_ast(&program, source)
}

/// The first class member's wire object.
fn first_member(source: &str) -> Value {
    parse_json(source)
        .pointer("/body/0/body/body/0")
        .cloned()
        .expect("a first class member")
}

/// A repeated accessibility modifier folds into the one `accessibility` the wire holds,
/// and prints once — the reading tsc's parser takes (TS1028 is its checker's).
#[test]
fn repeated_accessibility_folds() {
    for (source, expected, accessibility) in [
        (
            "class C {\n\tpublic public foo = 1;\n}\n",
            "class C {\n\tpublic foo = 1;\n}\n",
            "public",
        ),
        (
            "class C {\n\tprivate static private x = 1;\n}\n",
            "class C {\n\tprivate static x = 1;\n}\n",
            "private",
        ),
        (
            "class C {\n\tprotected readonly protected x = 1;\n}\n",
            "class C {\n\tprotected readonly x = 1;\n}\n",
            "protected",
        ),
        (
            "class C {\n\tpublic public m() {}\n}\n",
            "class C {\n\tpublic m() {}\n}\n",
            "public",
        ),
        (
            "class C {\n\tpublic public get g() {\n\t\treturn 1;\n\t}\n}\n",
            "class C {\n\tpublic get g() {\n\t\treturn 1;\n\t}\n}\n",
            "public",
        ),
    ] {
        assert_eq!(
            first_member(source).get("accessibility"),
            Some(&Value::from(accessibility)),
            "wire accessibility of {source:?}",
        );
        let once = format(source);
        assert_eq!(once, expected, "format of {source:?}");
        assert_eq!(format(&once), once, "idempotent for {source:?}");
    }
}

/// Two DIFFERENT accessibility modifiers reject: tsc's parser accepts them (TS1028 is its
/// checker's), but the wire holds one `accessibility` and the printer could print only one
/// of them, so the reading fails the representability floor
/// (`docs/conformance_tsc.md` §The reject-vs-defer line). acorn-typescript and prettier
/// reject them too, with the message tsv reports.
#[test]
fn conflicting_accessibility_rejects() {
    for source in [
        "class C { public private x = 1 }",
        "class C { public static private x = 1 }",
        "class C { private readonly protected m() {} }",
    ] {
        let err = tsv_ts::__private::parse(source)
            .expect_err(&format!("expected a parse error for {source:?}"));
        let message = err.to_string();
        assert!(
            message.contains("Accessibility modifier already seen"),
            "{source:?} should report a second accessibility, got {message:?}",
        );
    }
}

/// A second `static` is not a modifier in tsc's parser (`tryParseModifier`'s
/// `hasSeenStaticModifier`): it is the member's NAME, so a key after it is a syntax error
/// (TS1434), and with nothing after it the member is a field named `static`.
#[test]
fn second_static_is_the_name() {
    for source in [
        "class C { static static x = 1 }",
        "class C { static public static x = 1 }",
        "class C { static static m() {} }",
    ] {
        assert!(
            tsv_ts::__private::parse(source).is_err(),
            "{source:?} should reject, as tsc's parser does",
        );
    }
    for (source, expected) in [
        (
            "class C {\n\tstatic static;\n}\n",
            "class C {\n\tstatic static;\n}\n",
        ),
        (
            "class C {\n\tstatic static = 1;\n}\n",
            "class C {\n\tstatic static = 1;\n}\n",
        ),
    ] {
        assert_eq!(format(source), expected, "format of {source:?}");
    }
}

/// The name guard runs on every modifier, a repeat included: a keyword with no member name
/// after it is the NAME (`readonly readonly;` is a readonly field named `readonly`), as in
/// tsc's `nextTokenCanFollowModifier`.
#[test]
fn trailing_repeat_is_the_name() {
    for (source, name) in [
        ("class C {\n\treadonly readonly;\n}\n", "readonly"),
        ("class C {\n\treadonly readonly: number;\n}\n", "readonly"),
        ("class C {\n\tpublic public = 1;\n}\n", "public"),
        ("class C {\n\tdeclare declare;\n}\n", "declare"),
        ("class C {\n\taccessor accessor;\n}\n", "accessor"),
        ("class C {\n\tasync async() {}\n}\n", "async"),
        ("class C {\n\tpublic public() {}\n}\n", "public"),
    ] {
        assert_eq!(
            first_member(source).pointer("/key/name"),
            Some(&Value::from(name)),
            "the member name of {source:?}",
        );
        assert_eq!(format(source), source, "format of {source:?}");
    }
}

/// A static block takes no modifier. tsc's parser stops its modifier run at a `static`
/// opening a block and reports any modifier ahead of it from its checker (TS1184), but the
/// wire has no field for it, so tsv rejects it (the representability floor), wherever in
/// the run the modifier stands.
#[test]
fn static_block_takes_no_modifier() {
    for source in [
        "class C { public static {} }",
        "class C { declare static {} }",
        "class C { readonly static {} }",
        "class C { override static {} }",
        "class C { accessor static {} }",
    ] {
        assert!(
            tsv_ts::__private::parse(source).is_err(),
            "{source:?} should reject",
        );
    }
}

/// The member at `/body/0/body/body/0` as the structure a reparse must keep: how many
/// members the class has, and the first one's key and modifier flags.
fn member_structure(source: &str) -> (usize, Value) {
    let json = parse_json(source);
    let members = json
        .pointer("/body/0/body/body")
        .and_then(Value::as_array)
        .expect("a class body");
    let first = &members[0];
    let structure = ["static", "readonly", "override", "accessibility", "kind"]
        .iter()
        .map(|key| {
            (
                key.to_string(),
                first.get(key).cloned().unwrap_or(Value::Null),
            )
        })
        .chain([(
            "key".to_string(),
            first.pointer("/key/name").cloned().unwrap_or(Value::Null),
        )])
        .chain([(
            "async".to_string(),
            first
                .pointer("/value/async")
                .cloned()
                .unwrap_or(Value::Null),
        )])
        .collect();
    (members.len(), Value::Object(structure))
}

/// A line break in a reordered run may only follow `static`, so the comment that holds it
/// must stay after `static` however the whitespace between `static` and the comment is
/// spelled — the lexer's whitespace class, a no-break space, an ideographic space and a
/// byte-order mark included. Where the run's end was found by an ASCII-only scan, a
/// Unicode space stopped it short, the comment fell to the pre-name emitter after the
/// LAST printed keyword, and the line-guarded keyword before the break became a field of
/// its own (`override static<NBSP>// a⏎x` → `static override // a⏎x`).
#[test]
fn unicode_whitespace_keeps_the_break_after_static() {
    for space in ['\u{a0}', '\u{3000}', '\u{feff}'] {
        for (head, tail) in [
            ("override static", "x = 1;"),
            ("readonly static", "x = 1;"),
            ("accessor static", "x = 1;"),
            ("override public static", "x = 1;"),
            ("async static", "m() {}"),
            ("@dec readonly static", "x = 1;"),
        ] {
            for comment in ["// a\n\t", "/* a\n */ "] {
                let source = format!("class C extends B {{\n\t{head}{space}{comment}{tail}\n}}\n");
                let once = format(&source);
                assert_eq!(
                    member_structure(&once),
                    member_structure(&source),
                    "{source:?} formats to {once:?}, which reparses differently",
                );
                assert_eq!(member_structure(&source).0, 1, "{source:?} is one member");
                assert_eq!(format(&once), once, "idempotent for {source:?}");
            }
        }
    }
}
