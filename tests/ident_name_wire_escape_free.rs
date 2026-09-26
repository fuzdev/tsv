// helper fns here aren't `#[test]`, so clippy.toml's allow-expect/panic-in-tests don't reach them
#![expect(clippy::expect_used, clippy::panic)]

//! The wire writer's name funnel writes a span-identity identifier name WITHOUT a JSON
//! escape scan, and this file grades the claim that licenses it on the inputs that could
//! break it.
//!
//! `write_name_field` hands the raw source slice of an `IdentName` whose `escaped` is
//! `None` straight to `JsonWriter::string_escape_free_led`. That is sound only because
//! such a slice holds no `"`, `\` or control byte: a lexed `IdentifierName` by the grammar
//! (`ID_Start` / `ID_Continue` characters, `$`, ZWNJ and ZWJ), and a Svelte directive
//! name — an attribute-name run, which may be no identifier at all (`foo-bar`) — by the
//! constructor's own byte check. A `\u`-escaped name, and a directive name holding a `"`,
//! a `\` or a control byte, carry their name as `escaped` and keep the escape scan.
//!
//! ⚠️ **A violation is quiet on the wire.** A raw `\u0061` blitted into a JSON string
//! reads back as `a` — the right *value* in the wrong *bytes* — so a value comparison
//! cannot see it, and a raw `a\b` reads back as `a` + backspace. So every case reads
//! each `"name":` field off the emitted BYTES and requires it to be `serde_json`'s own
//! spelling of its value, requires the expected name in at least as many fields as the
//! source has name positions, and — for the Svelte-synthesized `Identifier`, whose
//! directive writes a name field of its own beside it — reads that node's field
//! directly. Debug builds additionally assert the claim at the write itself. Both wire
//! variants are graded, since they are two emissions. The byte check alone cannot see
//! one class: a directive name that is itself a backslash sequence (`class:a\n`, `\b`,
//! `\f`, `\r`, `\t`, `\\`) blitted raw spells exactly the bytes `serde_json` writes for
//! some other value, so it reads as canonical — the value checks catch those.
//!
//! A non-string literal's `raw` (a number, a BigInt, `true`, `false`, `null`) takes the
//! same unscanned write, on the grammar's word that its token holds none of those bytes.
//!
//! The corpus cannot stand in: real code holds no escaped identifier and no directive
//! name with a backslash, and a formatted fixture tree holds neither in name position.

use serde_json::Value;

/// Every name position the wire writer reaches `write_name_field` from, as a template with
/// `NAME` standing in for the identifier: a reference, a binding, a member and a
/// private name, a type parameter (the mapped-type parameter is its own arm), a class
/// member key, a label and an import.
const POSITIONS: &[&str] = &[
    "NAME;\n",
    "const NAME = 1;\n",
    "let NAME: string;\n",
    "x.NAME;\n",
    "x?.NAME;\n",
    "({ NAME });\n",
    "({ NAME: 1 });\n",
    "class C {\n\t#NAME = 1;\n\tNAME() {}\n\tm() {\n\t\treturn this.#NAME;\n\t}\n}\n",
    "type T<NAME> = NAME;\n",
    "type M = { [NAME in K]: 1 };\n",
    "function f<NAME>(NAME: NAME) {}\n",
    "NAME: for (;;) break NAME;\n",
    "import NAME from 'm';\n",
    "import { NAME } from 'm';\n",
    "enum E {\n\tNAME = 1\n}\n",
];

/// Every `"name":` string field in the wire, read off the BYTES and checked there: each
/// literal must be exactly `serde_json`'s spelling of the value it decodes to, so a name
/// blitted raw anywhere in the document — a `\u0061` that reads back as `a`, a quote, a
/// control byte — fails wherever it sits. Returns the decoded values in wire order.
#[track_caller]
fn checked_name_fields(source: &str, text: &str) -> Vec<String> {
    const KEY: &str = "\"name\":\"";
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    for (at, _) in text.match_indices(KEY) {
        let open = at + KEY.len() - 1;
        let mut end = open + 1;
        while bytes[end] != b'"' {
            end += if bytes[end] == b'\\' { 2 } else { 1 };
        }
        let literal = &text[open..=end];
        let decoded: String = serde_json::from_str(literal)
            .unwrap_or_else(|e| panic!("{source:?}: {literal} is not a JSON string: {e}"));
        let canonical = serde_json::to_string(&decoded).expect("a str serializes");
        assert_eq!(
            literal, canonical,
            "{source:?}: a name field is not serde_json's spelling of its value"
        );
        out.push(decoded);
    }
    out
}

/// Assert both wire variants spell every name field canonically (see
/// [`checked_name_fields`]) and carry `decoded` in at least `hits` of them — one per
/// name position the source holds, so a position written from the wrong bytes (a
/// shifted anchor, a truncated slice) cannot hide behind another that is right.
#[track_caller]
fn assert_wire_names(source: &str, wires: [Vec<u8>; 2], decoded: &str, hits: usize) {
    for bytes in wires {
        let text = String::from_utf8(bytes)
            .unwrap_or_else(|e| panic!("wire is not UTF-8 for {source:?}: {e}"));
        let found = checked_name_fields(source, &text)
            .iter()
            .filter(|n| *n == decoded)
            .count();
        assert!(
            found >= hits,
            "{source:?}: {found} name fields read {decoded:?}, want at least {hits} — {text}"
        );
    }
}

/// Parse `source` as TypeScript at `goal` and grade both wire variants.
#[track_caller]
fn ts_holds(source: &str, goal: tsv_ts::Goal, decoded: &str, hits: usize) {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse_with_goal(source, goal, &arena)
        .unwrap_or_else(|e| panic!("parse failed for {source:?}: {e}"));
    let wires = [
        tsv_ts::convert_ast_json_bytes(&program, source),
        tsv_ts::convert_ast_json_bytes_no_locations(&program, source),
    ];
    assert_wire_names(source, wires, decoded, hits);
}

/// Parse `source` as a Svelte component and return both wire variants.
#[track_caller]
fn svelte_wires(source: &str) -> [Vec<u8>; 2] {
    let arena = bumpalo::Bump::new();
    let root = tsv_svelte::parse(source, &arena)
        .unwrap_or_else(|e| panic!("parse failed for {source:?}: {e}"));
    [
        tsv_svelte::convert_ast_json_bytes(&root, source),
        tsv_svelte::convert_ast_json_bytes_no_locations(&root, source),
    ]
}

/// Parse `source` as a Svelte component and grade both wire variants.
#[track_caller]
fn svelte_holds(source: &str, decoded: &str, hits: usize) {
    assert_wire_names(source, svelte_wires(source), decoded, hits);
}

/// The `name` of every `Identifier` node in a wire tree.
fn identifier_names(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("Identifier")
                && let Some(Value::String(name)) = map.get("name")
            {
                out.push(name.clone());
            }
            map.values().for_each(|c| identifier_names(c, out));
        }
        Value::Array(items) => items.iter().for_each(|c| identifier_names(c, out)),
        _ => {}
    }
}

/// [`svelte_holds`], plus the `Identifier` node's OWN `name` field must read `decoded`:
/// a directive or attribute writes its name beside the expression's, so a byte count
/// over all name fields would let the directive's field stand in for a wrong one.
#[track_caller]
fn svelte_identifier_holds(source: &str, decoded: &str) {
    let wires = svelte_wires(source);
    for bytes in &wires {
        let mut names = Vec::new();
        identifier_names(&tsv_debug::json::wire_value(bytes), &mut names);
        assert_eq!(
            names,
            [decoded],
            "{source:?}: the synthesized Identifier's own name field"
        );
    }
    assert_wire_names(source, wires, decoded, 2);
}

/// Drive every position with `spelled` (the source spelling) reading back as `decoded`,
/// in a `.ts` module, a `.ts` script, a Svelte `<script lang="ts">` and — for the
/// expression positions — a Svelte template expression.
#[track_caller]
fn all_positions(spelled: &str, decoded: &str) {
    for template in POSITIONS {
        let hits = template.matches("NAME").count();
        let source = template.replace("NAME", spelled);
        ts_holds(&source, tsv_ts::Goal::Module, decoded, hits);
        if !template.starts_with("import") {
            ts_holds(&source, tsv_ts::Goal::Script, decoded, hits);
        }
        svelte_holds(
            &format!("<script lang=\"ts\">\n{source}</script>\n"),
            decoded,
            hits,
        );
    }
    for template in [
        "<p>{NAME}</p>\n",
        "<p>{x.NAME}</p>\n",
        "<p title={NAME}></p>\n",
    ] {
        svelte_holds(&template.replace("NAME", spelled), decoded, 1);
    }
}

/// A plain ASCII name at every length across the writer's copy classes (one to three
/// bytes, a four-byte pair, an eight-byte pair, a sixteen-byte pair, and past them),
/// and every ASCII `IdentifierPart` class.
#[test]
fn a_raw_ascii_name_is_written_verbatim() {
    for len in 1..=40usize {
        let name = format!("a{}", "b".repeat(len - 1));
        all_positions(&name, &name);
    }
    for name in ["_", "$", "_$a0", "$$props", "aB0_$", "Z9"] {
        all_positions(name, name);
    }
}

/// A non-ASCII name — every UTF-8 encoded length, a wide character, an astral one, and
/// ZWNJ / ZWJ inside a name — at every position of a name that straddles the copy
/// classes' boundaries.
#[test]
fn a_raw_non_ascii_name_is_written_verbatim() {
    for glyph in ["é", "ϕ", "中", "𝕏", "\u{200c}", "\u{200d}"] {
        for len in 1..=20usize {
            for at in 1..=len {
                let name = format!("{}{glyph}{}", "a".repeat(at), "b".repeat(len - at));
                all_positions(&name, &name);
            }
        }
    }
    all_positions("𐊧", "𐊧");
}

/// An escaped spelling is written as its DECODED name: the raw slice holds a `\`, so it
/// must never reach the escape-free write — blitted raw, `\u0061` would read back as `a`
/// from the wrong bytes.
#[test]
fn an_escaped_name_is_written_decoded() {
    for (spelled, decoded) in [
        (r"\u0061", "a"),
        (r"\u{61}", "a"),
        (r"a\u0062c", "abc"),
        (r"\u{1D54F}", "𝕏"),
        (r"a\u200Cb", "a\u{200c}b"),
        (r"\u00e9t\u00e9", "été"),
        // A reserved word spelled with an escape is an early error the parser defers
        // (it reads the identifier `class`), so if the parser ever rejects it this
        // case fails on purpose — drop it then.
        (r"\u0063lass", "class"),
    ] {
        all_positions(spelled, decoded);
    }
}

/// Svelte synthesizes an `Identifier` from a valueless directive's own name — an HTML
/// attribute-name run, NOT an `IdentifierName`, which may hold a `\` or a non-whitespace
/// control byte (the canonical parser keeps both). That name must keep the escape scan.
#[test]
fn a_synthesized_directive_name_is_escaped() {
    for directive in ["class", "bind"] {
        for (spelled, decoded) in [
            ("a\\b", "a\\b"),
            ("a\u{1}b", "a\u{1}b"),
            ("\\", "\\"),
            ("é", "é"),
            ("value", "value"),
            ("foo-bar", "foo-bar"),
        ] {
            svelte_identifier_holds(&format!("<div {directive}:{spelled}></div>\n"), decoded);
        }
    }
    for name in ["x", "é", "$$props", "a\u{200d}b"] {
        svelte_identifier_holds(&format!("<div {{{name}}}></div>\n"), name);
    }
}

/// The same claim graded at its constructor rather than at the write: the shorthand
/// directive's synthesized `Identifier` is emitted by the Svelte writer's own escaping
/// path today, so no wire byte can see a directive name wrongly left span-identity — but
/// `escaped: None` promises every future reader of the channel a clean slice, so the
/// channel itself must say which names are not.
#[test]
fn a_directive_name_claims_span_identity_only_when_clean() {
    use tsv_svelte::ast::internal::{AttributeNode, FragmentNode};
    use tsv_ts::ast::internal::ExpressionKind;
    for directive in ["class", "bind"] {
        for (name, clean) in [
            ("a\\b", false),
            ("a\u{1}b", false),
            ("a\u{1f}b", false),
            ("\\", false),
            ("é\\b", false),
            ("ab", true),
            ("é", true),
            ("foo-bar", true),
            ("a\u{7f}b", true),
        ] {
            let source = format!("<div {directive}:{name}></div>\n");
            let arena = bumpalo::Bump::new();
            let root = tsv_svelte::parse(&source, &arena)
                .unwrap_or_else(|e| panic!("parse failed for {source:?}: {e}"));
            let Some(FragmentNode::Element(element)) = root.fragment.nodes.first() else {
                panic!("{source:?}: no element");
            };
            let expression = match element.attributes.first() {
                Some(AttributeNode::ClassDirective(d)) => d.expression,
                Some(AttributeNode::BindDirective(d)) => d.expression,
                _ => panic!("{source:?}: no directive"),
            };
            let ExpressionKind::Identifier(id) = &expression.kind else {
                panic!("{source:?}: the shorthand expression is not an Identifier");
            };
            let channel = id.ident_name();
            if clean {
                assert!(
                    channel.escaped.is_none(),
                    "{source:?}: a clean name stays span-identity"
                );
                assert_eq!(
                    channel.plain_ascii,
                    !name.bytes().any(tsv_lang::printing::is_width_relevant),
                    "{source:?}: the plain-ASCII claim"
                );
            } else {
                assert_eq!(
                    channel.escaped,
                    Some(name),
                    "{source:?}: carried as the escape hatch"
                );
            }
        }
    }
}

/// A non-string literal's `raw` takes the same unscanned write: a numeric, BigInt,
/// boolean or `null` token is escape-free by grammar. Every numeric spelling, and
/// lengths on both sides of the inline copy's width.
#[test]
fn a_non_string_literal_raw_is_written_verbatim() {
    for raw in [
        "0",
        "1",
        "10",
        "1.5",
        ".5",
        "5.",
        "1e5",
        "1E-5",
        "1e+50",
        "0x1F",
        "0X1f",
        "0b101",
        "0o17",
        "1_000_000",
        "0.000_001",
        "123n",
        "0xFFn",
        "true",
        "false",
        "null",
        "1234567890123456789012345678901",
        "12345678901234567890123456789012",
        "123456789012345678901234567890123",
        "1234567890123456789012345678901234567890n",
    ] {
        let source = format!("x = {raw};\n");
        let arena = bumpalo::Bump::new();
        let program = tsv_ts::parse(&source, &arena)
            .unwrap_or_else(|e| panic!("parse failed for {source:?}: {e}"));
        let field = format!("\"raw\":\"{raw}\"");
        for bytes in [
            tsv_ts::convert_ast_json_bytes(&program, &source),
            tsv_ts::convert_ast_json_bytes_no_locations(&program, &source),
        ] {
            let text = String::from_utf8(bytes).expect("the wire is UTF-8");
            assert!(
                text.contains(&field),
                "{source:?}: the wire does not carry {field} — {text}"
            );
        }
    }
}

/// The meta-properties build their names from spans, not tokens.
#[test]
fn a_meta_property_is_written_verbatim() {
    ts_holds(
        "function f() {\n\tnew.target;\n}\n",
        tsv_ts::Goal::Module,
        "target",
        1,
    );
    ts_holds("import.meta;\n", tsv_ts::Goal::Module, "meta", 1);
}
